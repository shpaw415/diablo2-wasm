/* tslint:disable */
/* eslint-disable */

export class D2Game {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * 3b : fiche de l'allié sous le pointeur (px viewport) — JSON
     * `{slot,code,name,hp,maxHp,level,x,y,sx,sy}` ou `null`. La barre de
     * vie elle-même est dessinée par le moteur dans le framebuffer.
     */
    allyInfoAt(): string;
    /**
     * Mini-carte du niveau courant (9d) — JSON
     * `{level,name,mode,lvlType,cellW,cellH,sub,cells:[[x,y,flags,cel],…]}`
     * (voir `automap.rs` : mode `automap` / fallback polygone marchable).
     */
    automap(): string;
    /**
     * « Auto-ranger » : déplace les potions `autobelt=1` (hp1-5, mp1-5,
     * rvs, rvl) de l'inventaire vers la ceinture 4×4 — nombre rangé
     * (idempotent ; la page l'appelle après un ramassage).
     */
    beltAutoStore(): number;
    /**
     * État ceinture + buffs (lecture HUD) — JSON
     * `{"belt":[[4×4 cellules null|{code,name,kind}]],"buffs":[{"state",
     * "remainingMs","stats":[[id,val],…]},…]}` ; `{}` avant `finalize`.
     */
    beltInfo(): string;
    /**
     * 3b : slot suivi par la caméra (lecture — E2E/probe).
     */
    cameraSlot(): number;
    /**
     * Choix de l'option `index` (INDEX BRUT dans `node.options`) par le
     * TENEUR `slot` — JSON session à jour (nœud avancé) ou action produite
     * (`{"kind":"open_shop","vendor":…}`, `{"kind":"close"}`…), `null` si
     * refusé (verrou autre slot / condition quête / index).
     */
    chooseDialogueSlot(slot: number, index: number): string;
    /**
     * 3b : efface le survol (pointeur sorti du canvas).
     */
    clearPointer(): void;
    /**
     * Referme la session si `slot` en est le teneur (« partir » / Échap) —
     * `false` sinon (session inchangée). Le tick du monde reprend pour tous.
     */
    closeDialogueSlot(slot: number): boolean;
    /**
     * Collision grid as 0/1 bytes (row-major), for d2rules/debug overlays.
     */
    collisionGrid(): Uint8Array;
    /**
     * Compositor stats — surfaced in the demo status line.
     */
    composeStats(): Uint32Array;
    /**
     * Corps au sol (J1) — JSON `{"x","y","sx","sy","items"}` ou `null`.
     * Pollé par corpse-ui.js (flare + hint) et le clic « récupérer ».
     */
    corpseInfo(): string;
    /**
     * VERROU de dialogue (3c-b) — JSON `{"active":true,"npc","npcName",
     * "slot","player","playerName"}` quand `slot` discute avec le NPC :
     * les AUTRES clients affichent « <NPC> discute avec <joueur> » et
     * bloquent l'ouverture ; `{"active":false}` sinon.
     */
    dialogueLock(): string;
    /**
     * Session de dialogue courante (JSON, + `visible` + teneur) — `null`
     * sans dialogue. Poll de secours/refresh de l'UI.
     */
    dialogueSession(): string;
    /**
     * Current difficulty as JSON `{"difficulty":"hell","label":"Hell"}` —
     * HUD badge + E2E probe.
     */
    difficultyInfo(): string;
    /**
     * Compose the map + build sprites + spawn the world. Call once.
     */
    finalize(): void;
    gridSize(): Uint32Array;
    /**
     * Objets au sol projetés (EXP-H8, labels Alt) — JSON array
     * `[{index,code,name,kind,amount,x,y,sx,sy},…]`.
     */
    groundItems(): string;
    /**
     * Resolved hero class as JSON `{"code":"BA","name":"Barbare"}` — HUD
     * display + E2E probe. Valid before `finalize` too.
     */
    heroClass(): string;
    /**
     * 3b : position écran (pieds) du héros `slot` — `[x, y]` px viewport
     * (`NaN` hors bornes).
     */
    heroScreenSlot(slot: number): Float32Array;
    /**
     * Hero feet position in viewport px — for the JS click ring / cursor.
     */
    heroScreen(): Float32Array;
    /**
     * 3b : compétences du héros `slot` — même JSON que `heroSkills`.
     */
    heroSkillsSlot(slot: number): string;
    /**
     * Compétences connues du héros (EXP-H4) — JSON array
     * `[{index,code,name,level,missile},…]` pour les boutons L/R de hud.js.
     */
    heroSkills(): string;
    /**
     * 3b : état d'animation du héros `slot` — même JSON que `heroState`
     * (+ `slot`/`x`/`y`). `{}` hors bornes.
     */
    heroStateSlot(slot: number): string;
    /**
     * Hero animation state as JSON `{"anim":"RN","frame":3,"facing":6,
     * "run":true,"moving":false}` — HUD run flag + E2E probes.
     */
    heroState(): string;
    /**
     * 3b : stats HUD du héros `slot` — même JSON que `heroStats`.
     */
    heroStatsSlot(slot: number): string;
    /**
     * Hero HUD stats (`HeroStats`) as a JSON object string — level, life,
     * mana, xp, gold… polled by the demo HUD each frame.
     */
    heroStats(): string;
    /**
     * 3b : vue d'ensemble des héros (positions projetées + anims) — JSON
     * `[{slot,code,name,anim,frame,facing,x,y,sx,sy},…]` (E2E coop, UI salon).
     */
    heroesView(): string;
    /**
     * 3b : codes de héros par slot — JSON
     * `[{"slot":0,"code":"BA","name":"Barbare"},…]`.
     */
    heroes(): string;
    /**
     * 3b : inventaire du héros `slot` — même JSON que `inventory`.
     */
    inventorySlot(slot: number): string;
    /**
     * Inventory (40 slots, `null` = empty) as a JSON array string.
     */
    inventory(): string;
    /**
     * Apprend/monte le skill `code` (dépense 1 point) — JSON
     * `{"ok":true,"level":2,"points":1}` ou `{"ok":false,"error":"…"}`.
     */
    learnSkill(code: string): string;
    /**
     * Current level as JSON `{"key":"bloodmoor","name":"Blood Moor (Acte I)"}`
     * — HUD zone name + level-switch banner. `{}` if the key is unknown.
     */
    levelInfo(): string;
    /**
     * Restore a world from `saveGame` JSON. Throws (`JsValue`) on invalid
     * save or a save from another map. Map/sprites are untouched.
     */
    loadGame(json: string): void;
    mapPixelSize(): Uint32Array;
    /**
     * Fiche du mercenaire (H) — JSON
     * `{kind,subtype,level,hp,maxHp,alive,x,y,sx,sy}` ou `null` (aucun
     * mercenaire embauché). Pollé chaque frame par merc-ui.js.
     */
    mercInfo(): string;
    /**
     * Monster codes for the current level, parallel to the tick's monster
     * indices — debug/E2E. JSON `["FA","ZM",…]`.
     */
    monsterCodes(): string;
    /**
     * Fiche de survol du monstre `index` (EXP-H6) — JSON
     * `{code,name,kind,alive,hp,maxHp,x,y,sx,sy}` ou `null`.
     */
    monsterInfo(index: number): string;
    /**
     * Alive monster index nearest to a world point within `radius`
     * sub-tiles, or -1 (JS click router: attack vs move).
     */
    monsterNear(wx: number, wy: number, radius: number): number;
    /**
     * `manifest_json` — the raw `assets/manifest.json` text. `hero` — the
     * selected class code (`"BA"`, `"SOR"`, `"PA"`…, case-insensitive, default
     * `"BA"`): the hero's COF/DCC layers AND stats both come from
     * `manifest.heroes[code]` when present, else the legacy `hero` block.
     * Initial level: `"town"`.
     */
    constructor(manifest_json: string, hero: string);
    /**
     * NPC de ville (W16, villes vivantes) — JSON array
     * `[{index,id,code,name,questCode,role,x,y,vendor,assumed,talked},…]`
     * (coordonnées cellules ; projection écran ancrée sur `heroScreen()`).
     * `[]` hors camp. Contrat UI : demo/npc-ui.js (`npc_bridge.rs`).
     */
    npcs(): string;
    /**
     * Ouvre le dialogue du NPC `index` pour le joueur `slot` — JSON
     * `DialogueSession` + `visible` (indices bruts des options visibles) +
     * `player`/`playerName` (teneur), ou `Err` descriptif (« X discute avec
     * le joueur N — patientez » sous verrou, « trop loin — … », « … mort »).
     * L'ouverture GÈLE le monde pour TOUS les slots (décision style retail,
     * cf. d2rules/dialogue.rs § coop).
     */
    openDialogueSlot(slot: number, index: number): string;
    /**
     * État du groupe (JSON) — `[{slot, code, class, level, life, maxLife,
     * mana, maxMana, dead, gold}, …]` (ordre des slots). Pollé par le
     * panneau latéral (visible en coop uniquement) ; compaction des slots
     * (removePlayer 3c-c) suivie automatiquement.
     */
    partyView(): string;
    /**
     * Feed one asset blob (manifest path + bytes), e.g. fetched files.
     * Accepted any time — including between level switches (hot provide).
     */
    provide(path: string, bytes: Uint8Array): void;
    /**
     * 3b (coop lockstep, file PAR SLOT) : empile l'entrée `input_json`
     * (même contrat que `tick`) pour le héros `slot` — la page empile tout
     * le bundle courant, puis UN `tickQueued` consomme tout (slots
     * croissants, FIFO par slot — déterministe). Sans effet de bord tant
     * que `tickQueued` n'est pas appelé.
     */
    queueInput(slot: number, input_json?: string | null): void;
    /**
     * 3b : entrées empilées non consommées (tous slots) — diagnostic/E2E.
     */
    queuedInputs(): number;
    /**
     * 3c (coop lockstep) : demande le DÉPART du héros `slot` — mise en
     * file (relais : entrée synthétique `{type:"leave"}` du joueur qui
     * se déconnecte), appliquée au prochain tick par `World::remove_player`
     * (compaction déterministe des slots — tous les clients lockstep
     * retirent le même slot au même tick, mondes identiques bit-exact).
     * Événement `HeroLeft` au tick d'application ; sans effet (identique
     * chez tous) si le slot est hors monde.
     */
    removePlayer(slot: number): void;
    /**
     * RGBA framebuffer (`viewport_width * viewport_height * 4`) for
     * `putImageData` — floor, shadows, entities already composited.
     */
    render(): Uint8Array;
    /**
     * Serialize the world (JSON v1) — demo Save button → localStorage.
     */
    saveGame(): string;
    /**
     * Click (viewport px) → world sub-tile coords `[x, y]`.
     */
    screenToWorld(px: number, py: number): Float32Array;
    /**
     * 3b : sélection du skill actif du héros `slot` — false si hors bornes.
     */
    selectSkillSlot(slot: number, index: number): boolean;
    /**
     * Sélectionne la compétence active (main droite D2, EXP-H4) — l'input
     * `RangedAttack` consomme ce skill. `false` si hors bornes.
     */
    selectSkill(index: number): boolean;
    /**
     * 3b : verrouille la caméra (et `heroScreen()`) sur le héros `slot`.
     */
    setCameraSlot(slot: number): void;
    /**
     * Set the difficulty (`"normal"`, `"nm"`, `"nightmare"`, `"hell"` —
     * case-insensitive; unknown = normal). Call BEFORE `finalize` so the
     * boot level spawns difficulty-scaled monsters (MonLvl (N)/(H), MonDen/
     * MonUMin/Max (N)/(H), Res*(N)/(H) immunities). W08/K impl-difficultes.
     */
    setDifficulty(diff: string): void;
    /**
     * 3b : graine de partie (mêmes cartes chez tous les clients du salon —
     * l'hôte publie la seed du salon via `/meta`).
     */
    setGameSeed(seed: bigint): void;
    /**
     * 3b : survol page (px viewport) — arme la barre de vie allié du rendu
     * et la cible d'`allyInfoAt`. Présentation seule (jamais dans la save).
     */
    setPointer(x: number, y: number): void;
    setViewport(w: number, h: number): void;
    /**
     * Arbre complet de la classe du héros — JSON
     * `{class, hero, points, charLevel, skills:[{code,name,nameKey,level,
     * maxLevel,reqLevel,prereqs,page,row,column,icon,missile,mana,canLearn,
     * mvp},…]}` (30 skills/classe, tables réelles v1.12 embarquées).
     */
    skillTree(): string;
    /**
     * Dépense `points` points de caractéristique (`"str"|"dex"|"vit"|"ene"`)
     * du héros (P26-1, panneau perso) — formules réelles charstats. JSON
     * `{"ok":true,"stat":"vit","value":30,"points":2,"lifeGain":20,…}` ou
     * `{"ok":false,"error":"…"}`.
     */
    spendStat(stat: string, points: number): string;
    /**
     * Animation modes listed in the manifest but skipped (missing files),
     * as a JSON array of strings — the page logs them, the game still runs.
     */
    spriteNotes(): string;
    /**
     * Switch level: reloads DS1/DT1/collision from the provided (or hot-
     * provided) bytes, carries the hero over (stats/inventory/xp), spawns the
     * target level's monsters, and lands the hero at `(spawn_x, spawn_y)`
     * (snapped to the nearest walkable sub-tile). Throws on unknown level or
     * missing assets.
     */
    switchLevel(name: string, spawn_x: number, spawn_y: number): void;
    /**
     * Level-switch notes (degraded hero transfers…) as a JSON array.
     */
    switchNotes(): string;
    /**
     * Clic NPC (W16) : interaction si le héros est à portée (2.5 cellules) —
     * JSON `NpcInteraction` `{name,role,dialogue,actions,healed,quest_rewards}`
     * ou `Err` descriptif (`"trop loin — approchez-vous du NPC"`). Émet
     * `NpcTalked` (flux de tick) et avance le journal de quêtes branché
     * (q3/q4).
     */
    talkNpc(index: number): string;
    /**
     * 3b (coop lockstep, file PAR SLOT) : un pas de simulation consommant
     * TOUTES les files (`queueInput`), slots croissants / FIFO par slot,
     * avec UN SEUL dt. Chaque client passe le même bundle ⇒ le même tableau
     * `(slot, entrée)` chez tous (déterminisme lockstep). Retour : mêmes
     * événements JSON que `tick`.
     */
    tickQueued(dt_ms: number): string;
    /**
     * 3b (coop lockstep) : un pas de simulation avec l'entrée `input_json`
     * (même contrat que `tick`) routée au héros `slot` — le relais diffuse
     * les bundles PAR SLOT, la page exécute chaque entrée sur SON héros.
     * Retour : mêmes événements JSON que `tick`.
     */
    tickSlot(dt_ms: number, slot: number, input_json?: string | null): string;
    /**
     * Advance the simulation. `input_json` (nullable) mirrors the
     * `PlayerInput` contract:
     * `{"type":"move","x":123,"y":45}` · `{"type":"attack","x":..,"y":..}`
     * · `{"type":"pickup"}` · `{"type":"setRun","on":true}`
     * · `{"type":"useItem","slot":0}` · `{"type":"recoverCorpse"}` (J1).
     * Returns the tick's events as a JSON array string: the d2rules contract
     * events ({"HeroMoved":…}…) plus the engine's multi-level extras
     * `{"LevelSwitch":{"key":…,"name":…}}` / `{"LevelSwitchFailed":"…"}` —
     * entering a warp zone switches the level INSIDE the tick, so the very
     * next frame already renders the new level.
     */
    tick(dt_ms: number, input_json?: string | null): string;
    /**
     * Direct travel to an ACTIVATED waypoint (W05): reloads the target
     * level (seeded regen, cf. impl-drlg) and lands the hero on the target
     * stone. Returns the transition JSON `{"slot","key","name"}`; throws
     * on unknown slot, non-activated waypoint or level missing from the
     * port's manifest.
     */
    travelWaypoint(slot: number): string;
    /**
     * Touche de ceinture `key` (1..=4) — boit le premier objet de la
     * colonne (rangée 0 d'abord), mêmes effets/JSON que `useConsumable`.
     */
    useBeltKey(key: number): string;
    /**
     * Utilise le consommable du slot d'inventaire `slot` (effets réels par
     * code — potions hp1-5/mp1-5/rejuv/buffs 30 s, parchemin de porte de
     * ville, tomes à charges) — JSON
     * `{"use":{code,name,family,heal,mana,buffMs,townPortal,identified,
     * chargesLeft},"levelSwitch":{key,name}?}` ; un parchemin/tome de porte
     * de ville BASCULE vers le camp (switch_level existant). `Err` lisible
     * (`"slot vide"`, `"tome vide"`, `"rien à identifier"`…).
     */
    useConsumable(slot: number): string;
    viewportHeight(): number;
    viewportWidth(): number;
    /**
     * Waypoints (W05) — full panel state as JSON:
     * `{"current":{"key","name","levelId","slot","activated"},"list":[…]}`
     * (see `waypoint_travel.rs` / `d2rules::waypoints` for the shape).
     */
    waypoints(): string;
    /**
     * 3b (coop multi-héros) : comme [`D2Game::withLevel`] mais avec UN héros
     * PAR SLOT — `heroes_json` = tableau JSON de codes de classe
     * (`'["BA","SOR"]'`, index = slot, 1..=8, slot 0 = héros historique).
     * Chaque slot reçoit son personnage complet (d2rules) et son set de
     * sprites "heroN" (d2game) ; la page verrouille la caméra sur SON slot
     * (`setCameraSlot`) et route ses entrées via `tickSlot`.
     */
    static withLevelCoop(manifest_json: string, heroes_json: string, level: string): D2Game;
    /**
     * Like [`D2Game::new`] with the initial level key from the manifest's
     * `levels` block (`"town"`, `"bloodmoor"`…, case-insensitive).
     */
    static withLevel(manifest_json: string, hero: string, level: string): D2Game;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_d2game_free: (a: number, b: number) => void;
    readonly d2game_allyInfoAt: (a: number) => [number, number];
    readonly d2game_automap: (a: number) => [number, number];
    readonly d2game_beltAutoStore: (a: number) => number;
    readonly d2game_beltInfo: (a: number) => [number, number];
    readonly d2game_cameraSlot: (a: number) => number;
    readonly d2game_chooseDialogueSlot: (a: number, b: number, c: number) => [number, number, number, number];
    readonly d2game_clearPointer: (a: number) => void;
    readonly d2game_closeDialogueSlot: (a: number, b: number) => number;
    readonly d2game_collisionGrid: (a: number) => [number, number];
    readonly d2game_composeStats: (a: number) => [number, number];
    readonly d2game_corpseInfo: (a: number) => [number, number];
    readonly d2game_dialogueLock: (a: number) => [number, number];
    readonly d2game_dialogueSession: (a: number) => [number, number];
    readonly d2game_difficultyInfo: (a: number) => [number, number];
    readonly d2game_finalize: (a: number) => [number, number];
    readonly d2game_gridSize: (a: number) => [number, number];
    readonly d2game_groundItems: (a: number) => [number, number];
    readonly d2game_heroClass: (a: number) => [number, number];
    readonly d2game_heroScreen: (a: number) => [number, number];
    readonly d2game_heroScreenSlot: (a: number, b: number) => [number, number];
    readonly d2game_heroSkills: (a: number) => [number, number];
    readonly d2game_heroSkillsSlot: (a: number, b: number) => [number, number];
    readonly d2game_heroState: (a: number) => [number, number];
    readonly d2game_heroStateSlot: (a: number, b: number) => [number, number];
    readonly d2game_heroStats: (a: number) => [number, number];
    readonly d2game_heroStatsSlot: (a: number, b: number) => [number, number];
    readonly d2game_heroes: (a: number) => [number, number];
    readonly d2game_heroesView: (a: number) => [number, number];
    readonly d2game_inventory: (a: number) => [number, number];
    readonly d2game_inventorySlot: (a: number, b: number) => [number, number];
    readonly d2game_learnSkill: (a: number, b: number, c: number) => [number, number];
    readonly d2game_levelInfo: (a: number) => [number, number];
    readonly d2game_loadGame: (a: number, b: number, c: number) => [number, number];
    readonly d2game_mapPixelSize: (a: number) => [number, number];
    readonly d2game_mercInfo: (a: number) => [number, number];
    readonly d2game_monsterCodes: (a: number) => [number, number];
    readonly d2game_monsterInfo: (a: number, b: number) => [number, number];
    readonly d2game_monsterNear: (a: number, b: number, c: number, d: number) => number;
    readonly d2game_new: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly d2game_npcs: (a: number) => [number, number];
    readonly d2game_openDialogueSlot: (a: number, b: number, c: number) => [number, number, number, number];
    readonly d2game_partyView: (a: number) => [number, number];
    readonly d2game_provide: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly d2game_queueInput: (a: number, b: number, c: number, d: number) => void;
    readonly d2game_queuedInputs: (a: number) => number;
    readonly d2game_removePlayer: (a: number, b: number) => void;
    readonly d2game_render: (a: number) => [number, number];
    readonly d2game_saveGame: (a: number) => [number, number];
    readonly d2game_screenToWorld: (a: number, b: number, c: number) => [number, number];
    readonly d2game_selectSkill: (a: number, b: number) => number;
    readonly d2game_selectSkillSlot: (a: number, b: number, c: number) => number;
    readonly d2game_setCameraSlot: (a: number, b: number) => void;
    readonly d2game_setDifficulty: (a: number, b: number, c: number) => void;
    readonly d2game_setGameSeed: (a: number, b: bigint) => void;
    readonly d2game_setPointer: (a: number, b: number, c: number) => void;
    readonly d2game_setViewport: (a: number, b: number, c: number) => void;
    readonly d2game_skillTree: (a: number) => [number, number];
    readonly d2game_spendStat: (a: number, b: number, c: number, d: number) => [number, number];
    readonly d2game_spriteNotes: (a: number) => [number, number];
    readonly d2game_switchLevel: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly d2game_switchNotes: (a: number) => [number, number];
    readonly d2game_talkNpc: (a: number, b: number) => [number, number, number, number];
    readonly d2game_tick: (a: number, b: number, c: number, d: number) => [number, number];
    readonly d2game_tickQueued: (a: number, b: number) => [number, number];
    readonly d2game_tickSlot: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly d2game_travelWaypoint: (a: number, b: number) => [number, number, number, number];
    readonly d2game_useBeltKey: (a: number, b: number) => [number, number, number, number];
    readonly d2game_useConsumable: (a: number, b: number) => [number, number, number, number];
    readonly d2game_viewportHeight: (a: number) => number;
    readonly d2game_viewportWidth: (a: number) => number;
    readonly d2game_waypoints: (a: number) => [number, number];
    readonly d2game_withLevel: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly d2game_withLevelCoop: (a: number, b: number, c: number, d: number, e: number, f: number) => [number, number, number];
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
