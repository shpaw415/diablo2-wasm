// Diablo 2 WASM — musique par acte/zone + ambiance + mixage — migration TS
// 25a de demo/music.js (EXP-A1 + 14c), sur le moteur audio PARTAGÉ 25b.
//
// Contrats window.D2Music / window.D2Audio INCHANGÉS (game.js : setLevel au
// boot / LevelSwitch / syncZoneAfterLoad ; hud.js : rien ; bouton Musique +
// panneau Audio ⚙ avec volumes persistés par bus).
//
// TRANSITION : music.js est un <script> classique de game.html — il s'est
// déjà auto-installé (boutons + unlock + autoplay « town1 ») AVANT le
// bundle. Remplacer le contrat créerait DEUX lecteurs (double audio) :
// pendant la transition on ADOPT donc l'objet Legacy s'il existe ; la voie
// TS complète ci-dessous (moteur partagé getAudioEngine(), ZoneMusic,
// tables zones.ts 25b) prend le relais dès que le tag Legacy disparaît.
import {
  AMB_BASE, DEFAULT_VOLUMES, MUTE_KEY, clamp01, getAudioEngine,
  SfxQueue, ZoneMusic,
  type AudioEngine, type VolumeKind,
} from './audio/index';
import { registerModule, type D2AudioApi, type D2MusicApi, type D2Window } from './types';

// --- boutons toolbar / panneau de mixage (état module, voie TS) ------------
let btn: HTMLButtonElement | null = null;
let panel: HTMLDivElement | null = null;
const SLIDERS: ReadonlyArray<readonly [VolumeKind, string]> = [
  ['music', 'Musique'], ['amb', 'Ambiance'], ['sfx', 'Effets'],
];

function buildPanel(engine: AudioEngine): void {
  panel = document.createElement('div');
  panel.id = 'audio-mix';
  panel.style.cssText = 'display:none;position:fixed;right:10px;bottom:10px;z-index:60;' +
    'background:rgba(20,17,10,.94);border:1px solid #5a3c1e;padding:10px 12px;' +
    'font:11px/1.6 monospace;color:#d0b070;min-width:190px;';
  const title = document.createElement('div');
  title.textContent = 'MIXAGE AUDIO';
  title.style.cssText = 'color:#ffe9ad;margin-bottom:6px;letter-spacing:1px;';
  panel.appendChild(title);
  const rows: Array<[VolumeKind, HTMLLabelElement, HTMLInputElement]> = [];
  for (const [kind, label] of SLIDERS) {
    const lab = document.createElement('label');
    lab.style.cssText = 'display:flex;align-items:center;gap:6px;';
    lab.textContent = label + ' ';
    const range = document.createElement('input');
    range.type = 'range';
    range.min = '0';
    range.max = '100';
    range.step = '1';
    range.value = String(Math.round(engine.volumeOf(kind) * 100));
    range.style.cssText = 'flex:1;accent-color:#c7a45a;';
    range.addEventListener('input', () => {
      engine.setVolume(kind, Number(range.value) / 100); // persisté + gain live
      applyMuteVisual(engine);
    });
    lab.appendChild(range);
    panel.appendChild(lab);
    rows.push([kind, lab, range]);
  }
  (panel as HTMLDivElement & { rows?: unknown }).rows = rows;
  const hint = document.createElement('div');
  hint.textContent = 'la nuit tombe après 20 h (D2Audio.setNight)';
  hint.style.cssText = 'margin-top:6px;opacity:.6;';
  panel.appendChild(hint);
  document.body.appendChild(panel);
}

function applyMuteVisual(engine: AudioEngine): void {
  if (btn) btn.textContent = engine.isMuted() ? 'Musique : non' : 'Musique : oui';
  if (panel) {
    const rows = (panel as HTMLDivElement & { rows?: Array<[VolumeKind, HTMLLabelElement, HTMLInputElement]> }).rows ?? [];
    for (const [, lab, range] of rows) {
      range.disabled = engine.isMuted(); // muet : curseurs grisés
      lab.style.opacity = engine.isMuted() ? '0.5' : '1';
    }
  }
}

function initButton(engine: AudioEngine, zoneMusic: ZoneMusic): void {
  try {
    btn = document.createElement('button');
    btn.id = 'btn-music';
    btn.title = 'Musique par zone (fondu doux au changement — EXP-A1)';
    btn.addEventListener('click', (ev) => {
      ev.stopPropagation();
      engine.unlock(); // le clic EST un geste : autorise aussi le (re)démarrage
      zoneMusic.toggle();
      applyMuteVisual(engine);
    });
    const mix = document.createElement('button');
    mix.id = 'btn-audio';
    mix.title = 'Volumes : musique / ambiance / effets (persistés)';
    mix.textContent = 'Audio ⚙';
    mix.addEventListener('click', (ev) => {
      ev.stopPropagation();
      engine.unlock(); // geste = autorise la lecture aussi
      if (!panel) buildPanel(engine);
      if (!panel) return;
      const open = panel.style.display !== 'none';
      panel.style.display = open ? 'none' : 'block';
    });
    const ref = document.getElementById('btn-sound');
    if (ref && ref.parentNode) {
      ref.parentNode.insertBefore(btn, ref);
      ref.parentNode.insertBefore(mix, btn);
    } else {
      document.body.appendChild(mix);
      document.body.appendChild(btn);
    }
    applyMuteVisual(engine);
  } catch { /* pas de bouton : le clavier/jeu continuent de fonctionner */ }
}

/** Voie TS complète : moteur partagé + lecteur par zone + boutons + unlock. */
function installTsMusic(win: D2Window): void {
  registerModule('music', 'ts');
  const engine = getAudioEngine();
  const zoneMusic = new ZoneMusic(engine);
  const ambQueue = new SfxQueue(engine, AMB_BASE); // one-shots positionnels 14c

  // autoplay au premier geste (politique navigateur) — comme music.js
  window.addEventListener('pointerdown', () => engine.unlock());
  window.addEventListener('keydown', () => engine.unlock());
  zoneMusic.start(); // onUnlock -> applique le thème courant (town1 par défaut)

  const musicApi: D2MusicApi = {
    setLevel: (key, name) => zoneMusic.setLevel(key, name),
    setMuted: (m) => {
      zoneMusic.setMuted(m);
      applyMuteVisual(engine);
    },
    toggle: () => {
      zoneMusic.toggle();
      applyMuteVisual(engine);
    },
    theme: () => zoneMusic.theme(),
    isMuted: () => zoneMusic.isMuted(),
  };

  const audioApi: D2AudioApi = {
    pan: (xScreen, refWidth) => engine.pan(xScreen, refWidth),
    play: (name, xScreen) => ambQueue.playAt(name, xScreen),
    setAmbience: (stem) => zoneMusic.setAmbience(stem),
    setNight: (n) => zoneMusic.setNight(n),
    ambianceFor: (key, name) => zoneMusic.ambianceForNow(key, name),
    setVolume: (kind, v) => engine.setVolume(kind, clamp01(v)),
    volume: (kind) => engine.volumeOf(kind),
    panValue: () => engine.panValueNow(),
    ambient: () => zoneMusic.ambient(),
  };

  win.D2Music = musicApi;
  win.D2Audio = audioApi;

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', () => initButton(engine, zoneMusic), { once: true });
  } else {
    initButton(engine, zoneMusic);
  }
}

export function installMusic(win: D2Window): void {
  if (win.D2Music) {
    registerModule('music', 'adopt');
    return;
  } // TRANSITION
  // TRANSITION : music.js Legacy déjà actif — on adopte
  installTsMusic(win);
}

if (typeof window !== 'undefined') installMusic(window as unknown as D2Window);

// Exports de contrat pour les tests / autres modules TS.
export { DEFAULT_VOLUMES, MUTE_KEY };
