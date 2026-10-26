// online-ui.js — écran « Partie en ligne » (lobby, phases 2 + 3c du plan
// serveur-coop). Créer/rejoindre une salle via /api/rooms, présence par
// polling 2 s (le GET rafraîchit la présence côté serveur). 3c : CHAQUE slot
// choisit SA classe DANS le salon (POST /api/rooms/:code/hero — stockée par
// siège, liste slots + classes visible de tous), et le lancement par l'hôte
// va DIRECTEMENT au jeu : chacun charge game.html avec SON héros à SON slot
// (?online=CODE&hero=CHOIX — /meta redistribute la liste par slot). Script
// classique chargé par menu.html AVANT menu.js (contrat window.D2Online).
(function () {
  'use strict';

  var KEY = 'd2online.v1'; // siège {code, token, hero?} — survit au rechargement de l'onglet
  var POLL_MS = 2000;

  var seat = null;          // {code, token, youHost, hero?}
  var pollTimer = null;
  var active = false;       // écran online affiché
  var launched = false;     // passage au jeu déjà déclenché
  var hintBackup = null;
  var classes = [];         // 3c : [{code, name}] jouables (assets/manifest.json)

  function $(id) { return document.getElementById(id); }

  function loadSeat() {
    try { return JSON.parse(sessionStorage.getItem(KEY) || 'null'); }
    catch (e) { return null; }
  }
  function saveSeat(s) {
    try { s ? sessionStorage.setItem(KEY, JSON.stringify(s)) : sessionStorage.removeItem(KEY); }
    catch (e) { /* navigation privée : tant pis, pas de reprise après reload */ }
  }
  function myName() {
    return (localStorage.getItem('d2heroName') || 'Joueur').slice(0, 16);
  }
  function lastHero() {
    return (localStorage.getItem('d2hero') || '').toUpperCase();
  }

  function api(path, opts) {
    opts = opts || {};
    opts.headers = { 'Content-Type': 'application/json' };
    return fetch(path, opts).then(function (r) {
      return r.json().catch(function () { return {}; }).then(function (body) {
        if (!r.ok) throw new Error(body.error || 'HTTP ' + r.status);
        return body;
      });
    });
  }

  // ---------------- rendu ----------------

  function showHome() {
    $('onlineHome').hidden = false;
    $('onlineRoom').hidden = true;
    onlineErr('');
  }

  function showRoom() {
    $('onlineHome').hidden = true;
    $('onlineRoom').hidden = false;
    $('onlineCodeBig').textContent = 'SALLE ' + seat.code;
    renderPlayers([]);
    renderClassPicker();
    onlineErr('');
  }

  function onlineErr(t) {
    var el = $('onlineErr');
    el.hidden = !t;
    el.textContent = t || '';
  }

  function classLabel(code) {
    for (var i = 0; i < classes.length; i++) {
      if (classes[i].code === code) return classes[i].name;
    }
    return code || '';
  }

  /** Liste des slots + classes (3c) : une ligne PAR SLOT, la classe choisie
   *  par chaque joueur est visible de tous (poll 2 s → vue serveur). */
  function renderPlayers(players) {
    var box = $('onlinePlayers');
    box.textContent = '';
    (players || []).forEach(function (p, i) {
      var row = document.createElement('div');
      row.className = 'onlinerow';
      row.textContent = (i + 1) + '. ' + p.name +
        ' — ' + (p.hero ? classLabel(p.hero) : 'classe…') +
        (p.host ? ' — hôte' : '');
      if (p.host) row.className += ' host';
      box.appendChild(row);
    });
    if (!players || !players.length) {
      box.textContent = '…';
    }
    $('onlineStart').hidden = !(seat && seat.youHost);
    $('onlineStart').disabled = false;
  }

  /** 3c : sélecteur de classe DU SIÈGE local, injecté dans l'écran salon
   *  (aucune édition de menu.html) — un bouton par classe jouable du
   *  manifest ; le clic publie la classe via REST et la liste des slots se
   *  met à jour chez tous au prochain poll. */
  function renderClassPicker() {
    var old = $('onlineClassPicker');
    if (old) old.remove();
    if (!classes.length || !$('onlinePlayers')) return;
    var wrap = document.createElement('div');
    wrap.id = 'onlineClassPicker';
    var label = document.createElement('div');
    label.textContent = 'Votre classe :';
    label.style.margin = '6px 0 2px';
    wrap.appendChild(label);
    var row = document.createElement('div');
    row.style.display = 'flex';
    row.style.flexWrap = 'wrap';
    row.style.gap = '4px';
    classes.forEach(function (c) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'dbtn small';
      b.textContent = c.name;
      b.dataset.hero = c.code;
      b.style.padding = '2px 8px';
      if (seat && seat.hero === c.code) {
        b.style.outline = '2px solid #c8a049';
        b.disabled = false; // re-cliquer = re-publier (idempotent)
      }
      b.addEventListener('click', function () { chooseClass(c.code); });
      row.appendChild(b);
    });
    wrap.appendChild(row);
    $('onlinePlayers').after(wrap);
  }

  /** Publie MA classe pour CE siège (REST, stockée PAR SLOT côté serveur) ;
   *  le polling redistribue la liste complète à tous les clients du salon. */
  function chooseClass(code) {
    if (!seat) return;
    var prev = seat.hero;
    seat.hero = code; // optimiste (le poll corrige si refus)
    saveSeat(seat);
    renderClassPicker();
    api('/api/rooms/' + seat.code + '/hero', {
      method: 'POST',
      body: JSON.stringify({ token: seat.token, hero: code }),
    }).then(function (v) {
      seat.hero = code;
      saveSeat(seat);
      if (active && v && v.players) renderPlayers(v.players);
    }).catch(function (e) {
      seat.hero = prev; // refus : retour au choix précédent
      saveSeat(seat);
      renderClassPicker();
      onlineErr('Classe refusée : ' + e.message);
    });
  }

  // ---------------- polling ----------------

  function startPoll() {
    stopPoll();
    pollTimer = setInterval(poll, POLL_MS);
    poll();
  }
  function stopPoll() {
    if (pollTimer) { clearInterval(pollTimer); pollTimer = null; }
  }

  function poll() {
    if (!seat) return;
    api('/api/rooms/' + seat.code + '?token=' + encodeURIComponent(seat.token))
      .then(function (v) {
        if (!active && launched) return; // déjà parti jouer
        seat.youHost = v.you && v.you.host;
        if (v.state === 'playing') { launch(v); return; }
        if (active) renderPlayers(v.players);
      })
      .catch(function (e) {
        if (active) { stopPoll(); seat = null; saveSeat(null); showHome(); onlineErr('Salle fermée (' + e.message + ').'); }
      });
  }

  /** L'hôte a lancé (ou le polling voit playing) : passage AU JEU. 3c :
   *  la classe a été choisie DANS le salon (par slot) — chacun charge
   *  game.html avec SON héros à SON slot (?online=CODE&hero=CHOIX, siège en
   *  sessionStorage). Sans choix publié (vieux salon/repli), l'écran de
   *  sélection historique prend le relais (menu.js « Jouer » relaie). */
  function launch(v) {
    if (launched) return;
    launched = true;
    stopPoll();
    active = false;
    hideOnline();
    var hero = seat && seat.hero;
    if (hero && seat && seat.code) {
      location.href = 'game.html?' + new URLSearchParams({ online: seat.code, hero: hero }).toString();
      return;
    }
    // L'écran online s'efface derrière la sélection de héros (menu.js).
    markSelectHint('Salle ' + v.code + ' — choisissez votre héros, puis Jouer');
    if (window.D2Menu) D2Menu.showSelect();
  }

  /** Les show* de menu.js prédalent l'écran salon : il le retire lui-même. */
  function hideOnline() {
    $('screenOnline').classList.remove('on');
  }

  function markSelectHint(t) {
    var hint = $('selectHint');
    if (!hint) return;
    if (hintBackup == null) hintBackup = hint.textContent;
    hint.textContent = t;
  }
  function restoreSelectHint() {
    if (hintBackup == null) return;
    var hint = $('selectHint');
    if (hint) hint.textContent = hintBackup;
    hintBackup = null;
  }

  // ---------------- actions ----------------

  /** Publie automatiquement le dernier héros joué comme classe du salon
   *  (3c) : /meta est complet dès l'entrée en salle — un lancement immédiat
   *  part toujours avec UN héros PAR slot. Re-cliquable dans le salon. */
  function publishDefaultHero() {
    if (!seat || seat.hero) return;
    var code = lastHero();
    for (var i = 0; i < classes.length; i++) {
      if (classes[i].code === code) { chooseClass(code); return; }
    }
    if (classes.length) chooseClass(classes[0].code); // premier jouable
  }

  function create() {
    onlineErr('');
    api('/api/rooms', { method: 'POST', body: JSON.stringify({ name: myName() }) })
      .then(function (r) {
        seat = { code: r.code, token: r.token, youHost: true };
        saveSeat(seat);
        showRoom();
        startPoll();
        publishDefaultHero();
      })
      .catch(function (e) { onlineErr('Création impossible : ' + e.message); });
  }

  function join() {
    var code = ($('onlineCode').value || '').trim().toUpperCase();
    if (code.length !== 5) { onlineErr('Code à 5 caractères (ex : K7M2X).'); return; }
    onlineErr('');
    api('/api/rooms/' + code + '/join', { method: 'POST', body: JSON.stringify({ name: myName() }) })
      .then(function (r) {
        seat = { code: r.code, token: r.token, youHost: !!(r.you && r.you.host) };
        saveSeat(seat);
        showRoom();
        startPoll();
        publishDefaultHero();
      })
      .catch(function (e) { onlineErr('Rejoint impossible : ' + e.message); });
  }

  function startGame() {
    if (!seat) return;
    api('/api/rooms/' + seat.code + '/start', { method: 'POST', body: JSON.stringify({ token: seat.token }) })
      .then(function (v) { launch(v); })
      .catch(function (e) { onlineErr('Lancement refusé : ' + e.message); });
  }

  function leaveAndClose() {
    stopPoll();
    if (seat) {
      var s = seat;
      seat = null;
      saveSeat(null);
      // Départ signalé fire-and-forget (keepalive survit au changement de page).
      fetch('/api/rooms/' + s.code + '/leave', {
        method: 'POST',
        body: JSON.stringify({ token: s.token }),
        headers: { 'Content-Type': 'application/json' },
        keepalive: true,
      }).catch(function () {});
    }
    launched = false;
    active = false;
    hideOnline();
    restoreSelectHint();
    showHome();
    if (window.D2Menu) D2Menu.showTitle();
  }

  // ---------------- écran ----------------

  function show() {
    active = true;
    launched = false;
    $('screenTitle').classList.remove('on');
    $('screenSelect').classList.remove('on');
    $('screenSlots').classList.remove('on');
    $('screenOnline').classList.add('on');
    var prev = loadSeat();
    if (prev && prev.code && prev.token) {
      seat = { code: prev.code, token: prev.token, youHost: !!prev.youHost, hero: prev.hero || null };
      showRoom();
      startPoll(); // le premier poll valide le siège (403 → retour accueil)
    } else {
      seat = null;
      showHome();
    }
    $('onlineCode').focus();
  }

  function isActive() { return active; }
  function onlineParam() { return launched && seat ? seat.code : null; }

  // ---------------- branchement ----------------

  $('onlineCreate').addEventListener('click', create);
  $('onlineJoin').addEventListener('click', join);
  $('onlineCode').addEventListener('keydown', function (e) {
    if (e.key === 'Enter') { e.preventDefault(); join(); }
    e.stopPropagation(); // le clavier global du menu ne vole pas la saisie
  });
  $('onlineStart').addEventListener('click', startGame);
  $('onlineLeave').addEventListener('click', leaveAndClose);
  $('onlineBack').addEventListener('click', leaveAndClose);
  // Onglet resté en arrière-plan (timers bridés) : rafraîchit la présence
  // dès qu'il revient au premier plan, au lieu d'attendre le prochain tick.
  document.addEventListener('visibilitychange', function () {
    if (!document.hidden && seat && !launched) poll();
  });

  window.D2Online = {
    show: show,
    isActive: isActive,
    onlineParam: onlineParam,
    leaveAndClose: leaveAndClose,
  };

  // 3c : classes jouables du manifest (mêmes données que l'écran sélection)
  // — chargées une fois pour le sélecteur du salon ; un échec laisse le
  // salon fonctionner sans sélecteur (repli écran sélection historique).
  fetch('assets/manifest.json')
    .then(function (r) { return r.json(); })
    .then(function (m) {
      var heroes = (m && m.heroes) || {};
      classes = Object.keys(heroes)
        .filter(function (c) {
          var def = heroes[c];
          return !!(def && ((def.modes && (def.modes.WL || def.modes.NU || def.modes.A1)) ||
            (def.walk && def.walk.layers && Object.keys(def.walk.layers).length)));
        })
        .sort()
        .map(function (c) { return { code: c, name: heroes[c].name || c }; });
      if (active && seat && !$('onlineRoom').hidden) renderClassPicker();
    })
    .catch(function () { /* pas de manifest : salon sans sélecteur */ });
})();
