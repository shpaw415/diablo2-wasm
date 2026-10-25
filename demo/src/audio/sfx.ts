// Diablo 2 WASM — étape 25b : file de SFX NON BLOQUANTE (migration TS fidèle
// du socle demo/game-sfx.js étape 14b + one-shot positionnel de music.js 14c),
// construite sur le moteur partagé (audio/engine.ts).
//
//  - file NON BLOQUANTE : plafond de voix simultanées (4), anti-spam par
//    fichier (60 ms) — un son qu'on ne peut pas jouer est abandonné, jamais
//    mis en attente ;
//  - volume par voix + bus SFX persisté (le bus porte le volume persisté) ;
//  - playAt(name, xScreen) : one-shot POSITIONNEL — met à jour le pan du bus
//    pour CE son (contrat window.D2Audio.play de music.js 14c) ;
//  - mute global via le moteur (clé partagée menu/jeu, relue à chaque jeu) ;
//  - WAV absent = silence + console.info (cache d'échec du moteur), TOUTES
//    les promesses rattrapées — garde-fou E2E.
//
// Consommé par game-sfx.ts (events moteur) et mon-sfx.ts (SFX monstres,
// avec pan/gain par voix via engine.playBuffer) — étape 25a.
'use strict';

import { AudioEngine } from './engine';

/** Max simultanés (file non bloquante). */
export const MAX_VOICES = 4;

/** Même son : pas deux fois en 60 ms. */
export const ANTISPAM_MS = 60;

export interface SfxQueueOptions {
  /** Horloge injectable (tests) ; défaut : performance.now(). */
  nowMs?: () => number;
}

export class SfxQueue {
  private engine: AudioEngine;
  private base: string;
  private voices = 0;             // sources vivantes (plafond MAX_VOICES)
  private last = new Map<string, number>(); // fichier -> dernier déclenchement
  private nowMs: () => number;

  constructor(engine: AudioEngine, base: string, opts: SfxQueueOptions = {}) {
    this.engine = engine;
    this.base = base;
    this.nowMs = opts.nowMs || (() => performance.now());
  }

  /** Muet ? (relit l'état moteur à chaque jeu — bouton partagé menu/jeu.) */
  private mutedNow(): boolean {
    return this.engine.isMuted();
  }

  /**
   * File NON BLOQUANTE : repli immédiat (false) si plafond de voix,
   * anti-spam, muet ou avant le premier geste. vol/rate par voix.
   */
  play(file: string, vol = 1.0, rate = 1.0): boolean {
    if (!file || !this.engine.unlocked() || this.mutedNow()) return false;
    const now = this.nowMs();
    if (now - (this.last.get(file) ?? -1e9) < ANTISPAM_MS) return false; // anti-spam
    if (this.voices >= MAX_VOICES) return false; // file non bloquante : on abandonne
    this.voices += 1;
    this.last.set(file, now);
    void (async () => {
      try {
        const buf = await this.engine.bufferOf(`${this.base}${file}`);
        if (!buf || this.mutedNow()) {
          this.voices = Math.max(0, this.voices - 1);
          return;
        }
        const src = this.engine.playBuffer(buf, { gain: vol, rate });
        if (src) {
          src.onended = () => {
            this.voices = Math.max(0, this.voices - 1);
          };
        } else {
          this.voices = Math.max(0, this.voices - 1);
        }
      } catch {
        this.voices = Math.max(0, this.voices - 1);
        console.info('[sfx] lecture impossible.');
      }
    })();
    return true;
  }

  /**
   * One-shot POSITIONNEL depuis la base du lecteur (contrat D2Audio.play) :
   * xScreen fourni => pan du bus mis à jour pour CE son. Retourne la source
   * ou null (muet, avant geste, fichier absent).
   */
  async playAt(name: string, xScreen?: number): Promise<AudioBufferSourceNode | null> {
    if (!name || !this.engine.unlocked() || this.mutedNow()) return null;
    if (typeof xScreen === 'number') this.engine.pan(xScreen);
    let buf: AudioBuffer | null = null;
    try {
      buf = await this.engine.bufferOf(`${this.base}${name}`);
    } catch {
      return null;
    }
    if (!buf || this.mutedNow()) return null;
    const src = this.engine.playBuffer(buf, { gain: 1 });
    if (!src) console.info(`[sfx] ${name} non joué.`);
    return src;
  }

  /** Voix vivantes (tests/plombage). */
  activeVoices(): number {
    return this.voices;
  }
}
