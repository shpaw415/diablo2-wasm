//! DS1 level-layout decoder — version 18 (TownN1.ds1), exactly the
//! validated prototype `tools/map_proto2.py`.
//!
//! Layout: `version@0 == 18`, stored `w@4`, `h@8` — real size is
//! `(w+1) × (h+1)` (the v18 trap) — 2 zero ints, `fileCount@20`, that many
//! NUL-terminated `.tg1` paths from byte 24, a `(2, 1)` meta pair, then 6
//! layers of `w*h` int32 cells, then a PATH section (unused here).
//!
//! Cell packing: `p1 = v & 0xFF` (tile family base), `p2 = (v>>8) & 0xFF`
//! (variant — low nibble indexes consecutive tiles after `p1`), `p3 =
//! (v>>16) & 0xFF`, `orient = (v>>24) & 0xFF`; `orient & 0x80` hides the
//! cell. Layers: L4 = main floor, L0 = floor overlay (streets), L1 =
//! walls/trees/objects, L2/L5 = rare cells (stones, warp).

#[derive(Debug, Clone)]
pub struct Ds1 {
    pub version: i32,
    /// Real cell width (`stored + 1`).
    pub w: usize,
    /// Real cell height (`stored + 1`).
    pub h: usize,
    /// The `.tg1` paths in file order — defines the tileset concatenation
    /// order that tile indices refer to.
    pub files: Vec<String>,
    /// Six layers, each `w * h` packed cells, row-major (`y * w + x`).
    pub layers: Vec<Vec<i32>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ds1Error {
    TooShort,
    /// This crate only ports the v18 layout of the validated prototype.
    UnsupportedVersion(i32),
}

fn i32_at(d: &[u8], at: usize) -> Result<i32, Ds1Error> {
    let b = d.get(at..at + 4).ok_or(Ds1Error::TooShort)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Unpack one DS1 cell into `(p1, p2, p3, orient)`.
#[inline]
pub fn cell_fields(v: i32) -> (u8, u8, u8, u8) {
    (
        (v & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        ((v >> 16) & 0xFF) as u8,
        ((v >> 24) & 0xFF) as u8,
    )
}

/// Tile index for a cell: family base `p1` + variant `p2 & 0x0F`
/// (tile families are consecutive in the concatenated tilesets).
#[inline]
pub fn cell_tile_index(p1: u8, p2: u8) -> usize {
    p1 as usize + (p2 & 0x0F) as usize
}

pub fn decode_ds1(d: &[u8]) -> Result<Ds1, Ds1Error> {
    let version = i32_at(d, 0)?;
    if version != 18 {
        return Err(Ds1Error::UnsupportedVersion(version));
    }
    let w = i32_at(d, 4)?.max(0) as usize + 1; // v18 trap: real = stored + 1
    let h = i32_at(d, 8)?.max(0) as usize + 1;
    let filecount = i32_at(d, 20)?.max(0) as usize;
    let mut p = 24usize;
    let mut files = Vec::with_capacity(filecount);
    for _ in 0..filecount {
        let e = d
            .get(p..)
            .and_then(|rest| rest.iter().position(|&b| b == 0))
            .ok_or(Ds1Error::TooShort)?
            + p;
        files.push(String::from_utf8_lossy(&d[p..e]).into_owned());
        p = e + 1;
    }
    p += 8; // meta (2 ints) — unused
    let n = w * h;
    let mut layers = Vec::with_capacity(6);
    for _ in 0..6 {
        let mut cells = Vec::with_capacity(n);
        for i in 0..n {
            cells.push(i32_at(d, p + i * 4)?);
        }
        layers.push(cells);
        p += 4 * n;
    }
    Ok(Ds1 { version, w, h, files, layers })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cell_packing_matches_prototype() {
        // p1 + p2<<8 + p3<<16 + orient<<24, hidden bit 0x80 on orient.
        let v = (((3u32 | 0x80) << 24) | (2 << 16) | (5 << 8) | 7) as i32;
        assert_eq!(cell_fields(v), (7, 5, 2, 3 | 0x80));
        assert_eq!(cell_tile_index(10, 0x23), 13); // variant = low nibble only
    }
}
