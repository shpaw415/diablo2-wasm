//! Golden tests — DS1 PATH section (NPC patrols) against the RE evidence
//! `d2w-re/evidence-formats/ds1-path` (fixtures copied in `golden/ds1/`):
//! exact consumption to EOF on the 8 samples, NPC/point counts, full
//! deep-compare of the 3 decoded JSONs, 100 % object attachment.

use d2game::ds1_path::{decode_ds1_paths, LayerKind};
use serde_json::Value;

fn golden(p: &str) -> String {
    format!("{}/tests/golden/ds1/{p}", env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &str) -> Vec<u8> {
    std::fs::read(&golden(p)).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// (file, npc count, total path points) — evidence §1 table.
const SAMPLES: [(&str, usize, usize); 8] = [
    ("townW1.ds1", 5, 22),
    ("townE1.ds1", 5, 19),
    ("lutN.ds1", 31, 116),
    ("lutW.ds1", 28, 98),
    ("townsiege.ds1", 0, 0),
    ("striptownsiege.ds1", 0, 0),
    ("townWest.ds1", 5, 24),
    ("tri_town4.ds1", 0, 0),
];

#[test]
fn all_8_samples_parse_to_exact_eof_with_expected_counts() {
    for (file, npc_count, points) in SAMPLES {
        let ds1 = decode_ds1_paths(&read(file))
            .unwrap_or_else(|e| panic!("{file}: {e:?}"));
        assert!(ds1.consumed_to_eof, "{file}: trailing bytes after PATH");
        assert_eq!(ds1.version, 18, "{file}");
        let total: usize = ds1.npcs.iter().map(|n| n.paths.len()).sum();
        assert_eq!(ds1.npcs.len(), npc_count, "{file}: npc count");
        assert_eq!(total, points, "{file}: path points");
        // v18 grid trap: real = stored + 1 (57×41, 57×57, 81×49, 17×49,
        // 41×41, 44×49 per the evidence).
        assert!(ds1.w > 1 && ds1.h > 1);
        // All 8 samples have substType 0 → streams = walls*2 + floors + 1.
        assert_eq!(ds1.subst_type, Some(0));
        assert!(matches!(ds1.layers.last().map(|l| &l.kind), Some(LayerKind::Shadow)));
        assert_eq!(
            ds1.layers.iter().filter(|l| l.kind == LayerKind::Shadow).count(),
            1,
            "{file}: exactly one shadow stream"
        );
    }
}

/// Full deep-compare against the decoded JSON evidence: npc list (x, y,
/// path points with actions) AND the object attachment by exact (x, y).
#[test]
fn decoded_json_golden_townw1_lutn_townsiege() {
    for (file, json) in [
        ("townW1.ds1", "decoded/townW1.json"),
        ("lutN.ds1", "decoded/lutN.json"),
        ("townsiege.ds1", "decoded/townsiege.json"),
    ] {
        let ds1 = decode_ds1_paths(&read(file)).unwrap();
        let j: Value = serde_json::from_str(&std::fs::read_to_string(golden(json)).unwrap())
            .unwrap_or_else(|e| panic!("{json}: {e}"));

        // Grid + offsets + consumption.
        assert_eq!(ds1.w as u64, j["grid"]["w"].as_u64().unwrap(), "{file}");
        assert_eq!(ds1.h as u64, j["grid"]["h"].as_u64().unwrap(), "{file}");
        assert_eq!(
            ds1.path_section_offset,
            Some(j["offsets"]["path_section"].as_u64().unwrap() as usize),
            "{file}: PATH offset"
        );
        assert!(ds1.consumed_to_eof);
        assert_eq!(
            j["offsets"]["trailing_bytes"].as_u64(),
            Some(0),
            "{file}: evidence consumption"
        );

        // Streams: walls*2 + floors + 1 shadow (+0 substitution).
        let jw = j["layers"]["walls"].as_u64().unwrap() as usize;
        let jf = j["layers"]["floors"].as_u64().unwrap() as usize;
        assert_eq!(ds1.layers.len(), jw * 2 + jf + 1, "{file}: stream count");
        assert_eq!(ds1.layers.iter().filter(|l| l.kind == LayerKind::Floor).count(), jf);
        assert_eq!(ds1.layers.iter().filter(|l| l.kind == LayerKind::Wall).count(), jw);

        // Objects count.
        assert_eq!(ds1.objects.len(), j["object_count"].as_u64().unwrap() as usize);

        // NPCs, one by one, in file order.
        let jnpcs = j["npcs"].as_array().unwrap();
        assert_eq!(ds1.npcs.len(), jnpcs.len(), "{file}: npc count");
        for (npc, jn) in ds1.npcs.iter().zip(jnpcs) {
            assert_eq!(npc.x, jn["x"].as_i64().unwrap() as i32, "{file}");
            assert_eq!(npc.y, jn["y"].as_i64().unwrap() as i32, "{file}");
            match jn.get("object_index") {
                Some(oi) => {
                    let oi = oi.as_u64().unwrap() as usize;
                    assert_eq!(npc.object_index, Some(oi), "{file}: attach");
                    let ob = &ds1.objects[oi];
                    assert_eq!(ob.kind, jn["object_type_id"][0].as_i64().unwrap() as i32);
                    assert_eq!(ob.id, jn["object_type_id"][1].as_i64().unwrap() as i32);
                    assert_eq!((ob.x, ob.y), (npc.x, npc.y), "{file}: carrier pos");
                }
                None => assert_eq!(npc.object_index, None, "{file}: orphan"),
            }
            let jpts = jn["paths"].as_array().unwrap();
            assert_eq!(npc.paths.len(), jpts.len(), "{file}: point count");
            for (pt, jp) in npc.paths.iter().zip(jpts) {
                assert_eq!(pt.x, jp["x"].as_i64().unwrap() as i32, "{file}: pt x");
                assert_eq!(pt.y, jp["y"].as_i64().unwrap() as i32, "{file}: pt y");
                assert_eq!(
                    pt.action,
                    Some(jp["action"].as_i64().unwrap() as i32),
                    "{file}: action (v18 > 15)"
                );
            }
        }
    }
}

/// Evidence §5.2: every NPC (x, y) resolves exactly in the object table —
/// 5/5 townW1, 31/31 lutN, and the whole corpus (69 attach checks; a
/// one-dword misalignment anywhere would break at least one).
#[test]
fn every_npc_attaches_to_a_typed_object() {
    for (file, npc_count, _) in SAMPLES {
        let ds1 = decode_ds1_paths(&read(file)).unwrap();
        assert_eq!(ds1.npcs.len(), npc_count);
        for npc in &ds1.npcs {
            let Some(oi) = npc.object_index else {
                panic!("{file}: NPC ({},{}) has no carrier object", npc.x, npc.y);
            };
            // The (x, y) of the NPC IS the object-table key (ds1.go:421-429);
            // carrier types observed: 1 (dynamic unit) on act 0/1 towns,
            // 2 on Harrogath (townWest) — the position match is the invariant.
            let ob = &ds1.objects[oi];
            assert_eq!((ob.x, ob.y), (npc.x, npc.y));
        }
    }
}

/// Evidence §5.4: observed action codes are exactly {1, 2, 3, 4} over the
/// 279 points of the corpus.
#[test]
fn action_codes_stay_in_the_observed_range() {
    let mut all = std::collections::BTreeSet::new();
    for (file, _, _) in SAMPLES {
        let ds1 = decode_ds1_paths(&read(file)).unwrap();
        for npc in &ds1.npcs {
            for pt in &npc.paths {
                all.insert(pt.action.expect("v18 has actions"));
            }
        }
    }
    assert_eq!(all, [1, 2, 3, 4].into_iter().collect());
}
