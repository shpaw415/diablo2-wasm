//! End-to-end validation of the AnimData.d2 port against the real game file
//! and two independent references:
//!
//! 1. the retail v1.12 file extracted from `engine/patch_d2.mpq`
//!    (`data\global\animdata.d2`, 570 304 B — method in docs/GAME-DATA.md),
//! 2. a golden dump of every record produced by an independent Python
//!    parser mirroring the Go `Load` (`tests/golden/animdata_v112.txt`),
//! 3. the OpenDiablo2 Go package's own `testdata/AnimData.d2` — same 3 529
//!    names, same per-name values (only the intra-block order of homonym
//!    records differs), proving coherence with the Go reference.
//!
//! Like `d2mpq/tests`, the tests skip gracefully when the game workspace is
//! absent (override with `D2_WS`).

use std::fs;
use std::path::Path;

use d2animdata::{hash_name, AnimationData, AnimationEvent};

const DEFAULT_WORKSPACE: &str = "/home/agent/workspace/diablo-2-wasm";

fn ws_root() -> Option<String> {
    if let Ok(p) = std::env::var("D2_WS") {
        return Some(p);
    }
    let p = DEFAULT_WORKSPACE.to_string();
    if Path::new(&p).join("assets/animdata/AnimData.d2").exists() {
        Some(p)
    } else {
        None
    }
}

fn load_real(root: &str) -> AnimationData {
    let bytes = fs::read(format!("{root}/assets/animdata/AnimData.d2")).unwrap();
    AnimationData::load(&bytes).expect("retail v1.12 AnimData.d2 must parse")
}

/// 1 — the real file parses fully: exact known totals, block census.
#[test]
fn parse_real_v112_file() {
    let Some(root) = ws_root() else { return };
    let ad = load_real(&root);

    // 3 558 records, 3 529 distinct names (29 homonyms — Go GetRecords case).
    assert_eq!(ad.total_records(), 3558);
    assert!(ad.total_records() > 3000, "far more records than any hand table");
    assert_eq!(ad.entry_count(), 3529);

    // Block census: 256 blocks, exactly one packed at 67 records, 120 empty.
    let blocks = ad.blocks();
    assert_eq!(blocks.len(), d2animdata::NUM_BLOCKS);
    assert_eq!(blocks.iter().filter(|b| b.record_count == 67).count(), 1);
    assert_eq!(blocks.iter().filter(|b| b.record_count == 0).count(), 120);
    assert!(blocks.iter().all(|b| b.record_count as usize == b.records.len()));
    // `load` returning Ok already proves full consumption (LengthMismatch otherwise).
}

/// 2 — known in-game tokens resolve with the values measured in the retail
/// file (see docs/parity/impl-animdata.md for the extraction).
#[test]
fn known_game_tokens_lookup() {
    let Some(root) = ws_root() else { return };
    let ad = load_real(&root);

    // The MVP hero's walk animation — COF BAWL1HS.COF.
    let ba = ad.get_record("BAWL1HS").expect("BAWL1HS must exist");
    assert_eq!(ba.frames_per_direction, 8);
    assert_eq!(ba.speed, 168);
    assert!((ba.fps() - 25.0 * 168.0 / 256.0).abs() < 1e-12);

    // The three Act-1 monsters of the MVP.
    let fa = ad.get_record("FAWLHTH").expect("FAWLHTH must exist");
    assert_eq!((fa.frames_per_direction, fa.speed), (10, 256)); // 25 fps
    let zm = ad.get_record("ZMWLHTH").expect("ZMWLHTH must exist");
    assert_eq!((zm.frames_per_direction, zm.speed), (12, 104));
    let si = ad.get_record("SIWLHTH").expect("SIWLHTH must exist");
    assert_eq!((si.frames_per_direction, si.speed), (9, 256));

    // Absent token → None (e.g. Barbarian run HTH has no record in 1.12).
    assert!(ad.get_record("BARNUHTH").is_none());
    assert!(ad.get_records("BARNUHTH").is_empty());

    // `get_record` matches exactly (Go `entries[name]` does too); the
    // case-insensitive entry point is `get_by_token`, like the game's use.
    assert!(ad.get_record("bawl1hs").is_none());
    assert_eq!(ad.get_by_token("bawl1hs").len(), 1);
    assert_eq!(ad.get_by_token("bawl1hs")[0].speed, 168);
}

/// 3 — the file's own hash placement: EVERY record lives in the block
/// `hash_name(name)` predicts (3 558/3 558 on the real file), and the
/// hash-table path (`get_by_token`) yields exactly the same records as the
/// entries-map path (`get_records`).
#[test]
fn hash_placement_holds_for_every_record() {
    let Some(root) = ws_root() else { return };
    let ad = load_real(&root);

    for (block_idx, block) in ad.blocks().iter().enumerate() {
        for rec in &block.records {
            assert_eq!(hash_name(&rec.name) as usize, block_idx, "record {}", rec.name);
        }
    }

    for name in ad.record_names() {
        let by_map = ad.get_records(name);
        let by_hash = ad.get_by_token(name);
        assert_eq!(by_map.len(), by_hash.len(), "{name}");
        for (a, b) in by_map.iter().zip(by_hash.iter()) {
            assert_eq!(a, b, "{name}");
        }
    }
}

/// 4 — coherence with the independent Python dump: every one of the 3 558
/// records must match block/name/fpd/speed/trigger-events byte for byte.
#[test]
fn coherence_with_independent_golden_dump() {
    let Some(root) = ws_root() else { return };
    let ad = load_real(&root);

    let golden = fs::read_to_string(format!(
        "{}/tests/golden/animdata_v112.txt",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let mut lines = golden.lines();
    let mut compared = 0usize;

    for (block_idx, block) in ad.blocks().iter().enumerate() {
        for rec in &block.records {
            let line = lines.next().expect("golden dump shorter than parsed records");
            let triggers: Vec<(usize, AnimationEvent)> = rec.triggered_events().collect();
            let trig: String = if triggers.is_empty() {
                "-".to_string()
            } else {
                triggers.iter().map(|(i, e)| format!("{i}:{}", *e as u8)).collect::<Vec<_>>().join(",")
            };
            assert_eq!(
                line,
                format!("{block_idx}\t{}\t{}\t{}\t{trig}", rec.name, rec.frames_per_direction, rec.speed),
                "record mismatch at {}",
                rec.name
            );
            compared += 1;
        }
    }
    assert_eq!(compared, 3558);
    assert!(lines.next().is_none(), "golden dump longer than parsed records");
}

/// 5 — coherence with the Go reference: OpenDiablo2's own test file parses
/// with identical totals, identical name set and identical per-name values
/// (multiset — the two files store homonyms in different intra-block order).
#[test]
fn coherence_with_opendiablo2_go_testdata() {
    let Some(root) = ws_root() else { return };
    let go_testdata = format!(
        "{}/../opendiablo2/d2common/d2fileformats/d2animdata/testdata/AnimData.d2",
        root
    );
    if !Path::new(&go_testdata).exists() {
        return; // Go repo not checked out — skip portably.
    }

    let ours = load_real(&root);
    let go = AnimationData::load(&fs::read(&go_testdata).unwrap()).expect("Go testdata must parse");

    assert_eq!(go.total_records(), ours.total_records());
    assert_eq!(go.entry_count(), ours.entry_count());
    assert_eq!(go.record_names(), ours.record_names(), "same 3 529 tokens");

    let multiset = |ad: &AnimationData, name: &str| -> Vec<(u32, u16, Vec<u8>)> {
        let mut v: Vec<_> = ad
            .get_records(name)
            .into_iter()
            .map(|r| (r.frames_per_direction, r.speed, r.events.to_vec()))
            .collect();
        v.sort();
        v
    };
    for name in ours.record_names() {
        assert_eq!(multiset(&go, name), multiset(&ours, name), "values differ for {name}");
    }
}
