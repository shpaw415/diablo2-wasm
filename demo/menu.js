// Diablo II — menu : écran-titre + écran de sélection de héros.
// Les classes affichées viennent TOUJOURS de assets/manifest.json (heroes{})
// — aucune classe codée en dur ici. Dégradation BYO : chaque asset est
// sondé ; s'il manque (ex : build Pages sans assets/menu), un repli 100 % CSS
// prend le relais, et un héros sans sprites est marqué « bientôt » au lieu
// de faire planter la sélection.
(function () {
  'use strict';
  const MENU = 'assets/menu/';
  const stage = document.getElementById('stage');
  const errlog = document.getElementById('errlog');
  window.addEventListener('error', (e) => { errlog.textContent += '[err] ' + e.message + '\n'; });

  // ---- mise à l'échelle du stage 800x600 ----
  function fit() {
    const s = Math.min(innerWidth / 800, innerHeight / 600);
    stage.style.setProperty('--s', s);
  }
  addEventListener('resize', fit); fit();

  // ---- utilitaires ----
  function probe(url) {
    return new Promise((ok) => {
      const im = new Image();
      im.onload = () => ok(!!im.naturalWidth);
      im.onerror = () => ok(false);
      im.src = url;
    });
  }
  let audioOK = null;
  function clickSfx() {
    try { new Audio('assets/sfx/ui_click.wav').play(); } catch (e) {}
  }

  // ---- écrans ----
  const screenTitle = document.getElementById('screenTitle');
  const screenSelect = document.getElementById('screenSelect');
  let onTitle = true;
  function showSelect() { onTitle = false; screenTitle.classList.remove('on'); screenSelect.classList.add('on'); }
  function showTitle() { onTitle = true; screenSelect.classList.remove('on'); screenTitle.classList.add('on'); }

  // ---- dégradation des assets du titre ----
  // fond : si le png manque, le dégradé radial du #stage (CSS) prend le relais
  const bg = document.getElementById('bg');
  bg.addEventListener('error', () => { bg.style.display = 'none'; });
  // logo : frames manquantes -> titre 100 % CSS
  Promise.all([probe(MENU + 'logo_flame_l_00.png'), probe(MENU + 'logo_flame_r_00.png')])
    .then(([l, r]) => {
      if (l && r) return;
      stage.classList.add('no-logo');
      document.getElementById('logoText').removeAttribute('hidden');
      logoL.style.display = logoR.style.display = 'none';
      clearInterval(logoTimer);
    });
  // feu de camp : fire-sheet.png -> canvas animé, sinon feu CSS (classe .no-fire)
  probe(MENU + 'fire-sheet.png').then((ok) => { if (!ok) stage.classList.add('no-fire'); });

  // ---- logo en flammes : 30 frames (gauche + droite), ~15 fps ----
  // Chaque URL est demandée UNE seule fois (préchargement Image) ; ensuite
  // l'animation ne fait que permuter des src déjà en cache mémoire.
  const FRAMES = 30;
  const logoL = document.getElementById('logoL');
  const logoR = document.getElementById('logoR');
  const pad = (n) => String(n).padStart(2, '0');
  const frames = [];
  for (let i = 0; i < FRAMES; i++) {
    const L = new Image(), R = new Image();
    L.src = MENU + 'logo_flame_l_' + pad(i) + '.png';
    R.src = MENU + 'logo_flame_r_' + pad(i) + '.png';
    frames.push({ L, R });
  }
  let f = 0;
  const logoTimer = setInterval(() => {
    f = (f + 1) % FRAMES;
    const fr = frames[f];
    // frame prête (chargée, décodable) seulement — jamais de src en vol
    if (fr.L.complete && fr.R.complete && fr.L.naturalWidth && fr.R.naturalWidth) {
      logoL.src = fr.L.src;
      logoR.src = fr.R.src;
    }
  }, 66);

  // ---- feu de camp : fire-sheet.png = 30 cellules 180x140 ----
  (function () {
    const cv = document.getElementById('fireCanvas');
    const ctx = cv.getContext('2d');
    const sheet = new Image();
    sheet.onload = () => {
      if (stage.classList.contains('no-fire')) return;
      let ff = 0;
      setInterval(() => {
        ff = (ff + 1) % 30;
        ctx.clearRect(0, 0, 180, 140);
        ctx.drawImage(sheet, ff * 180, 0, 180, 140, 0, 0, 180, 140);
      }, 66);
    };
    sheet.onerror = () => stage.classList.add('no-fire');
    sheet.src = MENU + 'fire-sheet.png';
  })();

  // ================= sélection de héros =================
  // Libellés et compétences de départ : table locale UNIQUEMENT en repli —
  // le manifest fait foi dès qu'il fournit un champ name (déjà le cas).
  const LABELS = {
    BA: 'Barbare', SOR: 'Sorcière', PA: 'Paladin', AM: 'Amazone',
    AS: 'Assassin', DZ: 'Druide', NE: 'Nécromancien'
  };
  const SKILLS = {
    BA: 'Frappe', SOR: 'Trait de feu', PA: 'Sacrifice', AM: 'Flèche magique',
    AS: 'Explosion ardente', DZ: 'Tempête de feu', NE: 'Dégâts amplifiés'
  };
  const ORDER = ['AM', 'AS', 'BA', 'DZ', 'NE', 'PA', 'SOR']; // ordre alphabétique fr, inconnus ajoutés à la fin

  const heroGrid = document.getElementById('heroGrid');
  const playBtn = document.getElementById('play');
  const nameInput = document.getElementById('heroName');
  const manifestErr = document.getElementById('manifestErr');
  let heroList = [];       // [{code, def, playable, el}] dans l'ordre d'affichage
  let selectedCode = null;

  function heroName(def, code) { return (def && def.name) || LABELS[code] || code; }
  function isPlayable(def) {
    if (!def || typeof def !== 'object') return false;
    const hasModes = !!(def.modes && (def.modes.WL || def.modes.NU || def.modes.A1));
    const hasWalk = !!(def.walk && def.walk.layers && Object.keys(def.walk.layers).length);
    return hasModes || hasWalk;
  }

  function buildCards(manifest) {
    const heroes = (manifest && manifest.heroes) || {};
    const codes = Object.keys(heroes).sort((a, b) => {
      const ia = ORDER.indexOf(a), ib = ORDER.indexOf(b);
      return (ia < 0 ? ORDER.length : ia) - (ib < 0 ? ORDER.length : ib);
    });
    if (!codes.length) {
      manifestErr.hidden = false;
      manifestErr.textContent = 'Aucun héros dans assets/manifest.json (heroes{}) — sélection indisponible.';
      return;
    }
    heroGrid.textContent = '';
    heroList = codes.map((code) => {
      const def = heroes[code];
      const playable = isPlayable(def);
      const s = (def && def.stats) || {};
      const el = document.createElement('div');
      el.className = 'card' + (playable ? '' : ' disabled');
      el.dataset.hero = code;
      el.tabIndex = playable ? 0 : -1;
      el.setAttribute('role', 'button');
      el.setAttribute('aria-disabled', String(!playable));
      const nm = heroName(def, code);
      el.innerHTML =
        '<div class="med">' + code + '</div>' +
        '<div class="cname"></div>' +
        '<div class="stats">' +
        '<span>Vie <b>' + (s.life != null ? s.life : '?') + '</b></span>' +
        '<span>Mana <b>' + (s.mana != null ? s.mana : '?') + '</b></span>' +
        '<span>For <b>' + (s.strength != null ? s.strength : '?') + '</b></span>' +
        '<span>Dex <b>' + (s.dexterity != null ? s.dexterity : '?') + '</b></span>' +
        '</div>' +
        '<div class="skill">Compétence : ' + (SKILLS[code] || '—') + '</div>' +
        (playable ? '' : '<div class="soon">bientôt</div>');
      el.querySelector('.cname').textContent = nm;
      if (playable) {
        el.addEventListener('click', () => { selectHero(code); clickSfx(); });
      }
      heroGrid.appendChild(el);
      return { code, def, playable, el };
    });
    // présélection : dernier héros joué (s'il est jouable), sinon le premier jouable
    const last = (localStorage.getItem('d2hero') || '').toUpperCase();
    const pre = heroList.find((h) => h.playable && h.code === last) || heroList.find((h) => h.playable);
    if (pre) selectHero(pre.code, true);
  }

  function selectHero(code, silent) {
    const entry = heroList.find((h) => h.code === code);
    if (!entry || !entry.playable) return;
    selectedCode = code;
    for (const h of heroList) h.el.classList.toggle('selected', h.code === code);
    playBtn.disabled = false;
    if (!silent) clickSfx();
  }

  function moveSel(delta) {
    const idx = heroList.findIndex((h) => h.code === selectedCode);
    const start = idx < 0 ? 0 : idx;
    // cible jouable la plus proche dans la direction delta (max 4 pas de grille)
    for (let step = 1; step <= 4; step++) {
      const cand = heroList[start + delta * step];
      if (cand && cand.playable) { selectHero(cand.code); return; }
    }
  }

  function play() {
    if (!selectedCode) return;
    const nm = nameInput.value.trim();
    localStorage.setItem('d2hero', selectedCode);
    if (nm) localStorage.setItem('d2heroName', nm);
    else localStorage.removeItem('d2heroName');
    const p = new URLSearchParams({ hero: selectedCode });
    if (nm) p.set('name', nm);
    location.href = 'game.html?' + p.toString();
  }

  // ---- chargement manifest (données seules) ----
  fetch('assets/manifest.json')
    .then((r) => { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); })
    .then(buildCards)
    .catch((e) => {
      manifestErr.hidden = false;
      manifestErr.textContent = 'manifest.json indisponible (' + e.message + ') — sélection de héros impossible.';
    });

  // ---- boutons ----
  document.querySelectorAll('#buttons .dbtn').forEach((b) => {
    b.addEventListener('click', () => {
      clickSfx();
      const action = b.dataset.action;
      if (action === 'solo') showSelect();
      else if (action === 'continue') location.href = 'game.html?continue=1';
      else if (action === 'github') open('https://github.com/shpaw415/diablo2-wasm', '_blank');
      else if (action === 'quit') window.close();
    });
  });
  playBtn.addEventListener('click', play);
  document.getElementById('back').addEventListener('click', showTitle);

  // ---- Continuer grisé sans sauvegarde (clé posée par game.js) ----
  if (!localStorage.getItem('d2wasm.save.v1')) document.getElementById('btnContinue').disabled = true;

  // ---- clavier ----
  document.addEventListener('keydown', (e) => {
    if (e.target === nameInput) {
      if (e.key === 'Enter') { e.preventDefault(); play(); }
      else if (e.key === 'Escape') nameInput.blur();
      return;
    }
    if (onTitle) {
      if (e.key === 'Enter') { showSelect(); clickSfx(); }
      return;
    }
    switch (e.key) {
      case 'ArrowLeft': moveSel(-1); break;
      case 'ArrowRight': moveSel(1); break;
      case 'ArrowUp': moveSel(-4); break;   // grille 4 colonnes
      case 'ArrowDown': moveSel(4); break;
      case 'Enter': play(); break;
      case 'Escape': showTitle(); break;
      default: return;
    }
    e.preventDefault();
  });
})();
