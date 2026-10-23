// Diablo 2 WASM — demo/vendor.js (domaine G : marchands & gamble).
// =====================================================================
// Panneau d'achat/vente minimal : grille de stock, clic = acheter (si l'or
// suffit), onglet « Vendre » sur l'inventaire. Miroir JS des règles de
// d2rules/src/vendor.rs (mêmes formules, mêmes tables réelles v1.12) :
//   achat = cost (colonne réelle), vente = floor(cost/4),
//   stock par colonnes vendeurs (XxxMin/Max/MagicMin/Max/MagicLvl),
//   permanent = PermStoreItem 1, 80 %/exemplaire non-permanent,
//   réserve Gheed 350-750, gamble 12 slots à gamble cost × (100+2·clvl)/100,
//   pool gamble = spawnable && qlvl ≤ niveau vendeur.
// ÉCARTS documentés (le moteur Rust fait foi, bindings WASM restants) :
//   - les items « magiques » du stock sont marqués sans rouler d'affixes ici ;
//   - la détermination RNG est un miroir mulberry32, pas le SplitMix64 Rust.
//
// INTÉGRATION — contrat exact :
//   window.D2Vendor.init(dom, hooks)
//     dom   : { toolbar?, root?, log? }  (tous optionnels — défauts #toolbar/#status)
//     hooks : { getGold():number,        or du héros (source de vérité),
//               spend(n):boolean,        débit (false = refus),
//               earn(n):void,            crédit (vente)
//               inventory():[{code,name,kind,cost?}],  inventaire vendable,
//               heroLevel():number,      niveau → niveau des vendeurs
//             }                          (tous optionnels — défauts ci-dessous)
//   Sans hooks d'intégration (game.js intact) : auto-branchement en lecture
//   sur window.__d2 (or/inventaire réels affichés, achats en SIMULATION
//   locale — le solde réel du moteur n'est pas mutable sans bindings).
window.D2Vendor = (function () {
  'use strict';

  var ROOT = new URL('..', location);
  var VENDORS = ['Akara', 'Charsi', 'Gheed'];
  var SPAWN_PCT = 80, GAMBLE_SLOTS = 12, GHEED_GOLD = [350, 750], DEFAULT_GOLD = 10000;

  // --- PRNG miroir (mulberry32, déterministe au seed) -----------------------
  function mulberry32(a) {
    return function () {
      a |= 0; a = (a + 0x6D2B79F5) | 0;
      var t = Math.imul(a ^ (a >>> 15), 1 | a);
      t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }
  function ri(rand, lo, hi) { return hi <= lo ? lo : lo + Math.floor(rand() * (hi - lo + 1)); }
  function chance(rand, pct) { return Math.floor(rand() * 100) < pct; }

  // --- tables réelles v1.12 (fetch des TSV du projet) -----------------------
  function tsv(text) {
    return text.split(/\r?\n/).filter(function (l) { return l.trim(); })
      .map(function (l) { return l.split('\t').map(function (c) { return c.trim().replace(/^"|"$/g, ''); }); });
  }
  function num(s) { var v = parseInt(s, 10); return isNaN(v) ? 0 : v; }
  function count(s) { return (s === '' || s === '0' || s === '255') ? 0 : num(s); }

  var WEAPON_TYPES = ['axe','swor','mace','knif','hamm','scep','staf','pole','club','h2h','h2h2',
    'spea','jave','ajav','tkni','tpot','taxe','orb','aspe','bow','xbow','abow'];
  var ARMOR_TYPES = ['tors','body','helm','circ','pelt','phlm','boot','glov','shie','ashd','belt'];

  function loadBases(text, kindOf, isArmo, out) {
    var rows = tsv(text); if (!rows.length) return;
    var hdr = rows[0], idx = {};
    hdr.forEach(function (h, i) { idx[h.toLowerCase()] = i; });
    var need = ['code', 'level', 'spawnable', 'cost', 'gamble cost'];
    if (need.some(function (n) { return idx[n] === undefined; })) return;
    VENDORS.forEach(function (v) {
      ['min', 'max', 'magicmin', 'magicmax', 'magiclvl'].forEach(function (s) {
        var c = (v + s).toLowerCase();
        if (idx[c] === undefined) idx[c] = -1;
      });
    });
    rows.slice(1).forEach(function (r) {
      var code = r[idx.code]; if (!code) return;
      var wtype = idx.type !== undefined ? r[idx.type] : '';
      var stock = {};
      VENDORS.forEach(function (v) {
        var g = function (s) { return idx[(v + s).toLowerCase()] >= 0 ? r[idx[(v + s).toLowerCase()]] : ''; };
        var max = count(g('max')), mmax = count(g('magicmax'));
        if (max > 0 || mmax > 0) {
          stock[v] = { min: count(g('min')), max: max, mmin: count(g('magicmin')), mmax: mmax, mlvl: count(g('magiclvl')) };
        }
      });
      var kind = kindOf(wtype);
      out.push({
        code: code, name: r[idx.name] || code, level: num(r[idx.level]),
        spawnable: r[idx.spawnable] === '1', cost: num(r[idx.cost]), gambleCost: num(r[idx['gamble cost']]),
        kind: kind, permanent: idx.permstoreitem !== undefined && idx.permstoreitem >= 0 && r[idx.permstoreitem] === '1',
        stock: stock,
        magic: kind === 'Weapon' || kind === 'Armor' ||
          ['ring', 'amul', 'jewl', 'ques'].indexOf(wtype) >= 0
      });
    });
  }

  var state = { catalog: [], gambleCodes: [], odds: null, byCode: {}, hooks: null, vendor: null, seed: 0x0D2, log: null };

  function kindOfMisc(wtype) {
    return (wtype === 'hpot' || wtype === 'rpot') ? 'Potion' : 'Misc';
  }

  function build(done) {
    var files = ['patch_weapons.txt', 'patch_armor.txt', 'patch_misc.txt',
      'patch_gamble.txt', 'patch_difficultylevels.txt'];
    Promise.all(files.map(function (f) {
      return fetch(new URL('assets/' + f, ROOT)).then(function (r) {
        if (!r.ok) throw new Error(f + ' HTTP ' + r.status); return r.text();
      });
    })).then(function (texts) {
      loadBases(texts[0], function () { return 'Weapon'; }, false, state.catalog);
      loadBases(texts[1], function () { return 'Armor'; }, true, state.catalog);
      loadBases(texts[2], kindOfMisc, false, state.catalog);
      state.catalog.forEach(function (b) { state.byCode[b.code] = b; });
      state.gambleCodes = tsv(texts[3]).slice(1)
        .map(function (r) { return r[1]; }).filter(function (c) { return c && state.byCode[c]; });
      var d = tsv(texts[4]), hdr = d[0], i = hdr.map(function (h) { return h.toLowerCase(); });
      var row = d[1]; // Normal
      state.odds = {
        rare: num(row[i.indexOf('gamblerare')]), set: num(row[i.indexOf('gambleset')]),
        unique: num(row[i.indexOf('gambleunique')]), uber: num(row[i.indexOf('gambleuber')]),
        ultra: num(row[i.indexOf('gambleultra')])
      };
      done();
    }).catch(function (e) {
      console.info('[vendor] tables indisponibles :', e && e.message);
      if (state.log) state.log('Tables marchandes indisponibles (' + (e && e.message) + ').');
    });
  }

  // --- règles miroirs (vendor.rs) -------------------------------------------
  function goldRange(id) { return id === 'Gheed' ? GHEED_GOLD : [DEFAULT_GOLD, DEFAULT_GOLD]; }

  function generate(id, vendorLevel, seed) {
    var rand = mulberry32(seed ^ 0xD26);
    var vendor = { id: id, name: id, level: vendorLevel, gold: 0, stock: [] };
    vendor.gold = ri(rand, goldRange(id)[0], goldRange(id)[1]);
    state.catalog.forEach(function (b) {
      var cnt = b.stock[id]; if (!cnt) return;
      var i, n, unit;
      if (cnt.max > 0) {
        n = ri(rand, Math.min(cnt.min, cnt.max), cnt.max);
        for (i = 0; i < n; i++) {
          if (!b.permanent && !chance(rand, SPAWN_PCT)) continue;
          unit = { code: b.code, name: b.name, kind: b.kind, quality: 'Normal', buy: b.cost, sell: Math.floor(b.cost / 4), permanent: b.permanent };
          vendor.stock.push(unit);
        }
      }
      if (cnt.mmax > 0 && b.magic) {
        n = ri(rand, Math.min(cnt.mmin, cnt.mmax), cnt.mmax);
        for (i = 0; i < n; i++) {
          if (!b.permanent && !chance(rand, SPAWN_PCT)) continue;
          unit = { code: b.code, name: b.name + ' (magique)', kind: b.kind, quality: 'Magic', buy: b.cost, sell: Math.floor(b.cost / 4), permanent: b.permanent };
          vendor.stock.push(unit);
        }
      }
    });
    return vendor;
  }

  function refresh(vendor, seed) {
    return generate(vendor.id, vendor.level, (seed ^ (seed << 7) >>> 0) >>> 0);
  }

  function gambleList(vendorLevel, clvl, seed) {
    var rand = mulberry32((seed ^ 0x6A1B) >>> 0);
    var pool = state.gambleCodes.map(function (c) { return state.byCode[c]; })
      .filter(function (b) { return b.spawnable && b.level <= vendorLevel; });
    if (!pool.length) return [];
    var wU = state.odds.unique + state.odds.uber + state.odds.ultra, wS = state.odds.set;
    var wR = state.odds.rare, wM = state.odds.rare, total = wU + wS + wR + wM;
    var out = [];
    for (var s = 0; s < GAMBLE_SLOTS; s++) {
      var b = pool[Math.floor(rand() * pool.length)];
      var roll = Math.floor(rand() * total);
      var q = roll < wU ? 'Unique' : roll < wU + wS ? 'Set' : roll < wU + wS + wR ? 'Rare' : 'Magic';
      if (q !== 'Magic' && q !== 'Rare' && !b.magic) q = 'Normal';
      var price = Math.floor(b.gambleCost * (100 + 2 * Math.min(clvl, 50)) / 100);
      out.push({ code: b.code, name: q === 'Normal' ? b.name : b.name + ' (?)', kind: b.kind,
        quality: q, buy: price, sell: Math.floor(b.cost / 4), permanent: false, gamble: true });
    }
    return out;
  }

  // --- hooks par défaut (lecture window.__d2 ; économie locale) -------------
  function defaultHooks() {
    var localGold = null;
    function snapshot() {
      try { return window.__d2 && window.__d2.game ? JSON.parse(window.__d2.game.heroStats()).gold : 500; }
      catch (e) { return 500; }
    }
    return {
      getGold: function () { return localGold === null ? snapshot() : localGold; },
      spend: function (n) { var g = this.getGold(); if (g < n) return false; localGold = g - n; return true; },
      earn: function (n) { localGold = this.getGold() + n; },
      inventory: function () {
        try { return (window.__d2 && window.__d2.invCache) || []; } catch (e) { return []; }
      },
      heroLevel: function () {
        try { return window.__d2 && window.__d2.game ? JSON.parse(window.__d2.game.heroStats()).level : 1; }
        catch (e) { return 1; }
      },
      simulated: true // achats/ventes en simulation locale (bindings restants)
    };
  }

  // --- panneau DOM -----------------------------------------------------------
  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }

  var KIND_COLORS = { Potion: '#b03a2e', Weapon: '#8d99a6', Armor: '#8a6d3b', Gold: '#d9b96a', Misc: '#6e7a5e' };

  function init(dom, hooks) {
    state.hooks = hooks || defaultHooks();
    var toolbar = (dom && dom.toolbar) || document.getElementById('toolbar');
    state.log = (dom && dom.log) || function (m) {
      var s = document.getElementById('status');
      if (s) s.textContent = s.textContent.replace(/ · \d+ fps.*$/, '') + ' — ' + m;
    };
    if (!toolbar) return;
    var openBtn = el('button', null, 'Marchands');
    openBtn.id = 'btn-vendors';
    openBtn.title = 'Achat / vente / gamble (domaine G)';
    toolbar.appendChild(openBtn);

    // Styles scopés du panneau.
    var css = el('style');
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

    var panel = el('div');
    panel.id = 'd2vendor';
    document.body.appendChild(panel);

    var tab = 'Akara', invMode = false;
    function render() {
      if (!state.catalog.length) { panel.innerHTML = '<h3>MARCHANDS</h3><div>Chargement des tables…</div>'; return; }
      panel.innerHTML = '';
      var h = state.hooks, clvl = h.heroLevel ? h.heroLevel() : 1;
      var title = el('h3', null, (invMode ? 'VENDRE' : tab.toUpperCase() + ' — MAGASIN') +
        ' · or : ' + h.getGold() + (h.simulated ? ' (simulation)' : ''));
      panel.appendChild(title);
      var tabs = el('div', 'tabs');
      VENDORS.concat(['Gamble']).forEach(function (t) {
        var b = el('button', t === tab && !invMode ? 'on' : null, t === 'Gamble' ? 'Gamble (Gheed)' : t);
        b.addEventListener('click', function () { tab = t; invMode = false; render(); });
        tabs.appendChild(b);
      });
      var sell = el('button', invMode ? 'on' : null, 'Vendre');
      sell.addEventListener('click', function () { invMode = !invMode; render(); });
      var rs = el('button', null, 'Restock');
      rs.title = 'Rafraîchit le stock (nouveau seed dérivé — vendor.rs)';
      rs.addEventListener('click', function () {
        state.seed = (state.seed * 1103515245 + 12345) >>> 0;
        state.vendor = null; render();
      });
      tabs.appendChild(sell); tabs.appendChild(rs);
      panel.appendChild(tabs);

      var grid = el('div', 'stock');
      var gold = h.getGold();
      if (invMode) {
        var items = (h.inventory && h.inventory()) || [];
        if (!items.length) panel.appendChild(el('div', 'foot', "Inventaire vide (ou non connecté : hooks.inventory())."));
        items.forEach(function (it, i) {
          var base = state.byCode[it.code];
          if (!base) return; // hors catalogue v1.12
          var price = Math.floor(base.cost / 4);
          var row = el('div', 'it');
          row.appendChild(el('span', null, it.name || it.code));
          row.appendChild(el('span', 'p', '+' + price));
          row.addEventListener('click', function () {
            var got = Math.floor(state.byCode[it.code].cost / 4);
            h.earn(got);
            state.log('Vendu ' + (it.name || it.code) + ' (+' + got + ' or).');
            render();
          });
          grid.appendChild(row);
        });
      } else {
        if (tab === 'Gamble') {
          if (!state.vendorGamble || state.vendorGambleLevel !== clvl) {
            state.vendorGamble = gambleList(clvl, clvl, state.seed);
            state.vendorGambleLevel = clvl;
          }
          state.vendorGamble.forEach(function (e2, i) {
            var row = el('div', 'it' + (gold < e2.buy ? ' na' : ''));
            var label = el('span', 'q-' + e2.quality, e2.name);
            row.appendChild(label);
            row.appendChild(el('span', 'p', e2.buy));
            row.addEventListener('click', function () { tryBuy(state.vendorGamble, i, e2.buy, true); });
            grid.appendChild(row);
          });
          panel.appendChild(grid);
          panel.appendChild(el('div', 'foot',
            'Gamble : items non identifiés — prix = gamble cost × (100+2·clvl)/100. ' +
            'Acheté = révélé (affixes déjà roulés côté Rust).'));
          return;
        }
        if (!state.vendor || state.vendor.id !== tab) state.vendor = generate(tab, clvl, state.seed);
        state.vendor.stock.forEach(function (e2, i) {
          var row = el('div', 'it' + (gold < e2.buy ? ' na' : ''));
          var nm = el('span', 'q-' + e2.quality, e2.name);
          row.appendChild(nm);
          row.appendChild(el('span', 'p', e2.buy + (e2.permanent ? ' ⚑' : '')));
          row.addEventListener('click', function () { tryBuy(state.vendor.stock, i, e2.buy, false); });
          grid.appendChild(row);
        });
        panel.appendChild(grid);
        panel.appendChild(el('div', 'foot',
          'Réserve ' + tab + ' : ' + state.vendor.gold + ' or · prix = valeur tables v1.12 · ' +
          'vente = valeur/4 · ⚑ permanent.'));
      }
      panel.appendChild(grid);
    }

    function tryBuy(stock, i, price, gamble) {
      var h = state.hooks;
      if (h.getGold() < price) { state.log('Pas assez d’or (' + price + ' requis).'); return; }
      if (!h.spend(price)) { state.log('Achat refusé par l’intégration.'); return; }
      stock.splice(i, 1);
      state.log((gamble ? 'Gamble acheté et révélé' : 'Acheté') + ' pour ' + price + ' or.');
      render();
    }

    openBtn.addEventListener('click', function () {
      panel.style.display = panel.style.display === 'block' ? 'none' : 'block';
      render();
    });
    build(render);
    return { panel: panel, render: render };
  }

  // Auto-branchement (démo) : bouton dans #toolbar dès que le DOM existe.
  function auto() {
    if (document.getElementById('toolbar') && !document.getElementById('btn-vendors')) {
      init({ toolbar: document.getElementById('toolbar') });
    }
  }
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', auto);
  else auto();

  return { init: init, generate: generate, gambleList: gambleList, _state: state };
})();
