// AutoMap 9d (monde vivant) — mini-carte semi-transparente en coin.
//
// Données : endpoint moteur `game.automap()` (crates/d2game/src/automap.rs) —
// JSON {level,name,mode,lvlType,cellW,cellH,sub,cells:[[x,y,flags,cel],…]}.
// Zéro calcul lourd côté JS : la grille/résolution AutoMap viennent du
// moteur ; ce fichier ne fait que dessiner + fog of war simple.
//
// Fog of war (simple, mission 9d) : une cellule est visible si VISITÉE —
// le héros révèle un disque de RADIUS cellules autour de lui (poll 5 Hz,
// position via heroScreen()+screenToWorld(), endpoints existants).
//
// Projection iso (cellule → pixels du mini-canvas) :
//   px = (cx - cy + cellH) * s      py = (cx + cy) * s / 2
// (même famille que sub_to_screen du composeur, à l'échelle cellule).
//
// Câblage (possession 9d) :
//   * web/game.js : UNE ligne `import './automap.js';`
//   * bouton « Carte » du minipanel HUD (hud.js) → window.D2Automap.toggle()
//   * touche Tab = toggle (raccourci D2)
// Modes du moteur (13b : décidés par la COUVERTURE mesurée `coverage`,
// seuil 0,60 côté Rust) : "automap" (cel réels — dessin par catégorie, cel
// passé au title, badge « AutoMap résolu · NN % ») / "fallback" (polygone
// marchable : seules les cellules walk — badge « repli » ; occurs quand le
// groupe AutoMap du LvlType manque — « 5 Town » Harrogath — ou couvre mal
// les tuiles DT1 composées : pandemonium 0 %, bloodyfoothills 40 %,
// hallsvaught 18 % ; les niveaux A2-A5 « automap » portent en plus les
// TRAITS PRÉCIS 21b).
//
// Traits précis (21b) : en mode "automap", chaque cellule dont le Cel est
// résolu porte un index de glyphe — le contour EXACT de l'empreinte de sa
// tuile DT1 composée (blocs réels, canevas local 160×80 px, 10×10
// sous-tuiles de 16×8 ; sols = losange iso, murs = ligne d'assise au
// sommet). Le moteur expose `glyphs` (segments partagés [x1,y1,x2,y2] en
// px locaux) et cells devient [x,y,flags,cel,glyph]. Le rendu trace ces
// segments dans le repère iso de la cellule (échelle uniforme s/160) :
// mosaïque de losanges exacts, lignes de murs — là où le fallback dessine
// une grille de rectangles. Cellules sans glyphe (cel -1 ou empreinte
// vide, tuile sans blocs peints) : rectangle générique, comme avant.
(function () {
  'use strict';

  const RADIUS = 9;          // cellules révélées autour du héros
  const POLL_MS = 200;       // découverte + niveau (5 Hz)
  const PANEL_W = 240;       // largeur max du panneau
  const COLORS = {
    bg: 'rgba(10,8,5,0.78)',
    walk: '#c8b57a',         // sol (or terne, palette AutoMap D2)
    wall: '#5c4c30',         // murs / arbres / clôtures
    special: '#7ec8e3',      // cellules rares (pierres, warp)
    hero: '#ffffff',
    border: '#5a4a2a',
    text: '#cfc7b8',
  };

  const D2Automap = {
    open: false,
    game: null,              // résolu au premier tick où __d2.mode === 'wasm'
    canvas: null,
    ictx: null,              // canvas d'accumulation (cellules visitées)
    data: null,              // dernier automap() du moteur
    levelKey: null,
    visited: null,           // Uint8Array cellW*cellH — fog of war
    pending: [],             // cellules visitées pas encore peintes
    lastPoll: 0,
    heroCell: null,
  };

  function ensureDom() {
    if (D2Automap.canvas) return;
    const stage = document.getElementById('game')?.parentElement;
    if (!stage) return;
    const panel = document.createElement('div');
    panel.id = 'd2-automap';
    Object.assign(panel.style, {
      position: 'absolute', top: '34px', right: '10px', zIndex: 35,
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
    D2Automap.panel = panel;
    D2Automap.title = title;
    D2Automap.canvas = cv;
    D2Automap.modeLine = mode;
  }

  // (Re)charge les données du niveau courant — une seule requête moteur par
  // changement de zone ; le JSON est mis en cache.
  function loadLevelData(game) {
    let d = null;
    try { d = JSON.parse(game.automap() || 'null'); } catch { d = null; }
    if (!d || !d.cells || !d.cellW) { D2Automap.data = null; return; }
    D2Automap.data = d;
    D2Automap.levelKey = d.level;
    D2Automap.visited = new Uint8Array(d.cellW * d.cellH);
    D2Automap.pending = [];
    D2Automap.heroCell = null;
    // Traits précis 21b : un Path2D par glyphe (segments en px locaux du
    // canevas 160×80 — tracés sous transform à la peinture).
    D2Automap.paths = (d.glyphs || []).map((segs) => {
      const p = new Path2D();
      for (const s of segs) {
        p.moveTo(s[0], s[1]);
        p.lineTo(s[2], s[3]);
      }
      return p;
    });
    // échelle iso : (cellW + cellH) colonnes projetées
    const s = Math.max(1.4, Math.min(4, PANEL_W / (d.cellW + d.cellH)));
    D2Automap.s = s;
    const w = Math.ceil((d.cellW + d.cellH) * s);
    const h = Math.ceil((d.cellW + d.cellH) * s * 0.5) + 2;
    cvSize(w, h);
    D2Automap.ictx.clearRect(0, 0, w, h); // nouveau niveau : accumulation vierge
    if (D2Automap.title) D2Automap.title.textContent = d.name || d.level || '';
  }

  function cvSize(w, h) {
    const cv = D2Automap.canvas;
    const dcv = D2Automap.inner;
    if (!dcv || dcv.width !== w || dcv.height !== h) {
      const off = document.createElement('canvas');
      off.width = w; off.height = h;
      D2Automap.inner = off;
      D2Automap.ictx = off.getContext('2d');
    }
    cv.width = w; cv.height = h;
  }

  // Coordonnées iso d'une cellule dans le mini-canvas.
  function proj(d, s, cx, cy) {
    return [((cx - cy + d.cellH) * s), (((cx + cy) * s) / 2 + 1)];
  }

  // Peint les cellules nouvellement visitées sur le canvas d'accumulation.
  function paintPending() {
    const d = D2Automap.data, ctx = D2Automap.ictx;
    if (!d || !ctx) return;
    const s = D2Automap.s;
    const rw = Math.max(1, s), rh = Math.max(1, s / 2);
    for (const idx of D2Automap.pending) {
      const c = d.cells[idx];
      if (!c) continue;
      const [px, py] = proj(d, s, c[0], c[1]);
      const flags = c[2];
      const glyph = c[4] >= 0 ? D2Automap.paths[c[4]] : null;
      // Trait précis 21b (mode automap, glyphe présent) : le contour exact
      // de l'empreinte DT1 de la cellule — losange réel des sols (rempli),
      // ligne d'assise des murs (trait seul), teinte de la catégorie.
      if (glyph) {
        const k = s / 160;                       // canevas 160×80 → s × s/2
        ctx.save();
        ctx.translate(px, py);
        ctx.scale(k, k);
        ctx.lineWidth = Math.max(0.5, s / 4) / k; // épaisseur écran stable
        ctx.lineJoin = 'miter';
        ctx.strokeStyle = (flags & 4) ? COLORS.special : (flags & 1) ? COLORS.walk : COLORS.wall;
        ctx.stroke(glyph);
        if (flags & 1) {                         // sol : losange rempli
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
      // fallback (mode 2) : polygone marchable — les cellules bloquées
      // restent fond sombre (documenté automap.rs).
      if (d.mode === 'fallback' && !(flags & 1)) continue;
      ctx.fillRect(px, py, rw, rh);
    }
    D2Automap.pending = [];
  }

  // Fog of war : révèle un disque RADIUS autour de la cellule du héros.
  function revealAround(d, cx, cy) {
    const v = D2Automap.visited;
    if (!v) return;
    const r2 = RADIUS * RADIUS;
    const x0 = Math.max(0, cx - RADIUS), x1 = Math.min(d.cellW - 1, cx + RADIUS);
    const y0 = Math.max(0, cy - RADIUS), y1 = Math.min(d.cellH - 1, cy + RADIUS);
    // index des cellules visibles par coordonnée (le JSON ne liste que les
    // cellules visibles — construit une fois par niveau).
    if (!D2Automap.byCell) {
      const map = new Map();
      d.cells.forEach((c, i) => map.set(c[1] * d.cellW + c[0], i));
      D2Automap.byCell = map;
    }
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const dx = x - cx, dy = y - cy;
        if (dx * dx + dy * dy > r2) continue;
        const k = y * d.cellW + x;
        if (v[k]) continue;
        v[k] = 1;
        const i = D2Automap.byCell.get(k);
        if (i !== undefined) D2Automap.pending.push(i);
      }
    }
  }

  // Ligne de mode (13b + 21b) : « AutoMap · traits réels · NN % » (Cels
  // résolus ET glyphes d'empreintes DT1), « AutoMap résolu · NN % » (cels
  // sans glyphe — vieilles builds moteur), ou « polygone marchable ·
  // raison » (repli documenté automap.rs : groupe AutoMap absent ou
  // couverture < 60 % — ex. Harrogath « 5 Town » inexistant v1.12,
  // Pandemonium 0 %, Bloody Foothills 40 %, Halls of Vaught 18 %).
  function showMode(d) {
    const el = D2Automap.modeLine;
    if (!el) return;
    if (!d) { el.textContent = ''; return; }
    const pct = Math.round((d.coverage || 0) * 100);
    el.textContent = d.mode !== 'automap'
      ? `repli : polygone marchable · ${pct} %`
      : (D2Automap.paths && D2Automap.paths.length ? `AutoMap · traits réels · ${pct} %`
                                                    : `AutoMap résolu · ${pct} %`);
  }

  function poll(now) {
    const game = window.__d2 && window.__d2.mode === 'wasm' && window.__d2.game;
    if (!game) return;
    D2Automap.game = game;
    if (now - D2Automap.lastPoll < POLL_MS) return;
    D2Automap.lastPoll = now;
    // changement de zone (switch/warp/waypoint) → recharger le JSON
    let key = null;
    try { key = (JSON.parse(game.levelInfo() || '{}').key) || null; } catch { }
    if (!D2Automap.data || key !== D2Automap.levelKey) {
      D2Automap.byCell = null;
      loadLevelData(game);
      if (D2Automap.title) D2Automap.title.textContent =
        (D2Automap.data && (D2Automap.data.name || D2Automap.data.level)) || '';
      showMode(D2Automap.data);
    }
    const d = D2Automap.data;
    if (!d || !d.cells) return;
    // position héros (cellules) via les endpoints existants
    try {
      const [sx, sy] = game.heroScreen();
      const [wx, wy] = game.screenToWorld(sx, sy);
      const sub = d.sub || 5;
      const cx = Math.floor(wx / sub), cy = Math.floor(wy / sub);
      if (cx >= 0 && cy >= 0 && cx < d.cellW && cy < d.cellH) {
        D2Automap.heroCell = [cx, cy];
        revealAround(d, cx, cy);
      }
    } catch { /* pas finalisé / hors carte : ignoré */ }
  }

  // Rendu : accumulation + point héros. Léger (1 drawImage + 2 fillRect).
  function draw() {
    const d = D2Automap.data, cv = D2Automap.canvas;
    if (!d || !cv || !D2Automap.open) return;
    const ctx = cv.getContext('2d');
    ctx.clearRect(0, 0, cv.width, cv.height);
    ctx.drawImage(D2Automap.inner, 0, 0);
    if (D2Automap.heroCell) {
      const [px, py] = proj(d, D2Automap.s, D2Automap.heroCell[0], D2Automap.heroCell[1]);
      const s = D2Automap.s;
      ctx.fillStyle = COLORS.hero;
      ctx.fillRect(px - s * 0.5, py - s * 0.25, Math.max(2, s), Math.max(2, s * 0.5));
    }
  }

  // --- boucle propre au module (aucun point d'appel dans game.js) ----------
  function loop(now) {
    try {
      ensureDom();
      poll(now);
      paintPending();
      draw();
    } catch { /* l'overlay ne doit JAMAIS casser le jeu */ }
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);

  // --- toggle : bouton Carte (hud.js) + Tab --------------------------------
  D2Automap.toggle = function () {
    ensureDom();
    D2Automap.open = !D2Automap.open;
    if (D2Automap.panel) D2Automap.panel.style.display = D2Automap.open ? 'block' : 'none';
    if (D2Automap.open && D2Automap.data === null && D2Automap.game) {
      D2Automap.levelKey = null; // force le chargement au prochain poll
    }
  };
  D2Automap.show = function () { if (!D2Automap.open) D2Automap.toggle(); };
  D2Automap.hide = function () { if (D2Automap.open) D2Automap.toggle(); };

  window.addEventListener('keydown', (e) => {
    if (e.key !== 'Tab') return;
    const t = document.activeElement && document.activeElement.tagName;
    if (t === 'INPUT' || t === 'TEXTAREA') return;
    if (window.__d2 && window.__d2.mode === 'wasm') {
      e.preventDefault(); // pas de saut de focus : Tab = carte (D2)
      D2Automap.toggle();
    }
  });

  window.D2Automap = D2Automap;
})();
