// Diablo 2 WASM — demo/vendor (domaine G : marchands & gamble) — migration
// TS 25a de demo/vendor.js. Miroir des règles de d2rules/src/vendor.rs :
//   achat = cost (colonne réelle), vente = floor(cost/4),
//   stock par colonnes vendeurs (XxxMin/Max/MagicMin/Max/MagicLvl),
//   permanent = PermStoreItem 1, 80 %/exemplaire non-permanent,
//   réserve Gheed 350-750, gamble 12 slots à cost × (100+2·clvl)/100,
//   pool gamble = spawnable && qlvl ≤ niveau vendeur.
// ÉCARTS documentés (le moteur Rust fait foi) : items « magiques » marqués
// sans rouler d'affixes ici ; RNG = miroir mulberry32 (pas le SplitMix64).
//
// CONTRAT window.D2Vendor INCHANGÉ (init/generate/gambleList/_state).
// Toutes les RÈGLES sont pures et testables bun (catalogue passé en argument)
// ; le panneau DOM + fetch des tables vit dans la voie TS d'installation.
//
// TRANSITION : vendor.js est un <script> classique de game.html qui
// s'auto-branche à DOMContentLoaded (bouton « Marchands »). S'il est actif on
// l'ADOPTE (sinon double bouton/panneau) ; la voie TS prend le relais après.
import { registerModule } from './types';
import type { D2VendorHooks, D2VendorUnit, D2Window, Item } from './types';

export const VENDOR_IDS = ['Akara', 'Charsi', 'Gheed'] as const;
export const SPAWN_PCT = 80;
export const GAMBLE_SLOTS = 12;
export const GHEED_GOLD: readonly [number, number] = [350, 750];
export const DEFAULT_GOLD = 10000;

export const WEAPON_TYPES = ['axe', 'swor', 'mace', 'knif', 'hamm', 'scep', 'staf', 'pole', 'club', 'h2h', 'h2h2',
  'spea', 'jave', 'ajav', 'tkni', 'tpot', 'taxe', 'orb', 'aspe', 'bow', 'xbow', 'abow'];
export const ARMOR_TYPES = ['tors', 'body', 'helm', 'circ', 'pelt', 'phlm', 'boot', 'glov', 'shie', 'ashd', 'belt'];

// --- PRNG miroir (mulberry32, déterministe au seed) -----------------------
export function mulberry32(seed: number): () => number {
  let a = seed | 0;
  return () => {
    a = (a + 0x6D2B79F5) | 0;
    let t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

export function ri(rand: () => number, lo: number, hi: number): number {
  return hi <= lo ? lo : lo + Math.floor(rand() * (hi - lo + 1));
}

export function chance(rand: () => number, pct: number): boolean {
  return Math.floor(rand() * 100) < pct;
}

// --- parsing TSV (tables réelles du projet) --------------------------------
export interface VendorStockRange {
  min: number; max: number; mmin: number; mmax: number; mlvl: number;
}

export interface BaseItem {
  code: string;
  name: string;
  level: number;
  spawnable: boolean;
  cost: number;
  gambleCost: number;
  kind: 'Weapon' | 'Armor' | 'Potion' | 'Misc';
  permanent: boolean;
  magic: boolean;
  stock: Record<string, VendorStockRange>;
}

/** TSV -> lignes de cellules (pur). */
export function tsv(text: string): string[][] {
  return text.split(/\r?\n/).filter((l) => l.trim())
    .map((l) => l.split('\t').map((c) => c.trim().replace(/^"|"$/g, '')));
}

export function num(s: string): number {
  const v = parseInt(s, 10);
  return Number.isNaN(v) ? 0 : v;
}

export function count(s: string): number {
  return (s === '' || s === '0' || s === '255') ? 0 : num(s);
}

export function kindOfMisc(wtype: string): 'Potion' | 'Misc' {
  return (wtype === 'hpot' || wtype === 'rpot') ? 'Potion' : 'Misc';
}

/** Charge les bases d'un TSV (weapons/armor/misc) dans `out` (pur). */
export function loadBases(
  text: string,
  kindOf: (wtype: string) => BaseItem['kind'],
  out: BaseItem[],
): void {
  const rows = tsv(text);
  if (!rows.length) return;
  const hdr = rows[0]!;
  const idx: Record<string, number> = {};
  hdr.forEach((h, i) => {
    idx[h.toLowerCase()] = i;
  });
  const need = ['code', 'level', 'spawnable', 'cost', 'gamble cost'];
  if (need.some((n) => idx[n] === undefined)) return;
  for (const v of VENDOR_IDS) {
    for (const s of ['min', 'max', 'magicmin', 'magicmax', 'magiclvl']) {
      const c = (v + s).toLowerCase();
      if (idx[c] === undefined) idx[c] = -1;
    }
  }
  const col = (r: string[], name: string): string => {
    const i = idx[name.toLowerCase()];
    return i !== undefined && i >= 0 ? (r[i] ?? '') : '';
  };
  for (const r of rows.slice(1)) {
    const code = col(r, 'code');
    if (!code) continue;
    const wtype = idx.type !== undefined ? (r[idx.type] ?? '') : '';
    const stock: Record<string, VendorStockRange> = {};
    for (const v of VENDOR_IDS) {
      const max = count(col(r, v + 'max'));
      const mmax = count(col(r, v + 'magicmax'));
      if (max > 0 || mmax > 0) {
        stock[v] = {
          min: count(col(r, v + 'min')), max,
          mmin: count(col(r, v + 'magicmin')), mmax,
          mlvl: count(col(r, v + 'magiclvl')),
        };
      }
    }
    const kind = kindOf(wtype);
    const permIdx = idx.permstoreitem;
    out.push({
      code,
      name: col(r, 'name') || code,
      level: num(col(r, 'level')),
      spawnable: col(r, 'spawnable') === '1',
      cost: num(col(r, 'cost')),
      gambleCost: num(col(r, 'gamble cost')),
      kind,
      permanent: permIdx !== undefined && permIdx >= 0 && r[permIdx] === '1',
      stock,
      magic: kind === 'Weapon' || kind === 'Armor' ||
        ['ring', 'amul', 'jewl', 'ques'].includes(wtype),
    });
  }
}

// --- règles miroirs (vendor.rs) — pures -------------------------------------
export function goldRange(id: string): readonly [number, number] {
  return id === 'Gheed' ? GHEED_GOLD : [DEFAULT_GOLD, DEFAULT_GOLD];
}

export interface VendorStock {
  id: string;
  name: string;
  level: number;
  gold: number;
  stock: D2VendorUnit[];
}

/** Génère le stock d'un vendeur (pur — RNG mulberry32 seedé). */
export function generate(
  catalog: readonly BaseItem[], id: string, vendorLevel: number, seed: number,
): VendorStock {
  const rand = mulberry32(seed ^ 0xD26);
  const vendor: VendorStock = { id, name: id, level: vendorLevel, gold: 0, stock: [] };
  const [lo, hi] = goldRange(id);
  vendor.gold = ri(rand, lo, hi);
  for (const b of catalog) {
    const cnt = b.stock[id];
    if (!cnt) continue;
    if (cnt.max > 0) {
      const n = ri(rand, Math.min(cnt.min, cnt.max), cnt.max);
      for (let i = 0; i < n; i++) {
        if (!b.permanent && !chance(rand, SPAWN_PCT)) continue;
        vendor.stock.push({
          code: b.code, name: b.name, kind: b.kind, quality: 'Normal',
          buy: b.cost, sell: Math.floor(b.cost / 4), permanent: b.permanent,
        });
      }
    }
    if (cnt.mmax > 0 && b.magic) {
      const n = ri(rand, Math.min(cnt.mmin, cnt.mmax), cnt.mmax);
      for (let i = 0; i < n; i++) {
        if (!b.permanent && !chance(rand, SPAWN_PCT)) continue;
        vendor.stock.push({
          code: b.code, name: b.name + ' (magique)', kind: b.kind, quality: 'Magic',
          buy: b.cost, sell: Math.floor(b.cost / 4), permanent: b.permanent,
        });
      }
    }
  }
  return vendor;
}

/** Restock : nouveau seed dérivé (pur). */
export function restockSeed(seed: number): number {
  return ((seed ^ ((seed << 7) >>> 0)) >>> 0);
}

export interface GambleOdds {
  rare: number; set: number; unique: number; uber: number; ultra: number;
}

/** Liste gamble (pur) — pool = spawnable && qlvl ≤ niveau vendeur. */
export function gambleList(
  gambleCodes: readonly string[], byCode: Readonly<Record<string, BaseItem>>,
  odds: GambleOdds, vendorLevel: number, clvl: number, seed: number,
): D2VendorUnit[] {
  const rand = mulberry32((seed ^ 0x6A1B) >>> 0);
  const pool = gambleCodes
    .map((c) => byCode[c])
    .filter((b) => b && b.spawnable && b.level <= vendorLevel);
  if (!pool.length) return [];
  const wU = odds.unique + odds.uber + odds.ultra;
  const wS = odds.set;
  const wR = odds.rare;
  const wM = odds.rare;
  const total = wU + wS + wR + wM;
  const out: D2VendorUnit[] = [];
  for (let s = 0; s < GAMBLE_SLOTS; s++) {
    const b = pool[Math.floor(rand() * pool.length)]!;
    const roll = Math.floor(rand() * total);
    let q = roll < wU ? 'Unique' : roll < wU + wS ? 'Set' : roll < wU + wS + wR ? 'Rare' : 'Magic';
    if (q !== 'Magic' && q !== 'Rare' && !b.magic) q = 'Normal';
    const price = Math.floor((b.gambleCost * (100 + 2 * Math.min(clvl, 50))) / 100);
    out.push({
      code: b.code, name: q === 'Normal' ? b.name : b.name + ' (?)', kind: b.kind,
      quality: q, buy: price, sell: Math.floor(b.cost / 4), permanent: false, gamble: true,
    });
  }
  return out;
}

/** Prix de vente = valeur/4 (pur, testable). */
export function sellPrice(cost: number): number {
  return Math.floor(cost / 4);
}

// --- hooks par défaut (lecture window.__d2 ; économie locale) ---------------
function defaultHooks(win: D2Window): D2VendorHooks {
  let localGold: number | null = null;
  const snapshot = (): number => {
    try {
      return win.__d2 && win.__d2.game
        ? (JSON.parse(win.__d2.game.heroStats()) as { gold?: number }).gold ?? 500
        : 500;
    } catch {
      return 500;
    }
  };
  const getGold = (): number => (localGold === null ? snapshot() : localGold);
  return {
    getGold,
    spend(n: number): boolean {
      const g = getGold();
      if (g < n) return false;
      localGold = g - n;
      return true;
    },
    earn(n: number) {
      localGold = getGold() + n;
    },
    inventory(): Item[] {
      try {
        return (win.__d2?.invCache ?? []) as Item[];
      } catch {
        return [];
      }
    },
    heroLevel(): number {
      try {
        return win.__d2 && win.__d2.game
          ? (JSON.parse(win.__d2.game.heroStats()) as { level?: number }).level ?? 1
          : 1;
      } catch {
        return 1;
      }
    },
    simulated: true, // achats/ventes en simulation locale (bindings restants)
  };
}

// --- voie TS : panneau DOM + fetch des tables (port fidèle) ------------------
const KIND_COLORS: Readonly<Record<string, string>> = {
  Potion: '#b03a2e', Weapon: '#8d99a6', Armor: '#8a6d3b', Gold: '#d9b96a', Misc: '#6e7a5e',
};

function installTsVendor(win: D2Window): void {
  registerModule('vendor', 'ts');
  const ROOT = new URL('..', location.href);
  const catalog: BaseItem[] = [];
  const byCode: Record<string, BaseItem> = {};
  let gambleCodes: string[] = [];
  let odds: GambleOdds | null = null;
  let seed = 0x0D2;
  let hooks: D2VendorHooks | null = null;
  let logFn: (m: string) => void = (m) => {
    const s = document.getElementById('status');
    if (s) s.textContent = s.textContent!.replace(/ · \d+ fps.*$/, '') + ' — ' + m;
  };
  let vendor: VendorStock | null = null;
  let vendorGamble: D2VendorUnit[] | null = null;
  let vendorGambleLevel = -1;

  function build(done: () => void): void {
    const files = ['patch_weapons.txt', 'patch_armor.txt', 'patch_misc.txt',
      'patch_gamble.txt', 'patch_difficultylevels.txt'];
    Promise.all(files.map((f) =>
      fetch(new URL('assets/' + f, ROOT)).then((r) => {
        if (!r.ok) throw new Error(f + ' HTTP ' + r.status);
        return r.text();
      })))
      .then((texts) => {
        loadBases(texts[0]!, () => 'Weapon', catalog);
        loadBases(texts[1]!, () => 'Armor', catalog);
        loadBases(texts[2]!, kindOfMisc, catalog);
        for (const b of catalog) byCode[b.code] = b;
        gambleCodes = tsv(texts[3]!).slice(1)
          .map((r) => r[1] ?? '')
          .filter((c) => c && byCode[c]);
        const d = tsv(texts[4]!);
        const hdr = (d[0] ?? []).map((h) => h.toLowerCase());
        const row = d[1] ?? []; // Normal
        const at = (name: string): number => num(row[hdr.indexOf(name)] ?? '0');
        odds = {
          rare: at('gamblerare'), set: at('gambleset'),
          unique: at('gambleunique'), uber: at('gambleuber'), ultra: at('gambleultra'),
        };
        done();
      })
      .catch((e: unknown) => {
        console.info('[vendor] tables indisponibles :', e instanceof Error ? e.message : e);
        logFn('Tables marchandes indisponibles (' + (e instanceof Error ? e.message : e) + ').');
      });
  }

  function el(tag: string, cls?: string, text?: string): HTMLElement {
    const e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function init(dom?: { toolbar?: HTMLElement; root?: HTMLElement; log?: (m: string) => void }, h?: D2VendorHooks): { panel: HTMLElement; render(): void } | undefined {
    hooks = h ?? defaultHooks(win);
    const toolbar = (dom && dom.toolbar) || document.getElementById('toolbar');
    if (dom?.log) logFn = dom.log;
    if (!toolbar) return undefined;
    const openBtn = el('button', undefined, 'Marchands');
    openBtn.id = 'btn-vendors';
    openBtn.title = 'Achat / vente / gamble (domaine G)';
    toolbar.appendChild(openBtn);

    const css = el('style');
    css.textContent =
      '#d2vendor{position:fixed;right:10px;top:64px;width:330px;max-height:70vh;overflow:auto;' +
      'background:#141019f2;border:1px solid #5a4a2a;border-radius:4px;padding:8px;z-index:50;display:none}' +
      '#d2vendor h3{margin:0 0 6px;color:#c7a45a;font:600 12px system-ui;letter-spacing:.08em}' +
      '#d2vendor .tabs{display:flex;gap:4px;margin-bottom:6px}' +
      '#d2vendor .tabs button{padding:3px 8px;font-size:11px}' +
      '#d2vendor .tabs button.on{background:#5a4a2a;color:#ffe9b0}' +
      '#d2vendor .stock{display:grid;grid-template-columns:1fr 1fr;gap:4px}' +
      '#d2vendor .it{display:flex;justify-content:space-between;gap:6px;background:#1d1712;' +
      'border:1px solid #3a3226;padding:4px 6px;font:11px system-ui;cursor:pointer;border-radius:2px}' +
      '#d2vendor .it:hover{border-color:#c7a45a}' +
      '#d2vendor .it.na{opacity:.45;cursor:not-allowed}' +
      '#d2vendor .it .p{color:#c7a45a;font-weight:600;white-space:nowrap}' +
      '#d2vendor .q-Magic{color:#7f8fe0}.q-Rare{color:#e0c14f}.q-Set{color:#4fd07a}.q-Unique{color:#c99e3f}' +
      '#d2vendor .foot{margin-top:6px;font:10px system-ui;color:#8f8676}';
    document.head.appendChild(css);

    const panel = el('div');
    panel.id = 'd2vendor';
    document.body.appendChild(panel);

    let tab: string = 'Akara';
    let invMode = false;

    function tryBuy(stock: D2VendorUnit[], i: number, price: number, gamble: boolean): void {
      const hh = hooks!;
      if (hh.getGold() < price) {
        logFn('Pas assez d’or (' + price + ' requis).');
        return;
      }
      if (!hh.spend(price)) {
        logFn('Achat refusé par l’intégration.');
        return;
      }
      stock.splice(i, 1);
      logFn((gamble ? 'Gamble acheté et révélé' : 'Acheté') + ' pour ' + price + ' or.');
      render();
    }

    function render(): void {
      if (!catalog.length) {
        panel.innerHTML = '<h3>MARCHANDS</h3><div>Chargement des tables…</div>';
        return;
      }
      panel.innerHTML = '';
      const hh = hooks!;
      const clvl = hh.heroLevel ? hh.heroLevel() : 1;
      const title = el('h3', undefined, (invMode ? 'VENDRE' : tab.toUpperCase() + ' — MAGASIN') +
        ' · or : ' + hh.getGold() + ((hh as { simulated?: boolean }).simulated ? ' (simulation)' : ''));
      panel.appendChild(title);
      const tabs = el('div', 'tabs');
      for (const t of [...VENDOR_IDS, 'Gamble']) {
        const b = el('button', t === tab && !invMode ? 'on' : undefined, t === 'Gamble' ? 'Gamble (Gheed)' : t);
        b.addEventListener('click', () => {
          tab = t;
          invMode = false;
          render();
        });
        tabs.appendChild(b);
      }
      const sell = el('button', invMode ? 'on' : undefined, 'Vendre');
      sell.addEventListener('click', () => {
        invMode = !invMode;
        render();
      });
      const rs = el('button', undefined, 'Restock');
      rs.title = 'Rafraîchit le stock (nouveau seed dérivé — vendor.rs)';
      rs.addEventListener('click', () => {
        seed = (seed * 1103515245 + 12345) >>> 0;
        vendor = null;
        render();
      });
      tabs.appendChild(sell);
      tabs.appendChild(rs);
      panel.appendChild(tabs);

      const grid = el('div', 'stock');
      const gold = hh.getGold();
      if (invMode) {
        const items = (hh.inventory && hh.inventory()) || [];
        if (!items.length) panel.appendChild(el('div', 'foot', "Inventaire vide (ou non connecté : hooks.inventory())."));
        items.forEach((it) => {
          const base = byCode[it.code];
          if (!base) return; // hors catalogue v1.12
          const price = sellPrice(base.cost);
          const row = el('div', 'it');
          row.appendChild(el('span', undefined, it.name || it.code));
          row.appendChild(el('span', 'p', '+' + price));
          row.addEventListener('click', () => {
            const got = sellPrice(byCode[it.code]!.cost);
            hh.earn(got);
            logFn('Vendu ' + (it.name || it.code) + ' (+' + got + ' or).');
            render();
          });
          grid.appendChild(row);
        });
      } else if (tab === 'Gamble') {
        if (!vendorGamble || vendorGambleLevel !== clvl) {
          vendorGamble = odds
            ? gambleList(gambleCodes, byCode, odds, clvl, clvl, seed)
            : [];
          vendorGambleLevel = clvl;
        }
        vendorGamble.forEach((e2, i) => {
          const row = el('div', 'it' + (gold < e2.buy ? ' na' : ''));
          row.appendChild(el('span', 'q-' + e2.quality, e2.name));
          row.appendChild(el('span', 'p', String(e2.buy)));
          row.addEventListener('click', () => tryBuy(vendorGamble!, i, e2.buy, true));
          grid.appendChild(row);
        });
        panel.appendChild(grid);
        panel.appendChild(el('div', 'foot',
          'Gamble : items non identifiés — prix = gamble cost × (100+2·clvl)/100. ' +
          'Acheté = révélé (affixes déjà roulés côté Rust).'));
        return;
      } else {
        if (!vendor || vendor.id !== tab) vendor = generate(catalog, tab, clvl, seed);
        vendor.stock.forEach((e2, i) => {
          const row = el('div', 'it' + (gold < e2.buy ? ' na' : ''));
          row.appendChild(el('span', 'q-' + e2.quality, e2.name));
          row.appendChild(el('span', 'p', e2.buy + (e2.permanent ? ' ⚑' : '')));
          row.addEventListener('click', () => tryBuy(vendor!.stock, i, e2.buy, false));
          grid.appendChild(row);
        });
        panel.appendChild(grid);
        panel.appendChild(el('div', 'foot',
          'Réserve ' + tab + ' : ' + vendor.gold + ' or · prix = valeur tables v1.12 · ' +
          'vente = valeur/4 · ⚑ permanent.'));
      }
      void KIND_COLORS;
      panel.appendChild(grid);
    }

    openBtn.addEventListener('click', () => {
      panel.style.display = panel.style.display === 'block' ? 'none' : 'block';
      render();
    });
    build(render);
    return { panel, render };
  }

  // Auto-branchement (démo) : bouton dans #toolbar dès que le DOM existe.
  function auto(): void {
    if (document.getElementById('toolbar') && !document.getElementById('btn-vendors')) {
      init({ toolbar: document.getElementById('toolbar') as HTMLElement });
    }
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', auto);
  else auto();

  win.D2Vendor = {
    init,
    generate: (id, vendorLevel, s) => generate(catalog, id, vendorLevel, s),
    gambleList: (vendorLevel, clvl, s) =>
      (odds ? gambleList(gambleCodes, byCode, odds, vendorLevel, clvl, s) : []),
    _state: {
      get catalog() { return catalog; },
      get odds() { return odds; },
      get gambleCodes() { return gambleCodes; },
    },
  };
}

export function installVendor(win: D2Window): void {
  if (win.D2Vendor) {
    registerModule('vendor', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : vendor.js Legacy actif — on adopte
  installTsVendor(win);
}

if (typeof window !== 'undefined') installVendor(window as unknown as D2Window);
