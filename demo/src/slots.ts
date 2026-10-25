// Diablo 2 WASM — gestionnaire de slots de sauvegarde (mission save P0) —
// migration TS 25a de demo/slots.js (contrat window.D2Slots INCHANGÉ).
//
// Stockage : localStorage["d2wasm.slots.v1"] = {"slots":[{…}]} — 5 slots
// nommés, identité par héros. Rétro-compat clé historique "d2wasm.save.v1"
// (miroir du save le plus récent ; importé en slot fondateur si base vide).
//
// Migration : les fonctions de MÉTIER sont pures (testables bun) ; la partie
// DOM (exportSlot : téléchargement) reste derrière un garde `document`.
// Contrairement aux modules chargés par game.js, slots.js est un <script>
// classique de game.html/menu.html : aucun effet de bord au scope, le
// contrat window.D2Slots peut donc être remplacé par la voie TS dès
// l'évaluation du bundle (avant le corps de module de game.js).
import { registerModule,
   SaveSlot, SaveSlotMeta, SaveJson, D2SlotsApi, D2Window } from './types';

export const SLOTS_KEY = 'd2wasm.slots.v1';
export const LEGACY_SAVE_KEY = 'd2wasm.save.v1';
export const MAX_SLOTS = 5;

/** Libellés FR des classes (mêmes codes que manifest.heroes / menu.js). */
export const HERO_LABELS: Readonly<Record<string, string>> = {
  BA: 'Barbare', SOR: 'Sorcière', SO: 'Sorcière', PA: 'Paladin', AM: 'Amazone',
  AS: 'Assassin', DZ: 'Druide', NE: 'Nécromancien',
};

const ZONES: Readonly<Record<string, string>> = {
  town: 'Camp des Rousses', bloodmoor: 'Blood Moor',
};

export interface SlotsStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

/** Abstraction stockage (localStorage réel ou injecté en test). */
let storage: SlotsStorage | null = null;
function store(): SlotsStorage {
  if (storage) return storage;
  try {
    const ls = globalThis.localStorage;
    if (ls) return ls;
  } catch { /* stockage indisponible */ }
  const mem = new Map<string, string>();
  return {
    getItem: (k) => (mem.has(k) ? (mem.get(k) as string) : null),
    setItem: (k, v) => void mem.set(k, String(v)),
  };
}

/** Injection test (bun) : isole localStorage entre les cas. */
export function setSlotsStorage(s: SlotsStorage | null): void {
  storage = s;
}

function readSlots(): SaveSlot[] {
  try {
    const r: unknown = JSON.parse(store().getItem(SLOTS_KEY) || 'null');
    if (r && typeof r === 'object' && Array.isArray((r as { slots?: unknown }).slots)) {
      return ((r as { slots: unknown[] }).slots as SaveSlot[]).filter(
        (s) => s && s.id && typeof s.save === 'string',
      );
    }
  } catch { /* base corrompue : repart à vide */ }
  return [];
}

function writeSlots(slots: SaveSlot[]): void {
  store().setItem(SLOTS_KEY, JSON.stringify({ slots }));
  // miroir : la clé historique = save le plus récent (menu Continuer,
  // game.html?continue=1 des anciens onglets).
  const newest = slots.slice().sort((a, b) => b.ts - a.ts)[0];
  if (newest) store().setItem(LEGACY_SAVE_KEY, newest.save);
}

function newId(): string {
  return 's' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}

/** Métadonnées extraites d'un JSON de sauvegarde d2rules (v1..v3) —
 * null si le JSON est inexploitable (pas un save). */
export function metaOf(saveJson: string | SaveJson): SaveSlotMeta | null {
  try {
    const v: unknown = typeof saveJson === 'string' ? JSON.parse(saveJson) : saveJson;
    if (!v || typeof v !== 'object') return null;
    const o = v as SaveJson;
    if (!o.hero || !o.version) return null;
    const heroCode = String(o.hero_code || 'BA').toUpperCase();
    const level = (o.hero && o.hero.stats && o.hero.stats.level) || 1;
    const playtime = o.now_ms || 0;
    const levelKey = typeof o.level === 'string' && o.level ? o.level.toLowerCase() : 'town';
    return {
      hero_code: heroCode,
      hero_label: HERO_LABELS[heroCode] ?? heroCode,
      level,
      playtime_ms: playtime,
      level_key: levelKey,
      zone: ZONES[levelKey] ?? levelKey,
      version: o.version,
    };
  } catch {
    return null;
  }
}

function defaultName(meta: SaveSlotMeta): string {
  return meta.hero_label + ' niv. ' + meta.level;
}

/** Liste des slots — migration douce du save historique en slot fondateur,
 * tri plus récent d'abord. */
export function listSlots(): SaveSlot[] {
  let slots = readSlots();
  if (!slots.length) {
    const legacy = store().getItem(LEGACY_SAVE_KEY);
    if (legacy) {
      const meta = metaOf(legacy);
      if (meta) {
        slots = [Object.assign({ id: newId(), name: defaultName(meta), ts: Date.now() }, meta, { save: legacy })];
        writeSlots(slots);
      }
    }
  }
  return slots.slice().sort((a, b) => b.ts - a.ts);
}

/** Crée/écrase le slot du héros (identité hero_code) avec ce save. */
export function upsertFromSave(saveJson: string, opts?: { name?: string }): SaveSlot {
  const meta = metaOf(saveJson);
  if (!meta) throw new Error('sauvegarde illisible (JSON d2rules attendu)');
  const slots = readSlots();
  const now = Date.now();
  const existing = slots.find((s) => s.hero_code === meta.hero_code);
  if (existing) {
    existing.ts = now;
    existing.save = saveJson;
    Object.assign(existing, meta);
    writeSlots(slots);
    return existing;
  }
  if (slots.length >= MAX_SLOTS) {
    throw new Error('Slots pleins (' + MAX_SLOTS + ') — supprimez un héros depuis le menu « Parties ».');
  }
  const slot: SaveSlot = Object.assign(
    { id: newId(), name: (opts && opts.name) || defaultName(meta), ts: now },
    meta,
    { save: saveJson },
  );
  slots.push(slot);
  writeSlots(slots);
  return slot;
}

export function getSlot(id: string): SaveSlot | null {
  return readSlots().find((s) => s.id === id) ?? null;
}

export function removeSlot(id: string): SaveSlot[] {
  const slots = readSlots().filter((s) => s.id !== id);
  writeSlots(slots);
  return slots;
}

export function renameSlot(id: string, name: string): SaveSlot | undefined {
  const slots = readSlots();
  const slot = slots.find((s) => s.id === id);
  const clean = String(name || '').trim().slice(0, 24);
  if (slot && clean) {
    slot.name = clean;
    writeSlots(slots);
  }
  return slot;
}

/** Sauvegarde la plus récente (pour « Continuer »), null si aucune. */
export function mostRecentSlot(): SaveSlot | null {
  return listSlots()[0] ?? null;
}

function downloadFilename(slot: SaveSlot): string {
  const slug = slot.name.replace(/[^a-z0-9]+/gi, '-').toLowerCase();
  const day = new Date(slot.ts).toISOString().slice(0, 10);
  return `d2-${slug}-${day}.json`;
}

/** Export .json : le slot complet (métadonnées + save) — téléchargement. */
export function exportSlot(id: string, doc: Document = document): SaveSlot {
  const slot = getSlot(id);
  if (!slot) throw new Error('slot inconnu');
  const blob = new Blob([JSON.stringify(slot, null, 1)], { type: 'application/json' });
  const a = doc.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = downloadFilename(slot);
  doc.body.appendChild(a);
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(a.href);
    a.remove();
  }, 400);
  return slot;
}

/** Import : lit le texte d'un .json exporté (slot complet OU save brut),
 * valide, l'installe (fiche de CE héros remplacée, même id si existante). */
export function importSlot(text: string): SaveSlot {
  const v: unknown = JSON.parse(text); // throw = message erreur appelant
  if (v && typeof v === 'object') {
    const o = v as { save?: unknown; hero_code?: unknown; name?: unknown; ts?: unknown };
    if (typeof o.save === 'string' && o.hero_code) {
      // format export complet
      const meta = metaOf(o.save);
      if (!meta) throw new Error('fichier : sauvegarde interne illisible');
      const slots = readSlots();
      const heroCode = String(o.hero_code).toUpperCase();
      const existingIdx = slots.findIndex((s) => s.hero_code === heroCode);
      const slot: SaveSlot = Object.assign(
        {
          id: existingIdx >= 0 ? slots[existingIdx]!.id : newId(),
          name: typeof o.name === 'string' && o.name ? o.name : defaultName(meta),
          ts: typeof o.ts === 'number' ? o.ts : Date.now(),
        },
        meta,
        { save: o.save },
      );
      if (existingIdx >= 0) slots.splice(existingIdx, 1, slot);
      else {
        if (slots.length >= MAX_SLOTS) throw new Error('Slots pleins (' + MAX_SLOTS + ') — supprimez un héros.');
        slots.push(slot);
      }
      writeSlots(slots);
      return slot;
    }
  }
  // format save brut (game.html Sauvegarder → export manuel)
  return upsertFromSave(text);
}

/** API window.D2Slots (contrat identique au Legacy). */
export function installSlots(win: D2Window): D2SlotsApi {
  registerModule('slots', 'ts');
  const api: D2SlotsApi = {
    MAX: MAX_SLOTS,
    list: listSlots,
    get: getSlot,
    remove: removeSlot,
    rename: renameSlot,
    upsertFromSave,
    mostRecent: mostRecentSlot,
    metaOf,
    exportSlot,
    importSlot,
  };
  win.D2Slots = api;
  return api;
}

if (typeof window !== 'undefined') installSlots(window as unknown as D2Window);
