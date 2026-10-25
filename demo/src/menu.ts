// Diablo II — menu : écran-titre + écran de sélection de héros (étape 25b :
// migration TypeScript fidèle de demo/menu.js — le plus gros fichier UI).
//
// Les classes affichées viennent TOUJOURS de assets/manifest.json (heroes{})
// — aucune classe codée en dur ici. Dégradation BYO : chaque asset est
// sondé ; s'il manque (ex : build Pages sans assets/menu), un repli 100 % CSS
// prend le relais, et un héros sans sprites est marqué « bientôt » au lieu
// de faire planter la sélection.
//
// Écran de sélection, deux modes :
//  - SCÈNE ANIMÉE si le contrat assets/menu/heroes.json est présent
//    ({CODE:{file,frames,w,h,fps}} + heroes/<CODE>.png, possiblement fire) :
//    canvas plein écran, héros en idle autour du feu central, survol/souris
//    + clavier, panneau de stats, un seul rAF.
//  - CARTES sinon (repli historique).
// Dans les deux modes la liste, l'ordre, les noms et les stats viennent du
// manifest heroes{} — heroes.json ne décrit que l'animation.
//
// Contrats conservés à l'identique :
//  - URLs : game.html?hero=CODE(&name=) · game.html?continue=1 ·
//    game.html?slot=ID — et l'ouverture GitHub du projet ;
//  - localStorage : d2hero, d2heroName, d2wasm.save.v1 (lecture),
//    d2wasm.musicMuted (clé MUTUALISÉE avec demo/music.js via
//    src/audio/engine.ts — mute persistant menu/jeu) ;
//  - window.D2Slots (slots.js, non migré) pour l'écran « Parties ».
'use strict';

import { MUTE_KEY } from './audio/engine';

// ================= types des données (manifest + contrat heroes.json) ==========

/** Stats affichées du manifest (champs utilisés par le menu uniquement). */
interface HeroStats {
  life?: number | null;
  mana?: number | null;
  strength?: number | null;
  dexterity?: number | null;
  [key: string]: unknown;
}

/** Définition d'un héros du manifest (heroes{}). */
interface HeroDef {
  name?: string;
  stats?: HeroStats | null;
  modes?: Record<string, unknown> | null;
  walk?: { layers?: Record<string, unknown> | null } | null;
  [key: string]: unknown;
}

/** assets/manifest.json — seuls heroes{} est consommé ici. */
interface GameManifest {
  heroes?: Record<string, HeroDef> | null;
  [key: string]: unknown;
}

/** Une feuille de sprites du contrat (frames collées horizontalement). */
interface SheetRef {
  file: string;
  frames: number;
  w: number;
  h: number;
  fps?: number;
  /** courbe de progression RACINE par frame (marche fw/bw). */
  prog?: number[] | null;
}

/** Entrée d'un héros dans heroes.json : repos + variantes + marches. */
type SheetEntry = SheetRef & {
  nu1?: SheetRef | null;
  nu2?: SheetRef | null;
  nu3?: SheetRef | null;
  fw?: SheetRef | null;
  bw?: SheetRef | null;
};

/** assets/menu/heroes.json : un record par code de classe (+fire/background). */
interface MenuContract {
  background?: string;
  fire?: SheetRef | null;
  FIRE?: SheetRef | null;
  [code: string]: unknown;
}

/** Feuille chargée et normalisée (fps >= 1, prog validée). */
interface LoadedSheet {
  img: HTMLImageElement;
  frames: number;
  w: number;
  h: number;
  fps: number;
  prog: number[] | null;
}

/** Pas de marche façon D2 : la POSITION glisse avec la séquence. */
interface WalkState {
  kind: 'fw' | 'bw';
  start: number;
  from: { x: number; y: number };
  to: { x: number; y: number };
  dur: number;
}

/** Héros affiché (cartes + scène animée). */
interface HeroEntry {
  code: string;
  def: HeroDef | null;
  playable: boolean;
  el: HTMLDivElement;
  anim: LoadedSheet | null;
  animHover: LoadedSheet | null;
  animSel: LoadedSheet | null;
  fw: LoadedSheet | null;
  bw: LoadedSheet | null;
  walk: WalkState | null;
  off: { x: number; y: number };
  pos: { x: number; y: number };
  st: LoadedSheet | null;
  stStart: number;
}

/** Slot de sauvegarde exposé par slots.js (window.D2Slots, non migré). */
interface SlotInfo {
  id: string;
  name: string;
  ts: number;
  hero_code: string;
  hero_label?: string;
  level?: number | null;
  zone?: string | null;
  level_key?: string | null;
  playtime_ms?: number | null;
}

interface D2SlotsApi {
  list(): SlotInfo[];
  mostRecent(): SlotInfo | null;
  remove(id: string): void;
  rename(id: string, name: string): void;
  exportSlot(id: string): void;
  importSlot(json: string): SlotInfo;
}

declare global {
  interface Window {
    D2Slots?: D2SlotsApi;
  }
}

// ================= utilitaires ==================================================

const MENU = 'assets/menu/';

/** Message d'erreur lisible depuis un catch (unknown). */
function errText(e: unknown): string {
  return e instanceof Error ? e.message : String(e);
}

/** localStorage protégé (jamais d'exception, même en navigation privée). */
function storeGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}
function storeSet(k: string, v: string): void {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* stockage indisponible */
  }
}
function storeDel(k: string): void {
  try {
    localStorage.removeItem(k);
  } catch {
    /* stockage indisponible */
  }
}

/** lookup DOM typé — les ids existent dans menu.html. */
function $id<T extends HTMLElement>(id: string): T | null {
  return document.getElementById(id) as T | null;
}

function probe(url: string): Promise<boolean> {
  return new Promise((ok) => {
    const im = new Image();
    im.onload = () => ok(!!im.naturalWidth);
    im.onerror = () => ok(false);
    im.src = url;
  });
}

function loadImg(url: string): Promise<HTMLImageElement | null> {
  return new Promise((ok) => {
    const im = new Image();
    im.onload = () => ok(im.naturalWidth ? im : null);
    im.onerror = () => ok(null);
    im.src = url;
  });
}

// ================= boot =========================================================

(function bootMenu(): void {
  const stage = document.getElementById('stage');
  const errlog = document.getElementById('errlog');
  if (!stage || !errlog) return; // pas menu.html : ne rien faire
  window.addEventListener('error', (e) => {
    errlog.textContent += '[err] ' + e.message + '\n';
  });
  try {
    initMenu(stage);
  } catch (e) {
    errlog.textContent += '[err] ' + errText(e) + '\n';
  }
})();

function initMenu(stage: HTMLElement): void {
  // ---- mise à l'échelle du stage 800x600 ----
  function fit(): void {
    const s = Math.min(innerWidth / 800, innerHeight / 600);
    stage.style.setProperty('--s', String(s));
  }
  addEventListener('resize', fit);
  fit();

  // ---- bruitages ----
  function clickSfx(): void {
    try {
      void new Audio('assets/sfx/ui_click.wav').play().catch(() => {});
    } catch {
      /* audio indisponible */
    }
  }

  // bruitages de sélection/désélection par classe (extraits des MPQ)
  const heroSfxCache: Record<string, HTMLAudioElement> = {};
  function playHeroSfx(code: string, kind: 'sel' | 'des'): void {
    const key = code + '-' + kind;
    if (!heroSfxCache[key]) {
      heroSfxCache[key] = new Audio(`assets/sfx/${code}-${kind}.wav`);
    }
    try {
      heroSfxCache[key].currentTime = 0;
      void heroSfxCache[key].play().catch(() => {});
    } catch {
      /* audio indisponible */
    }
  }

  // ---- musique du menu original (thème du titre, ogg extrait du MPQ) ----
  const menuAudio = new Audio('assets/menu/menu-music.ogg');
  menuAudio.loop = true;
  menuAudio.volume = 0.45;
  let musicStarted = false;
  let musicMuted = storeGet(MUTE_KEY) === '1'; // EXP-A1 : clé partagée avec demo/music.js (mute persistant menu/jeu)
  function startMusic(): void {
    if (musicStarted) return;
    musicStarted = true;
    if (!musicMuted) menuAudio.play().catch(() => { musicStarted = false; });
  }
  window.addEventListener('pointerdown', startMusic);
  window.addEventListener('keydown', startMusic);
  function toggleMusic(): void {
    musicMuted = !musicMuted;
    storeSet(MUTE_KEY, musicMuted ? '1' : '0'); // EXP-A1 : persiste la même clé que le jeu
    if (musicMuted) menuAudio.pause();
    else {
      startMusic();
      menuAudio.play().catch(() => {});
    }
    const b = document.getElementById('musicBtn');
    if (b) b.textContent = musicMuted ? '🔇' : '🔊';
  }

  // ---- écrans ----
  const screenTitle = $id<HTMLElement>('screenTitle');
  const screenSelect = $id<HTMLElement>('screenSelect');
  const screenSlots = $id<HTMLElement>('screenSlots');
  let onTitle = true;
  let onSlots = false;
  function setScreen(on: HTMLElement | null, value: boolean): void {
    if (on) on.classList.toggle('on', value);
  }
  function showSelect(): void {
    onTitle = false;
    onSlots = false;
    setScreen(screenTitle, false);
    setScreen(screenSlots, false);
    setScreen(screenSelect, true);
  }
  function showTitle(): void {
    onTitle = true;
    onSlots = false;
    setScreen(screenSelect, false);
    setScreen(screenSlots, false);
    setScreen(screenTitle, true);
  }
  function showSlots(): void {
    onTitle = false;
    onSlots = true;
    setScreen(screenTitle, false);
    setScreen(screenSelect, false);
    setScreen(screenSlots, true);
    renderSlots();
  }

  // ---- dégradation des assets du titre ----
  // fond : si le png manque, le dégradé radial du #stage (CSS) prend le relais
  // (complete&&naturalWidth : erreur déjà survenue avant l'écouteur — 404 rapide)
  const bg = $id<HTMLImageElement>('bg');
  if (bg) {
    const bgGone = () => {
      bg.style.display = 'none';
    };
    bg.addEventListener('error', bgGone);
    if (bg.complete && !bg.naturalWidth) bgGone();
  }
  // feu de camp : fire-sheet.png -> canvas animé, sinon feu CSS (classe .no-fire)
  probe(MENU + 'fire-sheet.png').then((ok) => {
    if (!ok) stage.classList.add('no-fire');
  });

  // ---- logo en flammes : 30 frames (gauche + droite), ~15 fps ----
  // Chaque URL est demandée UNE seule fois (préchargement Image) ; ensuite
  // l'animation ne fait que permuter des src déjà en cache mémoire.
  const FRAMES = 30;
  const logoL = $id<HTMLImageElement>('logoL');
  const logoR = $id<HTMLImageElement>('logoR');
  const pad = (n: number): string => String(n).padStart(2, '0');
  const frames: Array<{ L: HTMLImageElement; R: HTMLImageElement }> = [];
  for (let i = 0; i < FRAMES; i++) {
    const L = new Image();
    const R = new Image();
    L.src = MENU + 'logo_flame_l_' + pad(i) + '.png';
    R.src = MENU + 'logo_flame_r_' + pad(i) + '.png';
    frames.push({ L, R });
  }
  let f = 0;
  const logoTimer = setInterval(() => {
    f = (f + 1) % FRAMES;
    const fr = frames[f];
    if (!fr) return; // n'arrive pas (f < FRAMES) — garde noUncheckedIndexedAccess
    // frame prête (chargée, décodable) seulement — jamais de src en vol
    if (fr.L.complete && fr.R.complete && fr.L.naturalWidth && fr.R.naturalWidth) {
      if (logoL) logoL.src = fr.L.src;
      if (logoR) logoR.src = fr.R.src;
    }
  }, 66);
  // logo : frames manquantes -> titre 100 % CSS
  Promise.all([probe(MENU + 'logo_flame_l_00.png'), probe(MENU + 'logo_flame_r_00.png')])
    .then(([l, r]) => {
      if (l && r) return;
      stage.classList.add('no-logo');
      const logoText = document.getElementById('logoText');
      if (logoText) logoText.removeAttribute('hidden');
      if (logoL) logoL.style.display = 'none';
      if (logoR) logoR.style.display = 'none';
      clearInterval(logoTimer);
    });

  // ---- feu de camp : fire-sheet.png = 30 cellules 180x140 ----
  // fireTimer est global : un éventuel feu du contrat (heroes.json.fire)
  // plus fidèle prend le relais et coupe cette animation.
  let fireTimer: ReturnType<typeof setInterval> | null = null;
  (function (): void {
    const cv = $id<HTMLCanvasElement>('fireCanvas');
    const ctx = cv ? cv.getContext('2d') : null;
    if (!cv || !ctx) return;
    const sheet = new Image();
    sheet.onload = () => {
      if (stage.classList.contains('no-fire')) return;
      let ff = 0;
      fireTimer = setInterval(() => {
        ff = (ff + 1) % 30;
        ctx.clearRect(0, 0, 180, 140);
        ctx.drawImage(sheet, ff * 180, 0, 180, 140, 0, 0, 180, 140);
      }, 66);
    };
    sheet.onerror = () => stage.classList.add('no-fire');
    sheet.src = MENU + 'fire-sheet.png';
  })();

  // ================= sélection de héros =================
  // Libellés et compétences de départ : table locale UNIQUEMENT en repli —
  // le manifest fait foi dès qu'il fournit un champ name (déjà le cas).
  const LABELS: Record<string, string> = {
    BA: 'Barbare', SOR: 'Sorcière', PA: 'Paladin', AM: 'Amazone',
    AS: 'Assassin', DZ: 'Druide', NE: 'Nécromancien',
  };
  const SKILLS: Record<string, string> = {
    BA: 'Frappe', SOR: 'Trait de feu', PA: 'Sacrifice', AM: 'Flèche magique',
    AS: 'Explosion ardente', DZ: 'Tempête de feu', NE: 'Dégâts amplifiés',
  };
  const ORDER: ReadonlyArray<string> = ['AM', 'AS', 'BA', 'DZ', 'NE', 'PA', 'SOR']; // ordre alphabétique fr, inconnus ajoutés à la fin

  const heroGrid = $id<HTMLDivElement>('heroGrid');
  const playBtn = $id<HTMLButtonElement>('play');
  const nameInput = $id<HTMLInputElement>('heroName');
  const manifestErr = $id<HTMLDivElement>('manifestErr');
  const sceneCanvas = $id<HTMLCanvasElement>('sceneCanvas');
  const sctx = sceneCanvas ? sceneCanvas.getContext('2d') : null;
  const sceneStats = $id<HTMLDivElement>('sceneStats');
  const SCALE = 1.0;       // poses FrontEnd (charactercreationscreen) déjà à l'échelle écran
  let heroList: HeroEntry[] = [];   // ordre d'affichage
  let selectedCode: string | null = null;
  const scene = { on: false, hover: null as string | null };   // mode scène animée

  function heroName(def: HeroDef | null, code: string): string {
    return (def && def.name) || LABELS[code] || code;
  }
  function isPlayable(def: HeroDef | null | undefined): boolean {
    if (!def || typeof def !== 'object') return false;
    const hasModes = !!(def.modes && (def.modes.WL || def.modes.NU || def.modes.A1));
    const hasWalk = !!(def.walk && def.walk.layers && Object.keys(def.walk.layers).length);
    return hasModes || hasWalk;
  }
  function orderedCodes(heroes: Record<string, HeroDef>): string[] {
    return Object.keys(heroes).sort((a, b) => {
      const ia = ORDER.indexOf(a);
      const ib = ORDER.indexOf(b);
      return (ia < 0 ? ORDER.length : ia) - (ib < 0 ? ORDER.length : ib);
    });
  }

  function buildCards(manifest: GameManifest | null): void {
    if (!heroGrid || !manifestErr || !playBtn) return;
    const heroes = (manifest && manifest.heroes) || {};
    const codes = orderedCodes(heroes);
    if (!codes.length) {
      manifestErr.hidden = false;
      manifestErr.textContent = 'Aucun héros dans assets/manifest.json (heroes{}) — sélection indisponible.';
      return;
    }
    heroGrid.textContent = '';
    heroList = codes.map((code): HeroEntry => {
      const def = heroes[code] || null;
      const playable = isPlayable(def);
      const s = (def && def.stats) || {};
      const el = document.createElement('div');
      el.className = 'card' + (playable ? '' : ' disabled');
      el.dataset.hero = code;
      el.tabIndex = playable ? 0 : -1;
      el.setAttribute('role', 'button');
      el.setAttribute('aria-disabled', String(!playable));
      const nm = heroName(def, code);
      el.innerHTML =
        '<div class="med">' + code + '</div>' +
        '<div class="cname"></div>' +
        '<div class="stats">' +
        '<span>Vie <b>' + (s.life != null ? s.life : '?') + '</b></span>' +
        '<span>Mana <b>' + (s.mana != null ? s.mana : '?') + '</b></span>' +
        '<span>For <b>' + (s.strength != null ? s.strength : '?') + '</b></span>' +
        '<span>Dex <b>' + (s.dexterity != null ? s.dexterity : '?') + '</b></span>' +
        '</div>' +
        '<div class="skill">Compétence : ' + (SKILLS[code] || '—') + '</div>' +
        (playable ? '' : '<div class="soon">bientôt</div>');
      const cname = el.querySelector('.cname');
      if (cname) cname.textContent = nm;
      if (playable) {
        el.addEventListener('click', () => {
          selectHero(code);
          clickSfx();
        });
      }
      heroGrid.appendChild(el);
      return {
        code, def, playable, el,
        anim: null, animHover: null, animSel: null, fw: null, bw: null,
        walk: null, off: { x: 0, y: 0 }, pos: { x: 0, y: 0 },
        st: null, stStart: 0,
      };
    });
    // présélection : dernier héros joué (s'il est jouable), sinon le premier jouable
    const last = (storeGet('d2hero') || '').toUpperCase();
    const pre = heroList.find((h) => h.playable && h.code === last) || heroList.find((h) => h.playable);
    if (pre) selectHero(pre.code, true);
  }

  // héros sélectionnables selon le mode actif :
  // cartes -> manifest jouable ; scène -> manifest jouable ET sheet chargée
  function selectable(): HeroEntry[] {
    return heroList.filter((h) => (scene.on ? h.playable && !!h.anim : h.playable));
  }

  function selectHero(code: string, silent?: boolean): void {
    const entry = selectable().find((h) => h.code === code);
    if (!entry) return;
    const prev = selectedCode;
    selectedCode = code;
    for (const h of heroList) h.el.classList.toggle('selected', h.code === code);
    if (playBtn) playBtn.disabled = false;
    // animations de marche façon D2 : le nouveau héros avance d'un pas vers
    // la caméra (FW), l'ancien recule (BW) — la POSITION glisse en synchro
    // avec la séquence (de là où il est vers la cible), plus de héros qui
    // marche sur place.
    if (scene.on && prev !== code) {
      const now = performance.now();
      const STEP = { x: 0, y: 5 }; // pas vers la caméra, en % du stage
      const step = (h: HeroEntry, kind: 'fw' | 'bw', to: { x: number; y: number }): void => {
        const sheet = kind === 'fw' ? h.fw : h.bw;
        if (!sheet) {
          h.off = { ...to };
          h.walk = null;
          return;
        }
        h.walk = {
          kind, start: now, from: { ...h.off }, to: { ...to },
          dur: (sheet.frames / sheet.fps) * 1000, // fps normalisé >= 1 dans loadState
        };
      };
      const prevH = prev ? heroList.find((h) => h.code === prev) : undefined;
      if (prevH) step(prevH, 'bw', { x: 0, y: 0 });
      const newH = heroList.find((h) => h.code === code);
      if (newH) step(newH, 'fw', STEP);
      // bruitages officiels par classe (Cursor/intro/*.wav)
      if (prev) playHeroSfx(prev, 'des');
      playHeroSfx(code, 'sel');
    }
    if (scene.on) showSceneStats(entry);
    if (!silent) clickSfx();
  }

  function moveSel(delta: number): void {
    const s = selectable();
    if (!s.length) return;
    let i = s.findIndex((h) => h.code === selectedCode);
    i = i < 0 ? 0 : (i + delta + s.length) % s.length;
    const pick = s[i];
    if (pick) selectHero(pick.code);
  }

  function play(): void {
    if (!selectedCode || !nameInput) return;
    const nm = nameInput.value.trim();
    storeSet('d2hero', selectedCode);
    if (nm) storeSet('d2heroName', nm);
    else storeDel('d2heroName');
    const p = new URLSearchParams({ hero: selectedCode }); // contrat : ?hero=CODE
    if (nm) p.set('name', nm);
    location.href = 'game.html?' + p.toString();
  }

  // ================= scène animée (contrat heroes.json) =================
  // alias de codes : le contrat nomme la Sorcière « SO », le manifest « SOR »
  const ALIAS: Record<string, string> = { SO: 'SOR', SOR: 'SO' };

  function isSheetEntry(v: unknown): v is SheetEntry {
    if (!v || typeof v !== 'object') return false;
    const e = v as Partial<SheetEntry>;
    return (
      typeof e.file === 'string' &&
      Number(e.frames) > 0 &&
      Number(e.w) > 0 &&
      Number(e.h) > 0
    );
  }

  function sheetEntry(json: MenuContract | null, code: string): SheetEntry | null {
    if (!json || typeof json !== 'object') return null;
    const alias = ALIAS[code];
    const raw: unknown = json[code] || (alias ? json[alias] : null);
    return isSheetEntry(raw) ? raw : null;
  }
  function sheetURL(file: string): string {
    // contrat : heroes/<CODE>.png — un file contenant déjà un chemin est respecté
    return file.indexOf('/') >= 0 ? MENU + file : MENU + 'heroes/' + file;
  }

  // rangée à plat comme l'écran original : pieds alignés sur la cour, le feu
  // (braises du fond + animation) occupe l'avant-centre.
  function arcPos(i: number, n: number): { x: number; y: number } {
    const t = n > 1 ? i / (n - 1) : 0.5;
    return { x: 9 + 82 * t, y: 68 };
  }

  function loadState(o: SheetRef | null | undefined): Promise<LoadedSheet | null> {
    if (!o) return Promise.resolve(null);
    return loadImg(sheetURL(o.file)).then((img) => {
      if (!img) return null;
      const frames = o.frames | 0;
      const w = o.w | 0;
      const h = o.h | 0;
      if (frames <= 0 || w <= 0 || h <= 0) return null;
      return {
        img, frames, w, h,
        fps: typeof o.fps === 'number' && o.fps > 0 ? o.fps : 8,
        prog: Array.isArray(o.prog) ? o.prog : null,
      };
    });
  }

  async function tryScene(manifest: GameManifest | null): Promise<void> {
    let json: MenuContract | null = null;
    try {
      const r = await fetch(MENU + 'heroes.json');
      if (r.ok) json = (await r.json()) as MenuContract;
    } catch {
      /* contrat absent -> cartes */
    }
    if (!json) return;
    applyContractFire(json);
    // fond original de l'écran de sélection (charactercreationscreenEXP)
    if (json.background && stage) {
      const bgImg = await loadImg(MENU + json.background);
      if (bgImg) stage.style.backgroundImage = `url(${bgImg.src})`;
    }
    const heroes = (manifest && manifest.heroes) || {};
    const codes = orderedCodes(heroes);
    // préchargement des sheets (échec -> null, le héros sera « bientôt ») ;
    // contrat v2 : nu1 = repos, nu2 = survol, nu3 = sélectionné (DC6 FrontEnd) ;
    // prog = courbe de progression RÉELLE de la marche (racine par frame des
    // DC6 originaux — l'Assassine dart pendant sa roue, le Barbare avance en pas)
    const entries: Array<{
      code: string;
      anim: LoadedSheet | null;
      animHover: LoadedSheet | null;
      animSel: LoadedSheet | null;
      fw: LoadedSheet | null;
      bw: LoadedSheet | null;
    }> = [];
    for (const code of codes) {
      const e = sheetEntry(json, code);
      let anim: LoadedSheet | null = null;
      let animHover: LoadedSheet | null = null;
      let animSel: LoadedSheet | null = null;
      let fw: LoadedSheet | null = null;
      let bw: LoadedSheet | null = null;
      if (e) {
        anim = await loadState(e);
        animHover = (await loadState(e.nu2)) || anim;
        animSel = (await loadState(e.nu3)) || anim;
        fw = await loadState(e.fw);   // marche avant (sélection)
        bw = await loadState(e.bw);   // marche arrière (désélection)
      }
      entries.push({ code, anim, animHover, animSel, fw, bw });
    }
    // aucune sheet utilisable -> on reste sur les cartes
    if (!entries.some((x) => x.anim)) return;
    for (const h of heroList) {
      const idx = entries.findIndex((x) => x.code === h.code);
      const en = idx >= 0 ? entries[idx] : null;
      h.anim = en ? en.anim : null;
      h.animHover = en ? en.animHover : null;
      h.animSel = en ? en.animSel : null;
      h.fw = en ? en.fw : null;
      h.bw = en ? en.bw : null;
      h.walk = null;
      h.off = { x: 0, y: 0 }; // FIX-MENU-ANIM : décalage courant (pas avant/arrière)
      h.pos = arcPos(idx, entries.length);
    }
    scene.on = true;
    scene.hover = null;
    if (screenSelect) screenSelect.classList.add('scene');
    // resynchronise la sélection (buildCards a présélectionné côté cartes)
    const s = selectable();
    if (!s.length) return;
    const cur = s.find((h) => h.code === selectedCode);
    if (cur) showSceneStats(cur);
    else {
      const first = s[0];
      if (first) selectHero(first.code, true);
    }
  }

  // feu du contrat : heroes.json.fire {file,frames,w,h,fps} — prioritaire sur
  // fire-sheet.png ; à défaut les frames existantes, sinon feu CSS (.no-fire)
  function applyContractFire(json: MenuContract): void {
    const e = json && (json.fire || json.FIRE);
    if (!e || !e.file || !(e.frames > 0) || !(e.w > 0) || !(e.h > 0)) return;
    void (async (): Promise<void> => {
      let img = await loadImg(sheetURL(e.file));
      if (!img) img = await loadImg(MENU + e.file);
      if (!img) return;
      const frame = img; // capture typée pour les closures ci-dessous
      if (fireTimer !== null) {
        clearInterval(fireTimer);
        fireTimer = null;
      }
      stage.classList.remove('no-fire');
      const cv = $id<HTMLCanvasElement>('fireCanvas');
      if (!cv) return;
      cv.width = e.w;
      cv.height = e.h;
      cv.style.width = e.w + 'px';
      cv.style.height = e.h + 'px';
      const ctx = cv.getContext('2d');
      if (!ctx) return;
      let ff = 0;
      const step = (): void => {
        ff = (ff + 1) % e.frames;
        ctx.clearRect(0, 0, cv.width, cv.height);
        // le DC6 du feu n'a pas d'alpha : blend additif (le noir disparaît),
        // comme le rendu D2 original des flammes sur fond sombre.
        ctx.globalCompositeOperation = 'lighter';
        ctx.drawImage(frame, ff * e.w, 0, e.w, e.h, 0, 0, e.w, e.h);
        ctx.globalCompositeOperation = 'source-over';
      };
      step();
      fireTimer = setInterval(step, 1000 / (typeof e.fps === 'number' && e.fps > 0 ? e.fps : 15));
    })();
  }

  // ---- rendu de la scène : UN SEUL rAF, dessine seulement si visible ----
  function stageXY(p: { x: number; y: number }): { x: number; y: number } {
    return { x: (p.x / 100) * 800, y: (p.y / 100) * 600 };
  }

  function drawScene(t: number): void {
    if (!sctx) return;
    sctx.clearRect(0, 0, 800, 600);
    sctx.imageSmoothingEnabled = false;
    // du fond au premier plan (pieds les plus bas d'abord, position glissante)
    const list = heroList.slice().sort((a, b) => (a.pos.y + a.off.y) - (b.pos.y + b.off.y));
    for (const h of list) {
      if (!h.anim) {
        // socle discret + code, « noté » sans casser la scène
        const p0 = stageXY({ x: h.pos.x + h.off.x, y: h.pos.y + h.off.y });
        drawMissingHero(h, p0.x, p0.y);
        continue;
      }
      const hovered = scene.hover === h.code;
      const sel = selectedCode === h.code;
      // FIX-MENU-ANIM : séquences ancrées — marche et anims d'état jouent
      // depuis leur frame 0 (ancrage au moment du changement), la sélection
      // joue UNE fois puis TIENT sa dernière image (la pose finale).
      // L'indexation globale (t/1000*fps)%frames faisait démarrer chaque
      // séquence au milieu/à la fin, la rejouer (2 passes perçues), et
      // rogner la fin avant la pose du héros sélectionné.
      // état du sprite FrontEnd : marche > sélectionné > survol > repos
      const a = h.walk
        ? h.walk.kind === 'fw'
          ? h.fw
          : h.bw
        : (sel && h.animSel) || (hovered && h.animHover) || h.anim;
      if (!a) continue; // inatteignable : walk n'existe que si la sheet existe
      if (h.st !== a) {
        h.st = a;
        h.stStart = t;
      }
      let fi: number;
      if (h.walk) {
        // FIX-MENU-ANIM : la POSITION suit le rythme de l'animation
        // originale — courbe de racine par frame (prog) des DC6 du jeu :
        // chaque classe traverse la distance à sa façon (l'Assassine dart
        // pendant sa roue, le Barbare avance en pas réguliers) au lieu
        // d'un glissement linéaire uniforme.
        const el = t - h.walk.start;
        fi = Math.min(Math.floor((el / 1000) * a.fps), a.frames - 1);
        let q = Math.min(el / h.walk.dur, 1);
        const pr = a.prog;
        if (pr && pr.length === a.frames) {
          const frac = Math.min((el / 1000) * a.fps - fi, 1);
          const p0 = pr[fi] ?? 0;
          const p1 = pr[Math.min(fi + 1, a.frames - 1)] ?? 1;
          q = p0 + (p1 - p0) * frac;
        }
        q = Math.min(Math.max(q, 0), 1);
        h.off.x = h.walk.from.x + (h.walk.to.x - h.walk.from.x) * q;
        h.off.y = h.walk.from.y + (h.walk.to.y - h.walk.from.y) * q;
        if (q >= 1) {
          const to = h.walk.to;
          h.walk = null;
          h.off = { ...to };
        }
      } else if (sel && a === h.animSel) {
        // sélection : one-shot depuis 0, puis pose tenue
        fi = Math.min(Math.floor(((t - h.stStart) / 1000) * a.fps), a.frames - 1);
      } else {
        // survol / repos : boucle démarrant à la frame 0 de leur entrée
        fi = Math.floor(((t - h.stStart) / 1000) * a.fps) % a.frames;
      }
      const { x, y } = stageXY({ x: h.pos.x + h.off.x, y: h.pos.y + h.off.y });
      const dw = a.w * SCALE;
      const dh = a.h * SCALE;
      if (hovered || sel) drawHalo(x, y, sel);
      sctx.drawImage(a.img, fi * a.w, 0, a.w, a.h, x - dw / 2, y - dh, dw, dh);
      if (hovered) drawLabel(x, Math.max(22, y - dh - 12), heroName(h.def, h.code));
    }
  }

  function drawHalo(x: number, y: number, strong: boolean): void {
    if (!sctx) return;
    sctx.save();
    sctx.translate(x, y - 8);
    sctx.scale(1, 0.5);
    const g = sctx.createRadialGradient(0, 0, 4, 0, 0, 70);
    g.addColorStop(0, strong ? 'rgba(255,216,115,.5)' : 'rgba(255,200,90,.3)');
    g.addColorStop(1, 'rgba(255,200,90,0)');
    sctx.fillStyle = g;
    sctx.beginPath();
    sctx.arc(0, 0, 70, 0, Math.PI * 2);
    sctx.fill();
    sctx.restore();
  }

  function drawLabel(x: number, y: number, text: string): void {
    if (!sctx) return;
    sctx.font = '17px Georgia, serif';
    sctx.textAlign = 'center';
    sctx.shadowColor = 'rgba(0,0,0,.9)';
    sctx.shadowBlur = 4;
    sctx.fillStyle = '#ffd873';
    sctx.fillText(text, x, y);
    sctx.shadowBlur = 0;
  }

  // sheet manquante : socle discret + code, « noté » sans casser la scène
  function drawMissingHero(h: HeroEntry, x: number, y: number): void {
    if (!sctx) return;
    sctx.save();
    sctx.globalAlpha = 0.5;
    sctx.strokeStyle = '#6b5a2e';
    sctx.lineWidth = 1;
    sctx.beginPath();
    sctx.ellipse(x, y, 26, 8, 0, 0, Math.PI * 2);
    sctx.stroke();
    sctx.font = '13px Georgia, serif';
    sctx.textAlign = 'center';
    sctx.fillStyle = '#8a7a55';
    sctx.fillText(h.code + ' — bientôt', x, y - 12);
    sctx.restore();
  }

  function showSceneStats(entry: HeroEntry): void {
    if (!sceneStats) return;
    const s = (entry.def && entry.def.stats) || {};
    const row = (label: string, v: unknown): string =>
      '<div class="row"><span>' + label + '</span><b>' + (v != null ? String(v) : '?') + '</b></div>';
    sceneStats.innerHTML =
      '<h3></h3>' +
      row('Vie', s.life) + row('Mana', s.mana) +
      row('Force', s.strength) + row('Dextérité', s.dexterity) +
      '<div class="skill">Compétence : ' + (SKILLS[entry.code] || '—') + '</div>';
    const h3 = sceneStats.querySelector('h3');
    if (h3) h3.textContent = heroName(entry.def, entry.code);
    sceneStats.classList.add('on');
  }

  requestAnimationFrame(function loop(t: number): void {
    requestAnimationFrame(loop);
    if (!scene.on || onTitle) return;
    drawScene(t);
  });

  // ---- souris sur la scène : survol = halo + nom, clic = sélection ----
  function hitHero(mx: number, my: number): string | null {
    for (const h of heroList) {
      if (!h.anim || !h.playable) continue;
      const a = h.anim;
      // position glissante : la zone cliquable suit le héros en marche
      const p = stageXY({ x: h.pos.x + h.off.x, y: h.pos.y + h.off.y });
      const dw = a.w * SCALE;
      const dh = a.h * SCALE;
      if (mx >= p.x - dw * 0.35 && mx <= p.x + dw * 0.35 && my >= p.y - dh && my <= p.y) {
        return h.code;
      }
    }
    return null;
  }
  if (sceneCanvas) {
    sceneCanvas.addEventListener('mousemove', (e) => {
      if (!scene.on) return;
      const r = sceneCanvas.getBoundingClientRect();
      const mx = ((e.clientX - r.left) / r.width) * 800;
      const my = ((e.clientY - r.top) / r.height) * 600;
      const hit = hitHero(mx, my);
      if (hit !== scene.hover) {
        scene.hover = hit;
        sceneCanvas.style.cursor = hit ? 'pointer' : 'default';
      }
    });
    sceneCanvas.addEventListener('mouseleave', () => { scene.hover = null; });
    sceneCanvas.addEventListener('click', () => { if (scene.hover) selectHero(scene.hover); });
  }

  // ---- chargement manifest (données seules) puis contrat d'animation ----
  fetch('assets/manifest.json')
    .then((r) => {
      if (!r.ok) throw new Error('HTTP ' + r.status);
      return r.json() as Promise<GameManifest>;
    })
    .then((manifest: GameManifest) => {
      buildCards(manifest);   // cartes toujours prêtes (repli immédiat)
      void tryScene(manifest); // scène animée seulement si le contrat existe
    })
    .catch((e: unknown) => {
      if (!manifestErr) return;
      manifestErr.hidden = false;
      manifestErr.textContent = 'manifest.json indisponible (' + errText(e) + ') — sélection de héros impossible.';
    });

  // ---- boutons ----
  document.querySelectorAll('#buttons .dbtn').forEach((b) => {
    b.addEventListener('click', () => {
      clickSfx();
      const action = (b as HTMLElement).dataset.action;
      if (action === 'solo') showSelect();
      else if (action === 'continue') location.href = 'game.html?continue=1';
      else if (action === 'slots') showSlots();
      else if (action === 'github') window.open('https://github.com/shpaw415/diablo2-wasm', '_blank');
      else if (action === 'quit') window.close();
    });
  });
  if (playBtn) playBtn.addEventListener('click', play);
  const backBtn = $id<HTMLButtonElement>('back');
  if (backBtn) backBtn.addEventListener('click', showTitle);

  // ================= écran « Parties » (slots de sauvegarde) =================
  const slotsList = $id<HTMLDivElement>('slotsList');
  const slotsEmpty = $id<HTMLDivElement>('slotsEmpty');
  const slotsMsg = $id<HTMLDivElement>('slotsMsg');
  const slotFile = $id<HTMLInputElement>('slotFile');

  function fmtPlaytime(ms: number | null | undefined): string {
    const s = Math.floor((ms || 0) / 1000);
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    return h ? h + ' h ' + m + ' min' : m ? m + ' min' : s + ' s';
  }
  function slotsMessage(t: string): void {
    if (!slotsMsg) return;
    slotsMsg.hidden = !t;
    slotsMsg.textContent = t || '';
  }
  function renderSlots(): void {
    if (!slotsList || !slotsEmpty) return;
    slotsMessage('');
    const api = window.D2Slots;
    const slots = api ? api.list() : [];
    slotsList.textContent = '';
    slotsEmpty.hidden = slots.length > 0;
    for (const slot of slots) {
      const row = document.createElement('div');
      row.className = 'slotrow';
      const info = document.createElement('div');
      info.className = 'slotinfo';
      const nameEl = document.createElement('div');
      nameEl.className = 'slotname';
      nameEl.textContent = slot.name;
      const meta = document.createElement('div');
      meta.className = 'slotmeta';
      meta.innerHTML = '<b></b> · niv. <b></b> · <b></b> · <b></b> · <b></b>';
      const bs = meta.querySelectorAll('b');
      const d = new Date(slot.ts);
      if (bs[0]) bs[0].textContent = slot.hero_label || slot.hero_code;
      if (bs[1]) bs[1].textContent = slot.level != null ? String(slot.level) : '?';
      if (bs[2]) bs[2].textContent = slot.zone || slot.level_key || 'town';
      if (bs[3]) bs[3].textContent = fmtPlaytime(slot.playtime_ms);
      if (bs[4]) bs[4].textContent = d.toLocaleDateString() + ' ' + d.toLocaleTimeString().slice(0, 5);
      info.append(nameEl, meta);
      const btns = document.createElement('div');
      btns.className = 'slotbtns';
      const mk = (label: string, cls: string, fn: () => void, title?: string): HTMLButtonElement => {
        const b = document.createElement('button');
        b.className = 'sbtn' + (cls ? ' ' + cls : '');
        b.textContent = label;
        if (title) b.title = title;
        b.addEventListener('click', (ev) => {
          ev.stopPropagation();
          clickSfx();
          fn();
        });
        return b;
      };
      btns.append(
        mk('Jouer', '', () => { location.href = 'game.html?slot=' + encodeURIComponent(slot.id); }, 'Reprendre ce héros'),
        mk('Renommer', '', () => beginRename(slot, nameEl), 'Modifier le nom du slot'),
        mk('Exporter', '', () => {
          if (!api) return;
          try {
            api.exportSlot(slot.id);
            slotsMessage('Exporté : ' + slot.name + ' (.json téléchargé).');
          } catch (e) {
            slotsMessage('Export impossible : ' + errText(e));
          }
        }, 'Télécharger la sauvegarde (.json)'),
        mk('Supprimer', 'danger', () => {
          if (!api) return;
          api.remove(slot.id);
          renderSlots();
        }, 'Effacer ce slot'),
      );
      row.append(info, btns);
      slotsList.appendChild(row);
    }
  }
  // Renommage inline : le nom devient un input, Entrée/flou valident.
  function beginRename(slot: SlotInfo, nameEl: HTMLDivElement): void {
    const api = window.D2Slots;
    if (!api) return;
    const input = document.createElement('input');
    input.type = 'text';
    input.maxLength = 24;
    input.value = slot.name;
    nameEl.textContent = '';
    nameEl.appendChild(input);
    input.focus();
    input.select();
    const commit = (): void => {
      api.rename(slot.id, input.value);
      renderSlots();
    };
    input.addEventListener('keydown', (e) => {
      if (e.key === 'Enter') {
        e.preventDefault();
        commit();
      }
      if (e.key === 'Escape') {
        e.stopPropagation();
        renderSlots();
      }
      e.stopPropagation(); // le clavier global (flèches…) ne vole pas la saisie
    });
    input.addEventListener('blur', commit);
    input.addEventListener('click', (e) => e.stopPropagation());
  }
  const slotsBack = $id<HTMLButtonElement>('slotsBack');
  if (slotsBack) slotsBack.addEventListener('click', showTitle);
  const slotImport = $id<HTMLButtonElement>('slotImport');
  if (slotImport && slotFile) slotImport.addEventListener('click', () => slotFile.click());
  if (slotFile) {
    slotFile.addEventListener('change', () => {
      const files = slotFile.files;
      const file = files && files.length > 0 ? files[0] : null;
      slotFile.value = '';
      if (!file) return;
      const r = new FileReader();
      r.onload = (): void => {
        const api = window.D2Slots;
        if (!api) return;
        try {
          const slot = api.importSlot(String(r.result));
          renderSlots();
          slotsMessage('Importé : ' + slot.name + ' (' + (slot.hero_label || slot.hero_code) + ').');
        } catch (e) {
          slotsMessage('Import refusé : ' + errText(e));
        }
      };
      r.readAsText(file);
    });
  }

  // ---- Continuer grisé sans sauvegarde (slots ou clé historique) ----
  const btnContinue = $id<HTMLButtonElement>('btnContinue');
  const saveApi = window.D2Slots;
  if (btnContinue && !storeGet('d2wasm.save.v1') && !(saveApi && saveApi.mostRecent())) {
    btnContinue.disabled = true;
  }

  // ---- clavier ----
  document.addEventListener('keydown', (e) => {
    if (nameInput && e.target === nameInput) {
      if (e.key === 'Enter') {
        e.preventDefault();
        play();
      } else if (e.key === 'Escape') {
        nameInput.blur();
      }
      return;
    }
    if (onSlots) {
      if (e.key === 'Escape') {
        showTitle();
        clickSfx();
      }
      return; // saisie/clavier des slots gérés localement (inputs)
    }
    if (onTitle) {
      if (e.key === 'Enter') {
        showSelect();
        clickSfx();
      }
      return;
    }
    switch (e.key) {
      case 'ArrowLeft': moveSel(-1); break;
      case 'ArrowRight': moveSel(1); break;
      // scène : arc de héros, ↑/↓ = ±1 ; cartes : grille, ↑/↓ = ±4
      case 'ArrowUp': moveSel(scene.on ? -1 : -4); break;
      case 'ArrowDown': moveSel(scene.on ? 1 : 4); break;
      case 'Enter': play(); break;
      case 'Escape': showTitle(); break;
      default: return;
    }
    e.preventDefault();
  });

  // bouton bascule musique (créé dynamiquement, haut-droite du stage)
  const musicBtn = document.createElement('button');
  musicBtn.id = 'musicBtn';
  musicBtn.textContent = musicMuted ? '🔇' : '🔊'; // EXP-A1 : reflète l'état persistant
  musicBtn.title = 'Musique du menu';
  musicBtn.addEventListener('click', (ev) => {
    ev.stopPropagation();
    toggleMusic();
  });
  stage.appendChild(musicBtn);
  startMusic();
}
