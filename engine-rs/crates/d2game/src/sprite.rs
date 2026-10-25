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

use d2animdata::AnimationDataRecord;
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
    /// AnimData.d2 record of this animation (key = COF file stem, e.g.
    /// `BAWL1HS`), when the optional asset `assets/animdata/AnimData.d2`
    /// was provided to [`GameCore`]. When set, the record's official
    /// `frames_per_direction`/`speed` take precedence over the COF header
    /// bytes (audit od2-features-formats #19 — the retail COF speed byte is
    /// unreliable: FAWLHTH.COF stores 0 where AnimData.d2 says 256 = 25 fps).
    pub animdata: Option<AnimationDataRecord>,
}

impl SpriteSet {
    pub fn new(cof: Cof) -> SpriteSet {
        SpriteSet { cof, layers: HashMap::new(), animdata: None }
    }

    pub fn add_layer(&mut self, token: &str, dcc: Dcc) {
        self.layers.insert(token.to_ascii_uppercase(), dcc);
    }

    /// Branch the official AnimData.d2 record (frames/speed) onto this set.
    /// `GameCore::finalize` does it automatically when the file was
    /// provided; without it the set keeps the COF-header fallback.
    pub fn apply_animdata(&mut self, record: &AnimationDataRecord) {
        self.animdata = Some(record.clone());
    }

    /// Frames per direction — AnimData.d2 when branched, else the COF
    /// header byte (historical behavior).
    pub fn frames(&self) -> usize {
        let from_animdata = self
            .animdata
            .as_ref()
            .map(|r| r.frames_per_direction as usize);
        from_animdata.unwrap_or(self.cof.frames_per_direction).max(1)
    }

    /// Official FPS (25 × speed ÷ 256) when AnimData.d2 is branched, `None`
    /// otherwise (the COF speed byte has no reliable fps meaning).
    pub fn fps(&self) -> Option<f64> {
        self.animdata.as_ref().map(|r| r.fps())
    }

    /// Official per-frame duration in ms when AnimData.d2 is branched.
    pub fn frame_duration_ms(&self) -> Option<f64> {
        self.animdata.as_ref().map(|r| r.frame_duration_ms())
    }

    /// Compose `frame` of game-facing `facing` (0–15) through `pal`.
    ///
    /// The anchor convention: the returned canvas is the union of the
    /// layers' direction boxes; the entity's world position sits at the
    /// **bottom-center** of that canvas (feet on the tile). `left`/`top`
    /// carry the box origin for callers that want exact layer geometry.
    /// La couche `token` est-elle présente dans le set ? (tests FIX-HEROS.)
    pub fn has_layer(&self, token: &str) -> bool {
        self.layers.contains_key(token)
    }

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
        // FIX-HEROS : la couche LG (jambes) n'existe pas dans l'enum COF
        // (0=HD..14=S8, evidence/DCC-SPEC.md §7.3) — draw_order ne la
        // renverra jamais. Empilée en bas de pile (dessinée en premier, sous
        // torse/bras/tête), aux coords de sa propre box DCC ; l'union des
        // boxes l'intègre sans rognure.
        if let Some(lg) = self.layers.get("LG") {
            let s = dir64_to_dcc(dir64, lg.number_of_directions);
            ordered.push(&lg.directions[s]);
        }
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

    #[test]
    fn animdata_record_takes_precedence_over_cof_fallback() {
        // COF synthétique : 4 dirs × 10 frames × 1 couche (TR).
        let mut cof_bytes: Vec<u8> = vec![
            1, 10, 4, // layers, frames_per_direction, directions
            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 21 unknown
            0, // speed (octet COF — la valeur FA réelle, 0, y est bien)
            0, 0, 0, // 3 unknown body
            // 1 layer: type TR, shadow/selectable/transparent/effect, wclass "HTH\0"
            1, 0, 0, 0, 0, b'H', b'T', b'H', 0,
        ];
        cof_bytes.extend_from_slice(&[0; 10]); // animation_frames
        cof_bytes.extend_from_slice(&[1; 40]); // priority 4 dirs × 10 frames × 1 layer
        let cof = Cof::parse(&cof_bytes).unwrap();
        let mut set = SpriteSet::new(cof);

        // Fallback (pas d'animdata) : comportement historique — octets COF.
        assert_eq!(set.frames(), 10);
        assert!(set.fps().is_none());
        assert!(set.frame_duration_ms().is_none());

        // Branché : frames/speed officiels prennent la main.
        let rec = d2animdata::AnimationDataRecord {
            name: "FAWLHTH".into(),
            frames_per_direction: 8, // volontairement ≠ COF pour prouver la priorité
            speed: 256,
            events: [0; 144],
        };
        set.apply_animdata(&rec);
        assert_eq!(set.frames(), 8);
        assert_eq!(set.fps(), Some(25.0));
        assert_eq!(set.frame_duration_ms(), Some(40.0));
    }
}
