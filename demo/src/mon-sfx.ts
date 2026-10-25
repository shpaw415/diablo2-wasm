// Diablo 2 WASM — étape 14a : SFX monstres data-driven — migration TS 25a
// de demo/mon-sfx.js (PLAN-FIDÉLITÉ phase 14). Contrat window.D2MonSfx
// INCHANGÉ : feed(ev) / setMuted(m) / familyOf(code) / isEnabled().
//
// Données : assets/monsounds.txt (table v1.12, 141 familles) + extraction
// RÉELLE des MPQ (assets/sfx/monsounds/, index FILES ci-dessous). Familles
// taillées -> repli générique (hit.wav / fallen_die.wav), jamais d'erreur.
//
// CORRECTIF TS (documenté) : le Legacy référençait LEGACY_FALLBACK sans le
// définir (ReferenceError latente dès qu'une famille portait null pour
// l'effet demandé — ex. attaque « fallen ») ; la table TS existe (vide :
// même repli générique voulu par le commentaire d'origine).
//
// TRANSITION : mon-sfx.js est un <script> classique de game.html — s'il est
// déjà actif on l'ADOPTE (sinon double unlock/double sons). La voie TS
// complète (moteur audio partagé 25b + panning par voix) prend le relais
// quand le tag Legacy disparaît.
import {   getAudioEngine } from './audio/index';
import { registerModule } from './types';
import type { D2Window, GameEvent } from './types';

export const MONSFX_BASE = 'assets/sfx/monsounds/'; // extraction réelle
export const MONSFX_LEGACY = 'assets/sfx/';         // SFX du port (étapes ant.)
export const MONSFX_VOLUME = 0.5;                   // bus SFX monstres
export const MONSFX_MAX_DIST_PX = 900;              // au-delà : volume plancher
export const MONSFX_VOL_FLOOR = 0.22;
export const MONSFX_SPAM_MS = 70;

/** Codes sprites du port -> famille sonore (miroir d2rules::mon_audio). */
export const CODE_FAMILY: Readonly<Record<string, string>> = {
  FA: 'fallen', ZM: 'zombie', SI: 'quillrat', SK: 'skeleton',
  FS: 'fallenshaman', YE: 'brute', CR: 'corruptrogue', GM: 'goatman',
  RD: 'radament', SU: 'summoner', GZ: 'zombieunique', AN: 'andariel',
  DU: 'duriel', MP: 'mephisto', DI: 'diablo', '22': 'izual',
  '5P': 'smithdemon', '0D': 'ancient', '0E': 'ancient', '0F': 'ancient',
  GY: 'unraveler', '42': 'baalcrab', BK: 'foulcrow',
};

export type MonSfxKind = 'attack' | 'hit' | 'death' | 'taunt';

/** Fichiers RÉELS extraits des MPQ (null = non extrait -> replis). */
export const FILES: Readonly<Record<string, Partial<Record<MonSfxKind, string | null>>>> = {
  ancient: { attack: null, hit: null, death: null, taunt: null },
  andariel: { attack: 'andariel_attack1.wav', hit: 'andariel_gethit1.wav', death: 'andariel_death.wav', taunt: 'andariel_taunt1.wav' },
  arach: { attack: 'spider_attack1.wav', hit: 'spider_gethit1.wav', death: 'spider_death1.wav', taunt: null },
  baalcrab: { attack: null, hit: null, death: null, taunt: null },
  baalminion: { attack: null, hit: null, death: null, taunt: null },
  batdemon: { attack: 'batdemon_attack1.wav', hit: 'batdemon_gethit1.wav', death: 'batdemon_death1.wav', taunt: null },
  bighead: { attack: 'bighead_attack1.wav', hit: 'bighead_gethit1.wav', death: 'bighead_death1.wav', taunt: null },
  bloodlord: { attack: null, hit: null, death: null, taunt: null },
  bloodraven: { attack: null, hit: 'bloodraven_gethit1.wav', death: 'bloodraven_death.wav', taunt: 'bloodraven_taunt1.wav' },
  brute: { attack: 'yeti_attack1.wav', hit: 'yeti_gethit1.wav', death: 'yeti_death1.wav', taunt: null },
  clawviper: { attack: 'serpentdemon_attacktail1.wav', hit: 'serpentdemon_gethit1.wav', death: 'serpentdemon_death1.wav', taunt: null },
  corruptrogue: { attack: 'corrupt_attack1.wav', hit: 'corrupt_gethit1.wav', death: null, taunt: null },
  councilmember: { attack: 'zakarumhighpriest_attack1.wav', hit: 'zakarumhighpriest_gethit1.wav', death: 'zakarumhighpriest_death1.wav', taunt: null },
  cr_archer: { attack: null, hit: 'corrupt_gethit1.wav', death: null, taunt: null },
  cr_lancer: { attack: 'corrupt_attack1.wav', hit: 'corrupt_gethit1.wav', death: null, taunt: null },
  deathmauler: { attack: null, hit: null, death: null, taunt: null },
  diablo: { attack: 'diablo_attack1.wav', hit: 'diablo_gethit1.wav', death: 'diablo_death.wav', taunt: null },
  doomknight: { attack: 'undeadhorror_attack1.wav', hit: 'undeadhorror_gethit1.wav', death: 'undeadhorror_death1.wav', taunt: null },
  duriel: { attack: 'duriel_attack1.wav', hit: 'duriel_gethit1.wav', death: null, taunt: 'duriel_taunt1.wav' },
  fallen: { attack: null, hit: 'fallen_gethit1.wav', death: 'fallen_death1.wav', taunt: null },
  fallenshaman: { attack: null, hit: 'fallenshaman_gethit1.wav', death: 'fallenshaman_death1.wav', taunt: null },
  fetishshaman: { attack: 'pygmyshaman_spell1.wav', hit: 'pygmyshaman_gethit1.wav', death: 'pygmyshaman_death1.wav', taunt: null },
  fingermage: { attack: 'fingermage_attack1.wav', hit: 'fingermage_gethit1.wav', death: 'fingermage_death1.wav', taunt: null },
  foulcrow: { attack: 'hawk_attack1.wav', hit: 'hawk_gethit1.wav', death: 'hawk_death1.wav', taunt: null },
  frozenhorror: { attack: null, hit: null, death: null, taunt: null },
  goatman: { attack: 'goatman_attack1.wav', hit: 'goatman_gethit1.wav', death: 'goatman_death1.wav', taunt: null },
  hellbovine: { attack: 'moo_attack1.wav', hit: 'moo_gethit1.wav', death: 'moo_death1.wav', taunt: null },
  imp: { attack: null, hit: null, death: null, taunt: null },
  izual: { attack: 'izual_attack1.wav', hit: 'izual_gethit1.wav', death: 'izual_death.wav', taunt: 'izual_taunt1.wav' },
  maggotqueen: { attack: null, hit: 'sandmaggot_gethit1.wav', death: 'sandmaggotmama_death.wav', taunt: null },
  megademon: { attack: 'megademon_attack1.wav', hit: 'megademon_gethit1.wav', death: 'megademon_death1.wav', taunt: null },
  mephisto: { attack: 'mephisto_attack1.wav', hit: 'mephisto_gethit1.wav', death: 'mephisto_death.wav', taunt: 'mephisto_taunt1.wav' },
  minion: { attack: null, hit: null, death: null, taunt: null },
  mummy: { attack: 'mummy_attack1.wav', hit: 'mummy_gethit1.wav', death: 'mummy_death1.wav', taunt: null },
  nihlathak: { attack: null, hit: null, death: null, taunt: null },
  overseer: { attack: null, hit: null, death: null, taunt: null },
  pantherwoman: { attack: 'pantherwoman_attack1.wav', hit: 'pantherwoman_gethit1.wav', death: 'pantherwoman_death1.wav', taunt: null },
  quillrat: { attack: 'spikefiend_attack1.wav', hit: 'spikefiend_gethit1.wav', death: 'spikefiend_death1.wav', taunt: null },
  radament: { attack: 'greatermummy_attack1.wav', hit: 'greatermummy_gethit1.wav', death: 'greatermummy_death1.wav', taunt: 'radament_taunt1.wav' },
  reanimatedhorde: { attack: null, hit: null, death: null, taunt: null },
  regurgitator: { attack: 'regurgitator_attack1.wav', hit: 'regurgitator_gethit1.wav', death: 'regurgitator_death1.wav', taunt: null },
  sandraider: { attack: 'sandraider_attack1.wav', hit: 'sandraider_gethit1.wav', death: 'sandraider_death1.wav', taunt: null },
  scarab: { attack: 'beetle_attack1.wav', hit: 'beetle_gethit1.wav', death: 'beetle_death1.wav', taunt: null },
  siegebeast: { attack: null, hit: null, death: null, taunt: null },
  sk_mage: { attack: null, hit: 'skeleton_gethit1.wav', death: 'skeleton_death1.wav', taunt: null },
  skeleton: { attack: null, hit: 'skeleton_gethit1.wav', death: 'skeleton_death1.wav', taunt: null },
  smith: { attack: 'smith_attack1.wav', hit: 'smith_gethit1.wav', death: 'smith_death.wav', taunt: 'smith_taunt2.wav' },
  smithdemon: { attack: 'smith_attack1.wav', hit: 'smith_gethit1.wav', death: 'smith_death.wav', taunt: null },
  succubus: { attack: null, hit: null, death: null, taunt: null },
  summoner: { attack: null, hit: 'summoner_gethit1.wav', death: 'summoner_death.wav', taunt: 'summoner_taunt1.wav' },
  thornhulk: { attack: null, hit: 'thornhulk_gethit1.wav', death: 'thornhulk_death1.wav', taunt: null },
  unraveler: { attack: 'greatermummy_attack1.wav', hit: 'greatermummy_gethit1.wav', death: 'greatermummy_death1.wav', taunt: null },
  vilemother: { attack: 'vilemother_attack1.wav', hit: 'vilemother_gethit1.wav', death: 'vilemother_death1.wav', taunt: null },
  willowisp: { attack: 'willowisp_attack1.wav', hit: 'willowisp_gethit1.wav', death: 'willowisp_death1.wav', taunt: null },
  zombie: { attack: 'zombie_attack1.wav', hit: 'zombie_gethit1.wav', death: 'zombie_death1.wav', taunt: null },
  zombieunique: { attack: 'zombie_attack1.wav', hit: 'zombie_gethit1.wav', death: 'zombie_death1.wav', taunt: null },
};

/** Générique final (manifest.sfx du port) quand ni extraction ni legacy. */
export const GENERIC: Readonly<Record<MonSfxKind, string | null>> = {
  attack: null, hit: 'hit.wav', death: 'fallen_die.wav', taunt: null,
};

/** Repli « port » par famille (table vide : le Legacy voulait ce repli et
 * plantait avant — cf. en-tête). Extensible sans toucher au moteur. */
export const LEGACY_FALLBACK: Readonly<Record<string, Partial<Record<MonSfxKind, string>>>> = {};

/** Famille d'un code sprite (pur, testable). */
export function familyOf(code: string | null | undefined): string | null {
  return (code != null && CODE_FAMILY[code]) || null;
}

/** Chemin du fichier d'un effet (pur, testable) : extraction réelle >
 * repli port > générique ; null = muet. */
export function fileFor(fam: string, ev: MonSfxKind): string | null {
  const f = FILES[fam]?.[ev];
  if (f) return MONSFX_BASE + f;
  const legacy = LEGACY_FALLBACK[fam]?.[ev];
  if (legacy) return MONSFX_LEGACY + legacy;
  return GENERIC[ev] ? MONSFX_LEGACY + (GENERIC[ev] as string) : null;
}

/** Variante de feed : quels (index, effet) un GameEvent déclenche —
 * pur, testable (le Legacy routait inline). */
export function feedTargets(ev: GameEvent): Array<{ index: number; kind: MonSfxKind }> {
  if (!ev || typeof ev !== 'object') return [];
  if (ev.HeroAttacked) return [{ index: ev.HeroAttacked.target, kind: 'hit' }];
  if (ev.MissileHit) return [{ index: ev.MissileHit.target, kind: 'hit' }];
  if (ev.MercAttacked) return [{ index: ev.MercAttacked.target, kind: 'hit' }];
  if (ev.HeroHit) return [{ index: ev.HeroHit.index, kind: 'attack' }];
  if (ev.MonsterDied) return [{ index: ev.MonsterDied.index, kind: 'death' }];
  return [];
}

// --- voie TS (moteur partagé) ----------------------------------------------
function installTsMonSfx(win: D2Window): void {
  registerModule('mon-sfx', 'ts');
  const engine = getAudioEngine();
  let enabled = true;
  const buffers = new Map<string, Promise<AudioBuffer | null>>();
  const last = new Map<string, number>();
  const announced = new Set<number>();

  const unlock = (): void => {
    engine.unlock();
  };

  const bufferOf = (path: string): Promise<AudioBuffer | null> => {
    const hit = buffers.get(path);
    if (hit) return hit;
    const job = (async () => {
      const r = await fetch(path);
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const bytes = await r.arrayBuffer();
      const ctx = engine.ctx;
      if (!ctx) throw new Error('contexte absent');
      return await ctx.decodeAudioData(bytes);
    })().catch((e: unknown) => {
      const msg = e instanceof Error ? e.message : String(e);
      console.info(`[mon-sfx] ${path} indisponible (${msg}) — silence.`);
      return null;
    });
    buffers.set(path, job);
    return job;
  };

  const play = (path: string | null, pan: number, vol: number, rate: number): void => {
    if (!enabled || !path || !engine.unlocked()) return;
    const now = performance.now();
    const key = path + '@' + Math.round(pan * 4);
    if (now - (last.get(key) ?? 0) < MONSFX_SPAM_MS) return;
    last.set(key, now);
    void (async () => {
      const buf = await bufferOf(path);
      if (!buf || !engine.unlocked() || !enabled) return;
      const src = engine.playBuffer(buf, { gain: Math.min(1, vol), pan, rate });
      if (!src) console.info('[mon-sfx] lecture impossible.');
    })().catch(() => { /* promesse toujours rattrapée (sonde E2E) */ });
  };

  const panAndVolOf = (index: number): [number, number] => {
    let pan = 0;
    let vol = 0.6;
    try {
      const game = win.__d2?.game;
      if (!game) return [pan, vol];
      const info: { sx?: number; sy?: number } = JSON.parse(game.monsterInfo(index) || 'null');
      if (info) {
        const canvas = document.querySelector('canvas');
        const w = (canvas && canvas.width) || window.innerWidth || 960;
        const half = w / 2;
        pan = Math.max(-1, Math.min(1, ((info.sx ?? half) - half) / half));
        try {
          const screen = game.heroScreen();
          const d = Math.hypot((info.sx ?? 0) - screen[0]!, (info.sy ?? 0) - screen[1]!);
          vol = Math.max(MONSFX_VOL_FLOOR, 1 - d / MONSFX_MAX_DIST_PX);
        } catch { /* position héros indisponible : volume moyen */ }
      }
    } catch { /* hors carte : centre, volume moyen */ }
    return [pan, vol];
  };

  const monsterEvent = (index: number, ev: MonSfxKind): void => {
    if (!enabled) return;
    let code: string | null = null;
    try {
      code = (win.__d2?.monCodes ?? [])[index] ?? null;
    } catch {
      code = null;
    }
    const fam = familyOf(code);
    if (!fam) return;
    const [pan, vol] = panAndVolOf(index);
    play(fileFor(fam, ev), pan, vol, 1);
    // Annonce : PREMIÈRE ENGAGEMENT d'un super-unique/boss/champion.
    if (ev === 'hit' || ev === 'attack') {
      try {
        const game = win.__d2?.game;
        const info: { kind?: string } = game ? JSON.parse(game.monsterInfo(index) || 'null') : null;
        const rank = info && info.kind;
        if (info && (rank === 'super_unique' || rank === 'boss' || rank === 'champion')
          && !announced.has(index)) {
          announced.add(index);
          const t = FILES[fam]?.taunt;
          if (t) play(MONSFX_BASE + t, pan, Math.min(1, vol + 0.2), 1);
        }
      } catch { /* jamais bloquant */ }
    }
  };

  // Suit le bouton « Son » de game.js (SFX du jeu) — listener passif.
  const watchSoundButton = (): void => {
    try {
      const b = document.getElementById('btn-sound');
      if (b) b.addEventListener('click', () => setMuted(enabled), { passive: true });
    } catch { /* pas de bouton : le clavier/jeu continuent */ }
  };

  const setMuted = (m: boolean): void => {
    enabled = !m;
    if (enabled) unlock();
  };

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', watchSoundButton, { once: true });
  } else {
    watchSoundButton();
  }
  // autoplay au premier geste (politique navigateur)
  window.addEventListener('pointerdown', unlock);
  window.addEventListener('keydown', unlock);

  win.D2MonSfx = {
    feed(ev) {
      for (const t of feedTargets(ev)) monsterEvent(t.index, t.kind);
    },
    setMuted,
    familyOf: (code) => familyOf(code),
    isEnabled: () => enabled,
  };
}

export function installMonSfx(win: D2Window): void {
  if (win.D2MonSfx) {
    registerModule('mon-sfx', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : mon-sfx.js Legacy actif — on adopte
  installTsMonSfx(win);
}

if (typeof window !== 'undefined') installMonSfx(window as unknown as D2Window);
