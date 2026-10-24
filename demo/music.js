// Diablo 2 WASM — EXP-A1 (14b : musique par acte/zone) + ÉTAPE 14c (ambiance
// + panning stéréo). Lecteur autoportant : WebAudio, boucles sans couture
// (AudioBufferSourceNode .loop), fondu entrant/sortant ~1,5 s au changement
// de zone (jamais de coupure sèche), autoplay au PREMIER GESTE (politique
// navigateur — même pattern que demo/menu.js, commit 34ee67c), volumes
// distincts MUSIQUE / AMBIANCE / SFX persistés localStorage, bouton
// « Musique : oui/non » persistant (localStorage, clé partagée avec menu.js).
//
// 14c — couche AMBIANCE : sous la musique, boucle « scene » de SoundEnviron
// (wind, oiseaux, criquets, lave…), volume bas, fondu 1,5 s au changement de
// région, résolution PAR DATA : SoundEnv (levels.txt col. 119, valeurs
// réelles patch_levels.txt) → record SoundEnviron v1.12 → boucle extraite —
// miroir EXACT du contrat Rust `d2rules::audio_env` (golden 12 niveaux
// A1-A5, tests intégrés). 15 boucles courtes extraites de d2sfx.mpq (LECTURE
// SEULE) via tools/extract_ambiance.py vers assets/sfx/ambiance/.
//
// 14c — STÉRÉO PANNING : window.D2Audio.pan(x_screen) pan les SFX positionnels
// (bus dédié → StereoPannerNode) selon la position ÉCRAN de la source ; le
// rendu ou les hooks existants l'appellent avec le x écran de l'entité
// (canvas pleine fenêtre → pan = (x/width)×2-1, borné ±0.85).
// window.D2Audio.play(name, x_screen) joue un one-shot positionnel
// (assets/sfx/ambiance/<name>.wav — gouttes, cris, rafales… du dossier ou
// tout WAV déposé là). Jamais d'erreur JS : WAV absent = silence + info.
//
// 14c — MIXAGE : bus distincts musique / ambiance / sfx-panoramiques, chacun
// avec volume persisté (localStorage d2wasm.vol.music|amb|sfx) et un curseur
// dans le petit panneau « Audio ⚙ » de la toolbar. Le volume MUSIQUE par
// défaut reste 0.4 (EXP-A1) — le gain s'applique UNE fois au bus (l'ancien
// code cumulait par erreur master×piste = 0.4²=0.16 ; normalisé au 14c).
// Nuit/jour : horloge locale (≥20h / <6h) — D2Audio.setNight(true|false|null)
// pour forcer (null = horloge).
//
// Câblage côté game.js (3 points, légers et tous gardés par `if (window.D2Music)`) :
//   boot            -> window.D2Music.setLevel(li.key, li.name)
//   ev.LevelSwitch  -> window.D2Music.setLevel(info.key, info.name)
//   syncZoneAfterLoad -> window.D2Music.setLevel(li.key, li.name)
// setLevel met à jour MUSIQUE + AMBIANCE (aucun appel game.js de plus).
// game.html charge déjà <script src="./music.js"> avant game.js.
//
// Garde-fous E2E : la sonde de game.js compte console.error et les promesses
// non rattrapées comme erreurs -> ici uniquement console.info, et TOUTES les
// promesses sont rattrapées. Un WAV absent = silence + note info, jamais une
// erreur JS. Pas de lecture avant le premier geste utilisateur.
//
// Choix des thèmes — fondés sur les 33 fichiers RÉELS de demo/assets/music/
// (« <thème>-t2.wav », extraits des MPQ v1.12, déjà taillés pour boucler) et
// le nommage officiel de music.txt (Diablo II v1.12) :
//   Acte I   : town1 (Camp des Rousses) · wild (Landes du Sang et extérieurs)
//              · caves (grottes) · crypt (cryptes/mausolées) · monastery
//              (monastère, cathédrale, prisons, catacombes) · tristram
//   Acte II  : town2 (Lut Gholein) · desert (extérieurs) · tombs (tombeaux
//              Tal Rasha / temple du serpent) · sewer (égouts) · harem
//              (palais) · sanctuary (Sanctuaire arcanique) · valley (Canyon
//              des Magi / vallée des serpents)
//   Acte III : town3 (Quais de Kurast) · jungle (forêts/marais) · spider
//              (cavernes d'araignées) · kurast (bazar/temples) · kurastsewer
//              (égouts/temples ruinés de Kurast)
//   Acte IV  : town4 (Forteresse de Pandémonium) · mesa (marches/désespoir)
//              · lair (ville des damnés, fleuve de feu) · diablo (Sanctuaire
//              du Chaos / boss)
//   Events   : denofevilaction · bloodravenresolution · andarielaction ·
//              radamentresolution · izualaction · forgeaction ·
//              horadricaction · orbaction · taintedsunaction · mefdeathaction
//   Hors jeu : options (écran options) — non câblé ici.
//   Acte V   : aucune piste A5 extraite (siege/shire/… absentes du dossier)
//              -> zone inconnue = thème courant conservé (ni silence ni
//              erreur). L'AMBIANCE 14c couvre en revanche tout l'acte V.
'use strict';

(function () {
  const MUSIC_BASE = 'assets/music/';    // relatif à demo/ (comme les SFX du menu)
  const AMB_BASE = 'assets/sfx/ambiance/'; // 14c : boucles « scene » extraites
  const MUTE_KEY = 'd2wasm.musicMuted';  // clé partagée menu (menu.js) / jeu
  const VOL_KEYS = { music: 'd2wasm.vol.music', amb: 'd2wasm.vol.amb', sfx: 'd2wasm.vol.sfx' };
  const DEFAULTS = { music: 0.4, amb: 0.18, sfx: 0.5 }; // musique = défaut EXP-A1, ambiance basse
  const FADE = 1.5;      // fondu de transition musique (constant ~FADE/3)
  const FADE_AMB = 1.5;  // 14c : fondu ambiance au changement de région (1-2 s)
  const PAN_MAX = 0.85;  // 14c : borné pour garder la source audible des deux côtés

  // --- mapping DATA-DRIVEN zone -> thème musique (inchangé EXP-A1) ------------
  const BY_LEVEL = {
    town: 'town1',          // Camp des Rousses (Rogue Encampment)
    bloodmoor: 'wild',      // Landes du Sang (Acte I, extérieur)
    denofevil: 'denofevilaction',
    coldplains: 'wild', stonyfield: 'wild', darkwood: 'wild', blackmarsh: 'wild',
    tamoe: 'wild', tamoehighland: 'wild', bloodmoorwarp: 'wild',
    cave: 'caves', hole: 'caves',
    crypt: 'crypt', mausoleum: 'crypt',
    monasterygate: 'monastery', barracks: 'monastery', jail: 'monastery',
    cathedral: 'monastery', catacombs: 'monastery', andariel: 'andarielaction',
    tristram: 'tristram',
    lutgholein: 'town2',
    rockywaste: 'desert', dryhills: 'desert', faroasis: 'desert',
    lostcity: 'desert', ancienttunnels: 'desert',
    sewers: 'sewer', sewersa2: 'sewer',
    clawviper: 'tombs', talrasha: 'tombs',
    harem: 'harem', palace: 'harem',
    arcane: 'sanctuary', arcaneanctuary: 'sanctuary', sanctuary: 'sanctuary',
    canyon: 'valley', magi: 'valley',
    kurastdocks: 'town3',
    spiderforest: 'jungle', greatmarsh: 'jungle', flayerjungle: 'jungle',
    spidercavern: 'spider', arachnid: 'spider', spider: 'spider',
    kurastbazaar: 'kurast', templedistrict: 'kurast', travincal: 'kurast',
    kurastsewer: 'kurastsewer', ruinedtemple: 'kurastsewer', disusedfane: 'kurastsewer',
    pandemonium: 'town4', pandemoniumfortress: 'town4',
    outersteppes: 'mesa', plainsofdespair: 'mesa', mesa: 'mesa',
    cityofthedamned: 'lair', riverofflame: 'lair', lair: 'lair',
    chaossanctuary: 'diablo', diablo: 'diablo',
  };

  const BY_NAME = [
    [/blood\s?raven|ravenne/i, 'bloodravenresolution'],
    [/andariel/i, 'andarielaction'],
    [/radament/i, 'radamentresolution'],
    [/izual/i, 'izualaction'],
    [/den\s?of\s?evil|antre du mal/i, 'denofevilaction'],
    [/hellforge|forge de l'enfer/i, 'forgeaction'],
    [/horadric|bâton horadrim/i, 'horadricaction'],
    [/tainted\s?sun|soleil (noirci|gâté)/i, 'taintedsunaction'],
    [/diablo|sanctuaire du chaos|chaos/i, 'diablo'],
    [/tristram/i, 'tristram'],
    [/spider\s?cav|arachnid|caverne d'araign/i, 'spider'],
    [/monast|cathédrale|cathedral|catacomb|caserne|barrack|prison|jail|geôl/i, 'monastery'],
    [/crypte|crypt|mausol/i, 'crypt'],
    [/caverne|grotte|\bcave|\bhole\b|\btrou\b/i, 'caves'],
    [/rousses|rogue/i, 'town1'],
    [/lut gholein/i, 'town2'],
    [/harem|palais|palace|sérail|serail/i, 'harem'],
    [/arcane|arcanique/i, 'sanctuary'],
    [/kurast.*(sewer|égout|egout)|(sewer|égout|egout).*kurast/i, 'kurastsewer'],
    [/égout|egout|sewer/i, 'sewer'],
    [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, 'tombs'],
    [/maggot|larve/i, 'desert'],
    [/canyon|vallée des|valley of/i, 'valley'],
    [/désert|desert|waste|dry\s?hills|oasis|lost\s?city|cité perdue|tunnels/i, 'desert'],
    [/quai|dock/i, 'town3'],
    [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche|spider/i, 'jungle'],
    [/kurast/i, 'kurast'],
    [/pand[ée]monium/i, 'town4'],
    [/mesa|d[ée]sespoir|despair|marches|steppes/i, 'mesa'],
    [/damn[ée]s|damned|flamme|flame|fleuve|river/i, 'lair'],
    [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, 'wild'],
    [/harrogath|arreat|worldstone|baal|acte\s*v|act\s*5/i, null],
  ];

  function themeFor(key, name) {
    if (key && Object.prototype.hasOwnProperty.call(BY_LEVEL, key)) return BY_LEVEL[key] || null;
    if (name) {
      for (const [re, th] of BY_NAME) {
        if (re.test(name)) return th; // peut être null (acte V, etc.)
      }
    }
    return null;
  }

  // --- 14c : région -> AMBIANCE (miroir du contrat Rust d2rules::audio_env) ---
  // SoundEnv = valeurs RÉELLES de `SoundEnv` (levels.txt col. 119) pour les
  // clés démo (patch_levels.txt) ; valeur = [boucle jour, boucle nuit] parmi
  // les 15 fichiers extraits de d2sfx.mpq. Fallbacks nuit résolus en data
  // (57→56, 60→59, 69→68 — même table que audio_env::AMBIENCE_LOOPS).
  // Généré depuis SoundEnviron.txt v1.12 + sounds.txt (patch_d2.mpq).
  const ENV_STEMS = {
    0: [null, null],
    1: ['scene_wilderness_day', 'scene_wilderness_night'],
    2: ['scene_wilderness_day', 'scene_wilderness_night'],
    3: ['scene_wilderness_day', 'scene_wilderness_night'],
    4: ['scene_cave', 'scene_cave'],
    5: ['scene_crypt', 'scene_crypt'],
    6: ['scene_monastery', 'scene_monastery'],
    7: ['scene_monastery', 'scene_monastery'],
    8: ['scene_catacombs', 'scene_catacombs'],
    9: ['scene_cathedral', 'scene_cathedral'],
    10: ['scene_catacombs', 'scene_catacombs'],
    11: ['scene_catacombs', 'scene_catacombs'],
    12: ['scene_desert_day', 'scene_desert_day'],
    13: ['scene_desert_day', 'scene_desert_day'],
    14: ['scene_desert_day', 'scene_desert_day'],
    15: ['scene_sewer', 'scene_sewer'],
    16: ['scene_harem', 'scene_harem'],
    17: ['scene_harem', 'scene_harem'],
    18: ['scene_catacombs', 'scene_catacombs'],
    19: ['scene_cave', 'scene_cave'],
    20: ['scene_tomb', 'scene_tomb'],
    21: ['scene_tomb', 'scene_tomb'],
    22: ['scene_mesa', 'scene_mesa'],
    23: ['scene_town_3_day', 'scene_town_3_day'],
    24: ['scene_jungle_day', 'scene_jungle_day'],
    25: ['scene_town_3_day', 'scene_jungle_day'],
    26: ['scene_cave', 'scene_cave'],
    27: ['scene_sewer', 'scene_sewer'],
    28: ['scene_cathedral', 'scene_cathedral'],
    29: ['scene_cave', 'scene_cave'],
    30: ['scene_catacombs', 'scene_catacombs'],
    31: ['scene_mesa', 'scene_mesa'],
    32: ['scene_mesa', 'scene_mesa'],
    33: ['scene_lava', 'scene_lava'],
    34: ['scene_lava', 'scene_lava'],
    35: ['scene_wilderness_day', 'scene_wilderness_night'],
    36: ['scene_wilderness_day', 'scene_wilderness_night'],
    37: ['scene_wilderness_day', 'scene_wilderness_night'],
    38: ['scene_wilderness_day', 'scene_wilderness_night'],
    39: ['scene_cave', 'scene_cave'],
    40: ['scene_cave', 'scene_cave'],
    41: ['scene_cave', 'scene_cave'],
    42: ['scene_monastery', 'scene_monastery'],
    43: ['scene_monastery', 'scene_monastery'],
    44: ['scene_catacombs', 'scene_catacombs'],
    45: ['scene_catacombs', 'scene_catacombs'],
    46: ['scene_mesa', 'scene_mesa'],
    47: ['scene_catacombs', 'scene_catacombs'],
    48: ['scene_catacombs', 'scene_catacombs'],
    49: ['scene_cathedral', 'scene_cathedral'],
  };

  // SoundEnv par clé de niveau (valeurs réelles patch_levels.txt) — sinon
  // heuristique par nom (fr+en), sinon null = ambiance courante conservée.
  const ENV_BY_LEVEL = {
    town: 1, bloodmoor: 2, denofevil: 2, coldplains: 2, stonyfield: 2,
    darkwood: 2, blackmarsh: 2, tamoe: 2, graveyard: 2,
    cave: 4, hole: 4, crypt: 5, mausoleum: 5,
    monasterygate: 6, monastery: 6, courtyard: 7, barracks: 8, jail: 8,
    cathedral: 9, catacombs: 10, andariel: 11,
    tristram: 3, lutgholein: 12,
    rockywaste: 13, dryhills: 13, faroasis: 13, lostcity: 13, ancienttunnels: 13,
    canyon: 14, magi: 14,
    sewers: 15, sewersa2: 15, harem: 16, palace: 16, basement: 18,
    clawviper: 19, talrasha: 19, maggotlair: 20, duriel: 21,
    arcane: 22, arcaneanctuary: 22, sanctuary: 22,
    kurastdocks: 23, spiderforest: 24, greatmarsh: 24, flayerjungle: 24,
    spidercavern: 29, arachnid: 29, spider: 29,
    kurastbazaar: 25, templedistrict: 25, travincal: 25,
    kurastsewer: 26, ruinedtemple: 26, disusedfane: 26,
    dungeon: 27, temple: 28, mephisto: 30,
    pandemonium: 31, pandemoniumfortress: 31,
    outersteppes: 32, plainsofdespair: 32, mesa: 32,
    cityofthedamned: 33, riverofflame: 33, lair: 33,
    chaossanctuary: 34, diablo: 34,
    harrogath: 36, siege: 37, barricade: 37, barricadesnow: 38,
    icecave: 39, templeentrance: 43, xtemple: 44, templeboss: 45,
    hell5: 46, baaltemple: 47, throne: 48, worldstone: 49,
  };

  // Ordre du spécifique au générique (comme BY_NAME).
  const ENV_BY_NAME = [
    [/tristram/i, 3],
    [/town|ville|\bcamp\b|rousses|rogue/i, 1], // TOWN_1 : extérieur « wilderness »
    [/lut gholein/i, 12],                       // TOWN_2 : ambiance désert (réel)
    [/spider\s?cav|arachnid|caverne d'araign/i, 29],
    [/catacomb/i, 10], [/cathédrale|cathedral/i, 9], [/monast|caserne|barrack|prison|jail|geôl/i, 6],
    [/crypte|crypt|mausol/i, 5],
    [/caverne|grotte|\bcave\b|\bhole\b|\btrou\b/i, 4],
    [/harem|palais|palace|sérail|serail/i, 16],
    [/égout|egout|sewer/i, 15],
    [/maggot|larve/i, 20], [/duriel/i, 21],
    [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, 19],
    [/arcane|arcanique/i, 22],
    [/quai|dock/i, 23],
    [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche/i, 24],
    [/kurast/i, 25],
    [/donjon|dungeon/i, 27],
    [/temple/i, 28],
    [/lave|lava|flamme|flame|fleuve|river|damn[ée]s|damned/i, 33],
    [/mesa|marches|steppes|d[ée]sespoir|despair/i, 32],
    [/glace|ice/i, 39],
    [/enfer|hell/i, 46],
    [/pierre du monde|world\s?stone/i, 49],
    [/baal/i, 47],
    [/arreat|sommet|mountain/i, 42],
    [/désert|desert|waste|oasis|lost\s?city|cité perdue|tunnels/i, 13],
    [/pand[ée]monium/i, 31],
    [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, 2],
  ];

  function envFor(key, name) {
    if (key && Object.prototype.hasOwnProperty.call(ENV_BY_LEVEL, key)) return ENV_BY_LEVEL[key];
    if (name) {
      for (const [re, env] of ENV_BY_NAME) {
        if (re.test(name)) return env;
      }
    }
    return null;
  }

  let nightOverride = null; // true/false forcé, null = horloge locale
  function isNight() {
    if (nightOverride !== null) return nightOverride;
    const h = new Date().getHours();
    return h >= 20 || h < 6;
  }

  // --- état / lecture ---------------------------------------------------------
  let actx = null;          // AudioContext créé AU PREMIER GESTE seulement
  let muteBus = null;       // 14c : coupure globale (musique+ambiance+sfx)
  let musicBus = null;      // gain musique (volume persisté)
  let ambBus = null;        // 14c : gain ambiance (volume persisté, BAS)
  let sfxBus = null;        // 14c : gain SFX positionnels
  let panNode = null;       // 14c : StereoPannerNode (fallback gain = pas de pan)
  let panValue = 0;
  let now = null;           // { theme, src, gain } — piste musique en cours
  let ambNow = null;        // 14c : { stem, src, gain } — boucle ambiance en cours
  let wanted = 'town1';     // thème musique demandé (défaut : ville de départ)
  let wantedAmb = null;     // 14c : stem ambiance demandé
  let currentEnv = null;    // 14c : SoundEnv courant (bascule nuit/jour)
  let started = false;      // premier geste consommé
  let muted = false;
  try { muted = localStorage.getItem(MUTE_KEY) === '1'; } catch (e) { /* stockage indispo */ }
  let epoch = 0;            // garde anti-course entre transitions MUSIQUE
  let ambEpoch = 0;         // 14c : idem AMBIANCE (transitions indépendantes)
  const buffers = new Map();     // thème -> Promise<AudioBuffer|null> (échec mis en cache)
  const ambBuffers = new Map();  // 14c : stem -> Promise<AudioBuffer|null>
  const volumes = loadVolumes();

  function clamp01(v) { return Math.min(1, Math.max(0, Number(v) || 0)); }

  function loadVolumes() {
    const out = { ...DEFAULTS };
    try {
      for (const k of Object.keys(VOL_KEYS)) {
        const raw = localStorage.getItem(VOL_KEYS[k]);
        if (raw !== null && raw !== '') out[k] = clamp01(raw);
      }
    } catch (e) { /* stockage indispo */ }
    return out;
  }

  function saveVolume(kind) {
    try { localStorage.setItem(VOL_KEYS[kind], String(volumes[kind])); } catch (e) { /* stockage indispo */ }
  }

  function bufferOf(theme) {
    if (!buffers.has(theme)) {
      buffers.set(theme, (async () => {
        const r = await fetch(`${MUSIC_BASE}${theme}-t2.wav`);
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const bytes = await r.arrayBuffer();
        return actx.decodeAudioData(bytes); // boucle sans couture : buffer entier + .loop
      })().catch((e) => {
        // fichier absent/corrompu : silence propre, JAMAIS d'erreur JS
        console.info(`[musique] ${theme} indisponible (${e && e.message ? e.message : e}) — silence.`);
        return null;
      }));
    }
    return buffers.get(theme);
  }

  function ambienceBufferOf(stem) {
    if (!ambBuffers.has(stem)) {
      ambBuffers.set(stem, (async () => {
        const r = await fetch(`${AMB_BASE}${stem}.wav`);
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const bytes = await r.arrayBuffer();
        return actx.decodeAudioData(bytes); // boucles « scene » D2 (Loop=1) : .loop sans couture
      })().catch((e) => {
        console.info(`[ambiance] ${stem} indisponible (${e && e.message ? e.message : e}) — silence.`);
        return null;
      }));
    }
    return ambBuffers.get(stem);
  }

  function stopCurrent(fadeOut = 0.5) {
    if (!now || !actx) return;
    const old = now;
    now = null;
    try {
      old.gain.gain.setTargetAtTime(0, actx.currentTime, fadeOut / 3);
      old.src.stop(actx.currentTime + fadeOut + 0.1); // fondu sortant, pas de coupure sèche
    } catch (e) { /* source déjà arrêtée */ }
  }

  function stopAmbience(fadeOut = FADE_AMB) {
    if (!ambNow || !actx) return;
    const old = ambNow;
    ambNow = null;
    try {
      old.gain.gain.setTargetAtTime(0, actx.currentTime, fadeOut / 3);
      old.src.stop(actx.currentTime + fadeOut + 0.1);
    } catch (e) { /* source déjà arrêtée */ }
  }

  async function apply() {
    if (!started || !actx) return;
    const my = ++epoch;
    if (muted || !wanted) { stopCurrent(); applyAmbience(); return; }
    if (now && now.theme === wanted) return; // déjà en place
    const buf = await bufferOf(wanted);
    if (my !== epoch) return; // une transition plus récente a pris le dessus
    if (!buf || muted) return;
    // fondu sortant du thème précédent PENDANT l'entrée du nouveau (crossfade)
    stopCurrent(FADE);
    try {
      const src = actx.createBufferSource();
      src.buffer = buf;
      src.loop = true; // les -t2 sont taillés pour boucler -> couture invisible
      const g = actx.createGain();
      g.gain.value = 0;
      src.connect(g); g.connect(musicBus);
      src.start();
      g.gain.setTargetAtTime(1, actx.currentTime, FADE / 3); // fondu entrant ~1,5 s (le bus porte le volume)
      src.onended = () => { if (now && now.src === src) now = null; };
      now = { theme: wanted, src, gain: g };
    } catch (e) {
      console.info(`[musique] lecture impossible (${e && e.message ? e.message : e}).`);
    }
  }

  // 14c : couche AMBIANCE en boucle SOUS la musique (bus séparé, volume bas,
  // fondu 1-2 s au changement de région, jamais de coupure sèche).
  async function applyAmbience() {
    if (!started || !actx) return;
    const my = ++ambEpoch;
    if (muted || !wantedAmb) { stopAmbience(); return; }
    if (ambNow && ambNow.stem === wantedAmb) return;
    const buf = await ambienceBufferOf(wantedAmb);
    if (my !== ambEpoch) return; // transition plus récente (anti-course)
    if (!buf || muted) return;
    stopAmbience(FADE_AMB); // fondu sortant pendant l'entrée du nouveau
    try {
      const src = actx.createBufferSource();
      src.buffer = buf;
      src.loop = true;
      const g = actx.createGain();
      g.gain.value = 0;
      src.connect(g); g.connect(ambBus); // large, NON panné : le décor enveloppe
      src.start();
      g.gain.setTargetAtTime(1, actx.currentTime, FADE_AMB / 3);
      src.onended = () => { if (ambNow && ambNow.src === src) ambNow = null; };
      ambNow = { stem: wantedAmb, src, gain: g };
    } catch (e) {
      console.info(`[ambiance] lecture impossible (${e && e.message ? e.message : e}).`);
    }
  }

  function unlock() {
    if (started) return;
    started = true;
    try {
      actx = new (window.AudioContext || window.webkitAudioContext)();
      // graphe : [musique|ambiance|sfx->pan] -> muteBus -> destination
      muteBus = actx.createGain();
      muteBus.gain.value = muted ? 0 : 1;
      muteBus.connect(actx.destination);
      musicBus = actx.createGain();
      musicBus.gain.value = volumes.music;
      musicBus.connect(muteBus);
      ambBus = actx.createGain();
      ambBus.gain.value = volumes.amb;
      ambBus.connect(muteBus);
      sfxBus = actx.createGain();
      sfxBus.gain.value = volumes.sfx;
      panNode = actx.createStereoPanner ? actx.createStereoPanner() : actx.createGain();
      if (panNode.pan) panNode.pan.value = panValue; // fallback GainNode : pas de pan, jamais d'erreur
      sfxBus.connect(panNode); panNode.connect(muteBus);
      if (actx.state === 'suspended') actx.resume().catch(() => {});
    } catch (e) {
      console.info('[musique] WebAudio indisponible — silence.');
      return;
    }
    void apply().catch(() => {}); // promesse toujours rattrapée (sonde E2E)
  }

  // --- API publique D2Music (compat 14b) --------------------------------------
  // Résolution pure région → stem d'ambiance (jour/nuit selon l'horloge ou
  // l'override) — même contrat que d2rules::audio_env::ambience. Utilisée par
  // setLevel et exposée pour les tests.
  function ambianceFor(key, name) {
    const env = envFor(key, name);
    if (env === null) return null;
    const stems = ENV_STEMS[env];
    return stems ? (isNight() ? stems[1] : stems[0]) : null;
  }

  function setLevel(key, name) {
    const th = themeFor(key, name);
    if (th === null) {
      if (key || name) console.info(`[musique] zone sans thème dédié (${key || '?'}) — thème courant conservé.`);
    } else if (th !== wanted) {
      wanted = th;
      void apply().catch(() => {});
    }
    // 14c : même appel (hooks existants) met à jour l'ambiance — aucune
    // ligne de plus dans game.js.
    const stem = ambianceFor(key, name);
    if (!stem) return;
    currentEnv = envFor(key, name); // retenu pour le basculement nuit/jour (setNight)
    if (stem !== wantedAmb) {
      wantedAmb = stem;
      void applyAmbience().catch(() => {});
    }
  }

  function setMuted(m) {
    muted = !!m;
    try { localStorage.setItem(MUTE_KEY, muted ? '1' : '0'); } catch (e) { /* stockage indispo */ }
    void apply().catch(() => {});
    applyMuteVisual();
  }

  // --- API publique D2Audio (NOUVEAU 14c) -------------------------------------
  // pan(x_screen, refWidth) : pan les SFX selon la position ÉCRAN de la
  // source — le rendu (ou un hook) appelle au tick avec le x de l'entité.
  // Retourne la valeur normalisée appliquée (utile aux tests).
  function pan(xScreen, refWidth) {
    const w = Number(refWidth) > 0 ? Number(refWidth) : (window.innerWidth || 1280);
    const n = w > 0 ? (Number(xScreen) / w) * 2 - 1 : 0;
    panValue = Math.max(-PAN_MAX, Math.min(PAN_MAX, isFinite(n) ? n : 0));
    if (actx && panNode) {
      try {
        const t = actx.currentTime;
        if (panNode.pan) panNode.pan.setTargetAtTime(panValue, t, 0.05);
      } catch (e) { /* pan indisponible (fallback gain) : on garde la valeur */ }
    }
    return panValue;
  }

  // play(name, x_screen) : one-shot positionnel depuis assets/sfx/ambiance/
  // (goutte, cri, rafale… ou tout WAV déposé là). x_screen fourni => pan mis
  // à jour pour CE son. Silence total si muet ou avant le premier geste.
  async function play(name, xScreen) {
    if (!started || !actx || muted || !name) return null;
    if (typeof xScreen === 'number') pan(xScreen);
    let buf = null;
    try { buf = await ambienceBufferOf(name); } catch (e) { return null; }
    if (!buf || muted) return null;
    try {
      const src = actx.createBufferSource();
      src.buffer = buf;
      const g = actx.createGain();
      g.gain.value = 1;
      src.connect(g); g.connect(sfxBus); // bus SFX -> panner -> muteBus
      src.start();
      return src;
    } catch (e) {
      console.info(`[sfx] ${name} non joué (${e && e.message ? e.message : e}).`);
      return null;
    }
  }

  function setVolume(kind, v) {
    if (!Object.prototype.hasOwnProperty.call(volumes, kind)) return volumes[kind] || 0;
    volumes[kind] = clamp01(v);
    saveVolume(kind);
    if (actx) {
      const bus = kind === 'music' ? musicBus : kind === 'amb' ? ambBus : sfxBus;
      if (bus) bus.gain.setTargetAtTime(volumes[kind], actx.currentTime, 0.05);
    }
    return volumes[kind];
  }

  function setNight(n) {
    nightOverride = typeof n === 'boolean' ? n : null;
    if (currentEnv === null) return;
    const stems = ENV_STEMS[currentEnv];
    const stem = stems ? (isNight() ? stems[1] : stems[0]) : null;
    if (stem && stem !== wantedAmb) { wantedAmb = stem; void applyAmbience().catch(() => {}); }
  }

  window.D2Music = {
    setLevel,        // (key, name) -> musique + ambiance de la zone (fondu doux)
    setMuted,        // (bool) -> mute persistant (clé partagée avec menu.js)
    toggle: () => setMuted(!muted),
    theme: () => wanted,
    isMuted: () => muted,
  };

  window.D2Audio = {
    pan,                                     // (x_screen, refW?) -> [-0.85..0.85]
    play,                                    // (name, x_screen?) -> one-shot panné
    setAmbience: (stem) => {                 // override manuel (debug/tests)
      if (!stem || stem === wantedAmb) return;
      wantedAmb = stem;
      void applyAmbience().catch(() => {});
    },
    setNight,                                // true|false|null (null = horloge)
    ambianceFor,                             // (key, name) -> stem jour/nuit résolu
    setVolume,                               // ('music'|'amb'|'sfx', 0..1)
    volume: (k) => volumes[k] || 0,
    panValue: () => panValue,
    ambient: () => ambNow ? ambNow.stem : null,
  };

  // --- boutons toolbar ---------------------------------------------------------
  let btn = null;
  let panel = null;
  const SLIDERS = [
    ['music', 'Musique'], ['amb', 'Ambiance'], ['sfx', 'Effets'],
  ];
  function applyMuteVisual() {
    if (btn) btn.textContent = muted ? 'Musique : non' : 'Musique : oui';
    if (panel) {
      for (const [, lab, range] of panel.rows || []) {
        range.disabled = muted; // muet : curseurs grisés (rappel visuel)
        lab.style.opacity = muted ? '0.5' : '1';
      }
    }
  }
  function updateBtn() {
    if (btn) btn.textContent = muted ? 'Musique : non' : 'Musique : oui';
  }
  function buildPanel() {
    panel = document.createElement('div');
    panel.id = 'audio-mix';
    panel.style.cssText = 'display:none;position:fixed;right:10px;bottom:10px;z-index:60;' +
      'background:rgba(20,17,10,.94);border:1px solid #5a3c1e;padding:10px 12px;' +
      'font:11px/1.6 monospace;color:#d0b070;min-width:190px;';
    const title = document.createElement('div');
    title.textContent = 'MIXAGE AUDIO';
    title.style.cssText = 'color:#ffe9ad;margin-bottom:6px;letter-spacing:1px;';
    panel.appendChild(title);
    panel.rows = [];
    for (const [kind, label] of SLIDERS) {
      const lab = document.createElement('label');
      lab.style.cssText = 'display:flex;align-items:center;gap:6px;';
      lab.textContent = label + ' ';
      const range = document.createElement('input');
      range.type = 'range';
      range.min = '0'; range.max = '100'; range.step = '1';
      range.value = String(Math.round(volumes[kind] * 100));
      range.style.cssText = 'flex:1;accent-color:#c7a45a;';
      range.addEventListener('input', () => {
        setVolume(kind, Number(range.value) / 100); // persisté + gain live
      });
      lab.appendChild(range);
      panel.appendChild(lab);
      panel.rows.push([kind, lab, range]);
    }
    const hint = document.createElement('div');
    hint.textContent = 'la nuit tombe après 20 h (D2Audio.setNight)';
    hint.style.cssText = 'margin-top:6px;opacity:.6;';
    panel.appendChild(hint);
    document.body.appendChild(panel);
  }
  function initButton() {
    try {
      btn = document.createElement('button');
      btn.id = 'btn-music';
      btn.title = 'Musique par zone (fondu doux au changement — EXP-A1)';
      btn.addEventListener('click', (ev) => {
        ev.stopPropagation();
        unlock(); // le clic EST un geste : autorise aussi le (re)démarrage
        setMuted(!muted);
      });
      const mix = document.createElement('button');
      mix.id = 'btn-audio';
      mix.title = 'Volumes : musique / ambiance / effets (persistés)';
      mix.textContent = 'Audio \u2699';
      mix.addEventListener('click', (ev) => {
        ev.stopPropagation();
        unlock(); // geste = autorise la lecture aussi
        if (!panel) buildPanel();
        const open = panel.style.display !== 'none';
        panel.style.display = open ? 'none' : 'block';
      });
      const ref = document.getElementById('btn-sound');
      if (ref && ref.parentNode) {
        ref.parentNode.insertBefore(btn, ref);
        ref.parentNode.insertBefore(mix, btn);
      } else {
        document.body.appendChild(mix);
        document.body.appendChild(btn);
      }
      updateBtn();
    } catch (e) { /* pas de bouton : le clavier/jeu continuent de fonctionner */ }  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initButton, { once: true });
  } else {
    initButton();
  }

  // autoplay au premier geste (politique navigateur) — comme menu.js
  window.addEventListener('pointerdown', unlock);
  window.addEventListener('keydown', unlock);
})();
