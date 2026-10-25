// AutoMap 9d (monde vivant) — migration TS 25a de demo/automap.js.
// Mini-carte semi-transparente en coin + fog of war (disque RADIUS autour du
// héros, poll 5 Hz) ; projection iso (cx-cy+cellH)*s / (cx+cy)*s/2 ; modes
// moteur « automap » (traits précis 21b via glyphs) / « fallback » (polygone
// marchable). Données : endpoint game.automap() (automap.rs).
//
// CONTRAT window.D2Automap INCHANGÉ (toggle/show/hide — bouton Carte du
// minipanel hud.ts + touche Tab).
//
// TRANSITION : automap.js est importé STATIQUEMENT par game.js — une boucle
// rAF propre au module tourne dès son évaluation. Pour une exécution unique,
// on warm-importe la même URL puis on ADOPTE l'objet Legacy s'il existe ; la
// voie TS complète (port fidèle ci-dessous) prend le relais sans Legacy.
import {   warmLegacy } from './legacy';
import { registerModule } from './types';
import { parseOr, type AutomapData, type D2Game, type D2Window } from './types';

export const AUTOMAP_RADIUS = 9;   // cellules révélées autour du héros
export const AUTOMAP_POLL_MS = 200; // découverte + niveau (5 Hz)
export const PANEL_W = 240;        // largeur max du panneau

const COLORS = {
  bg: 'rgba(10,8,5,0.78)',
  walk: '#c8b57a',     // sol (or terne, palette AutoMap D2)
  wall: '#5c4c30',     // murs / arbres / clôtures
  special: '#7ec8e3',  // cellules rares (pierres, warp)
  hero: '#ffffff',
  border: '#5a4a2a',
  text: '#cfc7b8',
};

/** Coordonnées iso d'une cellule dans le mini-canvas (pur, testable). */
export function proj(d: AutomapData, s: number, cx: number, cy: number): [number, number] {
  return [(cx - cy + d.cellH) * s, ((cx + cy) * s) / 2 + 1];
}

/** Ligne de mode 13b/21b (pur, testable). */
export function modeLineText(mode: string, coverage: number | undefined, hasGlyphs: boolean): string {
  const pct = Math.round((coverage || 0) * 100);
  if (mode !== 'automap') return `repli : polygone marchable · ${pct} %`;
  return hasGlyphs ? `AutoMap · traits réels · ${pct} %` : `AutoMap résolu · ${pct} %`;
}

/** Fog of war : cellules nouvellement révélées par un disque RADIUS —
 * pur, testable (renvoie les index JSON correspondants). */
export function revealIndices(
  d: AutomapData, visited: Uint8Array, byCell: Map<number, number>, cx: number, cy: number,
): number[] {
  const out: number[] = [];
  const r2 = AUTOMAP_RADIUS * AUTOMAP_RADIUS;
  const x0 = Math.max(0, cx - AUTOMAP_RADIUS), x1 = Math.min(d.cellW - 1, cx + AUTOMAP_RADIUS);
  const y0 = Math.max(0, cy - AUTOMAP_RADIUS), y1 = Math.min(d.cellH - 1, cy + AUTOMAP_RADIUS);
  for (let y = y0; y <= y1; y++) {
    for (let x = x0; x <= x1; x++) {
      const dx = x - cx, dy = y - cy;
      if (dx * dx + dy * dy > r2) continue;
      const k = y * d.cellW + x;
      if (visited[k]) continue;
      visited[k] = 1;
      const i = byCell.get(k);
      if (i !== undefined) out.push(i);
    }
  }
  return out;
}

/** Index des cellules JSON par coordonnée (une fois par niveau). */
export function buildByCell(d: AutomapData): Map<number, number> {
  const map = new Map<number, number>();
  d.cells.forEach((c, i) => map.set(c[1]! * d.cellW + c[0]!, i));
  return map;
}

// --- voie TS (port fidèle) ---------------------------------------------------
interface AutomapState {
  open: boolean;
  game: D2Game | null;
  canvas: HTMLCanvasElement | null;
  panel: HTMLDivElement | null;
  title: HTMLDivElement | null;
  modeLine: HTMLDivElement | null;
  inner: HTMLCanvasElement | null;
  ictx: CanvasRenderingContext2D | null;
  data: AutomapData | null;
  levelKey: string | null;
  visited: Uint8Array | null;
  byCell: Map<number, number> | null;
  paths: Path2D[];
  pending: number[];
  s: number;
  lastPoll: number;
  heroCell: [number, number] | null;
}

function installTsAutomap(win: D2Window): void {
  registerModule('automap', 'ts');
  const st: AutomapState = {
    open: false, game: null, canvas: null, panel: null, title: null, modeLine: null,
    inner: null, ictx: null, data: null, levelKey: null, visited: null, byCell: null,
    paths: [], pending: [], s: 2, lastPoll: 0, heroCell: null,
  };

  function ensureDom(): void {
    if (st.canvas) return;
    const stage = document.getElementById('game')?.parentElement;
    if (!stage) return;
    const panel = document.createElement('div');
    panel.id = 'd2-automap';
    Object.assign(panel.style, {
      position: 'absolute', top: '34px', right: '10px', zIndex: '35',
      background: COLORS.bg, border: `1px solid ${COLORS.border}`,
      padding: '6px 8px 4px', display: 'none', pointerEvents: 'none',
    });
    const title = document.createElement('div');
    Object.assign(title.style, {
      font: '600 10px system-ui', letterSpacing: '.12em', color: COLORS.text,
      textShadow: '0 1px 2px #000', marginBottom: '4px', textTransform: 'uppercase',
    });
    const cv = document.createElement('canvas');
    cv.id = 'd2-automap-cv';
    Object.assign(cv.style, { display: 'block', imageRendering: 'pixelated' });
    const mode = document.createElement('div');
    mode.id = 'd2-automap-mode';
    Object.assign(mode.style, {
      font: '9px system-ui', color: '#8a7f6a', marginTop: '3px',
      textShadow: '0 1px 2px #000',
    });
    panel.appendChild(title);
    panel.appendChild(cv);
    panel.appendChild(mode);
    stage.appendChild(panel);
    st.panel = panel;
    st.title = title;
    st.canvas = cv;
    st.modeLine = mode;
  }

  function cvSize(w: number, h: number): void {
    const cv = st.canvas;
    const dcv = st.inner;
    if (!cv) return;
    if (!dcv || dcv.width !== w || dcv.height !== h) {
      const off = document.createElement('canvas');
      off.width = w;
      off.height = h;
      st.inner = off;
      st.ictx = off.getContext('2d');
    }
    cv.width = w;
    cv.height = h;
  }

  function loadLevelData(game: D2Game): void {
    const d = parseOr<AutomapData | null>(safeAutomap(game), null);
    if (!d || !d.cells || !d.cellW) {
      st.data = null;
      return;
    }
    st.data = d;
    st.levelKey = d.level;
    st.visited = new Uint8Array(d.cellW * d.cellH);
    st.pending = [];
    st.heroCell = null;
    st.byCell = null;
    // Traits précis 21b : un Path2D par glyphe (segments px locaux 160×80).
    st.paths = (d.glyphs || []).map((segs) => {
      const p = new Path2D();
      for (const s of segs) {
        p.moveTo(s[0]!, s[1]!);
        p.lineTo(s[2]!, s[3]!);
      }
      return p;
    });
    // échelle iso : (cellW + cellH) colonnes projetées
    st.s = Math.max(1.4, Math.min(4, PANEL_W / (d.cellW + d.cellH)));
    const s = st.s;
    const w = Math.ceil((d.cellW + d.cellH) * s);
    const h = Math.ceil((d.cellW + d.cellH) * s * 0.5) + 2;
    cvSize(w, h);
    st.ictx?.clearRect(0, 0, w, h); // nouveau niveau : accumulation vierge
    if (st.title) st.title.textContent = d.name || d.level || '';
  }

  function paintPending(): void {
    const d = st.data;
    const ctx = st.ictx;
    if (!d || !ctx) return;
    const s = st.s;
    const rw = Math.max(1, s), rh = Math.max(1, s / 2);
    for (const idx of st.pending) {
      const c = d.cells[idx];
      if (!c) continue;
      const [px, py] = proj(d, s, c[0]!, c[1]!);
      const flags = c[2]!;
      const glyphIdx = c[4];
      const glyph = glyphIdx !== undefined && glyphIdx >= 0 ? st.paths[glyphIdx] : undefined;
      if (glyph) {
        const k = s / 160; // canevas 160×80 → s × s/2
        ctx.save();
        ctx.translate(px, py);
        ctx.scale(k, k);
        ctx.lineWidth = Math.max(0.5, s / 4) / k;
        ctx.lineJoin = 'miter';
        ctx.strokeStyle = (flags & 4) ? COLORS.special : (flags & 1) ? COLORS.walk : COLORS.wall;
        ctx.stroke(glyph);
        if (flags & 1) { // sol : losange rempli
          ctx.globalAlpha = 0.55;
          ctx.fillStyle = (flags & 4) ? COLORS.special : COLORS.walk;
          ctx.fill(glyph);
          ctx.globalAlpha = 1;
        }
        ctx.restore();
        continue;
      }
      ctx.fillStyle = (flags & 1) ? COLORS.walk : COLORS.wall;
      if (flags & 4) ctx.fillStyle = COLORS.special;
      // fallback (mode 2) : polygone marchable — bloquées = fond sombre.
      if (d.mode === 'fallback' && !(flags & 1)) continue;
      ctx.fillRect(px, py, rw, rh);
    }
    st.pending = [];
  }

  function showMode(): void {
    const el = st.modeLine;
    if (!el) return;
    if (!st.data) {
      el.textContent = '';
      return;
    }
    el.textContent = modeLineText(st.data.mode, st.data.coverage, !!st.paths.length);
  }

  function poll(now: number): void {
    const game = win.__d2 && win.__d2.mode === 'wasm' ? win.__d2.game : undefined;
    if (!game) return;
    st.game = game;
    if (now - st.lastPoll < AUTOMAP_POLL_MS) return;
    st.lastPoll = now;
    // changement de zone (switch/warp/waypoint) → recharger le JSON
    let key: string | null = null;
    try {
      key = parseOr<{ key?: string }>(game.levelInfo() || '{}', {}).key ?? null;
    } catch {
      key = null;
    }
    if (!st.data || key !== st.levelKey) {
      st.byCell = null;
      loadLevelData(game);
      if (st.title) {
        st.title.textContent = (st.data && (st.data.name || st.data.level)) || '';
      }
      showMode();
    }
    const d = st.data;
    if (!d || !d.cells) return;
    // position héros (cellules) via les endpoints existants
    try {
      const screen = game.heroScreen();
      const world = game.screenToWorld(screen[0]!, screen[1]!);
      const wx = world[0]!;
      const wy = world[1]!;
      const sub = d.sub || 5;
      const cx = Math.floor(wx / sub), cy = Math.floor(wy / sub);
      if (cx >= 0 && cy >= 0 && cx < d.cellW && cy < d.cellH && st.visited) {
        st.heroCell = [cx, cy];
        if (!st.byCell) st.byCell = buildByCell(d);
        st.pending.push(...revealIndices(d, st.visited, st.byCell, cx, cy));
      }
    } catch { /* pas finalisé / hors carte : ignoré */ }
  }

  function draw(): void {
    const d = st.data;
    const cv = st.canvas;
    if (!d || !cv || !st.open) return;
    const ctx = cv.getContext('2d');
    if (!ctx || !st.inner) return;
    ctx.clearRect(0, 0, cv.width, cv.height);
    ctx.drawImage(st.inner, 0, 0);
    if (st.heroCell) {
      const [px, py] = proj(d, st.s, st.heroCell[0], st.heroCell[1]);
      const s = st.s;
      ctx.fillStyle = COLORS.hero;
      ctx.fillRect(px - s * 0.5, py - s * 0.25, Math.max(2, s), Math.max(2, s * 0.5));
    }
  }

  // --- boucle propre au module (aucun point d'appel dans game.js) ----------
  function loop(now: number): void {
    try {
      ensureDom();
      poll(now);
      paintPending();
      draw();
    } catch { /* l'overlay ne doit JAMAIS casser le jeu */ }
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);

  // --- toggle : bouton Carte (hud) + Tab ------------------------------------
  const automap = {
    open: false,
    toggle() {
      ensureDom();
      automap.open = !automap.open;
      st.open = automap.open;
      if (st.panel) st.panel.style.display = automap.open ? 'block' : 'none';
      if (automap.open && st.data === null && st.game) {
        st.levelKey = null; // force le chargement au prochain poll
      }
    },
    show() {
      if (!automap.open) automap.toggle();
    },
    hide() {
      if (automap.open) automap.toggle();
    },
  };

  window.addEventListener('keydown', (e) => {
    if (e.key !== 'Tab') return;
    const t = document.activeElement?.tagName;
    if (t === 'INPUT' || t === 'TEXTAREA') return;
    if (win.__d2 && win.__d2.mode === 'wasm') {
      e.preventDefault(); // pas de saut de focus : Tab = carte (D2)
      automap.toggle();
    }
  });

  win.D2Automap = automap;
}

function safeAutomap(game: D2Game): string {
  try {
    return game.automap() || 'null';
  } catch {
    return 'null';
  }
}

export function installAutomap(win: D2Window): void {
  if (win.D2Automap) {
    registerModule('automap', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : automap.js Legacy actif — on adopte
  installTsAutomap(win);
}

if (typeof window !== 'undefined') {
  await warmLegacy('automap.js');
  installAutomap(window as unknown as D2Window);
}
