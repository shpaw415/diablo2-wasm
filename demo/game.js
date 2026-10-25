// Diablo 2 WASM — glue navigateur.
// Le décodage/composition/rendu/simulation vivent dans Rust (demo/game-pkg).
// Ce fichier : fetch des assets, boucle putImageData, HUD (globes D2 dessinés
// sur le canvas), panneau inventaire (I), course (Maj), sons, sauvegarde,
// et un fallback JS visible si le moteur WASM ne charge pas.
//
// Import DYNAMIQUE avec buste de cache : le pkg wasm est régénéré à chaque
// build Rust (membres parallèles) — un module ES statique resterait collé au
// cache Chromium malgré le nouveau binaire.

const $ = (id) => document.getElementById(id);
import './automap.js'; // 9d monde vivant : mini-carte (window.D2Automap, Tab / bouton Carte)
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
const PARAMS = new URLSearchParams(location.search);
// Sélection de personnage : ?hero=BA|SOR|PA (défaut BA). La résolution
// effective (heroes[code] sinon bloc hero) est faite côté moteur Rust —
// ci-dessous on reflète la même règle pour les assets et le HUD.
let HERO_REQ = (PARAMS.get('hero') || 'BA').toUpperCase();
// ?continue=1 : « Continuer » du menu — le héros vient de la sauvegarde la
// plus récente (slots v1, miroir clé historique d2wasm.save.v1).
// ?slot=<id> : reprise d'un slot précis depuis l'écran « Parties » du menu.
let CONTINUE_SAVE = null;
let CONTINUE_SLOT_ID = null; // slot d'origine (ré-écrit par Sauvegarder)
if (PARAMS.get('slot')) {
  try {
    const slot = window.D2Slots && window.D2Slots.get(PARAMS.get('slot'));
    if (slot && slot.save) {
      CONTINUE_SAVE = slot.save;
      CONTINUE_SLOT_ID = slot.id;
    }
  } catch { CONTINUE_SAVE = null; } // slot inconnu : nouvelle partie
}
if (!CONTINUE_SAVE && PARAMS.get('continue') === '1') {
  try {
    let raw = null;
    const newest = window.D2Slots && window.D2Slots.mostRecent();
    if (newest && newest.save) { raw = newest.save; CONTINUE_SLOT_ID = newest.id; }
    else raw = localStorage.getItem(SAVE_KEY); // rétro-compat sans slots
    if (raw) CONTINUE_SAVE = raw;
  } catch { CONTINUE_SAVE = null; } // save corrompue : nouvelle partie
}
if (CONTINUE_SAVE) {
  try {
    const hc = JSON.parse(CONTINUE_SAVE).hero_code;
    if (typeof hc === 'string' && hc) HERO_REQ = hc.toUpperCase();
  } catch { CONTINUE_SAVE = null; }
}

// Niveau de boot : ?level=bloodmoor… ; sinon le niveau de la sauvegarde
// (« Continuer ») — save v3 : la clé `level` explicite du JSON ; save v1/v2 :
// repli historique sur les dimensions de grille (town 285, bloodmoor 360
// colonnes) ; sinon town.
function detectSaveLevel(manifest, saveJson) {
  try {
    if (typeof saveJson.level === 'string' && saveJson.level) return saveJson.level.toLowerCase();
  } catch { /* save illisible : niveau par défaut */ }
  try {
    const cols = saveJson.collision?.[0]?.length;
    if (!cols || !manifest.levels) return null;
    for (const [key, lv] of Object.entries(manifest.levels)) {
      const gw = (lv.grid || [lv.sizeTiles[0] + 1, 0])[0] * 5;
      if (gw === cols) return key;
    }
  } catch { /* save illisible : niveau par défaut */ }
  return null;
}

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

// Assets CRITIQUES (cartes + blocs walk) : un échec = partie impossible.
// Assets OPTIONNELS (modes NU/RN/A1/DD) : un échec = mode sauté, note,
// le moteur retombe sur la marche — jamais d'erreur fatale.
// Multi-niveaux : les assets de TOUS les blocs `levels` sont chargés au boot
// (ds1 + dt1 + palette par niveau) — changer de niveau est instantané.
function collectPaths(manifest, heroDef) {
  const critical = new Set();
  const optional = new Set();
  const levels = manifest.levels && Object.keys(manifest.levels).length
    ? Object.values(manifest.levels)
    : [manifest.level]; // rétro-compat : bloc `level` historique seul
  for (const lv of levels) {
    critical.add(lv.ds1);
    critical.add(lv.palette);
    for (const p of lv.dt1 || []) critical.add(p);
  }
  const addAnim = (set, w) => {
    if (!w || typeof w !== 'object') return;
    set.add(w.cof);
    for (const p of Object.values(w.layers || {})) set.add(p);
  };
  addAnim(critical, heroDef.walk);
  // modes explicites {NU|WL|RN|A1|DD} ou blocs nommés idle/attack/death :
  // WL retombe sur walk (déjà critique) ; les autres sont optionnels.
  const extra = new Set();
  if (heroDef.modes) {
    for (const tag of ['NU', 'WL', 'RN', 'A1', 'DD']) addAnim(extra, heroDef.modes[tag]);
  } else {
    addAnim(extra, heroDef.idle);
    addAnim(extra, heroDef.attack);
    addAnim(extra, heroDef.death);
  }
  for (const p of extra) if (!critical.has(p)) optional.add(p);
  for (const m of manifest.monsters) {
    addAnim(critical, m.walk);
    const mExtra = new Set();
    if (m.modes) {
      for (const tag of ['NU', 'WL', 'RN', 'A1', 'DD']) addAnim(mExtra, m.modes[tag]);
    }
    for (const p of mExtra) if (!critical.has(p)) optional.add(p);
  }
  // AnimData.d2 (frames/speed officiels par token, audit #19) : optionnel —
  // absent ⇒ le moteur garde le repli octets COF.
  optional.add('assets/animdata/AnimData.d2');
  return { critical: [...critical], optional: [...optional] };
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
  playNote(c, buf, c.currentTime, vol, rate);
}
function playNote(c, buf, when, vol, rate) {
  const src = c.createBufferSource();
  src.buffer = buf;
  src.playbackRate.value = rate;
  const g = c.createGain();
  g.gain.value = vol;
  src.connect(g).connect(c.destination);
  src.start(when);
}
// Fanfare simplifiée : 3 notes du même buffer, montante (level up).
function fanfare(key) {
  if (!audio.enabled || !key) return;
  const c = acx();
  const buf = audio.buffers.get(key);
  if (!c || !buf) return;
  const t = c.currentTime;
  playNote(c, buf, t, 0.55, 1.2);
  playNote(c, buf, t + 0.13, 0.6, 1.5);
  playNote(c, buf, t + 0.26, 0.7, 2.0);
}

// --- HUD : globes circulaires D2 dessinés sur le canvas ---------------------
let HERO_NAME = 'Barbare'; // surchargé une fois le manifest lu
const hud = {
  hpt: $('hptext'), mpt: $('mptext'), lvl: $('lvltext'), gold: $('goldtext'),
  xp: $('xpfill'), zone: $('zonetext'), run: $('runbadge'),
};
const GLOBE_R = 50;
const HP_GLOBE = { x: 66, y: canvas.height - 62 };
const MP_GLOBE = { x: canvas.width - 66, y: canvas.height - 62 };

function drawGlobe(g, frac, topColor, bottomColor) {
  const { x, y } = g;
  const r = GLOBE_R;
  const f = Math.max(0, Math.min(1, frac));
  ctx.save();
  // fond sombre du globe
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fillStyle = '#100a09';
  ctx.fill();
  // liquide : niveau = fraction, dégradé vertical
  ctx.save();
  ctx.clip();
  const lvl = y + r - 2 * r * f;
  const grad = ctx.createLinearGradient(0, y - r, 0, y + r);
  grad.addColorStop(0, topColor);
  grad.addColorStop(1, bottomColor);
  ctx.fillStyle = grad;
  ctx.fillRect(x - r, lvl, 2 * r, y + r - lvl);
  // reflet de surface
  ctx.fillStyle = 'rgba(255,255,255,0.10)';
  ctx.fillRect(x - r, lvl, 2 * r, 3);
  ctx.restore();
  // reflet vitre
  const glass = ctx.createRadialGradient(x - r * 0.35, y - r * 0.45, r * 0.1, x, y, r);
  glass.addColorStop(0, 'rgba(255,255,255,0.16)');
  glass.addColorStop(1, 'rgba(255,255,255,0)');
  ctx.beginPath();
  ctx.arc(x, y, r, 0, Math.PI * 2);
  ctx.fillStyle = glass;
  ctx.fill();
  // cerclage pierre/or
  ctx.lineWidth = 4;
  ctx.strokeStyle = '#4a3c22';
  ctx.stroke();
  ctx.lineWidth = 1;
  ctx.strokeStyle = '#000000cc';
  ctx.beginPath();
  ctx.arc(x, y, r - 2.5, 0, Math.PI * 2);
  ctx.stroke();
  ctx.restore();
}
function drawGlobes(s) {
  drawGlobe(HP_GLOBE, s ? s.life / Math.max(1, s.max_life) : 0, '#e05545', '#5e120c');
  drawGlobe(MP_GLOBE, s ? s.mana / Math.max(1, s.max_mana) : 0, '#4f8fd4', '#122a52');
}

function updateHud(s) {
  if (!s || s.max_life === undefined) return;
  hud.hpt.textContent = `${s.life}/${s.max_life}`;
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
  note(msg);
}
function note(msg) {
  const log = $('msglog');
  if (log) {
    const line = document.createElement('div');
    line.textContent = `• ${msg}`;
    log.appendChild(line);
    while (log.children.length > 12) log.removeChild(log.firstChild);
    log.scrollTop = log.scrollHeight;
  }
}

// --- inventaire visuel (panneau 10×4 dessiné sur le canvas) ------------------
const INV_COLS = 10, INV_ROWS = 4;
const inv = {
  open: false,
  hover: -1,
  cell: 42, pad: 14, header: 30,
  get x0() { return (canvas.width - this.w) / 2; },
  get y0() { return (canvas.height - this.h) / 2; },
  get w() { return this.pad * 2 + INV_COLS * this.cell; },
  get h() { return this.pad * 2 + this.header + INV_ROWS * this.cell; },
};
function invSlotAt(px, py) {
  const cx = Math.floor((px - inv.x0 - inv.pad) / inv.cell);
  const cy = Math.floor((py - inv.y0 - inv.pad - inv.header) / inv.cell);
  if (cx < 0 || cy < 0 || cx >= INV_COLS || cy >= INV_ROWS) return -1;
  return cy * INV_COLS + cx;
}
const ITEM_COLORS = {
  Potion: ['#b03a2e', '#57150e'],
  Weapon: ['#8d99a6', '#39424c'],
  Armor: ['#8a6d3b', '#41331a'],
  Gold: ['#d9b96a', '#6e5626'],
};
function drawInventory() {
  const { x0, y0, w, h } = inv;
  ctx.save();
  // cadre pierre sombre
  ctx.fillStyle = 'rgba(16,12,18,0.92)';
  ctx.fillRect(x0, y0, w, h);
  ctx.strokeStyle = '#5a4a2a';
  ctx.lineWidth = 2;
  ctx.strokeRect(x0 + 1, y0 + 1, w - 2, h - 2);
  ctx.strokeStyle = '#000';
  ctx.strokeRect(x0 + 3.5, y0 + 3.5, w - 7, h - 7);
  ctx.fillStyle = '#c7a45a';
  ctx.font = '600 12px system-ui';
  ctx.textAlign = 'center';
  ctx.fillText('INVENTAIRE', x0 + w / 2, y0 + inv.pad + 8);
  // grille 10×4
  for (let row = 0; row < INV_ROWS; row++) {
    for (let col = 0; col < INV_COLS; col++) {
      const cx = x0 + inv.pad + col * inv.cell;
      const cy = y0 + inv.pad + inv.header + row * inv.cell;
      ctx.fillStyle = '#2c2c34';
      ctx.fillRect(cx + 1, cy + 1, inv.cell - 2, inv.cell - 2);
      // biseau pierre
      ctx.strokeStyle = 'rgba(255,255,255,0.07)';
      ctx.beginPath();
      ctx.moveTo(cx + 2, cy + inv.cell - 3);
      ctx.lineTo(cx + 2, cy + 2);
      ctx.lineTo(cx + inv.cell - 3, cy + 2);
      ctx.stroke();
      ctx.strokeStyle = 'rgba(0,0,0,0.55)';
      ctx.beginPath();
      ctx.moveTo(cx + inv.cell - 2, cy + 2);
      ctx.lineTo(cx + inv.cell - 2, cy + inv.cell - 2);
      ctx.lineTo(cx + 2, cy + inv.cell - 2);
      ctx.stroke();
      const slot = row * INV_COLS + col;
      if (slot === inv.hover) {
        ctx.strokeStyle = '#d9b96a';
        ctx.lineWidth = 1.5;
        ctx.strokeRect(cx + 0.5, cy + 0.5, inv.cell - 1, inv.cell - 1);
      }
    }
  }
  // objets (rectangles colorés)
  const slots = window.__d2.invCache || [];
  for (let i = 0; i < slots.length && i < INV_COLS * INV_ROWS; i++) {
    const it = slots[i];
    if (!it) continue;
    const [body, edge] = ITEM_COLORS[it.kind] || ['#9a8f7a', '#4a4438'];
    const cx = x0 + inv.pad + (i % INV_COLS) * inv.cell;
    const cy = y0 + inv.pad + inv.header + Math.floor(i / INV_COLS) * inv.cell;
    const m = 5;
    ctx.fillStyle = body;
    ctx.fillRect(cx + m, cy + m, inv.cell - 2 * m, inv.cell - 2 * m);
    ctx.strokeStyle = edge;
    ctx.lineWidth = 2;
    ctx.strokeRect(cx + m, cy + m, inv.cell - 2 * m, inv.cell - 2 * m);
    // lueur
    ctx.fillStyle = 'rgba(255,255,255,0.14)';
    ctx.fillRect(cx + m, cy + m, inv.cell - 2 * m, 4);
  }
  // info-bulle survol
  if (inv.hover >= 0 && slots[inv.hover]) {
    const it = slots[inv.hover];
    const label = it.name + (it.amount ? ` (${it.amount} or)` : '');
    ctx.font = '11px system-ui';
    const tw = ctx.measureText(label).width + 16;
    const tx = Math.min(Math.max(x0, x0 + w / 2 - tw / 2), x0 + w - tw);
    const ty = y0 - 26;
    ctx.fillStyle = '#141019ee';
    ctx.fillRect(tx, ty, tw, 20);
    ctx.strokeStyle = '#5a4a2a';
    ctx.strokeRect(tx, ty, tw, 20);
    ctx.fillStyle = '#e8dcbf';
    ctx.textAlign = 'left';
    ctx.fillText(label, tx + 8, ty + 14);
  }
  ctx.restore();
}

// --- événements gameplay → sons + messages ----------------------------------
function showLevelBanner(name) {
  const b = $('levelBanner');
  b.textContent = name || '';
  b.classList.remove('show');
  void b.offsetWidth; // relance l'animation CSS
  b.classList.add('show');
}
function levelFade() {
  // Fondu noir court masquant le rechargement du niveau (le moteur change
  // de carte DANS le tick ; le fondu couvre la bascule visuelle).
  const f = $('fade');
  f.classList.add('black');
  setTimeout(() => f.classList.remove('black'), 260);
}
function handleEvents(events, manifest) {
  for (const ev of events) {
    window.__d2.events++;
    if (window.D2MonSfx) window.D2MonSfx.feed(ev); // 14a : sons monstres data-driven (attaque/hit/mort/annonce)
    if (ev.LevelSwitch) {
      const info = ev.LevelSwitch;
      window.__d2.level = info.key;
      hud.zone.textContent = info.name;
      levelFade();
      showLevelBanner(info.name);
      if (window.D2Music) window.D2Music.setLevel(info.key, info.name); // EXP-A1 : musique par zone (fondu doux)
      sfx(manifest.sfx.uiClick, 0.5, 0.7);
      note(`— ${info.name} —`);
      if (window.__d2.refreshStatus) window.__d2.refreshStatus();
    } else if (ev.LevelSwitchFailed) {
      flash(`Transition impossible : ${ev.LevelSwitchFailed}`);
    } else if (ev.HeroAttacked) {
      sfx(manifest.sfx.attack);
      flash('Attaque !');
    } else if (ev.HeroMissed) {
      sfx(manifest.sfx.attack, 0.25, 1.4);
    } else if (ev.HeroHit) {
      sfx(manifest.sfx.hit);
    } else if (ev.MonsterDied) {
      // Les monstres spawnent PAR NIVEAU (codes du moteur, pas l'ordre manifest).
      const code = (window.__d2.monCodes || [])[ev.MonsterDied.index]
        || manifest.monsters[ev.MonsterDied.index]?.code;
      const die = (code && manifest.sfx.monsters[code] && manifest.sfx.monsters[code].die) || manifest.sfx.die;
      sfx(die);
      flash(`Monstre tué (+${ev.MonsterDied.xp} xp) — un butin est peut-être au sol !`);
    } else if (ev.ItemDropped) {
      // son de drop (or grave, potion aiguë) — nouveauté HUD/sons
      const it = ev.ItemDropped.item;
      sfx(manifest.sfx.uiClick, 0.45, it.kind === 'Gold' ? 0.85 : 1.15);
      flash(`Butin : ${it.name}${it.amount ? ` (${it.amount} or)` : ''}`);
    } else if (ev.HeroLeveled) {
      fanfare(manifest.sfx.uiClick);
      flash(`NIVEAU ${ev.HeroLeveled.level} !`);
    } else if (ev.GoldGained) {
      sfx(manifest.sfx.uiClick, 0.4, 1.2);
      flash(`+${ev.GoldGained.amount} or`);
    } else if (ev.HeroPickedUp) {
      sfx(manifest.sfx.uiClick, 0.5, 1.1);
      flash('Objet ramassé.');
      // 11b : auto-ranger les potions autobelt (hp/mp/rejuv) en ceinture 4×4
      try {
        const moved = window.__d2.game.beltAutoStore();
        if (moved > 0) note(`${moved} potion(s) rangée(s) en ceinture (touches 1-4).`);
      } catch { /* cosmétique : ne bloque jamais le ramassage */ }
    } else if (ev.PotionUsed) {
      sfx(manifest.sfx.uiClick, 0.5, 0.8);
      flash(`Potion bue (+${ev.PotionUsed.heal} pv)`);
    } else if (ev.InventoryFull) {
      flash('Inventaire plein — objet laissé au sol.');
    } else if (ev.HeroDied) {
      sfx(manifest.sfx.hit, 0.8, 0.6);
      flash('Vous êtes mort… résurrection au camp dans 2 s.');
    } else if (ev.CorpseDropped) {
      // J1 (corpse-ui.js) : le corps est posé au sol AVEC l'équipement ;
      // l'or porté est tombé à côté (péage officiel déduit par le moteur).
      if (window.D2CorpseUI) window.D2CorpseUI.onDeath(ev.CorpseDropped);
      sfx(manifest.sfx.hit, 0.6, 0.5);
      flash(`Corps posé à (${ev.CorpseDropped.x}, ${ev.CorpseDropped.y})${ev.CorpseDropped.gold ? ` — ${ev.CorpseDropped.gold} or au sol` : ''}.`);
    } else if (ev.CorpseRecovered) {
      // J1 : équipement ré-équipé depuis le corps (corps vide = disparu).
      if (window.D2CorpseUI) window.D2CorpseUI.onRecovered();
      sfx(manifest.sfx.uiClick, 0.5, 1.0);
      flash('Équipement récupéré sur votre corps.');
    } else if (ev.HeroRespawned) {
      sfx(manifest.sfx.uiClick, 0.55, 0.7);
      if (window.D2CorpseUI) window.D2CorpseUI.onRespawned(); // J1 : masque l'overlay de mort
      flash('Résurrection au Camp des Rousses.');
    }
  }
}

// --- boucle commune ---------------------------------------------------------
const loop = { queue: [], lastClick: null, last: performance.now(), frames: 0, fpsClock: performance.now() };
// EXP-H* (HUD parité) : la file d'inputs est exposée à demo/hud.js pour le
// routage clic droit / `rangedAttack` (boutons de compétence L/R).
window.__d2.queue = loop.queue;
const mouse = { x: -1, y: -1, inside: false };
function canvasXY(e) {
  const r = canvas.getBoundingClientRect();
  return [
    (e.clientX - r.left) * (canvas.width / r.width),
    (e.clientY - r.top) * (canvas.height / r.height),
  ];
}
function clickToInput(game, px, py) {
  const [wx, wy] = game.screenToWorld(px, py);
  loop.lastClick = { sx: px, sy: py, t: performance.now() };
  // J1 (corpse-ui.js) : clic sur SON CORPS au sol → {"type":"recoverCorpse"}.
  // À portée le moteur ré-équipe tout, sinon le héros marche vers le corps
  // et la récupération s'exécute automatiquement à l'arrivée (grab D2).
  try {
    const c = JSON.parse(game.corpseInfo() || 'null');
    if (c && Math.hypot(c.x - wx, c.y - wy) < 2.5) return { type: 'recoverCorpse' };
  } catch { /* pas de corps / pas finalisé */ }
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
// Après un loadGame multi-zones (save v3) : le HUD et le statut suivent le
// niveau réellement restauré par le moteur (levelInfo()).
function syncZoneAfterLoad() {
  const game = window.__d2.game;
  if (!game || !game.isFinalized()) return;
  try {
    const li = JSON.parse(game.levelInfo() || '{}');
    window.__d2.level = li.key || window.__d2.level;
    if (li.name) {
      hud.zone.textContent = li.name;
      showLevelBanner(li.name);
    }
    if (window.__d2.refreshStatus) window.__d2.refreshStatus();
    if (window.D2Music) window.D2Music.setLevel(li.key, li.name); // EXP-A1 : musique par zone (waypoints/Charger)
  } catch { /* HUD : ne bloque jamais le chargement */ }
}
// 11b consommables : toast d'un usage à effets réels (potions par code,
// buffs 30 s, tomes à charges) + retour au camp éventuel (parchemin TP :
// le moteur a déjà basculé via switch_level, le HUD/zone/musique suivent).
function handleConsumed(r, manifest) {
  const u = r.use || {};
  if (u.heal > 0 && u.mana > 0) flash(`${u.name} : +${u.heal} pv, +${u.mana} mana`);
  else if (u.heal > 0) flash(`${u.name} : +${u.heal} pv`);
  else if (u.mana > 0) flash(`${u.name} : +${u.mana} mana`);
  else if (u.buffMs > 0) flash(`${u.name} : effet ${Math.round(u.buffMs / 1000)} s`);
  else if (u.townPortal) flash(`${u.name} — retour au camp`);
  else if (u.chargesLeft != null) flash(`${u.name} : ${u.chargesLeft} charge(s) restante(s)`);
  sfx(manifest.sfx.uiClick, 0.5, 0.8);
  if (r.levelSwitch) syncZoneAfterLoad();
}
function wireCommon(manifest) {
  canvas.addEventListener('mousemove', (e) => {
    [mouse.x, mouse.y] = canvasXY(e);
    mouse.inside = true;
    if (inv.open) {
      inv.hover = invSlotAt(mouse.x, mouse.y);
      canvas.style.cursor = inv.hover >= 0 ? 'pointer' : 'crosshair';
    }
  });
  canvas.addEventListener('mouseleave', () => { mouse.inside = false; inv.hover = -1; });
  canvas.addEventListener('click', (e) => {
    acx();
    const [px, py] = canvasXY(e);
    // clic dans le panneau inventaire ouvert : utiliser l'objet, pas bouger
    if (inv.open && px >= inv.x0 && px < inv.x0 + inv.w && py >= inv.y0 && py < inv.y0 + inv.h) {
      const slot = invSlotAt(px, py);
      const it = slot >= 0 ? (window.__d2.invCache || [])[slot] : null;
      if (it) {
        // 11b consommables : effets réels par code (potions, parchemins de
        // porte de ville, tomes à charges) — retombe sur le soin MVP 60
        // (useItem historique) si l'endpoint refuse.
        const scrolls = ['tsc', 'tbk', 'isc', 'ibk'];
        if (it.kind === 'Potion' || scrolls.includes(it.code)) {
          try {
            handleConsumed(JSON.parse(window.__d2.game.useConsumable(slot)), manifest);
          } catch (err) {
            if (it.kind === 'Potion') loop.queue.push({ type: 'useItem', slot });
            else note(String(err && err.message ? err.message : err));
          }
        } else {
          note(`${it.name} — équipement à venir (phase 3).`);
        }
      }
      return;
    }
    // W16 villes vivantes : clic NPC (npc-ui.js) prioritaire — marqueur visé
    // = dialogue/approche ; sinon routage historique attaque/déplacement.
    if (window.D2Npc && window.D2Npc.clickAt(px, py)) return;
    loop.queue.push(clickToInput(window.__d2.game, px, py));
  });
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Shift') {
      acx();
      loop.queue.push({ type: 'setRun', on: true });
    }
    if (e.key === 'i' || e.key === 'I') {
      inv.open = !inv.open;
      inv.hover = -1;
      canvas.style.cursor = 'crosshair';
      sfx(manifest.sfx.uiClick, 0.4, inv.open ? 1.3 : 1.0);
      note(inv.open ? 'Inventaire ouvert.' : 'Inventaire fermé.');
    }
    if (e.key === 'Escape' && inv.open) {
      inv.open = false;
      canvas.style.cursor = 'crosshair';
    }
    if (e.key === 'e' || e.key === 'E') { acx(); loop.queue.push({ type: 'pickup' }); }
    // 11b : ceinture 4×4 — touches 1-4 = colonnes (potion au sommet bue)
    if (['1', '2', '3', '4'].includes(e.key)) {
      acx();
      try {
        handleConsumed(JSON.parse(window.__d2.game.useBeltKey(Number(e.key))), manifest);
      } catch (err) {
        note(String(err && err.message ? err.message : err));
      }
    }
    if (e.key === 'p' || e.key === 'P') {
      acx();
      const slots = JSON.parse(window.__d2.game.inventory());
      const slot = slots.findIndex((s) => s && s.kind === 'Potion');
      if (slot >= 0) loop.queue.push({ type: 'useItem', slot });
      else note("Aucune potion dans l'inventaire.");
    }
  });
  window.addEventListener('keyup', (e) => {
    if (e.key === 'Shift') loop.queue.push({ type: 'setRun', on: false });
  });
  $('btn-sound').addEventListener('click', () => {
    audio.enabled = !audio.enabled;
    $('btn-sound').textContent = `Son : ${audio.enabled ? 'oui' : 'non'}`;
    if (audio.enabled) acx();
  });
  $('btn-save').addEventListener('click', () => {
    try {
      const json = window.__d2.game.saveGame();
      localStorage.setItem(SAVE_KEY, json); // miroir rétro-compat (clé historique)
      let slotName = 'fiche héros';
      try {
        // Slots nommés (save P0) : identité par héros, nom conservé si déjà
        // nommé. En reprise (?slot=), c'est CE slot qui est mis à jour.
        const slot = window.D2Slots.upsertFromSave(json);
        slotName = slot.name;
        if (CONTINUE_SLOT_ID && slot.id !== CONTINUE_SLOT_ID) {
          CONTINUE_SLOT_ID = slot.id; // slot (re)créé : les prochaines saves suivent
        }
      } catch (se) { note('Slots : ' + (se && se.message ? se.message : se)); }
      sfx(manifest.sfx.uiClick, 0.6, 1.3);
      flash('Partie sauvegardée — ' + slotName + '.');
    } catch (err) { fail('Sauvegarde', err); }
  });
  $('btn-load').addEventListener('click', () => {
    try {
      const json = localStorage.getItem(SAVE_KEY);
      if (!json) { flash('Aucune sauvegarde trouvée.'); return; }
      window.__d2.game.loadGame(json); // peut changer de zone (save v3 multi-niveaux)
      syncZoneAfterLoad();
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
    const base = loop.statusBase || statusBase;
    status.textContent = base.replace(/ · \d+ fps$/, '') + ` · ${fps.toFixed(0)} fps` + extra;
  }
}

// --- chemin principal : moteur WASM -----------------------------------------
// Chargement du module moteur, tolérant aux proxys (embed dashboard) :
// 1) import avec buste de cache (?cb=) — idéal en direct, mais un proxy peut
//    refuser les query strings ;
// 2) import nu, sans query ;
// 3) réemballage blob — contourne un proxy qui servirait le JS avec un
//    mauvais MIME (les imports de module exigent un type JavaScript).
// Chaque tentative ratée est notée en console.info (sans polluer le journal
// d'erreurs) ; la dernière erreur remonte au fallback avec son vrai message.
async function loadEngine() {
  const base = './game-pkg/d2game.js';
  const attempts = [
    () => import(`${base}?cb=${Date.now()}`),
    () => import(base),
    async () => {
      const txt = await (await fetch(base)).text();
      return import(URL.createObjectURL(new Blob([txt], { type: 'text/javascript' })));
    },
  ];
  let lastErr;
  for (let i = 0; i < attempts.length; i++) {
    try {
      return await attempts[i]();
    } catch (e) {
      lastErr = e;
      console.info(`moteur : tentative ${i + 1}/3 échouée — ${e && e.message ? e.message : e}`);
    }
  }
  throw lastErr;
}

async function main() {
  $('loadmsg').textContent = 'Compilation JIT du moteur…';
  const mod = await loadEngine();
  const D2Game = mod.D2Game;
  if (typeof D2Game !== 'function') throw new Error('module moteur chargé mais export D2Game absent');
  // Le .wasm est chargé explicitement en octets (MIME-agnostique), avec le
  // même repli sans query si le proxy refuse les query strings.
  const wasmBytes = await (async () => {
    const urls = [`./game-pkg/d2game_bg.wasm?cb=${Date.now()}`, './game-pkg/d2game_bg.wasm'];
    let lastErr;
    for (const url of urls) {
      try {
        const r = await fetch(url);
        if (r.ok) return await r.arrayBuffer();
        lastErr = new Error(`fetch ${url} -> HTTP ${r.status}`);
      } catch (e) { lastErr = e; }
    }
    throw lastErr;
  })();
  await mod.default(wasmBytes);

  $('loadmsg').textContent = 'Lecture du manifest…';
  const manifestText = await (await fetch(new URL('assets/manifest.json', ROOT))).text();
  const manifest = JSON.parse(manifestText);

  // Niveau de boot : ?level=bloodmoor… > niveau de la sauvegarde (« Continuer »,
  // clé v3 `level` — repli dimensions pour saves v1/v2) > town. Le bloc
  // `level` historique reste l'alias de town côté moteur.
  let LEVEL_REQ = (PARAMS.get('level') || '').toLowerCase();
  if (!LEVEL_REQ && CONTINUE_SAVE) {
    try { LEVEL_REQ = detectSaveLevel(manifest, JSON.parse(CONTINUE_SAVE)) || 'town'; }
    catch { LEVEL_REQ = 'town'; }
  }
  if (!LEVEL_REQ) LEVEL_REQ = 'town';
  if (LEVEL_REQ !== 'town' && !(manifest.levels && manifest.levels[LEVEL_REQ])) {
    flash(`Niveau « ${LEVEL_REQ} » inconnu — démarrage au Camp des Rousses.`);
    LEVEL_REQ = 'town';
  }

  const hero = resolveHero(manifest, HERO_REQ);
  HERO_NAME = hero.def.name || hero.code;
  window.__d2.hero = hero.code;
  if (hero.fallback) {
    flash(`Classe ${HERO_REQ} absente du manifest — partie lancée en ${HERO_NAME}.`);
  }

  const game = D2Game.withLevel(manifestText, HERO_REQ, LEVEL_REQ);
  window.__d2.game = game;

  // Difficulté (W08/K, impl-difficultes) : ?diff=nm|hell — appliquée AVANT
  // finalize : le niveau de boot instancie ses monstres avec les facteurs
  // MonLvl (N)/(H), résists et densités de la difficulté demandée.
  const DIFF_REQ = (PARAMS.get('diff') || '').toLowerCase();
  if (DIFF_REQ && DIFF_REQ !== 'normal') {
    try { game.setDifficulty(DIFF_REQ); } catch (e) { flash(`Difficulté « ${DIFF_REQ} » refusée (${e.message}) — Normal.`); }
  }

  // Sanité : le moteur Rust doit résoudre le héros comme le JS (même règle).
  const hcRust = JSON.parse(game.heroClass());
  if (hcRust.code !== hero.code) {
    logErr(`désaccord résolution héros JS/Rust : ${hero.code} vs ${hcRust.code}`);
  }

  const { critical, optional } = collectPaths(manifest, hero.def);
  const paths = [...critical, ...optional];
  const skipped = new Set();
  for (let i = 0; i < paths.length; i++) {
    $('loadmsg').textContent = `Chargement des assets ${i + 1}/${paths.length}…`;
    $('loadfill').style.width = `${(100 * i) / paths.length}%`;
    const p = paths[i];
    const isOptional = i >= critical.length;
    try {
      game.provide(p, await fetchBytes(p));
    } catch (e) {
      if (!isOptional) throw e;
      skipped.add(p); // mode optionnel absent : le moteur le notera aussi
    }
  }
  $('loadfill').style.width = '100%';

  $('loadmsg').textContent = 'Composition de la carte…';
  await new Promise((r) => setTimeout(r, 30)); // laisse peindre le message
  game.setViewport(canvas.width, canvas.height);
  game.finalize();

  for (const n of JSON.parse(game.spriteNotes())) note(`anim sautée : ${n}`);
  for (const n of JSON.parse(game.switchNotes())) note(n);
  if (skipped.size) note(`${skipped.size} fichier(s) optionnel(s) absent(s) — modes en repli marche.`);

  // Badge difficulté (W08/K) : masqué en Normal, visible NM/Enfer.
  try {
    const di = JSON.parse(game.difficultyInfo());
    const badge = $('diffbadge');
    if (badge && di && di.difficulty && di.difficulty !== 'normal') {
      badge.textContent = `Difficulté : ${di.label}`;
      badge.style.display = 'block';
    }
  } catch { /* cosmétique : jamais bloquant */ }

  // « Continuer » : restaure la sauvegarde. Le niveau de boot est déjà
  // aligné sur celui du save (clé v3 ou repli dimensions), donc on restaure
  // toujours ; un ?level= explicite aurait déjà priorisé (résolution
  // au-dessus) et le moteur re-switch de toute façon (load v3 multi-zones).
  if (CONTINUE_SAVE) {
    try {
      game.loadGame(CONTINUE_SAVE); // re-switch interne si le save v3 vise une autre zone
      syncZoneAfterLoad();
      flash('Partie chargée — bon retour dans la zone.');
    } catch (e) {
      note(`Sauvegarde incompatible (${e && e.message ? e.message : e}) — nouvelle partie.`);
    }
  }
  // ?run=1 : course engagée au boot (E2E — l'automation ne peut pas tenir Maj).
  if (PARAMS.get('run') === '1') loop.queue.push({ type: 'setRun', on: true });

  // --- niveau courant : HUD + helpers multi-niveaux -------------------------
  const li = JSON.parse(game.levelInfo() || '{}');
  window.__d2.level = li.key || LEVEL_REQ;
  hud.zone.textContent = li.name || manifest.level.name || 'Camp des Rousses';
  showLevelBanner(hud.zone.textContent);
  if (window.D2Music) window.D2Music.setLevel(li.key, li.name); // EXP-A1 : thème de la zone de boot
  // Debug/E2E : téléporte via le moteur (mêmes points d'entrée que les warps).
  window.__d2.goto = (lvl) => {
    const block = manifest.levels && manifest.levels[lvl];
    if (!block || !block.warp) { note(`Pas de warp défini vers « ${lvl} ».`); return; }
    game.switchLevel(lvl, block.warp.entry[0], block.warp.entry[1]);
  };

  function buildStatusBase() {
    const [mw, mh] = game.mapPixelSize();
    const [gw, gh] = game.gridSize();
    const [drawn, hidden, , oob] = game.composeStats();
    return `Héros : ${HERO_NAME} · Zone : ${hud.zone.textContent} · Carte ${mw}×${mh}px · grille ${gw}×${gh}` +
      ` · tuiles dessinées ${drawn} (cachées ${hidden}, hors bornes ${oob})` +
      ' · clic = déplacer · Maj = courir · clic monstre = attaquer · E = ramasser · P = potion · 1-4 = ceinture · I = inventaire';
  }
  loop.statusBase = buildStatusBase();
  window.__d2.refreshStatus = () => { loop.statusBase = buildStatusBase(); };
  status.textContent = loop.statusBase;

  await loadSfx(manifest);
  wireCommon(manifest);
  // EXP-H* (HUD parité) : init de l'overlay DOM (boutons L/R, minipanel,
  // survol monstre, labels Alt, dégâts flottants, endurance) — hud.js.
  if (window.D2HUD) window.D2HUD.init(manifest, window.__d2.game);
  $('loading').style.display = 'none';
  window.__d2.mode = 'wasm';

  const imgData = ctx.createImageData(canvas.width, canvas.height);
  function frame(now) {
    const dt = Math.min(now - loop.last, 100);
    loop.last = now;
    try {
      const inputJson = loop.queue.length ? JSON.stringify(loop.queue.shift()) : null;
      const events = JSON.parse(game.tick(dt, inputJson));
      handleEvents(events, manifest);
      if (window.D2HUD) window.D2HUD.events(events); // EXP-H7 : dégâts flottants
      const stats = JSON.parse(game.heroStats());
      updateHud(stats);
      if (window.D2HUD) window.D2HUD.update(stats); // EXP-H2 : barre d'endurance
      window.__d2.invCache = JSON.parse(game.inventory());
      window.__d2.monCodes = JSON.parse(game.monsterCodes());
      const hs = JSON.parse(game.heroState());
      hud.run.style.display = hs.run && hs.moving ? 'block' : 'none';
      imgData.data.set(game.render());
      ctx.putImageData(imgData, 0, 0);
      drawGlobes(stats);
      if (inv.open) drawInventory();
      const [hsx, hsy] = game.heroScreen();
      const [hwx, hwy] = game.screenToWorld(hsx, hsy);
      if (loop.lastClick && now - loop.lastClick.t < 450) {
        drawClickRing(loop.lastClick.sx, loop.lastClick.sy, 1 - (now - loop.lastClick.t) / 450);
      }
      if (window.D2HUD) window.D2HUD.frame(canvas, game); // EXP-H6/H8 : survol monstre + labels Alt
      if (window.D2CorpseUI) window.D2CorpseUI.frame(canvas, game); // J1 : flare + hint du corps au sol
      if (window.D2Waypoints && (!loop.lastWp || now - loop.lastWp > 500)) {
        loop.lastWp = now; // W19 : état waypoints ~2×/s (découverte au contact)
        try { window.D2Waypoints.refresh(JSON.parse(game.waypoints())); } catch { /* cosmétique */ }
        if (window.D2Merc) window.D2Merc.frame(game); // H : carte mercenaire (même throttle)
      }
      if (window.D2Npc) window.D2Npc.frame(canvas, game); // W16 : marqueurs NPC + bulles de dialogue
      fpsTick(loop.statusBase, ` · héros ${hwx | 0},${hwy | 0} (${hs.anim})`);
      requestAnimationFrame(frame);
    } catch (err) {
      fail('tick', err);
    }
  }
  // Waypoints (W05/W19) : panneau + rafraîchissement ~2×/s de l'état
  // (waypoints découverts/activés — game.waypoints() est du JSON du moteur).
  if (window.D2Waypoints) {
    window.D2Waypoints.init({
      getGame: () => window.__d2.game,
      queue: loop.queue,
      note,
      flash,
      manifest,
      sfx,
      onTraveled: () => syncZoneAfterLoad(),
    });
    try { window.D2Waypoints.refresh(JSON.parse(game.waypoints())); } catch { /* panneau cosmétique */ }
  }
  // Compétences (C) : panneau arbre (T / bouton) — refresh à l'ouverture.
  if (window.D2Skills) {
    window.D2Skills.init({ getGame: () => window.__d2.game, flash, note });
  }
  // Mercenaire (H) : carte vie/KO — pollée dans la boucle (throttle waypoints).
  if (window.D2Merc) window.D2Merc.init({ getGame: () => window.__d2.game });
  // W16 villes vivantes : NPC du camp — marqueurs + bulles au clic
  // (npc-ui.js ; no-op si le pkg wasm ne porte pas encore npcs()/talkNpc()).
  if (window.D2Npc) {
    window.D2Npc.init({ getGame: () => window.__d2.game, queue: loop.queue, note, flash, manifest, sfx });
    window.D2Npc.frame(canvas, game); // premier rendu des marqueurs
  }
  // 6b : export/import .d2s — toute la logique vit dans d2s-io.js (le
  // moteur WASM encode/décode via d2rules::save_d2s) ; on ne branche que
  // les boutons et les aides d'affichage.
  if (window.D2sIO) window.D2sIO.init({ getGame: () => window.__d2.game, note, flash });
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
  const fbStats = { life: 55, max_life: 55, mana: 15, max_mana: 15, level: 1, xp: 0, gold: 0 };
  updateHud(fbStats);

  const fb = {
    hx: 400, hy: 300, tx: 400, ty: 300, speed: 120,
    mons: Array.from({ length: 3 }, (_, i) => ({
      x: 120 + i * 250, y: 150 + (i % 2) * 260, hp: 3,
      c: ['#c0392b', '#7d6608', '#a04000'][i],
    })),
  };
  canvas.addEventListener('click', (e) => {
    const [px, py] = canvasXY(e);
    fb.tx = px; fb.ty = py;
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
    drawGlobes(fbStats);
    fpsTick(statusBase);
    requestAnimationFrame(frame);
  }
  requestAnimationFrame(frame);
}

main().catch((e) => fallback(e && e.message ? e.message : String(e)));

import("./stash-ui.js").catch(() => {}); // 11c : panneau Stash 6×4 + or (window.D2Stash, miroir economy.rs)
import("./game-sfx.js").catch(() => {}); // 14b : SFX gameplay par event (window.D2SFX, miroir game_audio.rs — file non bloquante, mute partagé musique)
import("./cube-ui.js").catch(() => {}); // 18c : Cube Horadrim 3×4 + tooltips fr (window.D2CubeUI, moteurs cube.rs/tooltips)
import("./panels-ui.js").catch(() => {}); // 18b : UI fidèle — perso/inventaire/stash (window.D2Panels, heroPanel+stash* economy.rs)
