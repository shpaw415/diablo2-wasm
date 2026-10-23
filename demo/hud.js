// HUD parité D2 — overlay DOM au-dessus du canvas (EXP-H* de
// docs/parity/finish-experience.md, rapport docs/parity/impl-hud-parite.md).
//
// NOUVEAU FICHIER (aucune reprise de game.js) — câblage minimal côté
// game.js/game.html : 5 points d'appel commentés `EXP-H*`.
//   D2HUD.init(manifest, game)   — construit le DOM, écoute souris/clavier
//   D2HUD.update(stats)          — EXP-H2 barre d'endurance (par frame)
//   D2HUD.events(events)         — EXP-H7 dégâts flottants (par tick)
//   D2HUD.frame(canvas, game)    — EXP-H6 survol monstre + EXP-H8 labels Alt
//
// Endpoints wasm additifs consommés : heroSkills, selectSkill, monsterInfo,
// groundItems ; input `rangedAttack` (clic gauche/droit selon la main).
// L'état du moteur (`window.__d2.game`) est lu directement ; chaque appel est
// gardé pour ne JAMAIS casser la boucle de jeu si l'overlay échoue.
(function () {
  'use strict';

  const D2HUD = {
    ready: false,
    game: null,
    canvas: null,
    skills: [],                 // [{index,code,name,level,missile}]
    hands: { L: -1, R: -1 },    // index skill par main ; -1 = attaque de base
    pendingHand: 'R',           // main cible du popup compétences ouvert
    altDown: false,
    mouse: { x: -1, y: -1, inside: false },
    els: {},
    labelPool: new Map(),       // clé "index:code" → div réutilisée
  };

  // --- helpers -------------------------------------------------------------
  function el(tag, id, parent) {
    const n = document.createElement(tag);
    if (id) n.id = id;
    (parent || D2HUD.els.root).appendChild(n);
    return n;
  }

  function shortSkillName(name) {
    // Fire Bolt → FB ; Attaque → ⚔
    if (!name || name === 'Attaque') return '⚔';
    return name.split(/\s+/).map((w) => w[0]).join('').toUpperCase().slice(0, 3);
  }

  function skillOf(hand) {
    const idx = D2HUD.hands[hand];
    if (idx < 0) return { index: -1, code: 'Attaque', name: 'Attaque', level: 0, missile: null };
    return D2HUD.skills[idx] || { index: -1, code: 'Attaque', name: 'Attaque', level: 0, missile: null };
  }

  function refreshSkillButtons() {
    for (const hand of ['L', 'R']) {
      const btn = D2HUD.els['skbtn' + hand];
      const s = skillOf(hand);
      btn.textContent = shortSkillName(s.name);
      btn.title = `${hand === 'L' ? 'Gauche' : 'Droite'} : ${s.name}` +
        (s.level ? ` (niv. ${s.level})` : '') + (s.missile ? ' · projectile' : '') +
        ' — cliquer pour choisir';
      btn.classList.toggle('selected', s.missile != null);
    }
  }

  // --- init ----------------------------------------------------------------
  D2HUD.init = function (manifest, game) {
    try {
      D2HUD.game = game || window.__d2.game || null;
      D2HUD.canvas = document.getElementById('game');
      if (!D2HUD.canvas || !D2HUD.game) return; // fallback JS : overlay inactif

      const stage = D2HUD.canvas.parentElement;
      const root = document.createElement('div');
      root.id = 'hudx';
      stage.appendChild(root);
      D2HUD.els.root = root;

      // EXP-H2 : barre d'endurance
      const stw = el('div', 'staminawrap');
      stw.title = 'Endurance';
      D2HUD.els.staminaFill = el('div', 'staminafill', stw);
      D2HUD.els.staminaWrap = stw;

      // EXP-H6 : survol monstre
      const mh = el('div', 'monhover');
      D2HUD.els.monName = el('div', 'monname', mh);
      const hpw = el('div', 'monhpwrap', mh);
      D2HUD.els.monHpFill = el('div', 'monhpfill', hpw);
      D2HUD.els.monHover = mh;

      // EXP-H8 : étiquettes objets (Alt) + EXP-H7 : dégâts flottants
      D2HUD.els.labels = el('div', 'itemlabels');
      D2HUD.els.floats = el('div', 'dmgfloats');

      // EXP-H4 : boutons L/R + popup compétences
      for (const hand of ['L', 'R']) {
        const b = el('button', 'skbtn-' + hand);
        b.className = 'skbtn';
        b.type = 'button';
        b.addEventListener('click', (e) => { e.stopPropagation(); openSkillPopup(hand); });
        D2HUD.els['skbtn' + hand] = b;
      }
      const pop = el('div', 'skpopup');
      D2HUD.els.skPopup = pop;

      // EXP-H3 : minipanel 8 boutons (Inventaire/Son câblés ; le reste
      // décoratif tant que les écrans EXP-E* correspondants n'existent pas).
      const mp = el('div', 'minipanel');
      const defs = [
        ['Menu', 'Menu jeu / pause — à venir (EXP-E4)', null],
        ['Inv', 'Inventaire (I)', () => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'i' }))],
        ['Stats', 'Panneau de stats — à venir (EXP-E9)', null],
        ['Skills', 'Arbre de compétences — à venir (EXP-E8)', () => openSkillPopup('R')],
        ['Quêtes', 'Journal de quêtes — à venir (EXP-E7)', null],
        ['Carte', 'Automap Tab — à venir (EXP-E6)', null],
        ['Aide', 'Aide (H) — à venir (EXP-E5)', null],
        ['Son', 'Activer/couper le son', () => {
          const b = document.getElementById('btn-sound');
          if (b) b.click();
        }],
      ];
      D2HUD.els.mpButtons = {};
      for (const [label, title, fn] of defs) {
        const b = document.createElement('button');
        b.type = 'button';
        b.textContent = label;
        b.title = title;
        if (!fn) b.classList.add('soon');
        else b.addEventListener('click', (e) => { e.stopPropagation(); fn(); });
        mp.appendChild(b);
        D2HUD.els.mpButtons[label] = b;
      }

      wireInput();
      loadSkills();
      refreshSkillButtons();
      D2HUD.ready = true;
    } catch (err) {
      console.info('D2HUD init ignoré :', err && err.message ? err.message : err);
    }
  };

  function loadSkills() {
    try {
      D2HUD.skills = JSON.parse(D2HUD.game.heroSkills() || '[]');
    } catch { D2HUD.skills = []; }
    // Défaut D2 : main gauche = attaque de base ; main droite = le premier
    // skill à projectile (sinon le premier skill, sinon attaque). C'est aussi
    // le `active_skill` par défaut du moteur (default_active = index 0).
    D2HUD.hands.L = -1;
    const s = D2HUD.skills.find((k) => k.missile) || D2HUD.skills[0];
    D2HUD.hands.R = s ? s.index : -1;
  }

  // --- EXP-H4 : popup de sélection de compétence ----------------------------
  function openSkillPopup(hand) {
    D2HUD.pendingHand = hand;
    const pop = D2HUD.els.skPopup;
    pop.textContent = '';
    const h = document.createElement('h3');
    h.textContent = `Main ${hand === 'L' ? 'gauche' : 'droite'} — choisir`;
    pop.appendChild(h);
    const rows = [{ index: -1, name: 'Attaque', level: 0, missile: null }, ...D2HUD.skills];
    for (const s of rows) {
      const row = document.createElement('div');
      row.className = 'skrow';
      const nm = document.createElement('span');
      nm.textContent = s.name;
      const meta = document.createElement('span');
      meta.className = s.missile ? 'missile' : 'lvl';
      meta.textContent = s.missile ? 'projectile' : (s.level ? `niv. ${s.level}` : 'mêlée');
      row.appendChild(nm);
      row.appendChild(meta);
      row.addEventListener('click', () => assignSkill(hand, s.index));
      pop.appendChild(row);
    }
    pop.classList.add('open');
  }
  function assignSkill(hand, index) {
    D2HUD.hands[hand] = index;
    if (index >= 0) {
      try { D2HUD.game.selectSkill(index); } catch { /* hors bornes : ignoré */ }
    }
    D2HUD.els.skPopup.classList.remove('open');
    refreshSkillButtons();
  }

  // --- entrées : clic gauche/droit selon la main, Alt, souris ---------------
  function wireInput() {
    const cv = D2HUD.canvas;
    cv.addEventListener('mousemove', (e) => {
      const r = cv.getBoundingClientRect();
      D2HUD.mouse.x = (e.clientX - r.left) * (cv.width / r.width);
      D2HUD.mouse.y = (e.clientY - r.top) * (cv.height / r.height);
      D2HUD.mouse.inside = true;
    });
    cv.addEventListener('mouseleave', () => {
      D2HUD.mouse.inside = false;
      if (D2HUD.els.monHover) D2HUD.els.monHover.style.display = 'none';
    });
    // Alt = étiquettes objets (EXP-H8) ; pas de menu navigateur sur le canvas.
    window.addEventListener('keydown', (e) => {
      if (e.key === 'Alt') { D2HUD.altDown = true; e.preventDefault(); }
      if (e.key === 'Escape' && D2HUD.els.skPopup.classList.contains('open')) {
        D2HUD.els.skPopup.classList.remove('open');
      }
    });
    window.addEventListener('keyup', (e) => { if (e.key === 'Alt') D2HUD.altDown = false; });
    window.addEventListener('blur', () => { D2HUD.altDown = false; });

    // Clic DROIT = main droite (D2). Sur le canvas : menu contextuel annulé,
    // routage via window.__d2.queue (exposé par game.js — point de branchement
    // commenté `EXP-H*`).
    cv.addEventListener('contextmenu', (e) => {
      e.preventDefault();
      castWithHand('R', e);
    });
    // Clic GAUCHE = main gauche : si le skill de main gauche porte un
    // projectile, il prime sur le routage move/attack de game.js (capture
    // sur window, AVANT les listeners canvas de game.js).
    window.addEventListener('click', (e) => {
      if (e.target !== cv || e.button !== 0) return;
      const s = skillOf('L');
      if (s.missile) {
        e.stopPropagation();
        castWithHand('L', e);
      }
    }, true);
  }

  /// Routage d'un clic-compétence : sélectionne le skill de la main côté
  /// moteur puis pousse l'input `rangedAttack` (projectile) — ou retombe sur
  /// le routage move/attack historique si la main n'a pas de projectile.
  function castWithHand(hand, e) {
    if (!D2HUD.ready || !D2HUD.game) return;
    const r = D2HUD.canvas.getBoundingClientRect();
    const px = (e.clientX - r.left) * (D2HUD.canvas.width / r.width);
    const py = (e.clientY - r.top) * (D2HUD.canvas.height / r.height);
    const s = skillOf(hand);
    let wx, wy;
    try {
      [wx, wy] = D2HUD.game.screenToWorld(px, py);
    } catch { return; }
    const tx = Math.round(wx), ty = Math.round(wy);
    const queue = window.__d2 && window.__d2.queue;
    if (!queue) return;
    if (s.missile) {
      try { D2HUD.game.selectSkill(s.index); } catch { /* ignoré */ }
      refreshSkillButtons();
      queue.push({ type: 'rangedAttack', x: tx, y: ty });
    } else {
      // mêlée / attaque de base : monstre sous le clic ⇒ attaque, sinon move
      let near = -1;
      try { near = D2HUD.game.monsterNear(wx, wy, 2.5); } catch { }
      queue.push(near >= 0
        ? { type: 'attack', x: tx, y: ty }
        : { type: 'move', x: tx, y: ty });
    }
  }

  // --- EXP-H2 : endurance ----------------------------------------------------
  D2HUD.update = function (stats) {
    if (!D2HUD.ready || !stats || stats.max_stamina === undefined) return;
    const max = Math.max(1, stats.max_stamina);
    const f = Math.max(0, Math.min(1, (stats.stamina || 0) / max));
    D2HUD.els.staminaFill.style.width = `${(f * 100).toFixed(1)}%`;
    D2HUD.els.staminaWrap.classList.toggle('low', f < 0.5 && f >= 0.2);
    D2HUD.els.staminaWrap.classList.toggle('empty', f < 0.2);
    D2HUD.els.staminaWrap.title = `Endurance : ${stats.stamina}/${stats.max_stamina}`;
  };

  // --- EXP-H7 : dégâts flottants ---------------------------------------------
  D2HUD.events = function (events) {
    if (!D2HUD.ready || !Array.isArray(events)) return;
    for (const ev of events) {
      if (ev.LevelSwitch) {
        // changement de zone : tout ce qui est projeté au sol est périmé
        D2HUD.els.floats.textContent = '';
        clearLabels();
        hideMonHover();
        continue;
      }
      if (ev.HeroAttacked) {
        spawnFloatAtMonster(ev.HeroAttacked.target, `−${ev.HeroAttacked.dmg}`, '#ffb347');
      } else if (ev.MissileHit) {
        spawnFloatAtMonster(ev.MissileHit.target, `−${ev.MissileHit.dmg}`, '#ffb347');
      } else if (ev.HeroMissed) {
        spawnFloatAtMonster(ev.HeroMissed.target, 'raté', '#b8ad98');
      } else if (ev.MissileMissed) {
        spawnFloatAtMonster(ev.MissileMissed.target, 'raté', '#b8ad98');
      } else if (ev.HeroHit) {
        spawnFloatAtHero(`−${ev.HeroHit.dmg}`, '#ff5545');
      }
    }
  };

  function monsterScreen(index) {
    try {
      const info = JSON.parse(D2HUD.game.monsterInfo(index) || 'null');
      return info ? [info.sx, info.sy] : null;
    } catch { return null; }
  }
  function spawnFloatAtMonster(index, text, color) {
    const pos = monsterScreen(index);
    if (pos) spawnFloat(pos[0], pos[1] - 34, text, color);
  }
  function spawnFloatAtHero(text, color) {
    try {
      const [x, y] = D2HUD.game.heroScreen();
      spawnFloat(x, y - 44, text, color);
    } catch { /* hors carte : ignoré */ }
  }
  function spawnFloat(x, y, text, color) {
    const n = document.createElement('span');
    n.className = 'dmgfloat';
    n.textContent = text;
    n.style.left = `${Math.round(x + (Math.random() * 16 - 8))}px`;
    n.style.top = `${Math.round(y)}px`;
    n.style.color = color;
    D2HUD.els.floats.appendChild(n);
    setTimeout(() => n.remove(), 950);
  }

  // --- EXP-H6 + EXP-H8 : poll par frame --------------------------------------
  D2HUD.frame = function (canvas, game) {
    if (!D2HUD.ready) return;
    try { frameMonsterHover(game); } catch { }
    try { frameItemLabels(game); } catch { }
    try { refreshSonLabel(); } catch { }
  };

  function hideMonHover() {
    if (D2HUD.els.monHover) D2HUD.els.monHover.style.display = 'none';
  }

  function frameMonsterHover(game) {
    if (!D2HUD.mouse.inside) { hideMonHover(); return; }
    const [wx, wy] = game.screenToWorld(D2HUD.mouse.x, D2HUD.mouse.y);
    const idx = game.monsterNear(wx, wy, 1.4);
    if (idx < 0) { hideMonHover(); return; }
    const info = JSON.parse(game.monsterInfo(idx) || 'null');
    if (!info || !info.alive) { hideMonHover(); return; }
    const mh = D2HUD.els.monHover;
    mh.className = 'k-' + info.kind; // garde #monhover + teinte du nom
    mh.id = 'monhover';
    D2HUD.els.monName.textContent = info.name;
    const frac = info.maxHp > 0 ? Math.max(0, Math.min(1, info.hp / info.maxHp)) : 0;
    D2HUD.els.monHpFill.style.width = `${(frac * 100).toFixed(0)}%`;
    mh.style.left = `${Math.round(info.sx)}px`;
    mh.style.top = `${Math.round(info.sy - 30)}px`;
    mh.style.display = 'block';
  }

  function clearLabels() {
    D2HUD.els.labels.textContent = '';
    D2HUD.labelPool.clear();
  }

  function frameItemLabels(game) {
    if (!D2HUD.altDown) {
      if (D2HUD.labelPool.size) clearLabels();
      return;
    }
    const items = JSON.parse(game.groundItems() || '[]');
    const seen = new Set();
    for (const it of items) {
      const key = `${it.index}:${it.code}:${it.amount}`;
      seen.add(key);
      let div = D2HUD.labelPool.get(key);
      if (!div) {
        div = document.createElement('div');
        div.className = 'ilabel k-' + it.kind;
        div.textContent = it.kind === 'Gold' ? `${it.name} (${it.amount})` : it.name;
        div.title = 'Cliquer pour ramasser (E au contact)';
        div.addEventListener('click', (e) => {
          e.stopPropagation();
          const q = window.__d2 && window.__d2.queue;
          if (q) q.push({ type: 'pickup' });
        });
        D2HUD.els.labels.appendChild(div);
        D2HUD.labelPool.set(key, div);
      }
      div.style.left = `${Math.round(it.sx)}px`;
      div.style.top = `${Math.round(it.sy - 14)}px`;
      div.style.display = 'block';
    }
    for (const [key, div] of D2HUD.labelPool) {
      if (!seen.has(key)) {
        div.remove();
        D2HUD.labelPool.delete(key);
      }
    }
  }

  function refreshSonLabel() {
    const b = document.getElementById('btn-sound');
    if (b && D2HUD.els.mpButtons && D2HUD.els.mpButtons.Son) {
      const on = /oui/i.test(b.textContent);
      D2HUD.els.mpButtons.Son.textContent = on ? 'Son' : 'Muet';
    }
  }

  window.D2HUD = D2HUD;
})();
