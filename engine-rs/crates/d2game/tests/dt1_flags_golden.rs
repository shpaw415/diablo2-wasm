//! Golden tests — DT1 sub-tile flags against the RE evidence
//! `d2w-re/evidence-formats/dt1-subtile` (fixtures in `golden/dt1/`):
//! every tile's 25 flags byte-for-byte vs the decoded JSON, the table at
//! `bodyPosition = 276`, the evidence §5 bit totals, the §3 hexdump
//! anchors, the §9 fine walkability semantics on real flag bytes, and a
//! zero-out-of-range prop check on the real camp map.

use d2game::ds1::{cell_fields, cell_tile_index};
use d2game::ds1_path::{decode_ds1_paths, Ds1Layer, Ds1Path, LayerKind};
use d2game::dt1_flags::{decode_dt1_flags, fine_walk_grid, Dt1TileFlags};
use serde_json::Value;

fn golden(p: &str) -> String {
    format!("{}/tests/golden/dt1/{p}", env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &str) -> Vec<u8> {
    std::fs::read(&golden(p)).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// (file, tiles, floors with 25 blocks, walk, jump, los) — evidence §1/§5.
const SAMPLES: [(&str, usize, usize, usize, usize, usize); 8] = [
    ("town_floor.dt1", 144, 144, 551, 50, 25),
    ("out_river.dt1", 45, 44, 972, 0, 0),
    ("out_stonewall.dt1", 52, 0, 631, 580, 0),
    ("town_fence.dt1", 31, 0, 147, 147, 0),
    ("cliff1.dt1", 13, 0, 129, 55, 55),
    ("cliff2.dt1", 13, 0, 171, 55, 55),
    ("corner.dt1", 3, 0, 35, 12, 12),
    ("out_fence.dt1", 23, 0, 116, 62, 0),
];

#[test]
fn all_8_containers_table_at_bodyposition_276() {
    for (file, tiles, floors, _, _, _) in SAMPLES {
        let t = decode_dt1_flags(&read(file)).unwrap();
        assert_eq!(t.len(), tiles, "{file}: tile count");
        let nfloor = t.iter().filter(|x| x.is_floor()).count();
        assert_eq!(nfloor, floors, "{file}: floor-kind tiles (nsub == 25)");
    }
}

/// Field checks of one tile against its decoded JSON entry.
fn check_tile(x: &Dt1TileFlags, jx: &Value) -> Result<(), String> {
    for (name, v) in [
        ("direction", x.direction as i64),
        ("type", x.tile_type as i64),
        ("style", x.style as i64),
        ("sequence", x.sequence as i64),
        ("num_blocks", x.num_blocks as i64),
    ] {
        let jv = jx[name].as_i64().unwrap_or(i64::MIN);
        if jv != v {
            return Err(format!("field {name}: {v} != json {jv}"));
        }
    }
    for (i, &b) in x.flags.iter().enumerate() {
        if jx["flags25"][i].as_u64() != Some(b as u64) {
            return Err(format!("flag[{i}]: {b:#04x} != json"));
        }
    }
    Ok(())
}

/// Byte-for-byte: the 25 flags of EVERY tile of all 8 containers vs the
/// decoded JSON dumps.
#[test]
fn flags25_match_the_evidence_json_on_every_tile() {
    for (file, tiles, _, _, _, _) in SAMPLES {
        let t = decode_dt1_flags(&read(file)).unwrap();
        let j: Value = serde_json::from_str(
            &std::fs::read_to_string(golden(&format!(
                "flags/{}.flags.json",
                file.trim_end_matches(".dt1")
            )))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(j["tile_count"].as_u64().unwrap() as usize, tiles, "{file}");
        assert_eq!(j["body_position"].as_u64().unwrap(), 276, "{file}");
        let jtiles = j["tiles"].as_array().unwrap();
        assert_eq!(jtiles.len(), t.len(), "{file}");
        for (x, jx) in t.iter().zip(jtiles) {
            check_tile(x, jx).unwrap_or_else(|e| panic!("{file} tile {}: {e}", jx["index"]));
        }
    }
}

/// Evidence §5: per-container walk/jump/LOS sub-tile totals, and the
/// corpus never leaves the distinct byte set {0x00, 0x01, 0x05, 0x07}.
#[test]
fn bit_totals_and_distinct_bytes() {
    for (file, _, _, walk, jump, los) in SAMPLES {
        let t = decode_dt1_flags(&read(file)).unwrap();
        let count = |bit: u8| {
            t.iter().map(|x| x.flags.iter().filter(|&&b| b & bit != 0).count()).sum::<usize>()
        };
        assert_eq!(count(0x01), walk, "{file}: BlockWalk total");
        assert_eq!(count(0x04), jump, "{file}: BlockJump total");
        assert_eq!(count(0x02), los, "{file}: BlockLOS total");
        let distinct: std::collections::BTreeSet<u8> =
            t.iter().flat_map(|x| x.flags).collect();
        assert!(
            distinct.iter().all(|b| matches!(b, 0x00 | 0x01 | 0x05 | 0x07)),
            "{file}: distinct flag bytes {distinct:?}"
        );
    }
}

/// Evidence §3 hexdumps, as in-situ anchors:
/// out_river tile 3 = 25 × 0x01 (water blocks the step, LOS clear);
/// town_fence tile 0 = 0x05 on the whole x=0 column (indices 0,5,10,15,20);
/// cliff1 tile 2 = 0x07 on column x=0 + 0x01 on column x=1.
#[test]
fn anchored_tiles_from_the_evidence_hexdumps() {
    let river = decode_dt1_flags(&read("out_river.dt1")).unwrap();
    assert!(river[3].flags.iter().all(|&b| b == 0x01), "water: 25 × 0x01");
    let fence = decode_dt1_flags(&read("town_fence.dt1")).unwrap();
    for i in [0usize, 5, 10, 15, 20] {
        assert_eq!(fence[0].flags[i], 0x05, "fence column x=0: walk + jump");
    }
    assert_eq!(fence[0].flags.iter().filter(|&&b| b != 0).count(), 5);
    let cliff = decode_dt1_flags(&read("cliff1.dt1")).unwrap();
    for row in 0..5usize {
        assert_eq!(cliff[2].flags[row * 5], 0x07, "cliff x=0: walk + jump + LOS");
        assert_eq!(cliff[2].flags[row * 5 + 1], 0x01, "cliff x=1: walk only");
    }
}

/// §9 semantics on REAL flag bytes (town_fence tile 0: 0x05 on column
/// x=0): the fence cell blocks the step on its x=0 sub-column but keeps
/// the sight line clear — the fine-collision gain over the all-or-nothing
/// `collision.json`. Floor stays open; void stays closed.
#[test]
fn fine_grid_semantics_on_real_fence_flags() {
    let floor = decode_dt1_flags(&read("town_floor.dt1")).unwrap();
    let fence = decode_dt1_flags(&read("town_fence.dt1")).unwrap();
    assert!(floor[0].flags.iter().all(|&b| b == 0), "town_floor tile 0 is open grass");
    let tiles: Vec<Dt1TileFlags> = floor.into_iter().take(144).chain(fence.into_iter().take(1))
        .collect();

    // 3×1 cells: grass floor (prop 1 → real 0), fence wall (prop 145 →
    // real 144), void (prop 0).
    let ds1 = Ds1Path {
        version: 18,
        w: 3,
        h: 1,
        act: Some(0),
        subst_type: Some(0),
        files: vec![],
        layers: vec![
            Ds1Layer { kind: LayerKind::Wall, cells: vec![0, 145, 0] },
            Ds1Layer { kind: LayerKind::Orientation, cells: vec![0; 3] },
            Ds1Layer { kind: LayerKind::Floor, cells: vec![1, 1, 0] },
            Ds1Layer { kind: LayerKind::Shadow, cells: vec![0; 3] },
        ],
        objects: vec![],
        npcs: vec![],
        path_section_offset: None,
        consumed_to_eof: true,
    };
    let g = fine_walk_grid(&ds1, &tiles);
    assert_eq!((g.w, g.h), (15, 5));

    // Grass cell: fully walkable, LOS clear.
    for y in 0..5 {
        for x in 0..5 {
            assert!(g.walkable(x, y), "grass ({x},{y}) walkable");
            assert!(!g.los_blocked(x, y), "grass ({x},{y}) LOS clear");
        }
    }
    // Fence cell (sub-tile x offset 5): the x=5 sub-column (fence
    // storage column 0) blocks the step — screen rows are the remap of
    // the storage rows, the column is untouched — but LOS stays clear.
    for y in 0..5 {
        assert!(!g.walkable(5, y), "fence blocks the step at (5,{y})");
        assert!(!g.los_blocked(5, y), "fence keeps the sight at (5,{y})");
        for x in 6..10 {
            assert!(g.walkable(x, y), "fence cell open at ({x},{y})");
        }
    }
    // Void cell: no floor → never walkable (compositor parity).
    for y in 0..5 {
        for x in 10..15 {
            assert!(!g.walkable(x, y), "void ({x},{y}) blocked");
        }
    }
}

/// Real camp (TownN1.ds1 in the workspace assets — auto-skip like the
/// proof tests): the DS1 props must ALL resolve inside the concatenated
/// REAL tile table read at 276, in the DS1's own .tg1 container order —
/// the 1-based-prop / correct-table invariant of evidence §7 on the map
/// the game actually renders.
#[test]
fn real_camp_props_resolve_inside_the_correct_table() {
    let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap();
    let Ok(ds1_bytes) = std::fs::read(ws.join("assets/map/TownN1.ds1")) else {
        return; // assets absent — proof-style skip
    };
    let ds1 = match decode_ds1_paths(&ds1_bytes) {
        Ok(d) => d,
        Err(e) => panic!("TownN1.ds1: {e:?}"),
    };
    assert!(ds1.consumed_to_eof, "TownN1.ds1 parses to exact EOF");

    // Containers in the DS1's own .tg1 order — the render pipeline's rule.
    let mut tiles = Vec::new();
    for tg1 in &ds1.files {
        let container = d2game::game::tg1_to_container(tg1);
        let Ok(bytes) = std::fs::read(ws.join("assets/map").join(&container)) else {
            return; // container missing from this tree — skip
        };
        tiles.append(&mut decode_dt1_flags(&bytes).unwrap());
    }
    assert!(!tiles.is_empty(), "at least one container loaded");

    let mut floor_cells = 0usize;
    let mut out_of_range = 0usize;
    for layer in &ds1.layers {
        if !matches!(layer.kind, LayerKind::Wall | LayerKind::Floor | LayerKind::Shadow) {
            continue;
        }
        for &v in &layer.cells {
            let (p1, p2, _p3, orient) = cell_fields(v);
            if p1 == 0 || orient & 0x80 != 0 {
                continue;
            }
            let index = cell_tile_index(p1, p2);
            // Props are 1-based: real tile = index − 1 (evidence §7).
            if index == 0 || index > tiles.len() {
                out_of_range += 1;
            } else if layer.kind == LayerKind::Floor {
                floor_cells += 1;
            }
        }
    }
    assert_eq!(out_of_range, 0, "every prop resolves in the 276-based table");
    assert!(floor_cells > 0, "the camp has floor cells");
}
