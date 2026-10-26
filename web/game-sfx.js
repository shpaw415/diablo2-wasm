// Diablo 2 WASM — étape 14b : SFX gameplay par event moteur (PLAN-FIDELITE
// phase 14, mission d2w-audio2). Lecteur autoportant : file d'attente NON
// BLOQUANTE (max simultanés + anti-spam), volume modéré, mute GLOBAL partagé
// avec music.js (MÊME clé localStorage), autoplay au premier geste (politique
// navigateur — même pattern que web/music.js).
//
// CONTRAT MOTEUR — miroir exact de crates/d2rules/src/game_audio.rs (le
// moteur Rust fait foi : `sfx_of(GameEvent) -> Option<&str>` +
// `missile_cast_sfx(skill)`). Toute la logique vit ICI ; game.js ne gagne
// qu'une ligne (`import("./game-sfx.js")`), comme stash-ui.js.
//
// Intégration : ce module se branche TOUT SEUL en enveloppant `game.tick` dès
// que window.__d2.game existe (poll léger) — aucun autre point de contact.
// Les events arrivent au format JSON sérialisé du moteur
// (externally-tagged : {"HeroLeveled":{"level":2}}, {"LevelSwitch":{...}}).
//
// RÉPARTITION DES SONS (contrat game_audio.rs §Répartition) :
//   - HeroAttacked/HeroMissed/HeroHit/MonsterDied : DÉJÀ sonorisés par
//     game.js avec des sons réels par classe/monstre (manifest.sfx) -> ici
//     muets (pas de doublon).
//   - MissileSpawned -> cast par type élémentaire via missileCastSfx(skill)
//     (skill ACTIF lu sur heroStats(), table skills.txt v1.12).
//   - LevelSwitch (CoreEvent démo, hors GameEvent) -> traversée de
//     portail/warp = portal_enter.wav ; LevelSwitchFailed muet.
//   - tout le reste : table MIRROR ci-dessous (17 events sonorisés).
//
// Garde-fous E2E (identiques à music.js) : la sonde de game.js compte
// console.error et les promesses non rattrapées comme erreurs -> ici
// uniquement console.info, et TOUTES les promesses sont rattrapées. Un WAV
// absent = silence + note info, JAMAIS une erreur JS. Pas de lecture avant
// le premier geste utilisateur ; un event émis avant l'unlock est perdu
// (file non bloquante : on ne stocke jamais un son qu'on ne peut pas jouer).
'use strict';

(function () {
  const BASE = 'assets/sfx/gameplay/';  // relatif à web/ (comme assets/music)
  const MUTE_KEY = 'd2wasm.musicMuted'; // CLÉ PARTAGÉE music.js/menu.js : mute GLOBAL
  const VOLUME = 0.35;                  // volume modéré (musique : 0.4, bus séparé)
  const MAX_VOICES = 4;                 // max simultanés (file non bloquante)
  const ANTISPAM_MS = 60;               // même son : pas deux fois en 60 ms

  // --- table MIROIR de game_audio.rs (sfx_of + constants objets) -------------
  const SFX = {
    levelup: 'levelup.wav',            // HeroLeveled — cursor\levelup.wav
    heroDeath: 'hero_death.wav',       // HeroDied — combat\player\barbarian\death1.wav
    heroRespawn: 'hero_respawn.wav',   // HeroRespawned — skill\sorceress\teleport.wav
    gold: 'gold.wav',                  // GoldGained — item\gold.wav
    pickup: 'pickup.wav',              // HeroPickedUp — cursor\pickup.wav
    potionDrink: 'potion_drink.wav',   // PotionUsed — item\potiondrink.wav
    inventoryFull: 'inventory_full.wav', // InventoryFull — cursor\button.wav
    itemDrop: 'item_drop.wav',         // ItemDropped — item\flippy.wav
    missileHit: 'missile_hit.wav',     // MissileHit — skill\sorceress\blastimpact1.wav
    questFlash: 'quest_flash.wav',     // ActQuestChanged{done} — quest\flash1.wav
    actUnlock: 'act_unlock.wav',       // ActUnlocked — quest\flash2.wav
    gameWon: 'game_won.wav',           // GameWon — quest\soulrise1.wav
    corpseDrop: 'corpse_drop.wav',     // CorpseDropped — object\corpsefall.wav
    corpseRecover: 'corpse_recover.wav', // CorpseRecovered — cursor\convert.wav
    npcTalk: 'npc_talk.wav',           // NpcTalked — cursor\windowopen.wav
    mercHired: 'merc_hired.wav',       // MercHired — cursor\select.wav
    waypoint: 'waypoint.wav',          // WaypointActivated — object\waypointignite2.wav
    monResurrect: 'mon_resurrect.wav', // MonResurrected — fallenshaman\resurrectcast.wav
    // objets (pas d'event moteur dédié — constants game_audio.rs, prêts) :
    shrine: 'shrine.wav',              // SFX_SHRINE — object\shrine\refill.wav
    chest: 'chest.wav',                // SFX_CHEST — object\chestsmall.wav
    portalOpen: 'portal_open.wav',     // SFX_PORTAL_OPEN — object\portalopen.wav
    portalEnter: 'portal_enter.wav',   // LevelSwitch — skill\misc\portalenter.wav
  };

  // --- MIROIR de missile_cast_sfx (game_audio.rs) : froid AVANT foudre --------
  const CAST_KEYWORDS = [
    ['cast_fire', ['fire', 'inferno', 'meteor', 'blaze', 'enchant', 'hydra']],
    ['cast_cold', ['ice', 'cold', 'frost', 'blizz', 'glacial', 'frozen', 'shiver', 'chill']],
    ['cast_lightning', ['lightning', 'charg', 'static', 'thunder', 'nova', 'chain', 'shock']],
  ];
  const CAST_FILES = { cast_fire: 'cast_fire.wav', cast_cold: 'cast_cold.wav', cast_lightning: 'cast_lightning.wav' };
  function missileCastSfx(skill) {
    const s = String(skill || '').toLowerCase();
    if (!s) return null;
    for (const [key, words] of CAST_KEYWORDS) {
      if (words.some((w) => s.includes(w))) return CAST_FILES[key];
    }
    return null; // physique/autre : pas de cast sonore (comme le Rust)
  }

  // --- état / lecture ---------------------------------------------------------
  let actx = null;          // AudioContext créé AU PREMIER GESTE seulement
  let master = null;        // bus de gain SFX (distinct du bus musique)
  let voices = 0;           // sources vivantes (plafond MAX_VOICES)
  const last = new Map();   // fichier -> timestamp du dernier déclenchement
  const buffers = new Map(); // fichier -> Promise<AudioBuffer|null> (échec mis en cache)

  function mutedNow() {
    // lecture à chaque jeu : le bouton « Musique » de music.js écrit la même
    // clé — on reste synchronisé sans couplage entre modules.
    try { return localStorage.getItem(MUTE_KEY) === '1'; } catch (e) { return false; }
  }

  function bufferOf(file) {
    if (!buffers.has(file)) {
      buffers.set(file, (async () => {
        const r = await fetch(`${BASE}${file}`);
        if (!r.ok) throw new Error(`HTTP ${r.status}`);
        const bytes = await r.arrayBuffer();
        return actx.decodeAudioData(bytes); // octets bruts MPQ (PCM 22 050 Hz 16 bits)
      })().catch((e) => {
        // fichier absent/corrompu : silence propre, JAMAIS d'erreur JS (E2E)
        console.info(`[sfx] ${file} indisponible (${e && e.message ? e.message : e}) — silence.`);
        return null;
      }));
    }
    return buffers.get(file);
  }

  /** File NON BLOQUANTE : repli immédiat si plafond de voix ou avant unlock. */
  function play(file, vol = 1.0, rate = 1.0) {
    if (!file || !actx || mutedNow()) return false;
    const now = performance.now();
    if (now - (last.get(file) || -1e9) < ANTISPAM_MS) return false; // anti-spam
    if (voices >= MAX_VOICES) return false; // file non bloquante : on abandonne
    const my = voices + 1;
    if (my > MAX_VOICES) return false;
    voices = my;
    last.set(file, now);
    void (async () => {
      try {
        const buf = await bufferOf(file);
        if (!buf || mutedNow()) return;
        const src = actx.createBufferSource();
        src.buffer = buf;
        src.playbackRate.value = rate;
        const g = actx.createGain();
        g.gain.value = VOLUME * vol;
        src.connect(g); g.connect(master);
        src.onended = () => { voices = Math.max(0, voices - 1); };
        src.start();
      } catch (e) {
        voices = Math.max(0, voices - 1);
        console.info(`[sfx] lecture impossible (${e && e.message ? e.message : e}).`);
      }
    })();
    return true;
  }

  function unlock() {
    if (actx) {
      if (actx.state === 'suspended') actx.resume().catch(() => {});
      return;
    }
    try {
      actx = new (window.AudioContext || window.webkitAudioContext)();
      master = actx.createGain();
      master.gain.value = 1; // le volume est par voix (VOLUME), bus brut
      master.connect(actx.destination);
      if (actx.state === 'suspended') actx.resume().catch(() => {});
    } catch (e) {
      console.info('[sfx] WebAudio indisponible — silence.');
    }
  }

  // --- events -> sons (miroir sfx_of ; None = muet, see game_audio.rs) --------
  function handleEvent(ev) {
    if (!ev || typeof ev !== 'object') return;
    const kind = Object.keys(ev)[0]; // externally-tagged : première clé = variante
    const p = ev[kind] || {};
    switch (kind) {
      // déjà sonorisés côté game.js (sons réels par classe/monstre) : muets
      case 'HeroAttacked': case 'HeroMissed': case 'HeroHit': case 'MonsterDied':
      // flux continu / muets MVP (contrat game_audio.rs) :
      case 'HeroMoved': case 'MonFled': case 'MissileMissed':
      case 'MercAttacked': case 'MercMissed': case 'MercHit':
      case 'MercDied': case 'MercLeveled': case 'MercResurrected':
        return;
      case 'MissileSpawned': case 'MercMissileSpawned': {
        // cast par type élémentaire : skill ACTIF du héros (heroStats().skill)
        let skill = '';
        try { skill = (JSON.parse(window.__d2.game.heroStats()) || {}).skill || ''; } catch (e) { /* muet */ }
        play(missileCastSfx(skill), 0.7);
        return;
      }
      case 'ActQuestChanged': play(p.phase === 'done' ? SFX.questFlash : null, 0.8); return;
      case 'HeroLeveled': play(SFX.levelup, 0.9); return;
      case 'HeroDied': play(SFX.heroDeath, 0.9); return;
      case 'HeroRespawned': play(SFX.heroRespawn, 0.7); return;
      case 'GoldGained': play(SFX.gold, 0.75); return;
      case 'HeroPickedUp': play(SFX.pickup, 0.9); return;
      case 'PotionUsed': play(SFX.potionDrink, 0.8); return;
      case 'InventoryFull': play(SFX.inventoryFull, 0.8); return;
      case 'ItemDropped': play(SFX.itemDrop, 0.6); return; // drop fréquent : discret
      case 'MissileHit': play(SFX.missileHit, 0.55); return;
      case 'ActUnlocked': play(SFX.actUnlock, 0.9); return;
      case 'GameWon': play(SFX.gameWon, 1.0); return;
      case 'CorpseDropped': play(SFX.corpseDrop, 0.8); return;
      case 'CorpseRecovered': play(SFX.corpseRecover, 0.8); return;
      case 'NpcTalked': play(SFX.npcTalk, 0.7); return;
      case 'MercHired': play(SFX.mercHired, 0.8); return;
      case 'WaypointActivated': play(SFX.waypoint, 0.8); return;
      case 'MonResurrected': play(SFX.monResurrect, 0.7); return;
      // CoreEvent démo (hors GameEvent) : traversée de portail/warp
      case 'LevelSwitch': play(SFX.portalEnter, 0.7); return;
      case 'LevelSwitchFailed': return; // muet (message HUD déjà affiché)
      default: return; // variante future : muette jusqu'à classification Rust
    }
  }

  // --- branchement autonome : enveloppe game.tick dès qu'il existe -------------
  let wired = false;
  function tryWire() {
    if (wired) return true;
    const g = window.__d2 && window.__d2.game;
    if (!g || typeof g.tick !== 'function') return false;
    const orig = g.tick.bind(g);
    g.tick = function (dt, inputJson) {
      const eventsJson = orig(dt, inputJson);
      try {
        if (eventsJson && eventsJson !== '[]') {
          const events = JSON.parse(eventsJson);
          if (Array.isArray(events) && actx) for (const ev of events) handleEvent(ev);
        }
      } catch (e) { /* cosmétique : jamais bloquer le tick */ }
      return eventsJson;
    };
    wired = true;
    return true;
  }

  // --- API publique ------------------------------------------------------------
  window.D2SFX = {
    play,             // (file, vol?, rate?) -> joué maintenant si possible
    handleEvent,      // (event) -> classification selon la table miroir
    missileCastSfx,   // (skill) -> fichier|null (miroir game_audio::missile_cast_sfx)
    shrine: () => play(SFX.shrine, 0.8),     // constants objets (câblage futur)
    chest: () => play(SFX.chest, 0.8),
    portalOpen: () => play(SFX.portalOpen, 0.8),
    unlock,           // premier geste (pointerdown/keydown ci-dessous)
    isMuted: mutedNow,
    toggleMute: () => {
      const m = !mutedNow();
      try { localStorage.setItem(MUTE_KEY, m ? '1' : '0'); } catch (e) { /* stockage indispo */ }
      // même clé -> le bouton « Musique » de music.js reste l'UI de référence ;
      // si music.js est chargé, on passe par son API pour rafraîchir son label.
      if (window.D2Music && window.D2Music.setMuted) window.D2Music.setMuted(m);
      return m;
    },
    wired: () => wired,
  };

  // autoplay au premier geste (politique navigateur) — comme music.js/menu.js
  window.addEventListener('pointerdown', unlock, { passive: true });
  window.addEventListener('keydown', unlock);
  // branchement du flux d'events (poll léger jusqu'à l'instanciation WASM)
  const wireTimer = setInterval(() => { if (tryWire()) clearInterval(wireTimer); }, 250);
  setTimeout(() => clearInterval(wireTimer), 60000); // garde-fou : stop après 60 s
})();
