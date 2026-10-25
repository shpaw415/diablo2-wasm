// Diablo 2 WASM — demo/panels-ui.js (étape 18b : UI fidèle, PLAN-FIDELITE
// phase 18 — PANNEAU PERSONNAGE + INVENTAIRE visuel + STASH).
// ========================================================================
// Panneaux DOM au-dessus du canvas, style HUD D2 unique (bordures sombres
// #5a4a2a, pierre #141019, or #c7a45a — même grammaire que hud.css/stash-ui),
// libellés fr (tbl.rs/TBL v1.12 : or, défense, résists…).
//
// DONNÉES RÉELLES consommées (endpoints wasm additifs 18b, panels_ui.rs) :
//   heroPanel()      — panneau personnage complet fr (str/dex/vit/ene,
//                      dégâts dérivés arme ×(1+str/100) comme le combat,
//                      défense, toucher, résists 4 + pénalité difficulté
//                      patch_difficultylevels 0/−40/−100 + max 75, MF/GF,
//                      or + plafond 10 000×niveau, XP/niveau, endurance,
//                      points à dépenser) ;
//   stashInfo()      — stash 6×4 « Bank Page » (inventory.txt) : or +
//                      plafond difficulté (economy.rs 300k/500k/800k) +
//                      items EquipItem avec taille réelle (invwidth/invheight)
//                      et position (grille partagée) ;
//   stashDeposit(n) / stashWithdraw(n) — dépôt/retrait d'or via le
//                      branchement economy.rs RÉEL (bourse débitée/créditée,
//                      plafonds D2, erreurs fr, aucun effet partiel) ;
//   inventory()      — 40 slots (null vide) du moteur ;
//   beltInfo()       — ceinture 4×4 + buffs (consumables_ui.rs) ;
//   useConsumable(i) — potion/parchemin du slot i (effets réels 11b).
//
// GÉOMÉTRIE RÉELLE (références lecture seule) :
//   assets/inventory.txt — grille perso 10×4, stash « Bank Page 1 » 6×4,
//   ceinture 4×4, slots bodylocs (rArm/torso/lArm/head/neck/rHand/lHand/
//   belt/feet/gloves) dessinés en silhouette ;
//   patch_misc/weapons/armor.txt colonnes invwidth/invheight — fetch au
//   1er affichage (échec ⇒ repli table intégrée des codes courants).
//
// ÉCARTS DOCUMENTÉS (le moteur reste foi) :
//   - l'inventaire MOTEUR range chaque item en 1×1 (ItemDef sans taille) ;
//     la grille 10×4 ci-dessous est un MIROIR VISUEL : items empilés à
//     leur taille réelle X×Y en première place libre (balayage rangée par
//     rangée, comme inventory.rs::find_first_fit), l'index de slot moteur
//     est conservé pour les actions (usage consommable) ;
//   - drag & drop simplifié : CLIC = saisir/poser (clic-clic, pas HTML5 DnD) ;
//     double-clic = utiliser le consommable ;
//   - équipement non branché côté moteur ⇒ slots en silhouette (décor).
//
// INTÉGRATION — zéro intrusion : auto-init au chargement du module (le
// game.js n'ajoute QU'UNE ligne `import("./panels-ui.js")`) ; la touche I
// est interceptée en CAPTURE (avant game.js) pour ouvrir CE panneau au lieu
// de l'inventaire canvas 1×1 ; touches C (perso), B (stash), Échap.
export default {};
window.D2Panels = (function () {
  'use strict';

  // --- constantes réelles --------------------------------------------------
  var INV_W = 10, INV_H = 4;            // grille perso (inventory.txt)
  var STASH_W = 6, STASH_H = 4;         // Bank Page 1 (inventory.txt)
  var BELT_W = 4, BELT_H = 4;           // ceinture 4×4
  var KEY = 'd2w.panels.invl.v1';       // miroir visuel persisté
  var CELL = 30;                        // px par cellule de grille

  // Slots d'équipement bodylocs (inventory.txt) — disposition papier D2.
  // [id, libellé fr, largeur, hauteur, col, rangée] (silhouettes décor).
  var SLOTS = [
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

  // Couleurs D2 par catégorie (bodies/bordures, même palette que game.js).
  var KIND_COLORS = {
    Potion: ['#b03a2e', '#57150e'],
    Weapon: ['#8d99a6', '#39424c'],
    Armor: ['#8a6d3b', '#41331a'],
    Gold: ['#d9b96a', '#6e5626'],
    Misc: ['#7a8a5a', '#39442a'],
  };

  // Repli tailles (patch_misc/weapons/armor.txt v1.12, invwidth/invheight)
  // si le fetch des tables échoue — codes réellement produits au MVP.
  var FALLBACK_SIZES = {
    gld: [1, 1], hp1: [1, 1], hp2: [1, 1], hp3: [1, 1], hp4: [1, 1], hp5: [1, 1],
    mp1: [1, 1], mp2: [1, 1], mp3: [1, 1], mp4: [1, 1], mp5: [1, 1],
    rvs: [1, 1], rvl: [1, 1], tsc: [1, 1], tbk: [1, 2], isc: [1, 1], ibk: [1, 2],
    key: [1, 1], hax: [1, 3], buc: [2, 2], box: [2, 2],
  };

  var state = {
    inited: false, game: null,
    sizeTable: null,           // code → [w,h] (tables réelles) ou null
    layout: {},                // clé item → {x,y,w,h} (miroir visuel)
    sel: null,                 // item saisi (clic-clic)
    panels: {},                // id → {root, open}
    tablesNote: false,
  };

  // --- helpers ---------------------------------------------------------------
  function el(tag, cls, text) {
    var e = document.createElement(tag);
    if (cls) e.className = cls;
    if (text !== undefined) e.textContent = text;
    return e;
  }
  function game() {
    try { return window.__d2 && window.__d2.game ? window.__d2.game : null; }
    catch (e) { return null; }
  }
  function jparse(s, fb) {
    try { return JSON.parse(s); } catch (e) { return fb; }
  }
  function stats() {
    var g = game();
    return g ? jparse(g.heroStats(), {}) : {};
  }
  function invItems() {
    var g = game();
    var slots = g ? jparse(g.inventory(), []) : [];
    var out = [];
    for (var i = 0; i < slots.length; i++) {
      if (slots[i]) {
        var it = slots[i];
        it.slot = i;
        out.push(it);
      }
    }
    return out;
  }
  function itemKey(it) {
    // slot moteur inclus : DEUX items identiques coexistent (2 potions hp1)
    return it.code + '|' + it.name + '|' + (it.amount || 0) + '|' + (it.slot === undefined ? -1 : it.slot);
  }
  function sizeOf(code) {
    if (state.sizeTable && state.sizeTable[code]) return state.sizeTable[code];
    return FALLBACK_SIZES[code] || [1, 1];
  }
  function kindOf(it) {
    if (it.kind === 'Gold') return 'Gold';
    if (it.kind === 'Potion') return 'Potion';
    if (it.kind === 'Weapon') return 'Weapon';
    if (it.kind === 'Armor') return 'Armor';
    return 'Misc';
  }

  // Place un item X×Y en première place libre (balayage rangée/rangée —
  // même ordre qu'inventory.rs::find_first_fit) sur une grille w×h occupée
  // par `blocks` [{x,y,w,h,skip}].
  function firstFit(w, h, gw, gh, blocks, skip) {
    var occ = {};
    for (var b of blocks) {
      if (b === skip) continue;
      for (var y = b.y; y < b.y + b.h; y++)
        for (var x = b.x; x < b.x + b.w; x++) occ[x + ',' + y] = true;
    }
    for (var cy = 0; cy <= gh - h; cy++)
      for (var cx = 0; cx <= gw - w; cx++) {
        var ok = true;
        for (var y2 = cy; y2 < cy + h && ok; y2++)
          for (var x2 = cx; x2 < cx + w && ok; x2++)
            if (occ[x2 + ',' + y2]) ok = false;
        if (ok) return { x: cx, y: cy };
      }
    return null; // ne tient pas : dessiné compacté sous la grille (plein)
  }

  // Miroir visuel : positions X×Y stables pour la liste d'items moteur.
  function layoutOf(items) {
    var seen = {}, blocks = [], laid = [];
    var saved = state.layout;
    // 1) positions persistées encore valides
    for (var it of items) {
      var k = itemKey(it), s = sizeOf(it.code);
      var p = saved[k];
      if (p && p.w === s[0] && p.h === s[1] && !seen[k] &&
          p.x + s[0] <= INV_W && p.y + s[1] <= INV_H) {
        seen[k] = true;
        var b = { x: p.x, y: p.y, w: s[0], h: s[1] };
        blocks.push(b);
        laid.push({ item: it, box: b });
      }
    }
    // 2) nouvelles entrées : première place libre
    for (var it2 of items) {
      var k2 = itemKey(it2);
      if (seen[k2]) continue;
      seen[k2] = true;
      var s2 = sizeOf(it2.code);
      var spot = firstFit(s2[0], s2[1], INV_W, INV_H, blocks, null);
      var b2 = spot ? { x: spot.x, y: spot.y, w: s2[0], h: s2[1] }
                    : { x: 0, y: 0, w: s2[0], h: s2[1], overflow: true };
      blocks.push(b2);
      laid.push({ item: it2, box: b2 });
    }
    // persistance (best effort)
    var out = {};
    for (var l of laid) out[itemKey(l.item)] = l.box;
    state.layout = out;
    try { localStorage.setItem(KEY, JSON.stringify(out)); } catch (e) { /* quota */ }
    return laid;
  }

  function loadSizes() {
    if (state.sizeTable) return;
    var roots = [new URL('..', location), new URL('.', location)];
    var files = ['assets/patch_misc.txt', 'assets/patch_weapons.txt', 'assets/patch_armor.txt'];
    var table = {};
    var done = 0, failed = 0;
    roots.forEach(function (root) {
      files.forEach(function (f) {
        fetch(new URL(f, root)).then(function (r) {
          if (!r.ok) throw new Error(String(r.status));
          return r.text();
        }).then(function (txt) {
          var lines = txt.split('\n');
          var hdr = lines[0].split('\t');
          var iw = hdr.indexOf('invwidth'), ih = hdr.indexOf('invheight'), ic = hdr.indexOf('code');
          if (iw < 0 || ih < 0 || ic < 0) return;
          for (var i = 1; i < lines.length; i++) {
            var cols = lines[i].split('\t');
            if (cols.length <= Math.max(iw, ih, ic)) continue;
            var code = cols[ic].trim();
            var w = parseInt(cols[iw], 10), h = parseInt(cols[ih], 10);
            if (code && w > 0 && h > 0) table[code] = [w, h];
          }
        }).catch(function () { failed++; }).finally(function () {
          if (++done === roots.length * files.length) {
            if (Object.keys(table).length) state.sizeTable = table;
            else if (!state.tablesNote) { state.tablesNote = true; }
            refreshOpen();
          }
        });
      });
    });
  }

  // --- panneau PERSONNAGE ------------------------------------------------------
  // Deux colonnes D2 : à gauche Caractéristiques (fr), à droite stats
  // dérivées + résists ; barre XP en pied de panneau.
  function fmt(n) { return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ' '); }

  function statRow(label, value, cls) {
    var r = el('div', 'prow' + (cls ? ' ' + cls : ''));
    r.appendChild(el('span', 'plab', label));
    r.appendChild(el('span', 'pval', String(value)));
    return r;
  }

  // Seuils XP CUMULÉS niv 1..10 — patch_experience.txt colonne Barbarian
  // (identiques game.js, d2rules::xp) pour la barre de progression du panneau.
  var XP_THR = [0, 500, 1500, 3750, 7875, 14175, 22680, 32886, 44396, 57715, 72144];
  function xpFrac(level, xp) {
    var lvl = Math.min(level, 10);
    var base = XP_THR[lvl - 1] != null ? XP_THR[lvl - 1] : 0;
    var next = XP_THR[lvl] != null ? XP_THR[lvl] : XP_THR[10];
    return next > base ? Math.max(0, Math.min(1, (xp - base) / (next - base))) : 1;
  }

  function renderChar(root) {
    var p = jparse(game() ? game().heroPanel() : '{}', {});
    root.textContent = '';
    if (!p.level) {
      root.appendChild(el('div', 'foot', 'Moteur non initialisé.'));
      return;
    }
    var head = el('div', 'phead');
    head.appendChild(el('h3', null, p.className + ' — Niveau ' + p.level));
    var xp = el('div', 'xpbar');
    var xpFill = el('div', 'xpfill');
    xpFill.style.width = (xpFrac(p.level, p.xp) * 100).toFixed(1) + '%';
    xp.appendChild(xpFill);
    head.appendChild(xp);
    head.appendChild(el('div', 'foot', 'XP ' + fmt(p.xp) + ' — prochain niveau dans ' + fmt(p.xpToNext)));
    root.appendChild(head);

    var cols = el('div', 'pcols');
    var left = el('div', 'pcol');
    left.appendChild(el('h4', null, 'Caractéristiques'));
    left.appendChild(statRow('Force', p.strength));
    left.appendChild(statRow('Dextérité', p.dexterity));
    left.appendChild(statRow('Vitalité', p.vitality));
    left.appendChild(statRow('Énergie', p.energy));
    left.appendChild(statRow('Points à dépenser', (p.statPoints || 0) + ' / ' + (p.skillPoints || 0) + ' (car. / comp.)', 'dim'));
    left.appendChild(el('h4', null, 'Vie et mana'));
    left.appendChild(statRow('Vie', p.life + ' / ' + p.maxLife));
    left.appendChild(statRow('Mana', p.mana + ' / ' + p.maxMana));
    left.appendChild(statRow('Endurance', p.stamina + ' / ' + p.maxStamina));

    var right = el('div', 'pcol');
    right.appendChild(el('h4', null, 'Combat'));
    right.appendChild(statRow('Dégâts', p.dmgMin + ' – ' + p.dmgMax, 'gold'));
    right.appendChild(statRow('Arme', p.weapon || '—', 'dim'));
    right.appendChild(statRow('Défense', p.defense));
    right.appendChild(statRow('Toucher', p.toHit));
    right.appendChild(el('h4', null, 'Résistances (max ' + p.resMax + ' %)'));
    right.appendChild(statRow('Feu', (p.resFire > 0 ? '+' : '') + p.resFire + ' %', p.resFire < 0 ? 'neg' : ''));
    right.appendChild(statRow('Foudre', (p.resLight > 0 ? '+' : '') + p.resLight + ' %', p.resLight < 0 ? 'neg' : ''));
    right.appendChild(statRow('Froid', (p.resCold > 0 ? '+' : '') + p.resCold + ' %', p.resCold < 0 ? 'neg' : ''));
    right.appendChild(statRow('Poison', (p.resPoison > 0 ? '+' : '') + p.resPoison + ' %', p.resPoison < 0 ? 'neg' : ''));
    right.appendChild(el('h4', null, 'Chance de trouver'));
    right.appendChild(statRow('Objets magiques (MF)', '+' + p.mf + ' %'));
    right.appendChild(statRow('Or (GF)', '+' + p.gf + ' %'));
    right.appendChild(statRow('Or porté', fmt(p.gold) + ' / ' + fmt(p.goldCap), 'gold'));
    cols.appendChild(left);
    cols.appendChild(right);
    root.appendChild(cols);
    root.appendChild(el('div', 'foot',
      'Dégâts dérivés de l\'arme (×(1 + Force/100), formule du moteur) · résists : ' +
      'pénalité de difficulté patch_difficultylevels (0/−40/−100) · or : plafond 10 000 × niveau.'));
  }

  // --- INVENTAIRE visuel (10×4 X×Y + ceinture 4×4 + slots silhouette) --------
  function gridEl(w, h, cell) {
    var g = el('div', 'pgrid');
    g.style.gridTemplateColumns = 'repeat(' + w + ', ' + cell + 'px)';
    return g;
  }

  function itemBlock(laid, onClick, onUse) {
    var it = laid.item, b = laid.box;
    var colors = KIND_COLORS[kindOf(it)] || KIND_COLORS.Misc;
    var d = el('div', 'pitem');
    d.style.left = b.x * CELL + 'px';
    d.style.top = b.y * CELL + 'px';
    d.style.width = b.w * CELL - 2 + 'px';
    d.style.height = b.h * CELL - 2 + 'px';
    d.style.background = colors[0];
    d.style.borderColor = colors[1];
    d.title = it.name + (it.amount ? ' — ' + it.amount + ' or' : '') +
      ' (' + b.w + '×' + b.h + ')' + (onUse ? ' — double-clic : utiliser' : ' — clic : saisir/poser');
    if (onUse) d.addEventListener('dblclick', function (e) { e.stopPropagation(); onUse(it); });
    if (onClick) d.addEventListener('click', function (e) { e.stopPropagation(); onClick(it, d); });
    if (state.sel && itemKey(state.sel) === itemKey(it)) d.classList.add('sel');
    d.appendChild(el('span', 'pitemname', it.name));
    return d;
  }

  function renderInv(root) {
    root.textContent = '';
    var items = invItems();
    var laid = layoutOf(items);
    loadSizes();

    // Slots d'équipement en silhouette (inventory.txt bodylocs) — décor.
    var doll = el('div', 'pdoll');
    doll.style.width = (9 * CELL) + 'px';
    SLOTS.forEach(function (s) {
      var d = el('div', 'pslot');
      d.style.left = s[4] * CELL + 'px';
      d.style.top = s[5] * CELL + 'px';
      d.style.width = s[2] * CELL - 2 + 'px';
      d.style.height = s[3] * CELL - 2 + 'px';
      d.title = s[1] + ' — équipement non branché au moteur (silhouette)';
      d.appendChild(el('span', 'pslotname', s[1]));
      doll.appendChild(d);
    });
    root.appendChild(doll);

    // Grille 10×4 — items empilés à leur taille réelle.
    var gridWrap = el('div', 'pgridwrap');
    var grid = gridEl(INV_W, INV_H, CELL);
    grid.style.height = INV_H * CELL + 'px';
    for (var y = 0; y < INV_H; y++)
      for (var x = 0; x < INV_W; x++) {
        var c = el('div', 'pcell');
        c.style.left = x * CELL + 'px';
        c.style.top = y * CELL + 'px';
        grid.appendChild(c);
      }
    // clic-clic : poser l'item saisi sur une zone libre qui l'accepte.
    grid.addEventListener('click', function () {
      if (!state.sel) return;
      var rest = laid.filter(function (l) { return itemKey(l.item) !== itemKey(state.sel); });
      var s = sizeOf(state.sel.code);
      var spot = firstFit(s[0], s[1], INV_W, INV_H, rest.map(function (l) { return l.box; }), null);
      if (!spot) { note('Aucune place libre pour ' + state.sel.name + ' (' + s[0] + '×' + s[1] + ').'); return; }
      state.layout[itemKey(state.sel)] = { x: spot.x, y: spot.y, w: s[0], h: s[1] };
      note(state.sel.name + ' posé en (' + spot.x + ', ' + spot.y + ').');
      state.sel = null;
      refreshOpen();
    });
    laid.forEach(function (l) {
      grid.appendChild(itemBlock(l, function (it, node) {
        if (state.sel && itemKey(state.sel) === itemKey(it)) { state.sel = null; }
        else { state.sel = it; }
        refreshOpen();
      }, function (it) {
        useConsumable(it);
      }));
    });
    // items qui ne tiennent pas visuellement : compacts sous la grille
    var over = laid.filter(function (l) { return l.box.overflow; });
    gridWrap.appendChild(grid);
    root.appendChild(gridWrap);
    if (over.length) {
      var ov = el('div', 'pover');
      over.forEach(function (l) {
        var chip = itemBlock({ item: l.item, box: { x: 0, y: 0, w: l.box.w, h: l.box.h } }, null, null);
        chip.classList.add('chip');
        chip.style.position = 'static';
        ov.appendChild(chip);
      });
      root.appendChild(el('div', 'foot', over.length + ' objet(s) sans place visible (grille visuelle pleine).'));
      root.appendChild(ov);
    }

    // Ceinture 4×4 (consumables_ui.rs — 4 colonnes, potions empilées).
    var belt = jparse(game() && game().beltInfo ? game().beltInfo() : '{}', {});
    var beltSec = el('div', 'pbelthead');
    beltSec.appendChild(el('h4', null, 'Ceinture (1-4)'));
    var bg = gridEl(BELT_W, BELT_H, CELL);
    bg.style.height = BELT_H * CELL + 'px';
    for (var by = 0; by < BELT_H; by++)
      for (var bx = 0; bx < BELT_W; bx++) {
        var bc = el('div', 'pcell');
        bc.style.left = bx * CELL + 'px';
        bc.style.top = by * CELL + 'px';
        bg.appendChild(bc);
      }
    var beltArr = (belt && belt.belt) || [];
    for (var col = 0; col < BELT_W; col++) {
      var cells = beltArr[col] || [];
      for (var row = 0; row < BELT_H; row++) {
        var cell = cells[row];
        if (!cell) continue;
        (function (cnum, potion) {
          bg.appendChild(itemBlock(
            { item: { code: cell.code, name: cell.name, kind: cell.kind, amount: 0, slot: -1 },
              box: { x: cnum, y: row, w: 1, h: 1 } },
            null,
            potion ? function () { tryBeltKey(cnum + 1); } : null));
        })(col, cell.kind === 'Potion' || cell.kind === 'Misc');
      }
    }
    beltSec.appendChild(bg);
    root.appendChild(beltSec);
    root.appendChild(el('div', 'foot',
      'Miroir visuel : le moteur range chaque objet en 1×1 (écart documenté) — ici les tailles réelles ' +
      'invwidth/invheight (patch_*.txt) sont empilées en première place libre ; l\'index moteur est conservé pour les actions.'));
  }

  function useConsumable(it) {
    var g = game();
    if (!g || it.slot < 0) return;
    var scrolls = ['tsc', 'tbk', 'isc', 'ibk'];
    if (it.kind !== 'Potion' && scrolls.indexOf(it.code) < 0) {
      note(it.name + ' — équipement non branché au moteur (phase 3).');
      return;
    }
    try {
      var r = jparse(g.useConsumable(it.slot), {});
      if (r.use) {
        var u = r.use;
        note(u.name + ' : ' + (u.heal > 0 ? '+' + u.heal + ' pv ' : '') +
          (u.mana > 0 ? '+' + u.mana + ' mana ' : '') +
          (u.buffMs > 0 ? 'effet ' + Math.round(u.buffMs / 1000) + ' s ' : '') +
          (u.townPortal ? 'retour au camp' : '').trim());
        if (r.levelSwitch && window.__d2 && window.__d2.refreshStatus) window.__d2.refreshStatus();
      }
    } catch (e) {
      note(String(e && e.message ? e.message : e));
    }
    state.sel = null;
    refreshOpen();
  }

  function tryBeltKey(n) {
    var g = game();
    if (!g || !g.useBeltKey) return;
    try {
      var r = jparse(g.useBeltKey(n), {});
      if (r.use) note(r.use.name + ' bue (touche ' + n + ').');
      else note('Colonne ' + n + ' : vide.');
    } catch (e) { note(String(e && e.message ? e.message : e)); }
    refreshOpen();
  }

  // --- STASH 6×4 + or (economy.rs réel) ---------------------------------------
  function renderStash(root) {
    root.textContent = '';
    var g = game();
    var s = g ? jparse(g.stashInfo ? g.stashInfo() : '{}', {}) : {};
    if (!s.w) {
      root.appendChild(el('div', 'foot', 'Moteur non initialisé.'));
      return;
    }
    var head = el('div', 'phead');
    head.appendChild(el('h3', null, 'STASH — ' + fmt(s.gold) + ' / ' + fmt(s.goldCap) + ' or'));
    head.appendChild(el('div', 'foot', 'Difficulté ' + s.difficulty + ' · bourse : ' +
      fmt(stats().gold || 0) + ' / ' + fmt(s.carryCap) + ' or (10 000 × niveau)'));
    root.appendChild(head);

    // Dépôt / retrait — branchement economy.rs RÉEL (stashDeposit/stashWithdraw).
    // kind : nombre > 0 = dépôt du montant ; nombre < 0 = retrait du montant ;
    // 'dep' = déposer tout ; 'wd' = retirer tout. L'engine reste foi (elle
    // recalcule, plafonne et refuse en fr) — les montants « tout » sont les
    // soldes exacts pour éviter un aller-retour d'erreur évitable.
    function goldBtn(label, kind) {
      var b = el('button', null, label);
      b.addEventListener('click', function () {
        var gg = game();
        if (!gg || !gg.stashDeposit) return;
        var info = jparse(gg.stashInfo ? gg.stashInfo() : '{}', {}) || {};
        var carried = stats().gold || 0;
        var dep = (typeof kind === 'number' && kind > 0) || kind === 'dep';
        var amt;
        if (kind === 'dep') amt = Math.min(carried, Math.max(0, (info.goldCap || 0) - (info.gold || 0)));
        else if (kind === 'wd') amt = info.gold || 0;
        else amt = Math.abs(kind);
        if (amt <= 0) {
          note(dep ? 'Dépôt refusé : ' + (carried <= 0 ? 'bourse vide.' : 'stash plein.')
                   : 'Retrait refusé : stash vide.');
          return;
        }
        try {
          var v = jparse(dep ? gg.stashDeposit(amt) : gg.stashWithdraw(amt), {});
          note((dep ? 'Déposé ' : 'Retiré ') + fmt(amt) + ' or (stash ' + fmt(v.gold) + ').');
        } catch (e) {
          note(String(e && e.message ? e.message : e).replace(/^Error: /, ''));
        }
        refreshOpen();
      });
      return b;
    }
    var row = el('div', 'pgoldrow');
    row.appendChild(goldBtn('Déposer 100', 100));
    row.appendChild(goldBtn('Déposer 1 000', 1000));
    row.appendChild(goldBtn('Déposer tout', 'dep'));
    row.appendChild(goldBtn('Retirer 100', -100));
    row.appendChild(goldBtn('Retirer 1 000', -1000));
    row.appendChild(goldBtn('Retirer tout', 'wd'));
    root.appendChild(row);

    // Grille 6×4 — items EquipItem avec taille + position RÉELLES (moteur).
    var grid = gridEl(s.w, s.h, CELL);
    grid.style.height = s.h * CELL + 'px';
    for (var y = 0; y < s.h; y++)
      for (var x = 0; x < s.w; x++) {
        var c = el('div', 'pcell');
        c.style.left = x * CELL + 'px';
        c.style.top = y * CELL + 'px';
        grid.appendChild(c);
      }
    (s.items || []).forEach(function (it) {
      var colors = KIND_COLORS.Misc;
      var d = el('div', 'pitem');
      d.style.left = it.x * CELL + 'px';
      d.style.top = it.y * CELL + 'px';
      d.style.width = it.w * CELL - 2 + 'px';
      d.style.height = it.h * CELL - 2 + 'px';
      d.style.background = colors[0];
      d.style.borderColor = colors[1];
      d.title = it.name + ' (' + it.w + '×' + it.h + ') — taille réelle du moteur';
      d.appendChild(el('span', 'pitemname', it.name));
      grid.appendChild(d);
    });
    root.appendChild(grid);
    root.appendChild(el('div', 'foot',
      'Dépôt/retrait : branchement economy.rs RÉEL (bourse débitée/créditée, plafonds ' +
      '300k/500k/800k par difficulté, porté 10 000 × niveau) — save additive `gold_stash`.'));
  }

  // --- framework panneaux ------------------------------------------------------
  function note(m) {
    var s = document.getElementById('msglog');
    if (s) {
      var line = el('div', null, '• ' + m);
      s.appendChild(line);
      while (s.children.length > 12) s.removeChild(s.firstChild);
      s.scrollTop = s.scrollHeight;
    } else if (window.console) console.info('[d2panels]', m);
  }

  function refreshOpen() {
    for (var id in state.panels) {
      var p = state.panels[id];
      if (p.open) render(id);
    }
  }

  function render(id) {
    var p = state.panels[id];
    if (id === 'char') renderChar(p.body);
    else if (id === 'inv') renderInv(p.body);
    else if (id === 'stash') renderStash(p.body);
  }

  function buildPanel(id, title) {
    var root = el('div', 'd2panel');
    root.id = 'd2panel-' + id;
    var bar = el('div', 'pbar');
    bar.appendChild(el('span', 'ptitle', title));
    var close = el('button', 'pclose', '×');
    close.title = 'Fermer (Échap)';
    close.addEventListener('click', function () { togglePanel(id, false); });
    bar.appendChild(close);
    root.appendChild(bar);
    var body = el('div', 'pbody');
    root.appendChild(body);
    document.body.appendChild(root);
    state.panels[id] = { root: root, body: body, open: false };
    return state.panels[id];
  }

  function togglePanel(id, force) {
    var p = state.panels[id] || buildPanel(
      id, id === 'char' ? 'PERSONNAGE' : id === 'inv' ? 'INVENTAIRE' : 'STASH');
    p.open = force !== undefined ? force : !p.open;
    p.root.style.display = p.open ? 'block' : 'none';
    if (p.open) {
      render(id);
      // fermeture mutuelle (D2 : un seul écran plein à la fois)
      for (var other in state.panels)
        if (other !== id && state.panels[other].open) togglePanel(other, false);
    }
  }

  function injectCss() {
    var css = el('style');
    css.textContent =
      // style HUD D2 unique : pierre sombre, bordure or, comme hud.css
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
      // grilles
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
      // paperdoll silhouettes (inventory.txt bodylocs, 9 colonnes × 5 rangées)
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

  function injectButtons() {
    var toolbar = document.getElementById('toolbar');
    if (!toolbar) return;
    [['Perso', 'Panneau personnage (C)', 'char'],
     ['Inventaire', 'Inventaire (I)', 'inv'],
     ['Stash', 'Stash 6×4 + or (B)', 'stash']].forEach(function (d) {
      var b = el('button', null, d[0]);
      b.id = 'btn-panel-' + d[2];
      b.title = d[1];
      b.addEventListener('click', function () { togglePanel(d[2]); });
      toolbar.appendChild(b);
    });
  }

  function onKey(e) {
    if (e.ctrlKey || e.altKey || e.metaKey) return;
    var k = (e.key || '').toLowerCase();
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
      for (var id in state.panels)
        if (state.panels[id].open) { togglePanel(id, false); }
    } // touches 1-4 (ceinture) : laissées à game.js — aucun double usage
  }

  function init() {
    if (state.inited) return;
    state.inited = true;
    try { state.layout = JSON.parse(localStorage.getItem(KEY) || '{}') || {}; }
    catch (e) { state.layout = {}; }
    injectCss();
    injectButtons();
    // CAPTURE + enregistrement au plus tôt : ce module est importé DYNAMIQUEMENT
    // au bas de game.js, donc AVANT que wireCommon() (async, post-wasm) ne
    // pose SES listeners `keydown` — au target (window.dispatchEvent du
    // bouton « Inv » du minipanel) comme en capture (vraies touches),
    // l'ordre d'enregistrement fait foi : nos stopImmediatePropagation
    // gagnent, l'inventaire canvas 1×1 de game.js ne s'ouvre plus.
    window.addEventListener('keydown', onKey, true);
  }

  function initLive() {
    // moteur prêt : rafraîchissement doux des panneaux ouverts (or/vie/XP)
    if (state.live) return;
    state.live = true;
    setInterval(refreshOpen, 1500);
  }

  // Le wasm peut finir de charger APRÈS ce module (import dynamique en bas
  // de game.js) : listeners/CSS/boutons immédiatement, rendu quand prêt.
  init();
  (function whenReady() {
    var g = game();
    if (g && g.isFinalized && g.isFinalized()) initLive();
    else setTimeout(whenReady, 400);
  })();

  return {
    toggle: togglePanel,
    open: function (id) { togglePanel(id, true); },
    refresh: refreshOpen,
    state: state,
  };
})();
