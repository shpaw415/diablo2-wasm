// 3c-b COOP — demo/dialogue-ui.js (port du corps maître 22c, touches multi).
//
// Bandeau de dialogue fidèle D2 en bas d'écran, branché sur le moteur coop
// (d2rules/src/dialogue.rs — port 22a adapté multi-joueurs ; endpoints wasm
// `openDialogueSlot(slot,index)` / `chooseDialogueSlot(slot,i)` /
// `closeDialogueSlot(slot)` / `dialogueSession()` / `dialogueLock()`).
// Les lignes fr RÉELLES viennent du Rust (TBL v1.12 résolues côté moteur) —
// ici : rendu + routage, ZÉRO texte de jeu.
//
// Schéma session (DialogueSession serde du moteur, champ par champ) :
//   { slot, npc_index, npc, npc_name, quest, phase,
//     node: { id, line_keys, lignes:[fr], options:[{ label_key, label,
//             next, cond:{kind:always|quest_done,quest}, action:
//             {kind:say|close|travel|open_shop|hire_mercenary|repair,
//              vendor?} }] },
//     visible: [indices bruts visibles],   ← pont d2game (dialogue_bridge)
//     player, playerName }                 ← teneur (identité classe)
//
// Contrat UI :
//   * PANNEAU (teneur) : bandeau bas (nom du NPC en or, texte fr en blanc,
//     options cliquables — style D2) ; le monde est GELÉ pendant le
//     dialogue — gel MOTEUR GLOBAL (World::tick_inputs ne simule plus rien
//     pour AUCUN slot, décision style retail cf. dialogue.rs § coop) DOUBLÉ
//     d'un gel UI local (tick(0,null) tant que MA session est ouverte) ;
//     Échap = fermer (« partir »).
//   * VERROU (les AUTRES joueurs) : dialogueLock() pollé — quand un AUTRE
//     slot tient une session, bandeau haut « <NPC> discute avec <joueur> »
//     et TOUS les clics/touches jeu sont bloqués (phase capture) : le monde
//     gelé est partagé, personne ne joue pendant une conversation. Le clic
//     sur un NPC est consommé (jamais de bulle W16 par-dessus le verrou) ;
//     le moteur refuse de toute façon (DialogueError::Busy).
//   * BOUTIQUE (mission (3)) : l'action open_shop n'ouvre le panneau
//     Marchands QUE sur le client du TENEUR (lui seul clique les options ;
//     le moteur refuse les autres slots — chooseDialogueSlot → null).
//   * WIRE : clic NPC → wrapper D2Npc.clickAt (posé ici) → tryOpen(index)
//     via openDialogueSlot(MON slot) ; « trop loin » retombe sur l'approche
//     W16 de npc-ui.js (marche puis re-clic). Slot LOCAL = window.D2Net
//     status().slot (LECTURE seule — net-client.js reste 3c-c ; repli 0).
//
// NOTE COOP (documentée) : la session vit dans le monde LOCAL du client
// (comme talkNpc W16, déjà hors lockstep dans ce prototype). La propagation
// inter-clients du verrou passera par le relais (3c-c) — l'UI poll DÉJÀ
// dialogueLock() toutes les 250 ms : dès que l'état sera relayé, le bandeau
// s'allumera chez tous sans changement d'UI.
//
// Défensif (même pattern que npc-ui.js) : si le pkg wasm n'expose pas encore
// les endpoints de dialogue (build antérieur), le module reste inerte — la
// page reste jouable et la bulle W16 de npc-ui.js prend le relais.
//
// Branchement (game.js, 1 ligne) : `import './dialogue-ui.js';` — le module
// importe lui-même party-ui.js (panneau du groupe) et s'auto-accroche au
// moteur dès window.__d2.game est finalisé.

import './party-ui.js'; // 3c-b : panneau des membres (mission (2)) — zéro autre ligne game.js

(function () {
  'use strict';
  if (typeof window !== 'undefined' && window.D2Dialogue) return; // anti double-chargement

  var POLL_MS = 250;
  var hooked = null;   // instance D2Game dont `tick` est wrappé (pause monde locale)
  var realTick = null; // tick d'origine (prototype wasm-bindgen)
  var origClickAt = null; // D2Npc.clickAt d'origine (approche W16 préservée)
  var session = null;  // session moteur parsée {slot,name,lines,options,…}
  var lock = null;     // dernier dialogueLock() {active,npc,npcName,slot,player,playerName}
  var panel = null, elName = null, elText = null, elOpts = null;
  var banner = null, elBanner = null;
  var pollIv = null;

  // --- petits utilitaires (même famille que vendor.js : pas de hooks requis) -
  function game() {
    return (typeof window !== 'undefined' && window.__d2 && window.__d2.game) || null;
  }
  function available(g) {
    return !!g && typeof g.openDialogueSlot === 'function' &&
      typeof g.chooseDialogueSlot === 'function';
  }
  function emsg(e) { return e && e.message ? e.message : String(e); }
  function status(msg) {
    var s = typeof document !== 'undefined' && document.getElementById('status');
    if (s) s.textContent = s.textContent.replace(/ · \d+ fps.*$/, '') + ' — ' + msg;
  }
  function note(msg) {
    if (typeof document === 'undefined') return;
    var log = document.getElementById('msglog');
    if (log) {
      var line = document.createElement('div');
      line.textContent = '• ' + msg;
      log.appendChild(line);
      while (log.children.length > 12) log.removeChild(log.firstChild);
      log.scrollTop = log.scrollHeight;
    }
  }

  /** Slot LOCAL (lecture seule — net-client.js reste la propriété 3c-c). */
  function mySlot() {
    try {
      var st = window.D2Net && typeof window.D2Net.status === 'function' ? window.D2Net.status() : null;
      return st && typeof st.slot === 'number' ? st.slot : 0;
    } catch (e) { return 0; }
  }

  // --- session moteur → modèle UI (DialogueSession serde de dialogue.rs) -----
  function actionOf(a) {
    if (a == null) return null;
    if (typeof a === 'string') return { kind: a.toLowerCase(), arg: null };
    if (typeof a === 'object') {
      return {
        kind: String(a.kind || a.type || '').toLowerCase(),
        arg: a.vendor || a.target || null,
      };
    }
    return null;
  }
  function optOf(o, i) {
    if (o == null) return null;
    if (typeof o === 'string' || typeof o === 'number') {
      return { index: i, text: String(o), action: null };
    }
    return {
      index: i, // INDEX BRUT dans node.options (contrat choose_dialogue)
      text: o.label || o.text || o.label_key || 'Option ' + (i + 1),
      action: actionOf(o.action),
    };
  }
  function parseSession(raw) {
    var s;
    try { s = typeof raw === 'string' ? JSON.parse(raw) : raw; } catch (e) { return null; }
    if (!s || typeof s !== 'object') return null;
    var node = s.node && typeof s.node === 'object' ? s.node : s;
    var opts = node.options || s.options || s.choices || [];
    var options = [];
    for (var i = 0; i < opts.length; i++) {
      var o = optOf(opts[i], i);
      if (o) options.push(o);
    }
    // Options VISIBLES : le pont porte `visible` (indices bruts issus de
    // World::dialogue_visible_options) ; sinon tout le nœud s'affiche — un
    // choix caché serait de toute façon refusé par le moteur (null).
    var vis = s.visible || s.visibleOptions || node.visible || null;
    if (vis && vis.length !== undefined) {
      var keep = {};
      for (var v = 0; v < vis.length; v++) keep[vis[v]] = true;
      options = options.filter(function (o) { return keep[o.index]; });
    }
    var lines = node.lignes || s.lignes || s.lines || s.text || s.dialogue || '';
    if (typeof lines === 'string') lines = lines ? [lines] : [];
    return {
      slot: typeof s.slot === 'number' ? s.slot : 0,
      index: s.npc_index != null ? s.npc_index : (s.index != null ? s.index : null),
      name: s.npc_name || s.name || s.npc || 'PNJ',
      role: s.npc || s.role || '',
      lines: lines.map(String),
      options: options,
      done: !!(s.done || s.closed || s.end),
    };
  }
  function hasContent(s) { return s && (s.lines.length > 0 || s.options.length > 0); }
  function parseLock(raw) {
    try {
      var v = typeof raw === 'string' ? JSON.parse(raw) : raw;
      return v && typeof v === 'object' ? v : { active: false };
    } catch (e) { return { active: false }; }
  }

  /** Verrou tenu par un AUTRE que le client local ? */
  function lockedByOther() {
    return !!(lock && lock.active && lock.slot !== mySlot());
  }

  // --- accroche moteur : wrapper de tick (pause) + wrapers + panneau --------
  function hook() {
    var g = game();
    if (hooked || !g || typeof g.isFinalized !== 'function' || !g.isFinalized()) return false;
    // PAUSE MONDE LOCALE : tant que MA session est ouverte, tick(0, null) —
    // le moteur gèle DÉJÀ tout seul (World::tick_inputs, gel global tous
    // slots) ; ce wrap ne couvre qu'un pkg intermédiaire. Les events en
    // attente (NpcTalked, quêtes…) continuent de couler côté moteur.
    realTick = g.tick;
    g.tick = function (dt, inputJson) {
      if (session) return realTick.call(g, 0, null);
      return realTick.call(g, dt, inputJson);
    };
    // PRIORITÉ DE CLIC : le clic NPC ouvre la SESSION (pas la bulle W16).
    // « trop loin » retombe sur l'approche W16 (npc-ui marche, re-clic).
    if (window.D2Npc && typeof window.D2Npc.clickAt === 'function' && !origClickAt) {
      origClickAt = window.D2Npc.clickAt;
      window.D2Npc.clickAt = function (px, py) {
        if (!available(g)) return origClickAt.call(window.D2Npc, px, py);
        if (session) return true; // panneau ouvert : le monde est en pause
        if (lockedByOther()) { pulseBanner(); return true; } // verrou : consommé
        var opened = tryOpenAt(px, py);
        if (opened != null) return opened; // true = session ouverte (consommé)
        return origClickAt.call(window.D2Npc, px, py); // sinon : W16 inchangé
      };
    }
    hooked = g;
    if (pollIv) { clearInterval(pollIv); pollIv = null; }
    buildDom();
    installGuards();
    pollIv = setInterval(poll, POLL_MS);
    poll();
    return true;
  }

  // Hit-test NPC (même projection que npc-ui.js : [(dx-dy)*16, (dx+dy)*8]
  // ancrée sur le héros de la caméra = MON slot côté client).
  function hitNpcAt(g, px, py) {
    var npcs;
    try { npcs = JSON.parse(g.npcs()); } catch (e) { return null; }
    if (!npcs || !npcs.length || typeof g.heroScreen !== 'function') return null;
    var hs, hw;
    try {
      hs = g.heroScreen();
      hw = g.screenToWorld(hs[0], hs[1]);
    } catch (e) { return null; }
    var best = null, bd = 22; // CLICK_RADIUS_PX = npc-ui (marqueur visé)
    for (var i = 0; i < npcs.length; i++) {
      var n = npcs[i];
      var dx = n.x - hw[0], dy = n.y - hw[1];
      var sx = hs[0] + (dx - dy) * 16, sy = hs[1] + (dx + dy) * 8;
      var d = Math.hypot(px - sx, py - (sy - 10));
      if (d <= bd) { bd = d; best = n; }
    }
    return best ? best.index : null;
  }

  /** Clic canvas → session moteur. `true` session ouverte, `false` consommé
   *  (erreur verrou/mort affichée), `null` = pas un NPC / trop loin (W16). */
  function tryOpenAt(px, py) {
    var g = hooked || game();
    if (!g || !available(g)) return null;
    var ix = hitNpcAt(g, px, py);
    if (ix == null) return null;
    return tryOpen(ix);
  }

  /** Ouvre la session du NPC `index` pour MON slot. Renvoie true (ouverte),
   *  false (clic consommé — verrou/mort), null (trop loin → approche W16). */
  function tryOpen(npcIndex) {
    var g = hooked || game();
    if (!g || !available(g) || session) return null;
    var raw;
    try {
      raw = g.openDialogueSlot(mySlot(), npcIndex);
    } catch (e) {
      var m = emsg(e);
      if (m.indexOf('trop loin') >= 0) return null; // npc-ui approche (W16)
      if (m.indexOf('discute') >= 0) { poll(); pulseBanner(); return false; }
      note('Dialogue : ' + m);
      return false;
    }
    var s = parseSession(raw);
    if (!s || (!hasContent(s) && !s.done)) return null;
    session = s.done ? null : s;
    render();
    if (session) status('Dialogue avec ' + session.name + ' — monde en pause (Échap = fermer).');
    return true;
  }

  function choose(o) {
    var g = hooked || game();
    if (!g || !session) return;
    var raw;
    try { raw = g.chooseDialogueSlot(mySlot(), o.index); } // INDEX BRUT (contrat moteur)
    catch (e) { note('Dialogue : ' + emsg(e)); return; }
    var parsed, okParse = false;
    try { parsed = typeof raw === 'string' ? JSON.parse(raw) : raw; okParse = true; } catch (e) { parsed = null; }
    // `null` moteur = choix refusé (condition non remplie) ou session perdue.
    if (okParse && parsed == null) { poll(); return; }
    // Action produite ?
    var kind = okParse && parsed && typeof parsed === 'object' && !parsed.node ? String(parsed.kind || '') : '';
    if (kind === 'close') {
      session = null;
      render();
      status('Monde repris.');
      return;
    }
    if (kind === 'open_shop') {
      // Mission (3) : la boutique s'ouvre pour LE teneur — ce code ne
      // tourne que sur SON client (lui seul clique ; moteur refuse les
      // autres slots). npc-ui.js openVendorTab : bouton + onglet par nom.
      openVendorTab(parsed.vendor || session.role);
      session = null;
      render();
      status('Monde repris — boutique ouverte.');
      return;
    }
    if (kind === 'travel') {
      note('Voyage : la caravane part (panneau waypoints).');
      session = null;
      render();
      status('Monde repris.');
      return;
    }
    if (kind === 'hire_mercenary' || kind === 'repair') {
      note('Action ' + (kind === 'hire_mercenary' ? "engagement (Kashya)" : 'réparation (Charsi)') + ' : panneau à venir.');
      session = null;
      render();
      status('Monde repris.');
      return;
    }
    // Say (ragots « dites-m'en plus ») : le pont renvoie la session à jour.
    var s = parseSession(raw);
    if (s) session = (!hasContent(s) || s.done) ? null : s;
    else { poll(); return; }
    render();
  }

  /** Fermeture (Échap / bouton) — réservée au teneur côté moteur. */
  function close(fromUi) {
    var g = hooked || game();
    if (!g || !session) return;
    try { g.closeDialogueSlot(mySlot()); } catch (e) { /* session déjà fermée */ }
    session = null;
    render();
    if (fromUi) status('Monde repris.');
  }

  // --- poll : session fermée côté moteur / bandeau verrou --------------------
  var lastBanner = '';
  function poll() {
    var g = hooked || game();
    if (!g || !available(g)) return;
    // Session : fermée côté moteur (partir d'un autre chemin) → sync.
    if (session) {
      var raw = null;
      try { raw = g.dialogueSession(); } catch (e) { raw = null; }
      if (raw == null || raw === 'null') {
        session = null;
        render();
        status('Monde repris.');
      } else {
        lock = parseLock((function () { try { return g.dialogueLock(); } catch (e) { return '{"active":false}'; } })());
        renderBanner();
        return;
      }
    }
    // Verrou : bandeau des NON-teneurs (poll 250 ms — prêt pour le relais).
    var lraw = '{"active":false}';
    try { lraw = g.dialogueLock(); } catch (e) { /* pkg ancien */ }
    lock = parseLock(lraw);
    renderBanner();
    // Si C'EST ma session (ex. re-render après travel), resynchroniser.
    if (lock.active && lock.slot === mySlot() && !session) {
      var sraw = null;
      try { sraw = g.dialogueSession(); } catch (e) { sraw = null; }
      var s = sraw && sraw !== 'null' ? parseSession(sraw) : null;
      if (s && hasContent(s)) { session = s; render(); }
    }
    if (typeof window !== 'undefined' && window.D2Party && typeof window.D2Party.render === 'function') {
      window.D2Party.render(); // marqueur « discute » du panneau groupe
    }
  }

  // --- DOM : panneau (teneur) + bandeau verrou (autres) -----------------------
  function buildDom() {
    if (panel || typeof document === 'undefined') return;
    var stage = document.getElementById('stage') || document.body;
    panel = document.createElement('div');
    panel.id = 'd2dlg';
    panel.innerHTML =
      '<div class="d2dlg-frame">' +
      '<div class="d2dlg-name"></div>' +
      '<div class="d2dlg-text"></div>' +
      '<div class="d2dlg-opts"></div>' +
      '<div class="d2dlg-pause">monde en pause · Échap = fermer</div>' +
      '</div>';
    panel.style.display = 'none';
    stage.appendChild(panel);
    elName = panel.querySelector('.d2dlg-name');
    elText = panel.querySelector('.d2dlg-text');
    elOpts = panel.querySelector('.d2dlg-opts');
    banner = document.createElement('div');
    banner.id = 'd2dlglock';
    banner.style.display = 'none';
    stage.appendChild(banner);
    elBanner = banner;
    var css = document.createElement('style');
    css.textContent =
      '#d2dlg{position:absolute;left:50%;bottom:10px;transform:translateX(-50%);' +
      'width:600px;max-width:92%;z-index:6;pointer-events:auto;}' +
      '#d2dlg .d2dlg-frame{position:relative;background:rgba(16,12,6,.95);' +
      'border:2px solid #5a4a2a;outline:1px solid #000;padding:10px 16px 12px;' +
      'box-shadow:0 4px 18px #000d,inset 0 0 0 1px #2e2718;}' +
      '#d2dlg .d2dlg-name{text-align:center;color:#c7a45a;font:700 13px system-ui;' +
      'letter-spacing:.22em;text-transform:uppercase;text-shadow:0 1px 2px #000;' +
      'border-bottom:1px solid #3a3226;padding-bottom:5px;margin-bottom:7px;}' +
      '#d2dlg .d2dlg-text{color:#ffffff;font:12.5px/1.55 system-ui;text-align:center;' +
      'text-shadow:0 1px 2px #000;}' +
      '#d2dlg .d2dlg-line{display:block;margin:2px 0;}' +
      '#d2dlg .d2dlg-opts{margin-top:8px;text-align:center;}' +
      '#d2dlg .d2dlg-opt{display:block;width:100%;background:none;border:0;' +
      'padding:2px 6px;color:#ffffff;font:12.5px system-ui;cursor:pointer;' +
      'text-shadow:0 1px 2px #000;}' +
      '#d2dlg .d2dlg-opt:hover{text-decoration:underline;color:#ffe9ad;}' +
      '#d2dlg .d2dlg-pause{position:absolute;right:10px;top:8px;color:#8f8676;' +
      'font:italic 10px system-ui;letter-spacing:.04em;}' +
      '#d2dlglock{position:absolute;left:50%;top:12px;transform:translateX(-50%);' +
      'z-index:7;background:rgba(16,12,6,.92);border:2px solid #5a4a2a;' +
      'outline:1px solid #000;padding:6px 18px;color:#ffe9ad;font:600 12.5px system-ui;' +
      'letter-spacing:.05em;text-shadow:0 1px 2px #000;box-shadow:0 4px 18px #000d;' +
      'transition:transform .15s;pointer-events:none;}' +
      '#d2dlglock.pulse{transform:translateX(-50%) scale(1.06);}';
    document.head.appendChild(css);
  }

  function render() {
    if (!panel) return;
    if (!session || !hasContent(session)) {
      panel.style.display = 'none';
      return;
    }
    panel.style.display = 'block';
    elName.textContent = session.name;
    elText.innerHTML = '';
    session.lines.forEach(function (l) {
      String(l).split('\n').forEach(function (part) {
        var d = document.createElement('span');
        d.className = 'd2dlg-line';
        d.textContent = part;
        elText.appendChild(d);
      });
    });
    elOpts.innerHTML = '';
    session.options.forEach(function (o) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'd2dlg-opt';
      b.textContent = o.text;
      b.addEventListener('click', function () { choose(o); });
      elOpts.appendChild(b);
    });
  }

  /** Bandeau verrou : « <NPC> discute avec <joueur> » pour les AUTRES. */
  function renderBanner() {
    if (!elBanner) return;
    var mine = !!(lock && lock.active && lock.slot === mySlot());
    var show = lock && lock.active && !mine; // le teneur a SON panneau
    if (!show) {
      elBanner.style.display = 'none';
      lastBanner = '';
      return;
    }
    var txt = (lock.npcName || lock.npc || 'PNJ') + ' discute avec ' +
      (lock.playerName || lock.player || 'un joueur') +
      ' (J' + ((lock.slot | 0) + 1) + ') — monde en pause';
    if (txt !== lastBanner) {
      elBanner.textContent = txt;
      lastBanner = txt;
    }
    elBanner.style.display = 'block';
  }

  function pulseBanner() {
    if (!elBanner || elBanner.style.display === 'none') {
      note((lock && lock.npcName ? lock.npcName : 'Un PNJ') + ' discute déjà avec ' +
        (lock && lock.playerName ? lock.playerName : 'un joueur') + ' — patientez.');
      return;
    }
    elBanner.classList.add('pulse');
    setTimeout(function () { if (elBanner) elBanner.classList.remove('pulse'); }, 180);
  }

  // Gardes d'input (phase capture sur document → avant game.js/hud.js) :
  // pendant MA session ou un verrou TENU PAR UN AUTRE, AUCUNE touche ni clic
  // ne traverse vers le jeu (monde gelé partagé — décision style retail)
  // sauf les clics DANS le panneau (options) et Échap (fermeture teneur).
  function blockedNow() {
    return !!session || lockedByOther();
  }
  function installGuards() {
    document.addEventListener('keydown', function (e) {
      if (!blockedNow()) return;
      e.stopImmediatePropagation();
      if (e.key === 'Escape' && session) {
        e.preventDefault();
        close(true);
      }
    }, true);
    var swallow = function (e) {
      if (!blockedNow()) return;
      if (panel && panel.contains(e.target)) return; // le panneau reste cliquable
      e.preventDefault();
      e.stopImmediatePropagation();
      if (lockedByOther() && (e.type === 'mousedown' || e.type === 'click')) pulseBanner();
    };
    ['mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu'].forEach(function (t) {
      document.addEventListener(t, swallow, true);
    });
  }

  // Panneau Marchands (même DOM que npc-ui.js openVendorTab — mission (3)).
  function openVendorTab(vendor) {
    var btn = document.getElementById('btn-vendors');
    var p = document.getElementById('d2vendor');
    if (!btn || !p) return;
    if (p.style.display !== 'block') btn.click();
    var want = vendor === 'Gheed' ? 'Gamble (Gheed)' : vendor;
    var tabs = p.querySelectorAll('.tabs button');
    for (var i = 0; i < tabs.length; i++) {
      if (tabs[i].textContent.indexOf(want) === 0) { tabs[i].click(); break; }
    }
  }

  // Auto-accroche : le jeu est prêt quand window.__d2.game est finalisé
  // (même contrat que les autres modules UI — aucun hook game.js requis).
  if (typeof window !== 'undefined') {
    var tries = 0;
    var boot = setInterval(function () {
      tries++;
      if (hook() || tries > 600) clearInterval(boot);
    }, 100);
  }

  // API publique (sondes E2E + party-ui marqueur « discute »).
  window.D2Dialogue = {
    tryOpen: tryOpen,
    close: close,
    lockView: function () { return lock || { active: false }; },
    sessionView: function () { return session; },
  };
})();
