// ui-d2.js — HUD de jeu fidèle à Diablo 2 v1.12 (d2w-fix-ui).
//
// Toute l'art vient des MPQ réels (data/global/ui/PANEL + SPELLS + items),
// extraits par tools/extract_ui_fix.py dans demo/assets/ui/ (palette `sky`
// pour l'UI, `units` pour les objets). Références de composition :
// OpenDiablo2 d2game/d2player/{hud,globeWidget,game_controls,mini_panel}.go
// et inventory.txt v1.12 (grille 10×4 à (419,315), cellules 29×29).
//
// API :
//   D2UI.init(manifest)         — charge les PNG + crée les zones cliquables
//   D2UI.ready                  — true quand les sprites sont chargés
//   D2UI.drawHud(ctx, stats)    — barre du bas (appelée chaque frame, après putImageData)
//   D2UI.drawInventory(ctx, items, stats) — panneau inventaire (I)
//   D2UI.invHit(px, py)         — true si (px,py) est sur le panneau inventaire
//   D2UI.toggleMiniPanel()      — replie/déploie le minipanel (flèches D2)
(function () {
  'use strict';

  const D2UI = {
    ready: false,
    img: {},          // nom → Image
    items: {},        // code objet → Image (item_<code>.png)
    missing: new Set(),
    miniOpen: false,  // minipanel déployé (D2 : replié par défaut)
    points: -1,       // points de skill non dépensés (-1 = inconnu)
    belt: null,       // belt[col][row] du moteur (beltInfo)
    treeClass: null,  // clé de feuille d'icônes ("ba"…) pour les icônes L/R
    icons: {},        // code skill → IconCel (skillTree)
    lastPoll: 0,
    canvas: null,
  };
  // token classe du moteur (skills.txt charclass : bar/sor/pal/ama/ass/dru/nec)
  // → clé de feuille extraite (code héros du manifest : ba/so/pa/am/as/dz/ne).
  const CLS = { bar: 'ba', sor: 'so', pal: 'pa', ama: 'am', ass: 'as', dru: 'dz', nec: 'ne' };

  // ---------------------------------------------------------------- géométrie
  // Barre du bas (frames de 800ctrlpnl7.dc6, posées bord à bord, deux trous
  // de 48 px pour les icônes de compétence L/R — OpenDiablo2 hud.go).
  const PANEL = [
    { f: 0, x: 0 },                       // logement orbe de vie (117×104)
    { f: 1, x: 165 },                     // bouton + stats + rigoles (128×55)
    { f: 2, x: 293 },                     // rigoles endurance/XP (128×55)
    { f: 3, x: 421 },                     // ceinture 4 alvéoles (128×55)
    { f: 4, x: 549 },                     // bouton + skills (86×55)
    { f: 5, x: 683 },                     // logement orbe de mana (117×104)
  ];
  const H = 600; // canvas fixe 800×600 comme D2
  // Orbes : sprites 80×80 (hlthmana.dc6 f1 = vie rouge, f0 = mana bleu),
  // remplissage = tranche BASSE du sprite (globeWidget.go RenderSection).
  const HP = { img: 'orbs_f1', x: 30, y: 507 };
  const MP = { img: 'orbs_f0', x: 690, y: 508 };
  const OVERLAP = { hp: { img: 'overlap_f0', x: 28, y: 507 }, mp: { img: 'overlap_f1', x: 691, y: 504 } };
  // Ceinture : alvéoles de panel_f3 (bevaux mesurés) — intérieurs 28 px.
  const BELT_X = [424, 455, 486, 517], BELT_Y = 561;
  // Icônes de compétence L/R (48×48, feuilles skillicon / <classe>skillicon).
  const SKL_L = { x: 117, y: 552 }, SKL_R = { x: 635, y: 552 };
  const XP = { x: 256, y: 562, w: 120 };
  const STAM = { x: 273, y: 573, w: 102, h: 19 };
  const ADDSTATS = { x: 206, y: 561 }, ADDSKILL = { x: 563, y: 561 };
  // Minipanel (mini_panel.go) : barre 176×26 centrée, 8 boutons 20×20.
  const MINI = { bar: { x: 325, y: 525 }, btn: { x: 328, y: 528 }, step: 22 };
  const ARROWS = { x: 392, y: 538 };
  // Inventaire (inventory.txt « Barbarian2 » v1.12 — identique pour les 7 classes).
  const INV = { panel: { x: 400, y: 60 }, grid: { x: 419, y: 315 }, cell: 29, cols: 10, rows: 4 };
  const GOLD_BTN = { x: 485, y: 455 }, GOLD_LABEL = { x: 510, y: 458 };
  const INV_CLOSE = { x: 419, y: 449 };

  // ------------------------------------------------------------ chargement
  function loadImg(name) {
    return new Promise((res) => {
      const im = new Image();
      im.onload = () => { D2UI.img[name] = im; res(); };
      im.onerror = () => { console.info('ui-d2 : sprite absent', name); res(); };
      im.src = `assets/ui/${name}.png`;
    });
  }
  function loadItem(code) {
    if (D2UI.items[code] || D2UI.missing.has(code)) return Promise.resolve();
    return new Promise((res) => {
      const im = new Image();
      im.onload = () => { D2UI.items[code] = im; res(); };
      im.onerror = () => { D2UI.missing.add(code); res(); };
      im.src = `assets/ui/item_${code}.png`;
    });
  }

  D2UI.init = function (manifest) {
    if (D2UI.ready || D2UI.booting) return;
    D2UI.booting = true;
    D2UI.canvas = document.getElementById('game');
    const names = [];
    for (const p of PANEL) names.push(`panel_f${p.f}`);
    names.push('orbs_f0', 'orbs_f1', 'overlap_f0', 'overlap_f1',
      'minipanel', 'inv_panel', 'goldcoinbtn_f0', 'goldcoinbtn_f1',
      'sheet_generic', 'menubutton_f0', 'menubutton_f2', 'addskill_f0');
    for (const cls of ['ba', 'so', 'pa', 'am', 'as', 'dz', 'ne']) names.push(`sheet_${cls}`);
    // onglets de l'arbre (skills-ui.js les partage via D2UI.img)
    for (const cls of ['ba', 'so', 'pa', 'am', 'as', 'dz', 'ne']) {
      for (let t = 1; t <= 3; t++) names.push(`sktree_${cls}_tab${t}`);
    }
    Promise.all(names.map(loadImg)).then(() => {
      buildHits();
      D2UI.ready = true;
    });
  };

  // ------------------------------------------------- zones cliquables (DOM)
  function hit(x, y, w, h, title, onclick) {
    const d = document.createElement('div');
    d.className = 'd2-hit';
    d.style.left = `${x}px`; d.style.top = `${y}px`;
    d.style.width = `${w}px`; d.style.height = `${h}px`;
    if (title) d.title = title;
    if (onclick) d.addEventListener('click', (e) => { e.stopPropagation(); onclick(); });
    D2UI.canvas.parentElement.appendChild(d);
    return d;
  }
  function buildHits() {
    const game = () => window.__d2.game;
    const flash = (m) => { const s = document.getElementById('status'); if (s) s.textContent += ` — ${m}`; };
    // orbes : info-bulle D2 (vie/mana)
    hit(30, 525, 80, 60, 'Vie');
    hit(695, 525, 80, 60, 'Mana');
    // compétences L/R : le popup de hud.js (boutons DOM skbtn cachés dessous)
    for (const [x, hand] of [[SKL_L.x, 'L'], [SKL_R.x, 'R']]) {
      hit(x, 552, 48, 48, `Compétence ${hand === 'L' ? 'gauche' : 'droite'} — cliquer pour choisir`, () => {
        const b = document.getElementById(`skbtn-${hand}`);
        if (b) b.click();
      });
    }
    // ceinture : clic = boire la potion du haut de la colonne (touches 1-4)
    for (let c = 0; c < 4; c++) {
      const col = c;
      hit(BELT_X[col], BELT_Y, 28, 28, `Ceinture ${col + 1}`, () => {
        try { game().useBeltKey(col + 1); } catch (e) { flash(`ceinture : ${e.message}`); }
      });
    }
    // boutons + (points à dépenser)
    hit(ADDSTATS.x, ADDSTATS.y, 30, 30, 'Points de stats — écran à venir', () => flash('Attribution de stats : à venir.'));
    hit(ADDSKILL.x, ADDSKILL.y, 30, 30, 'Points de compétences — ouvrir l\u2019arbre (T)', () => {
      if (window.D2Skills) window.D2Skills.toggle();
    });
    // minipanel : flèches + 8 boutons
    hit(ARROWS.x, ARROWS.y, 15, 24, 'Minipanel', () => D2UI.toggleMiniPanel());
    const defs = [
      ['Perso', 'Panneau de personnage — à venir', null],
      ['Inv', 'Inventaire (I)', () => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'i' }))],
      ['Comp.', 'Arbre de compétences (T)', () => { if (window.D2Skills) window.D2Skills.toggle(); }],
      ['Groupe', 'Groupe — à venir', null],
      ['Carte', 'Automap (Tab)', () => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Tab' }))],
      ['Msg', 'Messages — à venir', null],
      ['Quêtes', 'Journal de quêtes — à venir', null],
      ['Menu', 'Menu jeu — à venir (Esc)', null],
    ];
    D2UI.miniBtns = [];
    for (let i = 0; i < defs.length; i++) {
      const [label, title, fn] = defs[i];
      const d = hit(MINI.btn.x + i * MINI.step, MINI.btn.y, 20, 20, `${label} — ${title}`, fn);
      D2UI.miniBtns.push(d);
    }
    applyMini();
    // inventaire : fermer (petit socle en bas à gauche du panneau)
    hit(INV_CLOSE.x, INV_CLOSE.y, 30, 30, 'Fermer l\u2019inventaire', () => {
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'i' }));
    });
  }
  D2UI.toggleMiniPanel = function () {
    D2UI.miniOpen = !D2UI.miniOpen;
    applyMini();
  };
  function applyMini() {
    const disp = D2UI.miniOpen ? '' : 'none';
    for (const d of D2UI.miniBtns || []) d.style.display = disp;
  }

  // ------------------------------------------------------------ objets
  function itemImg(code) {
    return D2UI.items[code] || null;
  }
  D2UI.ensureItemSprites = function (codes) {
    for (const c of codes) if (c) loadItem(String(c).toLowerCase());
  };

  // ------------------------------------------------------- barre du bas
  function drawSlice(ctx, img, x, y, frac) {
    const h = Math.max(0, Math.min(1, frac)) * 80;
    if (h <= 0) return;
    ctx.drawImage(img, 0, 80 - h, 80, h, x, y + (80 - h), 80, h);
  }
  function sheetIcon(cls, cel) {
    const key = cel < 24 ? 'sheet_generic' : `sheet_${CLS[cls] || cls || 'ba'}`;
    const img = D2UI.img[key];
    if (!img) return null;
    const cell = cel < 24 ? cel : cel - 24;
    return { img, sx: (cell % 8) * 48, sy: Math.floor(cell / 8) * 48 };
  }
  function handIcon(hand) {
    // main sans compétence = attaque de base (feuille générique, cellule 2 —
    // attackIconID d'OpenDiablo2 hud.go).
    const hud = window.D2HUD;
    let code = null;
    if (hud && hud.hands) {
      const idx = hud.hands[hand];
      const s = idx >= 0 ? (hud.skills || [])[idx] : null;
      code = s ? s.code : null;
    }
    const cel = (code != null && D2UI.icons[code] !== undefined) ? D2UI.icons[code] : 2;
    return sheetIcon(D2UI.treeClass, cel) || null;
  }

  D2UI.drawHud = function (ctx, stats) {
    if (!D2UI.ready) return false;
    // cadre de la barre
    for (const p of PANEL) {
      const im = D2UI.img[`panel_f${p.f}`];
      if (im) ctx.drawImage(im, p.x, H - im.height);
    }
    // remplissage des orbes (tranche basse = liquide D2) + verre par-dessus
    if (stats) {
      drawSlice(ctx, D2UI.img[HP.img], HP.x, HP.y, stats.life / Math.max(1, stats.max_life));
      drawSlice(ctx, D2UI.img[MP.img], MP.x, MP.y, stats.mana / Math.max(1, stats.max_mana));
    }
    ctx.drawImage(D2UI.img[OVERLAP.hp.img], OVERLAP.hp.x, OVERLAP.hp.y);
    ctx.drawImage(D2UI.img[OVERLAP.mp.img], OVERLAP.mp.x, OVERLAP.mp.y);
    // ceinture : potion du haut de chaque colonne
    if (D2UI.belt) {
      for (let c = 0; c < 4; c++) {
        const cell = D2UI.belt[c] && D2UI.belt[c][0];
        if (!cell) continue;
        const im = itemImg(cell.code);
        if (im) ctx.drawImage(im, BELT_X[c], BELT_Y, 28, 28);
      }
    }
    // barre d'XP (rigole du panneau, trait blanc 2 px comme OD2)
    if (stats) {
      const lvl = Math.min(stats.level || 1, 10);
      const thr = [0, 500, 1500, 3750, 7875, 14175, 22680, 32886, 44396, 57715, 72144];
      const base = thr[lvl - 1] ?? 0, next = thr[lvl] ?? thr[10];
      const frac = next > base ? (stats.xp - base) / (next - base) : 1;
      ctx.fillStyle = '#ffffff';
      ctx.fillRect(XP.x, XP.y, Math.max(0, Math.min(1, frac)) * XP.w, 2);
      // endurance (OD2 : brun-ocre alpha 78 %, rouge sous 25 %)
      if (stats.max_stamina) {
        const f = Math.max(0, Math.min(1, (stats.stamina || 0) / stats.max_stamina));
        ctx.fillStyle = f < 0.25 ? 'rgba(255,0,0,0.78)' : 'rgba(175,136,72,0.78)';
        ctx.fillRect(STAM.x, STAM.y, f * STAM.w, STAM.h);
      }
    }
    // icônes de compétence L/R
    for (const [pos, hand] of [[SKL_L, 'L'], [SKL_R, 'R']]) {
      const ic = handIcon(hand);
      if (ic) ctx.drawImage(ic.img, ic.sx, ic.sy, 48, 48, pos.x, pos.y, 48, 48);
    }
    // boutons + (visibles quand des points sont à dépenser)
    if (D2UI.points > 0) {
      const im = D2UI.img.addskill_f0;
      if (im) { ctx.drawImage(im, ADDSTATS.x, ADDSTATS.y); ctx.drawImage(im, ADDSKILL.x, ADDSKILL.y); }
    }
    // minipanel (replié par défaut, flèches toujours visibles)
    if (D2UI.miniOpen) {
      const bar = D2UI.img.minipanel;
      if (bar) ctx.drawImage(bar, MINI.bar.x, MINI.bar.y);
    }
    const arrows = D2UI.img[D2UI.miniOpen ? 'menubutton_f0' : 'menubutton_f2'];
    if (arrows) ctx.drawImage(arrows, ARROWS.x, ARROWS.y);
    return true;
  };

  // ----------------------------------------------- sondes paresseuses (~1 Hz)
  D2UI.poll = function (game, now) {
    if (!D2UI.ready || now - D2UI.lastPoll < 1000) return;
    D2UI.lastPoll = now;
    try {
      const t = JSON.parse(game.skillTree());
      D2UI.points = t.points ?? 0;
      D2UI.treeClass = CLS[t.class] || t.class || D2UI.treeClass;
      for (const s of t.skills || []) D2UI.icons[s.code] = s.icon;
    } catch { /* arbre indisponible : boutons + masqués */ }
    try {
      const b = JSON.parse(game.beltInfo());
      D2UI.belt = b.belt || null;
      const codes = new Set();
      for (const col of D2UI.belt || []) for (const cell of col) if (cell) codes.add(cell.code);
      D2UI.ensureItemSprites([...codes]);
    } catch { /* ceinture indisponible avant finalize */ }
  };

  // ------------------------------------------------------- inventaire (I)
  D2UI.invHit = function (px, py) {
    const p = INV.panel;
    return px >= p.x && px < p.x + 320 && py >= p.y && py < p.y + 432;
  };
  D2UI.drawInventory = function (ctx, items, stats) {
    if (!D2UI.ready) return false;
    const panel = D2UI.img.inv_panel;
    if (!panel) return false;
    ctx.drawImage(panel, INV.panel.x, INV.panel.y);
    // grille 10×4 — sprites d'objets réels (28×28 dans des cellules de 29)
    const codes = new Set();
    for (let i = 0; items && i < items.length && i < INV.cols * INV.rows; i++) {
      const it = items[i];
      if (!it) continue;
      const col = i % INV.cols, row = Math.floor(i / INV.cols);
      const x = INV.grid.x + col * INV.cell, y = INV.grid.y + row * INV.cell;
      const im = itemImg(it.code);
      if (im) {
        ctx.drawImage(im, x, y, 28, 28);
      } else {
        // repli : bloc biseauté teinté par kind (objets sans sprite extrait)
        const colors = { Potion: ['#b03a2e', '#57150e'], Weapon: ['#8d99a6', '#39424c'], Armor: ['#8a6d3b', '#41331a'], Gold: ['#d9b96a', '#6e5626'] };
        const [body, edge] = colors[it.kind] || ['#9a8f7a', '#4a4438'];
        ctx.fillStyle = body; ctx.fillRect(x, y, 28, 28);
        ctx.strokeStyle = edge; ctx.lineWidth = 2; ctx.strokeRect(x + 1, y + 1, 26, 26);
        ctx.fillStyle = 'rgba(255,255,255,0.14)'; ctx.fillRect(x, y, 28, 4);
      }
      codes.add(it.code);
    }
    D2UI.ensureItemSprites([...codes]);
    // survol : liseré doré D2 + info-bulle du nom (géométrie grille conservée)
    const hover = window.__d2 && window.__d2.invHover;
    if (hover >= 0 && items && items[hover]) {
      const col = hover % INV.cols, row = Math.floor(hover / INV.cols);
      ctx.strokeStyle = '#d9b96a'; ctx.lineWidth = 1.5;
      ctx.strokeRect(INV.grid.x + col * INV.cell + 0.5, INV.grid.y + row * INV.cell + 0.5, 28, 28);
      const it = items[hover];
      const label = it.name + (it.amount ? ` (${it.amount} or)` : '');
      ctx.font = '11px system-ui';
      const tw = ctx.measureText(label).width + 16;
      const tx = Math.min(Math.max(INV.panel.x, INV.panel.x + 160 - tw / 2), INV.panel.x + 320 - tw);
      const ty = INV.grid.y - 26;
      ctx.fillStyle = '#141019ee'; ctx.fillRect(tx, ty, tw, 20);
      ctx.strokeStyle = '#5a4a2a'; ctx.lineWidth = 1; ctx.strokeRect(tx, ty, tw, 20);
      ctx.fillStyle = '#e8dcbf'; ctx.textAlign = 'left';
      ctx.fillText(label, tx + 8, ty + 14);
    }
    // or : pièce + montant (coin bas du panneau, comme D2)
    const coin = D2UI.img.goldcoinbtn_f0;
    if (coin) ctx.drawImage(coin, GOLD_BTN.x, GOLD_BTN.y);
    if (stats) {
      ctx.font = '600 11px system-ui';
      ctx.fillStyle = '#e8dcbf';
      ctx.textAlign = 'left';
      ctx.fillText(String(stats.gold ?? 0), GOLD_LABEL.x, GOLD_LABEL.y + 9);
    }
    return true;
  };

  window.D2UI = D2UI;
})();
