// Diablo 2 WASM — web/cube-ui.js (étape 18c : UI fidèle — CUBE HORADRIM).
// ========================================================================
// Panneau Cube 3×4 + zone de transmutation + tooltips items fidèles (fr),
// style HUD D2 (même famille que #d2-waypoints/#d2-skills : DOM absolute
// dans #stage, espace canvas 800×600). Nouveau fichier : game.js ne gagne
// qu'UNE ligne d'import — tout le reste est autonome (window.D2CubeUI).
//
// ENDPOINTS WASM consommés (18c, append-only) :
//   game.cubeTransmute('{"slots":[…],"seed":N}')  → moteur d2rules::cube
//     (145 recettes réelles v1.12) : JSON {ok,line,recipe,consumed,item,
//     portal,message,slot} ou exception fr (« Aucune recette… ») ;
//   game.cubeItemTooltip(itemJson) → tooltip fidèle {name,color,colorHex,
//     baseName,lines[]} : nom coloré par qualité (blanc/bleu/jaune/or/vert),
//     stats de base + affixes/runeword une ligne chacun (descfunc
//     ItemStatCost v1.12 + tables STRING fr réelles — côté Rust).
// État moteur lu : inventory() (invCache, ~60 fps), beltInfo(), groundItems().
//
// INTERACTION (mission 18c ① — clic-select / clic-place) :
//   • touche « C » : ouvre/ferme le panneau (inventaire ouvert ou non) ;
//   • clic sur un objet de la VUE INVENTAIRE (10×4, dans le panneau) →
//     sélection ; clic sur une cellule du CUBE (3×4) → posé (visuellement
//     retiré de l'inventaire) ; re-clic sur la cellule → repris ;
//   • bouton « Transmuter » → le MOTEUR fait foi : les slots consommés
//     sont réellement vidés, le produit est posé à la 1re place libre et
//     son tooltip complet s'affiche ; échec → message fr, RIEN ne bouge ;
//   • « Tout reprendre » : les objets posés retournent à la vue inventaire
//     (aucun objet n'a quitté l'inventaire réel avant la transmutation).
//
// DIVERGENCES DOCUMENTÉES (18c) :
//   • l'inventaire réel du MVP (ItemDef) ne porte NI qualité NI affixes :
//     les recettes exigeant une qualité en ENTRÉE (3 anneaux MAGIQUES →
//     amulette, upgrades uniques…) ne matchent pas — erreur fr propre ;
//     la qualité des PRODUITS est conservée (JSON `item.rolled` gardé en
//     session pour le tooltip, miroir « products ») ;
//   • les items « posés » dans le cube restent dans l'inventaire réel
//     jusqu'au clic Transmuter (références par slot, pas de mouvement) ;
//   • portails (Corne de Wirt + livre) : message fr « Un portail s'ouvre… »
//     — branchement gameplay restant (annoncé côté moteur).
// TOOLTIPS (mission 18c ②) : objets de l'inventaire (cellules du panneau),
//   ceinture (bandeau 4×4 en bas, touches 1-4) et SOL (survol des étiquettes
//   Alt de #itemlabels) — même renderer Rust pour tous.
(function () {
  'use strict';

  if (window.D2CubeUI) return; // idempotent (import garde-fou)

  var W = 800, H = 600;                 // espace canvas (game.html)
  var INV_COLS = 10, INV_ROWS = 4;      // grille inventaire moteur (10×4)
  var CELL = 26, PAD = 10, HEAD = 26;   // cellules DOM compactes du panneau
  var CUBE_COLS = 3, CUBE_ROWS = 4;     // patch_inventory.txt : boîte 3×4

  var state = {
    open: false, built: false,
    selected: -1,        // slot inventaire sélectionné (clic-select)
    placed: [],          // [{slot, cell}] — références jusqu'au Transmuter
    products: {},        // slot → JSON produit complet (tooltip session)
    message: '', msgErr: false,
    resultTip: null,     // JSON tooltip du dernier produit (affiché fixe)
    root: null, overlay: null, octx: null,
    invWrap: null, cubeWrap: null, msgWrap: null, btnTrans: null,
    belt: null, beltTimer: 0, tipEl: null,
  };

  // --- helpers DOM ----------------------------------------------------------
  function el(tag, cls, parent, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    (parent || state.root).appendChild(e);
    return e;
  }

  function game() { return window.__d2 && window.__d2.game; }
  function invCache() {
    try { return (window.__d2 && window.__d2.invCache) || []; } catch (e) { return []; }
  }

  // --- construction (une seule fois) ----------------------------------------
  function build() {
    if (state.built) return;
    state.built = true;
    var stage = document.getElementById('stage') || document.body;
    var root = document.createElement('div');
    root.id = 'd2cube-root';
    stage.appendChild(root);
    state.root = root;

    var css = [
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
    var st = document.createElement('style');
    st.textContent = css;
    document.head.appendChild(st);

    // Canvas d'overlay (surlignage inventaire / items « au cube »).
    var overlay = document.createElement('canvas');
    overlay.width = W; overlay.height = H;
    overlay.style.cssText = 'position:absolute; inset:0; pointer-events:none;';
    root.appendChild(overlay);
    state.overlay = overlay;
    state.octx = overlay.getContext('2d');

    // Panneau cube.
    var panel = el('div', '', root);
    panel.id = 'd2cube-panel';
    var h3 = el('h3', '', panel, 'Cube Horadrim');
    h3.title = 'Touche C pour ouvrir/fermer';
    var cols = el('div', 'd2cube-cols', panel);
    var sideInv = el('div', 'd2cube-side', cols);
    el('div', 'cap', sideInv, 'Inventaire (10×4)');
    state.invWrap = el('div', 'd2cube-grid', sideInv);
    state.invWrap.style.gridTemplateColumns = 'repeat(' + INV_COLS + ', 32px)';
    var sideCube = el('div', 'd2cube-side', cols);
    el('div', 'cap', sideCube, 'Cube de transmutation (3×4)');
    state.cubeWrap = el('div', 'd2cube-grid', sideCube);
    state.cubeWrap.style.gridTemplateColumns = 'repeat(' + CUBE_COLS + ', 32px)';

    var actions = el('div', 'd2cube-actions', panel);
    state.btnTrans = el('button', '', actions, 'Transmuter');
    state.btnTrans.title = 'Appelle le moteur (145 recettes réelles v1.12)';
    var btnBack = el('button', '', actions, 'Tout reprendre');
    btnBack.title = 'Reprendre les objets posés dans le cube';
    var btnClose = el('button', '', actions, 'Fermer (C)');
    state.msgWrap = el('div', '', panel);
    state.msgWrap.id = 'd2cube-msg';
    var result = el('div', '', panel);
    result.id = 'd2cube-result';

    state.btnTrans.addEventListener('click', transmute);
    btnBack.addEventListener('click', function () {
      state.placed = [];
      state.selected = -1;
      setMessage('Objets repris.', false);
      render();
    });
    btnClose.addEventListener('click', close);

    // Bandeau ceinture 4 colonnes (touches 1-4) × 4 — tooltip au survol.
    var belt = el('div', '', root);
    belt.id = 'd2cube-belt';
    var bbtn = el('button', 'cubebtn', belt, 'Cube (C)');
    bbtn.title = 'Ouvrir le Cube Horadrim';
    bbtn.addEventListener('click', function () { toggle(); });
    var bcols = el('div', 'belt-cols', belt);
    for (var c = 0; c < 4; c++) {
      var col = el('div', 'd2cube-beltcol', bcols);
      el('div', 'bkey', col, String(c + 1));
      for (var r = 0; r < 4; r++) {
        el('div', 'bcell', col).dataset.b = c * 4 + r;
      }
    }
    state.belt = belt;

    // Tooltip unique (inventaire / ceinture / sol).
    var tip = el('div', '', root);
    tip.id = 'd2cube-tip';
    state.tipEl = tip;

    wireTooltipEvents();
    setInterval(refreshBelt, 500);
    requestAnimationFrame(frame);
  }

  // --- ouverture / fermeture -------------------------------------------------
  function open() {
    build();
    state.open = true;
    document.getElementById('d2cube-panel').classList.add('open');
    setMessage('Placez des objets dans le cube (clic-sélection), puis Transmuter.', false);
    render();
  }
  function close() {
    state.open = false;
    state.selected = -1;
    state.placed = [];
    hideTip();
    var p = document.getElementById('d2cube-panel');
    if (p) p.classList.remove('open');
    drawOverlay();
  }
  function toggle() { state.open ? close() : open(); }

  function setMessage(msg, isErr) {
    state.message = msg || '';
    state.msgErr = !!isErr;
    if (!state.msgWrap) return;
    state.msgWrap.textContent = state.message;
    state.msgWrap.className = state.msgErr ? 'err' : (state.message ? 'ok' : '');
  }

  // --- couleurs items (miroir ITEM_COLORS game.js + qualité produit) ---------
  var KIND_COLORS = {
    Potion: ['#b03a2e', '#57150e'],
    Weapon: ['#8d99a6', '#39424c'],
    Armor: ['#8a6d3b', '#41331a'],
    Gold: ['#d9b96a', '#6e5626'],
    Misc: ['#7d6f58', '#3c3428'],
  };
  var QUALITY_HEX = { Magic: '#6969ff', Rare: '#ffff64', Unique: '#a59263', Set: '#00ff00', Crafted: '#ff8040' };

  function itemColor(it) {
    return KIND_COLORS[it.kind] || KIND_COLORS.Misc;
  }

  function itemLabel(it) {
    if (!it) return '';
    if (it.kind === 'Gold') return 'or';
    var n = String(it.name || it.code || '');
    return n.length > 5 ? n.slice(0, 5) : n;
  }

  // --- rendu du panneau -------------------------------------------------------
  function render() {
    if (!state.open || !state.built) return;
    var slots = invCache();
    // Synchronise : un slot « posé » dont l'objet a disparu (consommé) sort.
    state.placed = state.placed.filter(function (p) { return !!slots[p.slot]; });

    // Vue inventaire 10×4.
    var inv = state.invWrap;
    inv.textContent = '';
    for (var i = 0; i < INV_COLS * INV_ROWS; i++) {
      (function (slot) {
        var cell = el('div', 'd2cube-cell', inv);
        var it = slots[slot];
        var inCube = state.placed.some(function (p) { return p.slot === slot; });
        if (it && !inCube) {
          var d = el('div', 'it', cell, itemLabel(it));
          var pair = itemColor(it);
          d.style.background = pair[0];
          d.style.borderColor = pair[1];
          if (state.products[slot]) d.style.boxShadow = '0 0 4px ' + (QUALITY_HEX[state.products[slot].rolled && state.products[slot].rolled.quality] || '#c7a45a');
          d.title = '';
        }
        if (slot === state.selected) cell.classList.add('sel');
        cell.addEventListener('click', function (e) {
          e.stopPropagation();
          if (!it || inCube) { state.selected = -1; render(); return; }
          state.selected = (state.selected === slot) ? -1 : slot;
          setMessage(state.selected >= 0
            ? 'Objet sélectionné — cliquez une cellule du cube pour le placer.'
            : 'Sélection annulée.', false);
          hideTip();
          render();
        });
        cell.addEventListener('mouseenter', function () {
          if (it && !inCube) showTipForSlot(slot, cell);
        });
        cell.addEventListener('mouseleave', hideTip);
      })(i);
    }

    // Vue cube 3×4.
    var cube = state.cubeWrap;
    cube.textContent = '';
    for (var c = 0; c < CUBE_COLS * CUBE_ROWS; c++) {
      (function (cellIdx) {
        var cell = el('div', 'd2cube-cell', cube);
        var entry = null;
        for (var k = 0; k < state.placed.length; k++) {
          if (state.placed[k].cell === cellIdx) { entry = state.placed[k]; break; }
        }
        if (entry) {
          var it = slots[entry.slot];
          if (it) {
            var d = el('div', 'it', cell, itemLabel(it));
            var pair = itemColor(it);
            d.style.background = pair[0];
            d.style.borderColor = pair[1];
          }
          cell.addEventListener('click', function (e) {
            e.stopPropagation();
            state.placed = state.placed.filter(function (p) { return p.cell !== cellIdx; });
            setMessage('Objet repris du cube.', false);
            hideTip();
            render();
          });
          cell.addEventListener('mouseenter', function () {
            if (entry) showTipForSlot(entry.slot, cell);
          });
          cell.addEventListener('mouseleave', hideTip);
        } else {
          cell.addEventListener('click', function (e) {
            e.stopPropagation();
            if (state.selected < 0) {
              setMessage('Sélectionnez d\'abord un objet dans l\'inventaire.', true);
              return;
            }
            if (state.placed.some(function (p) { return p.slot === state.selected; })) return;
            state.placed.push({ slot: state.selected, cell: cellIdx });
            state.selected = -1;
            setMessage('Dans le cube — ' + state.placed.length + ' objet(s).', false);
            hideTip();
            render();
          });
        }
      })(c);
    }
    state.btnTrans.disabled = state.placed.length === 0;
    state.btnTrans.textContent = 'Transmuter (' + state.placed.length + ')';
    drawOverlay();
    lastSig = signature(); // la boucle ne re-rendra pas pour rien
  }

  // Surlignage de l'inventaire CANVAS (panneau I de game.js) : items « au
  // cube » marqués d'un liseré bleu, sélection en or — purement indicatif.
  function drawOverlay() {
    var ctx = state.octx;
    if (!ctx) return;
    ctx.clearRect(0, 0, W, H);
    if (!state.open || !state.placed.length) return;
    var pad = 14, header = 30, cell = 42;
    var pw = pad * 2 + INV_COLS * cell, ph = pad * 2 + header + INV_ROWS * cell;
    var x0 = (W - pw) / 2, y0 = (H - ph) / 2;
    var slots = invCache();
    ctx.save();
    ctx.lineWidth = 2;
    for (var k = 0; k < state.placed.length; k++) {
      var p = state.placed[k];
      if (!slots[p.slot]) continue;
      var cx = x0 + pad + (p.slot % INV_COLS) * cell;
      var cy = y0 + pad + header + Math.floor(p.slot / INV_COLS) * cell;
      ctx.strokeStyle = '#6969ff';
      ctx.strokeRect(cx + 0.5, cy + 0.5, cell - 1, cell - 1);
      ctx.strokeStyle = '#00000088';
      ctx.beginPath();
      ctx.moveTo(cx + 6, cy + cell / 2); ctx.lineTo(cx + cell - 6, cy + cell / 2);
      ctx.stroke();
    }
    if (state.selected >= 0 && slots[state.selected]) {
      var cx = x0 + pad + (state.selected % INV_COLS) * cell;
      var cy = y0 + pad + header + Math.floor(state.selected / INV_COLS) * cell;
      ctx.strokeStyle = '#ffe9ad';
      ctx.strokeRect(cx + 0.5, cy + 0.5, cell - 1, cell - 1);
    }
    ctx.restore();
  }

  // --- transmutation (LE MOTEUR FAIT FOI) ------------------------------------
  function transmute() {
    var g = game();
    if (!g || typeof g.cubeTransmute !== 'function') {
      setMessage('Moteur indisponible (pkg wasm sans endpoint cube).', true);
      return;
    }
    var slots = state.placed.map(function (p) { return p.slot; });
    if (!slots.length) return;
    var payload = JSON.stringify({ slots: slots, seed: Date.now() & 0x7fffffff });
    var res;
    try {
      res = JSON.parse(g.cubeTransmute(payload));
    } catch (err) {
      setMessage(String(err && err.message ? err.message : err), true);
      state.resultTip = null;
      render();
      return;
    }
    // Succès : les consommés ont disparu, le produit est en inventaire.
    if (res.item && res.item.slot !== undefined) {
      state.products[res.item.slot] = res.item;
      state.resultTip = res.item.tooltip || null;
    } else {
      state.resultTip = null;
    }
    state.placed = [];
    state.selected = -1;
    setMessage(res.message || 'Transmutation réussie.', false);
    renderResult(res);
    try {
      if (window.__d2) window.__d2.invCache = JSON.parse(g.inventory());
    } catch (e) { /* le cache se rafraîchit à la frame suivante */ }
    render();
  }

  function renderResult(res) {
    var box = document.getElementById('d2cube-result');
    if (!box) return;
    box.textContent = '';
    if (res.portal) {
      box.style.color = '#d9b96a';
      box.textContent = '⚗ ' + res.message;
      return;
    }
    var tip = state.resultTip;
    if (!tip) return;
    box.style.color = '';
    var name = document.createElement('div');
    name.textContent = tip.name || '';
    name.style.color = tip.colorHex || '#e8e8e8';
    name.style.fontWeight = '700';
    box.appendChild(name);
    if (tip.baseName) {
      var b = document.createElement('div');
      b.textContent = tip.baseName;
      b.style.color = '#e8e8e8';
      box.appendChild(b);
    }
    for (var i = 0; i < (tip.lines || []).length; i++) {
      var l = document.createElement('div');
      l.textContent = tip.lines[i];
      l.style.color = '#7f7fff';
      box.appendChild(l);
    }
  }

  // --- tooltips (inventaire / ceinture / sol) --------------------------------
  function tipHtml(tip) {
    var html = '';
    var name = tip.name || '';
    html += '<div style="color:' + (tip.colorHex || '#e8e8e8') + ';font-weight:700">' +
      escapeHtml(name) + '</div>';
    if (tip.baseName) {
      html += '<div style="color:#e8e8e8">' + escapeHtml(tip.baseName) + '</div>';
    }
    var lines = tip.lines || [];
    for (var i = 0; i < lines.length; i++) {
      html += '<div style="color:#7f7fff">' + escapeHtml(lines[i]) + '</div>';
    }
    return html;
  }

  function escapeHtml(s) {
    return String(s).replace(/[&<>"]/g, function (c) {
      return { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c];
    });
  }

  function requestTooltip(item, anchor) {
    var g = game();
    if (!g || typeof g.cubeItemTooltip !== 'function' || !item) return;
    var json;
    try { json = g.cubeItemTooltip(JSON.stringify(item)); } catch (e) { return; }
    var tip;
    try { tip = JSON.parse(json); } catch (e) { return; }
    showTip(tipHtml(tip), anchor);
  }

  function showTipForSlot(slot, anchor) {
    var it = invCache()[slot];
    if (!it) return;
    var known = state.products[slot];
    var view = {
      code: it.code, name: it.name,
    };
    if (known && known.rolled) {
      view.quality = known.rolled.quality;
      view.affixIds = known.rolled.affixIds;
      view.sockets = known.rolled.sockets;
      view.min_dmg = known.rolled.minDmg; view.max_dmg = known.rolled.maxDmg;
      view.ac = known.rolled.ac;
      view.req_str = known.rolled.reqStr; view.req_dex = known.rolled.reqDex;
      view.req_level = known.rolled.reqLevel;
      if (known.rolled.runeword) view.runeword = known.rolled.runeword;
    }
    requestTooltip(view, anchor);
  }

  function showTip(html, anchor) {
    var tip = state.tipEl;
    if (!tip) return;
    tip.innerHTML = html;
    tip.style.display = 'block';
    var rect = anchor.getBoundingClientRect();
    var host = state.root.getBoundingClientRect();
    var x = rect.right - host.left + 8, y = rect.top - host.top;
    if (x + 290 > W) x = rect.left - host.left - 292;
    if (x < 0) x = 4;
    tip.style.left = Math.max(2, Math.round(x)) + 'px';
    tip.style.top = Math.max(2, Math.round(Math.min(y, H - 160))) + 'px';
  }

  function hideTip() {
    if (state.tipEl) state.tipEl.style.display = 'none';
  }

  // Survol du SOL : les étiquettes Alt (#itemlabels .ilabel de hud.js) —
  // retrouvées par leurs coordonnées (left/top posés = sx/sy arrondis).
  function wireTooltipEvents() {
    var labels = document.getElementById('itemlabels');
    if (labels) {
      labels.addEventListener('mouseover', function (e) {
        var lab = e.target.closest && e.target.closest('.ilabel');
        if (!lab) return;
        var g = game();
        if (!g) return;
        var lx = parseInt(lab.style.left, 10), ly = parseInt(lab.style.top, 10) + 14;
        var items = [];
        try { items = JSON.parse(g.groundItems() || '[]'); } catch (err) { return; }
        for (var i = 0; i < items.length; i++) {
          if (Math.round(items[i].sx) === lx && Math.round(items[i].sy) === ly) {
            requestTooltip({ code: items[i].code, name: items[i].name }, lab);
            return;
          }
        }
      });
      labels.addEventListener('mouseout', hideTip);
    }
    // Ceinture : délégation sur les cellules (rafraîchies par refreshBelt).
    state.belt.addEventListener('mouseover', function (e) {
      var cell = e.target.closest && e.target.closest('.bcell');
      if (!cell || !cell.dataset.item) return;
      var it;
      try { it = JSON.parse(cell.dataset.item); } catch (err) { return; }
      requestTooltip({ code: it.code, name: it.name }, cell);
    });
    state.belt.addEventListener('mouseout', hideTip);
  }

  // --- ceinture (beltInfo, touches 1-4) ---------------------------------------
  function refreshBelt() {
    var belt = state.belt;
    if (!belt || document.hidden) return;
    var g = game();
    if (!g || typeof g.beltInfo !== 'function') return;
    var info;
    try { info = JSON.parse(g.beltInfo() || '{}'); } catch (e) { return; }
    var grid = info.belt || [];
    var cells = belt.querySelectorAll('.bcell');
    for (var i = 0; i < cells.length; i++) {
      var cell = cells[i];
      var b = parseInt(cell.dataset.b, 10);
      var col = Math.floor(b / 4), row = b % 4;
      var it = grid[col] ? grid[col][row] : null;
      cell.dataset.item = it ? JSON.stringify({ code: it.code, name: it.name }) : '';
      cell.textContent = '';
      if (it) {
        var d = document.createElement('div');
        d.className = 'it';
        d.textContent = it.code && it.code[0] ? it.code[0].toUpperCase() : '?';
        var pair = KIND_COLORS[it.kind] || KIND_COLORS.Misc;
        d.style.background = pair[0];
        cell.appendChild(d);
        cell.title = '';
      } else {
        cell.removeAttribute('title');
      }
    }
  }

  // --- boucle (léger : re-rendu uniquement quand invCache change) ------------
  var lastSig = '';
  function signature() {
    var slots = invCache();
    var sig = state.placed.map(function (p) { return p.slot + ':' + p.cell; }).join(',') +
      '|' + state.selected;
    for (var i = 0; i < slots.length; i++) {
      sig += '|' + (slots[i] ? slots[i].code : '-');
    }
    return sig;
  }
  var lastFrame = 0;
  function frame(now) {
    if (now - lastFrame > 120) { // ~8 fps suffit pour un panneau
      lastFrame = now;
      if (state.open && state.built) {
        var sig = signature();
        if (sig !== lastSig) {
          lastSig = sig;
          render(); // resynchronise invCache → vues (place/consommés)
        }
      }
    }
    requestAnimationFrame(frame);
  }

  // --- clavier : « C » ouvre/ferme (hors champs de saisie) ---------------------
  window.addEventListener('keydown', function (e) {
    var t = e.target;
    if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
    if (e.key === 'c' || e.key === 'C') {
      if (!game()) return; // pas de panneau sans moteur (fallback JS)
      toggle();
    } else if (e.key === 'Escape' && state.open) {
      close();
    }
  });

  window.D2CubeUI = { open: open, close: close, toggle: toggle };
})();
