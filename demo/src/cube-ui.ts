// Diablo 2 WASM — demo/cube (étape 18c : UI fidèle — CUBE HORADRIM) —
// migration TS 25a de demo/cube-ui.js. Panneau Cube 3×4 + zone de
// transmutation + tooltips items fidèles (fr) ; clic-select / clic-place ;
// « Transmuter » = LE MOTEUR FAIT FOI (145 recettes réelles v1.12 via
// cubeTransmute) ; tooltips via cubeItemTooltip (inventaire / ceinture /
// sol). Endpoints absents du pkg wasm de certains arbres : appels gardés.
//
// CONTRAT window.D2CubeUI INCHANGÉ : open/close/toggle (touche C).
// Parties pures testables : libellés, couleurs/qualité, tooltip HTML.
//
// TRANSITION : cube-ui.js est importé DYNAMIQUEMENT par game.js avec un
// garde idempotent et pose SON listener keydown au scope. Warm-import de la
// même URL (exécution unique via le cache modules) puis adoption du Legacy ;
// la voie TS complète prend le relais sans Legacy.
import {   warmLegacy } from './legacy';
import { registerModule } from './types';
import { parseOr, type D2Window, type Item } from './types';

export const CUBE_W = 800, CUBE_H = 600;        // espace canvas (game.html)
export const CUBE_INV_COLS = 10, CUBE_INV_ROWS = 4;
export const CUBE_CELL = 26, CUBE_PAD = 10, CUBE_HEAD = 26;
export const CUBE_COLS = 3, CUBE_ROWS = 4;      // patch_inventory.txt : boîte 3×4

/** Couleurs D2 par catégorie (miroir ITEM_COLORS game.js). */
export const KIND_COLORS: Readonly<Record<string, [string, string]>> = {
  Potion: ['#b03a2e', '#57150e'],
  Weapon: ['#8d99a6', '#39424c'],
  Armor: ['#8a6d3b', '#41331a'],
  Gold: ['#d9b96a', '#6e5626'],
  Misc: ['#7d6f58', '#3c3428'],
};

export const QUALITY_HEX: Readonly<Record<string, string>> = {
  Magic: '#6969ff', Rare: '#ffff64', Unique: '#a59263', Set: '#00ff00', Crafted: '#ff8040',
};

/** Libellé court d'un item dans une cellule (pur, testable). */
export function itemLabel(it: Item | null | undefined): string {
  if (!it) return '';
  if (it.kind === 'Gold') return 'or';
  const n = String(it.name || it.code || '');
  return n.length > 5 ? n.slice(0, 5) : n;
}

/** Paire de couleurs d'un item (pur). */
export function itemColor(it: Item): [string, string] {
  return KIND_COLORS[it.kind] ?? KIND_COLORS.Misc!;
}

export function escapeHtml(s: unknown): string {
  return String(s).replace(/[&<>"]/g, (c) =>
    ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c] ?? c));
}

export interface CubeTooltip {
  name?: string;
  colorHex?: string;
  baseName?: string;
  lines?: string[];
}

/** Tooltip fidèle en HTML (pur, testable — nom coloré + lignes bleues). */
export function tipHtml(tip: CubeTooltip): string {
  let html = '';
  const name = tip.name ?? '';
  html += '<div style="color:' + (tip.colorHex || '#e8e8e8') + ';font-weight:700">' +
    escapeHtml(name) + '</div>';
  if (tip.baseName) {
    html += '<div style="color:#e8e8e8">' + escapeHtml(tip.baseName) + '</div>';
  }
  const lines = tip.lines || [];
  for (const line of lines) {
    html += '<div style="color:#7f7fff">' + escapeHtml(line) + '</div>';
  }
  return html;
}

/** Payload de transmutation (pur). */
export function transmutePayload(slots: number[], nowMs: number): string {
  return JSON.stringify({ slots, seed: nowMs & 0x7fffffff });
}

// --- voie TS (port fidèle) ---------------------------------------------------
function installTsCube(win: D2Window): void {
  registerModule('cube-ui', 'ts');
  interface Placed { slot: number; cell: number }

  const state = {
    open: false,
    built: false,
    selected: -1,
    placed: [] as Placed[],
    products: {} as Record<number, Item & { rolled?: { quality?: string; affixIds?: number[]; sockets?: number; minDmg?: number; maxDmg?: number; ac?: number; reqStr?: number; reqDex?: number; reqLevel?: number; runeword?: string } }>,
    message: '',
    msgErr: false,
    resultTip: null as CubeTooltip | null,
    root: null as HTMLDivElement | null,
    overlay: null as HTMLCanvasElement | null,
    octx: null as CanvasRenderingContext2D | null,
    invWrap: null as HTMLDivElement | null,
    cubeWrap: null as HTMLDivElement | null,
    msgWrap: null as HTMLDivElement | null,
    btnTrans: null as HTMLButtonElement | null,
    belt: null as HTMLDivElement | null,
    tipEl: null as HTMLDivElement | null,
  };
  let lastSig = '';

  function el(tag: string, cls?: string, parent?: HTMLElement, text?: string): HTMLElement {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    (parent ?? state.root!).appendChild(e);
    return e;
  }

  function game(): import('./types').D2Game | null {
    return win.__d2?.game ?? null;
  }

  function invCache(): Array<Item | null> {
    return win.__d2?.invCache ?? [];
  }

  function build(): void {
    if (state.built) return;
    state.built = true;
    const stage = document.getElementById('stage') || document.body;
    const root = document.createElement('div');
    root.id = 'd2cube-root';
    stage.appendChild(root);
    state.root = root;

    const css = [
      '#d2cube-root { position:absolute; inset:0; pointer-events:none; z-index:45;',
      '  font:12px monospace; color:#d0b070; }',
      '#d2cube-root * { box-sizing:border-box; }',
      '#d2cube-tip { position:absolute; z-index:60; pointer-events:none; display:none;',
      '  max-width:280px; background:#141019f2; border:1px solid #5a4a2a; padding:6px 9px;',
      '  font:11px/1.45 monospace; color:#e8e8e8; white-space:pre-wrap;',
      '  box-shadow:0 4px 14px #000d; }',
      '#d2cube-panel { position:absolute; left:50%; top:44px; transform:translateX(-50%);',
      '  display:none; pointer-events:auto; width:560px;',
      '  background:rgba(16,12,6,.95); border:2px solid #5a3c1e; padding:10px 14px 12px;',
      '  box-shadow:0 6px 24px #000d; }',
      '#d2cube-panel.open { display:block; }',
      '#d2cube-panel h3 { margin:0 0 8px; text-align:center; letter-spacing:2px;',
      '  color:#e8dcbf; font:700 13px monospace; text-transform:uppercase; }',
      '.d2cube-cols { display:flex; gap:16px; justify-content:center; }',
      '.d2cube-side { text-align:center; }',
      '.d2cube-side .cap { color:#a89670; margin:2px 0 4px; letter-spacing:1px; }',
      '.d2cube-grid { display:grid; gap:3px; padding:6px; background:#171208;',
      '  border:1px solid #5a3c1e; }',
      '.d2cube-cell { width:32px; height:32px; background:#2c2c34; cursor:pointer;',
      '  border:1px solid #1c1c22; position:relative; padding:0;',
      '  box-shadow:inset 1px 1px 0 #ffffff10, inset -1px -1px 0 #00000088; }',
      '.d2cube-cell:hover { outline:1px solid #c7a45a; }',
      '.d2cube-cell.sel { outline:2px solid #ffe9ad; }',
      '.d2cube-cell .it { position:absolute; inset:3px; border:1px solid #000000aa;',
      '  font:9px monospace; color:#f0e8d0; overflow:hidden; display:flex;',
      '  align-items:center; justify-content:center; text-align:center; }',
      '.d2cube-actions { display:flex; gap:8px; justify-content:center; margin-top:10px; }',
      '#d2cube-panel button { cursor:pointer; background:linear-gradient(#3a2f1c,#241c10);',
      '  color:#d9b96a; border:1px solid #5a4a2a; padding:4px 12px; font:700 11px monospace;',
      '  letter-spacing:.05em; }',
      '#d2cube-panel button:hover:not(:disabled) { border-color:#c7a45a; color:#ffe9ad; }',
      '#d2cube-panel button:disabled { opacity:.45; cursor:default; }',
      '#d2cube-msg { min-height:16px; margin-top:8px; text-align:center; }',
      '#d2cube-msg.err { color:#ffb59a; } #d2cube-msg.ok { color:#d9b96a; }',
      '#d2cube-result { margin-top:4px; text-align:center; font:11px/1.5 monospace;',
      '  color:#e8e8e8; white-space:pre-wrap; }',
      '#d2cube-belt { position:absolute; left:50%; bottom:6px; transform:translateX(-50%);',
      '  display:flex; gap:8px; align-items:flex-end; pointer-events:auto; }',
      '#d2cube-belt .belt-cols { display:flex; gap:4px; padding:5px; background:#171208cc;',
      '  border:1px solid #5a3c1e; }',
      '.d2cube-beltcol { display:flex; flex-direction:column; gap:3px; }',
      '.d2cube-beltcol .bkey { text-align:center; color:#8f7a50; font:9px monospace; }',
      '.d2cube-beltcol .bcell { width:24px; height:24px; background:#2c2c34;',
      '  border:1px solid #1c1c22; position:relative; }',
      '.d2cube-beltcol .bcell .it { position:absolute; inset:2px; display:flex;',
      '  align-items:center; justify-content:center; font:9px monospace; color:#f0e8d0; }',
      '#d2cube-belt .cubebtn { width:auto; height:auto; padding:6px 8px; }',
    ].join('\n');
    const st = document.createElement('style');
    st.textContent = css;
    document.head.appendChild(st);

    // Canvas d'overlay (surlignage inventaire / items « au cube »).
    const overlay = document.createElement('canvas');
    overlay.width = CUBE_W;
    overlay.height = CUBE_H;
    overlay.style.cssText = 'position:absolute; inset:0; pointer-events:none;';
    root.appendChild(overlay);
    state.overlay = overlay;
    state.octx = overlay.getContext('2d');

    // Panneau cube.
    const panel = el('div', '', root) as HTMLDivElement;
    panel.id = 'd2cube-panel';
    const h3 = el('h3', '', panel, 'Cube Horadrim');
    h3.title = 'Touche C pour ouvrir/fermer';
    const cols = el('div', 'd2cube-cols', panel);
    const sideInv = el('div', 'd2cube-side', cols);
    el('div', 'cap', sideInv, 'Inventaire (10×4)');
    const invWrap = el('div', 'd2cube-grid', sideInv) as HTMLDivElement;
    invWrap.style.gridTemplateColumns = 'repeat(' + CUBE_INV_COLS + ', 32px)';
    state.invWrap = invWrap;
    const sideCube = el('div', 'd2cube-side', cols);
    el('div', 'cap', sideCube, 'Cube de transmutation (3×4)');
    const cubeWrap = el('div', 'd2cube-grid', sideCube) as HTMLDivElement;
    cubeWrap.style.gridTemplateColumns = 'repeat(' + CUBE_COLS + ', 32px)';
    state.cubeWrap = cubeWrap;

    const actions = el('div', 'd2cube-actions', panel);
    const btnTrans = el('button', '', actions, 'Transmuter') as HTMLButtonElement;
    btnTrans.title = 'Appelle le moteur (145 recettes réelles v1.12)';
    state.btnTrans = btnTrans;
    const btnBack = el('button', '', actions, 'Tout reprendre');
    btnBack.title = 'Reprendre les objets posés dans le cube';
    const btnClose = el('button', '', actions, 'Fermer (C)');
    const msgWrap = el('div', '', panel) as HTMLDivElement;
    msgWrap.id = 'd2cube-msg';
    state.msgWrap = msgWrap;
    const result = el('div', '', panel);
    result.id = 'd2cube-result';

    btnTrans.addEventListener('click', transmute);
    btnBack.addEventListener('click', () => {
      state.placed = [];
      state.selected = -1;
      setMessage('Objets repris.', false);
      render();
    });
    btnClose.addEventListener('click', close);

    // Bandeau ceinture 4 colonnes (touches 1-4) × 4 — tooltip au survol.
    const belt = el('div', '', root);
    belt.id = 'd2cube-belt';
    const bbtn = el('button', 'cubebtn', belt, 'Cube (C)');
    bbtn.title = 'Ouvrir le Cube Horadrim';
    bbtn.addEventListener('click', () => toggle());
    const bcols = el('div', 'belt-cols', belt);
    for (let c = 0; c < 4; c++) {
      const col = el('div', 'd2cube-beltcol', bcols);
      el('div', 'bkey', col, String(c + 1));
      for (let r = 0; r < 4; r++) {
        (el('div', 'bcell', col) as HTMLElement).dataset.b = String(c * 4 + r);
      }
    }
    state.belt = belt as HTMLDivElement;

    // Tooltip unique (inventaire / ceinture / sol).
    const tip = el('div', '', root);
    tip.id = 'd2cube-tip';
    state.tipEl = tip as HTMLDivElement;

    wireTooltipEvents();
    setInterval(refreshBelt, 500);
    requestAnimationFrame(frame);
  }

  function open(): void {
    build();
    state.open = true;
    document.getElementById('d2cube-panel')?.classList.add('open');
    setMessage('Placez des objets dans le cube (clic-sélection), puis Transmuter.', false);
    render();
  }

  function close(): void {
    state.open = false;
    state.selected = -1;
    state.placed = [];
    hideTip();
    document.getElementById('d2cube-panel')?.classList.remove('open');
    drawOverlay();
  }

  function toggle(): void {
    if (state.open) close();
    else open();
  }

  function setMessage(msg: string, isErr: boolean): void {
    state.message = msg || '';
    state.msgErr = !!isErr;
    if (!state.msgWrap) return;
    state.msgWrap.textContent = state.message;
    state.msgWrap.className = state.msgErr ? 'err' : (state.message ? 'ok' : '');
  }

  // --- rendu du panneau -------------------------------------------------------
  function render(): void {
    if (!state.open || !state.built) return;
    const slots = invCache();
    // Synchronise : un slot « posé » dont l'objet a disparu (consommé) sort.
    state.placed = state.placed.filter((p) => !!slots[p.slot]);

    // Vue inventaire 10×4.
    const inv = state.invWrap;
    if (!inv) return;
    inv.textContent = '';
    for (let i = 0; i < CUBE_INV_COLS * CUBE_INV_ROWS; i++) {
      const slot = i;
      const cell = el('div', 'd2cube-cell', inv);
      const it = slots[slot];
      const inCube = state.placed.some((p) => p.slot === slot);
      if (it && !inCube) {
        const d = el('div', 'it', cell, itemLabel(it));
        const pair = itemColor(it);
        d.style.background = pair[0];
        d.style.borderColor = pair[1];
        const rolled = state.products[slot]?.rolled;
        if (rolled) {
          d.style.boxShadow = '0 0 4px ' + (QUALITY_HEX[rolled.quality ?? ''] ?? '#c7a45a');
        }
        d.title = '';
      }
      if (slot === state.selected) cell.classList.add('sel');
      cell.addEventListener('click', (e) => {
        e.stopPropagation();
        if (!it || inCube) {
          state.selected = -1;
          render();
          return;
        }
        state.selected = (state.selected === slot) ? -1 : slot;
        setMessage(state.selected >= 0
          ? 'Objet sélectionné — cliquez une cellule du cube pour le placer.'
          : 'Sélection annulée.', false);
        hideTip();
        render();
      });
      cell.addEventListener('mouseenter', () => {
        if (it && !inCube) showTipForSlot(slot, cell);
      });
      cell.addEventListener('mouseleave', hideTip);
    }

    // Vue cube 3×4.
    const cube = state.cubeWrap;
    if (!cube) return;
    cube.textContent = '';
    for (let c = 0; c < CUBE_COLS * CUBE_ROWS; c++) {
      const cellIdx = c;
      const cell = el('div', 'd2cube-cell', cube);
      const entry = state.placed.find((p) => p.cell === cellIdx) ?? null;
      if (entry) {
        const it = slots[entry.slot];
        if (it) {
          const d = el('div', 'it', cell, itemLabel(it));
          const pair = itemColor(it);
          d.style.background = pair[0];
          d.style.borderColor = pair[1];
        }
        cell.addEventListener('click', (e) => {
          e.stopPropagation();
          state.placed = state.placed.filter((p) => p.cell !== cellIdx);
          setMessage('Objet repris du cube.', false);
          hideTip();
          render();
        });
        cell.addEventListener('mouseenter', () => {
          if (entry) showTipForSlot(entry.slot, cell);
        });
        cell.addEventListener('mouseleave', hideTip);
      } else {
        cell.addEventListener('click', (e) => {
          e.stopPropagation();
          if (state.selected < 0) {
            setMessage('Sélectionnez d\'abord un objet dans l\'inventaire.', true);
            return;
          }
          if (state.placed.some((p) => p.slot === state.selected)) return;
          state.placed.push({ slot: state.selected, cell: cellIdx });
          state.selected = -1;
          setMessage('Dans le cube — ' + state.placed.length + ' objet(s).', false);
          hideTip();
          render();
        });
      }
    }
    if (state.btnTrans) {
      state.btnTrans.disabled = state.placed.length === 0;
      state.btnTrans.textContent = 'Transmuter (' + state.placed.length + ')';
    }
    drawOverlay();
    lastSig = signature(); // la boucle ne re-rendra pas pour rien
  }

  // Surlignage de l'inventaire CANVAS (panneau I de game.js) : items « au
  // cube » marqués d'un liseré bleu, sélection en or — purement indicatif.
  function drawOverlay(): void {
    const ctx = state.octx;
    if (!ctx) return;
    ctx.clearRect(0, 0, CUBE_W, CUBE_H);
    if (!state.open || !state.placed.length) return;
    const pad = 14, header = 30, cell = 42;
    const pw = pad * 2 + CUBE_INV_COLS * cell;
    const ph = pad * 2 + header + CUBE_INV_ROWS * cell;
    const x0 = (CUBE_W - pw) / 2, y0 = (CUBE_H - ph) / 2;
    const slots = invCache();
    ctx.save();
    ctx.lineWidth = 2;
    for (const p of state.placed) {
      if (!slots[p.slot]) continue;
      const cx = x0 + pad + (p.slot % CUBE_INV_COLS) * cell;
      const cy = y0 + pad + header + Math.floor(p.slot / CUBE_INV_COLS) * cell;
      ctx.strokeStyle = '#6969ff';
      ctx.strokeRect(cx + 0.5, cy + 0.5, cell - 1, cell - 1);
      ctx.strokeStyle = '#00000088';
      ctx.beginPath();
      ctx.moveTo(cx + 6, cy + cell / 2);
      ctx.lineTo(cx + cell - 6, cy + cell / 2);
      ctx.stroke();
    }
    if (state.selected >= 0 && slots[state.selected]) {
      const cx = x0 + pad + (state.selected % CUBE_INV_COLS) * cell;
      const cy = y0 + pad + header + Math.floor(state.selected / CUBE_INV_COLS) * cell;
      ctx.strokeStyle = '#ffe9ad';
      ctx.strokeRect(cx + 0.5, cy + 0.5, cell - 1, cell - 1);
    }
    ctx.restore();
  }

  // --- transmutation (LE MOTEUR FAIT FOI) ------------------------------------
  function transmute(): void {
    const g = game();
    if (!g || typeof g.cubeTransmute !== 'function') {
      setMessage('Moteur indisponible (pkg wasm sans endpoint cube).', true);
      return;
    }
    const slots = state.placed.map((p) => p.slot);
    if (!slots.length) return;
    let res: { ok?: boolean; line?: string; recipe?: string; consumed?: number[]; item?: Item & { slot?: number; tooltip?: CubeTooltip; rolled?: unknown }; portal?: boolean; message?: string };
    try {
      res = parseOr(g.cubeTransmute!(transmutePayload(slots, Date.now())), {});
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err), true);
      state.resultTip = null;
      render();
      return;
    }
    // Succès : les consommés ont disparu, le produit est en inventaire.
    if (res.item && res.item.slot !== undefined) {
      state.products[res.item.slot] = res.item as typeof state.products[number];
      state.resultTip = res.item.tooltip ?? null;
    } else {
      state.resultTip = null;
    }
    state.placed = [];
    state.selected = -1;
    setMessage(res.message || 'Transmutation réussie.', false);
    renderResult(res);
    try {
      if (win.__d2 && g) win.__d2.invCache = parseOr<Array<Item | null>>(g.inventory(), []);
    } catch { /* le cache se rafraîchit à la frame suivante */ }
    render();
  }

  function renderResult(res: { portal?: boolean; message?: string }): void {
    const box = document.getElementById('d2cube-result');
    if (!box) return;
    box.textContent = '';
    if (res.portal) {
      box.style.color = '#d9b96a';
      box.textContent = '⚗ ' + (res.message ?? '');
      return;
    }
    const tip = state.resultTip;
    if (!tip) return;
    box.style.color = '';
    const name = document.createElement('div');
    name.textContent = tip.name ?? '';
    name.style.color = tip.colorHex || '#e8e8e8';
    name.style.fontWeight = '700';
    box.appendChild(name);
    if (tip.baseName) {
      const b = document.createElement('div');
      b.textContent = tip.baseName;
      b.style.color = '#e8e8e8';
      box.appendChild(b);
    }
    for (const line of tip.lines ?? []) {
      const l = document.createElement('div');
      l.textContent = line;
      l.style.color = '#7f7fff';
      box.appendChild(l);
    }
  }

  // --- tooltips (inventaire / ceinture / sol) --------------------------------
  function requestTooltip(item: { code: string; name: string }, anchor: HTMLElement): void {
    const g = game();
    if (!g || typeof g.cubeItemTooltip !== 'function' || !item) return;
    let json: string;
    try {
      json = g.cubeItemTooltip!(JSON.stringify(item));
    } catch {
      return;
    }
    const tip = parseOr<CubeTooltip | null>(json, null);
    if (tip) showTip(tipHtml(tip), anchor);
  }

  function showTipForSlot(slot: number, anchor: HTMLElement): void {
    const it = invCache()[slot];
    if (!it) return;
    const known = state.products[slot];
    const view: { code: string; name: string; quality?: string; affixIds?: number[]; sockets?: number; min_dmg?: number; max_dmg?: number; ac?: number; req_str?: number; req_dex?: number; req_level?: number; runeword?: string } = {
      code: it.code, name: it.name,
    };
    const rolled = known?.rolled;
    if (rolled) {
      view.quality = rolled.quality;
      view.affixIds = rolled.affixIds;
      view.sockets = rolled.sockets;
      view.min_dmg = rolled.minDmg;
      view.max_dmg = rolled.maxDmg;
      view.ac = rolled.ac;
      view.req_str = rolled.reqStr;
      view.req_dex = rolled.reqDex;
      view.req_level = rolled.reqLevel;
      if (rolled.runeword) view.runeword = rolled.runeword;
    }
    requestTooltip(view, anchor);
  }

  function showTip(html: string, anchor: HTMLElement): void {
    const tip = state.tipEl;
    if (!tip || !state.root) return;
    tip.innerHTML = html;
    tip.style.display = 'block';
    const rect = anchor.getBoundingClientRect();
    const host = state.root.getBoundingClientRect();
    let x = rect.right - host.left + 8;
    const y = rect.top - host.top;
    if (x + 290 > CUBE_W) x = rect.left - host.left - 292;
    if (x < 0) x = 4;
    tip.style.left = Math.max(2, Math.round(x)) + 'px';
    tip.style.top = Math.max(2, Math.round(Math.min(y, CUBE_H - 160))) + 'px';
  }

  function hideTip(): void {
    if (state.tipEl) state.tipEl.style.display = 'none';
  }

  // Survol du SOL : les étiquettes Alt (#itemlabels .ilabel du hud) —
  // retrouvées par leurs coordonnées (left/top posés = sx/sy arrondis).
  function wireTooltipEvents(): void {
    const labels = document.getElementById('itemlabels');
    if (labels) {
      labels.addEventListener('mouseover', (e) => {
        const target = e.target as HTMLElement;
        const lab = target.closest?.('.ilabel') as HTMLElement | null;
        if (!lab) return;
        const g = game();
        if (!g) return;
        const lx = parseInt(lab.style.left, 10);
        const ly = parseInt(lab.style.top, 10) + 14;
        let items: Array<{ code: string; name: string; sx: number; sy: number }> = [];
        try {
          items = parseOr(g.groundItems() || '[]', []);
        } catch {
          return;
        }
        for (const it of items) {
          if (Math.round(it.sx) === lx && Math.round(it.sy) === ly) {
            requestTooltip({ code: it.code, name: it.name }, lab);
            return;
          }
        }
      });
      labels.addEventListener('mouseout', hideTip);
    }
    // Ceinture : délégation sur les cellules (rafraîchies par refreshBelt).
    if (state.belt) {
      state.belt.addEventListener('mouseover', (e) => {
        const cell = (e.target as HTMLElement).closest?.('.bcell') as HTMLElement | null;
        if (!cell || !cell.dataset.item) return;
        let it: { code: string; name: string };
        try {
          it = JSON.parse(cell.dataset.item);
        } catch {
          return;
        }
        requestTooltip(it, cell);
      });
      state.belt.addEventListener('mouseout', hideTip);
    }
  }

  // --- ceinture (beltInfo, touches 1-4) ---------------------------------------
  function refreshBelt(): void {
    const belt = state.belt;
    if (!belt || document.hidden) return;
    const g = game();
    if (!g || typeof g.beltInfo !== 'function') return;
    const info = parseOr<import('./types').BeltInfo>(g.beltInfo() || '{}', {});
    const grid = info.belt ?? [];
    const cells = belt.querySelectorAll<HTMLElement>('.bcell');
    for (const cell of cells) {
      const b = parseInt(cell.dataset.b ?? '0', 10);
      const col = Math.floor(b / 4), row = b % 4;
      const it = grid[col]?.[row] ?? null;
      cell.dataset.item = it ? JSON.stringify({ code: it.code, name: it.name }) : '';
      cell.textContent = '';
      if (it) {
        const d = document.createElement('div');
        d.className = 'it';
        d.textContent = it.code && it.code[0] ? it.code[0].toUpperCase() : '?';
        const pair = KIND_COLORS[it.kind] ?? KIND_COLORS.Misc!;
        d.style.background = pair[0];
        cell.appendChild(d);
        cell.title = '';
      } else {
        cell.removeAttribute('title');
      }
    }
  }

  // --- boucle (léger : re-rendu uniquement quand invCache change) ------------
  function signature(): string {
    const slots = invCache();
    let sig = state.placed.map((p) => p.slot + ':' + p.cell).join(',') +
      '|' + state.selected;
    for (let i = 0; i < slots.length; i++) {
      sig += '|' + (slots[i] ? slots[i]!.code : '-');
    }
    return sig;
  }

  let lastFrame = 0;
  function frame(now: number): void {
    if (now - lastFrame > 120) { // ~8 fps suffit pour un panneau
      lastFrame = now;
      if (state.open && state.built) {
        const sig = signature();
        if (sig !== lastSig) {
          lastSig = sig;
          render(); // resynchronise invCache → vues (place/consommés)
        }
      }
    }
    requestAnimationFrame(frame);
  }

  // --- clavier : « C » ouvre/ferme (hors champs de saisie) ---------------------
  window.addEventListener('keydown', (e) => {
    const t = e.target as HTMLElement | null;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
    if (e.key === 'c' || e.key === 'C') {
      if (!game()) return; // pas de panneau sans moteur (fallback JS)
      toggle();
    } else if (e.key === 'Escape' && state.open) {
      close();
    }
  });

  win.D2CubeUI = { open, close, toggle };
}

export function installCube(win: D2Window): void {
  if (win.D2CubeUI) {
    registerModule('cube-ui', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : cube-ui.js Legacy actif — on adopte
  installTsCube(win);
}

if (typeof window !== 'undefined') {
  await warmLegacy('cube-ui.js');
  installCube(window as unknown as D2Window);
}
