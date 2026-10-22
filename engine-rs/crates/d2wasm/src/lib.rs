//! # d2wasm
//!
//! wasm-bindgen bindings over [`d2mpq`] (read-only MPQ) and [`d2dc6`] (DC6
//! sprites) for the browser demo. Everything is resolved in memory: the page
//! fetches the bytes and hands them over — no file IO anywhere.
//!
//! * [`WasmMpq`] — open a user-provided `.mpq`, probe names, read entries.
//! * [`decode_dc6`] — DC6 bytes → array of `{width,height,offsetX,offsetY,flip,indices}`.
//! * [`indices_to_rgba`] — palette indices (256×RGB palette, 768 bytes) → RGBA8,
//!   index 0 transparent.

use js_sys::{Array, Object, Reflect, Uint8Array};
use wasm_bindgen::prelude::*;

fn err_js(e: d2mpq::D2Error) -> JsValue {
    JsValue::from_str(&e.to_string())
}

/// A Diablo 2 MPQ archive opened from raw bytes (resident in memory).
#[wasm_bindgen]
pub struct WasmMpq {
    archive: d2mpq::Archive,
}

#[wasm_bindgen]
impl WasmMpq {
    /// Open an archive; throws a JS `Error`-style string if it is not a
    /// readable MPQ v0/v1.
    #[wasm_bindgen(constructor)]
    pub fn new(data: Vec<u8>) -> Result<WasmMpq, JsValue> {
        d2mpq::Archive::open(data).map_err(err_js).map(|archive| WasmMpq { archive })
    }

    /// True if the name hashes to an existing entry (`/` and `\` equivalent,
    /// case-insensitive through the hash).
    pub fn has(&self, name: &str) -> bool {
        self.archive.has(name)
    }

    /// Read a whole entry, or `undefined` if absent/corrupt.
    pub fn read(&self, name: &str) -> Option<Vec<u8>> {
        self.archive.read(name).ok()
    }
}

/// Decode a DC6 file into a JS array of frame objects:
/// `{width, height, offsetX, offsetY, flip, indices: Uint8Array}` where
/// `indices` is `width*height` palette indices in top-down row-major order
/// (0 = transparent). Throws a descriptive string on malformed input.
#[wasm_bindgen]
pub fn decode_dc6(data: Vec<u8>) -> Result<JsValue, JsValue> {
    let dc6 = d2dc6::Dc6::decode(&data).map_err(err_js)?;
    let arr = Array::new();
    for f in &dc6.frames {
        let o = Object::new();
        Reflect::set(&o, &JsValue::from_str("width"), &JsValue::from(f.width))?;
        Reflect::set(&o, &JsValue::from_str("height"), &JsValue::from(f.height))?;
        Reflect::set(&o, &JsValue::from_str("offsetX"), &JsValue::from(f.offset_x))?;
        Reflect::set(&o, &JsValue::from_str("offsetY"), &JsValue::from(f.offset_y))?;
        Reflect::set(&o, &JsValue::from_str("flip"), &JsValue::from(f.flip))?;
        Reflect::set(&o, &JsValue::from_str("indices"), &Uint8Array::from(f.pixels.as_slice()))?;
        arr.push(&o);
    }
    Ok(arr.into())
}

/// Expand palette indices into RGBA8 with a 768-byte palette (256×RGB;
/// a 767-byte D2 palette is zero-padded first, as in the reference tools).
/// Index 0 → alpha 0; other indices → alpha 255.
#[wasm_bindgen]
pub fn indices_to_rgba(indices: &[u8], palette: &[u8]) -> Vec<u8> {
    d2dc6::indices_to_rgba(indices, palette)
}
