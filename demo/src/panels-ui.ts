// Diablo 2 WASM — demo/panels (étape 18b : UI fidèle — PERSONNAGE +
// INVENTAIRE visuel + STASH) — migration TS 25a de demo/panels-ui.js.
//
// DONNÉES RÉELLES consommées (endpoints wasm additifs 18b) : heroPanel(),
// stashInfo()/stashDeposit()/stashWithdraw() (economy.rs réel),
// inventory() (40 slots), beltInfo() (ceinture 4×4 + buffs),
// useConsumable(i). Endpoints absents du pkg wasm de certains arbres :
// appels gardés (message « Moteur non initialisé » au lieu d'une TypeError).
//
// CONTRAT window.D2Panels INCHANGÉ : toggle/open/refresh/state.
// GÉOMÉTRIE : grille perso 10×4, stash « Bank Page 1 » 6×4, ceinture 4×4,
// slots bodylocs en silhouette ; miroir visuel X×Y (tailles réelles
// invwidth/invheight des patch_*.txt, repli table intégrée).
//
// TRANSITION : panels-ui.js est importé DYNAMIQUEMENT par game.js et
// s'auto-branche AU SCOPE (CSS + boutons toolbar + capture i/c/b + poller).
// Warm-import de la même URL (exécution unique via le cache modules) puis
// adoption du Legacy ; la voie TS complète prend le relais sans Legacy.
import {   warmLegacy } from './legacy';
import { registerModule } from './types';
import { parseOr, type D2Game, type D2Window, type HeroPanel, type Item, type StashInfo } from './types';

export const INV_W = 10, INV_H = 4;    // grille perso (inventory.txt)
export const STASH_W2 = 6, STASH_H2 = 4; // Bank Page 1 (inventory.txt)
export const BELT_W = 4, BELT_H = 4;   // ceinture 4×4
export const PANELS_KEY = 'd2w.panels.invl.v1'; // miroir visuel persisté
export const CELL = 30;                // px par cellule de grille

/** Seuils XP CUMULÉS niv 1..10 — patch_experience.txt colonne Barbarian
 * (identiques game.js, d2rules::xp). */
export const XP_THR: readonly number[] = [0, 500, 1500, 3750, 7875, 14175, 22680, 32886, 44396, 57715, 72144];

/** Fraction d'XP du niveau courant (pur, testable). */
export function xpFrac(level: number, xp: number): number {
  const lvl = Math.min(level, 10);
  const base = XP_THR[lvl - 1] ?? 0;
  const next = XP_THR[lvl] ?? XP_THR[10]!;
  return next > base ? Math.max(0, Math.min(1, (xp - base) / (next - base))) : 1;
}

/** Format milliers « 12 345 » (pur). */
export function fmt(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ' ');
}

/** Clé stable d'un item (slot moteur inclus : deux items identiques
 * coexistent) — pur, testable. */
export function itemKey(it: Item & { slot?: number }): string {
  return it.code + '|' + it.name + '|' + (it.amount || 0) + '|' + (it.slot === undefined ? -1 : it.slot);
}

/** Catégorie couleur D2 (pur). */
export function kindOf(it: Item): 'Gold' | 'Potion' | 'Weapon' | 'Armor' | 'Misc' {
  if (it.kind === 'Gold') return 'Gold';
  if (it.kind === 'Potion') return 'Potion';
  if (it.kind === 'Weapon') return 'Weapon';
  if (it.kind === 'Armor') return 'Armor';
  return 'Misc';
}

export interface Box { x: number; y: number; w: number; h: number; overflow?: boolean }

/** Première place libre w×h sur une grille gw×gh occupée par `blocks`
 * (balayage rangée/rangée — même ordre qu'inventory.rs::find_first_fit) —
 * pur, testable. */
export function firstFitPure(
  w: number, h: number, gw: number, gh: number,
  blocks: ReadonlyArray<Box>, skip?: Box,
): { x: number; y: number } | null {
  const occ = new Set<string>();
  for (const b of blocks) {
    if (b === skip) continue;
    for (let y = b.y; y < b.y + b.h; y++) {
      for (let x = b.x; x < b.x + b.w; x++) occ.add(x + ',' + y);
    }
  }
  for (let cy = 0; cy <= gh - h; cy++) {
    for (let cx = 0; cx <= gw - w; cx++) {
      let ok = true;
      for (let y2 = cy; y2 < cy + h && ok; y2++) {
        for (let x2 = cx; x2 < cx + w && ok; x2++) {
          if (occ.has(x2 + ',' + y2)) ok = false;
        }
      }
      if (ok) return { x: cx, y: cy };
    }
  }
  return null; // ne tient pas : dessiné compacté sous la grille (plein)
}

/** Repli tailles (patch_misc/weapons/armor.txt v1.12, invwidth/invheight)
 * si le fetch des tables échoue — codes réellement produits au MVP. */
export const FALLBACK_SIZES: Readonly<Record<string, readonly [number, number]>> = {
  gld: [1, 1], hp1: [1, 1], hp2: [1, 1], hp3: [1, 1], hp4: [1, 1], hp5: [1, 1],
  mp1: [1, 1], mp2: [1, 1], mp3: [1, 1], mp4: [1, 1], mp5: [1, 1],
  rvs: [1, 1], rvl: [1, 1], tsc: [1, 1], tbk: [1, 2], isc: [1, 1], ibk: [1, 2],
  key: [1, 1], hax: [1, 3], buc: [2, 2], box: [2, 2],
};

export function sizeOfPure(sizeTable: Record<string, [number, number]> | null, code: string): readonly [number, number] {
  if (sizeTable && sizeTable[code]) return sizeTable[code]!;
  return FALLBACK_SIZES[code] ?? [1, 1];
}

/** Slots d'équipement bodylocs (inventory.txt) — disposition papier D2. */
const SLOTS: ReadonlyArray<readonly [string, string, number, number, number, number]> = [
  ['head', 'Tête', 2, 2, 4, 0],
  ['neck', 'Amulette', 1, 1, 7, 0],
  ['torso', 'Torse', 2, 3, 4, 1],
  ['rArm', 'Arme', 2, 3, 2, 1],
  ['lArm', 'Bouclier', 2, 3, 6, 1],
  ['gloves', 'Gants', 2, 2, 0, 3],
  ['rHand', 'Anneau', 1, 1, 2, 4],
  ['lHand', 'Anneau', 1, 1, 3, 4],
  ['belt', 'Ceinture', 2, 1, 4, 4],
  ['feet', 'Bottes', 2, 2, 6, 3],
];

const KIND_COLORS: Readonly<Record<string, [string, string]>> = {
  Potion: ['#b03a2e', '#57150e'],
  Weapon: ['#8d99a6', '#39424c'],
  Armor: ['#8a6d3b', '#41331a'],
  Gold: ['#d9b96a', '#6e5626'],
  Misc: ['#7a8a5a', '#39442a'],
};

// --- voie TS (port fidèle) ---------------------------------------------------
function installTsPanels(win: D2Window): void {
  registerModule('panels-ui', 'ts');
  interface PanelRec { root: HTMLDivElement; body: HTMLDivElement; open: boolean }

  const state = {
    inited: false,
    live: false,
    sizeTable: null as Record<string, [number, number]> | null,
    layout: {} as Record<string, Box>,
    sel: null as (Item & { slot?: number }) | null,
    panels: {} as Record<string, PanelRec>,
    tablesNote: false,
  };

  function el(tag: string, cls?: string, text?: string): HTMLElement {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function game(): D2Game | null {
    return win.__d2?.game ?? null;
  }

  function stats(): { gold?: number; level?: number } {
    const g = game();
    return g ? parseOr(g.heroStats(), {}) : {};
  }

  function invItems(): Array<Item & { slot: number }> {
    const g = game();
    const slots = g ? parseOr<Array<Item | null>>(g.inventory(), []) : [];
    const out: Array<Item & { slot: number }> = [];
    for (let i = 0; i < slots.length; i++) {
      const it = slots[i];
      if (it) out.push({ ...it, slot: i });
    }
    return out;
  }

  function sizeOf(code: string): readonly [number, number] {
    return sizeOfPure(state.sizeTable, code);
  }

  /** Miroir visuel : positions X×Y stables pour la liste d'items moteur. */
  function layoutOf(items: Array<Item & { slot?: number }>): Array<{ item: Item & { slot?: number }; box: Box }> {
    const seen = new Set<string>();
    const blocks: Box[] = [];
    const laid: Array<{ item: Item & { slot?: number }; box: Box }> = [];
    const saved = state.layout;
    // 1) positions persistées encore valides
    for (const it of items) {
      const k = itemKey(it);
      const s = sizeOf(it.code);
      const p = saved[k];
      if (p && p.w === s[0] && p.h === s[1] && !seen.has(k) &&
        p.x + s[0] <= INV_W && p.y + s[1] <= INV_H) {
        seen.add(k);
        const b: Box = { x: p.x, y: p.y, w: s[0], h: s[1] };
        blocks.push(b);
        laid.push({ item: it, box: b });
      }
    }
    // 2) nouvelles entrées : première place libre
    for (const it of items) {
      const k = itemKey(it);
      if (seen.has(k)) continue;
      seen.add(k);
      const s = sizeOf(it.code);
      const spot = firstFitPure(s[0], s[1], INV_W, INV_H, blocks);
      const b: Box = spot ? { x: spot.x, y: spot.y, w: s[0], h: s[1] }
        : { x: 0, y: 0, w: s[0], h: s[1], overflow: true };
      blocks.push(b);
      laid.push({ item: it, box: b });
    }
    // persistance (best effort)
    const out: Record<string, Box> = {};
    for (const l of laid) out[itemKey(l.item)] = l.box;
    state.layout = out;
    try {
      localStorage.setItem(PANELS_KEY, JSON.stringify(out));
    } catch { /* quota */ }
    return laid;
  }

  function loadSizes(): void {
    if (state.sizeTable) return;
    const roots = [new URL('..', location.href), new URL('.', location.href)];
    const files = ['assets/patch_misc.txt', 'assets/patch_weapons.txt', 'assets/patch_armor.txt'];
    const table: Record<string, [number, number]> = {};
    let done = 0;
    let failed = 0;
    for (const root of roots) {
      for (const f of files) {
        fetch(new URL(f, root))
          .then((r) => {
            if (!r.ok) throw new Error(String(r.status));
            return r.text();
          })
          .then((txt) => {
            const lines = txt.split('\n');
            const hdr = lines[0]!.split('\t');
            const iw = hdr.indexOf('invwidth'), ih = hdr.indexOf('invheight'), ic = hdr.indexOf('code');
            if (iw < 0 || ih < 0 || ic < 0) return;
            for (let i = 1; i < lines.length; i++) {
              const cols = lines[i]!.split('\t');
              if (cols.length <= Math.max(iw, ih, ic)) continue;
              const code = cols[ic]!.trim();
              const w = parseInt(cols[iw]!, 10), h = parseInt(cols[ih]!, 10);
              if (code && w > 0 && h > 0) table[code] = [w, h];
            }
          })
          .catch(() => {
            failed++;
          })
          .finally(() => {
            if (++done === roots.length * files.length) {
              if (Object.keys(table).length) state.sizeTable = table;
              else if (!state.tablesNote) state.tablesNote = true;
              refreshOpen();
            }
          });
      }
    }
    void failed;
  }

  function statRow(label: string, value: unknown, cls?: string): HTMLElement {
    const r = el('div', 'prow' + (cls ? ' ' + cls : ''));
    r.appendChild(el('span', 'plab', label));
    r.appendChild(el('span', 'pval', String(value)));
    return r;
  }

  // --- panneau PERSONNAGE (heroPanel — endpoint gardé) ------------------------
  function renderChar(root: HTMLElement): void {
    const g = game();
    const p = parseOr<HeroPanel>(g && g.heroPanel ? g.heroPanel() : '{}', {});
    root.textContent = '';
    if (!p.level) {
      root.appendChild(el('div', 'foot', 'Moteur non initialisé.'));
      return;
    }
    const head = el('div', 'phead');
    head.appendChild(el('h3', undefined, (p.className ?? '') + ' — Niveau ' + p.level));
    const xp = el('div', 'xpbar');
    const xpFill = el('div', 'xpfill');
    xpFill.style.width = (xpFrac(p.level, p.xp ?? 0) * 100).toFixed(1) + '%';
    xp.appendChild(xpFill);
    head.appendChild(xp);
    head.appendChild(el('div', 'foot', 'XP ' + fmt(p.xp ?? 0) + ' — prochain niveau dans ' + fmt(p.xpToNext ?? 0)));
    root.appendChild(head);

    const cols = el('div', 'pcols');
    const left = el('div', 'pcol');
    left.appendChild(el('h4', undefined, 'Caractéristiques'));
    left.appendChild(statRow('Force', p.strength));
    left.appendChild(statRow('Dextérité', p.dexterity));
    left.appendChild(statRow('Vitalité', p.vitality));
    left.appendChild(statRow('Énergie', p.energy));
    left.appendChild(statRow('Points à dépenser', (p.statPoints || 0) + ' / ' + (p.skillPoints || 0) + ' (car. / comp.)', 'dim'));
    left.appendChild(el('h4', undefined, 'Vie et mana'));
    left.appendChild(statRow('Vie', p.life + ' / ' + p.maxLife));
    left.appendChild(statRow('Mana', p.mana + ' / ' + p.maxMana));
    left.appendChild(statRow('Endurance', p.stamina + ' / ' + p.maxStamina));

    const right = el('div', 'pcol');
    right.appendChild(el('h4', undefined, 'Combat'));
    right.appendChild(statRow('Dégâts', p.dmgMin + ' – ' + p.dmgMax, 'gold'));
    right.appendChild(statRow('Arme', p.weapon || '—', 'dim'));
    right.appendChild(statRow('Défense', p.defense));
    right.appendChild(statRow('Toucher', p.toHit));
    right.appendChild(el('h4', undefined, 'Résistances (max ' + (p.resMax ?? 75) + ' %)'));
    const res = (label: string, v: number): void => {
      right.appendChild(statRow(label, (v > 0 ? '+' : '') + v + ' %', v < 0 ? 'neg' : ''));
    };
    res('Feu', p.resFire ?? 0);
    res('Foudre', p.resLight ?? 0);
    res('Froid', p.resCold ?? 0);
    res('Poison', p.resPoison ?? 0);
    right.appendChild(el('h4', undefined, 'Chance de trouver'));
    right.appendChild(statRow('Objets magiques (MF)', '+' + (p.mf ?? 0) + ' %'));
    right.appendChild(statRow('Or (GF)', '+' + (p.gf ?? 0) + ' %'));
    right.appendChild(statRow('Or porté', fmt(p.gold ?? 0) + ' / ' + fmt(p.goldCap ?? 0), 'gold'));
    cols.appendChild(left);
    cols.appendChild(right);
    root.appendChild(cols);
    root.appendChild(el('div', 'foot',
      'Dégâts dérivés de l\'arme (×(1 + Force/100), formule du moteur) · résists : ' +
      'pénalité de difficulté patch_difficultylevels (0/−40/−100) · or : plafond 10 000 × niveau.'));
  }

  // --- INVENTAIRE visuel (10×4 X×Y + ceinture 4×4 + slots silhouette) --------
  function gridEl(w: number, h: number, cell: number): HTMLElement {
    const g = el('div', 'pgrid');
    g.style.gridTemplateColumns = 'repeat(' + w + ', ' + cell + 'px)';
    void h;
    return g;
  }

  function itemBlock(
    laid: { item: Item & { slot?: number }; box: Box },
    onClick: ((it: Item & { slot?: number }, node: HTMLElement) => void) | null,
    onUse: ((it: Item & { slot?: number }) => void) | null,
  ): HTMLElement {
    const it = laid.item;
    const b = laid.box;
    const colors = KIND_COLORS[kindOf(it)] ?? KIND_COLORS.Misc!;
    const d = el('div', 'pitem');
    d.style.left = b.x * CELL + 'px';
    d.style.top = b.y * CELL + 'px';
    d.style.width = b.w * CELL - 2 + 'px';
    d.style.height = b.h * CELL - 2 + 'px';
    d.style.background = colors[0];
    d.style.borderColor = colors[1];
    d.title = it.name + (it.amount ? ' — ' + it.amount + ' or' : '') +
      ' (' + b.w + '×' + b.h + ')' + (onUse ? ' — double-clic : utiliser' : ' — clic : saisir/poser');
    if (onUse) d.addEventListener('dblclick', (e) => { e.stopPropagation(); onUse(it); });
    if (onClick) d.addEventListener('click', (e) => { e.stopPropagation(); onClick(it, d); });
    if (state.sel && itemKey(state.sel) === itemKey(it)) d.classList.add('sel');
    d.appendChild(el('span', 'pitemname', it.name));
    return d;
  }

  function note(m: string): void {
    const s = document.getElementById('msglog');
    if (s) {
      const line = el('div', undefined, '• ' + m);
      s.appendChild(line);
      while (s.children.length > 12) s.removeChild(s.firstChild!);
      s.scrollTop = s.scrollHeight;
    } else {
      console.info('[d2panels]', m);
    }
  }

  function useConsumable(it: Item & { slot?: number }): void {
    const g = game();
    if (!g || (it.slot ?? -1) < 0) return;
    const scrolls = ['tsc', 'tbk', 'isc', 'ibk'];
    if (it.kind !== 'Potion' && !scrolls.includes(it.code)) {
      note(it.name + ' — équipement non branché au moteur (phase 3).');
      return;
    }
    try {
      const r = parseOr<import('./types').UseConsumableResult>(g.useConsumable(it.slot!), {});
      if (r.use) {
        const u = r.use;
        note(u.name + ' : ' + ((u.heal ?? 0) > 0 ? '+' + u.heal + ' pv ' : '') +
          ((u.mana ?? 0) > 0 ? '+' + u.mana + ' mana ' : '') +
          ((u.buffMs ?? 0) > 0 ? 'effet ' + Math.round((u.buffMs ?? 0) / 1000) + ' s ' : '') +
          (u.townPortal ? 'retour au camp' : '').trim());
        if (r.levelSwitch && win.__d2?.refreshStatus) win.__d2.refreshStatus();
      }
    } catch (e) {
      note(e instanceof Error ? e.message : String(e));
    }
    state.sel = null;
    refreshOpen();
  }

  function tryBeltKey(n: number): void {
    const g = game();
    if (!g || !g.useBeltKey) return;
    try {
      const r = parseOr<import('./types').UseConsumableResult>(g.useBeltKey(n), {});
      if (r.use) note(r.use.name + ' bue (touche ' + n + ').');
      else note('Colonne ' + n + ' : vide.');
    } catch (e) {
      note(e instanceof Error ? e.message : String(e));
    }
    refreshOpen();
  }

  function renderInv(root: HTMLElement): void {
    root.textContent = '';
    const items = invItems();
    const laid = layoutOf(items);
    loadSizes();

    // Slots d'équipement en silhouette (inventory.txt bodylocs) — décor.
    const doll = el('div', 'pdoll');
    doll.style.width = (9 * CELL) + 'px';
    for (const s of SLOTS) {
      const d = el('div', 'pslot');
      d.style.left = s[4] * CELL + 'px';
      d.style.top = s[5] * CELL + 'px';
      d.style.width = s[2] * CELL - 2 + 'px';
      d.style.height = s[3] * CELL - 2 + 'px';
      d.title = s[1] + ' — équipement non branché au moteur (silhouette)';
      d.appendChild(el('span', 'pslotname', s[1]));
      doll.appendChild(d);
    }
    root.appendChild(doll);

    // Grille 10×4 — items empilés à leur taille réelle.
    const gridWrap = el('div', 'pgridwrap');
    const grid = gridEl(INV_W, INV_H, CELL);
    grid.style.height = INV_H * CELL + 'px';
    for (let y = 0; y < INV_H; y++) {
      for (let x = 0; x < INV_W; x++) {
        const c = el('div', 'pcell');
        c.style.left = x * CELL + 'px';
        c.style.top = y * CELL + 'px';
        grid.appendChild(c);
      }
    }
    // clic-clic : poser l'item saisi sur une zone libre qui l'accepte.
    grid.addEventListener('click', () => {
      if (!state.sel) return;
      const selKey = itemKey(state.sel);
      const rest = laid.filter((l) => itemKey(l.item) !== selKey).map((l) => l.box);
      const s = sizeOf(state.sel.code);
      const spot = firstFitPure(s[0], s[1], INV_W, INV_H, rest);
      if (!spot) {
        note('Aucune place libre pour ' + state.sel.name + ' (' + s[0] + '×' + s[1] + ').');
        return;
      }
      state.layout[selKey] = { x: spot.x, y: spot.y, w: s[0], h: s[1] };
      note(state.sel.name + ' posé en (' + spot.x + ', ' + spot.y + ').');
      state.sel = null;
      refreshOpen();
    });
    for (const l of laid) {
      grid.appendChild(itemBlock(l, (it) => {
        if (state.sel && itemKey(state.sel) === itemKey(it)) state.sel = null;
        else state.sel = it;
        refreshOpen();
      }, (it) => useConsumable(it)));
    }
    // items qui ne tiennent pas visuellement : compacts sous la grille
    const over = laid.filter((l) => l.box.overflow);
    gridWrap.appendChild(grid);
    root.appendChild(gridWrap);
    if (over.length) {
      const ov = el('div', 'pover');
      for (const l of over) {
        const chip = itemBlock({ item: l.item, box: { x: 0, y: 0, w: l.box.w, h: l.box.h } }, null, null);
        chip.classList.add('chip');
        chip.style.position = 'static';
        ov.appendChild(chip);
      }
      root.appendChild(el('div', 'foot', over.length + ' objet(s) sans place visible (grille visuelle pleine).'));
      root.appendChild(ov);
    }

    // Ceinture 4×4 (consumables_ui.rs — 4 colonnes, potions empilées).
    const g = game();
    const belt = parseOr<import('./types').BeltInfo>(g && g.beltInfo ? g.beltInfo() : '{}', {});
    const beltSec = el('div', 'pbelthead');
    beltSec.appendChild(el('h4', undefined, 'Ceinture (1-4)'));
    const bg = gridEl(BELT_W, BELT_H, CELL);
    bg.style.height = BELT_H * CELL + 'px';
    for (let by = 0; by < BELT_H; by++) {
      for (let bx = 0; bx < BELT_W; bx++) {
        const bc = el('div', 'pcell');
        bc.style.left = bx * CELL + 'px';
        bc.style.top = by * CELL + 'px';
        bg.appendChild(bc);
      }
    }
    const beltArr = (belt && belt.belt) || [];
    for (let col = 0; col < BELT_W; col++) {
      const cells = beltArr[col] ?? [];
      for (let row = 0; row < BELT_H; row++) {
        const cell = cells[row];
        if (!cell) continue;
        const cnum = col;
        const potion = cell.kind === 'Potion' || cell.kind === 'Misc';
        bg.appendChild(itemBlock(
          {
            item: { code: cell.code, name: cell.name, kind: cell.kind, amount: 0, slot: -1 },
            box: { x: cnum, y: row, w: 1, h: 1 },
          },
          null,
          potion ? () => tryBeltKey(cnum + 1) : null,
        ));
      }
    }
    beltSec.appendChild(bg);
    root.appendChild(beltSec);
    root.appendChild(el('div', 'foot',
      'Miroir visuel : le moteur range chaque objet en 1×1 (écart documenté) — ici les tailles réelles ' +
      'invwidth/invheight (patch_*.txt) sont empilées en première place libre ; l\'index moteur est conservé pour les actions.'));
  }

  // --- STASH 6×4 + or (economy.rs réel, endpoint gardé) ------------------------
  function renderStash(root: HTMLElement): void {
    root.textContent = '';
    const g = game();
    const s = parseOr<StashInfo>(g && g.stashInfo ? g.stashInfo() : '{}', {});
    if (!s.w) {
      root.appendChild(el('div', 'foot', 'Moteur non initialisé.'));
      return;
    }
    const head = el('div', 'phead');
    head.appendChild(el('h3', undefined, 'STASH — ' + fmt(s.gold ?? 0) + ' / ' + fmt(s.goldCap ?? 0) + ' or'));
    head.appendChild(el('div', 'foot', 'Difficulté ' + (s.difficulty ?? 'normal') + ' · bourse : ' +
      fmt(stats().gold ?? 0) + ' / ' + fmt(s.carryCap ?? 0) + ' or (10 000 × niveau)'));
    root.appendChild(head);

    function goldBtn(label: string, kind: number | 'dep' | 'wd'): HTMLElement {
      const b = el('button', undefined, label);
      b.addEventListener('click', () => {
        const gg = game();
        if (!gg || !gg.stashDeposit || !gg.stashWithdraw) return;
        const info = parseOr<StashInfo>(gg.stashInfo ? gg.stashInfo() : '{}', {});
        const carried = stats().gold ?? 0;
        const dep = (typeof kind === 'number' && kind > 0) || kind === 'dep';
        let amt: number;
        if (kind === 'dep') amt = Math.min(carried, Math.max(0, (info.goldCap ?? 0) - (info.gold ?? 0)));
        else if (kind === 'wd') amt = info.gold ?? 0;
        else amt = Math.abs(kind);
        if (amt <= 0) {
          note(dep ? 'Dépôt refusé : ' + (carried <= 0 ? 'bourse vide.' : 'stash plein.')
            : 'Retrait refusé : stash vide.');
          return;
        }
        try {
          const v = parseOr<{ gold?: number }>(dep ? gg.stashDeposit!(amt) : gg.stashWithdraw!(amt), {});
          note((dep ? 'Déposé ' : 'Retiré ') + fmt(amt) + ' or (stash ' + fmt(v.gold ?? 0) + ').');
        } catch (e) {
          note((e instanceof Error ? e.message : String(e)).replace(/^Error: /, ''));
        }
        refreshOpen();
      });
      return b;
    }
    const row = el('div', 'pgoldrow');
    row.appendChild(goldBtn('Déposer 100', 100));
    row.appendChild(goldBtn('Déposer 1 000', 1000));
    row.appendChild(goldBtn('Déposer tout', 'dep'));
    row.appendChild(goldBtn('Retirer 100', -100));
    row.appendChild(goldBtn('Retirer 1 000', -1000));
    row.appendChild(goldBtn('Retirer tout', 'wd'));
    root.appendChild(row);

    // Grille 6×4 — items EquipItem avec taille + position RÉELLES (moteur).
    const grid = gridEl(s.w ?? 6, s.h ?? 4, CELL);
    grid.style.height = (s.h ?? 4) * CELL + 'px';
    for (let y = 0; y < (s.h ?? 4); y++) {
      for (let x = 0; x < (s.w ?? 6); x++) {
        const c = el('div', 'pcell');
        c.style.left = x * CELL + 'px';
        c.style.top = y * CELL + 'px';
        grid.appendChild(c);
      }
    }
    for (const it of s.items ?? []) {
      const colors = KIND_COLORS.Misc!;
      const d = el('div', 'pitem');
      d.style.left = it.x * CELL + 'px';
      d.style.top = it.y * CELL + 'px';
      d.style.width = it.w * CELL - 2 + 'px';
      d.style.height = it.h * CELL - 2 + 'px';
      d.style.background = colors[0];
      d.style.borderColor = colors[1];
      d.title = it.name + ' (' + it.w + '×' + it.h + ') — taille réelle du moteur';
      d.appendChild(el('span', 'pitemname', it.name));
      grid.appendChild(d);
    }
    root.appendChild(grid);
    root.appendChild(el('div', 'foot',
      'Dépôt/retrait : branchement economy.rs RÉEL (bourse débitée/créditée, plafonds ' +
      '300k/500k/800k par difficulté, porté 10 000 × niveau) — save additive `gold_stash`.'));
  }

  function refreshOpen(): void {
    for (const id of Object.keys(state.panels)) {
      const p = state.panels[id]!;
      if (p.open) render(id);
    }
  }

  function render(id: string): void {
    const p = state.panels[id];
    if (!p) return;
    if (id === 'char') renderChar(p.body);
    else if (id === 'inv') renderInv(p.body);
    else if (id === 'stash') renderStash(p.body);
  }

  function buildPanel(id: 'char' | 'inv' | 'stash', title: string): PanelRec {
    const root = el('div', 'd2panel') as HTMLDivElement;
    root.id = 'd2panel-' + id;
    const bar = el('div', 'pbar');
    bar.appendChild(el('span', 'ptitle', title));
    const close = el('button', 'pclose', '×');
    close.title = 'Fermer (Échap)';
    (close as HTMLButtonElement).addEventListener('click', () => togglePanel(id, false));
    bar.appendChild(close);
    root.appendChild(bar);
    const body = el('div', 'pbody') as HTMLDivElement;
    root.appendChild(body);
    document.body.appendChild(root);
    const rec: PanelRec = { root, body, open: false };
    state.panels[id] = rec;
    return rec;
  }

  function togglePanel(id: 'char' | 'inv' | 'stash', force?: boolean): void {
    const p = state.panels[id] ?? buildPanel(
      id, id === 'char' ? 'PERSONNAGE' : id === 'inv' ? 'INVENTAIRE' : 'STASH');
    p.open = force !== undefined ? force : !p.open;
    p.root.style.display = p.open ? 'block' : 'none';
    if (p.open) {
      render(id);
      // fermeture mutuelle (D2 : un seul écran plein à la fois)
      for (const other of Object.keys(state.panels)) {
        if (other !== id && state.panels[other]!.open) togglePanel(other as 'char', false);
      }
    }
  }

  function injectCss(): void {
    const css = el('style');
    css.textContent =
      '.d2panel{position:fixed;top:64px;z-index:60;display:none;width:auto;max-width:94vw;' +
      'background:#141019f5;border:1px solid #5a4a2a;border-radius:4px;padding:10px;' +
      'color:#e8dcbf;font:11px/1.35 system-ui,sans-serif;box-shadow:0 4px 18px #000d}' +
      '#d2panel-char{left:16px}#d2panel-inv{left:50%;transform:translateX(-50%)}#d2panel-stash{right:16px}' +
      '.d2panel .pbar{display:flex;justify-content:space-between;align-items:center;margin-bottom:6px}' +
      '.d2panel .ptitle{color:#c7a45a;font-weight:700;letter-spacing:.1em;font-size:12px}' +
      '.d2panel .pclose{background:#241c10;color:#c7a45a;border:1px solid #4a3c22;border-radius:3px;' +
      'width:20px;height:20px;cursor:pointer;font-weight:700}' +
      '.d2panel .pclose:hover{border-color:#c7a45a;color:#ffe9ad}' +
      '.d2panel h3{margin:0 0 4px;color:#c7a45a;font-size:12px;letter-spacing:.08em}' +
      '.d2panel h4{margin:8px 0 3px;color:#a89670;font-size:10px;text-transform:uppercase;letter-spacing:.09em;' +
      'border-bottom:1px solid #2e2718;padding-bottom:2px}' +
      '.d2panel .phead{margin-bottom:6px}.d2panel .phead .foot{margin-top:2px}' +
      '.d2panel .pcols{display:flex;gap:18px}.d2panel .pcol{min-width:172px}' +
      '.d2panel .prow{display:flex;justify-content:space-between;gap:12px;padding:1px 0}' +
      '.d2panel .plab{color:#b8ad98}.d2panel .pval{color:#f0e6d2;font-weight:600}' +
      '.d2panel .prow.gold .pval{color:#d9b96a}.d2panel .prow.dim .pval{color:#8f8676;font-weight:400}' +
      '.d2panel .prow.neg .pval{color:#d8503c}' +
      '.d2panel .xpbar{height:7px;background:#16120ccc;border:1px solid #000000aa;border-radius:3px;' +
      'margin:3px 0 2px;overflow:hidden}' +
      '.d2panel .xpfill{height:100%;background:linear-gradient(#d8b23a,#7a5c14);border-radius:2px}' +
      '.d2panel .foot{font:9px system-ui;color:#8f8676;margin-top:6px;max-width:420px}' +
      '.pgridwrap{position:relative}.pgrid{position:relative;display:grid;margin:4px 0;' +
      'background:#100d14;border:1px solid #3a3226;border-radius:2px}' +
      '.pcell{position:absolute;background:#1d1712;border:1px solid #3a3226;' +
      'box-shadow:inset 0 1px 0 #ffffff08}' +
      '.pitem{position:absolute;box-sizing:border-box;border:1px solid;border-radius:2px;' +
      'display:flex;align-items:flex-end;justify-content:center;overflow:hidden;cursor:pointer;' +
      'box-shadow:inset 0 1px 0 #ffffff22,0 1px 3px #000a;z-index:2}' +
      '.pitem:hover{outline:1px solid #ffe9ad;filter:brightness(1.15)}' +
      '.pitem.sel{outline:2px solid #d9b96a;box-shadow:0 0 8px #d9b96a88}' +
      '.pitemname{font:8px system-ui;color:#ffe9b0;text-shadow:0 1px 2px #000;padding:0 2px;' +
      'max-height:100%;overflow:hidden;word-break:break-word;text-align:center}' +
      '.pitem.chip{position:static;display:inline-flex;min-width:52px;min-height:20px;padding:2px 6px;margin:2px}' +
      '.pdoll{position:relative;height:' + (5 * CELL) + 'px;margin-bottom:8px;' +
      'background:#100d14;border:1px solid #3a3226;border-radius:2px}' +
      '.pslot{position:absolute;box-sizing:border-box;background:repeating-linear-gradient(45deg,#181318,#181318 4px,#1d1712 4px,#1d1712 8px);' +
      'border:1px dashed #3a3226;border-radius:2px;display:flex;align-items:center;justify-content:center}' +
      '.pslotname{font:8px system-ui;color:#5f5648;letter-spacing:.05em}' +
      '.pbelthead h4{margin-top:10px}' +
      '.pgoldrow{display:flex;flex-wrap:wrap;gap:4px;margin-bottom:6px}' +
      '.pgoldrow button{padding:3px 7px;font-size:11px;background:#241c10;color:#d9b96a;' +
      'border:1px solid #4a3c22;border-radius:3px;cursor:pointer}' +
      '.pgoldrow button:hover{border-color:#c7a45a;color:#ffe9ad}';
    document.head.appendChild(css);
  }

  function injectButtons(): void {
    const toolbar = document.getElementById('toolbar');
    if (!toolbar) return;
    const defs: Array<[string, string, 'char' | 'inv' | 'stash']> = [
      ['Perso', 'Panneau personnage (C)', 'char'],
      ['Inventaire', 'Inventaire (I)', 'inv'],
      ['Stash', 'Stash 6×4 + or (B)', 'stash'],
    ];
    for (const [label, title, id] of defs) {
      const b = el('button', undefined, label) as HTMLButtonElement;
      b.id = 'btn-panel-' + id;
      b.title = title;
      b.addEventListener('click', () => togglePanel(id));
      toolbar.appendChild(b);
    }
  }

  function onKey(e: KeyboardEvent): void {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    const k = (e.key || '').toLowerCase();
    if (k === 'i') {
      // capture AVANT game.js : CE panneau remplace l'inventaire canvas 1×1
      e.preventDefault();
      e.stopImmediatePropagation();
      togglePanel('inv');
    } else if (k === 'c') {
      e.preventDefault();
      e.stopImmediatePropagation();
      togglePanel('char');
    } else if (k === 'b') {
      e.preventDefault();
      e.stopImmediatePropagation();
      togglePanel('stash');
    } else if (e.key === 'Escape') {
      for (const id of Object.keys(state.panels)) {
        if (state.panels[id]!.open) togglePanel(id as 'char' | 'inv' | 'stash', false);
      }
    } // touches 1-4 (ceinture) : laissées à game.js — aucun double usage
  }

  function init(): void {
    if (state.inited) return;
    state.inited = true;
    try {
      state.layout = parseOr<Record<string, Box>>(localStorage.getItem(PANELS_KEY) || '{}', {});
    } catch {
      state.layout = {};
    }
    injectCss();
    injectButtons();
    // CAPTURE + enregistrement au plus tôt : ce module est importé
    // DYNAMIQUEMENT au bas de game.js, donc AVANT que wireCommon() (async,
    // post-wasm) ne pose SES listeners keydown — nos stopImmediatePropagation
    // gagnent, l'inventaire canvas 1×1 de game.js ne s'ouvre plus.
    window.addEventListener('keydown', onKey, true);
  }

  function initLive(): void {
    // moteur prêt : rafraîchissement doux des panneaux ouverts (or/vie/XP)
    if (state.live) return;
    state.live = true;
    setInterval(refreshOpen, 1500);
  }

  init();
  (function whenReady(): void {
    const g = game();
    if (g && g.isFinalized && g.isFinalized()) initLive();
    else setTimeout(whenReady, 400);
  })();

  win.D2Panels = {
    toggle: togglePanel,
    open: (id) => togglePanel(id, true),
    refresh: refreshOpen,
    state: state as unknown as Record<string, unknown>,
  };
}

export function installPanels(win: D2Window): void {
  if (win.D2Panels) {
    registerModule('panels-ui', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : panels-ui.js Legacy actif — on adopte
  installTsPanels(win);
}

if (typeof window !== 'undefined') {
  await warmLegacy('panels-ui.js');
  installPanels(window as unknown as D2Window);
}
