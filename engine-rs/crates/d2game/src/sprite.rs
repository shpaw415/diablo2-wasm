//! Sprite composition — COF layers stacked in COF draw order, direction
//! picked from the 0–15 game facing, one frame at a time.
//!
//! Mirrors the validated `dcc_render compose` mode (`crates/d2dcc`):
//! `slot = dir64_to_dcc(facing * 4, num_directions)`, canvas =
//! `frames_union` of every layer's direction box, then
//! `composite_indices` in `cof.draw_order(slot, frame)` order (first =
//! bottom), colors through a BGR-fixed `d2dcc` palette. Index 0 stays
//! transparent.

use std::collections::HashMap;

use d2dcc::render::{self, Palette};
use d2dcc::{dir64_to_dcc, Cof, Dcc, DccDirection};

/// One composed frame, ready to blit.
pub struct SpriteFrame {
    /// Canvas size in px.
    pub width: i32,
    pub height: i32,
    /// Offset of the direction-box origin relative to the canvas — add to
    /// an entity anchor to place layers pixel-perfectly (see [`SpriteSet::compose`]).
    pub left: i32,
    pub top: i32,
    /// RGBA, `width * height * 4`, premultiplied-off (alpha 0 = transparent).
    pub rgba: Vec<u8>,
}

/// A playable animation set: one COF plus its layer DCCs.
pub struct SpriteSet {
    pub cof: Cof,
    /// Layer DCCs by two-letter token (HD, TR, LG, …). Missing tokens are
    /// simply absent — composition skips them (Fallen has no SH/RH).
    pub layers: HashMap<String, Dcc>,
}

impl SpriteSet {
    pub fn new(cof: Cof) -> SpriteSet {
        SpriteSet { cof, layers: HashMap::new() }
    }

    pub fn add_layer(&mut self, token: &str, dcc: Dcc) {
        self.layers.insert(token.to_ascii_uppercase(), dcc);
    }

    /// Frame count of the underlying COF (frames per direction).
    pub fn frames(&self) -> usize {
        self.cof.frames_per_direction.max(1)
    }

    /// Compose `frame` of game-facing `facing` (0–15) through `pal`.
    ///
    /// The anchor convention: the returned canvas is the union of the
    /// layers' direction boxes; the entity's world position sits at the
    /// **bottom-center** of that canvas (feet on the tile). `left`/`top`
    /// carry the box origin for callers that want exact layer geometry.
    pub fn compose(&self, facing: u8, frame: usize, pal: &Palette) -> Option<SpriteFrame> {
        let dir64 = facing as usize * 4;
        let slot = match self.layers.values().next() {
            Some(d) => dir64_to_dcc(dir64, d.number_of_directions),
            None => return None,
        };
        let slot = slot.min(self.cof.number_of_directions.saturating_sub(1));
        let frame = frame.min(self.cof.frames_per_direction.saturating_sub(1));

        let order = self.cof.draw_order(slot, frame);
        let mut ordered: Vec<&DccDirection> = Vec::new();
        for ct in order {
            if let Some(d) = self.layers.get(ct.token()) {
                let s = dir64_to_dcc(dir64, d.number_of_directions);
                ordered.push(&d.directions[s]);
            }
        }
        if ordered.is_empty() {
            return None;
        }
        let rect = render::frames_union(&ordered, frame, frame + 1);
        if rect.width <= 0 || rect.height <= 0 {
            return None;
        }
        let indices = render::composite_indices(&ordered, slot, frame, rect);
        let rgba = render::indices_to_rgba(&indices, pal);
        Some(SpriteFrame {
            width: rect.width,
            height: rect.height,
            left: rect.left,
            top: rect.top,
            rgba,
        })
    }
}

/// Display palette for sprites, straight from the corrected
/// `pal_act1_pl2.dat` (manifest `/level/palette`).
///
/// History (evidence/MAP-SPEC.md §7): the release `pal.dat` stores R and B
/// swapped, so DCC renders needed a BGR→RGB fix (`bgr = true`) — the red
/// Fallen came out blue. Since the integration of `pal_act1_pl2.dat` (the
/// true act-I PL2 base palette, already display RGB), NO swap is applied:
/// swapping again re-breaks the colors (blue barbarian bug).
pub fn sprite_palette(raw: &[u8]) -> Palette {
    Palette::load(raw, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_palette_is_straight_rgb_since_pl2() {
        // pal_act1_pl2.dat is display RGB: index (89,16,8) = earthy red
        // must come through unchanged (old pal.dat had it blue = (8,16,88)).
        let pal = sprite_palette(&[89, 16, 8]);
        assert_eq!(pal.rgb[0], [89, 16, 8]);
    }
}
