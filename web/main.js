// Diablo 2 in the browser — WASM MPQ reader + DC6 decoder demo.
import init, { WasmMpq, decode_dc6, indices_to_rgba } from './pkg/d2wasm.js';

const $ = (id) => document.getElementById(id);
const status = $('status');

// Surface any runtime error visibly (mission requirement: no silent failures).
window.addEventListener('error', (e) => fail('Erreur JS', e.error || e.message));
window.addEventListener('unhandledrejection', (e) => fail('Promesse rejetée', e.reason));

function fail(where, e) {
  const msg = `${where} : ${e && e.message ? e.message : e}`;
  status.textContent = msg;
  status.classList.add('err');
  console.error(msg);
}

// D2 palettes ship 767 bytes (256xRGB minus the very last byte) — zero-pad to 768.
function padPalette(bytes) {
  const pal = new Uint8Array(768);
  pal.set(bytes.subarray(0, Math.min(bytes.length, 768)));
  return pal;
}

// Draw one decoded DC6 frame onto a 2D context at (dx, dy), zoom x `scale`.
function drawFrame(ctx, frame, palette, dx, dy, scale) {
  const rgba = indices_to_rgba(frame.indices, palette);
  const off = new OffscreenCanvas(frame.width, frame.height);
  const octx = off.getContext('2d');
  octx.putImageData(new ImageData(new Uint8ClampedArray(rgba), frame.width, frame.height), 0, 0);
  ctx.imageSmoothingEnabled = false;
  ctx.drawImage(off, dx, dy, frame.width * scale, frame.height * scale);
}

async function fetchBytes(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(`fetch ${url} -> HTTP ${r.status}`);
  // Fallback SPA des hébergeurs statiques (ex. Pages sans 404.html) : un
  // asset manquant revient en 200 text/html (l'index). Refuser proprement.
  const ct = (r.headers.get('content-type') || '').toLowerCase();
  if (ct.includes('text/html')) {
    throw new Error(`fetch ${url} -> HTML au lieu d'un binaire (asset absent du déploiement)`);
  }
  return new Uint8Array(await r.arrayBuffer());
}

// ---- Section 1 : skillicon grid (8 x 3, zoom x3) --------------------------
async function renderIcons(palette) {
  const bytes = await fetchBytes('assets/skillicon.dc6');
  const frames = decode_dc6(bytes);
  const canvas = $('icons');
  const ctx = canvas.getContext('2d');
  ctx.imageSmoothingEnabled = false;
  const CELL = 144, ZOOM = 3;
  frames.forEach((f, i) => {
    const cx = (i % 8) * CELL, cy = Math.floor(i / 8) * CELL;
    drawFrame(ctx, f, palette, cx + (CELL - f.width * ZOOM) / 2, cy + (CELL - f.height * ZOOM) / 2, ZOOM);
  });
  $('icons-info').textContent =
    `24 frames 48×48 décodées en ${(performance.now()).toFixed(1)} ms (déchiffrement MPQ déjà fait côté extraction).`;
}

// ---- Section 2 : user MPQ -> invcap.dc6 ------------------------------------
async function renderFromMpq(bytes, label) {
  const msg = $('mpq-msg');
  msg.textContent = '';
  msg.style.color = '';
  let mpq;
  try {
    mpq = new WasmMpq(bytes);
  } catch (e) {
    msg.textContent = `Archive illisible : ${e}`;
    msg.style.color = 'var(--red)';
    return;
  }
  const NAME_DC6 = 'data\\global\\items\\invcap.dc6';
  const NAME_PAL = 'data\\global\\palette\\act1\\pal.dat';
  if (!mpq.has(NAME_DC6)) {
    msg.textContent = `${label} ouvert, mais data\\global\\items\\invcap.dc6 est absent de cette archive.`;
    msg.style.color = 'var(--red)';
    return;
  }
  const dc6 = decode_dc6(mpq.read(NAME_DC6));
  // Palette : celle de l'archive d'abord, sinon la palette locale de démo
  // (absente des déploiements BYO).
  let palette = PAL;
  const rawPal = mpq.read(NAME_PAL);
  if (rawPal) palette = padPalette(rawPal); // l'archive fournit sa propre palette
  if (!palette) {
    msg.textContent = `${label} décodé, mais ni le déploiement ni l'archive ne fournissent data\\global\\palette\\act1\\pal.dat — impossible d'appliquer les couleurs.`;
    msg.style.color = 'var(--red)';
    return;
  }
  const frame = dc6[0];
  const canvas = $('helm');
  const ctx = canvas.getContext('2d');
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  const zoom = Math.floor(Math.min(canvas.width / frame.width, canvas.height / frame.height));
  drawFrame(ctx, frame, palette,
    (canvas.width - frame.width * zoom) / 2, (canvas.height - frame.height * zoom) / 2, zoom);
  msg.textContent = `${label} — invcap.dc6 rendu (${frame.width}×${frame.height}, zoom ×${zoom})` +
    (rawPal ? ' avec la palette de l\'archive.' : ' avec la palette act1 locale.');
}

// ---- Section 3 : TSV table --------------------------------------------------
async function renderArmor() {
  const text = await (await fetch('assets/patch_armor.txt')).text();
  const lines = text.split(/\r?\n/).filter((l) => l.length > 0);
  const header = lines[0].split('\t').slice(0, 10);
  const rows = lines.slice(1, 13).map((l) => l.split('\t').slice(0, 10));
  const table = $('armor');
  const thead = document.createElement('thead');
  const trh = document.createElement('tr');
  header.forEach((h) => {
    const th = document.createElement('th');
    th.textContent = h;
    trh.appendChild(th);
  });
  thead.appendChild(trh);
  table.appendChild(thead);
  const tbody = document.createElement('tbody');
  rows.forEach((r) => {
    const tr = document.createElement('tr');
    r.forEach((cell, i) => {
      const td = document.createElement('td');
      td.textContent = cell.trim();
      if (i > 0 && /^-?\d+$/.test(cell.trim())) td.className = 'num';
      tr.appendChild(td);
    });
    tbody.appendChild(tr);
  });
  table.appendChild(tbody);
}

// ---- Boot -------------------------------------------------------------------
$('mpq-file').addEventListener('change', async (ev) => {
  const f = ev.target.files[0];
  if (f) renderFromMpq(new Uint8Array(await f.arrayBuffer()), f.name).catch((e) => fail('MPQ', e));
});
$('mpq-sample').addEventListener('click', () => {
  fetchBytes('assets/probe_d2data_invcap.dc6')
    .then((dc6bytes) => {
      // pas de MPQ ici : on décode directement l'asset extrait de d2data.mpq
      const frames = decode_dc6(dc6bytes);
      const palette = padPaletteSync();
      if (!palette) throw new Error('palette locale absente (déploiement BYO) — chargez votre MPQ, qui fournit la sienne');
      const frame = frames[0];
      const canvas = $('helm');
      const ctx = canvas.getContext('2d');
      ctx.clearRect(0, 0, canvas.width, canvas.height);
      const zoom = Math.floor(Math.min(canvas.width / frame.width, canvas.height / frame.height));
      drawFrame(ctx, frame, palette,
        (canvas.width - frame.width * zoom) / 2, (canvas.height - frame.height * zoom) / 2, zoom);
      $('mpq-msg').textContent =
        'Asset local invcap.dc6 (extrait de d2data.mpq) rendu — chargez un vrai MPQ ci-dessus pour tout décoder en direct.';
      $('mpq-msg').style.color = '';
    })
    .catch((e) => fail('Exemple', e));
});

let PAL = null;
function padPaletteSync() { return PAL; }

// Note visible de dégradation (assets de démo absents du déploiement BYO).
function degrade(sectionId, msgId, text) {
  const sec = $(sectionId);
  if (sec) sec.style.opacity = '0.45';
  const m = $(msgId);
  if (m) { m.textContent = text; m.style.color = 'var(--red)'; }
}

init()
  .then(async () => {
    let missing = [];
    try {
      PAL = padPalette(await fetchBytes('assets/pal_act1.dat'));
    } catch (e) {
      missing.push('palette act1');
    }
    try {
      await renderIcons(PAL);
    } catch (e) {
      missing.push('skillicon.dc6');
      degrade('icons-section', 'icons-info', 'Icônes indisponibles : asset non inclus dans ce déploiement (BYO).');
    }
    try {
      await renderArmor();
    } catch (e) {
      missing.push('patch_armor.txt');
      degrade('armor-section', 'armor-info', 'Table d\'armures indisponible : asset non inclus dans ce déploiement (BYO).');
    }
    status.textContent = missing.length === 0
      ? 'Moteur WebAssembly prêt — les 24 icônes et le TSV sont décodés côté navigateur.'
      : `Moteur WebAssembly prêt (mode BYO — assets de démo absents : ${missing.join(', ')}). Chargez votre MPQ ci-dessous : tout est décodé en direct depuis votre archive.`;
    $('app').hidden = false;
    // ?mpq=<url> : charge une archive au démarrage (deep-link de test/veille).
    const mpqUrl = new URLSearchParams(location.search).get('mpq');
    if (mpqUrl) {
      status.textContent = `Chargement de ${mpqUrl}…`;
      fetchBytes(mpqUrl)
        .then((b) => renderFromMpq(b, mpqUrl.split('/').pop()))
        .catch((e) => fail('MPQ distant', e));
    }
  })
  .catch((e) => fail('Échec du chargement WASM', e));
