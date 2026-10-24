//! DT1 **sub-tile flags** (25 per tile) and the fine walkability grid —
//! additive companion to [`crate::dt1`], which stays the validated render
//! path (its block-decoder quirks are documented there).
//!
//! Port of the RE campaign `d2w-re/evidence-formats/dt1-subtile` (8 real
//! DT1 v1.12, byte-verified against OpenDiablo2 `dt1.go`). The flags are
//! intact at a **fixed +40 offset in the 96-byte tile header**, the tile
//! table starting at `bodyPosition` (byte 272's value, 276 on all 8
//! samples) — NOT at 252 as the render decoder reads it. Because the DS1
//! props are 1-based (`0` = empty cell) the render shift cancels out for
//! drawing, but any per-tile flag lookup must use this correct table
//! (evidence §7).
//!
//! Tile header (96 bytes at `bodyPosition + 96*k`), fields read here:
//! `+0` direction i32, `+4` roofheight i16, `+6` material u16, `+8`
//! height i32, `+12` width i32, `+20` type i32, `+24` style i32, `+28`
//! sequence i32, `+32` rarityFrameIndex i32, **`+40` 25 × u8 flags**,
//! `+72` blockHeaderPointer i32, `+76` blockHeaderSize i32, `+80`
//! numBlocks i32.
//!
//! Flag bits (OpenDiablo2 `subtile.go:68-79`): bit 0 `BlockWalk`
//! (0x01), bit 1 `BlockLOS` (0x02), bit 2 `BlockJump` (0x04); bits 3+
//! never observed on the v1.12 town samples. Distinct bytes on the whole
//! corpus: `0x00, 0x01, 0x05, 0x07` only — water blocks the step but not
//! the arrows (0x01), fences the step and the jump (0x05), cliffs all
//! three (0x07).
//!
//! Grid order: storage `flags[i]`, `i = y*5 + x`, row 0 = **SOUTH**
//! screen row; display code must remap (`OpenDiablo2 map_tile.go:19-25`
//! `subtileLookup`) — [`Dt1TileFlags::screen`] does it.
//!
//! [`fine_walk_grid`] ORs the flags of every wall + floor + shadow
//! stream of a [`crate::ds1_path::Ds1Path`] into a 1/5-tile walkability +
//! LOS grid (Go `SubTileFlags.Combine`, `map_tile.go:42-78`) — the A*-
//! ready replacement for the all-or-nothing `collision.json` (evidence
//! §9). Golden tests: `tests/dt1_flags_golden.rs`.

use crate::compose::SUB;
use crate::ds1::{cell_fields, cell_tile_index};
use crate::ds1_path::{Ds1Path, LayerKind};

pub const BLOCK_WALK: u8 = 0x01;
pub const BLOCK_LOS: u8 = 0x02;
pub const BLOCK_JUMP: u8 = 0x04;

/// Sub-tile flags of one DT1 tile (read-only header view; the block
/// pixel payloads stay in [`crate::dt1`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dt1TileFlags {
    pub direction: i32,
    pub height: i32,
    pub width: i32,
    /// Classic tile type (0 = floor, 1/2 = L/R wall, 13 = shadow…). On
    /// the v1.12 samples floors carry `direction == 3` — do NOT filter
    /// on `direction == 0` (evidence §8.1); use [`Self::is_floor`].
    pub tile_type: i32,
    pub style: i32,
    pub sequence: i32,
    pub num_blocks: i32,
    /// Raw 25 bytes, storage order `i = y*5 + x`, row 0 = south row.
    pub flags: [u8; 25],
}

impl Dt1TileFlags {
    /// The release's floor rule (`nsub == 25`, [`crate::dt1`]): holds on
    /// 144/144 + 44/44 floor tiles of the samples, walls have 5–68.
    pub fn is_floor(&self) -> bool {
        self.num_blocks == 25
    }

    /// Raw storage-order flag byte (`i = y*5 + x`, row 0 = south).
    #[inline]
    pub fn raw(&self, i: usize) -> u8 {
        self.flags[i]
    }

    /// Flag byte at sub-tile `(x, y)` in screen orientation (y = 0 is
    /// the NORTH row; OpenDiablo2 `subtileLookup` remap).
    #[inline]
    pub fn screen(&self, x: usize, y: usize) -> u8 {
        self.flags[(4 - y) * 5 + x]
    }

    #[inline]
    pub fn walk_blocked(&self, x: usize, y: usize) -> bool {
        self.screen(x, y) & BLOCK_WALK != 0
    }

    #[inline]
    pub fn los_blocked(&self, x: usize, y: usize) -> bool {
        self.screen(x, y) & BLOCK_LOS != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dt1FlagsError {
    TooShort,
    /// The `(7, 6)` version pair is missing — not this release's layout.
    BadVersion(i32, i32),
}

fn i32_at(d: &[u8], at: usize) -> Result<i32, Dt1FlagsError> {
    let b = d.get(at..at + 4).ok_or(Dt1FlagsError::TooShort)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Decode the tile table of a DT1 container with its sub-tile flags —
/// one [`Dt1TileFlags`] per REAL tile, table at `bodyPosition` (not the
/// render decoder's 252 shift).
pub fn decode_dt1_flags(d: &[u8]) -> Result<Vec<Dt1TileFlags>, Dt1FlagsError> {
    let v1 = i32_at(d, 0)?;
    let v2 = i32_at(d, 4)?;
    if v1 != 7 || v2 != 6 {
        return Err(Dt1FlagsError::BadVersion(v1, v2));
    }
    let nbtiles = i32_at(d, 268)?.max(0) as usize;
    let body = i32_at(d, 272)? as usize;
    let mut tiles = Vec::with_capacity(nbtiles);
    for k in 0..nbtiles {
        let r = body + 96 * k;
        let mut flags = [0u8; 25];
        flags.copy_from_slice(d.get(r + 40..r + 65).ok_or(Dt1FlagsError::TooShort)?);
        tiles.push(Dt1TileFlags {
            direction: i32_at(d, r)?,
            height: i32_at(d, r + 8)?,
            width: i32_at(d, r + 12)?,
            tile_type: i32_at(d, r + 20)?,
            style: i32_at(d, r + 24)?,
            sequence: i32_at(d, r + 28)?,
            num_blocks: i32_at(d, r + 80)?,
            flags,
        });
    }
    Ok(tiles)
}

/// Fine walkability grid at 1/5 of a tile (sub-tile), assembled from a
/// full DS1 parse + the concatenated REAL tile flag tables of its `.tg1`
/// file list, in file order.
///
/// Semantics (Go `map_tile.go:42-78` `SubTileFlags.Combine`, evidence
/// §9): every wall, floor and shadow stream cell resolves to its tile
/// (DS1 props are 1-based: packed index `p1 + (p2 & 0x0F)`, real tile =
/// index − 1) and ORs its 25 flags into the cell; orientation and
/// substitution streams are skipped. A sub-tile is walkable when its
/// cell carries at least one floor tile (no floor = void, as in
/// [`crate::compose`]) AND the combined flags have no `BlockWalk` bit.
/// `los_blocked` exposes `BlockLOS` for ranged attacks / visibility.
#[derive(Debug, Clone)]
pub struct SubTileGrid {
    /// `ds1.w * 5` sub-tiles.
    pub w: usize,
    /// `ds1.h * 5` sub-tiles.
    pub h: usize,
    /// `h * w`, row-major, screen orientation (sub-row 0 = north edge
    /// of each cell, y growing south — same remap as [`Dt1TileFlags::screen`]).
    pub walk: Vec<bool>,
    /// Same shape: `BlockLOS` of the combined flags.
    pub los: Vec<bool>,
    /// Same shape: the combined flag byte per sub-tile (diagnostics).
    pub combined: Vec<u8>,
}

impl SubTileGrid {
    #[inline]
    pub fn walkable(&self, x: usize, y: usize) -> bool {
        self.walk.get(y * self.w + x).copied().unwrap_or(false)
    }

    #[inline]
    pub fn los_blocked(&self, x: usize, y: usize) -> bool {
        self.los.get(y * self.w + x).copied().unwrap_or(true)
    }

    /// The grid as rows of bools — the same contract as
    /// [`crate::compose::ComposedMap::walk_rows`] / `d2rules::World::new`,
    /// for A* over sub-tile nodes.
    pub fn walk_rows(&self) -> Vec<Vec<bool>> {
        self.walk.chunks(self.w).map(|r| r.to_vec()).collect()
    }
}

/// Build the fine grid — see [`SubTileGrid`].
pub fn fine_walk_grid(ds1: &Ds1Path, tiles: &[Dt1TileFlags]) -> SubTileGrid {
    let (gw, gh) = (ds1.w * SUB, ds1.h * SUB);
    let mut combined = vec![0u8; gw * gh];
    let mut cell_floor = vec![false; ds1.w * ds1.h];

    for layer in &ds1.layers {
        let physical = matches!(layer.kind, LayerKind::Wall | LayerKind::Floor | LayerKind::Shadow);
        if !physical {
            continue; // orientation / substitution streams carry no flags
        }
        for cy in 0..ds1.h {
            for cx in 0..ds1.w {
                let v = layer.cells[cy * ds1.w + cx];
                let (p1, p2, _p3, orient) = cell_fields(v);
                if p1 == 0 || orient & 0x80 != 0 {
                    continue;
                }
                let index = cell_tile_index(p1, p2);
                // DS1 props are 1-based (0 = empty): the concatenated list
                // of REAL tiles is indexed by prop − 1 (evidence §7).
                let Some(tile) = index.checked_sub(1).and_then(|i| tiles.get(i)) else {
                    continue;
                };
                if layer.kind == LayerKind::Floor {
                    cell_floor[cy * ds1.w + cx] = true;
                }
                for row in 0..SUB {
                    for col in 0..SUB {
                        // storage row 0 = south → screen sub-row 4 − row
                        let sy = cy * SUB + (SUB - 1 - row);
                        let sx = cx * SUB + col;
                        combined[sy * gw + sx] |= tile.flags[row * 5 + col];
                    }
                }
            }
        }
    }

    let mut walk = vec![false; gw * gh];
    // LOS from the combined flags everywhere — a cliff face blocks the
    // sight even where no floor tile was ever placed (void cells).
    let los: Vec<bool> = combined.iter().map(|f| f & BLOCK_LOS != 0).collect();
    for cy in 0..ds1.h {
        for cx in 0..ds1.w {
            if !cell_floor[cy * ds1.w + cx] {
                continue; // void: never walkable
            }
            for row in 0..SUB {
                for col in 0..SUB {
                    let sy = cy * SUB + row;
                    let sx = cx * SUB + col;
                    let i = sy * gw + sx;
                    walk[i] = combined[i] & BLOCK_WALK == 0;
                }
            }
        }
    }

    SubTileGrid { w: gw, h: gh, walk, los, combined }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ds1_path::{Ds1Layer, Ds1Path};

    fn flags_of(vals: &[(usize, u8)]) -> [u8; 25] {
        let mut f = [0u8; 25];
        for (i, v) in vals {
            f[*i] = *v;
        }
        f
    }

    /// Screen remap: storage row 0 (indices 0..4) must read at screen
    /// row 4 (south), storage row 4 at screen row 0 (north).
    #[test]
    fn screen_orientation_remaps_rows() {
        let t = Dt1TileFlags {
            direction: 3,
            height: 0,
            width: 0,
            tile_type: 0,
            style: 0,
            sequence: 0,
            num_blocks: 25,
            flags: flags_of(&[(0, 0x01), (24, 0x02)]),
        };
        assert!(t.walk_blocked(0, 4), "storage row 0 = south screen row");
        assert!(t.los_blocked(4, 0), "storage row 4 = north screen row");
        assert!(!t.walk_blocked(0, 0) && !t.los_blocked(0, 4));
    }

    fn v18_ds1(w: usize, h: usize, layers: Vec<(LayerKind, Vec<i32>)>) -> Ds1Path {
        let n = w * h;
        Ds1Path {
            version: 18,
            w,
            h,
            act: Some(0),
            subst_type: Some(0),
            files: vec![],
            layers: layers
                .into_iter()
                .map(|(kind, cells)| Ds1Layer { kind, cells })
                .chain(std::iter::once(Ds1Layer {
                    kind: LayerKind::Shadow,
                    cells: vec![0; n],
                }))
                .collect(),
            objects: vec![],
            npcs: vec![],
            path_section_offset: None,
            consumed_to_eof: true,
        }
    }

    /// 3×1 cells. Cell 0: floor tile (prop 1 → real 0) with a 0x01 flag
    /// at storage (2,1) — the water-style spot. Cell 1: wall-only cell
    /// (prop 2 → real 1, 0x07 at storage (2,2)) over NO floor — the wall
    /// flags still OR in, the void rule blocks the rest. Cell 2: open
    /// grass floor.
    #[test]
    fn fine_grid_or_and_void_rules() {
        let floor0 = Dt1TileFlags {
            direction: 3, height: -128, width: 96, tile_type: 0, style: 0, sequence: 0,
            num_blocks: 25,
            flags: flags_of(&[(7, 0x01)]),
        };
        let wall1 = Dt1TileFlags {
            direction: 1, height: -256, width: 96, tile_type: 1, style: 0, sequence: 0,
            num_blocks: 21,
            flags: flags_of(&[(12, 0x07)]),
        };
        let grass = Dt1TileFlags {
            direction: 3, height: -128, width: 96, tile_type: 0, style: 0, sequence: 0,
            num_blocks: 25,
            flags: [0u8; 25],
        };
        let tiles = vec![floor0, wall1, grass];
        let ds1 = v18_ds1(
            3,
            1,
            vec![
                (LayerKind::Floor, vec![1, 0, 1]),
                (LayerKind::Wall, vec![0, 2, 0]),
                (LayerKind::Orientation, vec![0, 0, 0]),
            ],
        );
        let g = fine_walk_grid(&ds1, &tiles);
        assert_eq!((g.w, g.h), (15, 5));

        // Cell 0: everything walkable except storage (2,1) = screen row
        // 3, col 2 — and LOS stays clear there (0x01, water-style).
        assert!(g.walkable(0, 0));
        assert!(!g.walkable(2, 3));
        assert!(!g.los_blocked(2, 3));
        assert!(g.walkable(2, 1), "storage row 1 = screen row 3, not 1");
        // Cell 1: no floor → void, blocked everywhere (compose parity)…
        assert!(!g.walkable(5, 0) && !g.walkable(9, 4));
        // …yet the wall's 0x07 still ORs in: storage (2,2) = screen (7,2)
        // cuts BOTH walk and LOS.
        assert!(g.los_blocked(5 + 2, 2));
        // Cell 2: open grass, walkable and LOS-clear.
        assert!(g.walkable(10, 0) && g.walkable(14, 4));
        assert!(!g.los_blocked(12, 2));
        // walk_rows contract for A*.
        assert_eq!(g.walk_rows().len(), 5);
        assert_eq!(g.walk_rows()[0].len(), 15);
    }

    #[test]
    fn truncated_container_errors_cleanly() {
        assert_eq!(decode_dt1_flags(&[7, 0, 0, 0]), Err(Dt1FlagsError::TooShort));
        assert_eq!(
            decode_dt1_flags(&[1, 0, 0, 0, 2, 0, 0, 0]),
            Err(Dt1FlagsError::BadVersion(1, 2))
        );
    }
}
