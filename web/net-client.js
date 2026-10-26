// net-client.js — client lockstep (phase 3a, plan serveur-coop ; 3b multi-
// héros ; 3c départs en jeu ; 4a robustesse : reconnexion, spectateur,
// garde au camp). Monde miroir : TOUS les clients exécutent le même monde
// déterministe (d2rules) au même tick de 20 Hz avec les mêmes entrées.
// 3b : CHAQUE joueur possède le héros de SON slot — ses entrées locales
// partent sans étiquette (op 0x10) et le relais les attribue à SON slot ;
// les bundles reviennent PAR SLOT (op 0x11) et chaque entrée est exécutée
// sur le héros de son slot (file par slot + un seul tick, ou tickSlot).
// 3c : l'entrée synthétique {type:"leave"} (RETRAIT EXPLICITE d'un joueur,
// relais 3c) retire SON héros au même tick chez tous (removePlayer —
// compaction déterministe ; NOTRE slot moteur suit la compaction, caméra
// comprise). En 4a, la simple DÉCONNEXION n'émet plus « leave » : le héros
// du joueur ABSENT RESTE dans le monde (IA idle — aucune entrée sur son
// slot), la reconnexion (même token, même slot) rejoue les bundles
// manquants et/ou applique le snapshot de l'hôte.
// 4a — reconnexion : connect() repart du monde EN MÉMOIRE s'il existe
// (execTick > 1) : le hello porte `resume` (dernier tick exécuté) ; le
// relais renvoie les bundles manquants seuls (replay) quand le retard
// tient dans le journal, sinon un snapshot 0x25 arrive et EST APPLIQUÉ
// même si le client était déjà « synced ». Un client neuf (page rechargée,
// execTick == 1) garde le chemin 3b : snapshot + replay.
// 4a — spectateur : l'opcode 0x26 du relais bascule la page en mode
// spectateur (siège sans héros, caméra libre) : le client ne CAPTURE plus
// et n'ÉMET plus aucune entrée — il continue d'exécuter le lockstep pour
// AFFICHER le monde.
// 4a — garde au camp : l'entrée synthétique {type:"guard_camp"} (héros
// orphelin depuis > 5 min, injectée par le relais dans le bundle) est
// résolue en MoveTo vers le spawn du héros (save v4 `players[slot].spawn`,
// identique chez tous les clients — déterministe) : le héros rentre au
// camp et y monte la garde ; il n'est ni tué ni supprimé.
// Buffer de jitter de 3 ticks, hash FNV-1a de saveGame() toutes les 60
// ticks, rompage propre au moindre écart.
//
// Contrat : window.D2Net — active(), pump(), connect(opts), status().
// Script classique chargé par game.html AVANT game.js.
(function () {
  'use strict';

  var TICK_MS = 50;         // 20 Hz, identique serveur
  var LAG_TICKS = 3;        // buffer de jitter (150 ms de latence d'entrée)
  var STALL_MS = 2500;      // sans bundle au-delà : connexion perdue
  var HASH_EVERY = 60;      // ticks entre deux hashes (~3 s)

  var ws = null;
  var on = false;
  var host = false;
  var code = null;
  var mySlot = 0;           // 3b : NOTRE slot (opts.slot, /meta ; E2E/statut)
  var spectator = false;    // 4a : mode spectateur (0x26) — aucune entrée
  var resumed = false;      // 4a : reprise avec monde en mémoire (hello resume)

  var sendTick = 0;         // tick d'émission local (fenêtre 50 ms)
  var localBuf = [];        // événements capturés dans la fenêtre courante
  var lastSend = 0;

  var execTick = 1;         // prochain tick à exécuter
  var bundles = {};         // tick -> [{slot, events}]
  var lastArrival = 0;
  var desynced = false;
  var synced = false;       // snapshot reçu (ou hôte : monde neuf)

  var hooks = { onFlash: function () {}, onStatus: function () {}, queue: null, getGame: function () { return null; } };

  // ---------------- encodage ----------------

  function pack(op, bytes) {
    var out = new Uint8Array(1 + (bytes ? bytes.length : 0));
    out[0] = op;
    if (bytes) out.set(bytes, 1);
    return out;
  }
  function u16(n) { return new Uint8Array([n & 255, (n >> 8) & 255]); }
  function text(s) { return new TextEncoder().encode(s); }
  function jsonFrame(op, obj) {
    var b = text(JSON.stringify(obj));
    var out = new Uint8Array(3 + b.length);
    out[0] = op; out.set(u16(b.length), 1); out.set(b, 3);
    return out;
  }

  function parseBundle(buf) {
    var v = new DataView(buf);
    var tick = v.getUint32(1, true);
    var cnt = v.getUint8(5);
    var o = 6;
    var entries = [];
    for (var i = 0; i < cnt; i++) {
      var slot = v.getUint8(o);
      var len = v.getUint16(o + 1, true);
      entries.push({ slot: slot, events: JSON.parse(new TextDecoder().decode(new Uint8Array(buf, o + 3, len))) });
      o += 3 + len;
    }
    return { tick: tick, entries: entries };
  }

  // FNV-1a 32 bits sur la sauvegarde complète de l'état (bit-exact).
  function stateHash(saveJson) {
    var h = 0x811c9dc5;
    for (var i = 0; i < saveJson.length; i++) {
      h ^= saveJson.charCodeAt(i) & 0xff;
      h = (h + ((h << 1) + (h << 4) + (h << 7) + (h << 8) + (h << 24))) >>> 0;
    }
    return h >>> 0;
  }

  // ---------------- API ----------------

  function active() { return on && ws && ws.readyState === 1 && !desynced; }

  function connect(opts) {
    hooks = Object.assign(hooks, opts || {});
    code = opts.code;
    host = !!opts.host;
    mySlot = typeof opts.slot === 'number' ? (opts.slot >= 0 ? opts.slot : -1) : 0; // -1 = spectateur (4a)
    // 4a : un second connect() sur la même page (reconnexion — même token,
    // même slot côté relais) repart du monde EN MÉMOIRE : le hello annonce
    // `resume` (dernier tick exécuté) et le client reste « synced » ; le
    // relais renvoie les bundles manquants (replay) ou un snapshot 0x25.
    resumed = !host && execTick > 1;
    spectator = false;
    var proto = location.protocol === 'https:' ? 'wss://' : 'ws://';
    ws = new WebSocket(proto + location.host + '/api/ws?code=' + encodeURIComponent(code) +
      '&token=' + encodeURIComponent(opts.token || ''));
    ws.binaryType = 'arraybuffer';
    ws.onopen = function () {
      on = true;
      synced = !!host || resumed; // hôte : monde neuf ; reprise 4a : monde en mémoire ; sinon snapshot requis
      ws.send(jsonFrame(0x01, { hero: opts.hero || '', name: opts.name || '', resume: resumed ? execTick - 1 : 0 }));
      hooks.onFlash(host
        ? 'Connecté au salon ' + code + ' — votre monde pilote le lockstep 20 Hz.'
        : resumed
          ? 'Reconnecté au salon ' + code + ' — rattrapage des ticks manquants…'
          : 'Connecté au salon ' + code + ' — synchronisation avec le monde du salon…');
    };
    ws.onmessage = function (e) {
      if (!(e.data instanceof ArrayBuffer)) return;
      var v = new DataView(e.data);
      var op = v.getUint8(0);
      if (op === 0x11) { // bundle
        var b = parseBundle(e.data);
        bundles[b.tick] = b.entries;
        lastArrival = performance.now();
      } else if (op === 0x25) { // snapshot de l'hôte [tick:u32][len:u32][json]
        // 4a : appliqué par un client en attente (chemin 3b) OU par une
        // reprise `resume` dont le retard dépassait le journal — le relais
        // ne laisse le snapshot partir que pour les sièges en demande.
        if (host) return; // écho reçu par l'hôte
        if (synced && !resumed) return; // déjà synchronisé (et pas en reprise)
        try {
          var snapTick = v.getUint32(1, true);
          var len = v.getUint32(5, true);
          var json = new TextDecoder().decode(new Uint8Array(e.data, 9, len));
          var game = hooks.getGame();
          if (game) game.loadGame(json);
          for (var t in bundles) { if (+t <= snapTick) delete bundles[t]; }
          execTick = snapTick + 1;
          synced = true;
          resumed = false;
          hooks.onFlash('Monde du salon synchronisé (tick ' + snapTick + ').');
        } catch (err) {
          desynced = true;
          hooks.onFlash('Snapshot illisible : ' + (err && err.message));
        }
      } else if (op === 0x26) { // 4a : mode spectateur (siège sans héros)
        spectator = true;
        hooks.onFlash('Mode spectateur : vous observez la partie (caméra libre, aucune entrée).');
      } else if (op === 0x23) { // demande de snapshot (hôte uniquement)
        if (!host) return;
        try {
          catchUp(); // l'état photographié doit être à jour (onglet possiblement endormi)
          var game = hooks.getGame();
          var snapTick = Math.max(execTick - 1, 0); // état = après ce tick
          var jb = new TextEncoder().encode(game.saveGame());
          var out = new Uint8Array(9 + jb.length);
          var vo = new DataView(out.buffer);
          vo.setUint8(0, 0x24);
          vo.setUint32(1, snapTick, true);
          vo.setUint32(5, jb.length, true);
          out.set(jb, 9);
          ws.send(out);
        } catch (err) { /* snapshot impossible : l'invité restera en attente */ }
      } else if (op === 0x20) { // desync
        desynced = true;
        hooks.onFlash('DÉSYNCHRONISATION détectée au tick ' + v.getUint32(1, true) + ' — partie rompue.');
        try { ws.close(); } catch (err) { /* déjà fermée */ }
      } else if (op === 0x21) { // hôte perdu
        desynced = true;
        hooks.onFlash("L'hôte a quitté — fin de la partie en ligne.");
        try { ws.close(); } catch (err) { /* déjà fermée */ }
      }
    };
    ws.onclose = function () {
      if (on && !desynced) hooks.onFlash('Salon déconnecté.');
      on = false;
    };
    lastArrival = performance.now();
  }

  /** Boucle de rendu (60 fps) : capte les entrées locales, émet à 20 Hz,
   *  exécute AU PLUS UN tick lockstep dû et renvoie ses événements (null si
   *  aucun tick prêt — la frame ne fait que rendre l'état courant). */
  function pump() {
    if (!active()) return undefined;
    var now = performance.now();

    // 1) capture : tout ce que le joueur a produit depuis la dernière
    //    fenêtre — 4a : JAMAIS en spectateur (aucune entrée).
    if (!spectator) {
      var q = hooks.queue || [];
      while (q.length) localBuf.push(q.shift());

      // 2) émission à 20 Hz
      if (now - lastSend >= TICK_MS) {
        lastSend = now;
        ws.send(jsonFrame(0x10, localBuf));
        localBuf = [];
      }
    }

    // 3) exécution des ticks dus (avec LAG_TICKS de repli) — un invité
    // n'exécute rien tant que le snapshot de l'hôte n'est pas appliqué.
    // Retour d'onglet : jusqu'à 8 ticks par frame pour rattraper vite.
    if (!synced) return null;
    var all = null;
    for (var burst = 0; burst < 8; burst++) {
      var target = maxTick();
      if (target === 0 || execTick > target - LAG_TICKS) break;
      var entries = bundles[execTick];
      if (!entries) break; // trou d'historique
      delete bundles[execTick];
      var tick = execTick;
      execTick += 1;
      var evs = applyTick(entries);
      if (all === null) all = evs;
      else all.push.apply(all, evs);
      if (tick % HASH_EVERY === 0) sendHash(tick);
      hooks.onStatus('tick ' + tick);
    }
    if (all === null) checkStall(now);
    return all;
  }

  /** Applique UN tick lockstep : entrées de tous les slots (3b : CHAQUE
   *  entrée est exécutée sur le héros de SON slot). Chemin attendu — file
   *  PAR SLOT : chaque entrée est empilée sur son slot (game.queueInput),
   *  puis UN SEUL tick (game.tickQueued) consomme tout avec dt = 50 ms
   *  (slots croissants, FIFO par slot — déterministe). 3c : une entrée
   *  {type:"leave"} (retrait EXPLICITE d'un joueur — relais 3c) RETIRE le
   *  héros de ce slot — game.removePlayer(slot) met le retrait en file,
   *  appliqué au MÊME tick chez tous (compaction déterministe des slots) ;
   *  NOTRE slot suit la compaction (caméra comprise). La simple
   *  DÉCONNEXION (4a) n'émet plus « leave » : le héros absent reste en
   *  monde (idle). 4a : {type:"guard_camp"} (héros orphelin > 5 min) est
   *  résolue en MoveTo vers le spawn du héros (save v4 players[slot].spawn
   *  — identique chez tous : déterminisme lockstep) ; le héros n'est ni
   *  tué ni supprimé, il monte la garde au camp. Replis : tickSlot par
   *  entrée (dt découpé), puis game.tick (moteur 3a, miroir). */
  function applyTick(entries) {
    var game = hooks.getGame();
    var all = [];
    if (!game) return all;
    var inputs = [];
    for (var i = 0; i < entries.length; i++) {
      var slot = entries[i].slot;
      var arr = entries[i].events || [];
      for (var j = 0; j < arr.length; j++) inputs.push([slot, arr[j]]);
    }
    if (typeof game.queueInput === 'function' && typeof game.tickQueued === 'function') {
      for (var q = 0; q < inputs.length; q++) {
        var s = inputs[q][0], ev = inputs[q][1];
        if (ev && ev.type === 'leave') {
          if (typeof game.removePlayer === 'function') {
            game.removePlayer(s); // retrait en file — appliqué au prochain tickQueued
            if (mySlot > s) {
              // compaction : les slots > s décalent d'un cran — NOTRE héros
              // change de slot moteur, la caméra le suit (présentation seule).
              mySlot -= 1;
              if (typeof game.setCameraSlot === 'function') game.setCameraSlot(mySlot);
            }
            hooks.onFlash('Le joueur du slot ' + s + ' a quitté la partie.');
          }
          continue; // pas une entrée de jeu — jamais empilée sur la file
        }
        if (ev && ev.type === 'guard_camp') {
          // 4a : héros orphelin (déco > 5 min) → garde au camp. Le spawn
          // vient de la save du monde (v4 : players[slot].spawn = [x, y]) —
          // les mondes étant bit-exacts chez tous les clients, la cible
          // MoveTo est IDENTIQUE partout (déterminisme).
          try {
            var save = JSON.parse(game.saveGame());
            var sp = save.players && save.players[s] ? save.players[s].spawn : null;
            if (sp && sp.length >= 2) {
              game.queueInput(s, JSON.stringify({ type: 'moveTo', x: Math.floor(sp[0]), y: Math.floor(sp[1]) }));
              hooks.onFlash('Le héros du slot ' + s + ' (absent > 5 min) rentre garder le camp.');
            }
          } catch (e) { /* save illisible : le héros reste où il est */ }
          continue; // pas une entrée de jeu — jamais empilée sur la file
        }
        game.queueInput(s, JSON.stringify(ev));
      }
      return JSON.parse(game.tickQueued(TICK_MS));
    }
    var k = inputs.length;
    var base = Math.floor(TICK_MS / (k + 1));
    var spent = 0;
    var useSlot = typeof game.tickSlot === 'function';
    for (var j2 = 0; j2 < k; j2++) {
      var evs2 = useSlot
        ? game.tickSlot(base, inputs[j2][0], JSON.stringify(inputs[j2][1]))
        : game.tick(base, JSON.stringify(inputs[j2][1])); // repli moteur 3a (miroir)
      all.push.apply(all, JSON.parse(evs2));
      spent += base;
    }
    all.push.apply(all, JSON.parse(game.tick(TICK_MS - spent, null)));
    return all;
  }

  /** Rattrape TOUS les ticks dus (hors boucle de rendu) — utilisé avant un
   *  snapshot : l'état photographié doit correspondre à un tick récent, même
   *  si l'onglet hôte était en arrière-plan (rAF suspendu). */
  function catchUp() {
    var guard = 0;
    while (guard++ < 20000) {
      var target = maxTick();
      if (target === 0 || execTick > target - LAG_TICKS) break;
      var entries = bundles[execTick];
      if (!entries) break; // trou d'historique : on s'arrête ici
      delete bundles[execTick];
      var tick = execTick;
      execTick += 1;
      applyTick(entries);
      if (tick % HASH_EVERY === 0) sendHash(tick);
    }
  }

  function sendHash(tick) {
    try {
      var game = hooks.getGame();
      var h = stateHash(game.saveGame());
      var f = new Uint8Array(9);
      var v = new DataView(f.buffer);
      v.setUint8(0, 0x12);
      v.setUint32(1, tick, true);
      v.setUint32(5, h, true);
      ws.send(f);
    } catch (e) { /* hash sauté, pas bloquant */ }
  }

  function maxTick() {
    var m = 0;
    for (var t in bundles) { var n = +t; if (n > m) m = n; }
    return m;
  }

  function checkStall(now) {
    if (!desynced && now - lastArrival > STALL_MS) {
      desynced = true;
      hooks.onFlash('Connexion au salon perdue (plus de bundles).');
    }
  }

  function status() {
    return {
      on: on, host: host, code: code, tick: execTick - 1, buffered: maxTick() - execTick, desynced: desynced,
      // 3b diagnostics (E2E) : NOTRE slot, cible de rattrapage + buffer brut
      slot: mySlot, exec: execTick, target: maxTick(), bundles: Object.keys(bundles).length, synced: synced,
      // 4a : mode spectateur (0x26) + reprise avec monde en mémoire
      spectator: spectator, resumed: resumed,
    };
  }

  window.D2Net = { active: active, connect: connect, pump: pump, status: status };
})();
