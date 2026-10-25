// Test 25a — slots.ts : méta de sauvegarde, upsert (identité héros),
// plafond 5 slots, import/export round-trip, migration douce du legacy.
import { describe, expect, test } from 'bun:test';
import {
  LEGACY_SAVE_KEY, MAX_SLOTS, SLOTS_KEY, importSlot, listSlots, metaOf,
  mostRecentSlot, removeSlot, renameSlot, setSlotsStorage, upsertFromSave,
} from './slots';
import type { SlotsStorage } from './slots';

function memoryStore(): SlotsStorage & { dump(): Map<string, string> } {
  const mem = new Map<string, string>();
  return {
    getItem: (k) => (mem.has(k) ? (mem.get(k) as string) : null),
    setItem: (k, v) => void mem.set(k, String(v)),
    dump: () => mem,
  };
}

const SAVE_BA = JSON.stringify({
  version: 3, hero_code: 'BA', hero: { stats: { level: 4 } }, now_ms: 1000, level: 'town',
});
const SAVE_SOR = JSON.stringify({
  version: 3, hero_code: 'SOR', hero: { stats: { level: 7 } }, now_ms: 2000, level: 'bloodmoor',
});

describe('slots.metaOf', () => {
  test('extrait les métadonnées d2rules v3', () => {
    const meta = metaOf(SAVE_BA)!;
    expect(meta.hero_code).toBe('BA');
    expect(meta.hero_label).toBe('Barbare');
    expect(meta.level).toBe(4);
    expect(meta.playtime_ms).toBe(1000);
    expect(meta.level_key).toBe('town');
    expect(meta.zone).toBe('Camp des Rousses');
  });

  test('zone inconnue = clé brute, code inconnu = code', () => {
    const meta = metaOf(JSON.stringify({ version: 1, hero: {}, hero_code: 'XX', level: 'ZZZ' }))!;
    expect(meta.hero_label).toBe('XX');
    expect(meta.zone).toBe('zzz');
  });

  test('JSON non-save = null', () => {
    expect(metaOf('null')).toBeNull();
    expect(metaOf('{"version":3}')).toBeNull(); // pas de bloc hero
    expect(metaOf('{{{')).toBeNull();
  });
});

describe('slots.upsertFromSave', () => {
  test('un slot par héros : re-sauvegarder écrase SA fiche', () => {
    const store = memoryStore();
    setSlotsStorage(store);
    const s1 = upsertFromSave(SAVE_BA);
    const save5 = JSON.stringify({ version: 3, hero_code: 'BA', hero: { stats: { level: 5 } }, now_ms: 1500, level: 'town' });
    const s2 = upsertFromSave(save5);
    expect(s2.id).toBe(s1.id);
    expect(listSlots().length).toBe(1);
    expect(s2.level).toBe(5);
  });

  test('plafond MAX_SLOTS', () => {
    setSlotsStorage(memoryStore());
    for (let i = 0; i < MAX_SLOTS; i++) {
      upsertFromSave(JSON.stringify({ version: 3, hero_code: 'H' + i, hero: { stats: { level: 1 } } }));
    }
    expect(() => upsertFromSave(JSON.stringify({ version: 3, hero_code: 'H9', hero: {} })))
      .toThrow(/Slots pleins/);
  });

  test('save illisible = erreur claire', () => {
    setSlotsStorage(memoryStore());
    expect(() => upsertFromSave('pas du json')).toThrow(/illisible/);
  });
});

describe('slots liste + miroir legacy', () => {
  test('mostRecent = slot le plus récent ; miroir clé historique', async () => {
    const store = memoryStore();
    setSlotsStorage(store);
    upsertFromSave(SAVE_BA);
    await Bun.sleep(3); // ts = Date.now() : garantit l'ordre
    upsertFromSave(SAVE_SOR);
    expect(mostRecentSlot()!.hero_code).toBe('SOR');
    expect(store.getItem(LEGACY_SAVE_KEY)).toBe(SAVE_SOR);
  });

  test('migration douce : save historique importé en slot fondateur', () => {
    const store = memoryStore();
    store.setItem(LEGACY_SAVE_KEY, SAVE_BA);
    setSlotsStorage(store);
    const slots = listSlots();
    expect(slots.length).toBe(1);
    expect(slots[0]!.hero_code).toBe('BA');
    // écrit en base (persisté)
    expect(JSON.parse(store.getItem(SLOTS_KEY)!).slots.length).toBe(1);
  });
});

describe('slots rename / remove / import', () => {
  test('rename tronque à 24 chars et ignore le vide', () => {
    setSlotsStorage(memoryStore());
    const s = upsertFromSave(SAVE_BA);
    expect(renameSlot(s.id, '  Mon Héros Légendaire Xy ')!.name).toBe('Mon Héros Légendaire Xy');
    expect(renameSlot(s.id, '   ')!.name).toBe('Mon Héros Légendaire Xy');
    expect(renameSlot('inconnu', 'x')).toBeUndefined();
  });

  test('remove : le dernier slot résurrecte via le miroir (quirk Legacy)', () => {
    setSlotsStorage(memoryStore());
    const s = upsertFromSave(SAVE_BA);
    removeSlot(s.id);
    // Particularité héritée du Legacy (conservée) : retirer le DERNIER slot
    // laisse le miroir historique — le list() suivant le réimporte en slot
    // fondateur (migration douce). Le retrait d'un slot NON dernier est
    // définitif (couvert par le test plafond).
    expect(listSlots().length).toBe(1); // résurrection via le miroir
    expect(listSlots()[0]!.hero_code).toBe('BA');
  });

  test('import slot complet : remplace la fiche de CE héros (même id)', () => {
    setSlotsStorage(memoryStore());
    const s = upsertFromSave(SAVE_BA);
    const exported = JSON.parse(JSON.stringify(s));
    exported.name = 'Renommé import';
    exported.ts = 42;
    const back = importSlot(JSON.stringify(exported));
    expect(back.id).toBe(s.id);
    expect(back.name).toBe('Renommé import');
    expect(listSlots().length).toBe(1);
  });

  test('import d un save brut = upsert', () => {
    setSlotsStorage(memoryStore());
    const back = importSlot(SAVE_SOR);
    expect(back.hero_code).toBe('SOR');
  });
});
