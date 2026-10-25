// Diablo 2 WASM — carte mercenaire (H) — migration TS 25a de demo/merc-ui.js.
// Contrat window.D2Merc INCHANGÉ : init({ getGame }) + frame(game) (pollé
// ~2×/s par la boucle, même throttle que les waypoints).
// Le moteur porte toute la logique (hireling.txt v1.12) ; mercInfo() = fiche
// JSON `{kind,subtype,level,hp,maxHp,alive,x,y,sx,sy}` ou `null`.
import { registerModule,
   parseOr, type D2Game, type D2MercApi, type D2Window, type MercInfo } from './types';

const api: D2MercApi & {
  opts: { getGame?: () => D2Game | null } | null;
  card: HTMLDivElement | null;
  lastPoll: number;
} = {
  ready: false,
  opts: null,
  card: null,
  lastPoll: 0,
  init(o) { return initMerc(o); },
  frame(game) { frameMerc(game); },
};

/** Ligne d'état de la carte (pur, testable). */
export function mercStateLine(m: MercInfo): string {
  return m.alive ? `${m.hp}/${m.maxHp} PV` : 'KO — résurrection chez le NPC';
}

function initMerc(o: { getGame: () => D2Game | null }): D2MercApi {
  if (api.ready) return api;
  api.opts = o;
  const card = document.createElement('div');
  card.id = 'd2-merc';
  card.style.cssText = 'position:absolute; left:10px; top:34px; z-index:25;' +
    'min-width:150px; font:11px monospace; color:#d0b070;' +
    'background:rgba(16,12,6,.85); border:1px solid #5a3c1e; padding:4px 8px; display:none;';
  card.innerHTML =
    `<div id="merc-name">—</div>` +
    `<div style="height:5px; background:#241c10; border:1px solid #5a3c1e; margin-top:3px;">` +
    `<div id="merc-hp" style="height:100%; width:0%; background:#8c2b1e;"></div></div>` +
    `<div id="merc-state" style="color:#7a6a4a;"></div>`;
  document.body.appendChild(card);
  api.card = card;
  api.ready = true;
  return api;
}

// Pollé par la boucle (throttle 500 ms côté appelant).
function frameMerc(game: D2Game): void {
  if (!api.card || !game || !game.mercInfo) return;
  let m: MercInfo | null = null;
  try {
    m = parseOr<MercInfo | null>(game.mercInfo(), null);
  } catch {
    m = null;
  }
  if (!m) {
    api.card.style.display = 'none';
    return;
  }
  api.card.style.display = 'block';
  const name = api.card.querySelector('#merc-name');
  if (name) name.textContent = `${m.kind || 'Mercenaire'} · niv ${m.level ?? '?'}`;
  const pct = m.maxHp > 0 ? Math.round((100 * m.hp) / m.maxHp) : 0;
  const bar = api.card.querySelector('#merc-hp') as HTMLDivElement | null;
  if (bar) {
    bar.style.width = `${pct}%`;
    bar.style.background = m.alive ? '#8c2b1e' : '#333';
  }
  const state = api.card.querySelector('#merc-state');
  if (state) state.textContent = mercStateLine(m);
}

export function installMerc(win: D2Window): void {
  registerModule('merc-ui', 'ts');
  win.D2Merc = api;
}

if (typeof window !== 'undefined') installMerc(window as unknown as D2Window);
