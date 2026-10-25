// Diablo 2 WASM — journal de quêtes ACTES IV-V (étape 10c) — migration TS
// 25a de demo/quests45.js. Miroir du moteur d2rules/src/quests_actes45.rs :
// 9 quêtes A4-A5, phases inactive/active/reward_pending/done, sous-objectifs
// hors ordre (sceaux, cages, Anciens), drapeaux .d2s (a4 : offset 395-399,
// a5 : 415-425 — ordre des ids = ORDRE DE SAUVEGARDE .d2s).
// Contrat window.D2Quests45 INCHANGÉ ; Quests45Log est pur et testable bun.
import { registerModule,
   D2Quests45Api, D2Window } from './types';

// ---------------------------------------------------------------------
// Authoring A4/A5 — miroir de DEFS_ACT4/DEFS_ACT5 (quests_actes45.rs).
// ---------------------------------------------------------------------
export interface Step45 {
  desc: string;
  zone?: number;
  kill?: string[];
  talk?: string;
  item?: string;
  any?: Array<{ kill?: string[]; zone?: number }>;
  count?: number;
}

export interface Quest45Def {
  id: string;
  act: number;
  name: string;
  gates?: string;
  steps: Step45[];
  npc: string | null;
  requires?: string;
  final?: boolean;
}

export const DEFS45: Quest45Def[] = [
  {
    id: 'a4q1', act: 4, name: 'The Fallen Angel', gates: 'act5',
    steps: [
      { desc: 'Traversez les Plaines du Désespoir.', zone: 105 },
      { desc: 'Abattez Izual pour libérer son âme.', kill: ['izual'] },
    ], npc: 'tyrael',
  },
  {
    id: 'a4q2', act: 4, name: "Terror's End", gates: 'act5',
    steps: [
      { desc: 'Trouvez Diablo dans son sanctuaire (Chaos Sanctuaire).', zone: 108 },
      { desc: 'Brisez les sceaux et abattez leurs gardiens.',
        any: [{ kill: ['infector of souls', 'megademon3'] }, { kill: ['lord de seis', 'doomknight3'] }, { kill: ['grand vizier of chaos', 'fingermage3'] }] },
      { desc: 'Tuez Diablo.', kill: ['diablo'] },
    ], npc: 'tyrael',
  },
  {
    id: 'a4q3', act: 4, name: "Hell's Forge",
    steps: [
      { desc: "Portez la Pierre d'âme de Mephisto.", item: 'mss' },
      { desc: 'Descendez dans la Rivière de feu.', zone: 107 },
      { desc: 'Abattez Hephasto le forgeron.', kill: ['hephasto', 'The Feature Creep'] },
      { desc: 'Emparez-vous du Marteau de la Forge infernale.', item: 'hfh' },
    ], npc: 'cain',
  },
  {
    id: 'a5q1', act: 5, name: 'Siege on Harrogath',
    steps: [
      { desc: 'Percez les Contreforts sanglants et levez le siège.', zone: 110 },
      { desc: 'Abattez Shenk le Surveillant (Siege Boss).', kill: ['siegeboss', 'Siege Boss', 'overseer1', 'shenk'] },
    ], npc: 'larzuk',
  },
  {
    id: 'a5q2', act: 5, name: 'Rescue on Mount Arreat',
    steps: [
      { desc: 'Libérez les barbares prisonniers des Hautes-Glaces (4 extérieurs).',
        any: [{ zone: 110 }, { zone: 111 }, { zone: 112 }, { zone: 117 }], count: 4 },
    ], npc: 'qual-kehk',
  },
  {
    id: 'a5q3', act: 5, name: 'Prison of Ice',
    steps: [
      { desc: 'Trouvez Anya gelée sous le Passage Cristallin (Ice Cave 3A).', zone: 119 },
      { desc: 'Abattez Frozenstein qui garde la prison de glace.', kill: ['frozenstein', 'snowyeti4', 'Frozenstein'] },
      { desc: 'Donnez à Anya la potion de Malah et ramenez-la en ville.', talk: 'anya' },
    ], npc: 'malah',
  },
  {
    id: 'a5q4', act: 5, name: 'Betrayal of Harrogath', requires: 'a5q3',
    steps: [
      { desc: "Prenez le portail d'Anya vers le temple de Nihlathak.", talk: 'anya' },
      { desc: 'Descendez jusqu’au Repaire de Vaught (Halls of Vaught).', zone: 124 },
      { desc: 'Tuez Nihlathak le traître.', kill: ['nihlathak', 'nihlathakboss', 'Nihlathak Boss'] },
    ], npc: 'anya',
  },
  {
    id: 'a5q5', act: 5, name: 'Rite of Passage',
    steps: [
      { desc: "Consultez les Anciens en activant l'Autel des Cieux (Sommet d'Arreat).", zone: 120 },
      { desc: 'Affrontez les trois Anciens sans quitter le Mont Arreat.',
        any: [{ kill: ['ancientbarb1', 'Ancient Barbarian 1'] }, { kill: ['ancientbarb2', 'Ancient Barbarian 2'] }, { kill: ['ancientbarb3', 'Ancient Barbarian 3'] }] },
    ], npc: null,
  },
  {
    id: 'a5q6', act: 5, name: 'Eve of Destruction', requires: 'a5q5', final: true,
    steps: [
      { desc: 'Affrontez les vagues de Baal dans le Trône de la Destruction.', zone: 131 },
      { desc: "Tuez Baal dans la Chambre de la Pierre d'Worldstone.", kill: ['baal', 'baalcrab'] },
      { desc: 'Parlez à Tyrael et prenez son portail.', talk: 'tyrael' },
    ], npc: null,
  },
];

export const ACT_TITLES: Readonly<Record<number, string>> = {
  4: 'Acte IV — la Forteresse de l’Enfer',
  5: 'Acte V — Harrogath (fin de jeu)',
};

export const STATE_VERSION_45 = 1;
export type Phase45 = 'inactive' | 'active' | 'reward_pending' | 'done';
export interface Run45 { id: null; phase: Phase45; step: number; count: number; flags: number; subdone: number }

/** Un événement ActEvent sérialisé (serde tagged) → normalisé. */
export interface ActEvent45 {
  kind: string;
  code?: string;
  superunique?: string | null;
  level_id?: number;
  npc?: string;
}

/** Normalise {"MonsterKilled":{"code":"izual"}} → {kind, code…}. */
export function normalizeActEvent(event: unknown): ActEvent45 | null {
  if (!event || typeof event !== 'object') return null;
  const keys = Object.keys(event as object);
  if (!keys.length) return null;
  const kind = keys[0]!;
  const rest = (event as Record<string, unknown>)[kind];
  const payload = rest && typeof rest === 'object' ? rest as Record<string, unknown> : {};
  return { kind, ...payload } as ActEvent45;
}

/** Un trigger (kill/zone/item/talk/any) matche-t-il l'événement ? */
export function trigMatches(t: Step45, ev: ActEvent45): boolean {
  if (t.kill) {
    if (ev.kind !== 'MonsterKilled') return false;
    return t.kill.some((a) => a.toLowerCase() === (ev.code || '').toLowerCase()
      || a.toLowerCase() === (ev.superunique || '').toLowerCase());
  }
  if (t.zone !== undefined) return ev.kind === 'ZoneEntered' && ev.level_id === t.zone;
  if (t.item !== undefined) return ev.kind === 'ItemPicked' && String(ev.code).toLowerCase() === t.item.toLowerCase();
  if (t.talk !== undefined) return ev.kind === 'NpcTalked' && String(ev.npc).toLowerCase() === t.talk.toLowerCase();
  return false;
}

function stepOf(def: Quest45Def, r: Run45): Step45 | null {
  return r.step < def.steps.length ? def.steps[r.step]! : null;
}

export class Quests45Log {
  runs: Run45[];

  constructor() {
    this.runs = DEFS45.map((): Run45 => ({ id: null, phase: 'inactive', step: 0, count: 0, flags: 0, subdone: 0 }));
  }

  index(id: string): number {
    return DEFS45.findIndex((d) => d.id === id);
  }

  phase(id: string): Phase45 | null {
    const i = this.index(id);
    return i < 0 ? null : this.runs[i]!.phase;
  }

  done(id: string): boolean {
    return this.phase(id) === 'done';
  }

  gateOpen(i: number): boolean {
    const d = DEFS45[i]!;
    if (d.requires && !this.done(d.requires)) return false;
    return true;
  }

  /** Avance le journal ; renvoie les changements de phase —
   * `{id:"a5q6", phase:"done", final:true}` = fin de jeu (GameWon). */
  onEvent(event: unknown): Array<{ id: string; phase: Phase45; final?: boolean }> {
    const ev = normalizeActEvent(event);
    if (!ev) return [];
    const out: Array<{ id: string; phase: Phase45; final?: boolean }> = [];
    for (let i = 0; i < DEFS45.length; i++) {
      const d = DEFS45[i]!;
      const r = this.runs[i]!;
      if (r.phase === 'done') continue;
      if (r.phase === 'reward_pending') {
        if (ev.kind === 'NpcTalked' && d.npc && String(ev.npc).toLowerCase() === d.npc.toLowerCase()) {
          r.phase = 'done';
          r.flags = (r.flags & ~2) | 1;
          if (d.id === 'a5q3') r.flags |= 128; // bit 7 : parchemin de résistance consommé
          out.push({ id: d.id, phase: 'done' });
        }
        continue;
      }
      if (!this.gateOpen(i)) continue;
      const st = stepOf(d, r);
      if (!st) continue;
      let hit = -1;
      if (st.any) {
        hit = st.any.findIndex((t, j) => !((r.subdone >> j) & 1) && trigMatches({ desc: '', ...t } as Step45, ev));
        if (hit < 0) continue;
        r.subdone |= 1 << hit;
      } else if (!trigMatches(st, ev)) {
        continue;
      }
      if (r.phase === 'inactive') {
        r.phase = 'active';
        if (ev.kind === 'NpcTalked') r.flags |= 4; // bit 2 « donnée par PNJ »
        out.push({ id: d.id, phase: 'active' });
      }
      r.count += 1;
      const req = Math.max(1, st.count ?? (st.any ? st.any.length : 1));
      if (r.count >= req) {
        r.count = 0;
        r.subdone = 0;
        if (r.step + 1 < d.steps.length) {
          r.step += 1;
        } else if (d.npc) {
          r.phase = 'reward_pending';
          r.flags |= 2; // bit 1 « récompense à prendre »
          out.push({ id: d.id, phase: 'reward_pending' });
        } else {
          r.phase = 'done';
          r.flags = (r.flags & ~2) | 1;
          if (d.id === 'a5q3') r.flags |= 128;
          out.push({ id: d.id, phase: 'done', final: !!d.final });
        }
      }
    }
    return out;
  }

  to_json(): string {
    return JSON.stringify({
      version: STATE_VERSION_45,
      // CORRECTIF TS (documenté) : le Legacy étalait `...this.runs[i]` (dont
      // id: null) APRÈS id/act — l'export portait id:null et l'import
      // D2Quests45.importLog(exportLog()) levait toujours « id null ≠ a4q1 ».
      // L'id/act font foi, écrasés en dernier ; le round-trip documenté
      // fonctionne (le format des CHAMPS est inchangé pour le Rust).
      quests: DEFS45.map((d, i) => ({ ...this.runs[i], id: d.id, act: d.act })),
    });
  }

  static from_json(json: string): Quests45Log {
    const s: unknown = JSON.parse(json); // throw si invalide (contrat Rust)
    const o = s as { version?: number; quests?: unknown };
    if (o.version !== STATE_VERSION_45 || !Array.isArray(o.quests) || o.quests.length !== DEFS45.length) {
      throw new Error('quests45: état inconnu');
    }
    const log = new Quests45Log();
    o.quests.forEach((qRaw: unknown, i: number) => {
      const q = qRaw as { id?: string; phase?: string; step?: number; count?: number; flags?: number; subdone?: number };
      if (q.id !== DEFS45[i]!.id) throw new Error(`quests45: id ${q.id} ≠ ${DEFS45[i]!.id}`);
      if (!['inactive', 'active', 'reward_pending', 'done'].includes(q.phase ?? '')) {
        throw new Error(`quests45: phase ${q.phase}`);
      }
      log.runs[i] = {
        id: null,
        phase: q.phase as Phase45,
        step: q.step ?? 0,
        count: q.count ?? 0,
        flags: q.flags ?? 0,
        subdone: q.subdone ?? 0,
      };
    });
    return log;
  }
}

// ---------------------------------------------------------------------
// Panneau journal A4-A5 (self-contenu, même style que quests.ts).
// ---------------------------------------------------------------------
const CSS = `
#d2-quests45 { position: fixed; top: 14px; right: 334px; width: 300px; max-height: 78vh;
  overflow-y: auto; background: #14110aee; border: 1px solid #5a4a2a; border-radius: 4px;
  padding: 10px 12px; z-index: 60; color: #cfc7b8; font: 12px/1.45 system-ui, sans-serif;
  box-shadow: 0 4px 24px #000c; }
#d2-quests45 h2 { margin: 0 0 2px; font-size: 13px; font-weight: 700; letter-spacing: .1em;
  color: #c7a45a; text-transform: uppercase; }
#d2-quests45 .d2q-act { color: #8f8676; font-size: 11px; margin: 6px 0 4px; letter-spacing: .06em; }
#d2-quests45 ul { list-style: none; margin: 0; padding: 0; }
#d2-quests45 li { padding: 6px 0; border-top: 1px solid #2e2718; }
#d2-quests45 .d2q-name { font-weight: 600; font-size: 12px; }
#d2-quests45 .d2q-status { float: right; font-size: 10px; font-weight: 700; letter-spacing: .08em;
  text-transform: uppercase; margin-left: 6px; }
#d2-quests45 .d2q-obj { color: #a89670; font-size: 11px; margin-top: 2px; }
#d2-quests45 .d2q-prog { color: #c7a45a; font-weight: 600; }
#d2-quests45 .inactive .d2q-name { color: #6e6555; }
#d2-quests45 .active .d2q-name { color: #e8dcbf; }
#d2-quests45 .active .d2q-status { color: #d9b96a; }
#d2-quests45 .reward_pending .d2q-status { color: #d9b96a; }
#d2-quests45 .done .d2q-name { color: #c7a45a; }
#d2-quests45 .done .d2q-status { color: #7ea05f; }
#d2-quests45[hidden] { display: none; }
`;

const STATUS_FR: Readonly<Record<Phase45, string>> = {
  inactive: 'À venir', active: 'En cours', reward_pending: 'Récompense', done: 'Terminée',
};

let doc: Document | null = null;
let panel: HTMLDivElement | null = null;
let listEl: HTMLUListElement | null = null;
let log = new Quests45Log();
let isOpen = false;

function render(): void {
  if (!listEl || !doc) return;
  listEl.textContent = '';
  let lastAct = 0;
  DEFS45.forEach((d, i) => {
    const r = log.runs[i]!;
    if (d.act !== lastAct) {
      lastAct = d.act;
      const act = doc!.createElement('div');
      act.className = 'd2q-act';
      act.textContent = ACT_TITLES[d.act] ?? '';
      listEl!.appendChild(act);
    }
    const li = doc!.createElement('li');
    li.className = r.phase;
    const head = doc!.createElement('div');
    const status = doc!.createElement('span');
    status.className = 'd2q-status';
    status.textContent = (r.phase === 'done' ? '✔ ' : '') + STATUS_FR[r.phase];
    const name = doc!.createElement('span');
    name.className = 'd2q-name';
    name.textContent = d.name;
    head.append(status, name);
    li.appendChild(head);
    if (r.phase === 'active') {
      const st = stepOf(d, r);
      const obj = doc!.createElement('div');
      obj.className = 'd2q-obj';
      obj.textContent = st ? st.desc : '';
      if (st && st.any) {
        const prog = doc!.createElement('span');
        prog.className = 'd2q-prog';
        prog.textContent = ` (${r.count}/${Math.max(1, st.count ?? st.any.length)})`;
        obj.appendChild(prog);
      }
      li.appendChild(obj);
    } else if (r.phase === 'reward_pending') {
      const obj = doc!.createElement('div');
      obj.className = 'd2q-obj';
      obj.textContent = 'Retournez voir le PNJ pour la récompense.';
      li.appendChild(obj);
    } else if (r.phase === 'done' && d.final) {
      const obj = doc!.createElement('div');
      obj.className = 'd2q-obj';
      obj.textContent = 'Fin de jeu — la Pierre d’Worldstone est détruite.';
      li.appendChild(obj);
    }
    listEl!.appendChild(li);
  });
}

export const quests45Api: D2Quests45Api & { Quests45Log?: unknown; DEFS?: unknown } = {
  Quests45Log,
  DEFS: DEFS45,
  init(dom?: Document): D2Quests45Api {
    doc = dom ?? (typeof document !== 'undefined' ? document : null);
    if (!doc || panel) return quests45Api;
    const style = doc.createElement('style');
    style.textContent = CSS;
    doc.head.appendChild(style);
    panel = doc.createElement('div');
    panel.id = 'd2-quests45';
    panel.hidden = true;
    const title = doc.createElement('h2');
    title.textContent = 'Journal — Actes IV-V';
    listEl = doc.createElement('ul');
    panel.append(title, listEl);
    doc.body.appendChild(panel);
    doc.addEventListener('d2quests45:toggle', () => quests45Api.toggle());
    return quests45Api;
  },
  feed(event: unknown) {
    const changes = log.onEvent(event);
    if (isOpen) render();
    return changes;
  },
  get log() { return log; },
  reset() { log = new Quests45Log(); if (isOpen) render(); },
  exportLog() { return log.to_json(); },
  importLog(json: string): boolean {
    log = Quests45Log.from_json(json);
    if (isOpen) render();
    return true;
  },
  show() { isOpen = true; if (panel) panel.hidden = false; render(); },
  hide() { isOpen = false; if (panel) panel.hidden = true; },
  toggle() { if (isOpen) quests45Api.hide(); else quests45Api.show(); },
  get isOpen() { return isOpen; },
};

export function installQuests45(win: D2Window): void {
  registerModule('quests45', 'ts');
  win.D2Quests45 = quests45Api;
}

if (typeof window !== 'undefined') installQuests45(window as unknown as D2Window);
