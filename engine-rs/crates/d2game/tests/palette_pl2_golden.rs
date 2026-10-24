//! Golden tests — `pal.pl2` (acts 1–5) against the RE evidence
//! `d2w-re/palettes` (fixtures in `golden/palette/`): the base palette
//! byte-to-byte vs the extracted `pal_actN.dat` (documented R/B swap),
//! the engine's region map on real bytes, and the project's display
//! convention (`pal_act1_pl2.dat` = base × 255/252).

use d2game::palette::pl2::{map_palette_from_pl2, sprite_palette_from_pl2, Pl2, Pl2Error,
                           ENGINE_SIZE};

fn golden(p: &str) -> String {
    format!("{}/tests/golden/palette/{p}", env!("CARGO_MANIFEST_DIR"))
}

fn read(p: &str) -> Vec<u8> {
    std::fs::read(&golden(p)).unwrap_or_else(|e| panic!("{p}: {e}"))
}

/// The engine consumes the WHOLE file: exactly [`ENGINE_SIZE`] bytes on
/// all 5 acts (evidence: 0 byte unread, no magic).
#[test]
fn all_5_acts_are_exact_engine_size() {
    for act in 1..=5 {
        let d = read(&format!("pal_act{act}.pl2"));
        assert_eq!(d.len(), ENGINE_SIZE, "act {act}");
        let pl2 = Pl2::parse(&d).unwrap();
        // Region 0: entry 1 = (0x24, 0, 0) pure red, 6-bit values (0x24
        // = 9 × 4) — evidence table.
        assert_eq!(&pl2.base_raw()[3..6], &[0x24, 0, 0], "act {act} entry 1");
        // 6-bit VGA values (multiples of 4) — except the last entry,
        // pure white stored raw (255,255,255), on all 5 acts.
        assert!(
            pl2.base_raw()[..765].iter().all(|&v| v % 4 == 0),
            "act {act}: 6-bit VGA values"
        );
        assert_eq!(&pl2.base_raw()[765..], &[255, 255, 255], "act {act}: white");
        // The 16 light ramps @0x2400 exist and differ from act to act.
        assert_eq!(pl2.light_ramps().len(), 16);
    }
    let short = Pl2::parse(&read("pal_act2.pl2")[..1024].to_vec());
    assert_eq!(short, Err(Pl2Error::TooShort));
}

/// Byte-to-byte: the base palette extracted from the PL2, read (R, G, B)
/// as stored, equals the evidence `pal_actN.dat` after the documented
/// per-triple R/B swap (the MPQ `pal.dat` release stores B, G, R —
/// `evidence/MAP-SPEC.md` §7).
#[test]
fn base_palette_matches_pal_dat_with_rb_swap() {
    for act in 1..=5 {
        let pl2 = Pl2::parse(&read(&format!("pal_act{act}.pl2"))).unwrap();
        let dat = read(&format!("pal_act{act}.dat"));
        assert_eq!(dat.len(), 768);
        for i in 0..256 {
            let (r, g, b) = (
                pl2.base_raw()[i * 3],
                pl2.base_raw()[i * 3 + 1],
                pl2.base_raw()[i * 3 + 2],
            );
            assert_eq!(
                (dat[i * 3], dat[i * 3 + 1], dat[i * 3 + 2]),
                (b, g, r),
                "act {act} entry {i}: dat is the R/B-swapped PL2 base"
            );
        }
    }
}

/// The project display convention: `assets/map/pal_act1_pl2.dat` (the
/// palette already used by the renderer for both tiles and sprites) is
/// the PL2 base upscaled 6→8 bits with `v * 255 // 252` — byte-for-byte
/// (auto-skip when the workspace assets are absent).
#[test]
fn base_rgb8_matches_the_project_display_palette() {
    let ws = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let Ok(cur) = std::fs::read(format!("{ws}/assets/map/pal_act1_pl2.dat")) else {
        return;
    };
    let pl2 = Pl2::parse(&read("pal_act1.pl2")).unwrap();
    assert_eq!(pl2.base_rgb8(), cur.as_slice(), "base_rgb8 == pal_act1_pl2.dat");
}

/// The loader's identity-ramp anchors (evidence: identity ramps observed
/// @0x2300, 0x33500, 0x53400, 0x53500 — they pin the table alignment of
/// the transform blocks) on all 5 acts.
#[test]
fn identity_ramps_at_the_documented_offsets() {
    for act in 1..=5 {
        let d = read(&format!("pal_act{act}.pl2"));
        for off in [0x2300usize, 0x33500, 0x53400, 0x53500] {
            assert_eq!(
                &d[off..off + 256],
                (0..=255u8).collect::<Vec<_>>().as_slice(),
                "act {act}: identity ramp @0x{off:X}"
            );
        }
        // The 39-byte unknown block is identical across acts (evidence).
    }
    let a1 = read("pal_act1.pl2");
    for act in 2..=5 {
        assert_eq!(
            a1[0x6B600..0x6B627],
            read(&format!("pal_act{act}.pl2"))[0x6B600..0x6B627],
            "act {act}: 39-byte block identical to act 1"
        );
    }
}

/// Renderer bridges: both color paths of the engine consume a PL2
/// directly, byte-identical to consuming the extracted display palette.
#[test]
fn renderer_bridges_feed_map_and_sprite_palettes() {
    let pl2 = Pl2::parse(&read("pal_act2.pl2")).unwrap();
    let rgb8 = pl2.base_rgb8();

    // Map path (DT1 tiles): from_pl2(gamma) == MapPalette::load(rgb8, gamma).
    let via_pl2 = map_palette_from_pl2(&pl2, 0.7);
    let via_dat = d2game::pal::MapPalette::load(&rgb8, 0.7);
    assert_eq!(via_pl2.rgb, via_dat.rgb);
    // Sanity: the act 2 palettes of the two acts differ (not aliases).
    let a3 = Pl2::parse(&read("pal_act3.pl2")).unwrap();
    assert_ne!(pl2.base_raw(), a3.base_raw(), "acts 2 and 3 have distinct palettes");

    // Sprite path (DCC frames): same index → same RGB either way (the
    // straight-RGB `sprite_palette` path).
    let sp_pl2 = sprite_palette_from_pl2(&pl2);
    let sp_dat = d2game::sprite::sprite_palette(&rgb8);
    assert_eq!(sp_pl2.rgb, sp_dat.rgb);
    assert_eq!(sp_pl2.rgb[1], [rgb8[3], rgb8[4], rgb8[5]], "entry 1 = pure red");
}
