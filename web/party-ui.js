// 3c-b COOP — web/party-ui.js (NOUVEAU).
//
// Panneau latéral des membres du groupe (mission (2)) : portrait/classe/
// niveau, barres vie + mana en TEMPS RÉEL depuis le JSON moteur, icône mort.
// Visible en COOP uniquement (le panneau se masque seul quand le monde n'a
// qu'un héros — solo = zéro DOM visible, zéro coût).
//
// Source de vérité : endpoint wasm `partyView()` (party_bridge.rs) — JSON
// `[{slot, code, class, level, life, maxLife, mana, maxMana, dead, gold}, …]`
// pollé toutes les ~300 ms. La compaction des slots (removePlayer 3c-c) est
// suivie automatiquement : la liste reflète `World.players` à chaque poll.
//
// Contrat :
//   * UN BLOC PAR MEMBRE : portrait (initiale de classe, cadre or, teinte
//     par code de classe), nom de classe + « Niv. X » (libellé manifest du
//     moteur), barre vie (rouge) + barre mana (bleue) avec valeurs, or ;
//     membre mort = rangée grisée + icône ☠ (compte à rebours respawn).
//   * « (vous) » marque le slot LOCAL (window.D2Net.status().slot — lecture
//     seule, aucune écriture dans net-client.js ; repli 0 hors réseau).
//   * Le slot TENEUR d'un dialogue porte un marqueur « discute » (verrou
//     3c-b, lu via window.D2Dialogue.lockView() — défensif si absent).
//
// Défensif (même famille que npc-ui.js) : si le pkg wasm n'expose pas encore
// `partyView()` (build antérieur) ou avant finalize ([]), le module reste
// inerte — jamais d'erreur JS (garde-fou E2E).

window.D2Party = (function () {
  'use strict';
  if (typeof window !== 'undefined' && window.D2Party) return window.D2Party; // anti double-chargement

  var POLL_MS = 300;
  var panel = null, list = null, styleEl = null;
  var iv = null;
  var lastJson = ''; // ne re-rendre QUE sur changement (évite le flicker)

  function game() {
    return (typeof window !== 'undefined' && window.__d2 && window.__d2.game) || null;
  }

  /** Slot local (lecture seule — net-client.js reste la propriété 3c-c). */
  function mySlot() {
    try {
      var st = window.D2Net && typeof window.D2Net.status === 'function' ? window.D2Net.status() : null;
      return st && typeof st.slot === 'number' ? st.slot : 0;
    } catch (e) { return 0; }
  }

  /** Marqueur « discute » : slot teneur du verrou de dialogue 3c-b, sinon -1. */
  function talkerSlot() {
    try {
      var v = window.D2Dialogue && typeof window.D2Dialogue.lockView === 'function'
        ? window.D2Dialogue.lockView() : null;
      return v && v.active && typeof v.slot === 'number' ? v.slot : -1;
    } catch (e) { return -1; }
  }

  // Teinte du portrait par classe (code du moteur).
  var HUES = { BA: 18, SOR: 210, PA: 285, NE: 95, SO: 155, DZ: 45, DR: 335 };
  function hueOf(code) { return HUES[String(code || '').toUpperCase()] != null ? HUES[String(code).toUpperCase()] : 30; }

  function ensureDom() {
    if (panel || typeof document === 'undefined') return;
    var stage = document.getElementById('stage') || document.body;
    panel = document.createElement('div');
    panel.id = 'd2party';
    panel.style.display = 'none';
    panel.innerHTML = '<div class="d2party-title">GROUPE</div><div class="d2party-list"></div>';
    stage.appendChild(panel);
    list = panel.querySelector('.d2party-list');
    styleEl = document.createElement('style');
    styleEl.textContent =
      '#d2party{position:absolute;right:8px;top:56px;width:168px;z-index:5;' +
      'background:rgba(12,9,4,.88);border:2px solid #5a4a2a;outline:1px solid #000;' +
      'box-shadow:0 4px 18px #000d,inset 0 0 0 1px #2e2718;padding:6px 7px 8px;pointer-events:none;}' +
      '#d2party .d2party-title{text-align:center;color:#c7a45a;font:700 10px system-ui;' +
      'letter-spacing:.28em;text-transform:uppercase;text-shadow:0 1px 2px #000;' +
      'border-bottom:1px solid #3a3226;padding-bottom:4px;margin-bottom:6px;}' +
      '#d2party .d2party-m{display:flex;gap:7px;align-items:center;padding:5px 2px;}' +
      '#d2party .d2party-m + .d2party-m{border-top:1px solid #2e2718;}' +
      '#d2party .d2party-m.dead{filter:grayscale(1) brightness(.6);}' +
      '#d2party .d2party-portrait{position:relative;width:34px;height:34px;flex:0 0 34px;' +
      'border:2px solid #5a4a2a;outline:1px solid #000;display:flex;align-items:center;' +
      'justify-content:center;font:700 16px system-ui;color:#fff;text-shadow:0 1px 2px #000;}' +
      '#d2party .d2party-skull{position:absolute;right:-7px;top:-7px;font:700 13px system-ui;' +
      'color:#ff5544;text-shadow:0 1px 2px #000;}' +
      '#d2party .d2party-info{flex:1;min-width:0;}' +
      '#d2party .d2party-name{color:#e8dcbf;font:600 11px system-ui;text-shadow:0 1px 2px #000;' +
      'white-space:nowrap;overflow:hidden;text-overflow:ellipsis;}' +
      '#d2party .d2party-you{color:#8f8676;font-weight:400;}' +
      '#d2party .d2party-talking{color:#ffd98a;font-weight:400;}' +
      '#d2party .d2party-bar{position:relative;height:7px;margin-top:3px;background:#1a140b;' +
      'border:1px solid #000;box-shadow:inset 0 0 0 1px #2e2718;}' +
      '#d2party .d2party-fill{position:absolute;left:0;top:0;bottom:0;}' +
      '#d2party .d2party-hp .d2party-fill{background:linear-gradient(#c0392b,#7e1f14);}' +
      '#d2party .d2party-mp .d2party-fill{background:linear-gradient(#2f6fb5,#1b3f6e);}' +
      '#d2party .d2party-vals{display:flex;justify-content:space-between;color:#b9ae95;' +
      'font:9px system-ui;margin-top:1px;text-shadow:0 1px 2px #000;}' +
      '#d2party .d2party-gold{color:#c7a45a;}';
    document.head.appendChild(styleEl);
  }

  function memberHtml(m, mine, talking) {
    var h = String(m.class || m.code || '?');
    var pctHp = m.maxLife > 0 ? Math.max(0, Math.min(100, (m.life / m.maxLife) * 100)) : 0;
    var pctMp = m.maxMana > 0 ? Math.max(0, Math.min(100, (m.mana / m.maxMana) * 100)) : 0;
    return (
      '<div class="d2party-m' + (m.dead ? ' dead' : '') + '">' +
      '<div class="d2party-portrait" style="background:hsl(' + hueOf(m.code) + ',42%,26%)">' +
      String(h).charAt(0).toUpperCase() +
      (m.dead ? '<span class="d2party-skull">☠</span>' : '') +
      '</div>' +
      '<div class="d2party-info">' +
      '<div class="d2party-name">' + h + ' · Niv. ' + (m.level != null ? m.level : 1) +
      (mine ? ' <span class="d2party-you">(vous)</span>' : '') +
      (talking ? ' <span class="d2party-talking">· discute</span>' : '') +
      '</div>' +
      '<div class="d2party-bar d2party-hp"><div class="d2party-fill" style="width:' + pctHp + '%"></div></div>' +
      '<div class="d2party-bar d2party-mp"><div class="d2party-fill" style="width:' + pctMp + '%"></div></div>' +
      '<div class="d2party-vals"><span>' + (m.life | 0) + '/' + (m.maxLife | 0) +
      ' · ' + (m.mana | 0) + '/' + (m.maxMana | 0) + '</span>' +
      '<span class="d2party-gold">' + (m.gold | 0) + ' or</span></div>' +
      '</div></div>'
    );
  }

  function render() {
    var g = game();
    var members = null;
    if (g && typeof g.partyView === 'function') {
      try { members = JSON.parse(g.partyView()); } catch (e) { members = null; }
    }
    // Coop uniquement : pas de monde, pas d'endpoint, solo (≤1) → masqué.
    if (!members || members.length <= 1) {
      if (panel) panel.style.display = 'none';
      lastJson = '';
      return;
    }
    ensureDom();
    var j = JSON.stringify(members) + '|' + mySlot() + '|' + talkerSlot();
    if (j === lastJson) return; // rien de neuf : pas de re-rendu (temps réel sans flicker)
    lastJson = j;
    var mine = mySlot();
    var talking = talkerSlot();
    var html = '';
    for (var i = 0; i < members.length; i++) {
      var m = members[i] || {};
      html += memberHtml(m, m.slot === mine, m.slot === talking);
    }
    list.innerHTML = html;
    panel.style.display = 'block';
  }

  function start() {
    if (iv || typeof window === 'undefined') return;
    iv = setInterval(render, POLL_MS);
    render(); // premier rendu immédiat
  }

  // Auto-accroche : le jeu est prêt quand window.__d2.game est finalisé
  // (même contrat que les autres modules UI — aucun hook game.js requis).
  if (typeof window !== 'undefined') {
    var tries = 0;
    var boot = setInterval(function () {
      tries++;
      var g = game();
      if ((g && typeof g.partyView === 'function') || tries > 600) {
        clearInterval(boot);
        start();
      }
    }, 100);
  }

  return { render: render };
})();
