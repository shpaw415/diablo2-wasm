/* tslint:disable */
/* eslint-disable */

/**
 * A Diablo 2 MPQ archive opened from raw bytes (resident in memory).
 */
export class WasmMpq {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * True if the name hashes to an existing entry (`/` and `\` equivalent,
     * case-insensitive through the hash).
     */
    has(name: string): boolean;
    /**
     * Open an archive; throws a JS `Error`-style string if it is not a
     * readable MPQ v0/v1.
     */
    constructor(data: Uint8Array);
    /**
     * Read a whole entry, or `undefined` if absent/corrupt.
     */
    read(name: string): Uint8Array | undefined;
}

/**
 * Decode a DC6 file into a JS array of frame objects:
 * `{width, height, offsetX, offsetY, flip, indices: Uint8Array}` where
 * `indices` is `width*height` palette indices in top-down row-major order
 * (0 = transparent). Throws a descriptive string on malformed input.
 */
export function decode_dc6(data: Uint8Array): any;

/**
 * Expand palette indices into RGBA8 with a 768-byte palette (256×RGB;
 * a 767-byte D2 palette is zero-padded first, as in the reference tools).
 * Index 0 → alpha 0; other indices → alpha 255.
 */
export function indices_to_rgba(indices: Uint8Array, palette: Uint8Array): Uint8Array;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_wasmmpq_free: (a: number, b: number) => void;
    readonly decode_dc6: (a: number, b: number) => [number, number, number];
    readonly indices_to_rgba: (a: number, b: number, c: number, d: number) => [number, number];
    readonly wasmmpq_has: (a: number, b: number, c: number) => number;
    readonly wasmmpq_new: (a: number, b: number) => [number, number, number];
    readonly wasmmpq_read: (a: number, b: number, c: number) => [number, number];
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
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
