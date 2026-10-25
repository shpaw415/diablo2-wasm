// Diablo 2 WASM — étape 25b : lecteur de musique par zone (migration TS
// fidèle du cœur EXP-A1 + 14c de demo/music.js), construit sur le moteur
// partagé (audio/engine.ts) et le mapping pur (audio/zones.ts).
//
//  - boucles SANS COUTURE : AudioBufferSourceNode.loop (les -t2 sont taillés
//    pour boucler), décodage mis en cache par le moteur ;
//  - transition DOUCE : crossfade ~1,5 s au changement de zone (fondu sortant
//    PENDANT l'entrée du nouveau, jamais de coupure sèche) ;
//  - couche AMBIANCE sous la musique (bus séparé, volume bas, fondu 1,5 s) ;
//  - anti-course : epoch musique et ambiance indépendants ;
//  - mute global via le moteur (clé partagée menu/jeu, persistée) ;
//  - nuit/jour : horloge locale (≥20 h / <6 h) ou override setNight() ;
//  - zone inconnue (acte V…) : thème ET ambiance courants conservés, note
//    console.info — jamais d'erreur JS, jamais de coupure sèche.
//
// Consommé par music.ts (wrapper window.D2Music/D2Audio, étape 25a) via
// onUnlock : ce module n'écoute aucun événement DOM lui-même.
'use strict';

import { AudioEngine, RAMP_S } from './engine';
import {
  AMB_BASE,
  MUSIC_BASE,
  ambianceFor,
  ambienceStems,
  envFor,
  isNight,
  themeFor,
} from './zones';

/** Fondu de transition musique (constant ~FADE/3 pour setTargetAtTime). */
export const FADE_S = 1.5;

/** Fondu ambiance au changement de région (1-2 s — 14c). */
export const FADE_AMB_S = 1.5;

/** Thème par défaut : ville de départ (Camp des Rousses). */
export const DEFAULT_THEME = 'town1';

interface MusicVoice {
  theme: string;
  src: AudioBufferSourceNode;
  gain: GainNode;
}

interface AmbVoice {
  stem: string;
  src: AudioBufferSourceNode;
  gain: GainNode;
}

export interface ZoneMusicOptions {
  /** Fondu musique en secondes (défaut 1,5). */
  fade?: number;
  /** Fondu ambiance en secondes (défaut 1,5). */
  fadeAmb?: number;
  /** Heure locale injectable (tests) ; défaut : horloge du navigateur. */
  hourNow?: () => number;
}

/**
 * Lecteur musique + ambiance par zone. Toutes les promesses sont rattrapées
 * (sonde E2E) : un WAV absent = silence + console.info, jamais une erreur.
 */
export class ZoneMusic {
  private engine: AudioEngine;
  private fade: number;
  private fadeAmb: number;
  private hourNow: () => number;

  private wanted = DEFAULT_THEME;  // thème musique demandé
  private wantedAmb: string | null = null; // stem ambiance demandé
  private current: MusicVoice | null = null;
  private ambNow: AmbVoice | null = null;
  private epoch = 0;       // garde anti-course transitions MUSIQUE
  private ambEpoch = 0;    // idem AMBIANCE (transitions indépendantes)
  private nightOverride: boolean | null = null; // true/false forcé, null=horloge
  private currentEnv: number | null = null;     // SoundEnv courant (setNight)

  constructor(engine: AudioEngine, opts: ZoneMusicOptions = {}) {
    this.engine = engine;
    this.fade = typeof opts.fade === 'number' && opts.fade > 0 ? opts.fade : FADE_S;
    this.fadeAmb =
      typeof opts.fadeAmb === 'number' && opts.fadeAmb > 0 ? opts.fadeAmb : FADE_AMB_S;
    this.hourNow = opts.hourNow || (() => new Date().getHours());
  }

  // ---- état -------------------------------------------------------------------

  theme(): string {
    return this.wanted;
  }

  ambient(): string | null {
    return this.ambNow ? this.ambNow.stem : null;
  }

  private night(): boolean {
    return this.nightOverride !== null ? this.nightOverride : isNight(this.hourNow());
  }

  // ---- lecture ------------------------------------------------------------------

  private stopCurrent(fadeOut = 0.5): void {
    const cur = this.current;
    const ctx = this.engine.ctx;
    if (!cur || !ctx) return;
    this.current = null;
    try {
      cur.gain.gain.setTargetAtTime(0, ctx.currentTime, fadeOut / 3);
      cur.src.stop(ctx.currentTime + fadeOut + 0.1); // fondu sortant, pas de coupure sèche
    } catch {
      /* source déjà arrêtée */
    }
  }

  private stopAmbience(fadeOut = this.fadeAmb): void {
    const cur = this.ambNow;
    const ctx = this.engine.ctx;
    if (!cur || !ctx) return;
    this.ambNow = null;
    try {
      cur.gain.gain.setTargetAtTime(0, ctx.currentTime, fadeOut / 3);
      cur.src.stop(ctx.currentTime + fadeOut + 0.1);
    } catch {
      /* source déjà arrêtée */
    }
  }

  /** Applique le thème demandé (crossfade). Promesse toujours rattrapée. */
  private async apply(): Promise<void> {
    const ctx = this.engine.ctx;
    const bus = this.engine.musicBus;
    if (!ctx || !bus) return;
    const my = ++this.epoch;
    const muted = this.engine.isMuted();
    if (muted || !this.wanted) {
      this.stopCurrent();
      void this.applyAmbience();
      return;
    }
    if (this.current && this.current.theme === this.wanted) return; // déjà en place
    const wantedTheme = this.wanted;
    const buf = await this.engine.bufferOf(`${MUSIC_BASE}${wantedTheme}-t2.wav`);
    if (my !== this.epoch) return; // une transition plus récente a pris le dessus
    if (!buf || this.engine.isMuted()) return;
    // fondu sortant du thème précédent PENDANT l'entrée du nouveau (crossfade)
    this.stopCurrent(this.fade);
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.loop = true; // les -t2 sont taillés pour boucler -> couture invisible
      const g = ctx.createGain();
      g.gain.value = 0;
      src.connect(g);
      g.connect(bus);
      src.start();
      g.gain.setTargetAtTime(1, ctx.currentTime, this.fade / 3); // fondu entrant ~1,5 s
      src.onended = () => {
        if (this.current && this.current.src === src) this.current = null;
      };
      this.current = { theme: wantedTheme, src, gain: g };
    } catch {
      console.info('[musique] lecture impossible.');
    }
  }

  /** Couche AMBIANCE en boucle SOUS la musique (bus séparé, volume bas). */
  private async applyAmbience(): Promise<void> {
    const ctx = this.engine.ctx;
    const bus = this.engine.ambBus;
    if (!ctx || !bus) return;
    const my = ++this.ambEpoch;
    const muted = this.engine.isMuted();
    if (muted || !this.wantedAmb) {
      this.stopAmbience();
      return;
    }
    if (this.ambNow && this.ambNow.stem === this.wantedAmb) return;
    const wantedStem = this.wantedAmb;
    const buf = await this.engine.bufferOf(`${AMB_BASE}${wantedStem}.wav`);
    if (my !== this.ambEpoch) return; // transition plus récente (anti-course)
    if (!buf || this.engine.isMuted()) return;
    this.stopAmbience(this.fadeAmb); // fondu sortant pendant l'entrée du nouveau
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.loop = true;
      const g = ctx.createGain();
      g.gain.value = 0;
      src.connect(g);
      g.connect(bus); // large, NON panné : le décor enveloppe
      src.start();
      g.gain.setTargetAtTime(1, ctx.currentTime, this.fadeAmb / 3);
      src.onended = () => {
        if (this.ambNow && this.ambNow.src === src) this.ambNow = null;
      };
      this.ambNow = { stem: wantedStem, src, gain: g };
    } catch {
      console.info('[ambiance] lecture impossible.');
    }
  }

  /** À brancher sur le unlock moteur (premier geste). Idempotent. */
  start(): void {
    this.engine.onUnlock(() => {
      void this.apply().catch(() => {});
    });
  }

  // ---- API (contrats window.D2Music / D2Audio du wrapper 25a) --------------------

  /**
   * Met à jour MUSIQUE + AMBIANCE de la zone (fondu doux). Zone sans thème
   * dédié : thème courant conservé (note info si clé fournie).
   */
  setLevel(key: string | null, name: string | null): void {
    const th = themeFor(key, name);
    if (th === null) {
      if (key || name) {
        console.info(`[musique] zone sans thème dédié (${key || '?'}) — thème courant conservé.`);
      }
    } else if (th !== this.wanted) {
      this.wanted = th;
      void this.apply().catch(() => {});
    }
    // même appel (hooks existants) met à jour l'ambiance — aucune ligne de plus
    const stem = ambianceFor(key, name, this.night());
    if (!stem) return;
    this.currentEnv = envFor(key, name); // retenu pour le basculement nuit/jour
    if (stem !== this.wantedAmb) {
      this.wantedAmb = stem;
      void this.applyAmbience().catch(() => {});
    }
  }

  setMuted(m: boolean): void {
    this.engine.setMuted(m);
    void this.apply().catch(() => {});
  }

  toggle(): boolean {
    this.setMuted(!this.engine.isMuted());
    return this.engine.isMuted();
  }

  isMuted(): boolean {
    return this.engine.isMuted();
  }

  /** true|false force la nuit/le jour, null revient à l'horloge locale. */
  setNight(n: boolean | null): void {
    this.nightOverride = typeof n === 'boolean' ? n : null;
    if (this.currentEnv === null) return;
    const stems = ambienceStems(this.currentEnv);
    const stem = stems ? (this.night() ? stems[1] : stems[0]) : null;
    if (stem && stem !== this.wantedAmb) {
      this.wantedAmb = stem;
      void this.applyAmbience().catch(() => {});
    }
  }

  /** Override manuel d'ambiance (debug/tests — contrat D2Audio.setAmbience). */
  setAmbience(stem: string | null): void {
    if (!stem || stem === this.wantedAmb) return;
    this.wantedAmb = stem;
    void this.applyAmbience().catch(() => {});
  }

  /** Résolution région -> stem avec l'état nuit/jour courant
   *  (contrat D2Audio.ambianceFor — même logique que d2rules::audio_env). */
  ambianceForNow(key: string | null, name: string | null): string | null {
    return ambianceFor(key, name, this.night());
  }

  /** Rampe utilitaire (constante partagée) — exposée pour les tests. */
  static rampConstant(): number {
    return RAMP_S;
  }
}
