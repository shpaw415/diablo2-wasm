// HUD parité D2 — overlay DOM au-dessus du canvas — migration TS 25a de
// demo/hud.js (contrat window.D2HUD INCHANGÉ : init/update/events/frame).
//
// Points d'appel game.js gardés par `if (window.D2HUD)` :
//   D2HUD.init(manifest, game)   — construit le DOM, écoute souris/clavier
//   D2HUD.update(stats)          — EXP-H2 barre d'endurance (par frame)
//   D2HUD.events(events)         — EXP-H7 dégâts flottants (par tick)
//   D2HUD.frame(canvas, game)    — EXP-H6 survol monstre + EXP-H8 labels Alt
//
// Aucun effet de bord au scope (tout vit dans init()) : le contrat peut être
// remplacé par la voie TS pendant l'évaluation du bundle — game.js appellera
// l'INIT TS, jamais celle du Legacy (déjà évalué mais orphelin).
import { registerModule,
  
  D2Game, D2HUDApi, D2Window, GameEvent, HeroSkill, HeroStats, Manifest, PlayerInput,
} from './types';
import { parseOr, win } from './types';

interface Hands { L: number; R: number }

const D2HUD: D2HUDApi & {
  game: D2Game | null;
  canvas: HTMLCanvasElement | null;
  skills: HeroSkill[];
  hands: Hands;
  pendingHand: 'L' | 'R';
  altDown: boolean;
  mouse: { x: number; y: number; inside: boolean };
  els: Record<string, HTMLElement>;
  labelPool: Map<string, HTMLDivElement>;
} = {
  ready: false,
  game: null,
  canvas: null,
  skills: [],
  hands: { L: -1, R: -1 },
  pendingHand: 'R',
  altDown: false,
  mouse: { x: -1, y: -1, inside: false },
  els: {},
  labelPool: new Map(),
  init(_manifest: Manifest, game?: D2Game) { init(game); },
  update(stats: HeroStats) { updateStamina(stats); },
  events(events: GameEvent[]) { handleEvents(events); },
  frame(canvas: HTMLCanvasElement, game: D2Game) { frame(canvas, game); },
};

// --- helpers -------------------------------------------------------------
function el(tag: string, id?: string, parent?: HTMLElement): HTMLElement {
  const n = document.createElement(tag);
  if (id) n.id = id;
  (parent ?? D2HUD.els.root!).appendChild(n);
  return n;
}

/** Fire Bolt → FB ; Attaque → ⚔ (pur, testable). */
export function shortSkillName(name: string | undefined): string {
  if (!name || name === 'Attaque') return '⚔';
  return name.split(/\s+/).map((w) => w[0]).join('').toUpperCase().slice(0, 3);
}

function skillOf(hand: 'L' | 'R'): HeroSkill {
  const idx = D2HUD.hands[hand];
  if (idx < 0) return { index: -1, code: 'Attaque', name: 'Attaque', level: 0, missile: null };
  return D2HUD.skills[idx] ?? { index: -1, code: 'Attaque', name: 'Attaque', level: 0, missile: null };
}

function refreshSkillButtons(): void {
  for (const hand of ['L', 'R'] as const) {
    const btn = D2HUD.els['skbtn' + hand];
    if (!btn) continue;
    const s = skillOf(hand);
    btn.textContent = shortSkillName(s.name);
    btn.title = `${hand === 'L' ? 'Gauche' : 'Droite'} : ${s.name}` +
      (s.level ? ` (niv. ${s.level})` : '') + (s.missile ? ' · projectile' : '') +
      ' — cliquer pour choisir';
    btn.classList.toggle('selected', s.missile != null);
  }
}

// --- init ----------------------------------------------------------------
function init(game?: D2Game): void {
  try {
    D2HUD.game = game ?? win()?.__d2?.game ?? null;
    D2HUD.canvas = document.getElementById('game') as HTMLCanvasElement | null;
    if (!D2HUD.canvas || !D2HUD.game) return; // fallback JS : overlay inactif

    const stage = D2HUD.canvas.parentElement;
    const root = document.createElement('div');
    root.id = 'hudx';
    stage?.appendChild(root);
    D2HUD.els.root = root;

    // EXP-H2 : barre d'endurance
    const stw = el('div', 'staminawrap');
    stw.title = 'Endurance';
    D2HUD.els.staminaFill = el('div', 'staminafill', stw);
    D2HUD.els.staminaWrap = stw;

    // EXP-H6 : survol monstre
    const mh = el('div', 'monhover');
    D2HUD.els.monName = el('div', 'monname', mh);
    const hpw = el('div', 'monhpwrap', mh);
    D2HUD.els.monHpFill = el('div', 'monhpfill', hpw);
    D2HUD.els.monHover = mh;

    // EXP-H8 : étiquettes objets (Alt) + EXP-H7 : dégâts flottants
    D2HUD.els.labels = el('div', 'itemlabels');
    D2HUD.els.floats = el('div', 'dmgfloats');

    // EXP-H4 : boutons L/R + popup compétences
    for (const hand of ['L', 'R'] as const) {
      const b = el('button', 'skbtn-' + hand) as HTMLButtonElement;
      b.className = 'skbtn';
      b.type = 'button';
      b.addEventListener('click', (e) => {
        e.stopPropagation();
        openSkillPopup(hand);
      });
      D2HUD.els['skbtn' + hand] = b;
    }
    D2HUD.els.skPopup = el('div', 'skpopup');

    // EXP-H3 : minipanel 8 boutons (Inventaire/Son câblés ; le reste
    // décoratif tant que les écrans EXP-E* correspondants n'existent pas).
    const mp = el('div', 'minipanel');
    const defs: Array<[string, string, (() => void) | null]> = [
      ['Menu', 'Menu jeu / pause — à venir (EXP-E4)', null],
      ['Inv', 'Inventaire (I)', () => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'i' }))],
      ['Stats', 'Panneau de personnage (C)', () => window.dispatchEvent(new KeyboardEvent('keydown', { key: 'c' }))],
      ['Skills', 'Arbre de compétences — à venir (EXP-E8)', () => openSkillPopup('R')],
      ['Quêtes', 'Journal de quêtes — à venir (EXP-E7)', null],
      ['Carte', 'Automap (Tab)', () => { const w = window as D2Window; if (w.D2Automap) w.D2Automap.toggle(); }],
      ['Aide', 'Aide (H) — à venir (EXP-E5)', null],
      ['Son', 'Activer/couper le son', () => {
        const b = document.getElementById('btn-sound');
        if (b) (b as HTMLButtonElement).click();
      }],
    ];
    const mpButtons: Record<string, HTMLElement> = {};
    for (const [label, title, fn] of defs) {
      const b = document.createElement('button');
      b.type = 'button';
      b.textContent = label;
      b.title = title;
      if (!fn) b.classList.add('soon');
      else b.addEventListener('click', (e) => { e.stopPropagation(); fn(); });
      mp.appendChild(b);
      mpButtons[label] = b;
    }
    D2HUD.els.mpButtons = mpButtons as unknown as HTMLElement;

    wireInput();
    loadSkills();
    refreshSkillButtons();
    D2HUD.ready = true;
  } catch (err) {
    console.info('D2HUD init ignoré :', err instanceof Error ? err.message : String(err));
  }
}

function loadSkills(): void {
  try {
    D2HUD.skills = parseOr<HeroSkill[]>(D2HUD.game ? D2HUD.game.heroSkills() : '[]', []);
  } catch {
    D2HUD.skills = [];
  }
  // Défaut D2 : main gauche = attaque de base ; main droite = le premier
  // skill à projectile (sinon le premier skill, sinon attaque).
  D2HUD.hands.L = -1;
  const s = D2HUD.skills.find((k) => k.missile) ?? D2HUD.skills[0];
  D2HUD.hands.R = s ? s.index : -1;
}

// --- EXP-H4 : popup de sélection de compétence ----------------------------
function openSkillPopup(hand: 'L' | 'R'): void {
  D2HUD.pendingHand = hand;
  const pop = D2HUD.els.skPopup;
  if (!pop) return;
  pop.textContent = '';
  const h = document.createElement('h3');
  h.textContent = `Main ${hand === 'L' ? 'gauche' : 'droite'} — choisir`;
  pop.appendChild(h);
  const rows: HeroSkill[] = [
    { index: -1, code: 'Attaque', name: 'Attaque', level: 0, missile: null },
    ...D2HUD.skills,
  ];
  for (const s of rows) {
    const row = document.createElement('div');
    row.className = 'skrow';
    const nm = document.createElement('span');
    nm.textContent = s.name;
    const meta = document.createElement('span');
    meta.className = s.missile ? 'missile' : 'lvl';
    meta.textContent = s.missile ? 'projectile' : (s.level ? `niv. ${s.level}` : 'mêlée');
    row.appendChild(nm);
    row.appendChild(meta);
    row.addEventListener('click', () => assignSkill(hand, s.index));
    pop.appendChild(row);
  }
  pop.classList.add('open');
}

function assignSkill(hand: 'L' | 'R', index: number): void {
  D2HUD.hands[hand] = index;
  if (index >= 0) {
    try {
      if (D2HUD.game) D2HUD.game.selectSkill(index);
    } catch { /* hors bornes : ignoré */ }
  }
  D2HUD.els.skPopup?.classList.remove('open');
  refreshSkillButtons();
}

// --- entrées : clic gauche/droit selon la main, Alt, souris ---------------
function wireInput(): void {
  const cv = D2HUD.canvas!;
  cv.addEventListener('mousemove', (e) => {
    const r = cv.getBoundingClientRect();
    D2HUD.mouse.x = (e.clientX - r.left) * (cv.width / r.width);
    D2HUD.mouse.y = (e.clientY - r.top) * (cv.height / r.height);
    D2HUD.mouse.inside = true;
  });
  cv.addEventListener('mouseleave', () => {
    D2HUD.mouse.inside = false;
    if (D2HUD.els.monHover) D2HUD.els.monHover.style.display = 'none';
  });
  // Alt = étiquettes objets (EXP-H8) ; pas de menu navigateur sur le canvas.
  window.addEventListener('keydown', (e) => {
    if (e.key === 'Alt') {
      D2HUD.altDown = true;
      e.preventDefault();
    }
    if (e.key === 'Escape' && D2HUD.els.skPopup?.classList.contains('open')) {
      D2HUD.els.skPopup.classList.remove('open');
    }
  });
  window.addEventListener('keyup', (e) => { if (e.key === 'Alt') D2HUD.altDown = false; });
  window.addEventListener('blur', () => { D2HUD.altDown = false; });

  // Clic DROIT = main droite (D2). Sur le canvas : menu contextuel annulé,
  // routage via window.__d2.queue (exposé par game.js).
  cv.addEventListener('contextmenu', (e) => {
    e.preventDefault();
    castWithHand('R', e);
  });
  // Clic GAUCHE = main gauche : si le skill de main gauche porte un
  // projectile, il prime sur le routage move/attack de game.js (capture
  // sur window, AVANT les listeners canvas de game.js).
  window.addEventListener('click', (e) => {
    if (e.target !== cv || e.button !== 0) return;
    const s = skillOf('L');
    if (s.missile) {
      e.stopPropagation();
      castWithHand('L', e);
    }
  }, true);
}

/** Routage d'un clic-compétence : sélectionne le skill de la main côté
 * moteur puis pousse l'input `rangedAttack` (projectile) — ou retombe sur
 * le routage move/attack historique si la main n'a pas de projectile. */
function castWithHand(hand: 'L' | 'R', e: MouseEvent): void {
  if (!D2HUD.ready || !D2HUD.game || !D2HUD.canvas) return;
  const r = D2HUD.canvas.getBoundingClientRect();
  const px = (e.clientX - r.left) * (D2HUD.canvas.width / r.width);
  const py = (e.clientY - r.top) * (D2HUD.canvas.height / r.height);
  const s = skillOf(hand);
  let wx: number, wy: number;
  try {
    const world = D2HUD.game.screenToWorld(px, py);
    wx = world[0]!;
    wy = world[1]!;
  } catch {
    return;
  }
  const tx = Math.round(wx), ty = Math.round(wy);
  const queue = win()?.__d2?.queue;
  if (!queue) return;
  if (s.missile) {
    try {
      if (D2HUD.game) D2HUD.game.selectSkill(s.index);
    } catch { /* ignoré */ }
    refreshSkillButtons();
    queue.push({ type: 'rangedAttack', x: tx, y: ty } satisfies PlayerInput);
  } else {
    // mêlée / attaque de base : monstre sous le clic ⇒ attaque, sinon move
    let near = -1;
    try {
      if (D2HUD.game) near = D2HUD.game.monsterNear(wx, wy, 2.5);
    } catch { /* ignoré */ }
    queue.push(near >= 0
      ? { type: 'attack', x: tx, y: ty }
      : { type: 'move', x: tx, y: ty });
  }
}

// --- EXP-H2 : endurance ----------------------------------------------------
function updateStamina(stats: HeroStats): void {
  if (!D2HUD.ready || !stats || stats.max_stamina === undefined) return;
  const wrap = D2HUD.els.staminaWrap;
  const fill = D2HUD.els.staminaFill;
  if (!wrap || !fill) return;
  const max = Math.max(1, stats.max_stamina);
  const f = Math.max(0, Math.min(1, (stats.stamina || 0) / max));
  fill.style.width = `${(f * 100).toFixed(1)}%`;
  wrap.classList.toggle('low', f < 0.5 && f >= 0.2);
  wrap.classList.toggle('empty', f < 0.2);
  wrap.title = `Endurance : ${stats.stamina}/${stats.max_stamina}`;
}

// --- EXP-H7 : dégâts flottants ---------------------------------------------
function handleEvents(events: GameEvent[]): void {
  if (!D2HUD.ready || !Array.isArray(events)) return;
  for (const ev of events) {
    if (ev.LevelSwitch) {
      // changement de zone : tout ce qui est projeté au sol est périmé
      const floats = D2HUD.els.floats;
      if (floats) floats.textContent = '';
      clearLabels();
      hideMonHover();
      continue;
    }
    if (ev.HeroAttacked) {
      spawnFloatAtMonster(ev.HeroAttacked.target, `−${ev.HeroAttacked.dmg}`, '#ffb347');
    } else if (ev.MissileHit) {
      spawnFloatAtMonster(ev.MissileHit.target, `−${ev.MissileHit.dmg}`, '#ffb347');
    } else if (ev.HeroMissed) {
      spawnFloatAtMonster(ev.HeroMissed.target, 'raté', '#b8ad98');
    } else if (ev.MissileMissed) {
      spawnFloatAtMonster(ev.MissileMissed.target, 'raté', '#b8ad98');
    } else if (ev.HeroHit) {
      spawnFloatAtHero(`−${ev.HeroHit.dmg}`, '#ff5545');
    }
  }
}

function monsterScreen(index: number): [number, number] | null {
  try {
    const info = parseOr<import('./types').MonsterInfo | null>(
      D2HUD.game ? D2HUD.game.monsterInfo(index) : 'null', null);
    return info ? [info.sx, info.sy] : null;
  } catch {
    return null;
  }
}

function spawnFloatAtMonster(index: number, text: string, color: string): void {
  const pos = monsterScreen(index);
  if (pos) spawnFloat(pos[0], pos[1] - 34, text, color);
}

function spawnFloatAtHero(text: string, color: string): void {
  try {
    const screen = D2HUD.game ? D2HUD.game.heroScreen() : null;
    if (!screen) return;
    spawnFloat(screen[0]!, screen[1]! - 44, text, color);
  } catch { /* hors carte : ignoré */ }
}

function spawnFloat(x: number, y: number, text: string, color: string): void {
  const floats = D2HUD.els.floats;
  if (!floats) return;
  const n = document.createElement('span');
  n.className = 'dmgfloat';
  n.textContent = text;
  n.style.left = `${Math.round(x + (Math.random() * 16 - 8))}px`;
  n.style.top = `${Math.round(y)}px`;
  n.style.color = color;
  floats.appendChild(n);
  setTimeout(() => n.remove(), 950);
}

// --- EXP-H6 + EXP-H8 : poll par frame --------------------------------------
function frame(_canvas: HTMLCanvasElement, game: D2Game): void {
  if (!D2HUD.ready) return;
  try { frameMonsterHover(game); } catch { /* jamais bloquant */ }
  try { frameItemLabels(game); } catch { /* jamais bloquant */ }
  try { refreshSonLabel(); } catch { /* jamais bloquant */ }
}

function hideMonHover(): void {
  if (D2HUD.els.monHover) D2HUD.els.monHover.style.display = 'none';
}

// 13b : label de rang élite — badge « Champion » bleu / « Unique » doré,
// « Super-unique », « Boss ». Aucun effet si kind normal/inconnu.
const MON_RANKS: Readonly<Record<string, string>> = {
  champion: 'Champion', unique: 'Unique', super_unique: 'Super-unique', boss: 'Boss',
};

function setMonRank(kind: string): void {
  const name = D2HUD.els.monName;
  if (!name) return;
  const old = name.querySelector('.monrank');
  if (old) old.remove();
  const label = MON_RANKS[kind];
  if (!label) return;
  const tag = document.createElement('span');
  tag.className = 'monrank';
  tag.textContent = label;
  name.appendChild(tag);
}

function frameMonsterHover(game: D2Game): void {
  if (!D2HUD.mouse.inside) { hideMonHover(); return; }
  const world = game.screenToWorld(D2HUD.mouse.x, D2HUD.mouse.y);
  const idx = game.monsterNear(world[0]!, world[1]!, 1.4);
  if (idx < 0) { hideMonHover(); return; }
  const info = parseOr<import('./types').MonsterInfo | null>(game.monsterInfo(idx) || 'null', null);
  if (!info || !info.alive) { hideMonHover(); return; }
  const mh = D2HUD.els.monHover;
  if (!mh) return;
  mh.className = 'k-' + info.kind; // garde #monhover + teinte du nom
  mh.id = 'monhover';
  const monName = D2HUD.els.monName;
  if (monName) monName.textContent = info.name;
  setMonRank(info.kind); // 13b : label de rang (champion bleu / unique doré)
  const frac = info.maxHp > 0 ? Math.max(0, Math.min(1, info.hp / info.maxHp)) : 0;
  const fill = D2HUD.els.monHpFill;
  if (fill) fill.style.width = `${(frac * 100).toFixed(0)}%`;
  mh.style.left = `${Math.round(info.sx)}px`;
  mh.style.top = `${Math.round(info.sy - 30)}px`;
  mh.style.display = 'block';
}

function clearLabels(): void {
  const labels = D2HUD.els.labels;
  if (labels) labels.textContent = '';
  D2HUD.labelPool.clear();
}

function frameItemLabels(game: D2Game): void {
  if (!D2HUD.altDown) {
    if (D2HUD.labelPool.size) clearLabels();
    return;
  }
  const labels = D2HUD.els.labels;
  if (!labels) return;
  const items = parseOr<import('./types').GroundItem[]>(game.groundItems() || '[]', []);
  const seen = new Set<string>();
  for (const it of items) {
    const key = `${it.index}:${it.code}:${it.amount}`;
    seen.add(key);
    let div = D2HUD.labelPool.get(key);
    if (!div) {
      div = document.createElement('div');
      div.className = 'ilabel k-' + it.kind;
      div.textContent = it.kind === 'Gold' ? `${it.name} (${it.amount})` : it.name;
      div.title = 'Cliquer pour ramasser (E au contact)';
      div.addEventListener('click', (e) => {
        e.stopPropagation();
        const q = win()?.__d2?.queue;
        if (q) q.push({ type: 'pickup' });
      });
      labels.appendChild(div);
      D2HUD.labelPool.set(key, div);
    }
    div.style.left = `${Math.round(it.sx)}px`;
    div.style.top = `${Math.round(it.sy - 14)}px`;
    div.style.display = 'block';
  }
  for (const [key, div] of D2HUD.labelPool) {
    if (!seen.has(key)) {
      div.remove();
      D2HUD.labelPool.delete(key);
    }
  }
}

function refreshSonLabel(): void {
  const b = document.getElementById('btn-sound');
  const son = (D2HUD.els.mpButtons as unknown as Record<string, HTMLElement>)?.Son;
  if (b && son) {
    const on = /oui/i.test(b.textContent ?? '');
    son.textContent = on ? 'Son' : 'Muet';
  }
}

export function installHUD(win: D2Window): void {
  registerModule('hud', 'ts');
  win.D2HUD = D2HUD;
}

if (typeof window !== 'undefined') installHUD(window as unknown as D2Window);
