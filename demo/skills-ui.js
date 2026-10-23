// Diablo 2 WASM — panneau de l'arbre de compétences (C, impl-council-d2-finish).
//
// Le moteur porte TOUTE la logique : skillTree() = arbre trié par onglet
// (SkillPage) puis géométrie d'arbre (SkillRow/SkillColumn), avec level,
// reqlevel, prereqs, canLearn (prérequis/reqlevel/plafond 20/points) ;
// learnSkill(code) dépense 1 point avec validation réelle d2rules.
//
// Ici : panneau DOM (touche T ou bouton « Skills »), groupé par onglet,
// bouton + par skill apprenable. Placeholder d'icône (feuille skillicon
// générique : cellules ≥ 24 = sheets par classe, hors portée MVP).
//
// Intégration (câblée) :
//   game.html : <button id="btn-skills"> + <script src="./skills-ui.js">
//   game.js   : D2Skills.init({ getGame, flash }) après le boot.
(function () {
  'use strict';
  const api = { ready: false, opts: null, panel: null, open: false };

  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"]/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

  function render() {
    if (!api.open || !api.panel) return;
    const g = api.opts.getGame && api.opts.getGame();
    if (!g || !g.skillTree) return;
    let t;
    try { t = JSON.parse(g.skillTree()); } catch { return; }
    api.panel.querySelector('#sk-points').textContent =
      `Points : ${t.points ?? 0}`;
    const byPage = new Map();
    for (const s of t.skills || []) {
      if (!byPage.has(s.page)) byPage.set(s.page, []);
      byPage.get(s.page).push(s);
    }
    let html = '';
    for (const [page, list] of byPage) {
      html += `<div class="sk-page">${esc(page)}</div>`;
      for (const s of list) {
        const can = !!s.canLearn;
        html += `<div class="sk-row${can ? '' : ' sk-locked'}">` +
          `<span class="sk-name">${esc(s.name)}</span>` +
          `<span class="sk-lvl">niv ${s.level}/${s.maxlvl ?? 20}` +
          (s.reqlevel > 1 ? ` · req ${s.reqlevel}` : '') + `</span>` +
          (can ? `<button class="sk-plus" data-code="${esc(s.code)}" title="Dépenser 1 point">+</button>`
               : `<span class="sk-dot">•</span>`) +
          `</div>`;
      }
    }
    api.panel.querySelector('#sk-list').innerHTML =
      html || '<div class="sk-empty">Aucun skill pour cette classe.</div>';
  }

  api.init = function (opts) {
    if (api.ready) return api;
    api.opts = opts;
    const doc = document;
    api.panel = doc.createElement('div');
    api.panel.id = 'd2-skills';
    api.panel.hidden = true;
    api.panel.innerHTML =
      `<div class="sk-title">COMPÉTENCES</div>` +
      `<div id="sk-points"></div><div id="sk-list"></div>` +
      `<div class="sk-hint">T / bouton : ouvrir · + = dépenser 1 point</div>`;
    doc.body.appendChild(api.panel);
    api.panel.addEventListener('click', (e) => {
      const btn = e.target.closest('button.sk-plus');
      if (!btn) return;
      const g = api.opts.getGame && api.opts.getGame();
      if (!g) return;
      let r;
      try { r = JSON.parse(g.learnSkill(btn.dataset.code)); }
      catch (err) { api.opts.flash && api.opts.flash(`Erreur : ${err.message}`); return; }
      if (r.ok) {
        api.opts.flash && api.opts.flash(`${btn.dataset.code} niveau ${r.level}`);
      } else {
        api.opts.flash && api.opts.flash(`Refusé : ${r.error}`);
      }
      render();
    });
    const toggle = () => { api.open = !api.open; api.panel.hidden = !api.open; if (api.open) render(); };
    doc.addEventListener('keydown', (e) => {
      if (e.key === 't' || e.key === 'T') { e.preventDefault(); toggle(); }
    });
    const btn = doc.getElementById('btn-skills');
    if (btn) btn.addEventListener('click', toggle);
    api.ready = true;
    return api;
  };

  api.refresh = render;
  window.D2Skills = api;
})();
