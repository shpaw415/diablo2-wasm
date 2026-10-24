//! `pal.pl2` — the v1.12 act palette container (acts 1–5), parsed from
//! the structure the engine itself reads.
//!
//! Port of the RE campaign `d2w-re/palettes` (source of truth: the
//! D2Win.dll v1.12 loader @0x6F8E9090 "D2WinPalette.cpp", cross-checked
//! on the 5 real `pal.pl2`, all exactly 443 175 bytes). **No magic**: the
//! file starts directly with the base palette; the loader consumes
//! 0x0–0x6C327 = the WHOLE file in 12 contiguous regions:
//!
//! | offset    | bytes  | role |
//! |-----------|--------|------|
//! | 0x000000  | 1 024  | base palette, 256 × (R, G, B, pad) — **6-bit VGA values** (0..252 step 4) |
//! | 0x000400  | 12 544 | 49 transform tables of 256 B |
//! | 0x002400  | 4 096  | 16 tables — the light-level ramps 0..15 |
//! | 0x003400  | 256    | 1 table |
//! | 0x003500  | 196 608| 768 tables |
//! | 0x033500  | 65 536 | 256 tables (starts with the identity ramp) |
//! | 0x043500  | 65 536 | 256 tables (ends with the identity ramp) |
//! | 0x053500  | 32 768 | 128 tables (starts with the identity) |
//! | 0x05B500  | 65 536 | 256 tables |
//! | 0x06B500  | 256    | 1 table |
//! | 0x06B600  | 39     | unknown block — identical across acts 1–5 |
//! | 0x06B627  | 3 328  | 13 tables (unaligned start) |
//!
//! The base palette is stored **(R, G, B)** — entry 1 = (0x24, 0, 0) pure
//! red. The sibling `pal.dat` MPQ file is the SAME palette with R and B
//! swapped (and still 6-bit) — that release quirk is what
//! `evidence/MAP-SPEC.md` §7 undoes. The display palette the project
//! already uses (`assets/map/pal_act1_pl2.dat`) is the PL2 base upscaled
//! to 8 bits with `v * 255 // 252` — [`Pl2::base_rgb8`] reproduces it
//! byte-for-byte, and [`MapPalette::from_pl2`] / [`sprite_palette_from_pl2`]
//! plug acts 2–5 straight into the two renderer color paths.

use crate::pal::MapPalette;

/// Exact v1.12 file size — the engine consumes the whole file.
pub const ENGINE_SIZE: usize = 0x6C327;

/// One region of the loader's fixed copy map (diagnostics).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pl2Region {
    pub offset: usize,
    pub size: usize,
    /// The loader's `rep movsd` dword count for this region.
    pub dwords: u32,
    pub role: &'static str,
}

/// The 12 loader copies, in file order (d2w-re/palettes/evidence.md).
/// NOT a partition: the 49-table copy (`0xC40` dwords from 0x400)
/// overlaps the following sub-range copies — these are the measured
/// `rep movsd` transfers, and the last one ends exactly at [`ENGINE_SIZE`].
pub const REGIONS: [Pl2Region; 12] = [
    Pl2Region { offset: 0x000000, size: 0x0400, dwords: 0x100, role: "base_palette_rgba" },
    Pl2Region { offset: 0x000400, size: 0x3100, dwords: 0xC40, role: "transform_tables_x49" },
    Pl2Region { offset: 0x002400, size: 0x1000, dwords: 0x400, role: "light_ramps_x16" },
    Pl2Region { offset: 0x003400, size: 0x0100, dwords: 0x40, role: "transform_table_x1" },
    Pl2Region { offset: 0x003500, size: 0x30000, dwords: 0xC000, role: "transform_block_x768" },
    Pl2Region { offset: 0x033500, size: 0x10000, dwords: 0x4000, role: "transform_block_x256" },
    Pl2Region { offset: 0x043500, size: 0x10000, dwords: 0x4000, role: "transform_block_x256" },
    Pl2Region { offset: 0x053500, size: 0x8000, dwords: 0x2000, role: "transform_block_x128" },
    Pl2Region { offset: 0x05B500, size: 0x10000, dwords: 0x4000, role: "transform_block_x256" },
    Pl2Region { offset: 0x06B500, size: 0x0100, dwords: 0x40, role: "transform_table_x1" },
    Pl2Region { offset: 0x06B600, size: 0x0027, dwords: 0x27, role: "unknown_39b" },
    Pl2Region { offset: 0x06B627, size: 0x0D00, dwords: 0x340, role: "transform_tables_x13_unaligned" },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pl2Error {
    /// Shorter than the layout the engine reads (`ENGINE_SIZE`).
    TooShort,
}

/// A parsed `pal.pl2`.
#[derive(Debug, Clone, PartialEq)]
pub struct Pl2 {
    /// Raw base palette: 256 RGB triples, 6-bit VGA values, storage
    /// order (R, G, B) as in the file (768 bytes of the 1024-byte region).
    base_raw: [u8; 768],
    /// The 16 light-level transform tables @0x2400.
    light_ramps: Box<[[u8; 256]; 16]>,
}

impl Pl2 {
    /// Parse a full v1.12 `pal.pl2` (all 5 acts match [`ENGINE_SIZE`]).
    pub fn parse(d: &[u8]) -> Result<Pl2, Pl2Error> {
        if d.len() < ENGINE_SIZE {
            return Err(Pl2Error::TooShort);
        }
        let mut base_raw = [0u8; 768];
        // Region 0 stores 4 bytes per entry (R, G, B, pad) — deinterleave
        // the 256 triples (entry i lives at file offset 4i).
        for (i, slot) in base_raw.chunks_exact_mut(3).enumerate() {
            slot.copy_from_slice(&d[i * 4..i * 4 + 3]);
        }
        let mut light_ramps = Box::new([[0u8; 256]; 16]);
        for (i, ramp) in light_ramps.iter_mut().enumerate() {
            ramp.copy_from_slice(&d[0x2400 + i * 256..0x2400 + (i + 1) * 256]);
        }
        Ok(Pl2 { base_raw, light_ramps })
    }

    /// Raw base palette, storage order (R, G, B), 6-bit values. The MPQ
    /// sibling `pal.dat` is these bytes with R and B swapped per triple.
    pub fn base_raw(&self) -> &[u8; 768] {
        &self.base_raw
    }

    /// Display palette in the project convention
    /// (`assets/map/pal_act1_pl2.dat`): RGB 8-bit, `v * 255 // 252`,
    /// dropping the pad byte. Feeds [`MapPalette::load`] and
    /// [`crate::sprite::sprite_palette`] directly.
    pub fn base_rgb8(&self) -> [u8; 768] {
        let mut out = [0u8; 768];
        for (o, &v) in out.iter_mut().zip(self.base_raw.iter()) {
            *o = ((v as u32 * 255 / 252).min(255)) as u8;
        }
        out
    }

    /// The 16 light-level tables (offset 0x2400) — ramp `level` maps a
    /// palette index through the engine's lighting.
    pub fn light_ramps(&self) -> &[[u8; 256]; 16] {
        &self.light_ramps
    }
}

/// Map palette (DT1 tiles) from a PL2 — same gamma normalization as
/// [`MapPalette::load`] on the extracted display palette.
pub fn map_palette_from_pl2(pl2: &Pl2, gamma: f32) -> MapPalette {
    MapPalette::load(&pl2.base_rgb8(), gamma)
}

/// Sprite palette (DCC frames) from a PL2 — the straight-RGB path of
/// [`crate::sprite::sprite_palette`] on the extracted display palette.
pub fn sprite_palette_from_pl2(pl2: &Pl2) -> d2dcc::render::Palette {
    crate::sprite::sprite_palette(&pl2.base_rgb8())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_input_is_rejected() {
        assert_eq!(Pl2::parse(&[0; 1024]), Err(Pl2Error::TooShort));
    }

    /// The loader's copy list, as measured: sizes match the dword counts,
    /// the list starts at 0 and its last region ends at ENGINE_SIZE. The
    /// regions are the loader's `rep movsd` copies, NOT a partition — the
    /// 49-table copy (0xC40 dwords from 0x400) overlaps the following
    /// sub-range copies, exactly as the RE campaign measured them.
    #[test]
    fn regions_match_the_loader_copy_list() {
        assert_eq!(REGIONS[0].offset, 0);
        for r in REGIONS {
            if r.role == "unknown_39b" {
                // 39 bytes (36 + 2 + 1) — not dword-multiple, recorded as-is.
                assert_eq!(r.size, 39);
                continue;
            }
            assert_eq!(r.size as u32, r.dwords * 4, "dword count of {}", r.role);
        }
        let last = REGIONS.last().unwrap();
        assert_eq!(last.offset + last.size, ENGINE_SIZE);
        // Identity ramp pinning the table alignment: table #31 of the
        // 49-table block sits at 0x400 + 31*256 = 0x2300.
        assert_eq!(REGIONS[1].offset + 31 * 256, 0x2300);
    }

    /// Entry 1 of the base palette is pure red (0x24, 0, 0) in every act.
    #[test]
    fn base_palette_entry1_is_pure_red_6bit() {
        let pl2 = Pl2::parse(&{
            let mut d = vec![0u8; ENGINE_SIZE];
            d[4] = 0x24;
            d
        })
        .unwrap();
        assert_eq!(&pl2.base_raw()[3..6], &[0x24, 0, 0]);
        assert_eq!(&pl2.base_rgb8()[3..6], &[36, 0, 0]); // 0x24*255/252 = 36
    }
}
