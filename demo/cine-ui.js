// Cinématiques de changement d'acte (étape 23b) — demo/cine-ui.js (NOUVEAU
// fichier, importé par game.js AVANT la boucle de rendu).
//
// Rôle (mission 23b — cinématiques de changement d'acte) :
//   * Les vraies cinématiques Bink (.bik) de la release digitale fr-FR ont été
//     retrouvées dans les MPQ vidéo IMBRIQUÉS des Installer Tomes (d2video.mpq
//     et D2xVideo.mpq — cf. tools/stormlib_read.py + evidence/cine-sweep/),
//     extraites puis transcodées en WebM (VP8 1M + Opus, résolution d'origine
//     640×292) dans demo/assets/cine/.
//   * Event ActUnlocked {act} → film d'intro de l'acte (act1..act5.webm) ;
//     event GameWon → endgame.webm (D2x_Out). Dédupe : chaque film une fois
//     par session (une ActUnlocked ne re-joue pas au retour de ville).
//   * Lecteur plein écran (overlay CSS z-max) : le monde est EN PAUSE pendant
//     la vidéo (tick(0, null) — pattern dialogue-ui : les events en attente
//     continuent de couler), Échap = passer, fin de vidéo = reprise du jeu.
//   * Format anamorphique fidèle au jeu d'origine : les .bik sont stockés en
//     640×292 et étirés en 4:3 à la lecture (object-fit: fill), comme le
//     faisait le moteur D2 (stretch vertical ×1.64).
//
// Branché sur : GameEvent ActUnlocked/GameWon (d2rules lib.rs:427) via le
// retour JSON de game.tick() — même mécanisme que game-sfx.js/main.ts.
// Test : node cine-ui.golden.mjs (mapping event→film + dédupe, sans DOM).
var D2Cine = (function () {
  'use strict';

  var BASE = 'assets/cine/';
  var MOVIES = {
    act1: BASE + 'act1.webm',        // d2intro640x292.bik — intro du jeu (ouverture acte 1)
    act2: BASE + 'act2.webm',        // Act02start640x292.bik
    act3: BASE + 'act3.webm',        // Act03start640x292.bik
    act4: BASE + 'act4.webm',        // Act04start640x292.bik
    act5: BASE + 'act5.webm',        // D2x_Intro_640x292.bik — intro LoD (acte 5)
    act4end: BASE + 'act4end.webm',  // Act04end640x292.bik — épilogue acte 4 (bonus, non branché)
    endgame: BASE + 'endgame.webm',  // D2x_Out_640x292.bik — fin de jeu (GameWon)
  };
  var TITLES = {
    act1: 'Acte I — Le Mal s’éveille',
    act2: 'Acte II — Le Désert d’Aranoch',
    act3: 'Acte III — La Jungle de Kurast',
    act4: 'Acte IV — Les Flammes de l’Enfer',
    act5: 'Acte V — Le Mont Arreat',
    act4end: 'La Harde est brisée',
    endgame: 'Et la porte se referma…',
  };

  var played = {};      // dédupe : clés déjà jouées cette session
  var playing = false;  // film en cours (monde en pause)
  var wired = null;     // instance D2Game dont tick est wrappé
  var realTick = null;
  var overlay = null;
  var video = null;
  var onKeyGuard = null;
  var swallowGuards = [];

  // --- mapping pur event → film (testé sans DOM par cine-ui.golden.mjs) -----
  function cineForEvent(ev, playedSet) {
    var set = playedSet || played;
    var key = null;
    if (ev && ev.GameWon) {
      key = 'endgame';
    } else if (ev && ev.ActUnlocked) {
      var act = ev.ActUnlocked.act || 1;
      if (act >= 1 && act <= 5) key = 'act' + act;
    }
    if (!key || set[key]) return null;
    set[key] = true;
    return { key: key, src: MOVIES[key], title: TITLES[key] };
  }

  // --- lecteur plein écran ---------------------------------------------------
  function ensureStyles() {
    if (document.getElementById('d2cine-style')) return;
    var css = document.createElement('style');
    css.id = 'd2cine-style';
    css.textContent =
      '#d2cine{position:fixed;inset:0;z-index:300;background:#000;' +
      'display:none;align-items:center;justify-content:center;}' +
      '#d2cine.show{display:flex;}' +
      '#d2cine video{width:min(100vw,133.33vh);height:min(75vw,100vh);' + // 4:3 anamorphique (640×292 étiré, comme le jeu)
      'object-fit:fill;background:#000;}' +
      '#d2cine .d2cine-title{position:absolute;left:0;right:0;top:22px;text-align:center;' +
      'color:#c7a45a;font:700 15px Georgia,serif;letter-spacing:.25em;text-transform:uppercase;' +
      'text-shadow:0 2px 6px #000;pointer-events:none;opacity:.9;}' +
      '#d2cine .d2cine-hint{position:absolute;right:14px;bottom:10px;color:#8f8676;' +
      'font:italic 11px system-ui;letter-spacing:.04em;pointer-events:none;}';
    document.head.appendChild(css);
  }

  function buildOverlay() {
    ensureStyles();
    overlay = document.createElement('div');
    overlay.id = 'd2cine';
    overlay.innerHTML =
      '<video preload="auto"></video>' +
      '<div class="d2cine-title"></div>' +
      '<div class="d2cine-hint">Échap : passer · fin du film = reprise du jeu</div>';
    document.body.appendChild(overlay);
    video = overlay.querySelector('video');
    video.addEventListener('ended', close);
  }

  // Gardes d'input (capture sur document → avant game.js/hud.js) : pendant le
  // film, AUCUNE touche ni clic ne traverse vers le jeu — sauf Échap (passer).
  function installGuards() {
    onKeyGuard = function (e) {
      if (!playing) return;
      e.stopImmediatePropagation();
      if (e.key === 'Escape') {
        e.preventDefault();
        close();
      }
    };
    document.addEventListener('keydown', onKeyGuard, true);
    var swallow = function (e) {
      if (!playing) return;
      e.preventDefault();
      e.stopImmediatePropagation();
    };
    ['mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu', 'wheel'].forEach(function (t) {
      document.addEventListener(t, swallow, true);
      swallowGuards.push([t, swallow]);
    });
  }

  function uninstallGuards() {
    if (onKeyGuard) {
      document.removeEventListener('keydown', onKeyGuard, true);
      onKeyGuard = null;
    }
    swallowGuards.forEach(function (p) { document.removeEventListener(p[0], p[1], true); });
    swallowGuards = [];
  }

  // play('act2') — lance un film par clé ; ignore si déjà en lecture.
  function play(key) {
    var src = MOVIES[key];
    if (!src || playing) return false;
    playing = true;
    buildOverlay();
    overlay.querySelector('.d2cine-title').textContent = TITLES[key] || '';
    video.muted = false;
    video.src = src;
    overlay.classList.add('show');
    installGuards();
    var p = video.play();
    if (p && typeof p.catch === 'function') {
      p.catch(function () {
        // Autoplay avec son refusé : on tente muet (le film reste un marqueur
        // de transition ; le hint « Échap » reste visible pour passer).
        video.muted = true;
        var q = video.play();
        if (q && typeof q.catch === 'function') q.catch(function () { close(); });
      });
    }
    return true;
  }

  // Fermeture (fin de vidéo OU Échap) → reprise du jeu.
  function close() {
    playing = false;
    uninstallGuards();
    if (video) {
      video.removeEventListener('ended', close);
      try { video.pause(); } catch (e) { /* muet */ }
      video.removeAttribute('src');
      video.load();
    }
    if (overlay) {
      overlay.remove();
      overlay = null;
      video = null;
    }
  }

  // --- event handler (d1 candidat golden : event moteur → cinématique) ------
  function handleEvent(ev) {
    var hit = cineForEvent(ev, played);
    if (hit) play(hit.key);
    return !!hit;
  }

  // --- accroche moteur : wrapper de tick (comme game-sfx / dialogue-ui) -----
  function hook(win) {
    var g = win && win.__d2 && win.__d2.game;
    if (wired || !g || typeof g.tick !== 'function') return false;
    realTick = g.tick; // appelé via .call(g, …) comme dialogue-ui.js
    g.tick = function (dt, inputJson) {
      // PAUSE MONDE pendant le film : tick(0, null) — le moteur gèle mais les
      // events en attente continuent de couler (d2rules world.rs draine
      // pending_events avant le pas dt).
      var eventsJson = playing
        ? realTick.call(g, 0, null)
        : realTick.call(g, dt, inputJson);
      try {
        if (eventsJson && eventsJson !== '[]') {
          var events = JSON.parse(eventsJson);
          if (Array.isArray(events)) {
            for (var i = 0; i < events.length; i++) handleEvent(events[i]);
          }
        }
      } catch (e) { /* cosmétique : ne bloque jamais la boucle */ }
      return eventsJson;
    };
    wired = g;
    return true;
  }

  // Démarrage navigateur : poll jusqu'à ce que window.__d2.game existe
  // (game.js assigne après l'init WASM), puis wrap une seule fois.
  function start() {
    if (typeof document === 'undefined') return; // import node (golden test) : pas de DOM
    var iv = setInterval(function () {
      if (hook(typeof window !== 'undefined' ? window : null)) clearInterval(iv);
    }, 250);
    setTimeout(function () { clearInterval(iv); }, 120000);
  }

  start();

  return {
    cineForEvent: cineForEvent,
    handleEvent: handleEvent,
    play: play,
    close: close,
    movies: MOVIES,
    isPlaying: function () { return playing; },
    // hooks test/debug
    _reset: function () { played = {}; },
    _hook: hook,
  };
})();

if (typeof window !== 'undefined') window.D2Cine = D2Cine;
// Point d'entrée test sans DOM (node cine-ui.golden.mjs) — inoffensif en navigateur.
if (typeof globalThis !== 'undefined') globalThis.D2CineTest = { cineForEvent: D2Cine.cineForEvent, movies: D2Cine.movies };
