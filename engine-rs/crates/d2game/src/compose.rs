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
//!
//! Two more engine features live here (Go parity, see
//! `docs/parity/od2-features-moteur.md` #9 and #16):
//!
//! * **Seeded tile variation** (`compose_seeded`, Go `map_tile.go:85
//!   getRandomTile`): every painted FLOOR cell picks its tile among the
//!   candidates of its family window — the tiles `p1..=p1+15` that are
//!   floors sharing the resolved tile's sub-block flag signature. Walls
//!   are structural (each index is a distinct piece: left/right/corner)
//!   and are never re-rolled. Deterministic for a given seed; the walk
//!   grid stays keyed to the DS1's explicit variant.
//! * **Two-layer occlusion**: wall-kind tiles whose pixels rise above
//!   the cell diamond (any block with `b < 0` — trees, fences, cliffs;
//!   the DT1/DS1 orientation bytes being unreliable in this release, the
//!   test is structural, see `dt1.rs` header) go to a second index
//!   framebuffer, [`ComposedMap::overlay`], that `game.rs` blits AFTER
//!   the y-sorted entities — a wall masks the hero walking behind it
//!   (simplified Go `renderer.go:231-358` passes 2/3: entities-behind,
//!   then tall walls). Short walls stay in the base buffer.

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
    /// Wall-kind tiles routed to the overlay buffer (tall walls).
    pub tall: usize,
    /// FIX-CAMPES : cellules réelles sans candidate `(type, style, seq)` —
    /// 0 attendu sur la ville 1 (validé : 2337/2337 sols, 346/346 murs).
    pub unresolved: usize,
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
    /// Tall-wall layer, same shape as `idx` — composed walls whose pixels
    /// rise above their cell diamond. Blitted AFTER the entities by
    /// `game.rs::render` (occlusion); 0 = transparent.
    pub overlay: Vec<u8>,
    /// Collision grid width — `w * 5` sub-tiles.
    pub grid_w: usize,
    /// Collision grid height — `h * 5` sub-tiles.
    pub grid_h: usize,
    /// `grid_h * grid_w` booleans, row-major. `true` = walkable.
    pub walk: Vec<bool>,
    pub stats: ComposeStats,
    /// FIX-CAMPES : point d'apparition du camp (sous-tuile), calculé depuis
    /// le centroïde des tentes (kind 15) résolues du layout réel — l'ouverture
    /// du jeu démarre DANS le camp des Rogues, pas au centre de la carte.
    /// `None` = pas de layout réel (DRLG/DS1 historique).
    pub camp_spawn: Option<(usize, usize)>,
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

/// LCG déterministe — le RNG seedé historique du spawn des monstres
/// (game.rs l'importe sous le nom `LevelRng`) et de la variation des
/// tuiles. Indépendant du rng d2rules : le contenu d'un niveau ne dépend
/// pas de l'historique de combat.
pub struct Lcg(u64);

impl Lcg {
    #[inline]
    pub fn new(seed: u64) -> Self {
        Lcg(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    #[inline]
    pub fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    #[inline]
    pub fn below(&mut self, n: u32) -> u32 {
        ((self.next() >> 33) as u32) % n.max(1)
    }
}

struct Painter<'a> {
    idx: &'a mut [u8],
    overlay: &'a mut [u8],
    width: usize,
    height: usize,
    origin_x: i32,
    origin_y: i32,
    /// Per-tileset-index tallness (`tile_is_tall`), precomputed.
    tall_of: &'a [bool],
}

impl<'a> Painter<'a> {
    fn draw_floor_block(buf: &mut [u8], w: usize, h: usize, dst_x: i32, dst_y: i32, data: &[u8]) {
        for (i, &v) in data.iter().enumerate() {
            if v == 0 {
                continue;
            }
            let x = dst_x + (i & 31) as i32;
            let y = dst_y + (i >> 5) as i32;
            if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                buf[y as usize * w + x as usize] = v;
            }
        }
    }

    fn draw_wall_block(buf: &mut [u8], w: usize, h: usize, dst_x: i32, dst_y: i32, data: &[u8]) {
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
                if x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h {
                    buf[y as usize * w + x as usize] = v;
                }
            }
        }
    }

    /// Draw one layer, exactly the prototype's `draw_layer`, with the two
    /// engine extras: seeded floor variation and tall-wall routing to the
    /// overlay buffer.
    fn draw_layer(
        &mut self,
        ds1: &Ds1,
        tilesets: &[Dt1Tile],
        flags_sig: &[u64],
        li: usize,
        rng: &mut Lcg,
        stats: &mut ComposeStats,
    ) {
        let cells = &ds1.layers[li];
        let mut cands: Vec<usize> = Vec::new();
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
                let mut index = if tilesets[index].blocks.is_empty() {
                    p1 as usize
                } else {
                    index
                };
                // --- variation seedée (Go map_tile.go:85 getRandomTile) --
                // SOL uniquement : tire parmi la fenêtre famille p1..=p1+15
                // les candidates de même nature et de même signature de
                // flags (la grille de collision est calculée sur la tuile
                // déterministe du DS1 — la variation ne doit pas la
                // contredire). Murs structurels : jamais re-tirés.
                if tilesets[index].is_floor {
                    let sig = flags_sig[index];
                    floor_candidates(tilesets, flags_sig, p1 as usize, sig, &mut cands);
                    if cands.len() > 1 {
                        index = cands[rng.below(cands.len() as u32) as usize];
                    }
                }
                let tall = self.tall_of[index];
                let sx = (cx as i32 - cy as i32) * CW + self.origin_x;
                let sy = (cx as i32 + cy as i32) * CH + self.origin_y;
                let (buf, w, h): (&mut [u8], usize, usize) = if tall {
                    stats.tall += 1;
                    (self.overlay, self.width, self.height)
                } else {
                    (self.idx, self.width, self.height)
                };
                for b in &tilesets[index].blocks {
                    if tilesets[index].is_floor {
                        Self::draw_floor_block(buf, w, h, sx + b.a as i32, sy + b.b as i32, &b.data);
                    } else {
                        Self::draw_wall_block(buf, w, h, sx + b.a as i32, sy + b.b as i32, &b.data);
                    }
                }
                stats.drawn += 1;
            }
        }
    }
}

/// A tile is "tall" — occluding — when any of its placed blocks carries
/// real pixels above the cell diamond top (`b < 0`). Trees, fences,
/// cliffs… The DS1/DT1 orientation bytes are unreliable in this release
/// (see `dt1.rs` header), so the test is structural.
fn tile_is_tall(tile: &Dt1Tile) -> bool {
    tile.blocks.iter().any(|b| b.b < 0 && b.data.iter().any(|&v| v != 0))
}

/// FNV-1a 64 of a tile's sub-block flag sequence — two tiles with the
/// same signature are interchangeable for the collision grid (computed
/// from the DS1's deterministic tile), so variation may swap them freely.
fn flags_signature(tile: &Dt1Tile) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    };
    for b in &tile.blocks {
        mix(b.flags as u8);
        mix((b.flags >> 8) as u8);
    }
    h
}

/// The variation candidates of family window `p1..=p1+15`: existing floor
/// tiles sharing `sig` (the RESOLVED tile's signature — the collision grid
/// is computed from it). The resolved tile itself is always among them
/// (it is a floor of that window with its own signature).
fn floor_candidates(
    tilesets: &[Dt1Tile],
    flags_sig: &[u64],
    base: usize,
    sig: u64,
    out: &mut Vec<usize>,
) {
    out.clear();
    for k in base..(base + 16).min(tilesets.len()) {
        if tilesets[k].is_floor && flags_sig[k] == sig {
            out.push(k);
        }
    }
}

/// Floor sub-block flags that mark a blocked sub-region even on floor
/// tiles (the rare `0x2005`-family blocks — 24 in the whole town).
const FLOOR_BLOCK_BITS: u16 = 0x2000;

/// Compose the whole map: index framebuffer + collision grid. Fixed tile
/// seed (see [`compose_seeded`] for the seeded variation).
pub fn compose(ds1: &Ds1, tilesets: &[Dt1Tile]) -> ComposedMap {
    compose_seeded(ds1, tilesets, 0)
}

/// [`compose`] with a seed: floor tiles are picked among their family
/// candidates with the seeded RNG (Go `getRandomTile`) — same seed, same
/// map.
pub fn compose_seeded(ds1: &Ds1, tilesets: &[Dt1Tile], seed: u64) -> ComposedMap {
    let width = (ds1.w + ds1.h) * CW as usize;
    let height = (ds1.w + ds1.h) * CH as usize + (WALL_HEAD + CW) as usize;
    let origin_x = ds1.h as i32 * CW;
    let origin_y = WALL_HEAD;
    let mut stats = ComposeStats::default();

    let tall_of: Vec<bool> = tilesets.iter().map(tile_is_tall).collect();
    let flags_sig: Vec<u64> = tilesets.iter().map(flags_signature).collect();

    // --- pass 1: pixels -------------------------------------------------
    let mut buf: Vec<u8> = vec![0u8; width * height];
    let mut overlay: Vec<u8> = vec![0u8; width * height];
    let mut rng = Lcg::new(seed);
    {
        let mut px = Painter {
            idx: &mut buf,
            overlay: &mut overlay,
            width,
            height,
            origin_x,
            origin_y,
            tall_of: &tall_of,
        };
        px.draw_layer(ds1, tilesets, &flags_sig, 4, &mut rng, &mut stats); // main floor
        px.draw_layer(ds1, tilesets, &flags_sig, 0, &mut rng, &mut stats); // streets
        px.draw_layer(ds1, tilesets, &flags_sig, 2, &mut rng, &mut stats); // rare (stones)
        px.draw_layer(ds1, tilesets, &flags_sig, 5, &mut rng, &mut stats); // rare (warp)
        px.draw_layer(ds1, tilesets, &flags_sig, 1, &mut rng, &mut stats); // walls / trees / objects
    }

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

    ComposedMap {
        width,
        height,
        origin_x,
        origin_y,
        idx: buf,
        overlay,
        grid_w,
        grid_h,
        walk,
        stats,
        camp_spawn: None,
    }
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
            overlay: vec![],
            grid_w: 0,
            grid_h: 0,
            walk: vec![],
            stats: ComposeStats::default(),
            camp_spawn: None,
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
            overlay: vec![],
            grid_w: 0,
            grid_h: 0,
            walk: vec![],
            stats: ComposeStats::default(),
            camp_spawn: None,
        };
        let (sx, sy) = m.sub_to_screen(5.0, 5.0); // sub-tile corner of cell (1,1)
        assert_eq!(sx, 3280.0);
        assert_eq!(sy, 420.0 + 80.0);
    }

    // --- fixtures -----------------------------------------------------------

    /// One floor tile painting a solid 32×40 rect at the cell origin,
    /// palette index `idx` (index 0 pixels are transparent by policy).
    fn floor_tile(idx: u8) -> Dt1Tile {
        Dt1Tile { is_floor: true, blocks: vec![block(idx, 0, 0, 32 * 40)] }
    }

    /// One wall block: `rows` rows of 32 px starting `b` px above the cell
    /// top (b < 0 = rises above the diamond = tall / occluding).
    fn wall_tile(idx: u8, b: i16, rows: usize) -> Dt1Tile {
        Dt1Tile { is_floor: false, blocks: vec![block(idx, 0, b, rows * 32)] }
    }

    fn block(idx: u8, a: i16, b: i16, len: usize) -> crate::dt1::Dt1Block {
        crate::dt1::Dt1Block { a, b, flags: 0, data: vec![idx; len] }
    }

    /// A v18-shaped DS1 with real dimensions `w×h`; `cells[li]` fills layer
    /// `li` (one packed cell value per position, row-major).
    fn ds1_fixture(w: usize, h: usize, cells: &[(usize, Vec<i32>)]) -> Ds1 {
        let mut layers = vec![vec![0i32; w * h]; 6];
        for (li, v) in cells {
            layers[*li] = v.clone();
        }
        Ds1 { version: 18, w, h, files: vec!["test.tg1".into()], layers }
    }

    // --- occlusion (feature 16, Go renderer.go:231-358 passes 2/3) ---------

    /// `0` = filler (jamais adressé : p1 = 0 est une cellule vide), puis
    /// 1: sol, 2: mur bas, 3: mur haut.
    fn occl_tilesets() -> Vec<Dt1Tile> {
        vec![Dt1Tile::default(), floor_tile(10), wall_tile(30, 0, 4), wall_tile(40, -160, 200)]
    }

    #[test]
    fn tall_wall_overlays_entity_cell() {
        let tilesets = occl_tilesets();
        let mut cells = vec![0i32; 16];
        cells[1 * 4 + 1] = 1; // floor at cell (1,1) — hero position
        let mut l1 = vec![0i32; 16];
        l1[2 * 4 + 2] = 3; // tall wall at cell (2,2) — in front (sum 4 > 2)
        let ds1 = ds1_fixture(4, 4, &[(4, cells), (1, l1)]);
        let map = compose_seeded(&ds1, &tilesets, 7);

        let px = |buf: &[u8], x: i32, y: i32| buf[y as usize * map.width + x as usize] as u32;
        let (ox, oy) = (map.origin_x, map.origin_y);
        // The wall at (2,2) paints x∈[ox,ox+32), y∈[oy,oy+200): the anchor
        // of an entity standing on cell (1,1) — (ox, oy+88) — is inside it.
        let (hx, hy) = (ox, oy + 88);

        // Layer 1 (base): the floor behind shows — an entity drawn there
        // was VISIBLE in the old single-buffer renderer.
        assert_eq!(px(&map.idx, hx, hy), 10, "sol visible en couche 1");
        assert_eq!(px(&map.idx, ox, oy + 20), 0, "le mur haut n'est PAS dans le tampon de base");
        // Layer 2 (overlay): the same pixel is covered by the wall — after
        // the entity blit, the wall masks it.
        assert_eq!(px(&map.overlay, hx, hy), 40, "mur haut par-dessus en couche 2");
        assert_eq!(px(&map.overlay, ox, oy + 20), 40);
        assert_eq!(map.stats.tall, 1);
        // The wall cell stays blocked in the grid.
        assert!(!map.walkable(2 * 5 + 2, 2 * 5 + 2));
    }

    #[test]
    fn short_wall_stays_in_base() {
        let tilesets = occl_tilesets();
        let mut l1 = vec![0i32; 16];
        l1[2 * 4 + 2] = 2; // short wall (all b >= 0)
        let ds1 = ds1_fixture(4, 4, &[(1, l1)]);
        let map = compose_seeded(&ds1, &tilesets, 7);
        let (ox, oy) = (map.origin_x, map.origin_y);
        let at = |buf: &[u8], x: i32, y: i32| buf[y as usize * map.width + x as usize];
        assert_eq!(at(&map.idx, ox, oy + 161), 30, "mur bas peint dans la base");
        assert!(map.overlay.iter().all(|&v| v == 0), "rien dans l'overlay");
        assert_eq!(map.stats.tall, 0);
    }

    // --- variation seedée (feature 9, Go map_tile.go:85 getRandomTile) -----

    /// 4×4 map of floors from family base `p1`, variant nibble `p2`.
    fn floors_ds1(p1: u8, p2: u8) -> Ds1 {
        let cells = vec![(p1 as i32) | ((p2 as i32) << 8); 16];
        ds1_fixture(4, 4, &[(4, cells)])
    }

    #[test]
    fn variation_is_deterministic_per_seed() {
        // Window of p1=1: 4 same-signature floor variants → re-rolled.
        let tilesets = vec![
            Dt1Tile::default(),
            floor_tile(10),
            floor_tile(11),
            floor_tile(12),
            floor_tile(13),
        ];
        let ds1 = floors_ds1(1, 0);
        let a = compose_seeded(&ds1, &tilesets, 42);
        let b = compose_seeded(&ds1, &tilesets, 42);
        assert_eq!(a.idx, b.idx, "même graine ⇒ même carte");
        let c = compose_seeded(&ds1, &tilesets, 43);
        assert_ne!(a.idx, c.idx, "graines différentes ⇒ carte différente");
        // Every painted pixel is one of the family's variants.
        assert!(
            a.idx.iter().filter(|&&v| v != 0).all(|&v| (10..=13).contains(&v)),
            "seuls des candidats de la famille sont peints"
        );
    }

    #[test]
    fn variation_never_picks_out_of_window_or_kind() {
        // Window of p1=1: a short WALL at index 2 (excluded: kind) and a
        // floor of another flag signature at index 3 (excluded: flags) —
        // only variant 1 is a candidate, so the pick is deterministic.
        let mut diff_flags = floor_tile(11);
        diff_flags.blocks[0].flags = 0x2000;
        let tilesets = vec![Dt1Tile::default(), floor_tile(10), wall_tile(99, 0, 4), diff_flags];
        let map = compose_seeded(&floors_ds1(1, 0), &tilesets, 9);
        assert!(
            map.idx.iter().all(|&v| v == 0 || v == 10),
            "jamais 11 (flags) ni 99 (mur) : {}",
            map.idx.iter().filter(|&&v| v != 0).collect::<Vec<_>>().len()
        );
        assert_eq!(map.stats.drawn, 16);
    }

    #[test]
    fn walls_are_never_rerolled() {
        // Two tall-wall variants in the window: the wall cell keeps the
        // DS1's explicit tile whatever the seed (walls are structural).
        let tilesets = vec![Dt1Tile::default(), wall_tile(40, -64, 8), wall_tile(41, -64, 8)];
        let mut l1 = vec![0i32; 16];
        l1[1 * 4 + 1] = 1; // p2 = 0 → tile 1 explicitly
        let ds1 = ds1_fixture(4, 4, &[(1, l1)]);
        for seed in [0u64, 1, 12345] {
            let map = compose_seeded(&ds1, &tilesets, seed);
            assert!(
                map.overlay.iter().all(|&v| v == 0 || v == 40),
                "mur re-tiré au seed {seed}"
            );
        }
    }

    #[test]
    fn explicit_variant_still_respected_without_candidates() {
        // p2 nibble = 2 → tile 3 is the DS1's explicit pick; the other
        // window floors carry another flag signature so it is the ONLY
        // candidate — no RNG, the DS1's choice stands whatever the seed.
        let other_sig = |k: u8| {
            let mut t = floor_tile(10 + k);
            t.blocks[0].flags = 1;
            t
        };
        let tilesets = vec![Dt1Tile::default(), other_sig(0), other_sig(1), floor_tile(12)];
        for seed in [0u64, 7, 999] {
            let map = compose_seeded(&floors_ds1(1, 2), &tilesets, seed);
            assert!(
                map.idx.iter().all(|&v| v == 0 || v == 12),
                "variante explicite seule candidate (seed {seed})"
            );
        }
    }
}

/// FIX-CAMPES : composition du layout RÉEL (ville 1) — la règle du jeu :
/// sols = candidats `(style, seq)` parmi les tuiles `kind == 0` ; murs =
/// candidats `(kind == code d'orientation, style, seq)` ; `prop % candidats`
/// choisit la variation ; `hidden` = cellule non dessinée ; flux d'ombre
/// jamais dessiné. Validé sur TownN1 : 2337/2337 sols, 346/346 murs,
/// 43 cachées, 19 tentes (kind 15), 16 feux (12), 118 arbres (14).
pub fn compose_real_town(
    real: &crate::ds1_real::Ds1Real,
    tilesets: &[Dt1Tile],
    metas: &[crate::dt1::Dt1Meta],
    seed: u64,
) -> ComposedMap {
    use std::collections::HashMap;
    let _ = seed; // la variation vient du `prop` du DS1 réel, pas du RNG

    let width = (real.w + real.h) * CW as usize;
    let height = (real.w + real.h) * CH as usize + (WALL_HEAD + CW) as usize;
    let origin_x = real.h as i32 * CW;
    let origin_y = WALL_HEAD;
    let mut stats = ComposeStats::default();

    let tall_of: Vec<bool> = tilesets.iter().map(tile_is_tall).collect();

    // Candidats par clé — construits UNE fois (2337 cellules × 2-3 flux).
    let mut floors: HashMap<(i32, i32), Vec<usize>> = HashMap::new();
    let mut walls: HashMap<(i32, i32, i32), Vec<usize>> = HashMap::new();
    for (k, m) in metas.iter().enumerate() {
        if m.kind == 0 {
            floors.entry((m.style, m.seq)).or_default().push(k);
        } else {
            walls.entry((m.kind, m.style, m.seq)).or_default().push(k);
        }
    }

    let mut buf: Vec<u8> = vec![0u8; width * height];
    let mut overlay: Vec<u8> = vec![0u8; width * height];
    let walk = vec![false; real.w * SUB * real.h * SUB];
    let grid_w = real.w * SUB;
    let mut walk = walk;
    let mut tents: Vec<(f32, f32)> = Vec::new();
    {
        let mut px = Painter {
            idx: &mut buf,
            overlay: &mut overlay,
            width,
            height,
            origin_x,
            origin_y,
            tall_of: &tall_of,
        };
        let mut draw_tile = |px: &mut Painter, k: usize, cx: usize, cy: usize, stats: &mut ComposeStats| {
            let sx = (cx as i32 - cy as i32) * CW + origin_x;
            let sy = (cx as i32 + cy as i32) * CH + origin_y;
            let (b, w, h): (&mut [u8], usize, usize) = if px.tall_of[k] {
                stats.tall += 1;
                (px.overlay, px.width, px.height)
            } else {
                (px.idx, px.width, px.height)
            };
            for blk in &tilesets[k].blocks {
                if tilesets[k].is_floor {
                    Painter::draw_floor_block(b, w, h, sx + blk.a as i32, sy + blk.b as i32, &blk.data);
                } else {
                    Painter::draw_wall_block(b, w, h, sx + blk.a as i32, sy + blk.b as i32, &blk.data);
                }
            }
            stats.drawn += 1;
        };

        // --- sols (flux floor) ------------------------------------------------
        for cy in 0..real.h {
            for cx in 0..real.w {
                let i = cy * real.w + cx;
                let c = real.floor[i];
                if c.prop == 0 {
                    stats.empty += 1;
                    continue;
                }
                if c.hidden {
                    stats.hidden += 1;
                    continue;
                }
                match floors.get(&(c.style as i32, c.seq as i32)) {
                    Some(cands) if !cands.is_empty() => {
                        let k = cands[c.prop as usize % cands.len()];
                        draw_tile(&mut px, k, cx, cy, &mut stats);
                        open_cell(&mut walk, grid_w, cx, cy, true);
                        for b in &tilesets[k].blocks {
                            if b.flags & FLOOR_BLOCK_BITS != 0 {
                                block_region(&mut walk, grid_w, cx, cy, b.a, b.b, 32, 8);
                            }
                        }
                    }
                    _ => stats.unresolved += 1,
                }
            }
        }

        // --- murs (wall1 puis wall2, code d'orientation = kind) ---------------
        for (wall, orient) in [(&real.wall1, &real.orient1), (&real.wall2, &real.orient2)] {
            for cy in 0..real.h {
                for cx in 0..real.w {
                    let i = cy * real.w + cx;
                    let o = orient[i];
                    if o == 0 {
                        continue;
                    }
                    let c = wall[i];
                    if c.prop == 0 {
                        stats.empty += 1;
                        continue;
                    }
                    if c.hidden {
                        stats.hidden += 1;
                        continue;
                    }
                    match walls.get(&(o as i32, c.style as i32, c.seq as i32)) {
                        Some(cands) if !cands.is_empty() => {
                            let k = cands[c.prop as usize % cands.len()];
                            if o == 15 {
                                tents.push((cx as f32, cy as f32));
                            }
                            draw_tile(&mut px, k, cx, cy, &mut stats);
                            open_cell(&mut walk, grid_w, cx, cy, false);
                        }
                        _ => stats.unresolved += 1,
                    }
                }
            }
        }
    }
    // Flux d'ombre : jamais dessiné, aucun effet de marche (rendu lumière du
    // jeu original) — `real.shadow` volontairement non consommé ici.

    // Apparition du camp : centroïde des tentes → sous-tuile centrale la
    // plus proche marchable (le héros démarre au milieu des tentes, comme
    // dans le jeu réel).
    let camp_spawn = if tents.is_empty() {
        None
    } else {
        let n = tents.len() as f32;
        let tcx = (tents.iter().map(|t| t.0).sum::<f32>() / n).round() as usize;
        let tcy = (tents.iter().map(|t| t.1).sum::<f32>() / n).round() as usize;
        find_nearby_walkable(&walk, grid_w, real.h * SUB, tcx * SUB + 2, tcy * SUB + 2, 12)
    };

    ComposedMap {
        width,
        height,
        origin_x,
        origin_y,
        idx: buf,
        overlay,
        grid_w,
        grid_h: real.h * SUB,
        walk,
        stats,
        camp_spawn,
    }
}
