// Diablo 2 WASM — étape 25b : moteur WebAudio PARTAGÉ (migration TS du socle
// demo/music.js EXP-A1 + 14c, réécriture fidèle). Un seul AudioContext par
// page, créé AU PREMIER GESTE (politique navigateur — même pattern que
// menu.js, commit 34ee67c). Graphe :
//   [musicBus] -> duckMusic ┐
//   [ambBus]    -> duckAmb  ├-> muteBus -> destination
//   [sfxBus] -> panNode ────┘
// Volumes distincts MUSIQUE / AMBIANCE / SFX persistés localStorage
// (d2wasm.vol.music|amb|sfx, défauts 0.4 / 0.18 / 0.5 — EXP-A1), mute GLOBAL
// partagé menu/jeu (clé unique d2wasm.musicMuted), panning stéréo positionnel
// borné ±0.85, cache de décodage (échec mis en cache : WAV absent = silence,
// JAMAIS d'erreur JS — garde-fou E2E), et ducking PRÉPARÉ pour la voix
// (étape 23) : setDucking(true) baisse musique + ambiance pendant la parole.
//
// Ce module est consommé par les wrappers music.ts / game-sfx.ts /
// mon-sfx.ts (étape 25a) et par menu.ts (clé de mute partagée). Il ne touche
// au DOM ni n'écoute d'événements à l'import : les wrappers branchent
// pointerdown/keydown -> unlock().
'use strict';

/** Bus de volume distinct (musique / ambiance / effets). */
export type VolumeKind = 'music' | 'amb' | 'sfx';

/** Abstraction du stockage persistant (localStorage réel ou repli mémoire). */
export interface VolumeStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Clé de mute partagée menu (menu.ts) / jeu (music.ts, game-sfx.ts). */
export const MUTE_KEY = 'd2wasm.musicMuted';

/** Clés localStorage des volumes par bus. */
export const VOLUME_KEYS: Readonly<Record<VolumeKind, string>> = {
  music: 'd2wasm.vol.music',
  amb: 'd2wasm.vol.amb',
  sfx: 'd2wasm.vol.sfx',
};

/** Défauts : musique = 0.4 (EXP-A1), ambiance basse, effets modérés. */
export const DEFAULT_VOLUMES: Readonly<Record<VolumeKind, number>> = {
  music: 0.4,
  amb: 0.18,
  sfx: 0.5,
};

/** Panning borné pour garder la source audible des deux côtés (14c). */
export const PAN_MAX = 0.85;

/** Ducking voix (étape 23, préparé) : musique/ambiance baissées à 30 %. */
export const DUCK_GAIN = 0.3;

/** Rampe douce générique (secondes, constante setTargetAtTime). */
export const RAMP_S = 0.05;

/** Borne un nombre dans [0, 1] (NaN/négatif -> 0). */
export function clamp01(v: unknown): number {
  const n = Number(v);
  if (!Number.isFinite(n)) return 0;
  return Math.min(1, Math.max(0, n));
}

/** Position écran -> pan normalisé [-PAN_MAX, +PAN_MAX] (pur, testable). */
export function panFor(xScreen: number, refWidth: number): number {
  if (!(refWidth > 0)) return 0;
  const n = (Number(xScreen) / refWidth) * 2 - 1;
  if (!Number.isFinite(n)) return 0;
  return Math.max(-PAN_MAX, Math.min(PAN_MAX, n));
}

/** localStorage réel protégé (peut lever en navigation privée) ; repli mémoire. */
export function safeStorage(): VolumeStorage {
  try {
    const ls = globalThis.localStorage;
    if (ls) {
      // sonde : certains contextes jettent À L'ÉCRITURE seulement
      ls.setItem('d2wasm.probe', '1');
      ls.removeItem('d2wasm.probe');
      return ls;
    }
  } catch {
    /* stockage indisponible */
  }
  const mem = new Map<string, string>();
  return {
    getItem: (k) => (mem.has(k) ? (mem.get(k) as string) : null),
    setItem: (k, v) => void mem.set(k, String(v)),
  };
}

/** Volumes persistés : défauts, puis clamp des valeurs valides trouvées. */
export function loadVolumes(store: VolumeStorage): Record<VolumeKind, number> {
  const out: Record<VolumeKind, number> = { ...DEFAULT_VOLUMES };
  try {
    for (const k of Object.keys(VOLUME_KEYS) as VolumeKind[]) {
      const raw = store.getItem(VOLUME_KEYS[k]);
      if (raw !== null && raw !== '') out[k] = clamp01(raw);
    }
  } catch {
    /* stockage indisponible : défauts */
  }
  return out;
}

/** Lit la clé de mute partagée (true = muet). */
export function readMuteKey(store: VolumeStorage): boolean {
  try {
    return store.getItem(MUTE_KEY) === '1';
  } catch {
    return false;
  }
}

/** Écrit la clé de mute partagée (même clé menu/jeu — EXP-A1). */
export function writeMuteKey(store: VolumeStorage, muted: boolean): void {
  try {
    store.setItem(MUTE_KEY, muted ? '1' : '0');
  } catch {
    /* stockage indisponible */
  }
}

/** Options d'une voix jouée via playBuffer. */
export interface VoiceOpts {
  /** gain linéaire de la voix (défaut 1 — le bus porte le volume persisté). */
  gain?: number;
  /** pan par voix [-1, 1] borné à ±PAN_MAX (undefined = bus non panné). */
  pan?: number;
  /** vitesse de lecture (défaut 1). */
  rate?: number;
  /** boucle (défaut false — les boucles de musique passent par ZoneMusic). */
  loop?: boolean;
}

type PanNode = StereoPannerNode | GainNode;

/**
 * Moteur audio partagé. Toutes les méthodes sont tolérantes aux pannes :
 * sans unlock(), sans WebAudio ou sans stockage, tout se dégrade en silence
 * propre sans jamais lever (garde-fou E2E : aucune promesse non rattrapée).
 */
export class AudioEngine {
  readonly storage: VolumeStorage;

  /** Contexte créé au premier geste seulement (null avant / en cas d'échec). */
  ctx: AudioContext | null = null;
  musicBus: GainNode | null = null;
  ambBus: GainNode | null = null;
  sfxBus: GainNode | null = null;

  private muteBus: GainNode | null = null;
  private panNode: PanNode | null = null;
  private duckMusic: GainNode | null = null;
  private duckAmb: GainNode | null = null;
  private panValue = 0;
  private ducking = false;
  private muted: boolean;
  private volumes: Record<VolumeKind, number>;
  private buffers = new Map<string, Promise<AudioBuffer | null>>();
  private unlockCallbacks: Array<() => void> = [];

  constructor(store: VolumeStorage = safeStorage()) {
    this.storage = store;
    this.volumes = loadVolumes(store);
    this.muted = readMuteKey(store);
  }

  // ---- état -----------------------------------------------------------------

  unlocked(): boolean {
    return this.ctx !== null;
  }

  isMuted(): boolean {
    return this.muted;
  }

  isDucking(): boolean {
    return this.ducking;
  }

  volumeOf(kind: VolumeKind): number {
    return this.volumes[kind] ?? 0;
  }

  panValueNow(): number {
    return this.panValue;
  }

  // ---- volumes / mute (persistés) --------------------------------------------

  /** Fixe + persiste un volume ; rampe le gain du bus si débloqué. */
  setVolume(kind: VolumeKind, v: number): number {
    if (!Object.prototype.hasOwnProperty.call(this.volumes, kind)) {
      return this.volumeOf(kind);
    }
    this.volumes[kind] = clamp01(v);
    try {
      this.storage.setItem(VOLUME_KEYS[kind], String(this.volumes[kind]));
    } catch {
      /* stockage indisponible */
    }
    const ctx = this.ctx;
    const bus =
      kind === 'music' ? this.musicBus : kind === 'amb' ? this.ambBus : this.sfxBus;
    if (ctx && bus) {
      try {
        bus.gain.setTargetAtTime(this.volumes[kind], ctx.currentTime, RAMP_S);
      } catch {
        /* bus déjà fermé */
      }
    }
    return this.volumes[kind];
  }

  /** Mute global persistant (clé partagée menu/jeu). */
  setMuted(m: boolean): void {
    this.muted = !!m;
    writeMuteKey(this.storage, this.muted);
    const ctx = this.ctx;
    if (ctx && this.muteBus) {
      try {
        this.muteBus.gain.setTargetAtTime(this.muted ? 0 : 1, ctx.currentTime, RAMP_S);
      } catch {
        /* bus déjà fermé */
      }
    }
  }

  toggleMuted(): boolean {
    this.setMuted(!this.muted);
    return this.muted;
  }

  // ---- panning / ducking ------------------------------------------------------

  /**
   * Pan les SFX positionnels selon la position ÉCRAN de la source
   * (canvas pleine fenêtre -> pan = (x/largeur)×2-1, borné ±PAN_MAX).
   * Retourne la valeur appliquée (utile aux tests).
   */
  pan(xScreen: number, refWidth?: number): number {
    const w =
      typeof refWidth === 'number' && refWidth > 0
        ? refWidth
        : (globalThis.innerWidth || 1280);
    this.panValue = panFor(xScreen, w);
    const ctx = this.ctx;
    const node: PanNode | null = this.panNode;
    if (ctx && node && 'pan' in node && node.pan) {
      try {
        node.pan.setTargetAtTime(this.panValue, ctx.currentTime, RAMP_S);
      } catch {
        /* fallback gain : on garde la valeur pour les tests */
      }
    }
    return this.panValue;
  }

  /**
   * Ducking VOIX (étape 23, préparé — non câblé) : baisse musique + ambiance
   * pendant la parole, remonte à la fin. Idempotent, sans context = no-op.
   */
  setDucking(on: boolean): void {
    this.ducking = !!on;
    const ctx = this.ctx;
    if (!ctx) return;
    const target = this.ducking ? DUCK_GAIN : 1;
    for (const node of [this.duckMusic, this.duckAmb]) {
      if (node) {
        try {
          node.gain.setTargetAtTime(target, ctx.currentTime, 0.15);
        } catch {
          /* bus déjà fermé */
        }
      }
    }
  }

  // ---- unlock (premier geste) --------------------------------------------------

  /**
   * Crée le contexte + le graphe de buses. Idempotent ; retourne true si le
   * contexte est disponible. Appelle les callbacks onUnlock enregistrés.
   */
  unlock(): boolean {
    if (this.ctx) {
      if (this.ctx.state === 'suspended') this.ctx.resume().catch(() => {});
      return true;
    }
    try {
      const Ctor =
        globalThis.AudioContext ||
        (globalThis as unknown as { webkitAudioContext?: typeof AudioContext })
          .webkitAudioContext;
      if (!Ctor) throw new Error('WebAudio indisponible');
      const ctx = new Ctor();
      this.ctx = ctx;
      this.buildGraph(ctx);
      if (ctx.state === 'suspended') ctx.resume().catch(() => {});
    } catch {
      // WebAudio indisponible : silence propre, JAMAIS d'erreur JS
      this.ctx = null;
      return false;
    }
    const cbs = this.unlockCallbacks;
    this.unlockCallbacks = [];
    for (const cb of cbs) {
      try {
        cb();
      } catch {
        /* un callback ne doit jamais casser l'unlock */
      }
    }
    return true;
  }

  /** Callback appelé au unlock (immédiatement si déjà débloqué). */
  onUnlock(cb: () => void): void {
    if (this.ctx) cb();
    else this.unlockCallbacks.push(cb);
  }

  private buildGraph(ctx: AudioContext): void {
    // graphe : [musique|ambiance|sfx->pan] -> muteBus -> destination
    this.muteBus = ctx.createGain();
    this.muteBus.gain.value = this.muted ? 0 : 1;
    this.muteBus.connect(ctx.destination);

    this.duckMusic = ctx.createGain();
    this.duckMusic.gain.value = 1;
    this.duckMusic.connect(this.muteBus);
    this.musicBus = ctx.createGain();
    this.musicBus.gain.value = this.volumeOf('music');
    this.musicBus.connect(this.duckMusic);

    this.duckAmb = ctx.createGain();
    this.duckAmb.gain.value = 1;
    this.duckAmb.connect(this.muteBus);
    this.ambBus = ctx.createGain();
    this.ambBus.gain.value = this.volumeOf('amb');
    this.ambBus.connect(this.duckAmb);

    this.sfxBus = ctx.createGain();
    this.sfxBus.gain.value = this.volumeOf('sfx');
    this.panNode = ctx.createStereoPanner ? ctx.createStereoPanner() : ctx.createGain();
    if ('pan' in this.panNode && this.panNode.pan) {
      this.panNode.pan.value = this.panValue; // fallback GainNode : pas de pan
    }
    this.sfxBus.connect(this.panNode);
    this.panNode.connect(this.muteBus);
  }

  // ---- buffers -----------------------------------------------------------------

  /**
   * Décode et met en cache le buffer d'une URL. Échec (HTTP absent, WAV
   * corrompu) mis en cache aussi : silence propre + note info, jamais d'erreur.
   */
  bufferOf(url: string): Promise<AudioBuffer | null> {
    const hit = this.buffers.get(url);
    if (hit) return hit;
    const job = (async () => {
      const r = await fetch(url);
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      const bytes = await r.arrayBuffer();
      const ctx = this.ctx;
      if (!ctx) throw new Error('contexte absent');
      return await ctx.decodeAudioData(bytes); // boucle sans couture : buffer entier
    })().catch((e: unknown) => {
      const msg = e && typeof e === 'object' && 'message' in e ? String((e as { message: unknown }).message) : String(e);
      console.info(`[audio] ${url} indisponible (${msg}) — silence.`);
      return null;
    });
    this.buffers.set(url, job);
    return job;
  }

  /**
   * Joue un buffer décodé (voix générique). opts.pan crée un panner PAR voix
   * (SFX monstres positionnels) ; sinon la voix part du bus SFX partagé.
   * Retourne la source (pour onended) ou null si lecture impossible.
   */
  playBuffer(buf: AudioBuffer, opts: VoiceOpts = {}): AudioBufferSourceNode | null {
    const ctx = this.ctx;
    if (!ctx) return null;
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      if (typeof opts.rate === 'number' && Number.isFinite(opts.rate)) {
        src.playbackRate.value = opts.rate;
      }
      src.loop = !!opts.loop;
      const g = ctx.createGain();
      g.gain.value = typeof opts.gain === 'number' ? clamp01(opts.gain) : 1;
      src.connect(g);
      let tail: AudioNode = g;
      if (typeof opts.pan === 'number' && ctx.createStereoPanner) {
        const sp = ctx.createStereoPanner();
        sp.pan.value = Math.max(-1, Math.min(1, opts.pan));
        g.connect(sp);
        tail = sp;
      }
      const bus = this.sfxBus || this.muteBus;
      if (bus) tail.connect(bus);
      else tail.connect(ctx.destination);
      src.start();
      return src;
    } catch {
      return null;
    }
  }
}

/** Singleton par page (menu et jeu partagent le même moteur). */
let singleton: AudioEngine | null = null;

export function getAudioEngine(): AudioEngine {
  if (!singleton) singleton = new AudioEngine();
  return singleton;
}
