// Test 25a — vendor.ts : règles miroir de d2rules/src/vendor.rs — RNG
// mulberry32 déterministe, stock par colonnes vendeurs (80 %/exemplaire non
// permanent), réserve Gheed 350-750, gamble 12 slots (prix ×(100+2·clvl)/100,
// pool spawnable && qlvl ≤ niveau), parsing TSV.
import { describe, expect, test } from 'bun:test';
import {
  DEFAULT_GOLD, GAMBLE_SLOTS, GHEED_GOLD, chance, count, gambleList,
  generate, kindOfMisc, loadBases, mulberry32, num, restockSeed, ri, sellPrice, tsv,
  type BaseItem, type GambleOdds,
} from './vendor';

describe('vendor RNG miroir', () => {
  test('mulberry32 déterministe au seed', () => {
    const a = mulberry32(42);
    const b = mulberry32(42);
    for (let i = 0; i < 10; i++) expect(a()).toBe(b());
    const vals = Array.from({ length: 100 }, mulberry32(7));
    for (const v of vals) {
      expect(v).toBeGreaterThanOrEqual(0);
      expect(v).toBeLessThan(1);
    }
  });

  test('ri bornes incluses ; chance en pourcentage', () => {
    const r = mulberry32(1);
    for (let i = 0; i < 50; i++) {
      const v = ri(r, 3, 7);
      expect(v).toBeGreaterThanOrEqual(3);
      expect(v).toBeLessThanOrEqual(7);
    }
    expect(ri(r, 5, 5)).toBe(5); // hi <= lo
    expect(chance(() => 0.999, 80)).toBe(false); // floor(99.9)=99 < 80 faux
    expect(chance(() => 0.1, 80)).toBe(true);    // floor(10)=10 < 80
  });
});

describe('vendor parsing TSV', () => {
  test('tsv/num/count/kindOfMisc', () => {
    expect(tsv('a\tb\n c \td ')[0]).toEqual(['a', 'b']);
    expect(num('12')).toBe(12);
    expect(num('x')).toBe(0);
    expect(count('')).toBe(0);
    expect(count('0')).toBe(0);
    expect(count('255')).toBe(0);
    expect(count('3')).toBe(3);
    expect(kindOfMisc('hpot')).toBe('Potion');
    expect(kindOfMisc('xxx')).toBe('Misc');
  });

  test('loadBases lit les colonnes vendeurs', () => {
    const hdr = ['code', 'name', 'level', 'type', 'spawnable', 'cost', 'gamble cost',
      'permstoreitem', 'Akaramin', 'Akaramax', 'Akaramagicmin', 'Akaramagicmax', 'Akaramagiclvl',
      'Charsimax', 'Gheedmax'].join('\t');
    const row = ['hp1', 'Potion de vie mineure', '1', 'hpot', '1', '50', '75', '0',
      '4', '10', '0', '0', '0', '8', '12'].join('\t');
    const out: BaseItem[] = [];
    loadBases(hdr + '\n' + row, kindOfMisc, out);
    expect(out.length).toBe(1);
    const hp1 = out[0]!;
    expect(hp1.code).toBe('hp1');
    expect(hp1.kind).toBe('Potion');
    expect(hp1.cost).toBe(50);
    expect(hp1.spawnable).toBe(true);
    expect(hp1.permanent).toBe(false);
    expect(hp1.stock.Akara!.max).toBe(10);
    expect(hp1.stock.Charsi!.max).toBe(8);
    expect(hp1.stock.Gheed!.max).toBe(12);
    // pas de stock magic (mmax 0)
    expect(hp1.stock.Akara!.mmax).toBe(0);
  });
});

describe('vendor.generate (règles vendor.rs)', () => {
  const catalog: BaseItem[] = [
    {
      code: 'hp1', name: 'Potion de vie mineure', level: 1, spawnable: true, cost: 50,
      gambleCost: 75, kind: 'Potion', permanent: false, magic: false,
      stock: { Akara: { min: 4, max: 10, mmin: 0, mmax: 0, mlvl: 0 } },
    },
    {
      code: 'rin', name: 'Ring', level: 1, spawnable: true, cost: 100,
      gambleCost: 150, kind: 'Misc', permanent: false, magic: true,
      stock: { Akara: { min: 1, max: 2, mmin: 1, mmax: 2, mlvl: 5 } },
    },
    {
      code: 'flag', name: 'Étendard', level: 1, spawnable: true, cost: 10,
      gambleCost: 0, kind: 'Misc', permanent: true, magic: false,
      stock: { Akara: { min: 1, max: 1, mmin: 0, mmax: 0, mlvl: 0 } },
    },
  ];

  test('réserve Gheed 350-750, autres vendeurs 10000', () => {
    const gheed = generate(catalog, 'Gheed', 8, 1);
    expect(gheed.gold).toBeGreaterThanOrEqual(GHEED_GOLD[0]);
    expect(gheed.gold).toBeLessThanOrEqual(GHEED_GOLD[1]);
    const akara = generate(catalog, 'Akara', 8, 1);
    expect(akara.gold).toBe(DEFAULT_GOLD);
  });

  test('déterministe au seed + vente = valeur/4', () => {
    const a = generate(catalog, 'Akara', 8, 99);
    const b = generate(catalog, 'Akara', 8, 99);
    expect(b.stock.length).toBe(a.stock.length);
    expect(b.gold).toBe(a.gold);
    for (let i = 0; i < a.stock.length; i++) {
      expect(b.stock[i]!.code).toBe(a.stock[i]!.code);
      expect(a.stock[i]!.sell).toBe(sellPrice(a.stock[i]!.buy));
    }
  });

  test('permanent ignore le tirage 80 % (toujours généré)', () => {
    for (let seed = 0; seed < 40; seed++) {
      const v = generate(catalog, 'Akara', 8, seed);
      expect(v.stock.some((u) => u.code === 'flag')).toBe(true);
    }
  });

  test('items magiques marqués quality Magic (sans affixes — écart documenté)', () => {
    let sawMagic = false;
    for (let seed = 0; seed < 60 && !sawMagic; seed++) {
      const v = generate(catalog, 'Akara', 8, seed);
      sawMagic = v.stock.some((u) => u.code === 'rin' && u.quality === 'Magic');
    }
    expect(sawMagic).toBe(true);
  });

  test('restockSeed dérive un nouveau seed (pur)', () => {
    expect(restockSeed(0x0D2)).toBe(((0x0D2 ^ ((0x0D2 << 7) >>> 0)) >>> 0));
  });
});

describe('vendor.gambleList', () => {
  const catalog: Record<string, BaseItem> = {
    rin: {
      code: 'rin', name: 'Ring', level: 1, spawnable: true, cost: 100,
      gambleCost: 50000, kind: 'Misc', permanent: false, magic: true,
      stock: {},
    },
    boc: { // non spawnable : exclu du pool
      code: 'boc', name: 'Crown', level: 1, spawnable: false, cost: 100,
      gambleCost: 50000, kind: 'Armor', permanent: false, magic: true, stock: {},
    },
    high: { // qlvl 30 > niveau vendeur 8 : exclu
      code: 'high', name: 'War Axe', level: 30, spawnable: true, cost: 100,
      gambleCost: 50000, kind: 'Weapon', permanent: false, magic: true, stock: {},
    },
  };
  const odds: GambleOdds = { rare: 256, set: 64, unique: 24, uber: 6, ultra: 1 };

  test('12 slots, prix = gamble cost × (100+2·clvl)/100, pool filtrée', () => {
    const list = gambleList(['rin', 'boc', 'high'], catalog, odds, 8, 5, 42);
    expect(list.length).toBe(GAMBLE_SLOTS);
    for (const u of list) {
      expect(u.code).toBe('rin'); // seul spawnable ≤ 8
      expect(u.buy).toBe(Math.floor((50000 * (100 + 2 * Math.min(5, 50))) / 100));
      expect(u.gamble).toBe(true);
      expect(u.name).toContain('(?)');
    }
  });

  test('pool vide = liste vide', () => {
    expect(gambleList(['boc'], catalog, odds, 8, 5, 1)).toEqual([]);
  });

  test('qualités bornées au pool magic (Normal si base non magique)', () => {
    const ringOnly = gambleList(['rin'], catalog, odds, 8, 5, 7);
    for (const u of ringOnly) {
      expect(['Unique', 'Set', 'Rare', 'Magic']).toContain(u.quality);
    }
  });
});
