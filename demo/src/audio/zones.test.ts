/// <reference types="bun" />
// Tests bun — mapping PUR zones -> thèmes / ambiance (étape 25b).
// Miroir des contrats EXP-A1 (musique par acte) et 14c (ambiance jour/nuit,
// miroir d2rules::audio_env). Les tables viennent de demo/music.js.
import { describe, expect, test } from 'bun:test';
import {
  ambienceStems,
  ambianceFor,
  envFor,
  isNight,
  themeFor,
} from './zones';

describe('themeFor — mapping zone -> thème musique', () => {
  test('acte I par clé', () => {
    expect(themeFor('town', null)).toBe('town1');
    expect(themeFor('bloodmoor', null)).toBe('wild');
    expect(themeFor('coldplains', null)).toBe('wild');
    expect(themeFor('cave', null)).toBe('caves');
    expect(themeFor('crypt', null)).toBe('crypt');
    expect(themeFor('catacombs', null)).toBe('monastery');
    expect(themeFor('tristram', null)).toBe('tristram');
  });

  test('events et boss par clé', () => {
    expect(themeFor('denofevil', null)).toBe('denofevilaction');
    expect(themeFor('andariel', null)).toBe('andarielaction');
    expect(themeFor('chaossanctuary', null)).toBe('diablo');
  });

  test('actes II à IV par clé', () => {
    expect(themeFor('lutgholein', null)).toBe('town2');
    expect(themeFor('faroasis', null)).toBe('desert');
    expect(themeFor('talrasha', null)).toBe('tombs');
    expect(themeFor('harem', null)).toBe('harem');
    expect(themeFor('kurastdocks', null)).toBe('town3');
    expect(themeFor('flayerjungle', null)).toBe('jungle');
    expect(themeFor('pandemoniumfortress', null)).toBe('town4');
    expect(themeFor('riverofflame', null)).toBe('lair');
  });

  test('acte V : null = thème courant conservé', () => {
    expect(themeFor('harrogath', null)).toBeNull();
    expect(themeFor('worldstone', null)).toBeNull();
  });

  test('heuristique par nom (fr + en)', () => {
    expect(themeFor(null, 'Camp des Rousses')).toBe('town1');
    expect(themeFor(null, 'Landes du Sang')).toBe('wild');
    expect(themeFor(null, 'Cathédrale')).toBe('monastery');
    expect(themeFor(null, 'Tristram')).toBe('tristram');
    expect(themeFor(null, 'Égouts de Kurast')).toBe('kurastsewer');
    expect(themeFor(null, 'Tombe de Tal Rasha')).toBe('tombs');
  });

  test('zone inconnue -> null (ni erreur ni thème)', () => {
    expect(themeFor('niveauinconnu', 'Endroit mystère')).toBeNull();
    expect(themeFor(null, null)).toBeNull();
    expect(themeFor(undefined, undefined)).toBeNull();
  });

  test('la clé prime sur le nom', () => {
    expect(themeFor('town', 'Tristram')).toBe('town1');
  });
});

describe('envFor — SoundEnv (miroir patch_levels.txt)', () => {
  test('par clé', () => {
    expect(envFor('town', null)).toBe(1);
    expect(envFor('bloodmoor', null)).toBe(2);
    expect(envFor('catacombs', null)).toBe(10);
    expect(envFor('lutgholein', null)).toBe(12);
    expect(envFor('kurastdocks', null)).toBe(23);
    expect(envFor('worldstone', null)).toBe(49);
  });

  test('par nom', () => {
    expect(envFor(null, 'Tristram')).toBe(3);
    expect(envFor(null, 'Catacombes')).toBe(10);
    expect(envFor(null, 'jungle')).toBe(24);
  });

  test('inconnu -> null', () => {
    expect(envFor('inconnu', null)).toBeNull();
    expect(envFor(null, null)).toBeNull();
  });
});

describe('ambiance — résolution jour/nuit', () => {
  test('stems jour/nuit sauvage', () => {
    const stems = ambienceStems(1);
    expect(stems).not.toBeNull();
    expect(stems![0]).toBe('scene_wilderness_day');
    expect(stems![1]).toBe('scene_wilderness_night');
  });

  test('stems identiques en cave', () => {
    const stems = ambienceStems(4);
    expect(stems![0]).toBe('scene_cave');
    expect(stems![1]).toBe('scene_cave');
  });

  test('env 0 et inconnu -> null', () => {
    expect(ambienceStems(0)).toEqual([null, null]);
    expect(ambienceStems(null)).toBeNull();
  });

  test('ambianceFor résout selon night', () => {
    expect(ambianceFor('town', null, false)).toBe('scene_wilderness_day');
    expect(ambianceFor('town', null, true)).toBe('scene_wilderness_night');
    expect(ambianceFor('catacombs', null, true)).toBe('scene_catacombs');
  });

  test("l'ambiance couvre tout l'acte V (14c) même sans thème musique", () => {
    expect(themeFor('harrogath', null)).toBeNull(); // musique : courant conservé
    expect(ambianceFor('harrogath', null, false)).toBe('scene_wilderness_day');
    expect(ambianceFor('worldstone', null, true)).toBe('scene_cathedral');
  });
});

describe('isNight — horloge D2 (≥ 20 h / < 6 h)', () => {
  test('bornes', () => {
    expect(isNight(20)).toBe(true);
    expect(isNight(23)).toBe(true);
    expect(isNight(0)).toBe(true);
    expect(isNight(5)).toBe(true);
    expect(isNight(6)).toBe(false);
    expect(isNight(12)).toBe(false);
    expect(isNight(19)).toBe(false);
  });
});
