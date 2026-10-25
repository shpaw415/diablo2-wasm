// 22c PNJ & DIALOGUES — demo/dialogue-ui.js (NOUVEAU).
//
// Bandeau de dialogue fidèle D2 en bas d'écran, branché sur le moteur du
// coéquipier Dialogue (d2rules/src/dialogue.rs — API World::open_dialogue /
// choose_dialogue / close_dialogue, endpoints wasm `openDialogue(index)` /
// `chooseDialogue(i)` / `closeDialogue()`). Les lignes fr RÉELLES viennent
// du Rust (TBL v1.12 résolues côté moteur) — ici : rendu + routage, ZÉRO
// texte de jeu.
//
// Schéma session (DialogueSession serde du moteur, champ par champ) :
//   { npc_index, npc, npc_name, quest, phase,
//     node: { id, line_keys, lignes:[fr], options:[{ label_key, label,
//             next, cond:{kind:always|quest_done,quest}, action:
//             {kind:say|close|travel|open_shop|hire_mercenary|repair, vendor?} }] },
//     visible: [indices bruts visibles] }   ← facultatif (pont d2game)
//
// Contrat UI :
//   * PANNEAU : bandeau bas (nom du NPC en or, texte fr en blanc, options de
//     choix cliquables — soulignées au survol, style D2) ; le monde est
//     PAUSED pendant le dialogue — GEL MOTEUR (World::tick ne simule plus
//     rien, cf. dialogue.rs) DOUBLÉ d'un gel UI (tick(0, null) tant que la
//     session est ouverte — couvre un pkg wasm antérieur) ; Échap = fermer.
//   * WIRE : clic NPC → npc-ui.js doTalk → D2Dialogue.tryOpen(index) →
//     openDialogue → rendu de la session (lignes + options) ; clic option →
//     chooseDialogue(INDEX BRUT) → rafraîchit (ragots « dites-m'en plus ») ;
//     les actions des options déclenchent l'existant : open_shop → panneau
//     Marchands, travel → panneau waypoints, quête → events du moteur
//     (NpcTalked/ActQuestChanged flushés même monde gelé), close → partir.
//   * PROCHE DU RETAIL : monde gelé — la caméra reste sur le NPC (le héros
//     ne bouge pas, pas de patrouille, pas de monstre).
//
// Défensif (même pattern que npc-ui.js) : si le pkg wasm n'expose pas encore
// les endpoints de dialogue (build antérieur), le module reste inerte — la
// page reste jouable et la bulle W16 de npc-ui.js prend le relais.
//
// 23a — VOIX (d2speech.mpq fr-FR v1.12) : à l'affichage de chaque ligne, la
// voix réelle du NPC joue (index assets/sfx/speech/index.json — clé TBL ->
// wav, généré par tools/extract_speech.py ; 91/91 clés du moteur couvertes,
// golden test d2rules/tests/speech_golden.rs). Musique DUCKÉE pendant la
// voix (API publique D2Audio.setVolume('music')), rendue à la fin/fermeture.
// Tout est défensif : index/wav absent ou mute global = silence propre, pas
// une seule erreur JS (garde-fou E2E, même contrat que game-sfx.js).
//
// Branchement (game.js, 1 ligne) : `import './dialogue-ui.js';` — le module
// s'auto-accroche au moteur dès `window.__d2.game` est finalisé.

(function () {
  'use strict';
  if (typeof window !== 'undefined' && window.D2Dialogue) return; // anti double-chargement

  var POLL_MS = 300;
  var hooked = null;   // instance D2Game dont `tick` est wrappé (pause monde)
  var realTick = null; // tick d'origine (prototype wasm-bindgen)
  var session = null;  // session moteur parsée {index,name,role,lines,options}
  var panel = null, elName = null, elText = null, elOpts = null;
  var pollIv = null;

  // --- petits utilitaires (même famille que vendor.js : pas de hooks requis) -
  function game() {
    return (typeof window !== 'undefined' && window.__d2 && window.__d2.game) || null;
  }
  function available(g) {
    return !!g && typeof g.openDialogue === 'function' &&
      typeof g.chooseDialogue === 'function';
  }
  function emsg(e) { return e && e.message ? e.message : String(e); }
  function status(msg) {
    var s = typeof document !== 'undefined' && document.getElementById('status');
    if (s) s.textContent = s.textContent.replace(/ · \d+ fps.*$/, '') + ' — ' + msg;
  }
  function note(msg) {
    if (typeof document === 'undefined') return;
    var log = document.getElementById('msglog');
    if (log) {
      var line = document.createElement('div');
      line.textContent = '• ' + msg;
      log.appendChild(line);
      while (log.children.length > 12) log.removeChild(log.firstChild);
      log.scrollTop = log.scrollHeight;
    }
  }

  // --- session moteur → modèle UI (DialogueSession serde de dialogue.rs) -----
  // `DialogueAction` : {kind:say|close|travel|open_shop|hire_mercenary|repair,
  //  vendor?} ; `DialogueCond` : {kind:always|quest_done, quest?}.
  function actionOf(a) {
    if (a == null) return null;
    if (typeof a === 'string') return { kind: a.toLowerCase(), arg: null };
    if (typeof a === 'object') {
      return {
        kind: String(a.kind || a.type || '').toLowerCase(),
        arg: a.vendor || a.target || null,
      };
    }
    return null;
  }
  function optOf(o, i) {
    if (o == null) return null;
    if (typeof o === 'string' || typeof o === 'number') {
      return { index: i, text: String(o), action: null };
    }
    return {
      index: i, // INDEX BRUT dans node.options (contrat choose_dialogue)
      text: o.label || o.text || o.label_key || 'Option ' + (i + 1),
      action: actionOf(o.action),
    };
  }
  function parseSession(raw) {
    var s;
    try { s = typeof raw === 'string' ? JSON.parse(raw) : raw; } catch (e) { return null; }
    if (!s || typeof s !== 'object') return null;
    var node = s.node && typeof s.node === 'object' ? s.node : s;
    var opts = node.options || s.options || s.choices || [];
    var options = [];
    for (var i = 0; i < opts.length; i++) {
      var o = optOf(opts[i], i);
      if (o) options.push(o);
    }
    // Options VISIBLES : le pont peut porter `visible` (indices bruts issus
    // de World::dialogue_visible_options) ; sinon tout le nœud s'affiche —
    // un choix caché serait de toute façon refusé par le moteur (None).
    var vis = s.visible || s.visibleOptions || node.visible || null;
    if (vis && vis.length !== undefined) {
      var keep = {};
      for (var v = 0; v < vis.length; v++) keep[vis[v]] = true;
      options = options.filter(function (o) { return keep[o.index]; });
    }
    var lines = node.lignes || s.lignes || s.lines || s.text || s.dialogue || '';
    if (typeof lines === 'string') lines = lines ? [lines] : [];
    // 23a : clés TBL des lignes (line_keys du moteur) — consommées par
    // l'index vocal assets/sfx/speech/index.json (une voix par ligne affichée).
    var keys = node.line_keys || s.line_keys || [];
    return {
      index: s.npc_index != null ? s.npc_index : (s.index != null ? s.index : null),
      name: s.npc_name || s.name || s.npc || 'PNJ',
      role: s.npc || s.role || '',
      lines: lines.map(String),
      keys: keys.map(String),
      options: options,
      done: !!(s.done || s.closed || s.end),
    };
  }
  function hasContent(s) { return s && (s.lines.length > 0 || s.options.length > 0); }
  // Session à jour après un choose (le moteur avance le nœud : ragots).
  // Contrat attendu du pont : chooseDialogue renvoie la session à jour ;
  // sinon un getter `dialogueSession()` ; sinon (action seule) échec doux.
  function fetchSession(g) {
    if (typeof g.dialogueSession === 'function') {
      try { return parseSession(g.dialogueSession()); } catch (e) { return null; }
    }
    return null;
  }

  // --- 23a : voix des PNJ (d2speech.mpq fr-FR v1.12) ---------------------------
  // Une voix RÉELLE par ligne affichée : index assets/sfx/speech/index.json
  // (clé TBL -> wav, généré par tools/extract_speech.py depuis les tables du
  // moteur — 91/91 clés, golden test tests/speech_golden.rs). DEFENSIF (même
  // famille que game-sfx.js) : index/wav absent = silence + note info, JAMAIS
  // d'erreur JS. Ducking de la musique via l'API publique D2Audio (bus musique
  // baissé pendant la voix, rendu à la fin) ; mute GLOBAL respecté (clé
  // partagée music.js) ; autoplay au premier geste (politique navigateur).
  var SPEECH_BASE = 'assets/sfx/speech/';
  var MUTE_KEY = 'd2wasm.musicMuted'; // clé partagée music.js / game-sfx.js
  var VOICE_VOLUME = 0.9;             // bus voix (la musique duck à 0.12)
  var DUCK_TARGET = 0.12;             // volume musique pendant la voix
  var speechIndex = null;             // { clé TBL -> wav basename }
  var speechIndexPromise = null;
  var speechBufs = new Map();         // wav -> Promise<AudioBuffer|null>
  var spActx = null, spBus = null;    // contexte audio voix (unlock 1er geste)
  var voiceSrc = null;                // source de la voix en cours
  var duckSaved = null;               // volume musique avant duck

  function mutedNow() {
    try { return localStorage.getItem(MUTE_KEY) === '1'; } catch (e) { return false; }
  }

  function unlockVoice() {
    try {
      if (!spActx) {
        var AC = window.AudioContext || window.webkitAudioContext;
        if (!AC) return;
        spActx = new AC();
        spBus = spActx.createGain();
        spBus.gain.value = VOICE_VOLUME;
        spBus.connect(spActx.destination);
      }
      if (spActx.state === 'suspended') spActx.resume().catch(function () {});
    } catch (e) { spActx = null; } // WebAudio indisponible : voix muettes
  }
  if (typeof window !== 'undefined') {
    window.addEventListener('pointerdown', unlockVoice, { passive: true });
    window.addEventListener('keydown', unlockVoice);
  }

  function speechIndexOf() {
    if (speechIndex) return Promise.resolve(speechIndex);
    if (!speechIndexPromise) {
      speechIndexPromise = fetch(SPEECH_BASE + 'index.json')
        .then(function (r) { return r.ok ? r.json() : null; })
        .catch(function () { return null; })
        .then(function (j) {
          speechIndex = j && j.lines && typeof j.lines === 'object' ? j.lines : {};
          if (!Object.keys(speechIndex).length) {
            console.info('[dialogue] index vocal indisponible — voix muettes (silence).');
          }
          return speechIndex;
        });
    }
    return speechIndexPromise;
  }

  function speechBuffer(file) {
    if (!speechBufs.has(file)) {
      speechBufs.set(file,
        fetch(SPEECH_BASE + file)
          .then(function (r) { if (!r.ok) throw new Error('HTTP ' + r.status); return r.arrayBuffer(); })
          .then(function (b) { return spActx.decodeAudioData(b); })
          .catch(function (e) {
            console.info('[dialogue] ' + file + ' indisponible (' +
              (e && e.message ? e.message : e) + ') — silence.');
            speechBufs.delete(file); // fichier revenu plus tard : on retentera
            return null;
          }));
    }
    return speechBufs.get(file);
  }

  /** Ducking : musique baissée pendant la voix (API publique D2Audio). */
  function duckMusic(on) {
    try {
      if (typeof window === 'undefined' || !window.D2Audio ||
          typeof window.D2Audio.setVolume !== 'function') return; // pas de mixage : la voix joue quand même
      if (on) {
        if (duckSaved == null) duckSaved = window.D2Audio.volume('music');
        window.D2Audio.setVolume('music', Math.min(duckSaved, DUCK_TARGET));
      } else if (duckSaved != null) {
        window.D2Audio.setVolume('music', duckSaved);
        duckSaved = null;
      }
    } catch (e) { /* cosmétique : jamais bloquer la voix */ }
  }

  function stopVoice() {
    if (voiceSrc) {
      try { voiceSrc.onended = null; voiceSrc.stop(); } catch (e) { /* déjà finie */ }
      voiceSrc = null;
    }
    duckMusic(false);
  }

  /** Joue la voix de la ligne affichée (première clé TBL connue de l'index). */
  function playVoice(keys) {
    stopVoice();
    if (!spActx || mutedNow() || !keys || !keys.length) return;
    var wanted = keys.slice();
    void (async function () {
      var idx = await speechIndexOf();
      var file = null;
      for (var i = 0; i < wanted.length && !file; i++) file = idx[wanted[i]] || null;
      if (!file || !spActx) return; // clé sans voix (TBD…) : silence propre
      var buf = await speechBuffer(file);
      if (!buf || mutedNow()) return;
      try {
        var src = spActx.createBufferSource();
        src.buffer = buf;
        var g = spActx.createGain();
        g.gain.value = 1; // le bus voix porte VOICE_VOLUME
        src.connect(g); g.connect(spBus);
        src.onended = function () {
          if (voiceSrc === src) { voiceSrc = null; duckMusic(false); }
        };
        voiceSrc = src;
        duckMusic(true);
        src.start();
      } catch (e) {
        console.info('[dialogue] lecture impossible — silence.');
      }
    })();
  }

  // --- accroche moteur : wrapper de tick (pause) + panneau + gardes d'input --
  function hook() {
    var g = game();
    if (hooked || !g || typeof g.isFinalized !== 'function' || !g.isFinalized()) return false;
    // PAUSE MONDE : tant qu'une session est ouverte, tick(0, null) — le
    // moteur gèle (héros, NPC, monstres, missiles) MAIS les events en attente
    // (NpcTalked, quêtes…) continuent de couler : world.tick prend
    // `pending_events` avant le pas dt (d2rules/src/world.rs).
    realTick = g.tick;
    g.tick = function (dt, inputJson) {
      if (session) return realTick.call(g, 0, null);
      return realTick.call(g, dt, inputJson);
    };
    hooked = g;
    if (pollIv) { clearInterval(pollIv); pollIv = null; }
    buildPanel();
    installGuards();
    return true;
  }

  function buildPanel() {
    var stage = document.getElementById('stage') || document.body;
    panel = document.createElement('div');
    panel.id = 'd2dlg';
    panel.innerHTML =
      '<div class="d2dlg-frame">' +
      '<div class="d2dlg-name"></div>' +
      '<div class="d2dlg-text"></div>' +
      '<div class="d2dlg-opts"></div>' +
      '<div class="d2dlg-pause">monde en pause · Échap = fermer</div>' +
      '</div>';
    panel.style.display = 'none';
    stage.appendChild(panel);
    elName = panel.querySelector('.d2dlg-name');
    elText = panel.querySelector('.d2dlg-text');
    elOpts = panel.querySelector('.d2dlg-opts');
    var css = document.createElement('style');
    css.textContent =
      '#d2dlg{position:absolute;left:50%;bottom:10px;transform:translateX(-50%);' +
      'width:600px;max-width:92%;z-index:6;pointer-events:auto;}' +
      '#d2dlg .d2dlg-frame{position:relative;background:rgba(16,12,6,.95);' +
      'border:2px solid #5a4a2a;outline:1px solid #000;padding:10px 16px 12px;' +
      'box-shadow:0 4px 18px #000d,inset 0 0 0 1px #2e2718;}' +
      '#d2dlg .d2dlg-name{text-align:center;color:#c7a45a;font:700 13px system-ui;' +
      'letter-spacing:.22em;text-transform:uppercase;text-shadow:0 1px 2px #000;' +
      'border-bottom:1px solid #3a3226;padding-bottom:5px;margin-bottom:7px;}' +
      '#d2dlg .d2dlg-text{color:#ffffff;font:12.5px/1.55 system-ui;text-align:center;' +
      'text-shadow:0 1px 2px #000;}' +
      '#d2dlg .d2dlg-line{display:block;margin:2px 0;}' +
      '#d2dlg .d2dlg-opts{margin-top:8px;text-align:center;}' +
      '#d2dlg .d2dlg-opt{display:block;width:100%;background:none;border:0;' +
      'padding:2px 6px;color:#ffffff;font:12.5px system-ui;cursor:pointer;' +
      'text-shadow:0 1px 2px #000;}' +
      '#d2dlg .d2dlg-opt:hover{text-decoration:underline;color:#ffe9ad;}' +
      '#d2dlg .d2dlg-pause{position:absolute;right:10px;top:8px;color:#8f8676;' +
      'font:italic 10px system-ui;letter-spacing:.04em;}';
    document.head.appendChild(css);
  }

  // Gardes d'input (phase capture sur document → avant game.js/hud.js) :
  // pendant la session, AUCUNE touche ni clic ne traverse vers le jeu —
  // sauf les clics DANS le panneau (options) et Échap (fermeture).
  function installGuards() {
    document.addEventListener('keydown', function (e) {
      if (!session) return;
      e.stopImmediatePropagation();
      if (e.key === 'Escape') {
        e.preventDefault();
        close(true);
      }
    }, true);
    var swallow = function (e) {
      if (!session) return;
      if (panel && panel.contains(e.target)) return; // le panneau reste cliquable
      e.preventDefault();
      e.stopImmediatePropagation();
    };
    ['mousedown', 'mouseup', 'click', 'dblclick', 'contextmenu'].forEach(function (t) {
      document.addEventListener(t, swallow, true);
    });
  }

  // --- rendu -------------------------------------------------------------------
  function render() {
    if (!panel) return;
    if (!session || !hasContent(session)) {
      panel.style.display = 'none';
      return;
    }
    panel.style.display = 'block';
    elName.textContent = session.name;
    elText.innerHTML = '';
    session.lines.forEach(function (l) {
      String(l).split('\n').forEach(function (part) {
        var d = document.createElement('span');
        d.className = 'd2dlg-line';
        d.textContent = part;
        elText.appendChild(d);
      });
    });
    elOpts.innerHTML = '';
    session.options.forEach(function (o) {
      var b = document.createElement('button');
      b.type = 'button';
      b.className = 'd2dlg-opt';
      b.textContent = o.text;
      b.addEventListener('click', function () { choose(o); });
      elOpts.appendChild(b);
    });
    // 23a : voix du NPC à l'affichage de la ligne (skip silencieux si le
    // fichier est absent — jamais d'erreur), musique duckée pendant.
    playVoice(session.keys);
  }

  // --- wire : ouverture / choix / fermeture ------------------------------------
  // Clic NPC (npc-ui.js doTalk) → openDialogue → rendu. Renvoie true si la
  // session est ouverte (le clic NPC est consommé), false sinon (npc-ui.js
  // garde son chemin W16 : bulle + approche).
  function tryOpen(npcIndex) {
    var g = hooked || game();
    if (!g || !available(g) || session) return false;
    var raw;
    try { raw = g.openDialogue(npcIndex); } catch (e) { return false; } // « trop loin » : npc-ui note + marche
    var s = parseSession(raw);
    if (!s || (!hasContent(s) && !s.done)) return false;
    session = s.done ? null : s;
    render();
    if (session) status('Dialogue avec ' + session.name + ' — monde en pause (Échap = fermer).');
    return true;
  }

  function choose(o) {
    var g = hooked || game();
    if (!g || !session) return;
    var raw;
    try { raw = g.chooseDialogue(o.index); } // INDEX BRUT dans node.options (contrat moteur)
    catch (e) { note('Dialogue : ' + emsg(e)); return; }
    var parsed, okParse = false;
    try { parsed = typeof raw === 'string' ? JSON.parse(raw) : raw; okParse = true; } catch (e) { parsed = null; }
    // `null` moteur = choix refusé (condition quête non remplie) ou session
    // absente — on reste sur le nœud courant, sans fermer.
    if (okParse && parsed == null) return;
    var act = parsed && typeof parsed === 'object'
      ? actionOf(parsed.action || parsed) : null; // {action:{kind…}} ou {kind…}
    var k = act ? act.kind : '';
    // Actions → UI existante (mission 22c §2). open_shop/travel FERMENT la
    // session (comme le retail : choisir de commercer quitte le dialogue).
    if (/(open_shop|shop|vendor|trade|store|gamble)/.test(k)) {
      openVendor(act.arg);
      close(true);
      return;
    }
    if (/(travel|waypoint|voyage)/.test(k)) {
      openWaypoints();
      close(true);
      return;
    }
    if (/(hire_mercenary|hire)/.test(k)) {
      note('Engagement de recrue : l’API moteur est prête (écran d’embauche à venir).');
      return; // nœud inchangé — le dialogue reste ouvert
    }
    if (/(repair|reparer)/.test(k)) {
      note('Rien à réparer pour l’instant (durabilité à venir).');
      return;
    }
    if (/(close|bye|leave|farewell|end|partir)/.test(k)) {
      close(true);
      return;
    }
    // say (ragots « dites-m'en plus ») ou action inconnue : le moteur a
    // avancé le nœud — rafraîchir (choose renvoie la session à jour, sinon
    // le getter `dialogueSession()`).
    var next = (parsed && typeof parsed === 'object' && parsed.node)
      ? parseSession(parsed)
      : fetchSession(g);
    if (next && !next.done && hasContent(next)) {
      session = next;
      render();
    }
  }

  function openVendor(vendor) {
    // Panneau Marchands existant (demo/vendor.js, bouton #btn-vendors).
    if (typeof window !== 'undefined' && window.D2Npc &&
        typeof window.D2Npc.openVendorTab === 'function') {
      window.D2Npc.openVendorTab(vendor);
      return;
    }
    if (typeof document === 'undefined') return;
    var btn = document.getElementById('btn-vendors');
    var p = document.getElementById('d2vendor');
    if (!btn || !p) { note('Panneau Marchands indisponible.'); return; }
    if (p.style.display !== 'block') btn.click();
    if (vendor) {
      var want = vendor === 'Gheed' ? 'Gamble (Gheed)' : vendor;
      var tabs = p.querySelectorAll('.tabs button');
      for (var i = 0; i < tabs.length; i++) {
        if (tabs[i].textContent.indexOf(want) === 0) { tabs[i].click(); break; }
      }
    }
  }

  function openWaypoints() {
    // Panneau waypoints existant (demo/waypoints.js).
    if (typeof window !== 'undefined' && window.D2Waypoints &&
        typeof window.D2Waypoints.setOpen === 'function') {
      window.D2Waypoints.setOpen(true);
      return;
    }
    if (typeof document === 'undefined') return;
    var b = document.getElementById('btn-waypoints');
    if (b) b.click();
    else note('Panneau waypoints indisponible.');
  }

  // Échap / option de fin : ferme (session + endpoint moteur), reprend le
  // monde (le wrapper de tick re-passe dt réel dès `session === null`).
  function close(withEngine) {
    if (!session) return;
    session = null;
    stopVoice(); // 23a : voix coupée + musique dé-duckée à la fermeture
    render();
    var g = hooked || game();
    if (withEngine && g && typeof g.closeDialogue === 'function') {
      try { g.closeDialogue(); } catch (e) { /* déjà fermée côté moteur */ }
    }
  }

  // --- capture-texte du panneau (recette + node --check hors navigateur) ------
  function panelTextLines(s) {
    var out = [String(s.name || 'PNJ').toUpperCase()];
    s.lines.forEach(function (l) {
      String(l).split('\n').forEach(function (x) { out.push(x); });
    });
    s.options.forEach(function (o) { out.push(o.text); });
    out.push('monde en pause · Échap = fermer');
    return out;
  }

  var api = {
    tryOpen: tryOpen,
    close: function () { close(true); },
    isOpen: function () { return !!session; },
    _choose: choose, // interne (harnais de test node — même flux qu'un clic)
    // Capture-texte du panneau (contenu EXACT rendu) — tests/recette.
    _debugText: function () { return session ? panelTextLines(session).join('\n') : ''; },
    _capture: function (sessionJson) {
      var s = parseSession(sessionJson); // tolère JSON string ET objet déjà parsé
      return s ? panelTextLines(s).join('\n') : '';
    },
    _parse: parseSession,
    _state: function () { return { hooked: !!hooked, session: session }; },
    // 23a : état voix (recette + harnais node) — index chargé, voix en
    // cours, musique duckée.
    _voiceState: function () {
      return { loaded: !!speechIndex, playing: !!voiceSrc, ducked: duckSaved != null };
    },
    _stopVoice: stopVoice, // interne (fermeture forcée des harnais)
  };
  if (typeof window !== 'undefined') window.D2Dialogue = api;
  if (typeof module !== 'undefined' && module.exports) module.exports = api;

  // Auto-accroche (game.js ne fait qu'importer le module — 1 ligne).
  if (typeof document !== 'undefined') {
    if (!hook()) pollIv = setInterval(hook, POLL_MS);
  }
})();
