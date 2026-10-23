// W16 VILLES VIVANTES — NPC du Camp des Rousses (demo/npc-ui.js, NOUVEAU).
//
// Rendu OVERLAY des NPC du moteur (aucun sprite DCC au manifest — marqueurs
// visuels) + bulle de dialogue minimale au clic (nom + rôle + actions) et
// câblage des actions de rôle :
//   Akara   → soin gratuit (fait côté Rust : talkNpc) + panneau Marchands/Akara
//   Charsi  → réparation (MVP : rien à réparer) + panneau Marchands/Charsi
//   Kashya  → résurrection mercenaire (bientôt — API Rust prête)
//   Gheed   → panneau Marchands, onglet Gamble (déjà branché, vendor.js)
//   Warriv  → voyage Lut Gholein (bientôt — API Rust prête)
//   Cain    → dialogue (quête q3 : l'événement NpcTalked part côté Rust)
//
// Le clic NPC est PRIORITAIRE sur le clic-déplacement : game.js appelle
// `D2Npc.clickAt(px, py)` avant de router vers move/attack (1 ligne).
// Détail technique : la projection monde→écran est linéaire
// ([(wx-wy)*16, (wx+wy)*8] + origine) — ancrée sur le héros (heroScreen +
// screenToWorld), tout NPC se place au pixel près sans nouvel appel wasm.
//
// Défensif : si le pkg wasm n'expose pas encore `npcs()`/`talkNpc()` (build
// antérieur), le module se désactive silencieusement — la page reste jouable.

window.D2Npc = (function () {
  'use strict';

  var state = {
    hooks: null,        // { getGame, queue, note, flash, manifest, sfx }
    npcs: [],           // dernier npcs() du moteur [{index,id,name,role,x,y,…}]
    lastRefresh: 0,     // throttle 250 ms
    pending: -1,        // index NPC en approche (le héros marche vers lui)
    bubble: null,       // { index, name, lines[], actions[], until }
    level: null,        // zone courante (window.__d2.level) — ferme la bulle
  };
  var REFRESH_MS = 250;
  var CLICK_RADIUS_PX = 22;   // tolérance de clic autour du marqueur
  var TALK_RANGE = 2.6;       // cellules (moteur : 2.5 + marge d'affichage)
  var MARKER_RANGE = 26;      // cellules : au-delà, marqueur estompé
  var BUBBLE_MS = 6500;

  function note(msg) { if (state.hooks && state.hooks.note) state.hooks.note(msg); }
  function flash(msg) { if (state.hooks && state.hooks.flash) state.hooks.flash(msg); }
  function sfx(manifest, key, vol, rate) {
    if (state.hooks && state.hooks.sfx && manifest && manifest.sfx && manifest.sfx[key]) {
      state.hooks.sfx(manifest.sfx[key], vol || 0.5, rate || 1);
    }
  }

  function init(hooks) {
    state.hooks = hooks;
  }

  function available(game) {
    return game && typeof game.npcs === 'function' && typeof game.talkNpc === 'function';
  }

  function refresh(game) {
    if (!available(game)) { state.npcs = []; return; }
    try { state.npcs = JSON.parse(game.npcs()) || []; }
    catch (e) { state.npcs = []; }
  }

  // --- projection monde→écran (ancrage héros, cf. tête de fichier) -----------
  function heroWorld(game) {
    var hs = game.heroScreen();
    return game.screenToWorld(hs[0], hs[1]);
  }
  function npcScreen(game, wx, wy) {
    var hs = game.heroScreen();
    var hw = heroWorld(game);
    var dx = wx - hw[0], dy = wy - hw[1];
    return [hs[0] + (dx - dy) * 16, hs[1] + (dx + dy) * 8];
  }
  function heroDist(game, wx, wy) {
    var hw = heroWorld(game);
    return Math.hypot(wx - hw[0], wy - hw[1]);
  }

  // --- interaction ------------------------------------------------------------
  function doTalk(index) {
    var game = state.hooks && state.hooks.getGame && state.hooks.getGame();
    if (!available(game)) return;
    var talk;
    try { talk = JSON.parse(game.talkNpc(index)); }
    catch (e) {
      // Err moteur (« trop loin… ») : message, pas d'exception page.
      note('NPC : ' + (e && e.message ? e.message : e));
      state.pending = -1;
      return;
    }
    state.pending = -1;
    state.bubble = {
      index: index,
      name: talk.name,
      lines: [talk.dialogue].concat(talk.healed ? ['Vous êtes guéri !'] : []),
      actions: talk.actions || [],
      until: performance.now() + BUBBLE_MS,
    };
    var manifest = state.hooks && state.hooks.manifest;
    sfx(manifest, 'uiClick', 0.5, 1.15);
    if (talk.healed) flash(talk.name + ' vous soigne — vie et mana pleins.');
    for (var i = 0; i < (talk.quest_rewards || []).length; i++) {
      var r = talk.quest_rewards[i];
      if (r.Item) note('Récompense : ' + r.Item.code + ' remis.');
      else if (r.Unlock) note('Quête : « ' + r.Unlock.flag + ' » débloqué !');
      else if (r.Xp) flash('Récompense : +' + r.Xp.amount + ' xp !');
    }
    // Action de rôle implicite : le marchand du rôle ouvre son onglet.
    var npc = state.npcs.filter(function (n) { return n.index === index; })[0];
    if (npc && npc.vendor) openVendorTab(npc.vendor);
  }

  function openVendorTab(vendor) {
    // Panneau demo/vendor.js : bouton « Marchands » + onglets par nom.
    var btn = document.getElementById('btn-vendors');
    var panel = document.getElementById('d2vendor');
    if (!btn || !panel) return;
    if (panel.style.display !== 'block') btn.click();
    var want = vendor === 'Gheed' ? 'Gamble (Gheed)' : vendor;
    var tabs = panel.querySelectorAll('.tabs button');
    for (var i = 0; i < tabs.length; i++) {
      if (tabs[i].textContent.indexOf(want) === 0) { tabs[i].click(); break; }
    }
  }

  // Clic canvas : consommé (true) si un marqueur NPC est visé.
  function clickAt(px, py) {
    var game = state.hooks && state.hooks.getGame && state.hooks.getGame();
    if (!available(game) || !state.npcs.length) return false;
    var best = null, bd = CLICK_RADIUS_PX;
    for (var i = 0; i < state.npcs.length; i++) {
      var n = state.npcs[i];
      var s = npcScreen(game, n.x, n.y);
      var d = Math.hypot(px - s[0], py - (s[1] - 10));
      if (d <= bd) { bd = d; best = n; }
    }
    if (!best) return false;
    var dist = heroDist(game, best.x, best.y);
    if (dist <= TALK_RANGE) {
      doTalk(best.index);
    } else {
      // Trop loin : le héros marche d'abord (file d'inputs), dialogue auto.
      state.pending = best.index;
      state.hooks.queue.push({ type: 'move', x: Math.floor(best.x), y: Math.floor(best.y) });
      note('Vous vous approchez de ' + best.name + '…');
    }
    return true;
  }

  // --- rendu -------------------------------------------------------------------
  function drawMarkers(ctx, game) {
    for (var i = 0; i < state.npcs.length; i++) {
      var n = state.npcs[i];
      var s = npcScreen(game, n.x, n.y);
      if (s[0] < -40 || s[1] < -60 || s[0] > ctx.canvas.width + 40 || s[1] > ctx.canvas.height + 40) continue;
      var dist = heroDist(game, n.x, n.y);
      var near = dist <= MARKER_RANGE;
      ctx.save();
      ctx.globalAlpha = near ? 1 : 0.45;
      // socle elliptique doré (NPC = or, monstres = rouge côté HUD)
      ctx.strokeStyle = '#c7a45a';
      ctx.fillStyle = 'rgba(199,164,90,0.18)';
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.ellipse(s[0], s[1], 10, 5, 0, 0, Math.PI * 2);
      ctx.fill(); ctx.stroke();
      // pique + tête (marqueur « personne »)
      ctx.beginPath();
      ctx.moveTo(s[0], s[1]);
      ctx.lineTo(s[0], s[1] - 22);
      ctx.stroke();
      ctx.fillStyle = n.role === 'healer' ? '#e8dcbf' : '#c7a45a';
      ctx.beginPath();
      ctx.arc(s[0], s[1] - 27, 5, 0, Math.PI * 2);
      ctx.fill();
      ctx.strokeStyle = '#4a3c22';
      ctx.stroke();
      // anneau d'approche en attente de dialogue
      if (state.pending === n.index) {
        ctx.strokeStyle = 'rgba(255,220,140,0.9)';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.ellipse(s[0], s[1], 14, 7, 0, 0, Math.PI * 2);
        ctx.stroke();
      }
      // nom (visible quand le héros est proche)
      if (near) {
        ctx.font = '600 11px system-ui';
        ctx.textAlign = 'center';
        ctx.fillStyle = '#000000cc';
        ctx.fillText(n.name, s[0] + 1, s[1] - 37);
        ctx.fillStyle = '#e8dcbf';
        ctx.fillText(n.name, s[0], s[1] - 38);
      }
      ctx.restore();
    }
  }

  function drawBubble(ctx) {
    var b = state.bubble;
    if (!b) return;
    var game = state.hooks && state.hooks.getGame && state.hooks.getGame();
    var npc = null;
    for (var i = 0; i < state.npcs.length; i++) if (state.npcs[i].index === b.index) npc = state.npcs[i];
    if (!npc || !game) { state.bubble = null; return; }
    var s = npcScreen(game, npc.x, npc.y);
    var w = 240, pad = 10, lineH = 15;
    var h = pad * 2 + 18 + (b.lines.length + b.actions.length) * lineH;
    var x = Math.min(Math.max(6, s[0] - w / 2), ctx.canvas.width - w - 6);
    var y = Math.max(6, s[1] - 70 - h);
    ctx.save();
    ctx.globalAlpha = 0.96;
    ctx.fillStyle = 'rgba(16,12,18,0.94)';
    ctx.strokeStyle = '#5a4a2a';
    ctx.lineWidth = 2;
    ctx.fillRect(x, y, w, h);
    ctx.strokeRect(x + 1, y + 1, w - 2, h - 2);
    ctx.textAlign = 'left';
    ctx.font = '700 12px system-ui';
    ctx.fillStyle = '#c7a45a';
    ctx.fillText(b.name + ' — ' + roleLabel(npc.role), x + pad, y + pad + 10);
    ctx.font = '12px system-ui';
    var yy = y + pad + 28;
    ctx.fillStyle = '#e8dcbf';
    for (var li = 0; li < b.lines.length; li++) {
      wrapText(ctx, b.lines[li], x + pad, yy, w - pad * 2, lineH);
      yy += lineH;
    }
    ctx.fillStyle = '#a89670';
    for (var ai = 0; ai < b.actions.length; ai++) {
      ctx.fillText('• ' + b.actions[ai], x + pad, yy);
      yy += lineH;
    }
    ctx.restore();
  }

  function roleLabel(role) {
    return { healer: 'Prêtresse', mercenary_captain: 'Capitaine', blacksmith: 'Forgeronne',
             gambler: 'Marchand', caravan: 'Caravane', scholar: 'Horadrim' }[role] || role;
  }

  function wrapText(ctx, text, x, y, maxW, lineH) {
    var words = String(text).split(' ');
    var line = '';
    for (var i = 0; i < words.length; i++) {
      var test = line ? line + ' ' + words[i] : words[i];
      if (ctx.measureText(test).width > maxW && line) {
        ctx.fillText(line, x, y);
        line = words[i];
        y += lineH;
      } else line = test;
    }
    if (line) ctx.fillText(line, x, y);
  }

  // Boucle (appelé chaque frame par game.js — 1 ligne).
  function frame(canvas, game) {
    if (!available(game)) return;
    var now = performance.now();
    // Changement de zone : bulle et approche annulées.
    if (window.__d2 && window.__d2.level !== state.level) {
      state.level = window.__d2.level;
      state.bubble = null;
      state.pending = -1;
      refresh(game);
      state.lastRefresh = now;
    }
    if (now - state.lastRefresh > REFRESH_MS) {
      state.lastRefresh = now;
      refresh(game);
    }
    // Approche terminée : dialogue automatique à portée.
    if (state.pending >= 0) {
      var n = null;
      for (var i = 0; i < state.npcs.length; i++) if (state.npcs[i].index === state.pending) n = state.npcs[i];
      if (n && heroDist(game, n.x, n.y) <= TALK_RANGE) doTalk(state.pending);
    }
    var ctx = canvas.getContext('2d');
    drawMarkers(ctx, game);
    if (state.bubble && now > state.bubble.until) state.bubble = null;
    drawBubble(ctx);
  }

  return {
    init: init,
    frame: frame,
    clickAt: clickAt,
    refresh: refresh,
    _state: state,
  };
})();
