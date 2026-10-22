// Diablo 2 WASM — glue navigateur.
// Le décodage/composition/rendu/simulation vivent dans Rust (demo/game-pkg).
// Ce fichier : fetch des assets, boucle putImageData, HUD, sons, sauvegarde,
// et un fallback JS visible si le moteur WASM ne charge pas.
import init, { D2Game } from './game-pkg/d2game.js';

const $ = (id) => document.getElementById(id);
const status = $('status');
const canvas = $('game');
const ctx = canvas.getContext('2d');
const errlog = $('errlog');

// --- capture d'erreurs (E2E : window.__d2.errors doit rester vide) ----------
window.__d2 = { mode: 'boot', errors: [], events: 0 };
function logErr(msg) {
  window.__d2.errors.push(String(msg));
  errlog.style.display = 'block';
  const line = document.createElement('div');
  line.textContent = `[${new Date().toISOString().slice(11, 19)}] ${msg}`;
  errlog.appendChild(line);
  errlog.scrollTop = errlog.scrollHeight;
}
window.addEventListener('error', (e) => {
  logErr(e.message);
  fail('Erreur JS', e.error || e.message);
});
window.addEventListener('unhandledrejection', (e) => {
  logErr('rejection: ' + (e.reason && e.reason.message ? e.reason.message : e.reason));
  fail('Promesse rejetée', e.reason);
});
// Sonde E2E : tout console.error (panics WASM inclus) compte comme erreur —
// visible dans window.__d2.errors et dans le titre de l'onglet (⚠ N).
const _consoleError = console.error.bind(console);
console.error = (...a) => {
  logErr(a.map((x) => (x && x.message) || String(x)).join(' '));
  syncTitleErrors();
};
function syncTitleErrors() {
  const n = window.__d2.errors.length;
  const clean = 'Diablo 2 WASM — Camp des Rousses';
  document.title = n ? `${clean} ⚠ ${n} erreur${n > 1 ? 's' : ''}` : clean;
}

function fail(where, e) {
  const msg = `${where} : ${e && e.message ? e.message : e}`;
  status.textContent = msg;
  status.classList.add('err');
  console.error(msg);
}

const ROOT = new URL('..', location); // racine projet (chemins du manifest)
const SAVE_KEY = 'd2wasm.save.v1';
// Sélection de personnage : ?hero=BA|SOR|PA (défaut BA). La résolution
// effective (heroes[code] sinon bloc hero) est faite côté moteur Rust —
// ci-dessous on reflète la même règle pour les assets et le HUD.
const HERO_REQ = (new URLSearchParams(location.search).get('hero') || 'BA').toUpperCase();

function resolveHero(manifest, requested) {
  if (manifest.heroes && Object.prototype.hasOwnProperty.call(manifest.heroes, requested)) {
    return { code: requested, def: manifest.heroes[requested], fallback: false };
  }
  const def = manifest.hero && typeof manifest.hero === 'object' ? manifest.hero : {};
  const code = def.class || 'BA';
  return { code, def, fallback: code !== requested };
}
// patch_experience.txt, colonne Barbarian, seuils CUMULÉS niv 1..10 (XP_TABLE d2rules).
const XP_THR = [0, 500, 1500, 3750, 7875, 14175, 22680, 32886, 44396, 57715, 72144];

async function fetchBytes(path) {
  const r = await fetch(new URL(path, ROOT));
  if (!r.ok) throw new Error(`fetch ${path} -> HTTP ${r.status}`);
  return new Uint8Array(await r.arrayBuffer());
}

function collectPaths(manifest, heroDef) {
  const paths = new Set();
  paths.add(manifest.level.ds1);
  paths.add(manifest.level.palette);
  for (const p of manifest.level.dt1) paths.add(p);
  const walkSets = [heroDef.walk, ...manifest.monsters.map((m) => m.walk)];
  for (const w of walkSets) {
    paths.add(w.cof);
    for (const p of Object.values(w.layers || {})) paths.add(p);
  }
  return [...paths];
}

// --- audio (WebAudio, décodage des WAV du manifest) -------------------------
const audio = { ctx: null, buffers: new Map(), enabled: true, last: new Map() };
function acx() {
  if (!audio.ctx) {
    try { audio.ctx = new (window.AudioContext || window.webkitAudioContext)(); }
    catch { audio.enabled = false; }
  }
  if (audio.ctx && audio.ctx.state === 'suspended') audio.ctx.resume().catch(() => {});
  return audio.ctx;
}
async function loadSfx(manifest) {
  const paths = new Set();
  const add = (p) => { if (typeof p === 'string') paths.add(p); };
  Object.values(manifest.sfx || {}).forEach((v) => {
    if (typeof v === 'string') add(v);
    else if (v && typeof v === 'object') Object.values(v).forEach(add);
  });
  for (const p of paths) {
    try {
      const bytes = await fetchBytes(p);
      const buf = await acx().decodeAudioData(bytes.buffer);
      audio.buffers.set(p, buf);
    } catch (e) { logErr(`sfx ${p}: ${e.message}`); }
  }
}
function sfx(key, vol = 0.5, rate = 1) {
  if (!audio.enabled || !key) return;
  const c = acx();
  const buf = audio.buffers.get(key);
  if (!c || !buf) return;
  const now = performance.now();
  if (now - (audio.last.get(key) || 0) < 70) return; // anti-spam
  audio.last.set(key, now);
  const src = c.createBufferSource();
  src.buffer = buf;
  src.playbackRate.value = rate;
  const g = c.createGain();
  g.gain.value = vol;
  src.connect(g).connect(c.destination);
  src.start();
}

// --- HUD --------------------------------------------------------------------
let HERO_NAME = 'Barbare'; // surchargé une fois le manifest lu
const hud = {
  hp: $('hpfill'), hpt: $('hptext'), mp: $('mpfill'), mpt: $('mptext'),
  lvl: $('lvltext'), gold: $('goldtext'), xp: $('xpfill'),
};
function updateHud(s) {
  if (!s || s.max_life === undefined) return;
  hud.hp.style.width = `${(100 * s.life) / Math.max(1, s.max_life)}%`;
  hud.hpt.textContent = `${s.life}/${s.max_life}`;
  hud.mp.style.width = `${(100 * s.mana) / Math.max(1, s.max_mana)}%`;
  hud.mpt.textContent = `${s.mana}/${s.max_mana}`;
  hud.lvl.textContent = `${HERO_NAME} · Niv. ${s.level}`;
  hud.gold.textContent = `Or : ${s.gold}`;
  const lvl = Math.min(s.level, 10);
  const base = XP_THR[lvl - 1] ?? 0, next = XP_THR[lvl] ?? XP_THR[10];
  const frac = next > base ? (s.xp - base) / (next - base) : 1;
  hud.xp.style.width = `${Math.max(0, Math.min(100, 100 * frac))}%`;
}

function flash(msg) {
  status.classList.remove('err');
  status.textContent = status.textContent.replace(/ · \d+ fps.*$/, '') + ' — ' + msg;
  const log = $('msglog');
  if (log) {
    const line = document.createElement('div');
    line.textContent = `• ${msg}`;
    log.appendChild(line);
    while (log.children.length > 12) log.removeChild(log.firstChild);
    log.scrollTop = log.scrollHeight;
  }
}

// --- événements gameplay → sons + messages ----------------------------------
function handleEvents(events, manifest) {
  for (const ev of events) {
    window.__d2.events++;
    if (ev.HeroAttacked) {
      sfx(manifest.sfx.attack);
      flash('Attaque !');
    } else if (ev.HeroMissed) {
      sfx(manifest.sfx.attack, 0.25, 1.4);
    } else if (ev.HeroHit) {
      sfx(manifest.sfx.hit);
    } else if (ev.MonsterDied) {
      const code = manifest.monsters[ev.MonsterDied.index]?.code;
      const die = (code && manifest.sfx.monsters[code] && manifest.sfx.monsters[code].die) || manifest.sfx.die;
      sfx(die);
      flash(`Monstre tué (+${ev.MonsterDied.xp} xp) — un butin est peut-être au sol !`);
    } else if (ev.ItemDropped) {
      const it = ev.ItemDropped.item;
      flash(`Butin : ${it.name}${it.amount ? ` (${it.amount} or)` : ''}`);
    } else if (ev.HeroLeveled) {
      sfx(manifest.sfx.uiClick, 0.7, 1.5);
      flash(`NIVEAU ${ev.HeroLeveled.level} !`);
    } else if (ev.GoldGained) {
      sfx(manifest.sfx.uiClick, 0.4, 1.2);
      flash(`+${ev.GoldGained.amount} or`);
    } else if (ev.HeroPickedUp) {
      sfx(manifest.sfx.uiClick, 0.5, 1.1);
      flash('Objet ramassé.');
    } else if (ev.PotionUsed) {
      sfx(manifest.sfx.uiClick, 0.5, 0.8);
      flash(`Potion bue (+${ev.PotionUsed.heal} pv)`);
    } else if (ev.InventoryFull) {
      flash('Inventaire plein — objet laissé au sol.');
    } else if (ev.HeroDied) {
      sfx(manifest.sfx.hit, 0.8, 0.6);
      flash('Vous êtes mort… résurrection au camp dans 2 s.');
    } else if (ev.HeroRespawned) {
      flash('Résurrection au Camp des Rousses.');
    }
  }
}

// --- boucle commune ---------------------------------------------------------
const loop = { pendingInput: null, lastClick: null, last: performance.now(), frames: 0, fpsClock: performance.now() };
function clickToInput(game, e) {
  const r = canvas.getBoundingClientRect();
  const px = (e.clientX - r.left) * (canvas.width / r.width);
  const py = (e.clientY - r.top) * (canvas.height / r.height);
  const [wx, wy] = game.screenToWorld(px, py);
  loop.lastClick = { sx: px, sy: py, t: performance.now() };
  return game.monsterNear(wx, wy, 2.5) >= 0
    ? { type: 'attack', x: Math.round(wx), y: Math.round(wy) }
    : { type: 'move', x: Math.round(wx), y: Math.round(wy) };
}
function drawClickRing(x, y, k) {
  ctx.save();
  ctx.strokeStyle = `rgba(255,220,140,${0.8 * k})`;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.ellipse(x, y, 14 * (1.4 - 0.4 * k), 7 * (1.4 - 0.4 * k), 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
}
function wireCommon(manifest) {
  canvas.addEventListener('click', (e) => { acx(); loop.pendingInput = clickToInput(window.__d2.game, e); });
  window.addEventListener('keydown', (e) => {
    if (e.key === 'e' || e.key === 'E') { acx(); loop.pendingInput = { type: 'pickup' }; }
    if (e.key === 'p' || e.key === 'P') {
      acx();
      const inv = JSON.parse(window.__d2.game.inventory());
      const slot = inv.findIndex((s) => s && s.kind === 'Potion');
      if (slot >= 0) loop.pendingInput = { type: 'useItem', slot };
      else flash('Aucune potion dans l\'inventaire.');
    }
  });
  $('btn-sound').addEventListener('click', () => {
    audio.enabled = !audio.enabled;
    $('btn-sound').textContent = `Son : ${audio.enabled ? 'oui' : 'non'}`;
    if (audio.enabled) acx();
  });
  $('btn-save').addEventListener('click', () => {
    try {
      const json = window.__d2.game.saveGame();
      localStorage.setItem(SAVE_KEY, json);
      sfx(manifest.sfx.uiClick, 0.6, 1.3);
      flash('Partie sauvegardée.');
    } catch (err) { fail('Sauvegarde', err); }
  });
  $('btn-load').addEventListener('click', () => {
    try {
      const json = localStorage.getItem(SAVE_KEY);
      if (!json) { flash('Aucune sauvegarde trouvée.'); return; }
      window.__d2.game.loadGame(json);
      sfx(manifest.sfx.uiClick, 0.6, 0.9);
      flash('Partie chargée.');
    } catch (err) { fail('Chargement', err); }
  });
}

function fpsTick(statusBase, extra = '') {
  loop.frames++;
  const now = performance.now();
  if (now - loop.fpsClock > 1000) {
    const fps = (loop.frames * 1000) / (now - loop.fpsClock);
    loop.frames = 0;
    loop.fpsClock = now;
    status.textContent = statusBase.replace(/ · \d+ fps$/, '') + ` · ${fps.toFixed(0)} fps` + extra;
  }
}

// --- chemin principal : moteur WASM -----------------------------------------
async function main() {
  $('loadmsg').textContent = 'Compilation JIT du moteur…';
  await init();

  $('loadmsg').textContent = 'Lecture du manifest…';
  const manifestText = await (await fetch(new URL('assets/manifest.json', ROOT))).text();
  const manifest = JSON.parse(manifestText);

  const hero = resolveHero(manifest, HERO_REQ);
  HERO_NAME = hero.def.name || hero.code;
  window.__d2.hero = hero.code;
  if (hero.fallback) {
    flash(`Classe ${HERO_REQ} absente du manifest — partie lancée en ${HERO_NAME}.`);
  }

  const game = new D2Game(manifestText, HERO_REQ);
  window.__d2.game = game;

  // Sanité : le moteur Rust doit résoudre le héros comme le JS (même règle).
  const hcRust = JSON.parse(game.heroClass());
  if (hcRust.code !== hero.code) {
    logErr(`désaccord résolution héros JS/Rust : ${hero.code} vs ${hcRust.code}`);
  }

  const paths = collectPaths(manifest, hero.def);
  for (let i = 0; i < paths.length; i++) {
    $('loadmsg').textContent = `Chargement des assets ${i + 1}/${paths.length}…`;
    $('loadfill').style.width = `${(100 * i) / paths.length}%`;
    game.provide(paths[i], await fetchBytes(paths[i]));
  }
  $('loadfill').style.width = '100%';

  $('loadmsg').textContent = 'Composition de la carte…';
  await new Promise((r) => setTimeout(r, 30)); // laisse peindre le message
  game.setViewport(canvas.width, canvas.height);
  game.finalize();

  const [mw, mh] = game.mapPixelSize();
  const [gw, gh] = game.gridSize();
  const [drawn, hidden, , oob] = game.composeStats();
  const statusBase =
    `Héros : ${HERO_NAME} · Carte ${mw}×${mh}px · grille ${gw}×${gh} · tuiles dessinées ${drawn} (cachées ${hidden}, hors bornes ${oob})` +
    ' · clic = déplacer · clic monstre = attaquer · E = ramasser · P = potion';
  status.textContent = statusBase;

  await loadSfx(manifest);
  wireCommon(manifest);
  $('loading').style.display = 'none';
  window.__d2.mode = 'wasm';

  const imgData = ctx.createImageData(canvas.width, canvas.height);
  function frame(now) {
    const dt = Math.min(now - loop.last, 100);
    loop.last = now;
    try {
      const inputJson = loop.pendingInput ? JSON.stringify(loop.pendingInput) : null;
      loop.pendingInput = null;
      const events = JSON.parse(game.tick(dt, inputJson));
      handleEvents(events, manifest);
      updateHud(JSON.parse(game.heroStats()));
      imgData.data.set(game.render());
      ctx.putImageData(imgData, 0, 0);
      const [hsx, hsy] = game.heroScreen();
      const [hwx, hwy] = game.screenToWorld(hsx, hsy);
      if (loop.lastClick && now - loop.lastClick.t < 450) {
        drawClickRing(loop.lastClick.sx, loop.lastClick.sy, 1 - (now - loop.lastClick.t) / 450);
      }
      fpsTick(statusBase, ` · héros ${hwx | 0},${hwy | 0} (monde)`);
      requestAnimationFrame(frame);
    } catch (err) {
      fail('tick', err);
    }
  }
  requestAnimationFrame(frame);
}

// --- fallback JS visible (le WASM n'a pas chargé) ---------------------------
function fallback(reason) {
  window.__d2.mode = 'fallback';
  $('fallback-banner').style.display = 'block';
  $('loading').style.display = 'none';
  logErr('wasm indisponible : ' + reason);
  const statusBase = 'MODE DÉGRADÉ JS — clic pour déplacer le héros (moteur Rust indisponible)';
  status.textContent = statusBase;
  updateHud({ life: 55, max_life: 55, mana: 15, max_mana: 15, level: 1, xp: 0, gold: 0 });

  const fb = {
    hx: 400, hy: 300, tx: 400, ty: 300, speed: 120,
    mons: Array.from({ length: 3 }, (_, i) => ({
      x: 120 + i * 250, y: 150 + (i % 2) * 260, hp: 3,
      c: ['#c0392b', '#7d6608', '#a04000'][i],
    })),
  };
  canvas.addEventListener('click', (e) => {
    const r = canvas.getBoundingClientRect();
    fb.tx = (e.clientX - r.left) * (canvas.width / r.width);
    fb.ty = (e.clientY - r.top) * (canvas.height / r.height);
  });
  let last = performance.now();
  function frame(now) {
    const dt = (now - last) / 1000;
    last = now;
    const dx = fb.tx - fb.hx, dy = fb.ty - fb.hy, d = Math.hypot(dx, dy);
    if (d > 2) { fb.hx += (dx / d) * fb.speed * dt; fb.hy += (dy / d) * fb.speed * dt; }
    ctx.fillStyle = '#1c2416';
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    ctx.strokeStyle = '#2a3a20';
    for (let y = 0; y < canvas.height; y += 40) {
      ctx.beginPath(); ctx.moveTo(0, y); ctx.lineTo(canvas.width, y); ctx.stroke();
    }
    for (let x = 0; x < canvas.width; x += 40) {
      ctx.beginPath(); ctx.moveTo(x, 0); ctx.lineTo(x, canvas.height); ctx.stroke();
    }
    for (const m of fb.mons) {
      if (m.hp <= 0) continue;
      ctx.fillStyle = m.c;
      ctx.fillRect(m.x - 10, m.y - 14, 20, 28);
    }
    ctx.fillStyle = '#d2b48c';
    ctx.fillRect(fb.hx - 10, fb.hy - 16, 20, 32);
    ctx.fillStyle = '#e8dcbf';
    ctx.font = '12px system-ui';
    ctx.fillText('fallback JS — WASM non chargé', 12, canvas.height - 12);
    fpsTick(statusBase);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

main().catch((e) => fallback(e && e.message ? e.message : String(e)));
