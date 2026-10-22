//! Isometric map compositor — palette-index framebuffer + walkability grid.
//!
//! Port of the validated `tools/map_proto2.py` renderer. Geometry: each
//! DS1 cell is an 80×40 diamond (`CW×CH`), cell `(cx, cy)` paints at
//! `((cx-cy)*80 + ox, (cx+cy)*40 + oy)` with `ox = h*80`, `oy = 420`
//! (wall head-room, as in the prototype). Paint order: L4 main floor, L0
//! streets, L2/L5 rare cells, then L1 walls/trees row by row.
//!
//! The collision grid lives at sub-tile resolution: 5×5 sub-tiles of
//! 16×8 px per cell (`SUB = 5`), so world positions in sub-tile units map
//! to screen with `x = (wx-wy)*16 + ox`, `y = (wx+wy)*8 + oy`. This grid
//! is the `Vec<Vec<bool>>` contract input of `d2rules::World::new`.
//!
//! Collision policy for this release (flags proved non-discriminating —
//! floor sub-blocks carry `flags == 1` on grass *and* river alike, see
//! REPORT.md §3): floor-kind tiles walkable, wall-kind tiles block their
//! whole cell, the rare `flags & 0x2000` floor blocks block their own
//! sub-tile region.

use crate::ds1::{cell_fields, cell_tile_index, Ds1};
use crate::dt1::Dt1Tile;

pub const CW: i32 = 80;
pub const CH: i32 = 40;
pub const SUB: usize = 5;
/// Head-room above the map for wall tiles (prototype `wall_head`).
pub const WALL_HEAD: i32 = 420;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct ComposeStats {
    pub drawn: usize,
    pub hidden: usize,
    pub empty: usize,
    pub out_of_bounds: usize,
}

pub struct ComposedMap {
    /// Framebuffer width in px — `(w + h) * 80`.
    pub width: usize,
    /// Framebuffer height in px — `(w + h) * 40 + 500`.
    pub height: usize,
    pub origin_x: i32,
    pub origin_y: i32,
    /// Palette indices, `width * height`, row-major. 0 = never painted
    /// (background); game.rs fills it with the palette's black.
    pub idx: Vec<u8>,
    /// Collision grid width — `w * 5` sub-tiles.
    pub grid_w: usize,
    /// Collision grid height — `h * 5` sub-tiles.
    pub grid_h: usize,
    /// `grid_h * grid_w` booleans, row-major. `true` = walkable.
    pub walk: Vec<bool>,
    pub stats: ComposeStats,
}

impl ComposedMap {
    /// World (sub-tile, f32) → screen (px) transform.
    pub fn sub_to_screen(&self, wx: f32, wy: f32) -> (f32, f32) {
        sub_to_screen(self.origin_x, self.origin_y, wx, wy)
    }

    /// Screen (px) → world (sub-tile) inverse transform.
    pub fn screen_to_sub(&self, sx: f32, sy: f32) -> (f32, f32) {
        screen_to_sub(self.origin_x, self.origin_y, sx, sy)
    }

    /// The grid as the `d2rules` contract expects: one `Vec<bool>` per row.
    pub fn walk_rows(&self) -> Vec<Vec<bool>> {
        self.walk.chunks(self.grid_w).map(|r| r.to_vec()).collect()
    }

    #[inline]
    pub fn walkable(&self, sx: usize, sy: usize) -> bool {
        self.walk.get(sy * self.grid_w + sx).copied().unwrap_or(false)
    }
}

pub fn sub_to_screen(origin_x: i32, origin_y: i32, wx: f32, wy: f32) -> (f32, f32) {
    ((wx - wy) * 16.0 + origin_x as f32, (wx + wy) * 8.0 + origin_y as f32)
}

/// Spiral search (rings, radius ≤ `radius`) for the nearest walkable
/// sub-tile to `(cx, cy)` — spawn placement for tests/tools.
pub fn find_nearby_walkable(
    walk: &[bool],
    w: usize,
    h: usize,
    cx: usize,
    cy: usize,
    radius: usize,
) -> Option<(usize, usize)> {
    for r in 0..=radius {
        for dy in -(r as isize)..=(r as isize) {
            for dx in -(r as isize)..=(r as isize) {
                if dx.abs() != r as isize && dy.abs() != r as isize {
                    continue; // ring only
                }
                let x = cx as isize + dx;
                let y = cy as isize + dy;
                if x >= 0
                    && y >= 0
                    && (x as usize) < w
                    && (y as usize) < h
                    && walk[y as usize * w + x as usize]
                {
                    return Some((x as usize, y as usize));
                }
            }
        }
    }
    None
}

pub fn screen_to_sub(origin_x: i32, origin_y: i32, sx: f32, sy: f32) -> (f32, f32) {
    let a = (sx - origin_x as f32) / 16.0;
    let b = (sy - origin_y as f32) / 8.0;
    ((a + b) / 2.0, (b - a) / 2.0)
}

struct Painter<'a> {
    idx: &'a mut [u8],
    width: usize,
    height: usize,
    origin_x: i32,
    origin_y: i32,
}

impl<'a> Painter<'a> {
    fn draw_floor_block(&mut self, dst_x: i32, dst_y: i32, data: &[u8]) {
        for (i, &v) in data.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let x = dst_x + (i & 31) as i32;
            let y = dst_y + (i >> 5) as i32;
            if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                self.idx[y as usize * self.width + x as usize] = v;
            }
        }
    }

    fn draw_wall_block(&mut self, dst_x: i32, dst_y: i32, data: &[u8]) {
        let rows = data.len() / 32;
        for r in 0..rows {
            let base = r * 32;
            let row = &data[base..base + 32];
            if row.iter().all(|&v| v == 0) {
                continue;
            }
            let y = dst_y + r as i32;
            for (i, &v) in row.iter().enumerate() {
                if v == 0 {
                    continue;
                }
                let x = dst_x + i as i32;
                if x >= 0 && y >= 0 && (x as usize) < self.width && (y as usize) < self.height {
                    self.idx[y as usize * self.width + x as usize] = v;
                }
            }
        }
    }

    /// Draw one layer, exactly the prototype's `draw_layer`.
    fn draw_layer(&mut self, ds1: &Ds1, tilesets: &[Dt1Tile], li: usize, stats: &mut ComposeStats) {
        let cells = &ds1.layers[li];
        for cy in 0..ds1.h {
            for cx in 0..ds1.w {
                let v = cells[cy * ds1.w + cx];
                let (p1, p2, _p3, orient) = cell_fields(v);
                if p1 == 0 {
                    stats.empty += 1;
                    continue;
                }
                if orient & 0x80 != 0 {
                    stats.hidden += 1;
                    continue;
                }
                let index = cell_tile_index(p1, p2);
                if index >= tilesets.len() || p1 as usize >= tilesets.len() {
                    stats.out_of_bounds += 1;
                    continue;
                }
                let tile = if tilesets[index].blocks.is_empty() {
                    &tilesets[p1 as usize]
                } else {
                    &tilesets[index]
                };
                let sx = (cx as i32 - cy as i32) * CW + self.origin_x;
                let sy = (cx as i32 + cy as i32) * CH + self.origin_y;
                for b in &tile.blocks {
                    if tile.is_floor {
                        self.draw_floor_block(sx + b.a as i32, sy + b.b as i32, &b.data);
                    } else {
                        self.draw_wall_block(sx + b.a as i32, sy + b.b as i32, &b.data);
                    }
                }
                stats.drawn += 1;
            }
        }
    }
}

/// Floor sub-block flags that mark a blocked sub-region even on floor
/// tiles (the rare `0x2005`-family blocks — 24 in the whole town).
const FLOOR_BLOCK_BITS: u16 = 0x2000;

/// Compose the whole map: index framebuffer + collision grid.
pub fn compose(ds1: &Ds1, tilesets: &[Dt1Tile]) -> ComposedMap {
    let width = (ds1.w + ds1.h) * CW as usize;
    let height = (ds1.w + ds1.h) * CH as usize + (WALL_HEAD + CW) as usize;
    let origin_x = ds1.h as i32 * CW;
    let origin_y = WALL_HEAD;
    let mut stats = ComposeStats::default();

    // --- pass 1: pixels -------------------------------------------------
    let mut buf: Vec<u8> = vec![0u8; width * height];
    let mut px = Painter {
        idx: &mut buf,
        width,
        height,
        origin_x,
        origin_y,
    };
    px.draw_layer(ds1, tilesets, 4, &mut stats); // main floor
    px.draw_layer(ds1, tilesets, 0, &mut stats); // streets
    px.draw_layer(ds1, tilesets, 2, &mut stats); // rare (stones)
    px.draw_layer(ds1, tilesets, 5, &mut stats); // rare (warp)
    px.draw_layer(ds1, tilesets, 1, &mut stats); // walls / trees / objects
    drop(px); // release the borrow before buf moves into the map

    // --- pass 2: walkability grid --------------------------------------
    let (grid_w, grid_h) = (ds1.w * SUB, ds1.h * SUB);
    let mut walk = vec![false; grid_w * grid_h];
    // Floor layers open their cells…
    for li in [4usize, 0, 2, 5] {
        for_cell_tiles(ds1, tilesets, li, |cx, cy, tile| {
            if tile.is_floor {
                open_cell(&mut walk, grid_w, cx, cy, true);
                for b in &tile.blocks {
                    if b.flags & FLOOR_BLOCK_BITS != 0 {
                        block_region(&mut walk, grid_w, cx, cy, b.a, b.b, 32, 8);
                    }
                }
            }
        });
    }
    // …then wall-kind tiles (any layer they appear on) block their cell.
    for li in [4usize, 0, 1, 2, 5] {
        for_cell_tiles(ds1, tilesets, li, |cx, cy, tile| {
            if !tile.is_floor {
                open_cell(&mut walk, grid_w, cx, cy, false);
            }
        });
    }

    ComposedMap { width, height, origin_x, origin_y, idx: buf, grid_w, grid_h, walk, stats }
}

/// Iterate the valid, visible cells of layer `li` with their tile.
fn for_cell_tiles(
    ds1: &Ds1,
    tilesets: &[Dt1Tile],
    li: usize,
    mut f: impl FnMut(usize, usize, &Dt1Tile),
) {
    let cells = &ds1.layers[li];
    for cy in 0..ds1.h {
        for cx in 0..ds1.w {
            let v = cells[cy * ds1.w + cx];
            let (p1, p2, _p3, orient) = cell_fields(v);
            if p1 == 0 || orient & 0x80 != 0 {
                continue;
            }
            let index = cell_tile_index(p1, p2);
            if index >= tilesets.len() {
                continue;
            }
            let tile = if tilesets[index].blocks.is_empty() {
                &tilesets[p1 as usize]
            } else {
                &tilesets[index]
            };
            f(cx, cy, tile);
        }
    }
}

/// Set/unset the 5×5 sub-tiles of cell `(cx, cy)`.
fn open_cell(walk: &mut [bool], grid_w: usize, cx: usize, cy: usize, val: bool) {
    for j in 0..SUB {
        let row = cy * SUB + j;
        for i in 0..SUB {
            let col = cx * SUB + i;
            if let Some(slot) = walk.get_mut(row * grid_w + col) {
                *slot = val;
            }
        }
    }
}

/// Mark the sub-tiles covered by a pixel rect `(x, y, w, h)` local to the
/// cell's 80×40 diamond as unwalkable (sub-tiles are 16×8 px).
fn block_region(
    walk: &mut [bool],
    grid_w: usize,
    cx: usize,
    cy: usize,
    bx: i16,
    by: i16,
    bw: i32,
    bh: i32,
) {
    let x0 = (bx.max(0) as i32) / 16;
    let y0 = (by.max(0) as i32) / 8;
    let x1 = ((bx as i32 + bw).min(80) - 1).max(0) / 16;
    let y1 = ((by as i32 + bh).min(40) - 1).max(0) / 8;
    for sy in y0..=y1 {
        for sx in x0..=x1 {
            let col = cx * SUB + sx as usize;
            let row = cy * SUB + sy as usize;
            if let Some(slot) = walk.get_mut(row * grid_w + col) {
                *slot = false;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transforms_round_trip() {
        let m = ComposedMap {
            width: 0,
            height: 0,
            origin_x: 3280,
            origin_y: 420,
            idx: vec![],
            grid_w: 0,
            grid_h: 0,
            walk: vec![],
            stats: ComposeStats::default(),
        };
        let (sx, sy) = m.sub_to_screen(10.0, 20.0);
        assert_eq!(sx, (10.0 - 20.0) * 16.0 + 3280.0);
        assert_eq!(sy, (10.0 + 20.0) * 8.0 + 420.0);
        let (wx, wy) = m.screen_to_sub(sx, sy);
        assert!((wx - 10.0).abs() < 1e-4 && (wy - 20.0).abs() < 1e-4);
    }

    #[test]
    fn cell_screen_matches_prototype() {
        // cell (0,0) paints at (ox, oy); cell (1,1) at (ox, oy+80).
        let m = ComposedMap {
            width: 0,
            height: 0,
            origin_x: 3280,
            origin_y: 420,
            idx: vec![],
            grid_w: 0,
            grid_h: 0,
            walk: vec![],
            stats: ComposeStats::default(),
        };
        let (sx, sy) = m.sub_to_screen(5.0, 5.0); // sub-tile corner of cell (1,1)
        assert_eq!(sx, 3280.0);
        assert_eq!(sy, 420.0 + 80.0);
    }
}
