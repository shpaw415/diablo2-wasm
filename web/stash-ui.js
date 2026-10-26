// Diablo 2 WASM — web/stash-ui.js (étape 11c : économie & inventaire — STASH).
// =====================================================================
// Panneau Stash 6×4 (inventory.txt « Bank Page ») + or séparé : miroir JS des
// règles de d2rules/src/economy.rs (le moteur Rust fait foi, mêmes contrats
// que web/vendor.js — bindings WASM restants) :
//   plafond or PORTÉ = 10 000 × niveau (formule D2),
//   plafond or STASH = 300 000 / 500 000 / 800 000 (Normal/NM/Enfer),
//   dépôt/retrait = débit/crédit réels, grille 6×4 canFit (première place
//   libre, balayage rangée par rangée — inventory.rs::InventoryGrid).
// PERSISTANCE : localStorage « d2w.stash.v1 » (miroir démo) ; côté moteur la
// save v3 porte le champ additif `gold_stash` (economy.rs, #[serde(default)]
// — rétro-compat saves antérieures : stash vide). ÉCARTS documentés :
//   - items du miroir en 1×1 (ItemDef wasm ne porte pas invwidth/invheight ;
//     les tailles réelles sont appliquées côté Rust par EquipItem) ;
//   - difficulté lue sur difficultyInfo() (défaut Normal).
// INTÉGRATION — contrat exact :
//   window.D2Stash.init(dom, hooks)
//     dom   : { toolbar?, log? }           (défauts #toolbar/#status)
//     hooks : { gold(), level(), diff(), spend(n), earn(n) } (défauts __d2)
//   Sans hooks (game.js intact) : lecture window.__d2 (or/niveau réels
//   affichés) ; dépôt/retrait opèrent sur le miroir local persisté.
export default {};
window.D2Stash = (function () {
  'use strict';

  var KEY = 'd2w.stash.v1';
  var W = 6, H = 4;                       // Bank Page 6×4 (inventory.txt)
  var CAP_PER_LEVEL = 10000;              // or porté : 10 000 × niveau (D2)
  var STASH_CAP = { normal: 300000, nightmare: 500000, hell: 800000 };

  var state = { gold: 0, cells: [], hooks: null, btn: null, panel: null };

  // --- état persisté (miroir démo) ------------------------------------------
  function load() {
    try {
      var raw = JSON.parse(localStorage.getItem(KEY) || '{}');
      state.gold = raw.gold >>> 0;
      state.cells = Array.isArray(raw.cells) ? raw.cells.slice(0, W * H) : [];
      while (state.cells.length < W * H) state.cells.push(null);
    } catch (e) { state.gold = 0; state.cells = []; }
  }
  function persist() {
    try { localStorage.setItem(KEY, JSON.stringify({ gold: state.gold, cells: state.cells })); }
    catch (e) { /* quota : la session reste utilisable, juste non persistée */ }
  }

  // --- hooks par défaut : lecture window.__d2 (moteur wasm) ------------------
  function stats() {
    try {
      return JSON.parse(window.__d2 && window.__d2.game ? window.__d2.game.heroStats() : '{}');
    } catch (e) { return {}; }
  }
  function diffName() {
    try {
      var s = window.__d2 && window.__d2.game ? window.__d2.game.difficultyInfo() : 'normal';
      s = String(s).toLowerCase();
      return s.indexOf('night') >= 0 || s.indexOf('cauch') >= 0 ? 'nightmare'
        : s.indexOf('hell') >= 0 || s.indexOf('enfer') >= 0 ? 'hell' : 'normal';
    } catch (e) { return 'normal'; }
  }
  function defaultHooks() {
    return {
      gold: function () { return stats().gold >>> 0; },
      level: function () { return (stats().level || 1) >>> 0; },
      diff: diffName,
      spend: function (n) { return false; }, // débit réel : branchement wasm restant
      earn: function () {},
    };
  }
  function carryCap() { return CAP_PER_LEVEL * Math.max(1, state.hooks.level()); }
  function stashCap() { return STASH_CAP[state.hooks.diff()] || STASH_CAP.normal; }

  // --- DOM -------------------------------------------------------------------
  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  function firstFit(w, h) { // première place libre, balayage rangée/rangée
    for (var y = 0; y <= H - h; y++) for (var x = 0; x <= W - w; x++) {
      var ok = true;
      for (var cy = y; cy < y + h && ok; cy++) for (var cx = x; cx < x + w && ok; cx++)
        if (state.cells[cy * W + cx]) ok = false;
      if (ok) return { x: x, y: y };
    }
    return null;
  }

  function render() {
    var p = state.panel, h = state.hooks;
    p.innerHTML = '';
    p.appendChild(el('h3', null, 'STASH — or : ' + state.gold + ' / ' + stashCap() +
      ' · bourse : ' + h.gold() + ' / ' + carryCap()));

    // Dépôt / retrait d'or (débit/crédit réels sur le miroir, plafonds D2).
    var goldRow = el('div', 'goldrow');
    [['Déposer 100', 100], ['Déposer 1 000', 1000], ['Déposer tout', Infinity]].forEach(function (d) {
      var b = el('button', null, d[0]);
      b.addEventListener('click', function () {
        var have = h.gold(), room = stashCap() - state.gold;
        var n = Math.min(d[1], have, Math.max(0, room));
        if (n <= 0) { log('Dépôt refusé : ' + (room <= 0 ? 'stash plein' : 'bourse vide') + '.'); return; }
        state.gold += n; persist(); render();
        log('Déposé ' + n + ' or au stash (bourse ' + have + ' → ' + (have - n) + ').');
      });
      goldRow.appendChild(b);
    });
    [['Retirer 100', 100], ['Retirer 1 000', 1000], ['Retirer tout', Infinity]].forEach(function (d) {
      var b = el('button', null, d[0]);
      b.addEventListener('click', function () {
        var have = h.gold(), room = carryCap() - have;
        var n = Math.min(d[1], state.gold, Math.max(0, room));
        if (n <= 0) { log('Retrait refusé : ' + (room <= 0 ? 'bourse au plafond 10 000 × niveau' : 'stash vide') + '.'); return; }
        state.gold -= n; persist(); render();
        log('Retiré ' + n + ' or du stash (bourse → ' + (have + n) + ').');
      });
      goldRow.appendChild(b);
    });
    p.appendChild(goldRow);

    // Grille 6×4 (Bank Page) — items du miroir (1×1, écart documenté).
    var grid = el('div', 'grid');
    for (var i = 0; i < W * H; i++) {
      var c = state.cells[i];
      var cell = el('div', 'cell' + (c ? ' full' : ''), c ? c.name : '');
      if (c) {
        cell.title = c.name + ' — clic : reprendre (miroir démo)';
        (function (idx) {
          cell.addEventListener('click', function () {
            log('Repris : ' + state.cells[idx].name + ' (miroir démo — branchement wasm restant).');
            state.cells[idx] = null; persist(); render();
          });
        })(i);
      }
      grid.appendChild(cell);
    }
    p.appendChild(grid);
    p.appendChild(el('div', 'foot', 'Plafonds D2 : porté 10 000 × niveau · stash 300k/500k/800k ' +
      '(Normal/NM/Enfer) — economy.rs · save additive `gold_stash` (rétro-compat v3).'));
  }

  function log(m) {
    var s = document.getElementById('status');
    if (s) s.textContent = s.textContent.replace(/ · \d+ fps.*$/, '') + ' — ' + m;
  }

  function init(dom, hooks) {
    load();
    state.hooks = hooks || defaultHooks();
    var toolbar = (dom && dom.toolbar) || document.getElementById('toolbar');
    if (toolbar && !state.btn) {
      state.btn = el('button', null, 'Stash');
      state.btn.id = 'btn-stash';
      state.btn.title = 'Stash 6×4 + or (étape 11c — economy.rs)';
      toolbar.appendChild(state.btn);
      state.btn.addEventListener('click', toggle);
    }
    if (!state.panel) {
      var css = el('style');
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
      state.panel = el('div');
      state.panel.id = 'd2stash';
      document.body.appendChild(state.panel);
    }
    render();
  }

  function toggle() {
    if (!state.panel) init();
    var on = state.panel.style.display !== 'block';
    state.panel.style.display = on ? 'block' : 'none';
    if (on) render();
  }

  return {
    init: init,
    toggle: toggle,
    /** Miroir (démo) : dépôt d'or avec les plafonds D2 — (déposé|0, message). */
    depositGold: function (n) {
      var room = stashCap() - state.gold;
      var done = Math.min(n >>> 0, Math.max(0, room));
      state.gold += done; persist();
      return { done: done, left: (n >>> 0) - done };
    },
    /** Miroir (démo) : retrait d'or sous plafond porté 10 000 × niveau. */
    withdrawGold: function (n) {
      var room = Math.max(0, carryCap() - state.hooks.gold());
      var done = Math.min(n >>> 0, state.gold, room);
      state.gold -= done; persist();
      return { done: done, left: (n >>> 0) - done };
    },
    /** Item 1×1 (miroir démo) : (x,y) première place libre ou null. */
    depositItem: function (name) {
      var spot = firstFit(1, 1);
      if (!spot) return null;
      state.cells[spot.y * W + spot.x] = { name: String(name) };
      persist();
      return spot;
    },
    state: function () { return { gold: state.gold, cells: state.cells.slice() }; },
  };
})();
