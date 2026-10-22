//! Byte-identity verification against real Diablo 2 game files, previously
//! extracted by the validated Python pipeline (`tools/mpq_mini.py`).
//!
//! Success criterion: the Rust reader must reproduce each asset byte for
//! byte (SHA-256 equality). The test skips itself gracefully when the game
//! workspace is not present (e.g. CI checkout without the archives) — set
//! `D2_WS` to override the workspace root.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use d2mpq::Archive;
use sha2::{Digest, Sha256};

const DEFAULT_WORKSPACE: &str = "/home/agent/workspace/diablo-2-wasm";

struct Case {
    /// MPQ path relative to the workspace root.
    mpq: &'static str,
    /// In-archive name (backslash form, as stored in the hash table).
    name: &'static str,
    /// Reference asset path relative to the workspace root.
    expected: &'static str,
    /// Compression path exercised by this case.
    tag: &'static str,
}

const CASES: &[Case] = &[
    Case {
        mpq: "data/d2data.mpq",
        name: "data\\global\\excel\\armor.txt",
        expected: "assets/armor.txt",
        tag: "zlib multi-sector",
    },
    Case {
        mpq: "data/d2data.mpq",
        name: "data\\global\\excel\\monstats.txt",
        expected: "assets/monstats.txt",
        tag: "zlib multi-sector",
    },
    Case {
        mpq: "data/d2data.mpq",
        name: "data\\global\\palette\\act1\\pal.dat",
        expected: "assets/pal_act1.dat",
        tag: "zlib small",
    },
    Case {
        mpq: "data/d2data.mpq",
        name: "data\\global\\ui\\spells\\skillicon.dc6",
        expected: "assets/skillicon.dc6",
        tag: "zlib",
    },
    Case {
        mpq: "data/d2data.mpq",
        name: "data\\global\\items\\invcap.dc6",
        expected: "assets/probe_d2data_invcap.dc6",
        tag: "zlib",
    },
    Case {
        mpq: "engine/patch_d2.mpq",
        name: "data\\global\\excel\\armor.txt",
        expected: "assets/patch_armor.txt",
        tag: "implode (DCL)",
    },
    Case {
        mpq: "engine/patch_d2.mpq",
        name: "data\\global\\excel\\monstats.txt",
        expected: "assets/patch_monstats.txt",
        tag: "implode (DCL)",
    },
];

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn verify_game_files_byte_identical() {
    let base = std::env::var("D2_WS").unwrap_or_else(|_| DEFAULT_WORKSPACE.to_string());
    if !Path::new(&base).join("data/d2data.mpq").is_file() {
        eprintln!("SKIP: game workspace not found under {base} (set D2_WS to override)");
        return;
    }

    // Archives are ~250 MB each: open every distinct MPQ only once.
    let mut archives: HashMap<&'static str, Archive> = HashMap::new();
    let mut verified = 0usize;

    for case in CASES {
        let archive = match archives.get(case.mpq) {
            Some(a) => a,
            None => {
                let raw = fs::read(Path::new(&base).join(case.mpq))
                    .unwrap_or_else(|e| panic!("{}: read {}: {e}", case.tag, case.mpq));
                let a = Archive::open(raw)
                    .unwrap_or_else(|e| panic!("{}: open {}: {e}", case.tag, case.mpq));
                archives.insert(case.mpq, a);
                archives.get(case.mpq).unwrap()
            }
        };

        let expected_path = Path::new(&base).join(case.expected);
        assert!(
            expected_path.is_file(),
            "{}: missing reference asset {}",
            case.tag,
            case.expected
        );
        let want = fs::read(&expected_path).unwrap();

        assert!(
            archive.has(case.name),
            "{}: archive does not expose {}",
            case.tag,
            case.name
        );
        let got = archive
            .read(case.name)
            .unwrap_or_else(|e| panic!("{}: read {}: {e}", case.tag, case.name));

        let sha = Sha256::digest(&got);
        assert_eq!(
            got.len(),
            want.len(),
            "{}: length mismatch (got {}, want {})",
            case.tag,
            got.len(),
            want.len()
        );
        assert!(
            got == want,
            "{}: content differs from {}",
            case.tag,
            case.expected
        );
        eprintln!(
            "OK [{}] {} -> {} ({} bytes, sha256={})",
            case.tag,
            case.name,
            case.expected,
            got.len(),
            hex(&sha)
        );
        verified += 1;
    }
    assert_eq!(verified, CASES.len(), "not every case was verified");
    eprintln!("{verified}/{} game files byte-identical", CASES.len());
}
