//! # d2dc6
//!
//! Diablo 2 DC6 sprite decoder, written as a byte-faithful Rust port of the
//! validated Python reference `tools/dc6_mini.py` (table by table, loop by
//! loop — same philosophy as the `d2mpq` crate).
//!
//! The decoder consumes raw bytes and produces palette indices — no file IO,
//! no color logic baked in: [`indices_to_rgba`] applies a 768-byte palette
//! (256×RGB) afterwards, with index 0 mapped to fully transparent.
//!
//! Both known file layouts are auto-detected, exactly as in the Python:
//!
//! * **Layout A — "flat"** (every file of the 1.12 digital release):
//!   `u32 framePtr[dirs*framesPerDir]` at offset 24, no direction indirection.
//! * **Layout B — "classic"** (public spec): `u32 dirPtr[dirs]` at 24, each
//!   pointing at `u32 framePtr[framesPerDir]`.
//!
//! Both RLE grammars are implemented:
//!
//! * **variant** (1.12 digital release, `encoding == 0` yet the data *is*
//!   RLE — the field lies): `b <= 0x7F` literal run, `b == 0x80` end of
//!   scanline (no operand), `b == 0x81 n` literal of n, `b >= 0x82`
//!   transparent run of `b - 0x80`. **Inverted polarity vs classic.**
//! * **classic** (`encoding == 1`, public spec): `b <= 0x7F` transparent
//!   run, `b == 0x80 n` rewind (n > 0) / end of scanline (n == 0),
//!   `b == 0x81 n` literal of n, `b >= 0x82` literal of `b & 0x7F`.
//!
//! Scanlines are stored bottom-up in a single stream: the i-th scanline
//! produced by the loop lands on destination row `height - 1 - i`. Runs are
//! clamped to the row width (a short row stays transparent); the 4-byte
//! padding after each frame payload (debug fill `0xCD`/`0xEE`) is never
//! decoded because decoding stops after `dataLen` bytes or `height` rows.
//!
//! ```no_run
//! use d2dc6::Dc6;
//!
//! let raw = std::fs::read("assets/skillicon.dc6").unwrap();
//! let dc6 = Dc6::decode(&raw).unwrap();
//! for frame in &dc6.frames {
//!     println!("{}x{}", frame.width, frame.height);
//! }
//! ```

use d2mpq::D2Error;

/// A decoded DC6 file: frames in direction-major order (the order of the
/// frame pointers in the file).
pub struct Dc6 {
    pub frames: Vec<Frame>,
}

/// One decoded frame: metadata plus `width * height` palette indices in
/// TOP-DOWN row-major order (index 0 = transparent).
pub struct Frame {
    pub flip: u32,
    pub width: u32,
    pub height: u32,
    pub offset_x: i32,
    pub offset_y: i32,
    pub pixels: Vec<u8>,
}

fn u32le(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Check a candidate frame header; `Some((flip, w, h, ox, oy))` if plausible.
/// Port of `_valid_frame_header`.
fn valid_frame_header(data: &[u8], off: usize) -> Option<(u32, u32, u32, i32, i32)> {
    if off + 28 > data.len() {
        return None;
    }
    let flip = u32le(data, off);
    let w = u32le(data, off + 4);
    let h = u32le(data, off + 8);
    let ox = u32le(data, off + 12) as i32;
    let oy = u32le(data, off + 16) as i32;
    // alloc @+20 and next @+24 are read but only used for plausibility there.
    if flip > 1 || w == 0 || h == 0 || w > 2048 || h > 2048 {
        return None;
    }
    Some((flip, w, h, ox, oy))
}

/// Frame pointers, layout A (flat) with layout B (classic direction
/// indirection) as fallback. Port of `_frame_pointers`.
fn frame_pointers(data: &[u8]) -> Result<(u32, u32, Vec<usize>), D2Error> {
    let version = u32le(data, 0);
    let encoding = u32le(data, 8);
    let ndir = u32le(data, 16) as usize;
    let nfpd = u32le(data, 20) as usize;
    if ndir == 0 || ndir > 32 || nfpd == 0 || nfpd > 4096 {
        return Err(D2Error::Corrupt("dc6: implausible dir/frame counts"));
    }
    let total = ndir * nfpd;

    // Layout A: flat frame pointers at 24 (Python reads as many u32 as fit,
    // then validates the first `total` — identical on every well-formed file).
    let n_avail = total.min((data.len() - 24) / 4);
    let mut ptrs: Vec<usize> = (0..n_avail).map(|i| u32le(data, 24 + i * 4) as usize).collect();
    if n_avail > 0 && ptrs.iter().all(|&p| valid_frame_header(data, p).is_some()) {
        return Ok((version, encoding, ptrs));
    }

    // Layout B: direction indirection (classic).
    ptrs.clear();
    if data.len() >= 24 + 4 * ndir {
        let mut ok = true;
        for d in 0..ndir {
            let dp = u32le(data, 24 + d * 4) as usize;
            if dp + 4 * nfpd > data.len() {
                ok = false;
                break;
            }
            for f in 0..nfpd {
                let fp = u32le(data, dp + f * 4) as usize;
                if valid_frame_header(data, fp).is_none() {
                    ok = false;
                    break;
                }
                ptrs.push(fp);
            }
            if !ok {
                break;
            }
        }
        if ok {
            return Ok((version, encoding, ptrs));
        }
    }
    Err(D2Error::Corrupt("dc6: neither flat nor classic pointer layout validates"))
}

/// Decode DC6 RLE frame data into a `width * height` palette-index buffer.
/// Port of `rle_rows` — both grammars, bottom-up scanlines, clamped runs,
/// stopping after `datalen` bytes or `height` scanlines so the 4-byte padding
/// junk after the payload is never decoded.
fn rle_rows(data: &[u8], pos: usize, width: usize, height: usize, classic: bool, datalen: u32) -> Vec<u8> {
    let mut out = vec![0u8; width * height];
    let n = data.len().min(pos + datalen as usize);
    let mut p = pos;
    let mut x: usize = 0;
    let mut row: usize = 0;
    while p < n && row < height {
        let b = data[p];
        p += 1;
        if classic {
            if b <= 0x7F {
                // transparent run
                x += b as usize;
            } else if b == 0x80 {
                // rewind n, or end of scanline when n == 0
                if p >= n {
                    break;
                }
                let r = data[p];
                p += 1;
                if r == 0 {
                    row += 1;
                    x = 0;
                } else {
                    x = x.saturating_sub(r as usize); // overwrite: re-decode backtracks
                }
            } else if b == 0x81 {
                // literal run, explicit count
                if p >= n {
                    break;
                }
                let cnt = data[p] as usize;
                let r = cnt.min(width.saturating_sub(x)).min(data.len().saturating_sub(p + 1));
                let y = height - 1 - row;
                let dst = y * width + x;
                out[dst..dst + r].copy_from_slice(&data[p + 1..p + 1 + r]);
                x += cnt;
                p += 1 + cnt;
            } else {
                // literal run, count in opcode
                let op = (b & 0x7F) as usize;
                let r = op.min(width.saturating_sub(x)).min(data.len().saturating_sub(p));
                let y = height - 1 - row;
                let dst = y * width + x;
                out[dst..dst + r].copy_from_slice(&data[p..p + r]);
                x += op;
                p += op;
            }
        } else {
            if b <= 0x7F {
                // literal run, count in opcode
                let r = (b as usize).min(width.saturating_sub(x)).min(data.len().saturating_sub(p));
                let y = height - 1 - row;
                let dst = y * width + x;
                out[dst..dst + r].copy_from_slice(&data[p..p + r]);
                x += b as usize;
                p += b as usize;
            } else if b == 0x80 {
                // end of scanline (no operand)
                row += 1;
                x = 0;
            } else if b == 0x81 {
                // literal run, explicit count
                if p >= n {
                    break;
                }
                let cnt = data[p] as usize;
                let r = cnt.min(width.saturating_sub(x)).min(data.len().saturating_sub(p + 1));
                let y = height - 1 - row;
                let dst = y * width + x;
                out[dst..dst + r].copy_from_slice(&data[p + 1..p + 1 + r]);
                x += cnt;
                p += 1 + cnt;
            } else {
                // transparent run of b - 0x80
                x = (x + (b - 0x80) as usize).min(width);
            }
        }
    }
    out
}

impl Dc6 {
    /// Decode a DC6 file (raw bytes). Auto-detects flat vs classic pointer
    /// layout and variant vs classic RLE (`encoding == 1` selects the classic
    /// grammar, anything else the variant grammar) — exactly like the Python
    /// reference.
    pub fn decode(data: &[u8]) -> Result<Dc6, D2Error> {
        if data.len() < 24 {
            return Err(D2Error::Corrupt("dc6: file shorter than the fixed header"));
        }
        let (version, encoding, ptrs) = frame_pointers(data)?;
        let classic = encoding == 1;
        let mut frames = Vec::with_capacity(ptrs.len());
        for off in ptrs {
            let (flip, w, h, ox, oy) =
                valid_frame_header(data, off).ok_or(D2Error::Corrupt("dc6: bad frame header"))?;
            let datalen = u32le(data, off + 28);
            let dstart = off + 32 + if version >= 7 { 4 } else { 0 };
            let pixels = rle_rows(data, dstart, w as usize, h as usize, classic, datalen);
            frames.push(Frame {
                flip,
                width: w,
                height: h,
                offset_x: ox,
                offset_y: oy,
                pixels,
            });
        }
        Ok(Dc6 { frames })
    }
}

/// Expand palette indices into RGBA8 using a 768-byte palette (256×RGB).
/// Index 0 is transparent (alpha 0); every other index gets alpha 255.
/// A short palette (e.g. the 767-byte D2 `pal.dat`) must be zero-padded to
/// 768 first; out-of-range colors read as black.
pub fn indices_to_rgba(indices: &[u8], palette: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len() * 4);
    for &i in indices {
        if i == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
        } else {
            let p = i as usize * 3;
            let (r, g, b) = if p + 2 < palette.len() {
                (palette[p], palette[p + 1], palette[p + 2])
            } else {
                (0, 0, 0)
            };
            out.extend_from_slice(&[r, g, b, 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal flat-layout DC6 (v6, encoding=0, variant grammar).
    fn build_flat_dc6(rows: &[&[u8]], w: u32, h: u32) -> Vec<u8> {
        let mut payload: Vec<u8> = Vec::new();
        for r in rows {
            payload.extend_from_slice(r);
            payload.push(0x80); // EOL
        }
        while !payload.len().is_multiple_of(4) {
            payload.push(0xCD); // 4-byte padding junk — must not be decoded
        }
        let fptr = 28u32; // flat frame-pointer table is at 24, frame data at 28
        let mut v = Vec::new();
        v.extend_from_slice(&6u32.to_le_bytes()); // version
        v.extend_from_slice(&1u32.to_le_bytes()); // flags
        v.extend_from_slice(&0u32.to_le_bytes()); // encoding -> variant grammar
        v.extend_from_slice(&0xCDCDCDCDu32.to_le_bytes()); // filler
        v.extend_from_slice(&1u32.to_le_bytes()); // dirs
        v.extend_from_slice(&1u32.to_le_bytes()); // frames per dir
        v.extend_from_slice(&fptr.to_le_bytes()); // flat frame pointer
        v.extend_from_slice(&0u32.to_le_bytes()); // flip
        v.extend_from_slice(&w.to_le_bytes());
        v.extend_from_slice(&h.to_le_bytes());
        v.extend_from_slice(&0i32.to_le_bytes()); // offset_x
        v.extend_from_slice(&0i32.to_le_bytes()); // offset_y
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // allocSize
        v.extend_from_slice(&0u32.to_le_bytes()); // nextBlock
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // dataLen
        v.extend_from_slice(&payload);
        v
    }

    #[test]
    fn variant_grammar_flat_layout() {
        // Row 0 (bottom): "AAAA" then EOL ; row 1 (top): 3 literals + EOL.
        // w=4 h=2 ; rows stored bottom-up in one stream.
        let file = build_flat_dc6(&[&[0x04, b'A', b'B', b'C', b'D'], &[0x03, b'X', b'Y', b'Z']], 4, 2);
        let dc6 = Dc6::decode(&file).unwrap();
        assert_eq!(dc6.frames.len(), 1);
        let f = &dc6.frames[0];
        assert_eq!((f.width, f.height, f.flip, f.offset_x, f.offset_y), (4, 2, 0, 0, 0));
        // stream row 0 -> dest row 1 (bottom), stream row 1 -> dest row 0 (top);
        // the top row is one pixel short (stays transparent at x=3)
        assert_eq!(&f.pixels, b"XYZ\0ABCD");
    }

    #[test]
    fn variant_transparent_run_and_short_row() {
        // transparent x2, literal X, EOL => row (top) = "..X." with clamped x
        let file = build_flat_dc6(&[&[0x82, 0x01, b'X']], 4, 1);
        let dc6 = Dc6::decode(&file).unwrap();
        assert_eq!(&dc6.frames[0].pixels, b"\0\0X\0");
    }

    #[test]
    fn variant_0x81_explicit_literal_count() {
        let file = build_flat_dc6(&[&[0x81, 0x04, b'A', b'B', b'C', b'D']], 4, 1);
        let dc6 = Dc6::decode(&file).unwrap();
        assert_eq!(&dc6.frames[0].pixels, b"ABCD");
    }

    #[test]
    fn classic_grammar_layout_b_with_rewind() {
        // Layout B: 1 dir pointing at 1 frame; classic grammar (encoding=1).
        // w=4 h=1 ; literal "AB", rewind 1, 0x81 literal "C" => pixels "AC..", EOL.
        let payload = [0x82, b'A', b'B', 0x80, 0x01, 0x81, 0x01, b'C', 0x80, 0x00];
        let mut v = Vec::new();
        v.extend_from_slice(&6u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // encoding=1 -> classic
        v.extend_from_slice(&0xEEEEEEEEu32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // dirs
        v.extend_from_slice(&1u32.to_le_bytes()); // frames/dir
        let dir_ptr_off = v.len();
        v.extend_from_slice(&0u32.to_le_bytes()); // dir ptr placeholder
        let frame_off = v.len() as u32;
        v[dir_ptr_off..dir_ptr_off + 4].copy_from_slice(&frame_off.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes()); // flip
        v.extend_from_slice(&4u32.to_le_bytes()); // w
        v.extend_from_slice(&1u32.to_le_bytes()); // h
        v.extend_from_slice(&0i32.to_le_bytes());
        v.extend_from_slice(&0i32.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&(payload.len() as u32).to_le_bytes()); // dataLen
        v.extend_from_slice(&payload);
        v.push(0xEE); v.push(0xEE); v.push(0xEE); // padding junk (not 4-aligned on purpose)
        let dc6 = Dc6::decode(&v).unwrap();
        assert_eq!(dc6.frames.len(), 1);
        assert_eq!(&dc6.frames[0].pixels, b"AC\0\0");
    }

    #[test]
    fn classic_transparent_run_and_literal_opcode() {
        // classic: 0x03 = transparent x3, 0x82 = literal x2 "QZ", 0x80 0x00 EOL
        let mut v = Vec::new();
        v.extend_from_slice(&7u32.to_le_bytes()); // version 7 -> data starts at +32+4
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // classic
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes());
        let dir_ptr_off = v.len();
        v.extend_from_slice(&0u32.to_le_bytes());
        let frame_off = v.len() as u32;
        v[dir_ptr_off..dir_ptr_off + 4].copy_from_slice(&frame_off.to_le_bytes());
        v.extend_from_slice(&1u32.to_le_bytes()); // flip=1
        v.extend_from_slice(&5u32.to_le_bytes()); // w
        v.extend_from_slice(&1u32.to_le_bytes()); // h
        v.extend_from_slice(&(-3i32).to_le_bytes());
        v.extend_from_slice(&9i32.to_le_bytes());
        v.extend_from_slice(&32u32.to_le_bytes());
        v.extend_from_slice(&0u32.to_le_bytes());
        v.extend_from_slice(&32u32.to_le_bytes()); // dataLen (padded, stops early)
        let payload = [0x03, 0x82, b'Q', b'Z', 0x80, 0x00, 0xEE, 0xEE];
        v.extend_from_slice(&[0xDE, 0xAD, 0xBE, 0xEF]); // v7 extra 4 bytes before data
        v.extend_from_slice(&payload);
        let dc6 = Dc6::decode(&v).unwrap();
        let f = &dc6.frames[0];
        assert_eq!((f.flip, f.offset_x, f.offset_y), (1, -3, 9));
        assert_eq!(&f.pixels, b"\0\0\0QZ");
    }

    #[test]
    fn garbage_rejected() {
        assert!(Dc6::decode(&[]).is_err());
        assert!(Dc6::decode(&[0u8; 24]).is_err()); // counts = 0
        let mut v = vec![0xFFu8; 64];
        v[16] = 1; // dirs = 1
        v[20] = 1; // frames = 1
        v[24] = 0x40; // frame pointer way out of range
        assert!(Dc6::decode(&v).is_err());
    }

    #[test]
    fn rgba_conversion() {
        let palette: Vec<u8> = (0u8..=255).collect(); // 256*3, palette[i*3..i*3+3] = i*3, ...
        let rgba = indices_to_rgba(&[0, 1, 2], &palette);
        assert_eq!(rgba.len(), 12);
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]); // index 0 -> transparent
        assert_eq!(&rgba[4..8], &[3, 4, 5, 255]);
        assert_eq!(&rgba[8..12], &[6, 7, 8, 255]);
    }

    #[test]
    fn palette_edge_cases() {
        // 767-byte D2 palette: a color past the provided bytes reads black+opaque
        let palette = vec![1u8, 2, 3, 4, 5, 6]; // index 0 and 1 only
        let rgba = indices_to_rgba(&[0, 1, 2], &palette);
        assert_eq!(&rgba[0..4], &[0, 0, 0, 0]); // index 0 -> transparent, whatever the color
        assert_eq!(&rgba[4..8], &[4, 5, 6, 255]);
        assert_eq!(&rgba[8..12], &[0, 0, 0, 255]); // out-of-palette index -> black
    }
}
