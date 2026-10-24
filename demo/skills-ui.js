// Diablo 2 WASM — panneau de l'arbre de compétences (C, impl-council-d2-finish)
// refondu d2w-fix-ui : UI FIDÈLE À D2 v1.12.
//
// Le moteur porte TOUTE la logique : skillTree() = arbre trié par onglet
// (SkillPage) puis géométrie d'arbre (SkillRow/SkillColumn), avec level,
// reqlevel, prereqs, canLearn (prérequis/reqlevel/plafond 20/points) ;
// learnSkill(code) dépense 1 point avec validation réelle d2rules.
//
// Fidélité D2 (d2w-fix-ui) :
//   - fond par classe = skltree_<cls>_back.dc6 réel (7 classes extraites,
//     demo/assets/ui/sktree_<cls>_tab<1..3>.png, panneaux 320×432 à (400,60)) ;
//   - icônes = feuilles skillicon.dc6 (IconCel<24) ou <classe>skillicon.dc6
//     (IconCel-24) réelles, 48×48 posées sur les alvéoles sculptées
//     (grille D2 mesurée : x=50+(col-1)*60, y=381-(row-1)*61 dans le panneau) ;
//   - onglets I/II/III en dalles à droite (haut = page 3, bas = page 1) ;
//   - clic sur une icône apprenable = dépense 1 point (learnSkill).
(function () {
  'use strict';
  const api = { ready: false, opts: null, panel: null, open: false, page: 1 };

  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"]/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

  // Géométrie D2 (mesurée sur les fonds réels, identique pour les 7 classes) :
  // panneau 320×432 à (400,60) ; centres d'alvéoles en coordonnées panneau.
  const PANEL = { x: 400, y: 60, w: 320, h: 432 };
  const socket = (col, row) => ({ x: 50 + (col - 1) * 60, y: 381 - (row - 1) * 61 });
  // token classe du moteur (skills.txt charclass) → clé de fichier extraite
  // (code héros du manifest : ba/so/pa/am/as/dz/ne).
  const CLS = { bar: 'ba', sor: 'so', pal: 'pa', ama: 'am', ass: 'as', dru: 'dz', nec: 'ne' };
  const TAB_NAMES = {
    bar: ['Guerrier', 'Maîtrises', 'Cri de guerre'], sor: ['Foudre', 'Feu', 'Froid'],
    pal: ['Arts de combat', 'Auras offensives', 'Auras défensives'],
    ama: ['Armes et bouclier', 'Combat à distance', 'Arc et arbalète'],
    ass: ['Arts martiaux', 'Disciplines', 'Pièges'], dru: ['Élémentaire', 'Formes', 'Invocation'],
    nec: ['Malédictions', 'Invocation', 'Poisons et os'],
  };

  function iconStyle(cls, cel) {
    // IconCel < 24 → feuille générique skillicon.dc6 ; ≥ 24 → feuille de
    // classe (cellule IconCel-24) — règle du moteur (game.rs skill_tree_json).
    const generic = cel < 24;
    const sheet = generic ? 'sheet_generic' : `sheet_${CLS[cls] || cls}`;
    const cell = generic ? cel : cel - 24;
    return `background-image:url('assets/ui/${sheet}.png');` +
      `background-position:${-(cell % 8) * 48}px ${-Math.floor(cell / 8) * 48}px;`;
  }

  function render() {
    if (!api.open || !api.panel) return;
    const g = api.opts.getGame && api.opts.getGame();
    if (!g || !g.skillTree) return;
    let t;
    try { t = JSON.parse(g.skillTree()); } catch { return; }
    const cls = t.class || 'bar';
    const key = CLS[cls] || cls;
    api.panel.style.backgroundImage = `url('assets/ui/sktree_${key}_tab${api.page}.png')`;
    api.panel.querySelector('#sk-points').textContent = `Points : ${t.points ?? 0}`;
    // icônes de la page courante, posées sur les alvéoles du fond
    const layer = api.panel.querySelector('#sk-icons');
    layer.textContent = '';
    const tabs = TAB_NAMES[cls] || ['I', 'II', 'III'];
    for (const s of t.skills || []) {
      if ((s.page | 0) !== api.page) continue;
      if (!s.row || !s.column) continue; // 'attack' et skills hors arbre
      const pos = socket(s.column, s.row);
      const d = document.createElement('div');
      d.className = 'skicon ' + (s.level > 0 ? 'owned' : (s.canLearn ? 'can' : 'locked'));
      d.style.cssText = `left:${pos.x - 24}px;top:${pos.y - 24}px;` + iconStyle(cls, s.icon | 0);
      d.title = `${esc(s.name)} — ${tabs[api.page - 1] || ''}\n` +
        `Niveau : ${s.level}/${s.maxlvl ?? 20}` +
        (s.reqlevel > 1 ? ` · requis niv. ${s.reqlevel}` : '') +
        (s.mana ? ` · mana ${s.mana}` : '') +
        (s.canLearn ? '\nCliquer : dépenser 1 point' : (s.level === 0 ? '\n(conditions non remplies)' : ''));
      if (s.canLearn) {
        d.addEventListener('click', (e) => {
          e.stopPropagation();
          const gg = api.opts.getGame && api.opts.getGame();
          if (!gg) return;
          let r;
          try { r = JSON.parse(gg.learnSkill(s.code)); }
          catch (err) { api.opts.flash && api.opts.flash(`Erreur : ${err.message}`); return; }
          if (r.ok) api.opts.flash && api.opts.flash(`${s.code} niveau ${r.level}`);
          else api.opts.flash && api.opts.flash(`Refusé : ${r.error}`);
          render();
        });
      }
      layer.appendChild(d);
    }
    // onglets : page active surlignée
    for (const b of api.panel.querySelectorAll('.sktab')) {
      b.classList.toggle('active', Number(b.dataset.page) === api.page);
    }
  }

  function buildDom() {
    const doc = document;
    api.panel = doc.createElement('div');
    api.panel.id = 'd2-tree';
    api.panel.hidden = true;
    api.panel.innerHTML =
      `<div id="sk-icons"></div>` +
      `<div id="sk-points">Points : 0</div>` +
      `<button class="sktab" data-page="3" title="Onglet III" style="top:110px">III</button>` +
      `<button class="sktab" data-page="2" title="Onglet II" style="top:217px">II</button>` +
      `<button class="sktab" data-page="1" title="Onglet I" style="top:325px">I</button>`;
    // FIX-UI : parent = #stage (position:relative) → coordonnées = espace canvas,
    // comme les globes dessinés et les zones cliquables de ui-d2.js.
    (doc.getElementById('stage') || doc.body).appendChild(api.panel);
    for (const b of api.panel.querySelectorAll('.sktab')) {
      b.addEventListener('click', (e) => {
        e.stopPropagation();
        api.page = Number(b.dataset.page);
        render();
      });
    }
  }

  api.init = function (opts) {
    if (api.ready) return api;
    api.opts = opts;
    buildDom();
    api.toggle = function () {
      api.open = !api.open;
      api.panel.hidden = !api.open;
      if (api.open) render();
    };
    doc_keydown();
    const btn = document.getElementById('btn-skills');
    if (btn) btn.addEventListener('click', () => api.toggle());
    api.ready = true;
    return api;
  };
  function doc_keydown() {
    document.addEventListener('keydown', (e) => {
      if (e.key === 't' || e.key === 'T') { e.preventDefault(); api.toggle(); }
    });
  }

  api.refresh = render;
  window.D2Skills = api;
})();
