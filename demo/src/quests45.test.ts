// Test 25a — quests45.ts : moteur Quests45Log (miroir quests_actes45.rs) —
// sous-objectifs "any" (sceaux/cages/Anciens), gates (requires), phases
// reward_pending → done par PNJ, drapeaux .d2s (bit 7 a5q3), round-trip JSON.
import { describe, expect, test } from 'bun:test';
import { DEFS45, Quests45Log, normalizeActEvent, trigMatches } from './quests45';

describe('quests45.normalizeActEvent', () => {
  test('JSON tagged + superunique', () => {
    expect(normalizeActEvent({ MonsterKilled: { code: 'izual', superunique: null } }))
      .toEqual({ kind: 'MonsterKilled', code: 'izual', superunique: null });
    expect(normalizeActEvent({ ZoneEntered: { level_id: 105 } })!.kind).toBe('ZoneEntered');
    expect(normalizeActEvent(null)).toBeNull();
  });
});

describe('quests45.trigMatches', () => {
  test('kill : alias code OU superunique, insensible à la casse', () => {
    const t = { desc: '', kill: ['frozenstein', 'snowyeti4', 'Frozenstein'] };
    expect(trigMatches(t, { kind: 'MonsterKilled', code: 'snowyeti4' })).toBe(true);
    expect(trigMatches(t, { kind: 'MonsterKilled', code: 'x', superunique: 'FROZENSTEIN' })).toBe(true);
    expect(trigMatches(t, { kind: 'MonsterKilled', code: 'baal' })).toBe(false);
    expect(trigMatches(t, { kind: 'NpcTalked', npc: 'frozenstein' })).toBe(false);
  });
  test('zone/item/talk', () => {
    expect(trigMatches({ desc: '', zone: 105 }, { kind: 'ZoneEntered', level_id: 105 })).toBe(true);
    expect(trigMatches({ desc: '', item: 'mss' }, { kind: 'ItemPicked', code: 'MSS' })).toBe(true);
    expect(trigMatches({ desc: '', talk: 'anya' }, { kind: 'NpcTalked', npc: 'Anya' })).toBe(true);
  });
});

describe('Quests45Log — a4q1 The Fallen Angel', () => {
  test('zone puis kill, reward chez Tyrael', () => {
    const log = new Quests45Log();
    log.onEvent({ MonsterKilled: { code: 'izual' } }); // hors ordre : step 0 = zone
    expect(log.phase('a4q1')).toBe('inactive');
    log.onEvent({ ZoneEntered: { level_id: 105 } });
    expect(log.phase('a4q1')).toBe('active');
    const changes = log.onEvent({ MonsterKilled: { code: 'IZUAL' } });
    expect(changes).toContainEqual({ id: 'a4q1', phase: 'reward_pending' });
    // bit 1 « récompense à prendre » posé
    expect(log.runs[0]!.flags & 2).toBe(2);
    // parler à Tyrael → done, bit 0
    const done = log.onEvent({ NpcTalked: { npc: 'tyrael' } });
    expect(done).toContainEqual({ id: 'a4q1', phase: 'done' });
    expect(log.phase('a4q1')).toBe('done');
    expect(log.runs[0]!.flags & 1).toBe(1);
  });
});

describe('Quests45Log — a4q2 sous-objectifs any (sceaux)', () => {
  test('les 3 sceaux hors ordre, masque subdone', () => {
    const log = new Quests45Log();
    log.onEvent({ ZoneEntered: { level_id: 108 } });
    log.onEvent({ MonsterKilled: { code: 'infector of souls' } });
    expect(log.runs[1]!.subdone).toBe(1);
    log.onEvent({ MonsterKilled: { code: 'grand vizier of chaos', superunique: null } });
    expect(log.runs[1]!.subdone).toBe(1 | 4);
    // même cible re-tuée : pas de double compte
    log.onEvent({ MonsterKilled: { code: 'infector of souls' } });
    expect(log.runs[1]!.subdone).toBe(1 | 4);
    log.onEvent({ MonsterKilled: { code: 'doomknight3', superunique: 'lord de seis' } });
    // step suivant : tuer Diablo
    expect(log.runs[1]!.step).toBe(2);
    log.onEvent({ MonsterKilled: { code: 'diablo' } });
    expect(log.phase('a4q2')).toBe('reward_pending');
  });
});

describe('Quests45Log — gates et fin de jeu', () => {
  test('a5q4 requires a5q3 : bloquée tant que Prison of Ice n est pas done', () => {
    const log = new Quests45Log();
    log.onEvent({ ZoneEntered: { level_id: 124 } }); // Halls of Vaught (a5q4 step 1)
    expect(log.phase('a5q4')).toBe('inactive'); // gate a5q3 fermée
    // a5q3 (zone 119 → kill Frozenstein → talk ANYA → récompense chez MALAH)
    log.onEvent({ ZoneEntered: { level_id: 119 } });
    log.onEvent({ MonsterKilled: { code: 'frozenstein' } });
    log.onEvent({ NpcTalked: { npc: 'anya' } }); // étape « donnez la potion à Anya »
    log.onEvent({ NpcTalked: { npc: 'malah' } }); // récompense a5q3 chez MALAH
    expect(log.phase('a5q3')).toBe('done');
    expect(log.runs[5]!.flags & 128).toBe(128); // bit 7 : parchemin consommé
    // a5q4 : talk anya (step 0), zone 124, kill nihlathak
    log.onEvent({ NpcTalked: { npc: 'anya' } });
    log.onEvent({ ZoneEntered: { level_id: 124 } });
    log.onEvent({ MonsterKilled: { code: 'nihlathak', superunique: 'Nihlathak Boss' } });
    expect(log.phase('a5q4')).toBe('reward_pending');
  });

  test('a5q6 final : done avec final=true (GameWon)', () => {
    const log = new Quests45Log();
    // a5q5 (sans NPC → done direct) : zone 120 + 3 Anciens
    log.onEvent({ ZoneEntered: { level_id: 120 } });
    log.onEvent({ MonsterKilled: { code: 'ancientbarb1' } });
    log.onEvent({ MonsterKilled: { code: 'ancientbarb2' } });
    const changes = log.onEvent({ MonsterKilled: { code: 'ancientbarb3' } });
    expect(changes).toContainEqual({ id: 'a5q5', phase: 'done', final: false });
    // a5q6 (requires a5q5) : zone 131 + baal + talk tyrael
    log.onEvent({ ZoneEntered: { level_id: 131 } });
    log.onEvent({ MonsterKilled: { code: 'baalcrab', superunique: 'baal' } });
    const done = log.onEvent({ NpcTalked: { npc: 'tyrael' } });
    expect(done).toContainEqual({ id: 'a5q6', phase: 'done', final: true });
  });
});

describe('Quests45Log — round-trip JSON', () => {
  test('to_json/from_json (9 slots, version 1)', () => {
    const log = new Quests45Log();
    log.onEvent({ ZoneEntered: { level_id: 105 } });
    const back = Quests45Log.from_json(log.to_json());
    expect(back.runs[0]!.phase).toBe('active');
    expect(JSON.parse(back.to_json()).quests.length).toBe(DEFS45.length);
  });

  test('états refusés', () => {
    expect(() => Quests45Log.from_json('{"version":2,"quests":[]}')).toThrow(/inconnu/);
    const badPhase = {
      version: 1,
      quests: DEFS45.map((d, i) => ({ id: d.id, phase: i === 0 ? 'zzz' : 'inactive' })),
    };
    expect(() => Quests45Log.from_json(JSON.stringify(badPhase))).toThrow(/phase/);
  });
});
