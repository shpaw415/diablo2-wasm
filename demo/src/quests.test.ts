// Test 25a — quests.ts : moteur QuestLog (miroir quests.rs) — étapes
// séquentielles, auto-start, joker "*", BossDead ⇔ MonsterKilled, states,
// round-trip JSON, unlock d'acte.
import { describe, expect, test } from 'bun:test';
import {
  DEFS, QuestLog, normalizeEvent, triggerMatches,
} from './quests';

describe('quests.normalizeEvent', () => {
  test('JSON tagged du moteur', () => {
    expect(normalizeEvent({ MonsterKilled: { code: 'FA' } })).toEqual({ kind: 'MonsterKilled', code: 'FA' });
    expect(normalizeEvent({ ZoneEntered: { level_id: 8 } })).toEqual({ kind: 'ZoneEntered', level_id: 8 });
    expect(normalizeEvent({ BossDead: { code: 'andariel' } })).toEqual({ kind: 'BossDead', code: 'andariel' });
  });

  test('raccourci {type, code}', () => {
    expect(normalizeEvent({ type: 'MonsterKilled', code: 'FA' })).toEqual({ kind: 'MonsterKilled', code: 'FA' });
  });

  test('inconnu = null', () => {
    expect(normalizeEvent(null)).toBeNull();
    expect(normalizeEvent({ LevelSwitch: {} })).toBeNull();
    expect(normalizeEvent({ type: 'NiveauEntre', level_id: 8 })).toBeNull();
  });
});

describe('quests.triggerMatches', () => {
  const args = { kind: 'MonsterKilled' as const, code: 'FA' };
  test('joker *', () => {
    expect(triggerMatches({ MonsterKilled: { code: '*' } }, 'MonsterKilled', args)).toBe(true);
    expect(triggerMatches({ MonsterKilled: { code: 'ZM' } }, 'MonsterKilled', args)).toBe(false);
    expect(triggerMatches({ MonsterKilled: { code: 'FA' } }, 'MonsterKilled', args)).toBe(true);
  });
  test('BossDead compte un MonsterKilled du même code (contrat)', () => {
    expect(triggerMatches({ BossDead: { code: 'andariel' } }, 'MonsterKilled', { kind: 'MonsterKilled', code: 'andariel' })).toBe(true);
    expect(triggerMatches({ BossDead: { code: 'andariel' } }, 'MonsterKilled', { kind: 'MonsterKilled', code: 'smith' })).toBe(false);
  });
});

describe('QuestLog — scénario a1q1 (Den of Evil)', () => {
  test('auto-start, compteur 6, récompense unlock skill_point', () => {
    const log = new QuestLog();
    expect(log.state('a1q1')).toBe('not_started');
    // étape 1 : entrer dans la Tanière (level_id 8)
    expect(log.onEvent({ ZoneEntered: { level_id: 7 } })).toEqual([]);
    expect(log.state('a1q1')).toBe('not_started');
    log.onEvent({ ZoneEntered: { level_id: 8 } });
    expect(log.state('a1q1')).toBe('in_progress');
    const p = log.progress('a1q1')!;
    expect(p.step).toBe(1);
    expect(p.required).toBe(6);
    // étape 2 : purifier (6 kills, joker)
    for (let i = 0; i < 5; i++) log.onEvent({ MonsterKilled: { code: 'FA' } });
    expect(log.state('a1q1')).toBe('in_progress');
    expect(log.progress('a1q1')!.count).toBe(5);
    const rewards = log.onEvent({ MonsterKilled: { code: 'ZM' } });
    expect(log.state('a1q1')).toBe('done');
    expect(rewards).toEqual([{ Unlock: { flag: 'skill_point' } }]);
  });

  test('BossDead déclenche les étapes kill (a1q2 Blood Raven)', () => {
    const log = new QuestLog();
    const rewards = log.onEvent({ BossDead: { code: 'bloodraven' } });
    expect(log.state('a1q2')).toBe('done');
    expect(rewards).toEqual([{ Unlock: { flag: 'rogue_scout' } }]);
    // un kill "monstre" du même code marche aussi
    const log2 = new QuestLog();
    log2.onEvent({ MonsterKilled: { code: 'bloodraven' } });
    expect(log2.state('a1q2')).toBe('done');
  });

  test('a1q5 gates_act : done = unlock act1_done', () => {
    const log = new QuestLog();
    expect(log.actDone(1)).toBe(false);
    const rewards = log.onEvent({ BossDead: { code: 'andariel' } });
    expect(log.state('a1q5')).toBe('done');
    expect(log.actDone(1)).toBe(true);
    expect(rewards).toEqual([{ Unlock: { flag: 'act1_done' } }]);
  });

  test('fail() : seulement si in_progress', () => {
    const log = new QuestLog();
    expect(log.fail('a1q1')).toBe(false); // not_started
    log.onEvent({ ZoneEntered: { level_id: 8 } });
    expect(log.fail('a1q1')).toBe(true);
    expect(log.state('a1q1')).toBe('failed');
    expect(log.fail('a1q1')).toBe(false); // déjà failed
  });
});

describe('QuestLog — round-trip JSON (contrat Rust v1)', () => {
  test('to_json/from_json conservent les runs', () => {
    const log = new QuestLog();
    log.onEvent({ ZoneEntered: { level_id: 8 } });
    log.onEvent({ MonsterKilled: { code: 'FA' } });
    const json = log.to_json();
    const back = QuestLog.from_json(json);
    expect(back.states()).toEqual(log.states());
    expect(JSON.parse(json).version).toBe(1);
  });

  test('états incohérents refusés', () => {
    expect(() => QuestLog.from_json('{"version":2,"quests":[]}')).toThrow(/version/);
    expect(() => QuestLog.from_json('{"version":1,"quests":[]}')).toThrow(/nombre/);
    const bad = JSON.stringify({
      version: 1,
      quests: DEFS.map((d, i) => ({
        id: d.id,
        state: i === 0 ? 'done' : 'not_started',
        step: 0, count: 0,
      })),
    });
    expect(() => QuestLog.from_json(bad)).toThrow(/incohérent/); // done exige step=last+count=lastReq
  });

  test('identité des défis (5 quêtes, ids stables)', () => {
    expect(DEFS.map((d) => d.id)).toEqual(['a1q1', 'a1q2', 'a1q3', 'a1q4', 'a1q5']);
  });
});
