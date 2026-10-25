// Étape 25a — types du moteur et contrats window.* (source de vérité :
// demo/game-pkg/d2game.d.ts + les endpoints consommés par game.js et les
// modules UI). Le JSON sérialisé du moteur (serde, externally-tagged) est
// décrit ici une seule fois ; tous les modules TS migrés s'appuient dessus.
//
// NB : le pkg wasm de CET arbre n'expose pas encore tous les endpoints
// récents (heroPanel, stashInfo, cubeTransmute, exportD2s, isFinalized…) —
// ils sont typés optionnels et chaque appel est gardé, comme le JS Legacy.

// ---------------------------------------------------------------------------
// Événements moteur (GameEvent d2rules + extras multi-niveaux de d2game)
// ---------------------------------------------------------------------------

/** Objet d'inventaire / au sol (ItemDef MVP : kind + code + name + amount). */
export interface Item {
  code: string;
  name: string;
  kind: 'Potion' | 'Weapon' | 'Armor' | 'Gold' | 'Misc' | string;
  amount?: number;
}

/** Objet projeté au sol (groundItems, labels Alt du HUD). */
export interface GroundItem extends Item {
  index: number;
  x: number;
  y: number;
  sx: number;
  sy: number;
}

/** Fiche de survol d'un monstre (monsterInfo). */
export interface MonsterInfo {
  code: string;
  name: string;
  kind: 'normal' | 'champion' | 'unique' | 'super_unique' | 'boss' | string;
  alive: boolean;
  hp: number;
  maxHp: number;
  x: number;
  y: number;
  sx: number;
  sy: number;
}

/**
 * Événements du tick — représentation large (champs optionnels) fidèle à la
 * sérialisation externally-tagged : chaque variante est une clé de premier
 * niveau portant sa charge (ou `true`-like vide pour les variantes sans
 * données — représentées par un objet vide).
 */
export interface GameEvent {
  LevelSwitch?: { key: string; name: string };
  LevelSwitchFailed?: string;
  HeroMoved?: Record<string, never>;
  HeroAttacked?: { target: number; dmg?: number };
  HeroMissed?: { target: number };
  HeroHit?: { index: number; dmg?: number };
  MonsterDied?: { index: number; xp?: number };
  ItemDropped?: { item: Item };
  HeroLeveled?: { level: number };
  GoldGained?: { amount: number };
  HeroPickedUp?: Record<string, never>;
  PotionUsed?: { heal: number };
  InventoryFull?: Record<string, never>;
  HeroDied?: Record<string, never>;
  CorpseDropped?: { x: number; y: number; gold?: number };
  CorpseRecovered?: Record<string, never>;
  HeroRespawned?: Record<string, never>;
  MissileSpawned?: { skill?: string; [k: string]: unknown };
  MissileHit?: { target: number; dmg?: number };
  MissileMissed?: { target: number };
  MonFled?: Record<string, never>;
  MonResurrected?: Record<string, never>;
  MercAttacked?: { target: number };
  MercMissed?: { target: number };
  MercHit?: Record<string, never>;
  MercDied?: Record<string, never>;
  MercLeveled?: { level?: number };
  MercResurrected?: Record<string, never>;
  MercMissileSpawned?: { [k: string]: unknown };
  ActQuestChanged?: { phase?: string; [k: string]: unknown };
  ActUnlocked?: { act?: number };
  GameWon?: Record<string, never>;
  NpcTalked?: { npc?: string; [k: string]: unknown };
  WaypointActivated?: { slot?: number };
  [variant: string]: unknown;
}

// ---------------------------------------------------------------------------
// JSON du héros / du monde
// ---------------------------------------------------------------------------

/** heroStats() — HUD chaque frame. */
export interface HeroStats {
  life: number;
  max_life: number;
  mana: number;
  max_mana: number;
  level: number;
  xp: number;
  gold: number;
  stamina?: number;
  max_stamina?: number;
  skill?: string;
  [k: string]: unknown;
}

/** heroState() — drapeau course + anim courante. */
export interface HeroState {
  anim: string;
  frame?: number;
  facing?: number;
  run: boolean;
  moving: boolean;
}

/** Compétence connue (heroSkills — boutons L/R du HUD). */
export interface HeroSkill {
  index: number;
  code: string;
  name: string;
  level: number;
  missile: string | null;
}

/** heroClass() — résolution (code + libellé). */
export interface HeroClassInfo {
  code: string;
  name: string;
}

/** levelInfo() — niveau courant. */
export interface LevelInfo {
  key?: string;
  name?: string;
  [k: string]: unknown;
}

/** difficultyInfo() — badge HUD. */
export interface DifficultyInfo {
  difficulty?: string;
  label?: string;
}

/** Fiche mercenaire (mercInfo). */
export interface MercInfo {
  kind: string;
  subtype?: string;
  level: number;
  hp: number;
  maxHp: number;
  alive: boolean;
  x: number;
  y: number;
  sx: number;
  sy: number;
}

/** Corps au sol (corpseInfo). */
export interface CorpseInfo {
  x: number;
  y: number;
  sx: number;
  sy: number;
  items?: number;
}

// ---------------------------------------------------------------------------
// Waypoints (W05/W19)
// ---------------------------------------------------------------------------

export interface WaypointEntry {
  slot: number;
  name: string;
  act: number;
  activated: boolean;
  here?: boolean;
  travel?: boolean;
  mvp?: boolean;
  pos?: [number, number];
}

export interface WaypointState {
  current?: { key?: string; name?: string; levelId?: number; slot?: number; activated?: boolean };
  list: WaypointEntry[];
}

// ---------------------------------------------------------------------------
// AutoMap (9d/13b/21b)
// ---------------------------------------------------------------------------

export interface AutomapData {
  level: string;
  name?: string;
  mode: 'automap' | 'fallback' | string;
  lvlType?: number | string;
  cellW: number;
  cellH: number;
  sub?: number;
  coverage?: number;
  /** Traits précis 21b : segments [x1,y1,x2,y2] en px locaux (160×80). */
  glyphs?: Array<Array<[number, number, number, number]>>;
  /** Cellules [x, y, flags, cel, (glyph)] — flags : 1=sol, 4=special. */
  cells: number[][];
}

// ---------------------------------------------------------------------------
// Arbre de compétences (18a)
// ---------------------------------------------------------------------------

export interface SkillTreeNode {
  code: string;
  name: string;
  nameFr?: string;
  descFr?: string;
  level: number;
  maxLevel?: number;
  reqLevel?: number;
  prereqs?: string[];
  page: number;
  row?: number;
  column?: number;
  icon?: number;
  missile?: string | null;
  mana?: number;
  canLearn?: boolean;
  mvp?: boolean;
  dmgPhys?: [number, number];
  dmgElem?: [number, number];
  elem?: string;
  elemLen?: number;
  srcDam?: number;
  dmgPhysNext?: [number, number];
  dmgElemNext?: [number, number];
  synergies?: Array<{ code?: string; nameFr?: string; pctPerLvl?: number; pctTotal?: number }>;
}

export interface SkillTree {
  class: string;
  hero: string;
  points: number;
  charLevel: number;
  tabs?: string[];
  iconFile?: string;
  bgFile?: string;
  skills: SkillTreeNode[];
}

// ---------------------------------------------------------------------------
// Panneaux (18b — heroPanel/stash* ; endpoints optionnels dans cet arbre)
// ---------------------------------------------------------------------------

export interface HeroPanel {
  className?: string;
  level?: number;
  xp?: number;
  xpToNext?: number;
  strength?: number;
  dexterity?: number;
  vitality?: number;
  energy?: number;
  statPoints?: number;
  skillPoints?: number;
  life?: number;
  maxLife?: number;
  mana?: number;
  maxMana?: number;
  stamina?: number;
  maxStamina?: number;
  dmgMin?: number;
  dmgMax?: number;
  weapon?: string;
  defense?: number;
  toHit?: number;
  resFire?: number;
  resLight?: number;
  resCold?: number;
  resPoison?: number;
  resMax?: number;
  mf?: number;
  gf?: number;
  gold?: number;
  goldCap?: number;
}

export interface StashInfo {
  w?: number;
  h?: number;
  gold?: number;
  goldCap?: number;
  carryCap?: number;
  difficulty?: string;
  items?: Array<{ code?: string; name: string; x: number; y: number; w: number; h: number }>;
}

/** Ceinture 4×4 + buffs (beltInfo). */
export interface BeltInfo {
  belt?: Array<Array<{ code: string; name: string; kind: string } | null>>;
  buffs?: Array<{ state?: string; remainingMs?: number; stats?: Array<[number, number]> }>;
}

/** Résultat useConsumable/useBeltKey. */
export interface UseConsumableResult {
  use?: {
    code?: string;
    name: string;
    family?: string;
    heal?: number;
    mana?: number;
    buffMs?: number;
    townPortal?: boolean;
    identified?: boolean;
    chargesLeft?: number | null;
  };
  levelSwitch?: { key: string; name: string };
}

// ---------------------------------------------------------------------------
// Manifest + sauvegardes (slots)
// ---------------------------------------------------------------------------

export interface ManifestAnimDef {
  cof?: string;
  layers?: Record<string, string>;
  [k: string]: unknown;
}

export interface ManifestHeroDef {
  name?: string;
  class?: string;
  walk?: ManifestAnimDef;
  idle?: ManifestAnimDef;
  attack?: ManifestAnimDef;
  death?: ManifestAnimDef;
  modes?: Record<string, ManifestAnimDef | undefined>;
  [k: string]: unknown;
}

export interface ManifestLevelDef {
  name?: string;
  ds1?: string;
  palette?: string;
  dt1?: string[];
  sizeTiles?: [number, number];
  grid?: [number, number];
  warp?: { entry: [number, number] };
  [k: string]: unknown;
}

export interface ManifestSfxDef {
  uiClick?: string;
  attack?: string;
  hit?: string;
  die?: string;
  monsters?: Record<string, { attack?: string; hit?: string; die?: string } | undefined>;
  [k: string]: unknown;
}

export interface Manifest {
  level?: ManifestLevelDef;
  levels?: Record<string, ManifestLevelDef>;
  heroes?: Record<string, ManifestHeroDef>;
  hero?: ManifestHeroDef;
  monsters: Array<ManifestHeroDef & { code?: string }>;
  sfx: ManifestSfxDef;
  [k: string]: unknown;
}

/**
 * JSON de sauvegarde d2rules (v1..v3) — méta lues par D2Slots.metaOf
 * (identité héros, niveau, temps de jeu, zone).
 */
export interface SaveJson {
  version?: number;
  hero_code?: string;
  hero?: { stats?: { level?: number } };
  now_ms?: number;
  level?: string;
  collision?: number[][];
  [k: string]: unknown;
}

/** Fiche d'un slot de sauvegarde (d2wasm.slots.v1). */
export interface SaveSlot {
  id: string;
  name: string;
  ts: number;
  hero_code: string;
  hero_label: string;
  level: number;
  playtime_ms: number;
  level_key: string;
  zone: string;
  version?: number;
  save: string;
}

// ---------------------------------------------------------------------------
// Surface moteur D2Game consommée par le JS (m wasm + endpoints additifs)
// ---------------------------------------------------------------------------

export interface D2Game {
  automap(): string;
  beltAutoStore(): number;
  beltInfo(): string;
  composeStats(): Uint32Array;
  corpseInfo(): string;
  difficultyInfo(): string;
  finalize(): void;
  gridSize(): Uint32Array;
  groundItems(): string;
  heroClass(): string;
  heroScreen(): Float32Array;
  heroSkills(): string;
  heroState(): string;
  heroStats(): string;
  inventory(): string;
  learnSkill(code: string): string;
  levelInfo(): string;
  loadGame(json: string): void;
  mapPixelSize(): Uint32Array;
  mercInfo(): string;
  monsterCodes(): string;
  monsterInfo(index: number): string;
  monsterNear(wx: number, wy: number, radius: number): number;
  provide(path: string, bytes: Uint8Array): void;
  render(): Uint8Array;
  saveGame(): string;
  screenToWorld(px: number, py: number): Float32Array;
  selectSkill(index: number): boolean;
  setDifficulty(diff: string): void;
  setViewport(w: number, h: number): void;
  skillTree(): string;
  spriteNotes(): string;
  switchLevel(name: string, spawnX: number, spawnY: number): void;
  switchNotes(): string;
  talkNpc(index: number): string;
  tick(dtMs: number, inputJson?: string | null): string;
  travelWaypoint(slot: number): string;
  useBeltKey(key: number): string;
  useConsumable(slot: number): string;
  waypoints(): string;
  // endpoints additifs — absents du pkg wasm de certains arbres : gardés.
  isFinalized?(): boolean;
  npcs?(): string;
  exportD2s?(name: string): Uint8Array;
  importD2s?(bytes: Uint8Array): string;
  heroPanel?(): string;
  stashInfo?(): string;
  stashDeposit?(amount: number): string;
  stashWithdraw?(amount: number): string;
  cubeTransmute?(payload: string): string;
  cubeItemTooltip?(itemJson: string): string;
  skillIconsPng?(dc6: Uint8Array, pal: Uint8Array): Uint8Array;
  skillTreePanelPng?(dc6: Uint8Array, tab: number, pal: Uint8Array): Uint8Array;
}

// ---------------------------------------------------------------------------
// Entrées joueur (queue game.js) + état __d2
// ---------------------------------------------------------------------------

export type PlayerInput =
  | { type: 'move'; x: number; y: number }
  | { type: 'attack'; x: number; y: number }
  | { type: 'rangedAttack'; x: number; y: number }
  | { type: 'pickup' }
  | { type: 'setRun'; on: boolean }
  | { type: 'useItem'; slot: number }
  | { type: 'recoverCorpse' };

/** État global exposé par game.js (sondes E2E + file d'inputs). */
export interface D2State {
  mode: 'boot' | 'wasm' | 'fallback';
  errors: string[];
  events: number;
  level?: string;
  hero?: string;
  game?: D2Game;
  queue?: PlayerInput[];
  invCache?: Array<Item | null>;
  monCodes?: string[];
  refreshStatus?: () => void;
  goto?: (lvl: string) => void;
}

// ---------------------------------------------------------------------------
// Contrats window.* (modules UI)
// ---------------------------------------------------------------------------

export interface D2SlotsApi {
  MAX: number;
  list(): SaveSlot[];
  get(id: string): SaveSlot | null;
  remove(id: string): SaveSlot[];
  rename(id: string, name: string): SaveSlot | undefined;
  upsertFromSave(saveJson: string, opts?: { name?: string }): SaveSlot;
  mostRecent(): SaveSlot | null;
  metaOf(saveJson: string | SaveJson): SaveSlotMeta | null;
  exportSlot(id: string): SaveSlot;
  importSlot(text: string): SaveSlot;
}

export interface SaveSlotMeta {
  hero_code: string;
  hero_label: string;
  level: number;
  playtime_ms: number;
  level_key: string;
  zone: string;
  version?: number;
}

export interface D2HUDApi {
  ready: boolean;
  init(manifest: Manifest, game: D2Game): void;
  update(stats: HeroStats): void;
  events(events: GameEvent[]): void;
  frame(canvas: HTMLCanvasElement, game: D2Game): void;
  [k: string]: unknown;
}

export interface D2WaypointsOpts {
  getGame: () => D2Game | null;
  queue?: PlayerInput[];
  note?: (msg: string) => void;
  flash?: (msg: string) => void;
  manifest?: Manifest;
  sfx?: (key: string, vol?: number, rate?: number) => void;
  onTraveled?: (out: { slot: number; key: string; name: string }) => void;
}

export interface D2WaypointsApi {
  ready: boolean;
  open: boolean;
  init(opts: D2WaypointsOpts): D2WaypointsApi;
  refresh(jsonText: string): void;
  nameOf(slot: number): string;
  toggle(): void;
  setOpen(open: boolean): void;
  e2eTour(): Promise<WaypointState>;
  [k: string]: unknown;
}

export interface D2CorpseUIApi {
  init(): void;
  onDeath(info: { x: number; y: number; gold?: number } | null): void;
  onRespawned(): void;
  onRecovered(): void;
  frame(canvas: HTMLCanvasElement, game: D2Game): void;
}

export interface D2MusicApi {
  setLevel(key: string | null, name: string | null): void;
  setMuted(m: boolean): void;
  toggle(): void;
  theme(): string;
  isMuted(): boolean;
}

export interface D2AudioApi {
  pan(xScreen: number, refWidth?: number): number;
  play(name: string, xScreen?: number): Promise<AudioBufferSourceNode | null>;
  setAmbience(stem: string): void;
  setNight(n: boolean | null): void;
  ambianceFor(key: string | null, name: string | null): string | null;
  setVolume(kind: 'music' | 'amb' | 'sfx', v: number): number;
  volume(kind: 'music' | 'amb' | 'sfx'): number;
  panValue(): number;
  ambient(): string | null;
}

export interface D2MonSfxApi {
  feed(ev: GameEvent): void;
  setMuted(m: boolean): void;
  familyOf(code: string): string | null;
  isEnabled(): boolean;
}

export interface D2SFXApi {
  play(file: string | null, vol?: number, rate?: number): boolean;
  handleEvent(ev: GameEvent): void;
  missileCastSfx(skill: string | null): string | null;
  shrine(): boolean;
  chest(): boolean;
  portalOpen(): boolean;
  unlock(): void;
  isMuted(): boolean;
  toggleMute(): boolean;
  wired(): boolean;
}

export interface D2SkillsApi {
  ready: boolean;
  open: boolean;
  init(opts: { getGame: () => D2Game | null; flash?: (m: string) => void; note?: (m: string) => void }): D2SkillsApi;
  refresh(): void;
  toggle(): void;
  [k: string]: unknown;
}

export interface D2MercApi {
  ready: boolean;
  init(opts: { getGame: () => D2Game | null }): D2MercApi;
  frame(game: D2Game): void;
  [k: string]: unknown;
}

export interface D2sIOApi {
  init(opts: { getGame: () => D2Game | null; note?: (m: string) => void; flash?: (m: string) => void; onImported?: (report: D2sImportReport) => void }): void;
  exportD2s(): void;
  importD2s(file: File): void;
}

export interface D2sImportReport {
  classChanged?: boolean;
  class?: string;
  name?: string;
  level?: number;
  items_loaded?: number;
  waypoints?: number;
}

export interface D2StashApi {
  init(dom?: { toolbar?: HTMLElement; log?: (m: string) => void }, hooks?: D2StashHooks): void;
  toggle(): void;
  depositGold(n: number): { done: number; left: number };
  withdrawGold(n: number): { done: number; left: number };
  depositItem(name: string): { x: number; y: number } | null;
  state(): { gold: number; cells: Array<{ name: string } | null> };
}

export interface D2StashHooks {
  gold(): number;
  level(): number;
  diff(): 'normal' | 'nightmare' | 'hell' | string;
  spend(n: number): boolean;
  earn(n: number): void;
}

export interface D2PanelsApi {
  toggle(id: 'char' | 'inv' | 'stash'): void;
  open(id: 'char' | 'inv' | 'stash'): void;
  refresh(): void;
  state: Record<string, unknown>;
}

export interface D2CubeUIApi {
  open(): void;
  close(): void;
  toggle(): void;
}

export interface D2AutomapApi {
  open: boolean;
  toggle(): void;
  show(): void;
  hide(): void;
  [k: string]: unknown;
}

export interface D2VendorUnit {
  code: string;
  name: string;
  kind: string;
  quality: string;
  buy: number;
  sell: number;
  permanent: boolean;
  gamble?: boolean;
}

export interface D2VendorApi {
  init(dom?: { toolbar?: HTMLElement; root?: HTMLElement; log?: (m: string) => void }, hooks?: D2VendorHooks): { panel: HTMLElement; render(): void } | undefined;
  generate(id: string, vendorLevel: number, seed: number): { id: string; name: string; level: number; gold: number; stock: D2VendorUnit[] };
  gambleList(vendorLevel: number, clvl: number, seed: number): D2VendorUnit[];
  _state: Record<string, unknown>;
}

export interface D2VendorHooks {
  getGold(): number;
  spend(n: number): boolean;
  earn(n: number): void;
  inventory(): Array<Item>;
  heroLevel(): number;
  simulated?: boolean;
}

export interface QuestReward {
  Item?: { code: string };
  Unlock?: { flag: string };
}

export interface D2QuestsApi {
  init(dom?: Document): D2QuestsApi;
  feed(event: unknown): QuestReward[];
  log: unknown;
  reset(): void;
  exportLog(): string;
  importLog(json: string): boolean;
  show(): void;
  hide(): void;
  toggle(): void;
  isOpen: boolean;
}

export interface D2Quests45Api {
  init(dom?: Document): D2Quests45Api;
  feed(event: unknown): Array<{ id: string; phase: string; final?: boolean }>;
  log: unknown;
  reset(): void;
  exportLog(): string;
  importLog(json: string): boolean;
  show(): void;
  hide(): void;
  toggle(): void;
  isOpen: boolean;
}

export interface D2NpcApi {
  init(opts: Record<string, unknown>): void;
  frame(canvas: HTMLCanvasElement, game: D2Game): void;
  clickAt(px: number, py: number): boolean;
  [k: string]: unknown;
}

/** window complet : contrats Legacy + migrés + état __d2. */
export interface D2Window extends Window {
  __d2?: D2State;
  D2Slots?: D2SlotsApi;
  D2HUD?: D2HUDApi;
  D2Waypoints?: D2WaypointsApi;
  D2CorpseUI?: D2CorpseUIApi;
  D2Music?: D2MusicApi;
  D2Audio?: D2AudioApi;
  D2MonSfx?: D2MonSfxApi;
  D2SFX?: D2SFXApi;
  D2Skills?: D2SkillsApi;
  D2Merc?: D2MercApi;
  D2sIO?: D2sIOApi;
  D2Stash?: D2StashApi;
  D2Panels?: D2PanelsApi;
  D2CubeUI?: D2CubeUIApi;
  D2Automap?: D2AutomapApi;
  D2Vendor?: D2VendorApi;
  D2Quests?: D2QuestsApi;
  D2Quests45?: D2Quests45Api;
  D2Npc?: D2NpcApi;
  webkitAudioContext?: typeof AudioContext;
}

/** Accès typé à window dans les modules migrés. */
export function win(): D2Window | null {
  return typeof window === 'undefined' ? null : (window as D2Window);
}

/** JSON.parse avec repli (garde-fou : jamais d'exception UI). */
export function parseOr<T>(text: string | null | undefined, fallback: T): T {
  try {
    const v = JSON.parse(text ?? '') as T;
    return v ?? fallback;
  } catch {
    return fallback;
  }
}

// ---------------------------------------------------------------------------
// Registre 25a (sonde window.__d2Ts) : voie TS pilote vs Legacy adopté.
// ---------------------------------------------------------------------------
export const TS25A_REGISTRY: { installed: string[]; adopted: string[] } = {
  installed: [],
  adopted: [],
};

export function registerModule(name: string, mode: 'ts' | 'adopt'): void {
  (mode === 'ts' ? TS25A_REGISTRY.installed : TS25A_REGISTRY.adopted).push(name);
}
