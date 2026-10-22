//! Proof renders — the same artifacts the validated Python prototype
//! produced, now from Rust (`assets/map/level-render.png` is the oracle):
//!
//! * `map-compose.png` — full map at half scale (comparable to the oracle)
//! * `map-compose-crop.png` — 1:1 crop around the hero spawn
//! * `collision-heat.png` — walkability grid overlay (green = walkable)
//! * `sprite-plate.png` — hero + monsters across facings/frames
//!
//! Auto-skips when the workspace assets are absent.

mod common;

use std::path::Path;

use d2game::compose;
use d2game::ds1;
use d2game::dt1;
use d2game::game::GameCore;
use d2game::pal::MapPalette;
use d2game::png_mini;
use serde_json::Value;

#[test]
fn proof_map_and_collision() {
    let Some(root) = common::ws_root() else { return };
    let out = common::proof_dir();

    let manifest_bytes = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_str(&manifest_bytes).unwrap();

    let ds1_path = manifest.pointer("/level/ds1").unwrap().as_str().unwrap();
    let pal_path = manifest.pointer("/level/palette").unwrap().as_str().unwrap();
    let ds1 = ds1::decode_ds1(&std::fs::read(format!("{root}/{ds1_path}")).unwrap()).unwrap();
    let raw_pal = std::fs::read(format!("{root}/{pal_path}")).unwrap();

    let mut tilesets = Vec::new();
    for tg1 in &ds1.files {
        let container = d2game::game::tg1_to_container(tg1);
        let bytes = std::fs::read(format!("{root}/assets/map/{container}"))
            .unwrap_or_else(|e| panic!("{container}: {e}"));
        tilesets.append(&mut dt1::decode_dt1(&bytes).unwrap());
    }

    let map = compose::compose(&ds1, &tilesets);
    println!("compose stats: {:?}", map.stats);
    println!("map: {}x{}, grid: {}x{}", map.width, map.height, map.grid_w, map.grid_h);
    assert!(map.stats.drawn > 500);
    assert_eq!((map.grid_w, map.grid_h), (57 * 5, 41 * 5));

    // --- full map, half scale (matches the validated prototype output) --
    let pal = MapPalette::load(&raw_pal, 0.7);
    let (w2, h2) = (map.width / 2, map.height / 2);
    let mut rgb = vec![0u8; w2 * h2 * 3];
    for y in 0..h2 {
        for x in 0..w2 {
            let v = map.idx[(y * 2) * map.width + x * 2];
            let c = pal.rgba(v);
            let o = (y * w2 + x) * 3;
            rgb[o..o + 3].copy_from_slice(&c[0..3]);
        }
    }
    let full = format!("{out}/map-compose.png");
    png_mini::write_png_rgb(&full, w2 as u32, h2 as u32, &rgb).unwrap();
    println!("wrote {full}");

    // --- 1:1 crop around the hero spawn (nearest walkable to center) ----
    let spawn = compose::find_nearby_walkable(
        &map.walk,
        map.grid_w,
        map.grid_h,
        map.grid_w / 2,
        map.grid_h / 2,
        64,
    )
    .expect("a walkable spawn exists near the town center");
    let (sx, sy) = map.sub_to_screen(spawn.0 as f32 + 0.5, spawn.1 as f32 + 0.5);
    let (cx0, cy0) = ((sx as i32 - 400).max(0) as usize, (sy as i32 - 300).max(0) as usize);
    let (cw, ch) = (800usize, 600usize);
    let mut crop = vec![0u8; cw * ch * 3];
    for y in 0..ch {
        let my = cy0 + y;
        if my >= map.height {
            continue;
        }
        for x in 0..cw {
            let mx = cx0 + x;
            if mx >= map.width {
                continue;
            }
            let c = pal.rgba(map.idx[my * map.width + mx]);
            let o = (y * cw + x) * 3;
            crop[o..o + 3].copy_from_slice(&c[0..3]);
        }
    }
    let crop_path = format!("{out}/map-compose-crop.png");
    png_mini::write_png_rgb(&crop_path, cw as u32, ch as u32, &crop).unwrap();
    println!("wrote {crop_path} (spawn sub-tile {spawn:?} at screen {sx:.0},{sy:.0})");

    // --- collision heatmap, 4px per sub-tile -----------------------------
    let s = 4usize;
    let mut heat = vec![0u8; map.grid_w * s * map.grid_h * s * 3];
    for gy in 0..map.grid_h {
        for gx in 0..map.grid_w {
            let walkable = map.walkable(gx, gy);
            let (r, g, b) = if walkable { (40u8, 200u8, 60) } else { (170, 30, 30) };
            for py in 0..s {
                for px in 0..s {
                    let o = ((gy * s + py) * map.grid_w * s + gx * s + px) * 3;
                    heat[o] = r;
                    heat[o + 1] = g;
                    heat[o + 2] = b;
                }
            }
        }
    }
    let heat_path = format!("{out}/collision-heat.png");
    png_mini::write_png_rgb(&heat_path, (map.grid_w * s) as u32, (map.grid_h * s) as u32, &heat)
        .unwrap();
    println!("wrote {heat_path}");
    let walkable_ratio =
        map.walk.iter().filter(|&&w| w).count() as f32 / map.walk.len() as f32;
    println!("walkable ratio: {walkable_ratio:.2}");
    assert!(walkable_ratio > 0.2 && walkable_ratio < 0.98);
}

#[test]
fn proof_sprite_plate() {
    let Some(root) = common::ws_root() else { return };
    let out = common::proof_dir();
    let manifest_bytes = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let manifest: Value = serde_json::from_str(&manifest_bytes).unwrap();
    let raw_pal = std::fs::read(format!(
        "{root}/{}",
        manifest.pointer("/level/palette").unwrap().as_str().unwrap()
    ))
    .unwrap();

    let mut game = build_game(&root, &manifest);
    game.finalize().unwrap();
    let _ = game; // forces the whole provide/finalize path once here

    // Render hero at several facings via the sprite API directly.
    let cof_path = manifest.pointer("/hero/walk/cof").unwrap().as_str().unwrap();
    let cof = d2dcc::Cof::parse(&std::fs::read(format!("{root}/{cof_path}")).unwrap()).unwrap();
    let mut set = d2game::sprite::SpriteSet::new(cof);
    for (tok, path) in manifest.pointer("/hero/walk/layers").unwrap().as_object().unwrap() {
        let bytes = std::fs::read(format!("{root}/{}", path.as_str().unwrap())).unwrap();
        set.add_layer(tok, d2dcc::Dcc::decode(&bytes).unwrap());
    }
    let pal = d2game::sprite::sprite_palette(&raw_pal);
    let facings = [0u8, 4, 8, 12];
    let frames = 4;
    let cell_w = 160;
    let cell_h = 200;
    let mut rgba = vec![0u8; cell_w * facings.len() * cell_h * frames * 4];
    // light checker background to visualize transparency
    for y in 0..cell_h * frames {
        for x in 0..cell_w * facings.len() {
            let o = (y * cell_w * facings.len() + x) * 4;
            let v = if (x / 16 + y / 16) % 2 == 0 { 70 } else { 55 };
            rgba[o] = v;
            rgba[o + 1] = v;
            rgba[o + 2] = v + 8;
            rgba[o + 3] = 255;
        }
    }
    for (fi, facing) in facings.iter().enumerate() {
        for frame in 0..frames {
            let Some(sf) = set.compose(*facing, frame, &pal) else { continue };
            let base_x = fi * cell_w + (cell_w - sf.width as usize) / 2;
            let base_y = frame * cell_h + (cell_h - sf.height as usize).min(cell_h) - sf.height as usize + cell_h - sf.height as usize;
            let base_y = frame * cell_h + cell_h - sf.height as usize; // feet on cell bottom
            for y in 0..sf.height as usize {
                for x in 0..sf.width as usize {
                    let si = (y * sf.width as usize + x) * 4;
                    if sf.rgba[si + 3] == 0 {
                        continue;
                    }
                    let dx = base_x + x;
                    let dy = base_y + y;
                    if dx >= cell_w * facings.len() || dy >= cell_h * frames {
                        continue;
                    }
                    let o = (dy * cell_w * facings.len() + dx) * 4;
                    rgba[o..o + 4].copy_from_slice(&sf.rgba[si..si + 4]);
                }
            }
        }
    }
    let plate = format!("{out}/sprite-plate.png");
    png_mini::write_png_rgba(
        &plate,
        (cell_w * facings.len()) as u32,
        (cell_h * frames) as u32,
        &rgba,
    )
    .unwrap();
    println!("wrote {plate} (facings {facings:?} x frames 0..{frames})");
}

fn build_game(root: &str, manifest: &Value) -> GameCore {
    let mut game =
        GameCore::new(&serde_json::to_string(manifest).unwrap(), "BA").unwrap();
    let mut paths: Vec<String> = vec![
        manifest.pointer("/level/ds1").unwrap().as_str().unwrap().into(),
        manifest.pointer("/level/palette").unwrap().as_str().unwrap().into(),
    ];
    for p in manifest.pointer("/level/dt1").unwrap().as_array().unwrap() {
        paths.push(p.as_str().unwrap().into());
    }
    paths.push(manifest.pointer("/hero/walk/cof").unwrap().as_str().unwrap().into());
    for (_, p) in manifest.pointer("/hero/walk/layers").unwrap().as_object().unwrap() {
        paths.push(p.as_str().unwrap().into());
    }
    for m in manifest.get("monsters").unwrap().as_array().unwrap() {
        if m.pointer("/walk/cof").is_some() {
            paths.push(m.pointer("/walk/cof").unwrap().as_str().unwrap().into());
            for (_, p) in m.pointer("/walk/layers").unwrap().as_object().unwrap() {
                paths.push(p.as_str().unwrap().into());
            }
        }
        // monstres MVP : `modes` remplace les blocs nommés
        if let Some(modes) = m.get("modes").and_then(|x| x.as_object()) {
            for mv in modes.values() {
                paths.push(mv["cof"].as_str().unwrap().into());
                for (_, p) in mv["layers"].as_object().unwrap() {
                    paths.push(p.as_str().unwrap().into());
                }
            }
        }
    }
    for p in &paths {
        let bytes = std::fs::read(format!("{root}/{p}")).unwrap_or_else(|e| panic!("{p}: {e}"));
        game.provide(p, bytes);
    }
    game
}

#[test]
fn proof_rendered_viewport() {
    let Some(root) = common::ws_root() else { return };
    let out = common::proof_dir();
    let manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap())
            .unwrap();
    let mut game = build_game(&root, &manifest);
    game.finalize().unwrap();

    // a few ticks so the hero walks a bit (visible WL frame, camera follow)
    for _ in 0..30 {
        let _ = game.tick(16, None);
    }
    let (vw, vh) = (800u32, 600u32);
    game.set_viewport(vw, vh);
    let frame = game.render();
    let mut rgb = vec![0u8; frame.len() / 4 * 3];
    for (i, px) in frame.chunks_exact(4).enumerate() {
        rgb[i * 3..i * 3 + 3].copy_from_slice(&px[0..3]);
    }
    let path = format!("{out}/viewport-render.png");
    png_mini::write_png_rgb(&path, vw, vh, &rgb).unwrap();
    println!("wrote {path}");
    assert!(Path::new(&path).is_file());
}
