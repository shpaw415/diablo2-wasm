/* tslint:disable */
/* eslint-disable */

export class D2Game {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Collision grid as 0/1 bytes (row-major), for d2rules/debug overlays.
     */
    collisionGrid(): Uint8Array;
    /**
     * Compositor stats — surfaced in the demo status line.
     */
    composeStats(): Uint32Array;
    /**
     * Compose the map + build sprites + spawn the world. Call once.
     */
    finalize(): void;
    gridSize(): Uint32Array;
    /**
     * Resolved hero class as JSON `{"code":"BA","name":"Barbare"}` — HUD
     * display + E2E probe. Valid before `finalize` too.
     */
    heroClass(): string;
    /**
     * Hero feet position in viewport px — for the JS click ring / cursor.
     */
    heroScreen(): Float32Array;
    /**
     * Hero animation state as JSON `{"anim":"RN","frame":3,"facing":6,
     * "run":true,"moving":false}` — HUD run flag + E2E probes.
     */
    heroState(): string;
    /**
     * Hero HUD stats (`HeroStats`) as a JSON object string — level, life,
     * mana, xp, gold… polled by the demo HUD each frame.
     */
    heroStats(): string;
    /**
     * Inventory (40 slots, `null` = empty) as a JSON array string.
     */
    inventory(): string;
    /**
     * Restore a world from `saveGame` JSON. Throws (`JsValue`) on invalid
     * save or a save from another map. Map/sprites are untouched.
     */
    loadGame(json: string): void;
    mapPixelSize(): Uint32Array;
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
     */
    constructor(manifest_json: string, hero: string);
    /**
     * Feed one asset blob (manifest path + bytes), e.g. fetched files.
     */
    provide(path: string, bytes: Uint8Array): void;
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
    setViewport(w: number, h: number): void;
    /**
     * Animation modes listed in the manifest but skipped (missing files),
     * as a JSON array of strings — the page logs them, the game still runs.
     */
    spriteNotes(): string;
    /**
     * Advance the simulation. `input_json` (nullable) mirrors the
     * `PlayerInput` contract:
     * `{"type":"move","x":123,"y":45}` · `{"type":"attack","x":..,"y":..}`
     * · `{"type":"pickup"}` · `{"type":"setRun","on":true}`
     * · `{"type":"useItem","slot":0}`.
     * Returns the tick's `GameEvent`s as a JSON array string.
     */
    tick(dt_ms: number, input_json?: string | null): string;
    viewportHeight(): number;
    viewportWidth(): number;
}

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_d2game_free: (a: number, b: number) => void;
    readonly d2game_collisionGrid: (a: number) => [number, number];
    readonly d2game_composeStats: (a: number) => [number, number];
    readonly d2game_finalize: (a: number) => [number, number];
    readonly d2game_gridSize: (a: number) => [number, number];
    readonly d2game_heroClass: (a: number) => [number, number];
    readonly d2game_heroScreen: (a: number) => [number, number];
    readonly d2game_heroState: (a: number) => [number, number];
    readonly d2game_heroStats: (a: number) => [number, number];
    readonly d2game_inventory: (a: number) => [number, number];
    readonly d2game_loadGame: (a: number, b: number, c: number) => [number, number];
    readonly d2game_mapPixelSize: (a: number) => [number, number];
    readonly d2game_monsterNear: (a: number, b: number, c: number, d: number) => number;
    readonly d2game_new: (a: number, b: number, c: number, d: number) => [number, number, number];
    readonly d2game_provide: (a: number, b: number, c: number, d: number, e: number) => void;
    readonly d2game_render: (a: number) => [number, number];
    readonly d2game_saveGame: (a: number) => [number, number];
    readonly d2game_screenToWorld: (a: number, b: number, c: number) => [number, number];
    readonly d2game_setViewport: (a: number, b: number, c: number) => void;
    readonly d2game_spriteNotes: (a: number) => [number, number];
    readonly d2game_tick: (a: number, b: number, c: number, d: number) => [number, number];
    readonly d2game_viewportHeight: (a: number) => number;
    readonly d2game_viewportWidth: (a: number) => number;
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
