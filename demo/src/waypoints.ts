// Diablo 2 WASM — panneau des waypoints (W05/W19) — migration TS 25a de
// demo/waypoints.js (contrat window.D2Waypoints INCHANGÉ).
//
// Le moteur Rust porte TOUTE la logique (waypoints.rs + waypoint_travel.rs) ;
// ici : panneau DOM (Tab / bouton), liste des découverts (clic = voyage),
// section grisée des non-découverts, sonde E2E `e2eTour()`.
// Intégration (game.js) : D2Waypoints.init({ getGame, queue, note, flash,
// manifest, sfx, onTraveled }) + D2Waypoints.refresh(game.waypoints()).
import { registerModule,
   parseOr, type D2WaypointsApi, type D2WaypointsOpts, type D2Window, type WaypointEntry, type WaypointState } from './types';

const api: D2WaypointsApi & {
  opts: D2WaypointsOpts | null;
  state: WaypointState | null;
  panel: HTMLDivElement | null;
} = {
  ready: false,
  open: false,
  opts: null,
  state: null,
  panel: null,
  init(o) { return initWaypoints(o); },
  refresh(jsonText) { refresh(jsonText); },
  nameOf(slot) { return nameOf(slot); },
  toggle() { setOpenState(!api.open); },
  setOpen(open) { setOpenState(open); },
  e2eTour() { return e2eTour(); },
};

// ------------------------------------------------------------------
// Rendu du panneau
// ------------------------------------------------------------------
function esc(s: unknown): string {
  return String(s).replace(/[&<>"']/g, (c) => (
    { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c] ?? c
  ));
}

function render(): void {
  if (!api.panel || !api.state) return;
  const list = (api.state.list || []).slice().sort((a, b) =>
    a.act - b.act || a.slot - b.slot);
  const found = list.filter((w) => w.activated);
  const cur = api.state.current || {};
  // --- découverts : boutons de voyage ---
  const rows = found.map((w) => {
    const tags = [
      w.here ? '<span class="wp-here">● ici</span>' : '',
      w.mvp ? '<span class="wp-mvp" title="Extension MVP : la table retail (patch_levels.txt) ne mappe pas ce niveau à un slot (255) — cf. impl-waypoints.md">MVP</span>' : '',
    ].join(' ');
    const travel = !!(w.travel && !w.here);
    return `<button class="wp-row${travel ? '' : ' wp-cur'}" data-slot="${w.slot}"` +
      `${travel ? '' : ' disabled'}>` +
      `<span class="wp-name">${esc(w.name)}</span>${tags}</button>`;
  });
  // --- non découverts : liste grisée (table réelle complète) ---
  const todo = list.filter((w) => !w.activated);
  const todoHtml = todo.length
    ? `<div class="wp-locked">${todo.map((w) =>
        `<div class="wp-locked-row">${esc(w.name)}</div>`).join('')}</div>`
    : '';
  const listEl = api.panel.querySelector('#wp-list');
  const zoneEl = api.panel.querySelector('#wp-zone');
  if (listEl) {
    listEl.innerHTML =
      (rows.length ? rows.join('') : '<div class="wp-empty">Aucun waypoint découvert — marchez jusqu’à une pierre.</div>') +
      (todoHtml ? `<div class="wp-locked-title">${todo.length} non découvert${todo.length > 1 ? 's' : ''}</div>${todoHtml}` : '');
  }
  if (zoneEl) zoneEl.textContent = cur.name ? `Zone : ${cur.name}` : '';
}

// ------------------------------------------------------------------
// Voyage
// ------------------------------------------------------------------
async function travel(slot: number): Promise<void> {
  const o = api.opts;
  const g = o?.getGame?.() ?? null;
  if (!g || !o) return;
  try {
    const out = parseOr<{ name?: string; slot: number; key: string }>(g.travelWaypoint(slot), { slot, key: '' });
    o.note?.(`Voyage : ${out.name ?? nameOf(slot)}.`);
    if (out.name !== undefined) o.onTraveled?.({ ...out, name: out.name });
    sfxUi(0.9);
  } catch (e) {
    o.note?.(`Voyage impossible : ${e instanceof Error ? e.message : String(e)}`);
  }
}

function sfxUi(rate: number): void {
  try {
    const m = api.opts?.manifest;
    if (m && m.sfx && api.opts?.sfx && m.sfx.uiClick) api.opts.sfx(m.sfx.uiClick, 0.5, rate);
  } catch { /* son indisponible : jamais bloquant */ }
}

// ------------------------------------------------------------------
// API publique
// ------------------------------------------------------------------
function initWaypoints(o: D2WaypointsOpts): D2WaypointsApi {
  if (api.ready) return api;
  api.opts = o;
  const doc = document;
  const panel = doc.createElement('div');
  panel.id = 'd2-waypoints';
  panel.hidden = true;
  panel.innerHTML =
    `<div class="wp-title">WAYPOINTS</div>` +
    `<div class="wp-zone" id="wp-zone"></div>` +
    `<div id="wp-list"></div>` +
    `<div class="wp-hint">Tab / bouton : ouvrir · clic = voyager (niveau régénéré seedé)</div>`;
  doc.body.appendChild(panel);
  api.panel = panel;
  panel.addEventListener('click', (e) => {
    const btn = (e.target as HTMLElement).closest('button.wp-row') as HTMLButtonElement | null;
    if (btn && !btn.disabled) void travel(Number(btn.dataset.slot));
  });
  // Tab : ouvrir/fermer (le panneau reste ouvert pendant les voyages).
  doc.addEventListener('keydown', (e) => {
    if (e.key === 'Tab') {
      e.preventDefault();
      setOpenState(!api.open);
    }
  });
  const btn = doc.getElementById('btn-waypoints');
  if (btn) btn.addEventListener('click', () => setOpenState(!api.open));
  api.ready = true;
  // Premier rendu si le moteur est déjà prêt.
  const g = o.getGame && o.getGame();
  if (g && g.waypoints) refresh(g.waypoints());
  return api;
}

function setOpenState(open: boolean): void {
  api.open = !!open;
  if (api.panel) api.panel.hidden = !api.open;
  sfxUi(api.open ? 1.3 : 1.0);
}

/** Rafraîchit l'état (JSON de game.waypoints()) — ~2×/s depuis la boucle. */
function refresh(jsonText: string): void {
  try {
    api.state = parseOr<WaypointState>(jsonText, { list: [] });
    render();
  } catch { /* JSON partiel : on garde le dernier état */ }
}

/** Nom d'un slot (pour les messages d'événements) — dernier état connu. */
function nameOf(slot: number): string {
  const w = api.state && (api.state.list || []).find((x) => x.slot === slot);
  return w ? w.name : `slot ${slot}`;
}

// ------------------------------------------------------------------
// Sonde E2E (demo/waypoints-e2e.png) : vraie marche → contact → panneau →
// voyage. Polling sur la surface publique (waypoints()/travelWaypoint()).
// ------------------------------------------------------------------
async function e2eTour(): Promise<WaypointState> {
  const g = api.opts?.getGame?.() ?? null;
  const q = api.opts?.queue;
  if (!g || !q || !api.opts) throw new Error('E2E: moteur ou queue absent');
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
  const activated = (slot: number): boolean => {
    refresh(g.waypoints());
    const w = (api.state?.list || []).find((x) => x.slot === slot);
    return !!(w && w.activated);
  };
  const walkUntil = async (slot: number, deadlineMs: number): Promise<boolean> => {
    const t0 = Date.now();
    while (Date.now() - t0 < deadlineMs) {
      if (activated(slot)) return true;
      const entry: WaypointEntry | undefined = api.state?.list.find((x) => x.slot === slot);
      if (entry?.pos) {
        q.push({ type: 'move', x: Math.round(entry.pos[0] ?? 0), y: Math.round(entry.pos[1] ?? 0) });
      }
      await sleep(400);
    }
    return false;
  };
  // 1) pierre du Camp des Rousses (objet TownN1.ds1 (144,51)) au contact.
  if (!(await walkUntil(0, 25000))) throw new Error('E2E: waypoint camp non activé');
  api.opts.flash?.(`Waypoint découvert : ${nameOf(0)}`);
  // 2) passage à la Blood Moor par l'entrée du warp (téléport debug = __d2.goto).
  g.switchLevel('bloodmoor', 137, 162);
  await sleep(300);
  // 3) pierre de la Blood Moor (chunk DRLG (27,27) → centre (152,152)).
  if (!(await walkUntil(39, 25000))) throw new Error('E2E: waypoint bloodmoor non activé');
  api.opts.flash?.(`Waypoint découvert : ${nameOf(39)}`);
  // 4) panneau + voyage retour vers le camp (régénération seedée côté Rust).
  setOpenState(true);
  await sleep(400);
  await travel(0);
  await sleep(600);
  setOpenState(true); // panneau ouvert pour la capture
  refresh(g.waypoints());
  return api.state ?? { list: [] };
}

export function installWaypoints(win: D2Window): void {
  registerModule('waypoints', 'ts');
  win.D2Waypoints = api;
}

if (typeof window !== 'undefined') installWaypoints(window as unknown as D2Window);
