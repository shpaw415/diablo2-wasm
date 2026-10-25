// Diablo 2 WASM — arbre de compétences FIDÈLE (18a, PLAN-FIDELITE phase 18).
//
// Le moteur porte TOUTE la logique : skillTree() = arbre enrichi (18a :
// nameFr/descFr TBL fr, dégâts réels courbes+synergies, tabs fr, DC6 par
// classe), learnSkill(code) dépense 1 point avec validation réelle d2rules,
// skillIconsPng(dc6, pal) → PNG strip 48×48/cellule (feuille par classe),
// skillTreePanelPng(dc6, tab, pal) → PNG 320×432 composé (tuiles communes +
// overlay onglet + pile d'onglets, cf. crates/d2game §18a).
//
// Ici : panneau DOM qui reproduit l'écran D2 640×480 —
//   * fond d'arbre DC6 par classe (skltree_<c>_back.dc6, 3 onglets réels) ;
//   * icônes DC6 48×48 par skill (feuille <c>skillicon.dc6, index IconCel) ;
//   * grille réelle (colonne 0-3 × rangée 1-6, pas 69×68 — géométrie D2) ;
//   * lignes de prérequis (SVG) + points dépensés affichés ;
//   * onglets cliquables (pile verticale droite, page 1 en bas comme D2) ;
//   * tooltip survol : nom fr, description fr, dégâts ACTUELS calculés par
//     le moteur (ex. « 6-12 feu ») + synergies (« +14 %/niv Trait de feu »)
//     + mana + niveau visé.
//
// Intégration (inchangée) : game.html <script src="./skills-ui.js"> ;
// game.js : D2Skills.init({ getGame, flash, note }) après le boot.
// (Ce fichier s'injecte son CSS : aucune autre ressource nécessaire.)
(function () {
  'use strict';

  const api = { ready: false, opts: null, open: false };
  const ROOT = new URL('..', location); // racine projet (comme game.js)

  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"]/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

  // --- géométrie D2 (écran 640×480) ----------------------------------------
  // Grille : x = 265 + SkillColumn×69, y = 27 + SkillRow×68 (icône 48×48
  // posée à x,y comme OpenDiablo2 skillIconXOff/YOff 346/59 sur son
  // 800×600 — ramené ici à l'écran 640×480 du panneau à droite).
  const PANEL_X = 320, PANEL_Y = 32;          // panneau 320×432 à droite
  const GRID_X0 = 265, GRID_Y0 = 27, DX = 69, DY = 68, ICON = 48;
  // Pile d'onglets : boutons 93×107 sur la colonne droite du panneau,
  // page 1 EN BAS (comme l'écran D2) — Onglets : p3 haut, p2 milieu, p1 bas.
  const TAB_W = 93;
  const TAB_Y = { 3: 138, 2: 245, 1: 352 };   // y (écran) du HAUT de chaque zone

  let state = { tab: 1, tree: null, iconsUrl: null, bgUrls: {}, palBytes: null };

  // --- CSS injecté (le panneau se suffit à lui-même) ------------------------
  function injectStyle() {
    if (document.getElementById('d2-skills-style')) return;
    const st = document.createElement('style');
    st.id = 'd2-skills-style';
    st.textContent = `
#d2-skills { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%);
  width: 640px; height: 480px; max-height: none; overflow: visible; padding: 0;
  z-index: 45; background: rgba(6, 4, 2, .88);
  border: 1px solid #5a3c1e; box-shadow: 0 0 24px #000; font: 12px/1.35 serif;
  color: #e8dcbf; user-select: none; image-rendering: pixelated; }
#d2-skills * { box-sizing: content-box; }
#d2-skills .skt-bg { position: absolute; left: ${PANEL_X}px; top: ${PANEL_Y}px;
  width: 320px; height: 432px; background-size: 320px 432px; }
#d2-skills .skt-tabzone { position: absolute; width: ${TAB_W}px; height: 107px;
  cursor: pointer; z-index: 6; }
#d2-skills svg.skt-lines { position: absolute; left: 0; top: 0; width: 640px; height: 480px;
  pointer-events: none; z-index: 2; }
#d2-skills .skt-icon { position: absolute; width: ${ICON}px; height: ${ICON}px;
  cursor: pointer; z-index: 4; background-repeat: no-repeat;
  image-rendering: pixelated; }
#d2-skills .skt-icon.locked { filter: grayscale(1) brightness(.5); cursor: default; }
#d2-skills .skt-icon.unspent:hover { outline: 1px solid #e8dcbf; filter: brightness(1.15); }
#d2-skills .skt-lvl { position: absolute; left: 49px; top: -5px; font: bold 11px monospace;
  color: #fff; text-shadow: 1px 1px 0 #000; z-index: 5; }
#d2-skills .skt-points { position: absolute; left: 8px; top: 8px; color: #d9b96a;
  z-index: 6; font-weight: bold; }
#d2-skills .skt-heroname { position: absolute; left: 8px; top: 24px; color: #a89670; z-index: 6; }
#d2-skills .skt-tip { position: absolute; width: 240px; max-height: 300px; overflow-y: auto;
  background: rgba(10, 7, 3, .96); border: 1px solid #6a5228; padding: 8px 10px;
  z-index: 20; pointer-events: none; font-size: 11px; }
#d2-skills .skt-tip h4 { margin: 0 0 2px; color: #d9b96a; font-size: 13px; }
#d2-skills .skt-tip .t-desc { color: #8f9fd0; white-space: pre-line; margin-bottom: 4px; }
#d2-skills .skt-tip .t-dmg { color: #e8dcbf; }
#d2-skills .skt-tip .t-syn { color: #9ab09a; }
#d2-skills .skt-tip .t-meta { color: #a89670; margin-top: 4px; }
#d2-skills .skt-tip .t-lock { color: #b05a4a; margin-top: 4px; }
#d2-skills .skt-close { position: absolute; right: 6px; top: 6px; z-index: 7; cursor: pointer;
  background: #241c10; color: #e8dcbf; border: 1px solid #5a3c1e; width: 22px; height: 22px; }
#d2-skills .skt-hint { position: absolute; left: 8px; bottom: 6px; color: #7a6a4a;
  font-size: 10px; z-index: 6; }
#d2-skills .skt-err { position: absolute; left: 8px; top: 44px; color: #b05a4a; z-index: 6; }
`;
    document.head.appendChild(st);
  }

  // --- fetch helpers ---------------------------------------------------------
  async function fetchBytes(rel) {
    const r = await fetch(new URL(rel, ROOT));
    if (!r.ok) throw new Error(`fetch ${rel} → HTTP ${r.status}`);
    return new Uint8Array(await r.arrayBuffer());
  }

  function pngUrl(bytes) {
    const blob = new Blob([bytes], { type: 'image/png' });
    return URL.createObjectURL(blob);
  }

  // --- chargements DC6 (une fois par classe) ---------------------------------
  async function loadAssets(g, tree) {
    const dc6 = async (rel) => await fetchBytes(rel);
    if (!state.palBytes) state.palBytes = await dc6('assets/pal_act1.dat');
    const cls = tree.class;
    state.err = '';
    if (state.loadedClass !== cls) {
      state.loadedClass = cls;
      state.iconsUrl = null;
      state.bgUrls = {};
      try {
        const sheet = await dc6(tree.iconFile);
        const png = g.skillIconsPng(sheet, state.palBytes);
        if (png && png.length) state.iconsUrl = pngUrl(png);
      } catch (e) { state.err = String(e.message || e); }
      try {
        const bg = await dc6(tree.bgFile);
        for (const tab of [0, 1, 2]) {
          const p = g.skillTreePanelPng(bg, tab, state.palBytes);
          if (p && p.length) state.bgUrls[tab] = pngUrl(p);
        }
      } catch (e) { state.err = String(e.message || e); }
    }
  }

  // --- tooltip ---------------------------------------------------------------
  function dmgText(s) {
    const parts = [];
    const [pn, px] = s.dmgPhys || [0, 0];
    if (px > 0) parts.push(`${pn}-${px} dégâts`);
    const [en, ex] = s.dmgElem || [0, 0];
    if (ex > 0) {
      const el = s.elem ? ` ${s.elem}` : '';
      parts.push(`${en}-${ex}${el}`);
      if (s.elemLen > 0) parts.push(`${Math.round(s.elemLen / 25)}s`);
    }
    if (!parts.length && s.srcDam > 0) {
      parts.push(`${Math.round(s.srcDam * 100 / 256)} % des dégâts de l'arme`);
    }
    return parts.join(', ');
  }

  function tooltipHtml(s, tree) {
    const lvl = s.level || 0;
    let h = `<h4>${esc(s.nameFr || s.code)}</h4>`;
    if (s.descFr) h += `<div class="t-desc">${esc(s.descFr).replace(/\\n/g, '\n')}</div>`;
    const cur = dmgText(s);
    if (cur) h += `<div class="t-dmg">dégâts : ${esc(cur)}${lvl ? ` (niv ${lvl})` : ' (niv 1 prévu)'}</div>`;
    if (s.mana > 0) h += `<div class="t-dmg">mana : ${s.mana}</div>`;
    const syn = (s.synergies || []).filter((x) => x.pctPerLvl || x.pctTotal);
    for (const x of syn) {
      h += `<div class="t-syn">+${x.pctPerLvl} %/niv ${esc(x.nameFr || x.code)}` +
        (x.pctTotal ? ` — total +${x.pctTotal} %` : '') + '</div>';
    }
    if (s.dmgElemNext || s.dmgPhysNext) {
      const nx = dmgText({ dmgPhys: s.dmgPhysNext, dmgElem: s.dmgElemNext, elem: s.elem, elemLen: 0, srcDam: 0 });
      if (nx) h += `<div class="t-meta">niv ${lvl + 1} : ${esc(nx)}</div>`;
    }
    if (s.reqLevel > 1 && tree.charLevel < s.reqLevel) {
      h += `<div class="t-lock">Niveau ${s.reqLevel} requis (héros niv ${tree.charLevel})</div>`;
    }
    for (const p of s.prereqs || []) {
      const known = (tree.skills || []).some((k) => k.code === p && k.level > 0);
      if (!known) h += `<div class="t-lock">Requiert : ${esc(p)}</div>`;
    }
    return h;
  }

  // --- rendu ------------------------------------------------------------------
  function iconXY(s) {
    return [GRID_X0 + (s.column || 0) * DX, GRID_Y0 + (s.row || 0) * DY];
  }

  function render() {
    if (!api.open || !api.panel) return;
    const g = api.opts.getGame && api.opts.getGame();
    if (!g || !g.skillTree) return;
    let t;
    try { t = JSON.parse(g.skillTree()); } catch { return; }
    state.tree = t;

    loadAssets(g, t).then(() => renderInto(t)).catch((e) => {
      state.err = String(e && e.message || e);
      renderInto(t);
    });
  }

  function renderInto(t) {
    const root = api.panel;
    const bg = state.bgUrls[state.tab - 1];
    const bgEl = root.querySelector('.skt-bg');
    if (bg) bgEl.style.backgroundImage = `url(${bg})`;
    root.querySelector('.skt-points').textContent =
      `Points : ${t.points ?? 0}`;
    root.querySelector('.skt-heroname').textContent =
      `${t.hero ?? ''} — ${t.tabs ? t.tabs[state.tab - 1] || '' : ''}`;
    const errEl = root.querySelector('.skt-err');
    errEl.textContent = state.err || (state.iconsUrl ? '' :
      (state.loadedClass ? 'icônes DC6 indisponibles (placeholder)' : 'chargement…'));

    const skills = (t.skills || []).filter((s) => (s.page || 0) === state.tab);
    // --- lignes de prérequis (même onglet, gris D2) ---
    let lines = '';
    const pos = new Map();
    for (const s of skills) pos.set(s.code, iconXY(s));
    for (const s of skills) {
      for (const p of s.prereqs || []) {
        const a = pos.get(p);
        if (!a) continue;
        const [x1, y1] = a, [x2, y2] = iconXY(s);
        lines += `<line x1="${x1 + ICON / 2}" y1="${y1 + ICON / 2}"` +
          ` x2="${x2 + ICON / 2}" y2="${y2 + ICON / 2}"` +
          ` stroke="rgba(210,190,150,.5)" stroke-width="2"/>`;
      }
    }
    root.querySelector('svg.skt-lines').innerHTML = lines;
    // --- icônes ---
    let html = '';
    for (const s of skills) {
      const [x, y] = iconXY(s);
      const known = (s.level || 0) > 0;
      const learnable = !!s.canLearn;
      const cls = 'skt-icon' + (known ? '' : ' locked') + (learnable ? ' unspent' : '');
      const cel = Math.max(0, s.icon || 0);
      let style = `left:${x}px;top:${y}px;`;
      if (state.iconsUrl) {
        style += `background-image:url(${state.iconsUrl});background-position:-${cel * ICON}px 0;`;
      }
      html += `<div class="${cls}" data-code="${esc(s.code)}" style="${style}">`;
      if (state.iconsUrl && !known) {
        // unlearned : la feuille est colorée → on désature via un filtre CSS
        // appliqué à l'élément (classe locked).
      }
      if (known) html += `<span class="skt-lvl">${s.level}</span>`;
      html += '</div>';
    }
    const holder = root.querySelector('.skt-icons');
    holder.innerHTML = html;
    for (const el of holder.querySelectorAll('.skt-icon')) {
      el.addEventListener('mouseenter', () => showTip(el));
      el.addEventListener('mouseleave', hideTip);
      el.addEventListener('click', () => learn(el.dataset.code));
    }
  }

  function showTip(el) {
    const t = state.tree;
    if (!t) return;
    const s = (t.skills || []).find((k) => k.code === el.dataset.code);
    if (!s) return;
    const tip = api.panel.querySelector('.skt-tip');
    tip.innerHTML = tooltipHtml(s, t);
    tip.style.display = 'block';
    const [x, y] = iconXY(s);
    // À GAUCHE de l'icône si la place le permet (le panneau est à droite).
    const tw = 240;
    tip.style.left = (x - tw - 8 >= 4 ? x - tw - 8 : x + ICON + 8) + 'px';
    tip.style.top = Math.max(4, Math.min(480 - tip.offsetHeight - 4, y - 8)) + 'px';
  }
  function hideTip() {
    const tip = api.panel.querySelector('.skt-tip');
    if (tip) tip.style.display = 'none';
  }

  function learn(code) {
    const g = api.opts.getGame && api.opts.getGame();
    if (!g || !code) return;
    const t = state.tree;
    const s = t && (t.skills || []).find((k) => k.code === code);
    if (s && !(s.level > 0) && !s.canLearn) return; // verrouillé : pas de requête
    let r;
    try { r = JSON.parse(g.learnSkill(code)); } catch (err) {
      api.opts.flash && api.opts.flash(`Erreur : ${err.message}`); return;
    }
    if (r.ok) {
      const nf = s && (s.nameFr || s.code);
      api.opts.flash && api.opts.flash(`${nf} niveau ${r.level}`);
      api.opts.note && api.opts.note(`${nf} niveau ${r.level}`);
    } else {
      api.opts.flash && api.opts.flash(`Refusé : ${r.error}`);
    }
    render();
  }

  // --- panneau ----------------------------------------------------------------
  function buildPanel() {
    api.panel = document.createElement('div');
    api.panel.id = 'd2-skills';
    api.panel.hidden = true;
    api.panel.innerHTML =
      `<div class="skt-bg"></div>` +
      `<svg class="skt-lines"></svg>` +
      `<div class="skt-icons"></div>` +
      `<div class="skt-points"></div>` +
      `<div class="skt-heroname"></div>` +
      `<div class="skt-err"></div>` +
      `<button class="skt-close" title="Fermer (T)">✕</button>` +
      `<div class="skt-hint">T : fermer · clic = dépenser 1 point · survol = détails</div>` +
      `<div class="skt-tip" style="display:none"></div>`;
    // zones d'onglets cliquables (page 1 bas, 2 milieu, 3 haut).
    for (const page of [1, 2, 3]) {
      const z = document.createElement('div');
      z.className = 'skt-tabzone';
      z.style.left = (PANEL_X + 320 - TAB_W) + 'px';
      z.style.top = TAB_Y[page] + 'px';
      z.title = 'Onglet ' + page;
      z.addEventListener('click', () => { state.tab = page; render(); });
      api.panel.appendChild(z);
    }
    document.body.appendChild(api.panel);
    api.panel.querySelector('.skt-close').addEventListener('click', toggle);
  }

  function toggle() {
    api.open = !api.open;
    api.panel.hidden = !api.open;
    if (api.open) render();
    else hideTip();
  }

  api.init = function (opts) {
    if (api.ready) return api;
    api.opts = opts;
    injectStyle();
    buildPanel();
    document.addEventListener('keydown', (e) => {
      if (e.key === 't' || e.key === 'T') { e.preventDefault(); toggle(); }
    });
    const btn = document.getElementById('btn-skills');
    if (btn) btn.addEventListener('click', toggle);
    api.ready = true;
    return api;
  };

  api.refresh = render;
  window.D2Skills = api;
})();
