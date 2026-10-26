// Diablo 2 WASM — carte mercenaire (H, impl-council-d2-finish).
//
// Le moteur porte TOUTE la logique (d2rules/mercenary.rs : hireling.txt
// v1.12, 4 types, embauche/KO/résurrection) ; mercInfo() = fiche JSON
// `{kind,subtype,level,hp,maxHp,alive,x,y,sx,sy}` ou `null`.
//
// Ici : carte DOM minimale (nom, niveau, barre de vie, état KO), pollée
// ~2×/s depuis la boucle — même pattern que le HUD/waypoints.
//
// Intégration (câblée) :
//   game.html : <script src="./merc-ui.js">
//   game.js   : D2Merc.init({ getGame }) + D2Merc.frame(game) dans la boucle.
(function () {
  'use strict';
  const api = { ready: false, opts: null, card: null, lastPoll: 0 };

  api.init = function (opts) {
    if (api.ready) return api;
    api.opts = opts;
    const card = document.createElement('div');
    card.id = 'd2-merc';
    card.style.cssText = 'position:absolute; left:10px; top:34px; z-index:25;' +
      'min-width:150px; font:11px monospace; color:#d0b070;' +
      'background:rgba(16,12,6,.85); border:1px solid #5a3c1e; padding:4px 8px; display:none;';
    card.innerHTML =
      `<div id="merc-name">—</div>` +
      `<div style="height:5px; background:#241c10; border:1px solid #5a3c1e; margin-top:3px;">` +
      `<div id="merc-hp" style="height:100%; width:0%; background:#8c2b1e;"></div></div>` +
      `<div id="merc-state" style="color:#7a6a4a;"></div>`;
    document.body.appendChild(card);
    api.card = card;
    api.ready = true;
    return api;
  };

  // Pollé par la boucle (throttle 500 ms côté appelant).
  api.frame = function (game) {
    if (!api.card || !game || !game.mercInfo) return;
    let m = null;
    try { m = JSON.parse(game.mercInfo()); } catch { m = null; }
    if (!m) { api.card.style.display = 'none'; return; }
    api.card.style.display = 'block';
    api.card.querySelector('#merc-name').textContent =
      `${m.kind || 'Mercenaire'} · niv ${m.level ?? '?'}`;
    const pct = m.maxHp > 0 ? Math.round((100 * m.hp) / m.maxHp) : 0;
    const bar = api.card.querySelector('#merc-hp');
    bar.style.width = `${pct}%`;
    bar.style.background = m.alive ? '#8c2b1e' : '#333';
    api.card.querySelector('#merc-state').textContent =
      m.alive ? `${m.hp}/${m.maxHp} PV` : 'KO — résurrection chez le NPC';
  };

  window.D2Merc = api;
})();
