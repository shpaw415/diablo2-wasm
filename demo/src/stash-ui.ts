// Diablo 2 WASM — demo/stash-ui (étape 11c : économie & inventaire — STASH)
// — migration TS 25a de demo/stash-ui.js. Contrat window.D2Stash INCHANGÉ :
// init(dom, hooks) / toggle / depositGold / withdrawGold / depositItem /
// state.
//
// Miroir des règles de d2rules/src/economy.rs (le moteur Rust fait foi) :
//   plafond or PORTÉ = 10 000 × niveau (formule D2),
//   plafond or STASH = 300 000 / 500 000 / 800 000 (Normal/NM/Enfer),
//   grille 6×4 « Bank Page » (inventory.txt), première place libre.
// PERSISTANCE : localStorage « d2w.stash.v1 » (miroir démo).
//
// TRANSITION : stash-ui.js est importé DYNAMIQUEMENT par game.js — on
// warm-importe la même URL AVANT de remplacer le contrat (cache modules ES
// = exécution unique ; l'import tardif de game.js est un no-op).
import {   warmLegacy } from './legacy';
import { registerModule } from './types';
import type { D2StashApi, D2StashHooks, D2Window } from './types';

export const STASH_KEY = 'd2w.stash.v1';
export const STASH_W = 6, STASH_H = 4;           // Bank Page 6×4 (inventory.txt)
export const CARRY_CAP_PER_LEVEL = 10000;        // or porté : 10 000 × niveau (D2)
export const STASH_CAP: Readonly<Record<string, number>> = {
  normal: 300000, nightmare: 500000, hell: 800000,
};

export interface StashStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

interface StashState {
  gold: number;
  cells: Array<{ name: string } | null>;
  hooks: D2StashHooks | null;
  btn: HTMLButtonElement | null;
  panel: HTMLDivElement | null;
}

const state: StashState = { gold: 0, cells: [], hooks: null, btn: null, panel: null };

let storage: StashStorage | null = null;
function store(): StashStorage {
  if (storage) return storage;
  try {
    const ls = globalThis.localStorage;
    if (ls) return ls;
  } catch { /* stockage indisponible */ }
  const mem = new Map<string, string>();
  return {
    getItem: (k) => (mem.has(k) ? (mem.get(k) as string) : null),
    setItem: (k, v) => void mem.set(k, String(v)),
  };
}

/** Injection test (bun). */
export function setStashStorage(s: StashStorage | null): void {
  storage = s;
}

function load(): void {
  try {
    const raw: { gold?: number; cells?: unknown } = JSON.parse(store().getItem(STASH_KEY) || '{}');
    state.gold = (raw.gold ?? 0) >>> 0;
    state.cells = Array.isArray(raw.cells)
      ? (raw.cells as Array<{ name: string } | null>).slice(0, STASH_W * STASH_H)
      : [];
    while (state.cells.length < STASH_W * STASH_H) state.cells.push(null);
  } catch {
    state.gold = 0;
    state.cells = [];
  }
}

function persist(): void {
  try {
    store().setItem(STASH_KEY, JSON.stringify({ gold: state.gold, cells: state.cells }));
  } catch { /* quota : la session reste utilisable, juste non persistée */ }
}

/** Première place libre w×h sur la grille (balayage rangée/rangée) —
 * pur, testable. */
export function firstFit(
  w: number, h: number,
  cells: ReadonlyArray<{ name: string } | null>,
): { x: number; y: number } | null {
  const occupied = (x: number, y: number): boolean => {
    for (let cy = y; cy < y + h; cy++) {
      for (let cx = x; cx < x + w; cx++) {
        if (cells[cy * STASH_W + cx]) return true;
      }
    }
    return false;
  };
  for (let y = 0; y <= STASH_H - h; y++) {
    for (let x = 0; x <= STASH_W - w; x++) {
      if (!occupied(x, y)) return { x, y };
    }
  }
  return null;
}

/** Dépôt d'or miroir (plafond stash par difficulté) — pur, testable. */
export function depositGoldPure(
  n: number, _gold: number, stashGold: number, diff: string,
): { done: number; left: number; newStash: number } {
  const room = (STASH_CAP[diff] ?? STASH_CAP.normal!) - stashGold;
  const done = Math.min(n >>> 0, Math.max(0, room));
  return { done, left: (n >>> 0) - done, newStash: stashGold + done };
}

/** Retrait d'or miroir (plafond porté 10 000 × niveau) — pur, testable. */
export function withdrawGoldPure(
  n: number, gold: number, stashGold: number, level: number,
): { done: number; left: number; newStash: number } {
  const room = Math.max(0, CARRY_CAP_PER_LEVEL * Math.max(1, level) - gold);
  const done = Math.min(n >>> 0, stashGold, room);
  return { done, left: (n >>> 0) - done, newStash: stashGold - done };
}

// --- hooks par défaut : lecture window.__d2 (moteur wasm) ------------------
function stats(): { gold?: number; level?: number } {
  const g = typeof window !== 'undefined' ? (window as D2Window).__d2?.game : null;
  try {
    return g ? JSON.parse(g.heroStats()) : {};
  } catch {
    return {};
  }
}

function diffName(): string {
  const g = typeof window !== 'undefined' ? (window as D2Window).__d2?.game : null;
  try {
    const raw = g ? String(g.difficultyInfo()).toLowerCase() : 'normal';
    return raw.includes('night') || raw.includes('cauch') ? 'nightmare'
      : raw.includes('hell') || raw.includes('enfer') ? 'hell' : 'normal';
  } catch {
    return 'normal';
  }
}

function defaultHooks(): D2StashHooks {
  return {
    gold: () => (stats().gold ?? 0) >>> 0,
    level: () => (stats().level ?? 1) >>> 0,
    diff: diffName,
    spend: () => false, // débit réel : branchement wasm restant
    earn: () => {},
  };
}

function stashCap(): number {
  return STASH_CAP[state.hooks?.diff() ?? 'normal'] ?? STASH_CAP.normal!;
}

function carryCap(): number {
  return CARRY_CAP_PER_LEVEL * Math.max(1, state.hooks ? state.hooks.level() : 1);
}

// --- DOM -------------------------------------------------------------------
function el(tag: string, cls?: string, text?: string): HTMLElement {
  const e = document.createElement(tag);
  if (cls) e.className = cls;
  if (text !== undefined) e.textContent = text;
  return e;
}

function logLine(m: string): void {
  const s = document.getElementById('status');
  if (s) s.textContent = s.textContent!.replace(/ · \d+ fps.*$/, '') + ' — ' + m;
}

function render(): void {
  const p = state.panel;
  const h = state.hooks;
  if (!p || !h) return;
  p.innerHTML = '';
  p.appendChild(el('h3', undefined, 'STASH — or : ' + state.gold + ' / ' + stashCap() +
    ' · bourse : ' + h.gold() + ' / ' + carryCap()));

  // Dépôt / retrait d'or (débit/crédit réels sur le miroir, plafonds D2).
  const goldRow = el('div', 'goldrow');
  const depositDefs: Array<[string, number]> = [
    ['Déposer 100', 100], ['Déposer 1 000', 1000], ['Déposer tout', Infinity],
  ];
  for (const [label, amount] of depositDefs) {
    const b = el('button', undefined, label);
    b.addEventListener('click', () => {
      const have = h.gold();
      const room = stashCap() - state.gold;
      const n = Math.min(amount, have, Math.max(0, room));
      if (n <= 0) {
        logLine('Dépôt refusé : ' + (room <= 0 ? 'stash plein' : 'bourse vide') + '.');
        return;
      }
      state.gold += n;
      persist();
      render();
      logLine('Déposé ' + n + ' or au stash (bourse ' + have + ' → ' + (have - n) + ').');
    });
    goldRow.appendChild(b);
  }
  const withdrawDefs: Array<[string, number]> = [
    ['Retirer 100', 100], ['Retirer 1 000', 1000], ['Retirer tout', Infinity],
  ];
  for (const [label, amount] of withdrawDefs) {
    const b = el('button', undefined, label);
    b.addEventListener('click', () => {
      const have = h.gold();
      const room = carryCap() - have;
      const n = Math.min(amount, state.gold, Math.max(0, room));
      if (n <= 0) {
        logLine('Retrait refusé : ' + (room <= 0 ? 'bourse au plafond 10 000 × niveau' : 'stash vide') + '.');
        return;
      }
      state.gold -= n;
      persist();
      render();
      logLine('Retiré ' + n + ' or du stash (bourse → ' + (have + n) + ').');
    });
    goldRow.appendChild(b);
  }
  p.appendChild(goldRow);

  // Grille 6×4 (Bank Page) — items du miroir (1×1, écart documenté).
  const grid = el('div', 'grid');
  for (let i = 0; i < STASH_W * STASH_H; i++) {
    const c = state.cells[i];
    const cell = el('div', 'cell' + (c ? ' full' : ''), c ? c.name : '');
    if (c) {
      cell.title = c.name + ' — clic : reprendre (miroir démo)';
      const idx = i;
      cell.addEventListener('click', () => {
        const item = state.cells[idx];
        logLine('Repris : ' + (item ? item.name : '?') + ' (miroir démo — branchement wasm restant).');
        state.cells[idx] = null;
        persist();
        render();
      });
    }
    grid.appendChild(cell);
  }
  p.appendChild(grid);
  p.appendChild(el('div', 'foot', 'Plafonds D2 : porté 10 000 × niveau · stash 300k/500k/800k ' +
    '(Normal/NM/Enfer) — economy.rs · save additive `gold_stash` (rétro-compat v3).'));
}

function initStash(dom?: { toolbar?: HTMLElement; log?: (m: string) => void }, hooks?: D2StashHooks): void {
  load();
  state.hooks = hooks ?? defaultHooks();
  const toolbar = (dom && dom.toolbar) || document.getElementById('toolbar');
  if (toolbar && !state.btn) {
    const btn = el('button', undefined, 'Stash') as HTMLButtonElement;
    btn.id = 'btn-stash';
    btn.title = 'Stash 6×4 + or (étape 11c — economy.rs)';
    toolbar.appendChild(btn);
    btn.addEventListener('click', toggleStash);
    state.btn = btn;
  }
  if (!state.panel) {
    const css = el('style');
    css.textContent =
      '#d2stash{position:fixed;right:10px;top:64px;width:262px;max-height:70vh;overflow:auto;' +
      'background:#141019f2;border:1px solid #5a4a2a;border-radius:4px;padding:8px;z-index:50;display:none}' +
      '#d2stash h3{margin:0 0 6px;color:#c7a45a;font:600 12px system-ui;letter-spacing:.08em}' +
      '#d2stash .goldrow{display:flex;flex-wrap:wrap;gap:4px;margin-bottom:6px}' +
      '#d2stash .goldrow button{padding:3px 6px;font-size:11px}' +
      '#d2stash .grid{display:grid;grid-template-columns:repeat(6,1fr);gap:2px}' +
      '#d2stash .cell{aspect-ratio:1;background:#1d1712;border:1px solid #3a3226;border-radius:2px;' +
      'font:9px system-ui;color:#8f8676;display:flex;align-items:center;justify-content:center;' +
      'overflow:hidden;text-align:center;padding:1px}' +
      '#d2stash .cell.full{border-color:#c7a45a;color:#ffe9b0;cursor:pointer}' +
      '#d2stash .foot{margin-top:6px;font:9px system-ui;color:#8f8676}';
    document.head.appendChild(css);
    const panel = el('div') as HTMLDivElement;
    panel.id = 'd2stash';
    document.body.appendChild(panel);
    state.panel = panel;
  }
  render();
}

function toggleStash(): void {
  if (!state.panel) {
    initStash();
    if (!state.panel) return;
  }
  const on = state.panel.style.display !== 'block';
  state.panel.style.display = on ? 'block' : 'none';
  if (on) render();
}

export function installStash(win: D2Window): void {
  registerModule('stash-ui', 'ts');
  const api: D2StashApi = {
    init: initStash,
    toggle: toggleStash,
    /** Miroir (démo) : dépôt d'or avec les plafonds D2. */
    depositGold(n: number) {
      const res = depositGoldPure(n, 0, state.gold, state.hooks ? state.hooks.diff() : 'normal');
      state.gold = res.newStash;
      persist();
      return { done: res.done, left: res.left };
    },
    /** Miroir (démo) : retrait d'or sous plafond porté 10 000 × niveau. */
    withdrawGold(n: number) {
      const res = withdrawGoldPure(
        n, state.hooks ? state.hooks.gold() : 0, state.gold, state.hooks ? state.hooks.level() : 1);
      state.gold = res.newStash;
      persist();
      return { done: res.done, left: res.left };
    },
    /** Item 1×1 (miroir démo) : (x,y) première place libre ou null. */
    depositItem(name: string) {
      const spot = firstFit(1, 1, state.cells);
      if (!spot) return null;
      state.cells[spot.y * STASH_W + spot.x] = { name: String(name) };
      persist();
      return spot;
    },
    state() {
      return { gold: state.gold, cells: state.cells.slice() };
    },
  };
  win.D2Stash = api;
}

// Transition : le Legacy stash-ui.js est importé dynamiquement par game.js —
// exécution unique garantie via le cache modules (même URL), puis la voie TS
// remplace le contrat.
if (typeof window !== 'undefined') {
  await warmLegacy('stash-ui.js');
  installStash(window as unknown as D2Window);
}
