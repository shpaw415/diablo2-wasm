/// <reference types="bun" />
// Tests bun — moteur audio partagé : volumes persistés, mute clé partagée,
// panning borné (étape 25b). Tout est testé SANS WebAudio (moteur non
// débloqué = les fonctions pures et la persistance restent opérationnelles).
import { describe, expect, test } from 'bun:test';
import {
  AudioEngine,
  DEFAULT_VOLUMES,
  DUCK_GAIN,
  MUTE_KEY,
  PAN_MAX,
  VOLUME_KEYS,
  clamp01,
  getAudioEngine,
  loadVolumes,
  panFor,
  readMuteKey,
  writeMuteKey,
} from './engine';
import { SfxQueue } from './sfx';

/** Faux stockage en mémoire (même interface que localStorage). */
function fakeStore(): Map<string, string> {
  return new Map();
}
function storeOf(m: Map<string, string>) {
  return {
    getItem: (k: string) => (m.has(k) ? (m.get(k) as string) : null),
    setItem: (k: string, v: string) => void m.set(k, v),
  };
}

describe('clamp01', () => {
  test('bornes et valeurs invalides', () => {
    expect(clamp01(0.5)).toBe(0.5);
    expect(clamp01(0)).toBe(0);
    expect(clamp01(1)).toBe(1);
    expect(clamp01(-0.1)).toBe(0);
    expect(clamp01(1.5)).toBe(1);
    expect(clamp01('0.75')).toBe(0.75);
    expect(clamp01(NaN)).toBe(0);
    expect(clamp01(undefined)).toBe(0);
    expect(clamp01('abc')).toBe(0);
  });
});

describe('panFor — panning borné ±0.85', () => {
  test('centre / milieu', () => {
    expect(panFor(640, 1280)).toBe(0);
    expect(panFor(960, 1280)).toBeCloseTo(0.5, 10);
    expect(panFor(320, 1280)).toBeCloseTo(-0.5, 10);
  });

  test('bornes : jamais au-delà de ±PAN_MAX', () => {
    expect(panFor(-5000, 1280)).toBe(-PAN_MAX);
    expect(panFor(5000, 1280)).toBe(PAN_MAX);
    expect(panFor(0, 1280)).toBe(-1 < -PAN_MAX ? -PAN_MAX : -1);
  });

  test('largeur invalide -> 0', () => {
    expect(panFor(100, 0)).toBe(0);
    expect(panFor(100, -10)).toBe(0);
    expect(panFor(NaN, 1280)).toBe(0);
  });
});

describe('volumes persistés (d2wasm.vol.*)', () => {
  test('défauts EXP-A1 : musique 0.4, ambiance 0.18, effets 0.5', () => {
    const m = fakeStore();
    const v = loadVolumes(storeOf(m));
    expect(v.music).toBe(0.4);
    expect(v.amb).toBe(0.18);
    expect(v.sfx).toBe(0.5);
  });

  test('valeurs lues puis clampées', () => {
    const m = fakeStore();
    m.set(VOLUME_KEYS.music, '0.7');
    m.set(VOLUME_KEYS.amb, '2');
    m.set(VOLUME_KEYS.sfx, '-1');
    const v = loadVolumes(storeOf(m));
    expect(v.music).toBe(0.7);
    expect(v.amb).toBe(1);
    expect(v.sfx).toBe(0);
  });

  test("valeur invalide -> clamp à 0 (comportement clamp01 d'origine), vide -> défaut", () => {
    const m = fakeStore();
    m.set(VOLUME_KEYS.music, 'abc');
    m.set(VOLUME_KEYS.amb, '');
    const v = loadVolumes(storeOf(m));
    expect(v.music).toBe(0); // Number('abc')=NaN -> clamp01 = 0 (comme music.js)
    expect(v.amb).toBe(DEFAULT_VOLUMES.amb); // '' ignoré : défaut conservé
  });

  test('setVolume persiste la valeur clampée (avant même le unlock)', () => {
    const m = fakeStore();
    const engine = new AudioEngine(storeOf(m));
    const out = engine.setVolume('music', 1.4);
    expect(out).toBe(1);
    expect(m.get(VOLUME_KEYS.music)).toBe('1');
    expect(engine.volumeOf('music')).toBe(1);
    // type inconnu : refusé sans effet
    expect((engine.setVolume as (k: string, v: number) => number)('bass', 0.9)).toBe(0);
  });
});

describe('mute global partagé (d2wasm.musicMuted)', () => {
  test('round-trip via les helpers purs', () => {
    const m = fakeStore();
    expect(readMuteKey(storeOf(m))).toBe(false);
    writeMuteKey(storeOf(m), true);
    expect(m.get(MUTE_KEY)).toBe('1');
    expect(readMuteKey(storeOf(m))).toBe(true);
    writeMuteKey(storeOf(m), false);
    expect(m.get(MUTE_KEY)).toBe('0');
  });

  test("l'engine lit la clé au démarrage et la réécrit via setMuted", () => {
    const m = fakeStore();
    m.set(MUTE_KEY, '1');
    const engine = new AudioEngine(storeOf(m));
    expect(engine.isMuted()).toBe(true);
    engine.setMuted(false);
    expect(m.get(MUTE_KEY)).toBe('0');
    expect(engine.toggleMuted()).toBe(true);
    expect(m.get(MUTE_KEY)).toBe('1');
  });
});

describe('ducking voix (étape 23, préparé)', () => {
  test('état bascule sans contexte (no-op sûr)', () => {
    const engine = new AudioEngine(storeOf(fakeStore()));
    expect(engine.isDucking()).toBe(false);
    engine.setDucking(true);
    expect(engine.isDucking()).toBe(true);
    engine.setDucking(false);
    expect(engine.isDucking()).toBe(false);
    expect(DUCK_GAIN).toBeLessThan(1);
  });
});

describe('moteur sans WebAudio (bun test) — dégradation propre', () => {
  test('unlock sans AudioContext -> false, aucune exception', () => {
    const engine = new AudioEngine(storeOf(fakeStore()));
    // bun n'a pas d'AudioContext : unlock doit retourner false sans lever
    expect(engine.unlock()).toBe(false);
    expect(engine.unlocked()).toBe(false);
  });

  test('pan sans contexte garde la valeur normalisée', () => {
    const engine = new AudioEngine(storeOf(fakeStore()));
    expect(engine.pan(320, 1280)).toBeCloseTo(-0.5, 10);
    expect(engine.panValueNow()).toBeCloseTo(-0.5, 10);
  });

  test('file SFX : rien ne joue avant le unlock (repli immédiat)', () => {
    const engine = new AudioEngine(storeOf(fakeStore()));
    const q = new SfxQueue(engine, 'assets/sfx/gameplay/', { nowMs: () => 1000 });
    expect(q.play('levelup.wav')).toBe(false);
    expect(q.activeVoices()).toBe(0);
  });

  test('singleton : même instance', () => {
    expect(getAudioEngine()).toBe(getAudioEngine());
  });
});
