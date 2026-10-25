// Test 25a — contrat musique : mapping zones→musique + ambiance (tables 25b
// consommées par music.ts), cohérence day/night, intégration wrapper.
import { describe, expect, test } from 'bun:test';
import {
  AMB_BASE, MUSIC_BASE, ambianceFor, envFor, isNight, themeFor,
} from './audio/index';

describe('zones.themeFor (EXP-A1)', () => {
  test('zones A1 du port', () => {
    expect(themeFor('town', 'Camp des Rousses')).toBe('town1');
    expect(themeFor('bloodmoor', 'Blood Moor')).toBe('wild');
    expect(themeFor('denofevil', null)).toBe('denofevilaction');
    expect(themeFor('catacombs', null)).toBe('monastery');
    expect(themeFor('andariel', null)).toBe('andarielaction');
    expect(themeFor('tristram', null)).toBe('tristram');
  });

  test('actes II-IV', () => {
    expect(themeFor('lutgholein', null)).toBe('town2');
    expect(themeFor('sewers', null)).toBe('sewer');
    expect(themeFor('kurastdocks', null)).toBe('town3');
    expect(themeFor('chaossanctuary', null)).toBe('diablo');
  });

  test('heuristique par nom (fr) quand la clé est inconnue', () => {
    expect(themeFor(null, 'Cathédrale maudite')).toBe('monastery');
    expect(themeFor(null, 'Les Landes du Sang')).toBe('wild');
    expect(themeFor(null, 'Tristram')).toBe('tristram');
  });

  test('acte V / inconnu = null (thème courant conservé)', () => {
    expect(themeFor('harrogath', 'Harrogath')).toBeNull();
    expect(themeFor(null, null)).toBeNull();
    expect(themeFor('niveau-inconnu', 'Endroit sans nom')).toBeNull();
  });
});

describe('zones.envFor/ambianceFor (miroir audio_env)', () => {
  test('SoundEnv par clé de niveau', () => {
    expect(envFor('town', null)).toBe(1);
    expect(envFor('catacombs', null)).toBe(10);
    expect(envFor('chaossanctuary', null)).toBe(34);
  });

  test('stems jour/nuit réels', () => {
    expect(ambianceFor('town', null, false)).toBe('scene_wilderness_day');
    expect(ambianceFor('town', null, true)).toBe('scene_wilderness_night');
    expect(ambianceFor('catacombs', null, true)).toBe('scene_catacombs'); // identique jour/nuit
  });

  test('zone inconnue = null (ambiance courante conservée)', () => {
    expect(ambianceFor('zzz', null, false)).toBeNull();
    expect(ambianceFor(null, null, false)).toBeNull();
  });

  test('isNight D2 : ≥20h ou <6h', () => {
    expect(isNight(20)).toBe(true);
    expect(isNight(23)).toBe(true);
    expect(isNight(5)).toBe(true);
    expect(isNight(6)).toBe(false);
    expect(isNight(12)).toBe(false);
    expect(isNight(19)).toBe(false);
  });
});

describe('contrat fichiers', () => {
  test('bases assets (t2.wav bouclés, scene 14c)', () => {
    expect(MUSIC_BASE).toBe('assets/music/');
    expect(AMB_BASE).toBe('assets/sfx/ambiance/');
  });
});
