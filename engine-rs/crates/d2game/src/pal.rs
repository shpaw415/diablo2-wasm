//! Palette handling for the two color paths of the engine.
//!
//! Two validated looks come out of the same `pal_act1.dat` (767 bytes —
//! zero-padded to 768, `assets/manifest.json` documents the truncation):
//!
//! * **Map** (DT1 tiles): raw channel order + gamma normalization 0.55 —
//!   exactly what the validated `assets/map/level-render.png` uses (there
//!   is no PL2 in this release; the prototype `tools/map_proto2.py`
//!   normalizes gamma for readability).
//! * **Sprites** (DCC frames): BGR on disk → swap to RGB, no gamma — what
//!   the validated `assets/dcc/composite.png` uses
//!   (`evidence/DCC-SPEC.md` §2: rendering the BGR triple as RGB turns the
//!   red Fallen blue).

/// Map palette: 256 RGB triples, gamma-normalized (validated look).
pub struct MapPalette {
    /// Flattened RGB, 768 bytes.
    pub rgb: [u8; 768],
}

impl MapPalette {
    /// `gamma = 0.55` reproduces `tools/map_proto2.py:load_palette`.
    pub fn load(raw: &[u8], gamma: f32) -> MapPalette {
        let mut rgb = [0u8; 768];
        for i in 0..768 {
            let v = raw.get(i).copied().unwrap_or(0);
            let g = 255.0 * (v as f32 / 255.0).powf(gamma);
            rgb[i] = g.round().clamp(0.0, 255.0) as u8;
        }
        MapPalette { rgb }
    }

    /// RGBA lookup (alpha 255) for one palette index.
    #[inline]
    pub fn rgba(&self, idx: u8) -> [u8; 4] {
        let i = idx as usize * 3;
        [self.rgb[i], self.rgb[i + 1], self.rgb[i + 2], 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamma_zero_stays_zero_and_full_stays_full() {
        let pal = MapPalette::load(&[0, 255, 128], 0.55);
        assert_eq!(pal.rgba(0)[0], 0); // gamma(0) = 0
        assert_eq!(pal.rgba(0)[1], 255); // gamma(255) = 255
        let boosted = pal.rgba(0)[2] as u32; // gamma boosts midtones
        assert!(boosted > 128 && boosted < 255, "gamma boosts midtones: {boosted}");
    }

    #[test]
    fn short_palette_is_zero_padded() {
        let pal = MapPalette::load(&[], 0.55);
        assert!(pal.rgb.iter().all(|&v| v == 0));
    }
}
