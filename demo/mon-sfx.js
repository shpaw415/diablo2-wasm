// Diablo 2 WASM — étape 14a : SFX monstres data-driven (PLAN-FIDÉLITÉ phase 14).
// Lecteur autoportant (pattern demo/music.js) : WebAudio propre, autoplay au
// PREMIER GESTE, panning stéréo (gauche/droite selon le x écran du monstre),
// volume selon la distance au héros, anti-spam 70 ms, TOUTES les promesses
// rattrapées (sonde E2E de game.js compte console.error/unhandledrejection).
//
// Données : assets/monsounds.txt (table v1.12, 141 familles) — la couche
// FICHIERS ci-dessous est l'extraction RÉELLE des MPQ (tools/extract_mon_sfx.py
// -> demo/assets/sfx/monsounds/index.json, d2sfx.mpq data\global\sfx\monster\…
// + d2speech.mpq pour les taunts). Familles taillées de la release digitale :
// repli générique (hit.wav / fallen_die.wav du port), jamais d'erreur ni
// d'invention. Parité vérifiée par les tests Rust (d2rules::mon_audio).
//
// Câblage minimal côté game.js (1 ligne dans handleEvents) :
//   if (window.D2MonSfx) window.D2MonSfx.feed(ev);
// game.html ajoute <script src="./mon-sfx.js"> après music.js. Le lecteur suit
// le bouton « Son » de la toolbar (SFX du jeu) via un listener passif.
//
// Événements consommés (GameEvent existants, zéro Rust moteur touché) :
//   HeroAttacked{target} / MissileHit{target} / MercAttacked{target}
//     -> son HIT du monstre cible (il encaisse)
//   HeroHit{index}   -> son ATTACK du monstre (il frappe le héros)
//   MonsterDied{index} -> son DEATH + annonce (taunt) à la PREMIÈRE rencontre
//     d'un super-unique/boss (monsterInfo().kind ∈ super_unique|boss|champion)
'use strict';

(function () {
  const BASE = 'assets/sfx/monsounds/'; // extraction réelle (index ci-dessous)
  const LEGACY = 'assets/sfx/';         // SFX du port (étapes antérieures)
  const VOLUME = 0.5;                   // bus SFX monstres (distinct musique)
  const MAX_DIST_PX = 900;              // au-delà : volume plancher
  const VOL_FLOOR = 0.22;
  const SPAM_MS = 70;

  // Codes sprites du port -> famille sonore (patch_monstats/quests_finale,
  // miroir de d2rules::mon_audio::PORT_CODES — vérifié par test Rust).
  const CODE_FAMILY = {
    "FA": 'fallen', "ZM": 'zombie', "SI": 'quillrat', "SK": 'skeleton',
    "FS": 'fallenshaman', "YE": 'brute', "CR": 'corruptrogue', "GM": 'goatman',
    "RD": 'radament', "SU": 'summoner', "GZ": 'zombieunique', "AN": 'andariel',
    "DU": 'duriel', "MP": 'mephisto', "DI": 'diablo', "22": 'izual',
    "5P": 'smithdemon', "0D": 'ancient', "0E": 'ancient', "0F": 'ancient',
    "GY": 'unraveler', "42": 'baalcrab', "BK": 'foulcrow',
  };

  // Fichiers RÉELS extraits des MPQ (demo/assets/sfx/monsounds/, null = non
  // extrait : cellule vide en retail ou dossier taillé -> repli générique).
  const FILES = {
    "ancient": { attack: null, hit: null, death: null, taunt: null },
    "andariel": { attack: "andariel_attack1.wav", hit: "andariel_gethit1.wav", death: "andariel_death.wav", taunt: "andariel_taunt1.wav" },
    "arach": { attack: "spider_attack1.wav", hit: "spider_gethit1.wav", death: "spider_death1.wav", taunt: null },
    "baalcrab": { attack: null, hit: null, death: null, taunt: null },
    "baalminion": { attack: null, hit: null, death: null, taunt: null },
    "batdemon": { attack: "batdemon_attack1.wav", hit: "batdemon_gethit1.wav", death: "batdemon_death1.wav", taunt: null },
    "bighead": { attack: "bighead_attack1.wav", hit: "bighead_gethit1.wav", death: "bighead_death1.wav", taunt: null },
    "bloodlord": { attack: null, hit: null, death: null, taunt: null },
    "bloodraven": { attack: null, hit: "bloodraven_gethit1.wav", death: "bloodraven_death.wav", taunt: "bloodraven_taunt1.wav" },
    "brute": { attack: "yeti_attack1.wav", hit: "yeti_gethit1.wav", death: "yeti_death1.wav", taunt: null },
    "clawviper": { attack: "serpentdemon_attacktail1.wav", hit: "serpentdemon_gethit1.wav", death: "serpentdemon_death1.wav", taunt: null },
    "corruptrogue": { attack: "corrupt_attack1.wav", hit: "corrupt_gethit1.wav", death: null, taunt: null },
    "councilmember": { attack: "zakarumhighpriest_attack1.wav", hit: "zakarumhighpriest_gethit1.wav", death: "zakarumhighpriest_death1.wav", taunt: null },
    "cr_archer": { attack: null, hit: "corrupt_gethit1.wav", death: null, taunt: null },
    "cr_lancer": { attack: "corrupt_attack1.wav", hit: "corrupt_gethit1.wav", death: null, taunt: null },
    "deathmauler": { attack: null, hit: null, death: null, taunt: null },
    "diablo": { attack: "diablo_attack1.wav", hit: "diablo_gethit1.wav", death: "diablo_death.wav", taunt: null },
    "doomknight": { attack: "undeadhorror_attack1.wav", hit: "undeadhorror_gethit1.wav", death: "undeadhorror_death1.wav", taunt: null },
    "duriel": { attack: "duriel_attack1.wav", hit: "duriel_gethit1.wav", death: null, taunt: "duriel_taunt1.wav" },
    "fallen": { attack: null, hit: "fallen_gethit1.wav", death: "fallen_death1.wav", taunt: null },
    "fallenshaman": { attack: null, hit: "fallenshaman_gethit1.wav", death: "fallenshaman_death1.wav", taunt: null },
    "fetishshaman": { attack: "pygmyshaman_spell1.wav", hit: "pygmyshaman_gethit1.wav", death: "pygmyshaman_death1.wav", taunt: null },
    "fingermage": { attack: "fingermage_attack1.wav", hit: "fingermage_gethit1.wav", death: "fingermage_death1.wav", taunt: null },
    "foulcrow": { attack: "hawk_attack1.wav", hit: "hawk_gethit1.wav", death: "hawk_death1.wav", taunt: null },
    "frozenhorror": { attack: null, hit: null, death: null, taunt: null },
    "goatman": { attack: "goatman_attack1.wav", hit: "goatman_gethit1.wav", death: "goatman_death1.wav", taunt: null },
    "hellbovine": { attack: "moo_attack1.wav", hit: "moo_gethit1.wav", death: "moo_death1.wav", taunt: null },
    "imp": { attack: null, hit: null, death: null, taunt: null },
    "izual": { attack: "izual_attack1.wav", hit: "izual_gethit1.wav", death: "izual_death.wav", taunt: "izual_taunt1.wav" },
    "maggotqueen": { attack: null, hit: "sandmaggot_gethit1.wav", death: "sandmaggotmama_death.wav", taunt: null },
    "megademon": { attack: "megademon_attack1.wav", hit: "megademon_gethit1.wav", death: "megademon_death1.wav", taunt: null },
    "mephisto": { attack: "mephisto_attack1.wav", hit: "mephisto_gethit1.wav", death: "mephisto_death.wav", taunt: "mephisto_taunt1.wav" },
    "minion": { attack: null, hit: null, death: null, taunt: null },
    "mummy": { attack: "mummy_attack1.wav", hit: "mummy_gethit1.wav", death: "mummy_death1.wav", taunt: null },
    "nihlathak": { attack: null, hit: null, death: null, taunt: null },
    "overseer": { attack: null, hit: null, death: null, taunt: null },
    "pantherwoman": { attack: "pantherwoman_attack1.wav", hit: "pantherwoman_gethit1.wav", death: "pantherwoman_death1.wav", taunt: null },
    "quillrat": { attack: "spikefiend_attack1.wav", hit: "spikefiend_gethit1.wav", death: "spikefiend_death1.wav", taunt: null },
    "radament": { attack: "greatermummy_attack1.wav", hit: "greatermummy_gethit1.wav", death: "greatermummy_death1.wav", taunt: "radament_taunt1.wav" },
    "reanimatedhorde": { attack: null, hit: null, death: null, taunt: null },
    "regurgitator": { attack: "regurgitator_attack1.wav", hit: "regurgitator_gethit1.wav", death: "regurgitator_death1.wav", taunt: null },
    "sandraider": { attack: "sandraider_attack1.wav", hit: "sandraider_gethit1.wav", death: "sandraider_death1.wav", taunt: null },
    "scarab": { attack: "beetle_attack1.wav", hit: "beetle_gethit1.wav", death: "beetle_death1.wav", taunt: null },
    "siegebeast": { attack: null, hit: null, death: null, taunt: null },
    "sk_mage": { attack: null, hit: "skeleton_gethit1.wav", death: "skeleton_death1.wav", taunt: null },
    "skeleton": { attack: null, hit: "skeleton_gethit1.wav", death: "skeleton_death1.wav", taunt: null },
    "smith": { attack: "smith_attack1.wav", hit: "smith_gethit1.wav", death: "smith_death.wav", taunt: "smith_taunt2.wav" },
    "smithdemon": { attack: "smith_attack1.wav", hit: "smith_gethit1.wav", death: "smith_death.wav", taunt: null },
    "succubus": { attack: null, hit: null, death: null, taunt: null },
    "summoner": { attack: null, hit: "summoner_gethit1.wav", death: "summoner_death.wav", taunt: "summoner_taunt1.wav" },
    "thornhulk": { attack: null, hit: "thornhulk_gethit1.wav", death: "thornhulk_death1.wav", taunt: null },
    "unraveler": { attack: "greatermummy_attack1.wav", hit: "greatermummy_gethit1.wav", death: "greatermummy_death1.wav", taunt: null },
    "vilemother": { attack: "vilemother_attack1.wav", hit: "vilemother_gethit1.wav", death: "vilemother_death1.wav", taunt: null },
    "willowisp": { attack: "willowisp_attack1.wav", hit: "willowisp_gethit1.wav", death: "willowisp_death1.wav", taunt: null },
    "zombie": { attack: "zombie_attack1.wav", hit: "zombie_gethit1.wav", death: "zombie_death1.wav", taunt: null },
    "zombieunique": { attack: "zombie_attack1.wav", hit: "zombie_gethit1.wav", death: "zombie_death1.wav", taunt: null },
  };
  // Générique final (manifest.sfx du port) quand ni extraction ni legacy.
  const GENERIC = { attack: null, hit: 'hit.wav', death: 'fallen_die.wav', taunt: null };

  // --- état / audio -----------------------------------------------------------
  let actx = null, master = null, started = false, enabled = true;
  const buffers = new Map();   // chemin -> Promise<AudioBuffer|null> (échec caché)
  const last = new Map();      // clé anti-spam -> performance.now()
  const announced = new Set(); // index déjà annoncés (taunt première rencontre)

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
      console.info('[mon-sfx] WebAudio indisponible — silence.');
      actx = null;
    }
  }

  function bufferOf(path) {
    if (!buffers.has(path)) {
      buffers.set(path, (async () => {
        const r = await fetch(path);
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        return actx.decodeAudioData(await r.arrayBuffer());
      })().catch((e) => {
        console.info(`[mon-sfx] ${path} indisponible (${e && e.message ? e.message : e}) — silence.`);
        return null;
      }));
    }
    return buffers.get(path);
  }

  function play(path, pan, vol, rate) {
    if (!enabled || !actx || !path) return;
    const now = performance.now();
    const key = path + '@' + Math.round(pan * 4);
    if (now - (last.get(key) || 0) < SPAM_MS) return;
    last.set(key, now);
    (async () => {
      const buf = await bufferOf(path);
      if (!buf || !actx || !enabled) return;
      const src = actx.createBufferSource();
      src.buffer = buf;
      src.playbackRate.value = rate || 1;
      const g = actx.createGain();
      g.gain.value = vol;
      let node = src.connect(g);
      if (actx.createStereoPanner) node = g.connect(actx.createStereoPanner());
      node.connect(master);
      src.start();
    })().catch(() => { /* promesse toujours rattrapée (sonde E2E) */ });
  }

  function panAndVolOf(index) {
    let pan = 0, vol = 0.6;
    try {
      const game = window.__d2 && window.__d2.game;
      if (!game) return [pan, vol];
      const info = JSON.parse(game.monsterInfo(index) || 'null');
      if (info) {
        const canvas = document.querySelector('canvas');
        const w = (canvas && canvas.width) || window.innerWidth || 960;
        const half = w / 2;
        pan = Math.max(-1, Math.min(1, (info.sx - half) / half));
        try {
          const [hx, hy] = game.heroScreen();
          const d = Math.hypot(info.sx - hx, info.sy - hy);
          vol = Math.max(VOL_FLOOR, 1 - d / MAX_DIST_PX);
        } catch { /* position héros indisponible : volume moyen */ }
      }
    } catch { /* hors carte : centre, volume moyen */ }
    return [pan, vol];
  }

  function fileFor(fam, ev) {
    const f = FILES[fam] && FILES[fam][ev];
    if (f) return BASE + f;
    const legacy = LEGACY_FALLBACK[fam] && LEGACY_FALLBACK[fam][ev];
    if (legacy) return LEGACY + legacy;
    return GENERIC[ev] ? LEGACY + GENERIC[ev] : null;
  }

  // Une famille peut porter deux lignes monsounds (vague super-unique vs
  // banal) : le code est unique, la famille suffit au MVP (documents MONSOUND).
  function monsterEvent(index, ev) {
    if (!enabled) return;
    let code = null;
    try { code = (window.__d2.monCodes || [])[index]; } catch { code = null; }
    const fam = code != null ? CODE_FAMILY[code] : null;
    if (!fam) return;
    const [pan, vol] = panAndVolOf(index);
    play(fileFor(fam, ev), pan, vol, 1);
    // Annonce : PREMIÈRE ENGAGEMENT d'un super-unique/boss/champion (pas à la
    // mort — le taunt D2 est un cri de rencontre).
    if (ev === 'hit' || ev === 'attack') {
      try {
        const game = window.__d2 && window.__d2.game;
        const info = JSON.parse(game.monsterInfo(index) || 'null');
        const rank = info && info.kind;
        if (info && (rank === 'super_unique' || rank === 'boss' || rank === 'champion')
            && !announced.has(index)) {
          announced.add(index);
          const t = FILES[fam] && FILES[fam].taunt;
          if (t) play(BASE + t, pan, Math.min(1, vol + 0.2), 1);
        }
      } catch { /* jamais bloquant */ }
    }
  }

  // --- API publique ------------------------------------------------------------
  function feed(ev) {
    if (!ev || typeof ev !== 'object') return;
    if (ev.HeroAttacked) monsterEvent(ev.HeroAttacked.target, 'hit');
    else if (ev.MissileHit) monsterEvent(ev.MissileHit.target, 'hit');
    else if (ev.MercAttacked) monsterEvent(ev.MercAttacked.target, 'hit');
    else if (ev.HeroHit) monsterEvent(ev.HeroHit.index, 'attack');
    else if (ev.MonsterDied) monsterEvent(ev.MonsterDied.index, 'death');
  }

  function setMuted(m) {
    enabled = !m;
    if (enabled) unlock();
  }

  // Suit le bouton « Son » de game.js (SFX du jeu) — listener passif.
  function watchSoundButton() {
    try {
      const b = document.getElementById('btn-sound');
      if (b) b.addEventListener('click', () => setMuted(enabled), { passive: true });
    } catch { /* pas de bouton : le clavier/jeu continuent */ }
  }
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', watchSoundButton, { once: true });
  } else {
    watchSoundButton();
  }

  // autoplay au premier geste (politique navigateur) — comme menu.js/music.js
  window.addEventListener('pointerdown', unlock);
  window.addEventListener('keydown', unlock);

  window.D2MonSfx = {
    feed,          // (GameEvent) -> joue les sons monstres de l'événement
    setMuted,      // (bool) -> coupe/rétablit le bus SFX monstres
    familyOf: (code) => CODE_FAMILY[code] || null,
    isEnabled: () => enabled,
  };
})();
