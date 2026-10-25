// Diablo II — menu : écran-titre + écran de sélection de héros.
// Les classes affichées viennent TOUJOURS de assets/manifest.json (heroes{})
// — aucune classe codée en dur ici. Dégradation BYO : chaque asset est
// sondé ; s'il manque (ex : build Pages sans assets/menu), un repli 100 % CSS
// prend le relais, et un héros sans sprites est marqué « bientôt » au lieu
// de faire planter la sélection.
//
// Écran de sélection, deux modes :
//  - SCÈNE ANIMÉE si le contrat assets/menu/heroes.json est présent
//    ({CODE:{file,frames,w,h,fps}} + heroes/<CODE>.png, possiblement fire) :
//    canvas plein écran, héros en idle autour du feu central, survol/souris
//    + clavier, panneau de stats, un seul rAF.
//  - CARTES sinon (repli historique).
// Dans les deux modes la liste, l'ordre, les noms et les stats viennent du
// manifest heroes{} — heroes.json ne décrit que l'animation.
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
  function loadImg(url) {
    return new Promise((ok) => {
      const im = new Image();
      im.onload = () => ok(im.naturalWidth ? im : null);
      im.onerror = () => ok(null);
      im.src = url;
    });
  }
  let audioOK = null;
  function clickSfx() {
    try { new Audio('assets/sfx/ui_click.wav').play(); } catch (e) {}
  }

  // bruitages de sélection/désélection par classe (extraits des MPQ)
  const heroSfxCache = {};
  function playHeroSfx(code, kind) {
    const key = code + '-' + kind;
    if (!heroSfxCache[key]) heroSfxCache[key] = new Audio(`assets/sfx/${code}-${kind}.wav`);
    try { heroSfxCache[key].currentTime = 0; heroSfxCache[key].play(); } catch (e) {}
  }

  // ---- musique du menu original (thème du titre, ogg extrait du MPQ) ----
  const menuAudio = new Audio('assets/menu/menu-music.ogg');
  menuAudio.loop = true;
  menuAudio.volume = 0.45;
  let musicStarted = false, musicMuted = false;
  try { musicMuted = localStorage.getItem('d2wasm.musicMuted') === '1'; } catch (e) {} // EXP-A1 : clé partagée avec demo/music.js (mute persistant menu/jeu)
  function startMusic() {
    if (musicStarted) return;
    musicStarted = true;
    if (!musicMuted) menuAudio.play().catch(() => { musicStarted = false; });
  }
  window.addEventListener('pointerdown', startMusic);
  window.addEventListener('keydown', startMusic);
  function toggleMusic() {
    musicMuted = !musicMuted;
    try { localStorage.setItem('d2wasm.musicMuted', musicMuted ? '1' : '0'); } catch (e) {} // EXP-A1 : persiste la même clé que le jeu
    if (musicMuted) menuAudio.pause();
    else { startMusic(); menuAudio.play().catch(() => {}); }
    const b = document.getElementById('musicBtn');
    if (b) b.textContent = musicMuted ? '🔇' : '🔊';
  }

  // ---- écrans ----
  const screenTitle = document.getElementById('screenTitle');
  const screenSelect = document.getElementById('screenSelect');
  const screenSlots = document.getElementById('screenSlots');
  let onTitle = true;
  let onSlots = false;
  function showSelect() { onTitle = false; onSlots = false; screenTitle.classList.remove('on'); screenSlots.classList.remove('on'); screenSelect.classList.add('on'); }
  function showTitle() { onTitle = true; onSlots = false; screenSelect.classList.remove('on'); screenSlots.classList.remove('on'); screenTitle.classList.add('on'); }
  function showSlots() { onTitle = false; onSlots = true; screenTitle.classList.remove('on'); screenSelect.classList.remove('on'); screenSlots.classList.add('on'); renderSlots(); }

  // ---- dégradation des assets du titre ----
  // fond : si le png manque, le dégradé radial du #stage (CSS) prend le relais
  // (complete&&naturalWidth : erreur déjà survenue avant l'écouteur — 404 rapide)
  const bg = document.getElementById('bg');
  const bgGone = () => { bg.style.display = 'none'; };
  bg.addEventListener('error', bgGone);
  if (bg.complete && !bg.naturalWidth) bgGone();
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
  // fireTimer est global : un éventuel feu du contrat (heroes.json.fire)
  // plus fidèle prend le relais et coupe cette animation.
  let fireTimer = null;
  (function () {
    const cv = document.getElementById('fireCanvas');
    const ctx = cv.getContext('2d');
    const sheet = new Image();
    sheet.onload = () => {
      if (stage.classList.contains('no-fire')) return;
      let ff = 0;
      fireTimer = setInterval(() => {
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
  const sceneCanvas = document.getElementById('sceneCanvas');
  const sctx = sceneCanvas.getContext('2d');
  const sceneStats = document.getElementById('sceneStats');
  const SCALE = 1.0;       // poses FrontEnd (charactercreationscreen) déjà à l'échelle écran
  let heroList = [];       // [{code, def, playable, el, anim?, pos?}] ordre d'affichage
  let selectedCode = null;
  const scene = { on: false, hover: null };   // mode scène animée

  function heroName(def, code) { return (def && def.name) || LABELS[code] || code; }
  function isPlayable(def) {
    if (!def || typeof def !== 'object') return false;
    const hasModes = !!(def.modes && (def.modes.WL || def.modes.NU || def.modes.A1));
    const hasWalk = !!(def.walk && def.walk.layers && Object.keys(def.walk.layers).length);
    return hasModes || hasWalk;
  }
  function orderedCodes(heroes) {
    return Object.keys(heroes).sort((a, b) => {
      const ia = ORDER.indexOf(a), ib = ORDER.indexOf(b);
      return (ia < 0 ? ORDER.length : ia) - (ib < 0 ? ORDER.length : ib);
    });
  }

  function buildCards(manifest) {
    const heroes = (manifest && manifest.heroes) || {};
    const codes = orderedCodes(heroes);
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

  // héros sélectionnables selon le mode actif :
  // cartes -> manifest jouable ; scène -> manifest jouable ET sheet chargée
  function selectable() {
    return heroList.filter((h) => (scene.on ? (h.playable && h.anim) : h.playable));
  }

  function selectHero(code, silent) {
    const entry = selectable().find((h) => h.code === code);
    if (!entry) return;
    const prev = selectedCode;
    selectedCode = code;
    for (const h of heroList) h.el.classList.toggle('selected', h.code === code);
    playBtn.disabled = false;
    // animations de marche façon D2 : le nouveau héros avance d'un pas vers
    // la caméra (FW), l'ancien recule (BW) — la POSITION glisse en synchro
    // avec la séquence (de là où il est vers la cible), plus de héros qui
    // marche sur place.
    if (scene.on && prev !== code) {
      const now = performance.now();
      // FIX-MENU-ANIM : le trajet original vit DANS la planche (frames posées
      // à leurs vrais offsets DC6) — on ne fait que la jouer depuis son ancre.
      const step = (h, kind) => {
        const sheet = kind === 'fw' ? h.fw : h.bw;
        if (!sheet || sheet.ax == null) return;
        h.walk = { kind, start: now };
      };
      const prevH = heroList.find((h) => h.code === prev);
      if (prevH) step(prevH, 'bw');
      const newH = heroList.find((h) => h.code === code);
      if (newH) step(newH, 'fw');
      // bruitages officiels par classe (Cursor/intro/*.wav)
      if (prev) playHeroSfx(prev, 'des');
      playHeroSfx(code, 'sel');
    }
    if (scene.on) showSceneStats(entry);
    if (!silent) clickSfx();
  }

  function moveSel(delta) {
    const s = selectable();
    if (!s.length) return;
    let i = s.findIndex((h) => h.code === selectedCode);
    i = i < 0 ? 0 : (i + delta + s.length) % s.length;
    selectHero(s[i].code);
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

  // ================= scène animée (contrat heroes.json) =================
  // alias de codes : le contrat nomme la Sorcière « SO », le manifest « SOR »
  const ALIAS = { SO: 'SOR', SOR: 'SO' };

  function sheetEntry(json, code) {
    if (!json || typeof json !== 'object') return null;
    const e = json[code] || json[ALIAS[code]] || null;
    return (e && e.file && e.frames > 0 && e.w > 0 && e.h > 0) ? e : null;
  }
  const BUST = 'v=' + Date.now(); // contrat en chantier multi-agents : jamais de cache
  function sheetURL(file) {
    // contrat : heroes/<CODE>.png — un file contenant déjà un chemin est respecté
    const base = file.indexOf('/') >= 0 ? MENU + file : MENU + 'heroes/' + file;
    return base + (base.indexOf('?') < 0 ? '?' : '&') + BUST;
  }

  // rangée à plat comme l'écran original : pieds alignés sur la cour, le feu
  // (braises du fond + animation) occupe l'avant-centre.
  function arcPos(i, n) {
    const t = n > 1 ? i / (n - 1) : 0.5;
    return { x: 9 + 82 * t, y: 68 };
  }

  async function tryScene(manifest) {
    let json = null;
    try {
      const r = await fetch(MENU + 'heroes.json?' + BUST);
      if (r.ok) json = await r.json();
    } catch (e) { /* contrat absent -> cartes */ }
    if (!json) return;
    applyContractFire(json);
    // fond original de l'écran de sélection (charactercreationscreenEXP)
    if (json.background) {
      const bg = await loadImg(MENU + json.background);
      if (bg) stage.style.backgroundImage = `url(${bg.src})`;
    }
    const heroes = (manifest && manifest.heroes) || {};
    const codes = orderedCodes(heroes);
    // préchargement des sheets (échec -> null, le héros sera « bientôt ») ;
    // contrat v2 : nu1 = repos, nu2 = survol, nu3 = sélectionné (DC6 FrontEnd) ;
    // prog = courbe de progression RÉELLE de la marche (racine par frame des
    // DC6 originaux — l'Assassine dart pendant sa roue, le Barbare avance en pas)
    const loadState = (o) => !o ? Promise.resolve(null) : loadImg(sheetURL(o.file))
      .then((img) => img && { img, frames: o.frames | 0, w: o.w | 0, h: o.h | 0, fps: +o.fps > 0 ? +o.fps : 8,
        ax: o.ax != null ? +o.ax : null, ay: o.ay != null ? +o.ay : null,
        netx: +o.netx || 0, nety: +o.nety || 0 });
    const entries = [];
    for (const code of codes) {
      const e = sheetEntry(json, code);
      let anim = null, animHover = null, animSel = null, fw = null, bw = null;
      if (e) {
        anim = await loadState(e);
        animHover = (await loadState(e.nu2)) || anim;
        animSel = (await loadState(e.nu3)) || anim;
        fw = await loadState(e.fw);   // marche avant (sélection)
        bw = await loadState(e.bw);   // marche arrière (désélection)
      }
      entries.push({ code, anim, animHover, animSel, fw, bw });
    }
    // aucune sheet utilisable -> on reste sur les cartes
    if (!entries.some((x) => x.anim)) return;
    for (const h of heroList) {
      const idx = entries.findIndex((x) => x.code === h.code);
      const en = idx >= 0 ? entries[idx] : null;
      h.anim = en && en.anim;
      h.animHover = en && en.animHover;
      h.animSel = en && en.animSel;
      h.fw = en && en.fw;
      h.bw = en && en.bw;
      h.walk = null;
      h.off = { x: 0, y: 0 }; // FIX-MENU-ANIM : décalage courant (pas avant/arrière)
      h.pos = arcPos(idx, entries.length);
    }
    scene.on = true;
    scene.hover = null;
    screenSelect.classList.add('scene');
    // resynchronise la sélection (buildCards a présélectionné côté cartes)
    const s = selectable();
    if (!s.length) return;
    const cur = s.find((h) => h.code === selectedCode);
    if (cur) showSceneStats(cur);
    else selectHero(s[0].code, true);
  }

  // feu du contrat : heroes.json.fire {file,frames,w,h,fps} — prioritaire sur
  // fire-sheet.png ; à défaut les frames existantes, sinon feu CSS (.no-fire)
  function applyContractFire(json) {
    const e = json && (json.fire || json.FIRE);
    if (!e || !e.file || !(e.frames > 0) || !(e.w > 0) || !(e.h > 0)) return;
    (async () => {
      let img = await loadImg(sheetURL(e.file));
      if (!img) img = await loadImg(MENU + e.file);
      if (!img) return;
      if (fireTimer) { clearInterval(fireTimer); fireTimer = null; }
      stage.classList.remove('no-fire');
      const cv = document.getElementById('fireCanvas');
      cv.width = e.w; cv.height = e.h;
      cv.style.width = e.w + 'px'; cv.style.height = e.h + 'px';
      const ctx = cv.getContext('2d');
      let ff = 0;
      const step = () => {
        ff = (ff + 1) % e.frames;
        ctx.clearRect(0, 0, cv.width, cv.height);
        // le DC6 du feu n'a pas d'alpha : blend additif (le noir disparaît),
        // comme le rendu D2 original des flammes sur fond sombre.
        ctx.globalCompositeOperation = 'lighter';
        ctx.drawImage(img, ff * e.w, 0, e.w, e.h, 0, 0, e.w, e.h);
        ctx.globalCompositeOperation = 'source-over';
      };
      step();
      fireTimer = setInterval(step, 1000 / ((+e.fps > 0 && +e.fps) || 15));
    })();
  }

  // ---- rendu de la scène : UN SEUL rAF, dessine seulement si visible ----
  function stageXY(p) { return { x: (p.x / 100) * 800, y: (p.y / 100) * 600 }; }

  function drawScene(t) {
    sctx.clearRect(0, 0, 800, 600);
    sctx.imageSmoothingEnabled = false;
    // du fond au premier plan (pieds les plus bas d'abord, position glissante)
    const list = heroList.slice().sort((a, b) => (a.pos.y + a.off.y) - (b.pos.y + b.off.y));
    for (const h of list) {
      try {
      if (!h.anim) { drawMissingHero(h, stageXY({ x: h.pos.x + h.off.x, y: h.pos.y + h.off.y })); continue; }
      const hovered = scene.hover === h.code, sel = selectedCode === h.code;
      // FIX-MENU-ANIM : séquences ancrées — marche et anims d'état jouent
      // depuis leur frame 0 (ancrage au moment du changement), la sélection
      // joue UNE fois puis TIENT sa dernière image (la pose finale).
      // L'indexation globale (t/1000*fps)%frames faisait démarrer chaque
      // séquence au milieu/à la fin, la rejouer (2 passes perçues), et
      // rogner la fin avant la pose du héros sélectionné.
      // état du sprite FrontEnd : marche > sélectionné > survol > repos
      const a = h.walk ? (h.walk.kind === 'fw' ? h.fw : h.bw)
        : (sel && h.animSel) || (hovered && h.animHover) || h.anim;
      if (h.st !== a) { h.st = a; h.stStart = t; }
      let fi;
      if (h.walk) {
        // FIX-MENU-ANIM : la planche alignée porte le trajet original
        // (frames posées à leurs offsets DC6) — on joue les frames et on
        // applique le déplacement net à la fin. Aucun glissement synthétique.
        const el = t - h.walk.start;
        fi = Math.min(Math.floor(el / 1000 * a.fps), a.frames - 1);
        if (el / 1000 * a.fps >= a.frames - 1) {
          h.off.x = Math.min(Math.max(h.off.x + a.netx / 8, -8), 18);  // px -> % (800)
          h.off.y = Math.min(Math.max(h.off.y + a.nety / 6, -6), 12);  // px -> % (600)
          h.walk = null;
        }
      } else if (sel && a === h.animSel) {
        // sélection : one-shot depuis 0, puis pose tenue
        fi = Math.min(Math.floor((t - h.stStart) / 1000 * a.fps), a.frames - 1);
      } else {
        // survol / repos : boucle démarrant à la frame 0 de leur entrée
        fi = Math.floor((t - h.stStart) / 1000 * a.fps) % a.frames;
      }
      const { x, y } = stageXY({ x: h.pos.x + h.off.x, y: h.pos.y + h.off.y });
      const dw = a.w * SCALE, dh = a.h * SCALE;
      if (hovered || sel) drawHalo(x, y, sel);
      // marche : ancre originale de la planche (pieds frame 0 sur la position)
      const wx = h.walk && a.ax != null ? x - a.ax * SCALE : x - dw / 2;
      const wy = h.walk && a.ax != null ? y - a.ay * SCALE : y - dh;
      sctx.drawImage(a.img, fi * a.w, 0, a.w, a.h, wx, wy, dw, dh);
      if (hovered) drawLabel(x, Math.max(22, y - dh - 12), heroName(h.def, h.code));
      } catch (e) { errlog.style.display = 'block'; errlog.textContent += '[draw ' + h.code + '] ' + e.message + '\n'; }
    }
  }

  function drawHalo(x, y, strong) {
    sctx.save();
    sctx.translate(x, y - 8);
    sctx.scale(1, 0.5);
    const g = sctx.createRadialGradient(0, 0, 4, 0, 0, 70);
    g.addColorStop(0, strong ? 'rgba(255,216,115,.5)' : 'rgba(255,200,90,.3)');
    g.addColorStop(1, 'rgba(255,200,90,0)');
    sctx.fillStyle = g;
    sctx.beginPath();
    sctx.arc(0, 0, 70, 0, Math.PI * 2);
    sctx.fill();
    sctx.restore();
  }

  function drawLabel(x, y, text) {
    sctx.font = '17px Georgia, serif';
    sctx.textAlign = 'center';
    sctx.shadowColor = 'rgba(0,0,0,.9)';
    sctx.shadowBlur = 4;
    sctx.fillStyle = '#ffd873';
    sctx.fillText(text, x, y);
    sctx.shadowBlur = 0;
  }

  // sheet manquante : socle discret + code, « noté » sans casser la scène
  function drawMissingHero(h, x, y) {
    sctx.save();
    sctx.globalAlpha = 0.5;
    sctx.strokeStyle = '#6b5a2e';
    sctx.lineWidth = 1;
    sctx.beginPath();
    sctx.ellipse(x, y, 26, 8, 0, 0, Math.PI * 2);
    sctx.stroke();
    sctx.font = '13px Georgia, serif';
    sctx.textAlign = 'center';
    sctx.fillStyle = '#8a7a55';
    sctx.fillText(h.code + ' — bientôt', x, y - 12);
    sctx.restore();
  }

  function showSceneStats(entry) {
    const s = (entry.def && entry.def.stats) || {};
    const row = (label, v) =>
      '<div class="row"><span>' + label + '</span><b>' + (v != null ? v : '?') + '</b></div>';
    sceneStats.innerHTML =
      '<h3></h3>' +
      row('Vie', s.life) + row('Mana', s.mana) +
      row('Force', s.strength) + row('Dextérité', s.dexterity) +
      '<div class="skill">Compétence : ' + (SKILLS[entry.code] || '—') + '</div>';
    sceneStats.querySelector('h3').textContent = heroName(entry.def, entry.code);
    sceneStats.classList.add('on');
  }

  requestAnimationFrame(function loop(t) {
    requestAnimationFrame(loop);
    if (!scene.on || onTitle) return;
    drawScene(t);
  });

  // ---- souris sur la scène : survol = halo + nom, clic = sélection ----
  function hitHero(mx, my) {
    for (const h of heroList) {
      if (!h.anim || !h.playable) continue;
      const a = h.anim;
      // position glissante : la zone cliquable suit le héros en marche
      const { x, y } = stageXY({ x: h.pos.x + h.off.x, y: h.pos.y + h.off.y });
      const dw = a.w * SCALE, dh = a.h * SCALE;
      if (mx >= x - dw * 0.35 && mx <= x + dw * 0.35 && my >= y - dh && my <= y) return h.code;
    }
    return null;
  }
  sceneCanvas.addEventListener('mousemove', (e) => {
    if (!scene.on) return;
    const r = sceneCanvas.getBoundingClientRect();
    const mx = ((e.clientX - r.left) / r.width) * 800;
    const my = ((e.clientY - r.top) / r.height) * 600;
    const hit = hitHero(mx, my);
    if (hit !== scene.hover) {
      scene.hover = hit;
      sceneCanvas.style.cursor = hit ? 'pointer' : 'default';
    }
  });
  sceneCanvas.addEventListener('mouseleave', () => { scene.hover = null; });
  sceneCanvas.addEventListener('click', () => { if (scene.hover) selectHero(scene.hover); });

  // ---- chargement manifest (données seules) puis contrat d'animation ----
  fetch('assets/manifest.json')
    .then((r) => { if (!r.ok) throw new Error('HTTP ' + r.status); return r.json(); })
    .then((manifest) => {
      buildCards(manifest);   // cartes toujours prêtes (repli immédiat)
      tryScene(manifest);     // scène animée seulement si le contrat existe
    })
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
      else if (action === 'slots') showSlots();
      else if (action === 'github') open('https://github.com/shpaw415/diablo2-wasm', '_blank');
      else if (action === 'quit') window.close();
    });
  });
  playBtn.addEventListener('click', play);
  document.getElementById('back').addEventListener('click', showTitle);

  // ================= écran « Parties » (slots de sauvegarde) =================
  const slotsList = document.getElementById('slotsList');
  const slotsEmpty = document.getElementById('slotsEmpty');
  const slotsMsg = document.getElementById('slotsMsg');
  const slotFile = document.getElementById('slotFile');

  function fmtPlaytime(ms) {
    const s = Math.floor((ms || 0) / 1000);
    const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60);
    return h ? h + ' h ' + m + ' min' : m ? m + ' min' : s + ' s';
  }
  function slotsMessage(t) {
    slotsMsg.hidden = !t;
    slotsMsg.textContent = t || '';
  }
  function renderSlots() {
    slotsMessage('');
    const slots = window.D2Slots ? D2Slots.list() : [];
    slotsList.textContent = '';
    slotsEmpty.hidden = slots.length > 0;
    for (const slot of slots) {
      const row = document.createElement('div');
      row.className = 'slotrow';
      const info = document.createElement('div');
      info.className = 'slotinfo';
      const nameEl = document.createElement('div');
      nameEl.className = 'slotname';
      nameEl.textContent = slot.name;
      const meta = document.createElement('div');
      meta.className = 'slotmeta';
      const d = new Date(slot.ts);
      meta.innerHTML = '<b></b> · niv. <b></b> · <b></b> · <b></b> · <b></b>';
      const bs = meta.querySelectorAll('b');
      bs[0].textContent = slot.hero_label || slot.hero_code;
      bs[1].textContent = slot.level != null ? slot.level : '?';
      bs[2].textContent = slot.zone || slot.level_key || 'town';
      bs[3].textContent = fmtPlaytime(slot.playtime_ms);
      bs[4].textContent = d.toLocaleDateString() + ' ' + d.toLocaleTimeString().slice(0, 5);
      info.append(nameEl, meta);
      const btns = document.createElement('div');
      btns.className = 'slotbtns';
      const mk = (label, cls, fn, title) => {
        const b = document.createElement('button');
        b.className = 'sbtn' + (cls ? ' ' + cls : '');
        b.textContent = label;
        if (title) b.title = title;
        b.addEventListener('click', (ev) => { ev.stopPropagation(); clickSfx(); fn(); });
        return b;
      };
      btns.append(
        mk('Jouer', '', () => { location.href = 'game.html?slot=' + encodeURIComponent(slot.id); }, 'Reprendre ce héros'),
        mk('Renommer', '', () => beginRename(slot, nameEl), 'Modifier le nom du slot'),
        mk('Exporter', '', () => {
          try { D2Slots.exportSlot(slot.id); slotsMessage('Exporté : ' + slot.name + ' (.json téléchargé).'); }
          catch (e) { slotsMessage('Export impossible : ' + e.message); }
        }, 'Télécharger la sauvegarde (.json)'),
        mk('Supprimer', 'danger', () => { D2Slots.remove(slot.id); renderSlots(); }, 'Effacer ce slot')
      );
      row.append(info, btns);
      slotsList.appendChild(row);
    }
  }
  // Renommage inline : le nom devient un input, Entrée/flou valident.
  function beginRename(slot, nameEl) {
    const input = document.createElement('input');
    input.type = 'text';
    input.maxLength = 24;
    input.value = slot.name;
    nameEl.textContent = '';
    nameEl.appendChild(input);
    input.focus();
    input.select();
    const commit = () => {
      D2Slots.rename(slot.id, input.value);
      renderSlots();
    };
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') { e.preventDefault(); commit(); }
      if (e.key === 'Escape') { e.stopPropagation(); renderSlots(); }
      e.stopPropagation(); // le clavier global (flèches…) ne vole pas la saisie
    });
    input.addEventListener('blur', commit);
    input.addEventListener('click', (e) => e.stopPropagation());
  }
  document.getElementById('slotsBack').addEventListener('click', showTitle);
  document.getElementById('slotImport').addEventListener('click', () => slotFile.click());
  slotFile.addEventListener('change', () => {
    const f = slotFile.files && slotFile.files[0];
    slotFile.value = '';
    if (!f) return;
    const r = new FileReader();
    r.onload = () => {
      try {
        const slot = D2Slots.importSlot(String(r.result));
        renderSlots();
        slotsMessage('Importé : ' + slot.name + ' (' + (slot.hero_label || slot.hero_code) + ').');
      } catch (e) {
        slotsMessage('Import refusé : ' + (e.message || e));
      }
    };
    r.readAsText(f);
  });

  // ---- Continuer grisé sans sauvegarde (slots ou clé historique) ----
  if (!localStorage.getItem('d2wasm.save.v1') && !(window.D2Slots && D2Slots.mostRecent())) {
    document.getElementById('btnContinue').disabled = true;
  }

  // ---- clavier ----
  document.addEventListener('keydown', (e) => {
    if (e.target === nameInput) {
      if (e.key === 'Enter') { e.preventDefault(); play(); }
      else if (e.key === 'Escape') nameInput.blur();
      return;
    }
    if (onSlots) {
      if (e.key === 'Escape') { showTitle(); clickSfx(); }
      return; // saisie/clavier des slots gérés localement (inputs)
    }
    if (onTitle) {
      if (e.key === 'Enter') { showSelect(); clickSfx(); }
      return;
    }
    switch (e.key) {
      case 'ArrowLeft': moveSel(-1); break;
      case 'ArrowRight': moveSel(1); break;
      // scène : arc de héros, ↑/↓ = ±1 ; cartes : grille, ↑/↓ = ±4
      case 'ArrowUp': moveSel(scene.on ? -1 : -4); break;
      case 'ArrowDown': moveSel(scene.on ? 1 : 4); break;
      case 'Enter': play(); break;
      case 'Escape': showTitle(); break;
      default: return;
    }
    e.preventDefault();
  });

  // bouton bascule musique (créé dynamiquement, haut-droite du stage)
  const musicBtn = document.createElement('button');
  musicBtn.id = 'musicBtn';
  musicBtn.textContent = musicMuted ? '🔇' : '🔊'; // EXP-A1 : reflète l'état persistant
  musicBtn.title = 'Musique du menu';
  musicBtn.addEventListener('click', (ev) => { ev.stopPropagation(); toggleMusic(); });
  document.getElementById('stage').appendChild(musicBtn);
  startMusic();
})();
