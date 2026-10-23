// Diablo 2 WASM — EXP-A1 : musique par acte/zone (docs/parity/finish-experience.md).
// Lecteur autoportant : WebAudio, boucles sans couture (AudioBufferSourceNode
// .loop), fondu entrant/sortant ~1,5 s au changement de zone (jamais de
// coupure sèche), autoplay au PREMIER GESTE (politique navigateur — même
// pattern que demo/menu.js, commit 34ee67c), volume distinct des SFX de
// game.js (contexte + GainNode séparés), bouton « Musique : oui/non » dans
// la toolbar, mute persistant (localStorage, clé partagée avec menu.js).
//
// Câblage côté game.js (3 points, légers et tous gardés par `if (window.D2Music)`) :
//   boot            -> window.D2Music.setLevel(li.key, li.name)
//   ev.LevelSwitch  -> window.D2Music.setLevel(info.key, info.name)
//   syncZoneAfterLoad -> window.D2Music.setLevel(li.key, li.name)
// game.html ajoute simplement <script src="./music.js"> avant game.js.
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
//              -> zone inconnue = thème courant conservé (ni silence ni erreur).
'use strict';

(function () {
  const MUSIC_BASE = 'assets/music/'; // relatif à demo/ (comme les SFX du menu)
  const MUTE_KEY = 'd2wasm.musicMuted'; // clé partagée menu (menu.js) / jeu
  const VOLUME = 0.4;   // volume musique par défaut (mission EXP-A1), distinct des SFX
  const FADE = 1.5;     // fondu de transition entre zones (constant de temps ~FADE/3)

  // --- mapping DATA-DRIVEN zone -> thème -------------------------------------
  // 1) par clé exacte de niveau (manifest.assets/manifest.json — aujourd'hui
  //    town + bloodmoor ; clés futures anticipées au format music.txt) ;
  // 2) sinon par mots-clés du nom de zone (fr+en) ;
  // 3) sinon null = conserver le thème courant.
  const BY_LEVEL = {
    // clés réelles du manifest actuel
    town: 'town1',          // Camp des Rousses (Rogue Encampment)
    bloodmoor: 'wild',      // Landes du Sang (Acte I, extérieur)
    // clés plausibles des prochains niveaux v1.12 (anticipation inoffensive :
    // une clé jamais rencontrée ne coûte rien)
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

  // Première règle dont le nom de zone (levelInfo()/LevelSwitch `name`,
  // anglais ou français) matche gagne. L'ordre va du plus spécifique au plus
  // générique (« Blood Moor » doit tomber sur wild, pas sur bloodraven ;
  // « Spider Forest » sur jungle, « Spider Cavern » sur spider).
  const BY_NAME = [
    // events/boss nommés (thèmes d'action réels du dossier)
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
    // cavernes d'araignées AVANT la règle générique « cave » (« Spider Cavern »)
    [/spider\s?cav|arachnid|caverne d'araign/i, 'spider'],
    // Acte I — structures
    [/monast|cathédrale|cathedral|catacomb|caserne|barrack|prison|jail|geôl/i, 'monastery'],
    [/crypte|crypt|mausol/i, 'crypt'],
    // \bhole\b : évite « Lut GHolein » (faux positif) tout en couvrant « The Hole »
    [/caverne|grotte|\bcave|\bhole\b|\btrou\b/i, 'caves'],
    [/rousses|rogue/i, 'town1'],
    // Acte II
    [/lut gholein/i, 'town2'],
    [/harem|palais|palace|sérail|serail/i, 'harem'],
    [/arcane|arcanique/i, 'sanctuary'],
    [/kurast.*(sewer|égout|egout)|(sewer|égout|egout).*kurast/i, 'kurastsewer'],
    [/égout|egout|sewer/i, 'sewer'],
    [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, 'tombs'],
    [/maggot|larve/i, 'desert'], // repaire des larves : usage du thème désert (fichier réel disponible)
    [/canyon|vallée des|valley of/i, 'valley'],
    [/désert|desert|waste|dry\s?hills|oasis|lost\s?city|cité perdue|tunnels/i, 'desert'],
    // Acte III
    [/quai|dock/i, 'town3'],
    [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche|spider/i, 'jungle'],
    [/kurast/i, 'kurast'],
    // Acte IV
    [/pand[ée]monium/i, 'town4'],
    [/mesa|d[ée]sespoir|despair|marches|steppes/i, 'mesa'],
    [/damn[ée]s|damned|flamme|flame|fleuve|river/i, 'lair'],
    // Acte I — extérieurs génériques (après les règles spécifiques)
    [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, 'wild'],
    // Acte V : aucune piste extraite -> null = thème courant conservé
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

  // --- état / lecture ---------------------------------------------------------
  let actx = null;        // AudioContext créé AU PREMIER GESTE seulement
  let master = null;      // bus de gain (volume musique, indépendant des SFX)
  let now = null;         // { theme, src, gain } — piste en cours
  let wanted = 'town1';   // thème demandé (défaut : ville de départ, camp/town)
  let started = false;    // premier geste consommé
  let muted = false;
  try { muted = localStorage.getItem(MUTE_KEY) === '1'; } catch (e) { /* stockage indispo */ }
  let epoch = 0;          // garde anti-course entre transitions asynchrones
  const buffers = new Map(); // thème -> Promise<AudioBuffer|null> (échec mis en cache)

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

  function stopCurrent(fadeOut = 0.5) {
    if (!now || !actx) return;
    const old = now;
    now = null;
    try {
      old.gain.gain.setTargetAtTime(0, actx.currentTime, fadeOut / 3);
      old.src.stop(actx.currentTime + fadeOut + 0.1); // fondu sortant, pas de coupure sèche
    } catch (e) { /* source déjà arrêtée */ }
  }

  async function apply() {
    if (!started || !actx) return;
    const my = ++epoch;
    if (muted || !wanted) { stopCurrent(); return; }
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
      src.connect(g); g.connect(master);
      src.start();
      g.gain.setTargetAtTime(VOLUME, actx.currentTime, FADE / 3); // fondu entrant ~1,5 s
      src.onended = () => { if (now && now.src === src) now = null; };
      now = { theme: wanted, src, gain: g };
    } catch (e) {
      console.info(`[musique] lecture impossible (${e && e.message ? e.message : e}).`);
    }
  }

  function unlock() {
    if (started) return;
    started = true;
    try {
      actx = new (window.AudioContext || window.webkitAudioContext)();
      master = actx.createGain();
      master.gain.value = VOLUME;
      master.connect(actx.destination);
      if (actx.state === 'suspended') actx.resume().catch(() => {});
    } catch (e) {
      console.info('[musique] WebAudio indisponible — silence.');
      return;
    }
    void apply().catch(() => {}); // promesse toujours rattrapée (sonde E2E)
  }

  // --- API publique ------------------------------------------------------------
  function setLevel(key, name) {
    const th = themeFor(key, name);
    if (th === null) {
      if (key || name) console.info(`[musique] zone sans thème dédié (${key || '?'}) — thème courant conservé.`);
      return;
    }
    if (th === wanted) return;
    wanted = th;
    void apply().catch(() => {});
  }

  function setMuted(m) {
    muted = !!m;
    try { localStorage.setItem(MUTE_KEY, muted ? '1' : '0'); } catch (e) { /* stockage indispo */ }
    void apply().catch(() => {});
    updateBtn();
  }

  // --- bouton toolbar (« Son : oui » des SFX existe déjà ; musique distincte) --
  let btn = null;
  function updateBtn() {
    if (btn) btn.textContent = muted ? 'Musique : non' : 'Musique : oui';
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
      const ref = document.getElementById('btn-sound');
      if (ref && ref.parentNode) ref.parentNode.insertBefore(btn, ref);
      else document.body.appendChild(btn);
      updateBtn();
    } catch (e) { /* pas de bouton : le clavier/jeu continuent de fonctionner */ }
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initButton, { once: true });
  } else {
    initButton();
  }

  // autoplay au premier geste (politique navigateur) — comme menu.js
  window.addEventListener('pointerdown', unlock);
  window.addEventListener('keydown', unlock);

  window.D2Music = {
    setLevel,        // (key, name) -> choisit + lance le thème (fondu doux)
    setMuted,        // (bool) -> mute persistant (clé partagée avec menu.js)
    toggle: () => setMuted(!muted),
    theme: () => wanted,
    isMuted: () => muted,
  };
})();
