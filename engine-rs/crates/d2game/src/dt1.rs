//! DT1 tile-container decoder (release v1.12 layout).
//!
//! Port of `tools/map_proto2.py` (`ContainerDT1`, `decode_wall_block`),
//! which produced the validated `assets/map/level-render.png`. The traps
//! that this release bakes in (documented in the prototype header):
//!
//! * **No `C7 AB 0B 00` signature** — the container starts with the int32
//!   pair `(7, 6)`, tile count lives at byte 268, the tile table at byte
//!   252 with a 96-byte stride.
//! * **Kind = sub-block count**: `nsub == 25` → floor (raw 32×8 blocks),
//!   anything else → wall (RLE rows). The classic "orientation" field is
//!   unreliable here; walls with orientation 0 are floors and do have 25
//!   sub-blocks, so the count rule covers them (the "murs ori-0 = sols"
//!   trap).
//! * Floor blocks are raw 256-byte bitmaps (32×8); wall blocks are
//!   line-RLE `(transparent, opaque, bytes)` triplets, `t >= 32 && o == 0`
//!   flags a `t & 0x1F` safety, trailing `0x00` rows are padding.

/// One placed sub-block of a tile, payload already decoded to raw pixels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dt1Block {
    /// Placement x, in tile-local pixels.
    pub a: i16,
    /// Placement y, in tile-local pixels (negative = above the tile top).
    pub b: i16,
    /// Sub-block flags (bit meanings unreliable in this release — see
    /// `compose::collision` for the policy actually used).
    pub flags: u16,
    /// Decoded pixels: floor = 256 bytes (32×8 rows), wall = `rows*32`.
    pub data: Vec<u8>,
}

/// One DT1 tile: floor or wall, with its placed blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Dt1Tile {
    pub is_floor: bool,
    pub blocks: Vec<Dt1Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dt1Error {
    /// Bytes shorter than the fixed header this format needs.
    TooShort,
    /// The `(7, 6)` version pair is missing — not this release's layout.
    BadVersion(i32, i32),
    /// A sub-block payload runs past the blob.
    Truncated,
}

fn read_i32(d: &[u8], at: usize) -> Result<i32, Dt1Error> {
    let b = d.get(at..at + 4).ok_or(Dt1Error::TooShort)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

fn read_i16(d: &[u8], at: usize) -> Result<i16, Dt1Error> {
    let b = d.get(at..at + 2).ok_or(Dt1Error::TooShort)?;
    Ok(i16::from_le_bytes([b[0], b[1]]))
}

fn read_u16(d: &[u8], at: usize) -> Result<u16, Dt1Error> {
    Ok(read_i16(d, at)? as u16)
}

/// `decode_wall_block` — expand line-RLE wall rows to raw `rows*32` bytes.
pub fn decode_wall_block(data: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    let (mut j, n) = (0usize, data.len());
    while j + 1 < n {
        let (mut t, o) = (data[j] as usize, data[j + 1] as usize);
        j += 2;
        if t >= 32 && o == 0 {
            t &= 0x1F; // safety, as in the prototype
        }
        out.resize(out.len() + t, 0);
        let take = o.min(n.saturating_sub(j)); // truncated tail = padding
        out.extend_from_slice(&data[j..j + take]);
        j += o;
    }
    out
}

/// Decode a whole DT1 container into its tiles, in table order.
pub fn decode_dt1(d: &[u8]) -> Result<Vec<Dt1Tile>, Dt1Error> {
    let v1 = read_i32(d, 0)?;
    let v2 = read_i32(d, 4)?;
    if v1 != 7 || v2 != 6 {
        return Err(Dt1Error::BadVersion(v1, v2));
    }
    let nbtiles = read_i32(d, 268)?.max(0) as usize;
    let mut tiles = Vec::with_capacity(nbtiles);
    for k in 0..nbtiles {
        let r = 252 + 96 * k;
        let dataoff = read_i32(d, r)? as usize;
        let datalen = read_i32(d, r + 4)? as usize;
        let nsub = read_i32(d, r + 8)?.max(0) as usize;
        let is_floor = nsub == 25;
        let mut blocks = Vec::with_capacity(nsub);
        if datalen > 0 {
            for j in 0..nsub {
                let h = dataoff + 20 * j;
                let a = read_i16(d, h)?;
                let b = read_i16(d, h + 2)?;
                let flags = read_u16(d, h + 8)?;
                let ln = read_i32(d, h + 10)?.max(0) as usize;
                let off = read_i32(d, h + 16)? as usize;
                let start = dataoff + off;
                let end = start + ln;
                if end > d.len() {
                    return Err(Dt1Error::Truncated);
                }
                let raw = &d[start..end];
                let data = if is_floor { raw.to_vec() } else { decode_wall_block(raw) };
                blocks.push(Dt1Block { a, b, flags, data });
            }
        }
        tiles.push(Dt1Tile { is_floor, blocks });
    }
    Ok(tiles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wall_rle_basic_rows() {
        // Row 1: (0 transp, 2 opaque AA BB). Row 2: (30 transp, 2 opaque
        // CC DD). Trailing (0x20, 0) = zero-width run (padding), no row.
        let data = [0, 2, 0xAA, 0xBB, 30, 2, 0xCC, 0xDD, 0x20, 0];
        let out = decode_wall_block(&data);
        assert_eq!(out.len(), 64 - 30); // 32 + 2
        assert_eq!(&out[0..2], &[0xAA, 0xBB]);
        assert!(out[2..32].iter().all(|&v| v == 0));
        assert_eq!(&out[32..34], &[0xCC, 0xDD]);
    }

    #[test]
    fn short_container_errors_cleanly() {
        assert_eq!(decode_dt1(&[7, 0, 0, 0]), Err(Dt1Error::TooShort));
        assert_eq!(decode_dt1(&[1, 0, 0, 0, 2, 0, 0, 0]), Err(Dt1Error::BadVersion(1, 2)));
    }
}
