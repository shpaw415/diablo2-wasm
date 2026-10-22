//! The game loop core — native, no WASM types, fully testable.
//!
//! Flow: JS fetches the manifest, then every asset it lists, feeds them
//! with [`GameCore::provide`], and calls [`GameCore::finalize`], which
//! composes the map (index framebuffer + collision grid), builds the
//! sprite sets, and constructs the contract `World`. Then it's
//! [`GameCore::tick`] (events out) and [`GameCore::render`] (RGBA
//! viewport, camera centered on the hero) once per frame.
//!
//! Draw order per frame: floor map → shadows → items/monsters/hero
//! sorted by screen y. Corpses (`anim == Dd`) are skipped (no DD frames
//! in the extracted assets yet).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::compose::{self, ComposedMap};
use crate::ds1;
use crate::dt1::{self, Dt1Tile};
use crate::pal::MapPalette;
use crate::sprite::{sprite_palette, SpriteFrame, SpriteSet};
use crate::world::{AnimState, Entity, GameEvent, PlayerInput, World};

pub const DEFAULT_VIEWPORT_W: u32 = 800;
pub const DEFAULT_VIEWPORT_H: u32 = 600;

pub struct GameCore {
    manifest: Value,
    assets: BTreeMap<String, Vec<u8>>,
    map: Option<ComposedMap>,
    map_rgba: [u8; 256 * 4], // palette-index → RGBA, precomputed for blits
    sprite_pal: d2dcc::render::Palette,
    sprites: BTreeMap<String, SpriteSet>,
    world: Option<World>,
    /// Héros demandé ("BA"|"SOR"|"PA"…, normalisé uppercase) — résolu contre
    /// `manifest.heroes` (retombée `hero` à l'ancienne via d2rules).
    hero_request: String,
    vw: u32,
    vh: u32,
    cam_x: f32,
    cam_y: f32,
    fb: Vec<u8>,
    finalized: bool,
}

impl GameCore {
    /// `manifest_json` — le `assets/manifest.json` brut ; `hero` — le code de
    /// classe choisi (`"BA"`, `"SOR"`, `"PA"`…, insensible à la casse, "BA" par
    /// défaut). Le sprite et les stats du héros viennent TOUS DEUX de
    /// `manifest.heroes[code]` si présent, sinon du bloc `hero` (rétro-compat).
    pub fn new(manifest_json: &str, hero: &str) -> Result<GameCore, String> {
        let manifest: Value =
            serde_json::from_str(manifest_json).map_err(|e| format!("manifest JSON: {e}"))?;
        let hero_request = {
            let t = hero.trim().to_ascii_uppercase();
            if t.is_empty() { "BA".to_string() } else { t }
        };
        Ok(GameCore {
            manifest,
            assets: BTreeMap::new(),
            map: None,
            map_rgba: [0; 256 * 4],
            sprite_pal: sprite_palette(&[]),
            sprites: BTreeMap::new(),
            world: None,
            hero_request,
            vw: DEFAULT_VIEWPORT_W,
            vh: DEFAULT_VIEWPORT_H,
            cam_x: 0.0,
            cam_y: 0.0,
            fb: Vec::new(),
            finalized: false,
        })
    }

    /// Register one asset blob under its manifest path (case-insensitive;
    /// the lowercase basename also indexes it for `.tg1` → DT1 mapping).
    pub fn provide(&mut self, path: &str, bytes: Vec<u8>) {
        let lower = path.to_ascii_lowercase();
        let base = lower.rsplit('/').next().unwrap_or(&lower).to_string();
        self.assets.insert(lower, bytes.clone());
        self.assets.entry(base).or_insert(bytes);
    }

    fn asset(&self, path: &str) -> Result<&Vec<u8>, String> {
        let lower = path.to_ascii_lowercase();
        self.assets
            .get(&lower)
            .or_else(|| self.assets.get(lower.rsplit('/').next().unwrap_or("")))
            .ok_or_else(|| format!("missing asset: {path} (provide it before finalize)"))
    }

    fn str_at(v: &Value, pointer: &str) -> Result<String, String> {
        v.pointer(pointer)
            .and_then(|s| s.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| format!("manifest: {pointer} missing"))
    }

    /// Compose the map, build sprites, construct the world.
    pub fn finalize(&mut self) -> Result<(), String> {
        // --- palette -----------------------------------------------------
        let pal_path = Self::str_at(&self.manifest, "/level/palette")?;
        let raw_pal = self.asset(&pal_path)?.clone();
        // Gamma 0.7 = the validated `level-render-v2.png` look on the
        // corrected PL2 palette (MAP-SPEC §7; 0.55 was tuned on the broken
        // R/B-swapped pal.dat).
        self.map_rgba = build_map_rgba_lut(&raw_pal, 0.7);
        self.sprite_pal = sprite_palette(&raw_pal);

        // --- tilesets in DS1 `.tg1` order --------------------------------
        let ds1_path = Self::str_at(&self.manifest, "/level/ds1")?;
        let ds1_bytes = self.asset(&ds1_path)?.clone();
        let ds1 = ds1::decode_ds1(&ds1_bytes).map_err(|e| format!("{ds1_path}: {e:?}"))?;
        let mut tilesets: Vec<Dt1Tile> = Vec::new();
        for tg1 in &ds1.files {
            let container = tg1_to_container(tg1);
            let bytes = self.asset(&container)?.clone();
            let mut tiles = dt1::decode_dt1(&bytes).map_err(|e| format!("{container}: {e:?}"))?;
            tilesets.append(&mut tiles);
        }

        // --- map ----------------------------------------------------------
        let map = compose::compose(&ds1, &tilesets);
        let walk_rows = map.walk_rows();
        self.cam_x = 0.0;
        self.cam_y = 0.0;
        self.map = Some(map);

        // --- sprites ------------------------------------------------------
        // Héros : bloc walk de `heroes[code]` si le code demandé y existe,
        // sinon `hero.walk` (rétro-compat) — même règle de résolution que
        // d2rules (resolve_hero_code), sprite et stats toujours en phase.
        let hero_code = d2rules::resolve_hero_code(&self.manifest, &self.hero_request);
        let hero_walk = self
            .manifest
            .pointer(&format!("/heroes/{hero_code}/walk"))
            .unwrap_or(&self.manifest["hero"]["walk"]);
        let mut sprites = BTreeMap::new();
        sprites.insert("hero".into(), self.build_sprite_set(hero_walk)?);
        if let Some(list) = self.manifest.get("monsters").and_then(|m| m.as_array()) {
            for m in list {
                let code = m.get("code").and_then(|c| c.as_str()).unwrap_or("??").to_string();
                if let Some(walk) = m.get("walk") {
                    match self.build_sprite_set(walk) {
                        Ok(set) => {
                            sprites.insert(code, set);
                        }
                        Err(e) => return Err(format!("monster {code}: {e}")),
                    }
                }
            }
        }
        self.sprites = sprites;

        // --- world (contract) ----------------------------------------------
        // Polarity: compose yields `true` = WALKABLE, the d2rules contract
        // takes `true` = BLOCKED — invert per-cell (see world.rs doc).
        let blocked = walk_rows
            .into_iter()
            .map(|row| row.into_iter().map(|walkable| !walkable).collect())
            .collect();
        self.world = Some(World::with_hero(blocked, self.manifest.clone(), &self.hero_request));

        let (hw, hh) = (self.vw as usize * 4, self.vh as usize);
        self.fb = vec![0; hw * hh];
        self.finalized = true;
        Ok(())
    }

    fn build_sprite_set(&self, walk: &Value) -> Result<SpriteSet, String> {
        let cof_path = walk
            .get("cof")
            .and_then(|c| c.as_str())
            .ok_or("manifest: walk.cof missing")?;
        let cof = Cof::parse(self.asset(cof_path)?).map_err(|e| format!("{cof_path}: {e:?}"))?;
        let mut set = SpriteSet::new(cof);
        if let Some(layers) = walk.get("layers").and_then(|l| l.as_object()) {
            for (token, path) in layers {
                let path = path.as_str().ok_or("manifest: layer path not a string")?;
                let dcc = Dcc::decode(self.asset(path)?).map_err(|e| format!("{path}: {e:?}"))?;
                set.add_layer(token, dcc);
            }
        }
        Ok(set)
    }

    pub fn set_viewport(&mut self, w: u32, h: u32) {
        self.vw = w.max(16);
        self.vh = h.max(16);
        if self.finalized {
            self.fb = vec![0; self.vw as usize * self.vh as usize * 4];
        }
    }

    /// Nearest alive monster within `radius` sub-tiles of a world point
    /// (for the JS click router: attack vs move). Index into monsters().
    pub fn monster_near(&self, wx: f32, wy: f32, radius: f32) -> Option<usize> {
        let world = self.world.as_ref()?;
        let mut best: Option<(f32, usize)> = None;
        for (i, m) in world.monsters().iter().enumerate() {
            if !world.monster_alive(i) {
                continue;
            }
            let d = ((m.x - wx).powi(2) + (m.y - wy).powi(2)).sqrt();
            if d <= radius && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Convert a canvas click (viewport px) to world sub-tile coords.
    pub fn screen_to_world(&self, px: f32, py: f32) -> (f32, f32) {
        let map = self.map.as_ref().expect("finalize first");
        map.screen_to_sub(px + self.cam_x, py + self.cam_y)
    }

    pub fn hero(&self) -> &Entity {
        self.world.as_ref().expect("finalize first").hero()
    }

    /// Hero position in viewport px (feet anchor), for the JS cursor UI.
    pub fn hero_screen(&self) -> (f32, f32) {
        let map = self.map.as_ref().expect("finalize first");
        let h = self.hero();
        let (sx, sy) = map.sub_to_screen(h.x, h.y);
        (sx - self.cam_x, sy - self.cam_y)
    }

    pub fn map_pixel_size(&self) -> (u32, u32) {
        let map = self.map.as_ref().expect("finalize first");
        (map.width as u32, map.height as u32)
    }

    /// The collision grid flattened as 0/1 bytes (for d2rules / debug UIs).
    pub fn collision(&self) -> Vec<u8> {
        let map = self.map.as_ref().expect("finalize first");
        map.walk.iter().map(|&w| w as u8).collect()
    }

    pub fn grid_size(&self) -> (u32, u32) {
        let map = self.map.as_ref().expect("finalize first");
        (map.grid_w as u32, map.grid_h as u32)
    }

    pub fn compose_stats(&self) -> ComposeStatsCopy {
        let map = self.map.as_ref().expect("finalize first");
        let s = map.stats;
        ComposeStatsCopy {
            drawn: s.drawn,
            hidden: s.hidden,
            empty: s.empty,
            out_of_bounds: s.out_of_bounds,
        }
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    /// Hero HUD stats as JSON (`HeroStats`), for the demo status/HUD.
    pub fn hero_stats_json(&self) -> String {
        match self.world.as_ref() {
            Some(w) => serde_json::to_string(w.hero_stats()).unwrap_or_else(|_| "{}".into()),
            None => "{}".into(),
        }
    }

    /// Classe du héros résolue, JSON `{"code":"BA","name":"Barbare"}` — pour le
    /// HUD (nom affiché) et l'E2E. Fonctionne avant comme après `finalize`.
    pub fn hero_class_json(&self) -> String {
        let code = d2rules::resolve_hero_code(&self.manifest, &self.hero_request);
        let name = self
            .manifest
            .pointer(&format!("/heroes/{code}/name"))
            .or_else(|| self.manifest.pointer("/hero/name"))
            .and_then(|n| n.as_str())
            .unwrap_or(&code);
        serde_json::json!({ "code": code, "name": name }).to_string()
    }

    /// Inventory (40 slots, `null` = empty) as JSON.
    pub fn inventory_json(&self) -> String {
        match self.world.as_ref() {
            Some(w) => serde_json::to_string(w.inventory()).unwrap_or_else(|_| "[]".into()),
            None => "[]".into(),
        }
    }

    /// Serialize the world (d2rules save format, JSON v1) for the demo's
    /// Save button. Empty string when not finalized.
    pub fn save_world(&self) -> String {
        self.world.as_ref().map(|w| w.save()).unwrap_or_default()
    }

    /// Replace the world with one deserialized from `json` (d2rules save).
    /// The map/sprites stay as-is (same level, same assets).
    pub fn load_world(&mut self, json: &str) -> Result<(), String> {
        let world = World::load(json).map_err(|()| "sauvegarde invalide ou version inconnue".to_string())?;
        // Sanity: the save's grid must match this map's (same level).
        if world.width() as usize != self.grid_size().0 as usize
            || world.height() as usize != self.grid_size().1 as usize
        {
            return Err("sauvegarde d'une autre carte".into());
        }
        self.world = Some(world);
        self.follow_camera();
        Ok(())
    }

    /// Advance the world, follow the hero with the camera, return events.
    pub fn tick(&mut self, dt_ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent> {
        let events = self
            .world
            .as_mut()
            .expect("finalize first")
            .tick(dt_ms, input);
        self.follow_camera();
        events
    }

    fn follow_camera(&mut self) {
        // Absolute hero screen position (hero_screen() is viewport-relative).
        let map = self.map.as_ref().unwrap();
        let h = *self.world.as_ref().unwrap().hero();
        let (sx, sy) = map.sub_to_screen(h.x, h.y);
        self.cam_x = (sx - self.vw as f32 / 2.0).clamp(0.0, (map.width - self.vw as usize) as f32);
        self.cam_y = (sy - self.vh as f32 / 2.0).clamp(0.0, (map.height - self.vh as usize) as f32);
    }

    /// Render one frame into the internal RGBA framebuffer; returns a view.
    pub fn render(&mut self) -> &[u8] {
        let map = self.map.as_ref().expect("finalize first");
        let (vw, vh) = (self.vw as usize, self.vh as usize);
        let lut = self.map_rgba;
        let mut fb = std::mem::take(&mut self.fb);

        // 1. floor: viewport blit of the index framebuffer through the LUT.
        for y in 0..vh {
            let dst = &mut fb[y * vw * 4..(y + 1) * vw * 4];
            let my = y + self.cam_y as usize;
            if my >= map.height {
                dst.fill(0);
                continue;
            }
            let x0 = self.cam_x as usize;
            let xs = x0..(x0 + vw).min(map.width);
            let row = &map.idx[my * map.width..my * map.width + map.width];
            let mut di = 0usize;
            for mx in xs.clone() {
                let p = row[mx] as usize * 4;
                dst[di..di + 4].copy_from_slice(&lut[p..p + 4]);
                di += 4;
            }
            // right margin (camera clamped): black
            dst[xs.len() * 4..vw * 4].fill(0);
        }

        // 2. entities: shadows first, then sprites sorted by screen y.
        let world = self.world.as_ref().unwrap();
        let mut drawlist: Vec<(f32, DrawKind)> = Vec::new();
        for (i, m) in world.monsters().iter().enumerate() {
            if m.anim != AnimState::DD {
                drawlist.push((m.y, DrawKind::Monster(i)));
            }
        }
        for (i, it) in world.items().iter().enumerate() {
            drawlist.push((it.y, DrawKind::Item(i)));
        }
        drawlist.push((world.hero().y, DrawKind::Hero));

        for (_, kind) in &drawlist {
            let (x, y) = self.entity_screen(*kind);
            shadow(&mut fb, vw, vh, x, y - 2.0, 16.0, 6.0, 70);
        }

        drawlist.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, kind) in &drawlist {
            match *kind {
                DrawKind::Hero => {
                    let h = *world.hero();
                    self.blit_entity(&mut fb, vw, vh, "hero", &h);
                }
                DrawKind::Monster(i) => {
                    let m = world.monsters()[i];
                    let key = self.monster_sprite_key(i);
                    self.blit_entity(&mut fb, vw, vh, &key, &m);
                }
                DrawKind::Item(i) => {
                    let (sx, sy) = self.entity_screen(DrawKind::Item(i));
                    item_marker(&mut fb, vw, vh, sx, sy);
                }
            }
        }

        self.fb = fb;
        &self.fb
    }

    fn monster_sprite_key(&self, index: usize) -> String {
        // Sprite sets are keyed by monster code; monsters spawn in
        // manifest order, so the index maps back to the manifest entry.
        self.manifest
            .get("monsters")
            .and_then(|m| m.as_array())
            .and_then(|arr| arr.get(index))
            .and_then(|m| m.get("code"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string()
    }

    fn entity_screen(&self, kind: DrawKind) -> (f32, f32) {
        let world = self.world.as_ref().unwrap();
        let map = self.map.as_ref().unwrap();
        let e = match kind {
            DrawKind::Hero => world.hero(),
            DrawKind::Monster(i) => &world.monsters()[i],
            DrawKind::Item(i) => {
                let it = &world.items()[i];
                let (sx, sy) = map.sub_to_screen(it.x, it.y);
                return (sx - self.cam_x, sy - self.cam_y);
            }
        };
        let (sx, sy) = map.sub_to_screen(e.x, e.y);
        (sx - self.cam_x, sy - self.cam_y)
    }

    fn blit_entity(&self, fb: &mut [u8], vw: usize, vh: usize, key: &str, e: &Entity) {
        let Some(set) = self.sprites.get(key) else { return };
        // Only the WL cycle is extracted in this phase; other states fall
        // back to it (idle = frame 0).
        let (facing, frame) = match e.anim {
            AnimState::NU => (e.facing, 0),
            _ => (e.facing, e.anim_frame as usize),
        };
        let Some(sf) = set.compose(facing, frame, &self.sprite_pal) else { return };
        let map = self.map.as_ref().unwrap();
        let (wx, wy) = map.sub_to_screen(e.x, e.y);
        blit_sprite(fb, vw, vh, &sf, wx - self.cam_x, wy - self.cam_y);
    }
}

#[derive(Debug, Clone, Copy)]
enum DrawKind {
    Hero,
    Monster(usize),
    Item(usize),
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ComposeStatsCopy {
    pub drawn: usize,
    pub hidden: usize,
    pub empty: usize,
    pub out_of_bounds: usize,
}

use d2dcc::{Cof, Dcc};

/// Precompute index → RGBA for the map palette (gamma-normalized, alpha 0
/// where the index was never painted).
fn build_map_rgba_lut(raw_pal: &[u8], gamma: f32) -> [u8; 256 * 4] {
    let pal = MapPalette::load(raw_pal, gamma);
    let mut lut = [0u8; 256 * 4];
    for i in 0..=255u8 {
        let [r, g, b, a] = pal.rgba(i);
        let o = i as usize * 4;
        lut[o] = r;
        lut[o + 1] = g;
        lut[o + 2] = b;
        lut[o + 3] = a;
    }
    lut
}

/// The validated `.tg1` → DT1-container mapping (`tools/map_proto2.py`).
pub fn tg1_to_container(tg1: &str) -> String {
    let norm = tg1.replace('/', "\\");
    let parts: Vec<&str> = norm.split('\\').filter(|p| !p.is_empty()).collect();
    let base = parts
        .last()
        .map(|p| p.strip_suffix(".tg1").unwrap_or(p).to_ascii_lowercase())
        .unwrap_or_default();
    let folder = parts
        .iter()
        .rev()
        .nth(1)
        .map(|p| p.to_ascii_lowercase())
        .unwrap_or_default();
    if base == "objects" {
        return if folder == "town" { "town_objects.dt1" } else { "out_objects.dt1" }.into();
    }
    match base.as_str() {
        "floor" => "town_floor.dt1",
        "fence" => "town_fence.dt1",
        "stonewall" => "out_stonewall.dt1",
        "river" => "out_river.dt1",
        "treegroups" => "out_treegroups.dt1",
        "warp" => "warp.dt1",
        // Unknown tg1 names: no container mapping — the finalize() asset
        // lookup surfaces the miss with a clear error.
        _ => "\0missing",
    }
    .to_string()
}

/// Blit a composed sprite: bottom-center anchored on the world position.
fn blit_sprite(fb: &mut [u8], vw: usize, vh: usize, sf: &SpriteFrame, sx: f32, sy: f32) {
    let dx0 = (sx - sf.width as f32 / 2.0).round() as i32;
    let dy0 = (sy - sf.height as f32).round() as i32;
    for y in 0..sf.height {
        let fy = dy0 + y;
        if fy < 0 || fy as usize >= vh {
            continue;
        }
        for x in 0..sf.width {
            let fx = dx0 + x;
            if fx < 0 || fx as usize >= vw {
                continue;
            }
            let si = ((y * sf.width + x) * 4) as usize;
            let a = sf.rgba[si + 3];
            if a == 0 {
                continue;
            }
            let di = (fy as usize * vw + fx as usize) * 4;
            fb[di..di + 4].copy_from_slice(&sf.rgba[si..si + 4]);
        }
    }
}

/// Soft dark ellipse under an entity (cheap D2-style ground shadow).
fn shadow(fb: &mut [u8], vw: usize, vh: usize, cx: f32, cy: f32, rx: f32, ry: f32, alpha: u8) {
    let x0 = (cx - rx).floor() as i32;
    let x1 = (cx + rx).ceil() as i32;
    let y0 = (cy - ry).floor() as i32;
    let y1 = (cy + ry).ceil() as i32;
    for y in y0..=y1 {
        if y < 0 || y as usize >= vh {
            continue;
        }
        for x in x0..=x1 {
            if x < 0 || x as usize >= vw {
                continue;
            }
            let nx = (x as f32 - cx) / rx;
            let ny = (y as f32 - cy) / ry;
            if nx * nx + ny * ny > 1.0 {
                continue;
            }
            let di = (y as usize * vw + x as usize) * 4;
            let a = alpha as u32;
            for c in 0..3 {
                let v = &mut fb[di + c];
                *v = ((*v as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
}

/// Small glowing marker for items on the ground (no item sprites yet).
fn item_marker(fb: &mut [u8], vw: usize, vh: usize, sx: f32, sy: f32) {
    let (w, h) = (7.0f32, 3.5f32);
    for dy in -h as i32..=h as i32 {
        for dx in -(w as i32)..=w as i32 {
            let x = sx as i32 + dx;
            let y = sy as i32 + dy;
            if x < 0 || y < 0 || x as usize >= vw || y as usize >= vh {
                continue;
            }
            let nx = dx as f32 / w;
            let ny = dy as f32 / h;
            if nx * nx + ny * ny <= 1.0 {
                let di = (y as usize * vw + x as usize) * 4;
                // warm gold sparkle, saturating add per channel
                fb[di] = fb[di].saturating_add(120);
                fb[di + 1] = fb[di + 1].saturating_add(220);
                fb[di + 2] = fb[di + 2].saturating_add(120);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");

    fn ws_root() -> Option<String> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        let assets = p.join("assets/manifest.json");
        assets.is_file().then(|| p.to_string_lossy().into_owned())
    }

    #[test]
    fn tg1_mapping_matches_prototype() {
        // Real TownN1.ds1 file list (release layout, leading drive slash).
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\town\\floor.tg1"), "town_floor.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\town\\fence.tg1"), "town_fence.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\town\\objects.tg1"), "town_objects.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\treegroups.tg1"), "out_treegroups.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\stonewall.tg1"), "out_stonewall.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\river.tg1"), "out_river.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\objects.tg1"), "out_objects.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\barracks\\warp.tg1"), "warp.dt1");
    }

    #[test]
    fn bad_manifest_rejected() {
        assert!(GameCore::new("not json", "BA").is_err());
    }

    #[test]
    fn empty_hero_code_defaults_to_ba() {
        let g = GameCore::new(MANIFEST, "").unwrap();
        let hc: Value = serde_json::from_str(&g.hero_class_json()).unwrap();
        assert_eq!(hc["code"], "BA");
        assert_eq!(hc["name"], "Barbare");
    }

    #[test]
    fn hero_class_resolves_from_heroes_block() {
        // manifest + `heroes` {BA, SOR} — la même règle que d2rules côté stats.
        let mut m: Value = serde_json::from_str(MANIFEST).unwrap();
        let mut sor = m["hero"].clone();
        sor["name"] = Value::String("Sorcière".into());
        m["heroes"] = serde_json::json!({ "BA": m["hero"], "SOR": sor });
        let g = GameCore::new(&m.to_string(), "sor").unwrap();
        let hc: Value = serde_json::from_str(&g.hero_class_json()).unwrap();
        assert_eq!(hc["code"], "SOR");
        assert_eq!(hc["name"], "Sorcière");
        // Code absent de `heroes` : retombée sur le bloc `hero` (BA).
        let g2 = GameCore::new(&m.to_string(), "NK").unwrap();
        let hc2: Value = serde_json::from_str(&g2.hero_class_json()).unwrap();
        assert_eq!(hc2["code"], "BA");
        assert_eq!(hc2["name"], "Barbare");
    }

    #[test]
    fn provide_then_missing_finalize_fails_cleanly() {
        let mut g = GameCore::new(MANIFEST, "BA").unwrap();
        g.provide("assets/map/pal_act1.dat", vec![0; 767]);
        assert!(g.finalize().is_err()); // ds1 not provided yet
    }

    #[test]
    fn full_pipeline_smoke() {
        let Some(root) = ws_root() else { return }; // skip outside workspace
        let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
        let mut g = GameCore::new(&manifest, "BA").unwrap();
        let m: Value = serde_json::from_str(&manifest).unwrap();
        let mut paths = vec![
            m.pointer("/level/ds1").unwrap().as_str().unwrap().to_string(),
            m.pointer("/level/palette").unwrap().as_str().unwrap().to_string(),
        ];
        for p in m.pointer("/level/dt1").unwrap().as_array().unwrap() {
            paths.push(p.as_str().unwrap().to_string());
        }
        paths.push(m.pointer("/hero/walk/cof").unwrap().as_str().unwrap().to_string());
        for (_, p) in m.pointer("/hero/walk/layers").unwrap().as_object().unwrap() {
            paths.push(p.as_str().unwrap().to_string());
        }
        for mon in m.get("monsters").unwrap().as_array().unwrap() {
            paths.push(mon.pointer("/walk/cof").unwrap().as_str().unwrap().to_string());
            for (_, p) in mon.pointer("/walk/layers").unwrap().as_object().unwrap() {
                paths.push(p.as_str().unwrap().to_string());
            }
        }
        for p in &paths {
            let bytes = std::fs::read(format!("{root}/{p}")).unwrap_or_else(|e| panic!("{p}: {e}"));
            g.provide(p, bytes);
        }
        g.finalize().unwrap();

        // Map sanity: plenty drawn, mostly-walkable town grid.
        let stats = g.compose_stats();
        assert!(stats.drawn > 500, "drawn too few: {stats:?}");
        let (gw, gh) = g.grid_size();
        assert_eq!((gw, gh), (57 * 5, 41 * 5));
        let grid = g.collision();
        let walkable = grid.iter().filter(|&&w| w == 1).count();
        let ratio = walkable as f32 / grid.len() as f32;
        assert!(ratio > 0.2 && ratio < 0.98, "walkable ratio suspicious: {ratio}");

        // Tick + render produce a sane framebuffer with non-black pixels.
        let _ = g.tick(16, None);
        let frame = g.render();
        assert_eq!(frame.len(), (DEFAULT_VIEWPORT_W * DEFAULT_VIEWPORT_H * 4) as usize);
        let lit = frame.chunks_exact(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count();
        assert!(lit > 10_000, "render too dark: {lit} lit pixels");
    }
}
