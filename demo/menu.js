// menu.js — Écran-titre Diablo II (WASM). Sélection de classe -> game.html?hero=BA|SOR|PA
'use strict';

let audioCtx = null;
let clickBuffer = null;

function initAudio() {
  try {
    audioCtx = new (window.AudioContext || window.webkitAudioContext)();
  } catch (e) {
    audioCtx = null;
    return;
  }
  fetch('assets/sfx/ui_click.wav')
    .then((r) => (r.ok ? r.arrayBuffer() : Promise.reject(r.status)))
    .then((buf) => audioCtx.decodeAudioData(buf))
    .then((buf) => { clickBuffer = buf; })
    .catch(() => { clickBuffer = null; }); // pas de son -> clic muet, zéro erreur console
}

function playClick() {
  if (!audioCtx || !clickBuffer) return;
  const src = audioCtx.createBufferSource();
  src.buffer = clickBuffer;
  src.connect(audioCtx.destination);
  src.start();
}

function selectedHero() {
  const el = document.querySelector('.class-card.selected');
  return el ? el.dataset.hero : 'BA';
}

function selectCard(card) {
  document.querySelectorAll('.class-card').forEach((c) => c.classList.remove('selected'));
  card.classList.add('selected');
}

function scaleStage() {
  const stage = document.getElementById('stage');
  const s = Math.min(window.innerWidth / 800, window.innerHeight / 600);
  stage.style.transform = `scale(${s})`;
}

// Portraits : chargement tolérant (sor/pal produits par un autre membre —
// absent -> initiales, aucune erreur console).
function loadPortraits() {
  document.querySelectorAll('.portrait img[data-src]').forEach((img) => {
    fetch(img.dataset.src)
      .then((r) => (r.ok ? r.blob() : Promise.reject(r.status)))
      .then((blob) => {
        img.src = URL.createObjectURL(blob);
        img.addEventListener('load', () => img.classList.add('ready'), { once: true });
      })
      .catch(() => { /* initiales affichées */ });
  });
}

document.addEventListener('DOMContentLoaded', () => {
  scaleStage();
  window.addEventListener('resize', scaleStage);
  loadPortraits();
  initAudio();

  document.querySelectorAll('.class-card').forEach((card) => {
    card.addEventListener('click', () => {
      selectCard(card);
      playClick();
    });
  });

  document.getElementById('play').addEventListener('click', () => {
    playClick();
    // laisse le clic s'entendre avant la navigation
    setTimeout(() => {
      window.location.href = `game.html?hero=${selectedHero()}`;
    }, 120);
  });

  document.addEventListener('keydown', (ev) => {
    if (ev.key === 'Enter') document.getElementById('play').click();
  });
});
