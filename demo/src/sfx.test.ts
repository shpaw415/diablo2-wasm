// Test 25a — game-sfx.ts + mon-sfx.ts : tables SFX miroirs de d2rules
// (game_audio.rs sfx_of + missile_cast_sfx ; mon_audio CODE_FAMILY/FILES).
import { describe, expect, test } from 'bun:test';
import { CAST_FILES, SFX, classifyEvent, missileCastSfx } from './game-sfx';
import {
  CODE_FAMILY, FILES, familyOf, fileFor, feedTargets,
} from './mon-sfx';

describe('game-sfx.missileCastSfx (miroir missile_cast_sfx)', () => {
  test('froid AVANT foudre (ordre des mots-clés)', () => {
    // « frozenorb » porte « frozen » (cold) et « orb »... test ordre froid :
    expect(missileCastSfx('ice bolt')).toBe(CAST_FILES.cast_cold);
    expect(missileCastSfx('chain lightning')).toBe(CAST_FILES.cast_lightning);
    expect(missileCastSfx('fire bolt')).toBe(CAST_FILES.cast_fire);
    // un skill contenant cold ET lightning : froid d'abord (table 25b/legacy)
    expect(missileCastSfx('frozen lightning')).toBe(CAST_FILES.cast_cold);
  });

  test('physique/inconnu/vide = null (comme le Rust)', () => {
    expect(missileCastSfx('attack')).toBeNull();
    expect(missileCastSfx('')).toBeNull();
    expect(missileCastSfx(null)).toBeNull();
    expect(missileCastSfx(undefined)).toBeNull();
  });

  test('insensible à la casse', () => {
    expect(missileCastSfx('Fire Ball')).toBe(CAST_FILES.cast_fire);
    expect(missileCastSfx('LIGHTNING')).toBe(CAST_FILES.cast_lightning);
  });
});

describe('game-sfx.classifyEvent (miroir sfx_of)', () => {
  test('events sonorisés -> table SFX', () => {
    expect(classifyEvent({ HeroLeveled: { level: 2 } }, '')).toEqual({ file: SFX.levelup, vol: 0.9 });
    expect(classifyEvent({ GoldGained: { amount: 42 } }, '')).toEqual({ file: SFX.gold, vol: 0.75 });
    expect(classifyEvent({ CorpseDropped: { x: 1, y: 2 } }, '')).toEqual({ file: SFX.corpseDrop, vol: 0.8 });
    expect(classifyEvent({ LevelSwitch: { key: 'bloodmoor', name: 'Blood Moor' } }, ''))
      .toEqual({ file: SFX.portalEnter, vol: 0.7 });
    expect(classifyEvent({ ActQuestChanged: { phase: 'done' } }, ''))
      .toEqual({ file: SFX.questFlash, vol: 0.8 });
    expect(classifyEvent({ ActQuestChanged: { phase: 'start' } }, '')).toBeNull();
  });

  test('déjà sonorisés game.js + flux continu : muets', () => {
    expect(classifyEvent({ HeroAttacked: { target: 0, dmg: 4 } }, '')).toBeNull();
    expect(classifyEvent({ HeroMissed: { target: 0 } }, '')).toBeNull();
    expect(classifyEvent({ HeroHit: { index: 0, dmg: 2 } }, '')).toBeNull();
    expect(classifyEvent({ MonsterDied: { index: 0, xp: 10 } }, '')).toBeNull();
    expect(classifyEvent({ HeroMoved: {} }, '')).toBeNull();
    expect(classifyEvent({ MercDied: {} }, '')).toBeNull();
    expect(classifyEvent({ LevelSwitchFailed: 'inconnu' }, '')).toBeNull();
  });

  test('MissileSpawned : cast selon le skill ACTIF du héros', () => {
    expect(classifyEvent({ MissileSpawned: {} }, 'fire ball'))
      .toEqual({ file: CAST_FILES.cast_fire, vol: 0.7 });
    expect(classifyEvent({ MercMissileSpawned: {} }, 'ice bolt'))
      .toEqual({ file: CAST_FILES.cast_cold, vol: 0.7 });
    expect(classifyEvent({ MissileSpawned: {} }, 'attack')).toBeNull(); // physique : muet
  });

  test('variante inconnue = muette (future classification Rust)', () => {
    expect(classifyEvent({ FutureVariant: {} } as never, '')).toBeNull();
    expect(classifyEvent(null as never, '')).toBeNull();
  });
});

describe('mon-sfx.familyOf (miroir PORT_CODES mon_audio.rs)', () => {
  test('codes du port A1 + finaux', () => {
    expect(familyOf('FA')).toBe('fallen');
    expect(familyOf('ZM')).toBe('zombie');
    expect(familyOf('DI')).toBe('diablo');
    expect(familyOf('22')).toBe('izual');
    expect(familyOf('BK')).toBe('foulcrow');
  });

  test('code inconnu / null = null (repli générique au play)', () => {
    expect(familyOf('ZZ')).toBeNull();
    expect(familyOf(null)).toBeNull();
    expect(familyOf(undefined)).toBeNull();
  });
});

describe('mon-sfx.fileFor (extraction réelle + replis)', () => {
  test('fichier réel extrait -> assets/sfx/monsounds/', () => {
    expect(fileFor('zombie', 'attack')).toBe('assets/sfx/monsounds/zombie_attack1.wav');
    expect(fileFor('andariel', 'taunt')).toBe('assets/sfx/monsounds/andariel_taunt1.wav');
  });

  test('cellule vide (null) -> replis jusqu au générique (jamais de ReferenceError)', () => {
    // fallen/attack est null en extraction : le Legacy plantait ici
    // (LEGACY_FALLBACK non défini) — la voie TS renvoie le générique voulu.
    expect(fileFor('fallen', 'attack')).toBeNull(); // pas de générique attack
  });

  test('génériques : hit.wav / fallen_die.wav', () => {
    expect(fileFor('ancient', 'hit')).toBe('assets/sfx/hit.wav');
    expect(fileFor('ancient', 'death')).toBe('assets/sfx/fallen_die.wav');
    expect(fileFor('baalcrab', 'taunt')).toBeNull(); // pas de générique taunt
  });

  test('toutes les familles de CODE_FAMILY ont une entrée FILES', () => {
    for (const fam of Object.values(CODE_FAMILY)) {
      expect(FILES[fam]).toBeDefined();
    }
  });
});

describe('mon-sfx.feedTargets (routage du GameEvent)', () => {
  test('HeroAttacked/MissileHit/MercAttacked -> hit sur la cible', () => {
    expect(feedTargets({ HeroAttacked: { target: 3 } })).toEqual([{ index: 3, kind: 'hit' }]);
    expect(feedTargets({ MissileHit: { target: 5 } })).toEqual([{ index: 5, kind: 'hit' }]);
    expect(feedTargets({ MercAttacked: { target: 7 } })).toEqual([{ index: 7, kind: 'hit' }]);
  });

  test('HeroHit -> attack ; MonsterDied -> death', () => {
    expect(feedTargets({ HeroHit: { index: 2 } })).toEqual([{ index: 2, kind: 'attack' }]);
    expect(feedTargets({ MonsterDied: { index: 9, xp: 12 } })).toEqual([{ index: 9, kind: 'death' }]);
  });

  test('autres events : aucun son monstre', () => {
    expect(feedTargets({ HeroLeveled: { level: 2 } })).toEqual([]);
    expect(feedTargets(null as never)).toEqual([]);
  });
});
