// online-ui.js — écran « Partie en ligne » (lobby, phase 2 du plan serveur-coop).
// Créer/rejoindre une salle via /api/rooms, présence par polling 2 s (le GET
// rafraîchit la présence côté serveur), lancement par l'hôte : chacun passe à
// la sélection de héros, et Jouer relaie le code salle à game.html (?online=).
// Script classique chargé par menu.html AVANT menu.js (contrat window.D2Online).
(function () {
  'use strict';

  var KEY = 'd2online.v1'; // siège {code, token} — survit au rechargement de l'onglet
  var POLL_MS = 2000;

  var seat = null;          // {code, token}
  var pollTimer = null;
  var active = false;       // écran online affiché
  var launched = false;     // passage à la sélection déjà déclenché
  var hintBackup = null;

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
    onlineErr('');
  }

  function onlineErr(t) {
    var el = $('onlineErr');
    el.hidden = !t;
    el.textContent = t || '';
  }

  function renderPlayers(players) {
    var box = $('onlinePlayers');
    box.textContent = '';
    (players || []).forEach(function (p, i) {
      var row = document.createElement('div');
      row.className = 'onlinerow';
      row.textContent = (i + 1) + '. ' + p.name + (p.host ? ' — hôte' : '');
      if (p.host) row.className += ' host';
      box.appendChild(row);
    });
    if (!players || !players.length) {
      box.textContent = '…';
    }
    $('onlineStart').hidden = !(seat && seat.youHost);
    $('onlineStart').disabled = false;
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

  /** L'hôte a lancé (ou le polling voit playing) : passage à la sélection. */
  function launch(v) {
    if (launched) return;
    launched = true;
    stopPoll();
    active = false;
    hideOnline();
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

  function create() {
    onlineErr('');
    api('/api/rooms', { method: 'POST', body: JSON.stringify({ name: myName() }) })
      .then(function (r) {
        seat = { code: r.code, token: r.token, youHost: true };
        saveSeat(seat);
        showRoom();
        startPoll();
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
      seat = { code: prev.code, token: prev.token, youHost: !!prev.youHost };
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
})();
