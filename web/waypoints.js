// Diablo 2 WASM — panneau des waypoints (W05/W19) — glue navigateur
// SELF-CONTENUE (même pattern que quests.js).
//
// Le moteur Rust (d2rules/src/waypoints.rs + d2game/src/waypoint_travel.rs)
// porte TOUTE la logique : table réelle v1.12 (patch_levels.txt col.
// `Waypoint` + positions DS1/DRLG), activation au contact (event
// `WaypointActivated`), persistance (champ save additif "waypoints"),
// voyage direct avec régénération seedée du niveau cible.
//
// Ici : panneau DOM (touche Tab ou bouton « Waypoints »), liste des
// waypoints découverts (clic = voyage), section grisée des non-découverts,
// et une sonde E2E `D2Waypoints.e2eTour()` (marche réelle → contact →
// panneau → voyage, cf. web/waypoints-e2e.png).
//
// Intégration (faite, cf. docs/parity/impl-waypoints.md) :
//   <script src="./waypoints.js"></script>       (avant ou après game.js)
//   D2Waypoints.init({ getGame, queue, note, flash, onTraveled });
//   D2Waypoints.refresh(game.waypoints());       // ~2×/s dans la boucle
//
// Libellés FR provisoires (TBL FR absentes du port — W18) : noms de
// niveaux EN canoniques de patch_levels.txt, en-têtes de panneau FR maison.
(function () {
  'use strict';

  const api = {
    ready: false,
    open: false,
    opts: null,
    state: null, // dernier JSON game.waypoints() parsé
    panel: null,
  };

  // ------------------------------------------------------------------
  // Rendu du panneau
  // ------------------------------------------------------------------
  function esc(s) {
    return String(s).replace(/[&<>"']/g, (c) => (
      { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]
    ));
  }

  function render() {
    if (!api.panel || !api.state) return;
    const list = (api.state.list || []).slice().sort((a, b) =>
      a.act - b.act || a.slot - b.slot);
    const found = list.filter((w) => w.activated);
    const cur = api.state.current || {};
    // --- découverts : boutons de voyage ---
    const rows = found.map((w) => {
      const tags = [
        w.here ? '<span class="wp-here">● ici</span>' : '',
        w.mvp ? '<span class="wp-mvp" title="Extension MVP : la table retail (patch_levels.txt) ne mappe pas ce niveau à un slot (255) — cf. impl-waypoints.md">MVP</span>' : '',
      ].join(' ');
      const travel = w.travel && !w.here;
      return `<button class="wp-row${travel ? '' : ' wp-cur'}" data-slot="${w.slot}"` +
        `${travel ? '' : ' disabled'}>` +
        `<span class="wp-name">${esc(w.name)}</span>${tags}</button>`;
    });
    // --- non découverts : liste grisée (table réelle complète) ---
    const todo = list.filter((w) => !w.activated);
    const todoHtml = todo.length
      ? `<div class="wp-locked">${todo.map((w) =>
          `<div class="wp-locked-row">${esc(w.name)}</div>`).join('')}</div>`
      : '';
    api.panel.querySelector('#wp-list').innerHTML =
      (rows.length ? rows.join('') : '<div class="wp-empty">Aucun waypoint découvert — marchez jusqu’à une pierre.</div>') +
      (todoHtml ? `<div class="wp-locked-title">${todo.length} non découvert${todo.length > 1 ? 's' : ''}</div>${todoHtml}` : '');
    api.panel.querySelector('#wp-zone').textContent =
      cur.name ? `Zone : ${cur.name}` : '';
  }

  // ------------------------------------------------------------------
  // Voyage
  // ------------------------------------------------------------------
  async function travel(slot) {
    const g = api.opts.getGame && api.opts.getGame();
    if (!g) return;
    try {
      const out = JSON.parse(g.travelWaypoint(slot));
      api.opts.note && api.opts.note(`Voyage : ${out.name}.`);
      api.opts.onTraveled && api.opts.onTraveled(out);
      sfxUi(0.9);
    } catch (e) {
      api.opts.note && api.opts.note(`Voyage impossible : ${e && e.message ? e.message : e}`);
    }
  }

  function sfxUi(rate) {
    try {
      const m = api.opts.manifest;
      if (m && m.sfx && api.opts.sfx) api.opts.sfx(m.sfx.uiClick, 0.5, rate);
    } catch { /* son indisponible : jamais bloquant */ }
  }

  // ------------------------------------------------------------------
  // API publique
  // ------------------------------------------------------------------
  api.init = function (opts) {
    if (api.ready) return api;
    api.opts = opts;
    const doc = document;
    api.panel = doc.createElement('div');
    api.panel.id = 'd2-waypoints';
    api.panel.hidden = true;
    api.panel.innerHTML =
      `<div class="wp-title">WAYPOINTS</div>` +
      `<div class="wp-zone" id="wp-zone"></div>` +
      `<div id="wp-list"></div>` +
      `<div class="wp-hint">Tab / bouton : ouvrir · clic = voyager (niveau régénéré seedé)</div>`;
    doc.body.appendChild(api.panel);
    api.panel.addEventListener('click', (e) => {
      const btn = e.target.closest('button.wp-row');
      if (btn && !btn.disabled) travel(Number(btn.dataset.slot));
    });
    // Tab : ouvrir/fermer (le panneau reste ouvert pendant les voyages).
    doc.addEventListener('keydown', (e) => {
      if (e.key === 'Tab') {
        e.preventDefault();
        api.toggle();
      }
    });
    const btn = doc.getElementById('btn-waypoints');
    if (btn) btn.addEventListener('click', () => api.toggle());
    api.ready = true;
    // Premier rendu si le moteur est déjà prêt.
    const g = opts.getGame && opts.getGame();
    if (g && g.waypoints) api.refresh(g.waypoints());
    return api;
  };

  api.toggle = function () {
    api.setOpen(!api.open);
  };

  api.setOpen = function (open) {
    api.open = !!open;
    if (api.panel) api.panel.hidden = !api.open;
    sfxUi(api.open ? 1.3 : 1.0);
  };

  // Rafraîchit l'état (JSON de game.waypoints()) — ~2×/s depuis la boucle.
  api.refresh = function (jsonText) {
    try {
      api.state = JSON.parse(jsonText);
      render();
    } catch { /* JSON partiel : on garde le dernier état */ }
  };

  // Nom d'un slot (pour les messages d'événements) — dernier état connu.
  api.nameOf = function (slot) {
    const w = api.state && (api.state.list || []).find((x) => x.slot === slot);
    return w ? w.name : `slot ${slot}`;
  };

  // ------------------------------------------------------------------
  // Sonde E2E (web/waypoints-e2e.png) : vraie marche → contact → panneau →
  // voyage. Polling sur la surface publique (waypoints()/travelWaypoint()).
  // ------------------------------------------------------------------
  api.e2eTour = async function () {
    const g = api.opts.getGame && api.opts.getGame();
    const q = api.opts.queue;
    if (!g || !q) throw new Error('E2E: moteur ou queue absent');
    const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
    const activated = (slot) => {
      api.refresh(g.waypoints());
      const w = (api.state.list || []).find((x) => x.slot === slot);
      return !!(w && w.activated);
    };
    const walkUntil = async (slot, deadlineMs) => {
      const t0 = Date.now();
      while (Date.now() - t0 < deadlineMs) {
        if (activated(slot)) return true;
        q.push({ type: 'move', x: Math.round(api.state.list.find((x) => x.slot === slot).pos[0]),
                 y: Math.round(api.state.list.find((x) => x.slot === slot).pos[1]) });
        await sleep(400);
      }
      return false;
    };
    // 1) pierre du Camp des Rousses (objet TownN1.ds1 (144,51)) au contact.
    if (!await walkUntil(0, 25000)) throw new Error('E2E: waypoint camp non activé');
    api.opts.flash && api.opts.flash(`Waypoint découvert : ${api.nameOf(0)}`);
    // 2) passage à la Blood Moor par l'entrée du warp (téléport debug = __d2.goto).
    g.switchLevel('bloodmoor', 137, 162);
    await sleep(300);
    // 3) pierre de la Blood Moor (chunk DRLG (27,27) → centre (152,152)).
    if (!await walkUntil(39, 25000)) throw new Error('E2E: waypoint bloodmoor non activé');
    api.opts.flash && api.opts.flash(`Waypoint découvert : ${api.nameOf(39)}`);
    // 4) panneau + voyage retour vers le camp (régénération seedée côté Rust).
    api.setOpen(true);
    await sleep(400);
    await travel(0);
    await sleep(600);
    api.setOpen(true); // panneau ouvert pour la capture
    api.refresh(g.waypoints());
    return api.state;
  };

  window.D2Waypoints = api;
})();
