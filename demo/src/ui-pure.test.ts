// Test 25a — stash-ui.ts + panels-ui.ts + cube-ui.ts + d2s-io.ts + automap.ts
// : règles pures migrées (plafonds economy.rs, find_first_fit, XP, tooltips).
import { describe, expect, test } from 'bun:test';
import {
  CARRY_CAP_PER_LEVEL, STASH_CAP, STASH_H, STASH_W, depositGoldPure,
  firstFit, setStashStorage, withdrawGoldPure,
} from './stash-ui';
import {
  FALLBACK_SIZES, firstFitPure, fmt, itemKey, kindOf, sizeOfPure, xpFrac,
} from './panels-ui';
import { QUALITY_HEX, itemColor, itemLabel, tipHtml, transmutePayload } from './cube-ui';
import { importMessage, sanitizeHeroName } from './d2s-io';
import { modeLineText, proj, revealIndices } from './automap';

describe('stash — plafonds economy.rs', () => {
  test('dépôt : plafond stash par difficulté', () => {
    // Normal 300k : dépose 250k + 100k -> 300k (50k refusés)
    const a = depositGoldPure(250000, 900000, 50000, 'normal')!;
    expect(a).toEqual({ done: 250000, left: 0, newStash: 300000 });
    const b = depositGoldPure(100000, 900000, 300000, 'normal')!;
    expect(b.done).toBe(0);
    expect(b.left).toBe(100000);
    // Enfer 800k
    expect(depositGoldPure(100000, 900000, 750000, 'hell')!.done).toBe(50000);
    // difficulté inconnue -> normal (300k)
    expect(depositGoldPure(1000000, 9999999, 0, 'zzz')!.done).toBe(STASH_CAP.normal!);
  });

  test('retrait : plafond porté 10 000 × niveau', () => {
    // niveau 3 -> 30 000 porté max ; bourse 25 000 -> retrait limité à 5 000
    const a = withdrawGoldPure(10000, 25000, 50000, 3);
    expect(a.done).toBe(5000);
    expect(a.left).toBe(5000);
    expect(a.newStash).toBe(45000);
    // stash vide
    const b = withdrawGoldPure(100, 0, 0, 1);
    expect(b.done).toBe(0);
    // niveau 1 : plafond 10 000
    expect(CARRY_CAP_PER_LEVEL).toBe(10000);
  });

  test('firstFit grille 6×4 (Bank Page)', () => {
    setStashStorage(null); // hors localStorage (bun test)
    const cells: Array<{ name: string } | null> = Array.from({ length: STASH_W * STASH_H }, () => null);
    expect(firstFit(1, 1, cells)).toEqual({ x: 0, y: 0 });
    cells[0] = { name: 'x' };
    expect(firstFit(1, 1, cells)).toEqual({ x: 1, y: 0 });
    // première place libre d'une pièce 2×2 : (2,0) si (0,0)-(1,1) occupées
    cells[1] = { name: 'x' };
    cells[STASH_W] = { name: 'x' };
    cells[STASH_W + 1] = { name: 'x' };
    expect(firstFit(2, 2, cells)).toEqual({ x: 2, y: 0 });
  });
});

describe('panels — règles pures', () => {
  test('xpFrac seuils patch_experience (colonne Barbarian)', () => {
    expect(xpFrac(1, 0)).toBe(0);
    expect(xpFrac(1, 250)).toBe(0.5);
    expect(xpFrac(1, 500)).toBe(1);
    expect(xpFrac(2, 1000)).toBe(0.5); // 500..1500
    expect(xpFrac(11, 999999)).toBe(1); // niv > 10 : barre pleine
  });

  test('firstFitPure : balayage rangée/rangée (inventory.rs)', () => {
    const empty: never[] = [];
    expect(firstFitPure(2, 2, 10, 4, empty)).toEqual({ x: 0, y: 0 });
    // 2×2 posé en (0,0) : le suivant à droite (2,0)
    const blocks = [{ x: 0, y: 0, w: 2, h: 2 }];
    expect(firstFitPure(2, 2, 10, 4, blocks)).toEqual({ x: 2, y: 0 });
    // ordre de balayage rangée/rangée (cy externe, cx interne) : la pièce
    // 1×2 prend (2,0) — première position libre en lisant ligne par ligne.
    expect(firstFitPure(1, 2, 10, 4, blocks)).toEqual({ x: 2, y: 0 });
    // ne tient pas -> null
    const full = [];
    for (let y = 0; y < 4; y++) for (let x = 0; x < 10; x++) full.push({ x, y, w: 1, h: 1 });
    expect(firstFitPure(1, 1, 10, 4, full)).toBeNull();
  });

  test('itemKey : slot moteur inclus (2 potions identiques coexistent)', () => {
    const a = { code: 'hp1', name: 'Potion', kind: 'Potion', slot: 3 };
    const b = { code: 'hp1', name: 'Potion', kind: 'Potion', slot: 7 };
    expect(itemKey(a)).not.toBe(itemKey(b));
    expect(itemKey(a)).toBe(itemKey({ ...a }));
  });

  test('kindOf/sizeOfPure/fmt', () => {
    expect(kindOf({ code: 'gld', name: 'or', kind: 'Gold' })).toBe('Gold');
    expect(kindOf({ code: 'zz', name: 'x', kind: 'Bizarre' })).toBe('Misc');
    expect(sizeOfPure(null, 'tbk')).toEqual(FALLBACK_SIZES.tbk!);
    expect(sizeOfPure({ hp1: [1, 2] }, 'hp1')).toEqual([1, 2] as const);
    expect(sizeOfPure(null, 'inconnu')).toEqual([1, 1]);
    expect(fmt(1234567)).toBe('1 234 567');
  });
});

describe('cube — tooltips et couleurs (purs)', () => {
  test('itemLabel : or, tronqué à 5', () => {
    expect(itemLabel({ code: 'gld', name: 'Or', kind: 'Gold' })).toBe('or');
    expect(itemLabel({ code: 'hp1', name: 'Potion de vie', kind: 'Potion' })).toBe('Potio');
    expect(itemLabel(null)).toBe('');
  });

  test('itemColor par catégorie, fallback Misc', () => {
    expect(itemColor({ code: 'gld', name: 'or', kind: 'Gold' })).toEqual(['#d9b96a', '#6e5626']);
    expect(itemColor({ code: 'zz', name: 'x', kind: 'Autre' })).toEqual(['#7d6f58', '#3c3428']);
  });

  test('tipHtml : nom coloré + base + lignes bleues, échappé', () => {
    const html = tipHtml({ name: '<Bane>', colorHex: '#a59263', baseName: 'Cap', lines: ['+5 Vie', '&plus;'] });
    expect(html).toContain('#a59263');
    expect(html).toContain('&lt;Bane&gt;');
    expect(html).toContain('Cap');
    expect(html).toContain('&amp;plus;');
  });

  test('QUALITY_HEX connu', () => {
    expect(QUALITY_HEX.Unique).toBe('#a59263');
  });

  test('payload transmutation : slots + seed 31 bits', () => {
    const p = JSON.parse(transmutePayload([2, 5], 0xFFFFFF)) as { slots: number[]; seed: number };
    expect(p.slots).toEqual([2, 5]);
    expect(p.seed).toBe(0xFFFFFF);
  });
});

describe('d2s-io — pure', () => {
  test('sanitizeHeroName : lettres/chiffres, ≤15, repli heros', () => {
    expect(sanitizeHeroName('Kael-Thas!')).toBe('KaelThas');
    expect(sanitizeHeroName('abcdefghijklmnop')).toBe('abcdefghijklmno');
    expect(sanitizeHeroName('')).toBe('heros');
    expect(sanitizeHeroName('///')).toBe('heros');
  });

  test('importMessage : mention de classe si changée', () => {
    const msg = importMessage({ name: 'Kael', class: 'SOR', level: 4, items_loaded: 3, waypoints: 2, classChanged: true }, 'k.d2s');
    expect(msg).toContain('Kael');
    expect(msg).toContain('classe SOR');
    expect(msg).toContain('3 objet(s)');
    expect(msg).toContain('2 waypoint(s)');
    const msg2 = importMessage({ name: 'Kael', class: 'BA', level: 4, items_loaded: 0, waypoints: 0 }, 'k.d2s');
    expect(msg2).not.toContain('classe');
  });
});

describe('automap — projection/fog/mode (purs)', () => {
  const d = {
    level: 'town', name: 'Camp', mode: 'automap', cellW: 10, cellH: 10, sub: 5,
    coverage: 0.42, cells: [[0, 0, 1, 0], [3, 2, 4, 0]],
  };

  test('proj iso : px=(cx-cy+cellH)*s, py=(cx+cy)*s/2+1', () => {
    const [px, py] = proj(d, 2, 0, 0);
    expect(px).toBe(20); // (0-0+10)*2
    expect(py).toBe(1);
    const [qx, qy] = proj(d, 2, 3, 2);
    expect(qx).toBe(22); // (3-2+10)*2
    expect(qy).toBe(6); // (5)*2/2+1
  });

  test('revealIndices : disque RADIUS, visite une seule fois', () => {
    const visited = new Uint8Array(100);
    const byCell = new Map([[0, 0], [32, 1]]); // (0,0)->idx0, (2,3)->idx1
    const first = revealIndices(d, visited, byCell, 0, 0);
    expect(first).toContain(0);
    const second = revealIndices(d, visited, byCell, 0, 0);
    expect(second).not.toContain(0); // déjà visitée
  });

  test('modeLineText 13b/21b', () => {
    expect(modeLineText('automap', 0.42, true)).toContain('traits réels');
    expect(modeLineText('automap', 0.42, false)).toContain('AutoMap résolu');
    expect(modeLineText('fallback', 0.4, false)).toContain('repli');
    expect(modeLineText('automap', 0.426, true)).toContain('43 %');
  });
});
