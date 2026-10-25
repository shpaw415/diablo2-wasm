// net-client.js — client lockstep (phase 3a, plan serveur-coop).
// Monde miroir : TOUS les clients exécutent le même monde déterministe
// (d2rules) au même tick de 20 Hz avec les mêmes entrées — l'hôte publie le
// héros, chaque joueur peut le piloter (les héros multiples = phase 3b,
// chantier moteur). Buffer de jitter de 3 ticks, hash FNV-1a de saveGame()
// toutes les 60 ticks, rompage propre au moindre écart.
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
    var proto = location.protocol === 'https:' ? 'wss://' : 'ws://';
    ws = new WebSocket(proto + location.host + '/api/ws?code=' + encodeURIComponent(code) +
      '&token=' + encodeURIComponent(opts.token || ''));
    ws.binaryType = 'arraybuffer';
    ws.onopen = function () {
      on = true;
      synced = !!host; // l'hôte démarre sur son monde neuf ; les invités attendent un snapshot
      ws.send(jsonFrame(0x01, { hero: opts.hero || '', name: opts.name || '' }));
      hooks.onFlash(host
        ? 'Connecté au salon ' + code + ' — votre monde pilote le lockstep 20 Hz.'
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
        if (synced || host) return; // déjà synchronisé, ou écho reçu par l'hôte
        try {
          var snapTick = v.getUint32(1, true);
          var len = v.getUint32(5, true);
          var json = new TextDecoder().decode(new Uint8Array(e.data, 9, len));
          var game = hooks.getGame();
          if (game) game.loadGame(json);
          for (var t in bundles) { if (+t <= snapTick) delete bundles[t]; }
          execTick = snapTick + 1;
          synced = true;
          hooks.onFlash('Monde du salon synchronisé (tick ' + snapTick + ').');
        } catch (err) {
          desynced = true;
          hooks.onFlash('Snapshot illisible : ' + (err && err.message));
        }
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

    // 1) capture : tout ce que le joueur a produit depuis la dernière fenêtre
    var q = hooks.queue || [];
    while (q.length) localBuf.push(q.shift());

    // 2) émission à 20 Hz
    var now = performance.now();
    if (now - lastSend >= TICK_MS) {
      lastSend = now;
      ws.send(jsonFrame(0x10, localBuf));
      localBuf = [];
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

  /** Applique UN tick lockstep : entrées de tous les slots dans l'ordre des
   *  slots, puis UNE clôture — dt total = 50 ms, découpage entier (déterministe). */
  function applyTick(entries) {
    var game = hooks.getGame();
    var all = [];
    if (!game) return all;
    var evs = [];
    for (var i = 0; i < entries.length; i++) {
      var arr = entries[i].events || [];
      for (var j = 0; j < arr.length; j++) evs.push(arr[j]);
    }
    var k = evs.length;
    var base = Math.floor(TICK_MS / (k + 1));
    var spent = 0;
    for (var j2 = 0; j2 < k; j2++) {
      all.push.apply(all, JSON.parse(game.tick(base, JSON.stringify(evs[j2]))));
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
    return { on: on, host: host, code: code, tick: execTick - 1, buffered: maxTick() - execTick, desynced: desynced };
  }

  window.D2Net = { active: active, connect: connect, pump: pump, status: status };
})();
