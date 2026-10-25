// Diablo 2 WASM — étape 14b : SFX gameplay par event moteur — migration TS
// 25a de demo/game-sfx.js, sur la file NON BLOQUANTE partagée 25b (SfxQueue,
// MAX_VOICES=4, ANTISPAM_MS=60, mute GLOBAL clé d2wasm.musicMuted).
//
// CONTRAT MOTEUR — miroir exact de crates/d2rules/src/game_audio.rs
// (`sfx_of(GameEvent) -> Option<&str>` + `missile_cast_sfx(skill)`).
// Contrat window.D2SFX INCHANGÉ : play/handleEvent/missileCastSfx/shrine/
// chest/portalOpen/unlock/isMuted/toggleMute/wired.
//
// TRANSITION : game-sfx.js est importé DYNAMIQUEMENT par game.js et
// s'auto-branche en enveloppant game.tick (poll). Pour garantir une
// exécution unique du Legacy (et donc UN seul wrap de tick), on warm-importe
// la même URL avant de décider : Legacy présent -> on l'ADOPTE ; absent ->
// la voie TS complète ci-dessous prend le relais.
import {   warmLegacy } from './legacy';
import { registerModule } from './types';
import { MUTE_KEY, getAudioEngine, SfxQueue } from './audio/index';
import { parseOr, type D2Window, type GameEvent } from './types';

export const GAMEPLAY_BASE = 'assets/sfx/gameplay/'; // relatif à demo/

// --- table MIROIR de game_audio.rs (sfx_of + constants objets) -------------
export const SFX = {
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
} as const;

// --- MIROIR de missile_cast_sfx (game_audio.rs) : froid AVANT foudre --------
export const CAST_KEYWORDS: ReadonlyArray<readonly [string, ReadonlyArray<string>]> = [
  ['cast_fire', ['fire', 'inferno', 'meteor', 'blaze', 'enchant', 'hydra']],
  ['cast_cold', ['ice', 'cold', 'frost', 'blizz', 'glacial', 'frozen', 'shiver', 'chill']],
  ['cast_lightning', ['lightning', 'charg', 'static', 'thunder', 'nova', 'chain', 'shock']],
];
export const CAST_FILES = {
  cast_fire: 'cast_fire.wav', cast_cold: 'cast_cold.wav', cast_lightning: 'cast_lightning.wav',
} as const;

/** Cast SFX par type élémentaire du skill ACTIF (pur, testable) —
 * froid AVANT foudre, physique = null (comme le Rust). */
export function missileCastSfx(skill: string | null | undefined): string | null {
  const s = String(skill || '').toLowerCase();
  if (!s) return null;
  for (const [key, words] of CAST_KEYWORDS) {
    if (words.some((w) => s.includes(w))) {
      return CAST_FILES[key as keyof typeof CAST_FILES] ?? null;
    }
  }
  return null;
}

export interface ClassifiedSfx { file: string; vol: number }

/** Classification PURE d'un event -> son à jouer (miroir sfx_of) ; les
 * variantes muettes (déjà sonorisées game.js / flux continu / futures)
 * renvoient null. `activeSkill` = heroStats().skill (casts). */
export function classifyEvent(ev: GameEvent, activeSkill: string): ClassifiedSfx | null {
  if (!ev || typeof ev !== 'object') return null;
  const kind = Object.keys(ev)[0]!; // externally-tagged : première clé = variante
  const p = (ev as Record<string, Record<string, unknown>>)[kind] ?? {};
  switch (kind) {
    // déjà sonorisés côté game.js (sons réels par classe/monstre) : muets
    case 'HeroAttacked': case 'HeroMissed': case 'HeroHit': case 'MonsterDied':
    // flux continu / muets MVP (contrat game_audio.rs) :
    case 'HeroMoved': case 'MonFled': case 'MissileMissed':
    case 'MercAttacked': case 'MercMissed': case 'MercHit':
    case 'MercDied': case 'MercLeveled': case 'MercResurrected':
      return null;
    case 'MissileSpawned': case 'MercMissileSpawned': {
      const file = missileCastSfx(activeSkill);
      return file ? { file, vol: 0.7 } : null;
    }
    case 'ActQuestChanged': return p.phase === 'done' ? { file: SFX.questFlash, vol: 0.8 } : null;
    case 'HeroLeveled': return { file: SFX.levelup, vol: 0.9 };
    case 'HeroDied': return { file: SFX.heroDeath, vol: 0.9 };
    case 'HeroRespawned': return { file: SFX.heroRespawn, vol: 0.7 };
    case 'GoldGained': return { file: SFX.gold, vol: 0.75 };
    case 'HeroPickedUp': return { file: SFX.pickup, vol: 0.9 };
    case 'PotionUsed': return { file: SFX.potionDrink, vol: 0.8 };
    case 'InventoryFull': return { file: SFX.inventoryFull, vol: 0.8 };
    case 'ItemDropped': return { file: SFX.itemDrop, vol: 0.6 }; // drop fréquent : discret
    case 'MissileHit': return { file: SFX.missileHit, vol: 0.55 };
    case 'ActUnlocked': return { file: SFX.actUnlock, vol: 0.9 };
    case 'GameWon': return { file: SFX.gameWon, vol: 1.0 };
    case 'CorpseDropped': return { file: SFX.corpseDrop, vol: 0.8 };
    case 'CorpseRecovered': return { file: SFX.corpseRecover, vol: 0.8 };
    case 'NpcTalked': return { file: SFX.npcTalk, vol: 0.7 };
    case 'MercHired': return { file: SFX.mercHired, vol: 0.8 };
    case 'WaypointActivated': return { file: SFX.waypoint, vol: 0.8 };
    case 'MonResurrected': return { file: SFX.monResurrect, vol: 0.7 };
    // CoreEvent démo (hors GameEvent) : traversée de portail/warp
    case 'LevelSwitch': return { file: SFX.portalEnter, vol: 0.7 };
    case 'LevelSwitchFailed': return null; // muet (message HUD déjà affiché)
    default: return null; // variante future : muette jusqu'à classification Rust
  }
}

/** Skill actif lu sur heroStats() (casts — pareil que le Legacy). */
function activeSkillOf(win: D2Window): string {
  try {
    const stats = parseOr<{ skill?: string }>(win.__d2?.game ? win.__d2.game.heroStats() : '{}', {});
    return stats.skill ?? '';
  } catch {
    return '';
  }
}

// --- voie TS (moteur partagé + SfxQueue 25b) --------------------------------
function installTsGameSfx(win: D2Window): void {
  registerModule('game-sfx', 'ts');
  const engine = getAudioEngine();
  const queue = new SfxQueue(engine, GAMEPLAY_BASE);

  const handleEvent = (ev: GameEvent): void => {
    const hit = classifyEvent(ev, activeSkillOf(win));
    if (hit) queue.play(hit.file, hit.vol);
  };

  // --- branchement autonome : enveloppe game.tick dès qu'il existe -------------
  let wired = false;
  const tryWire = (): boolean => {
    if (wired) return true;
    const g = win.__d2?.game;
    if (!g || typeof g.tick !== 'function') return false;
    const orig = g.tick.bind(g);
    g.tick = ((dt: number, inputJson?: string | null) => {
      const eventsJson = orig(dt, inputJson);
      try {
        if (eventsJson && eventsJson !== '[]') {
          const events = parseOr<GameEvent[]>(eventsJson, []);
          if (Array.isArray(events) && engine.unlocked()) {
            for (const ev of events) handleEvent(ev);
          }
        }
      } catch { /* cosmétique : jamais bloquer le tick */ }
      return eventsJson;
    }) as typeof g.tick;
    wired = true;
    return true;
  };

  // autoplay au premier geste (politique navigateur) — comme music.js/menu.js
  window.addEventListener('pointerdown', () => engine.unlock(), { passive: true });
  window.addEventListener('keydown', () => engine.unlock());
  // branchement du flux d'events (poll léger jusqu'à l'instanciation WASM)
  const wireTimer = setInterval(() => {
    if (tryWire()) clearInterval(wireTimer);
  }, 250);
  setTimeout(() => clearInterval(wireTimer), 60000); // garde-fou : stop après 60 s

  win.D2SFX = {
    play: (file, vol = 1.0, rate = 1.0) => (file ? queue.play(file, vol, rate) : false),
    handleEvent,
    missileCastSfx: (skill) => missileCastSfx(skill),
    shrine: () => queue.play(SFX.shrine, 0.8),
    chest: () => queue.play(SFX.chest, 0.8),
    portalOpen: () => queue.play(SFX.portalOpen, 0.8),
    unlock: () => void engine.unlock(),
    isMuted: () => {
      try {
        return localStorage.getItem(MUTE_KEY) === '1';
      } catch {
        return false;
      }
    },
    toggleMute: () => {
      const m = engine.toggleMuted();
      // même clé -> le bouton « Musique » de music.js reste l'UI de référence ;
      // si music.js est chargé, on passe par son API pour rafraîchir son label.
      if (win.D2Music?.setMuted) win.D2Music.setMuted(m);
      return m;
    },
    wired: () => wired,
  };
}

export function installGameSfx(win: D2Window): void {
  if (win.D2SFX) {
    registerModule('game-sfx', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : game-sfx.js Legacy actif — on adopte
  installTsGameSfx(win);
}

if (typeof window !== 'undefined') {
  await warmLegacy('game-sfx.js');
  installGameSfx(window as unknown as D2Window);
}
