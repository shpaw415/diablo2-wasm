//! Palette + composition helpers for rendering decoded DCC frames.
//!
//! Everything here is pure (no I/O) so it is unit-testable and usable from
//! WASM later. The bin `dcc_render` wires it to files.
//!
//! Two traps are baked in and covered by tests:
//!
//! * **Palette channel order is BGR.** The act-1 `pal.dat` stores each
//!   entry as `[blue, green, red]` — rendering the triple as RGB turns the
//!   Fallen (a red demon) blue. Proof bytes in `evidence/DCC-SPEC.md`.
//! * **Index 0 is transparent.** Composition starts from a zeroed canvas
//!   and never writes index 0, so unpainted pixels stay transparent.

use crate::direction::DccDirection;

/// Display palette: 256 RGB entries, channel order already fixed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    pub rgb: [[u8; 3]; 256],
}

impl Palette {
    /// Build a display palette from raw `pal.dat` bytes (768, or 767 for
    /// the truncated-file case — padded with zeros).
    ///
    /// `bgr = true` (default for Diablo 2 `pal.dat`) interprets each stored
    /// triple as `[blue, green, red]`.
    pub fn load(raw: &[u8], bgr: bool) -> Palette {
        let mut data = raw.to_vec();
        while data.len() < 768 {
            data.push(0);
        }
        let mut rgb = [[0u8; 3]; 256];
        for (i, out) in rgb.iter_mut().enumerate() {
            let b = data[i * 3];
            let g = data[i * 3 + 1];
            let r = data[i * 3 + 2];
            *out = if bgr { [r, g, b] } else { [b, g, r] };
        }
        Palette { rgb }
    }
}

/// Axis-aligned rectangle in direction-box coordinate space.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub left: i32,
    pub top: i32,
    pub width: i32,
    pub height: i32,
}

/// Union of the frame boxes of `frames[f0..f1]` across all `directions`
/// (one per COF layer, same DCC direction slot). Every decoded frame's
/// pixels live inside its own frame box, so this is the tight canvas for
/// composing those frames without per-frame jitter.
pub fn frames_union(directions: &[&DccDirection], f0: usize, f1: usize) -> Rect {
    let mut left = i32::MAX;
    let mut top = i32::MAX;
    let mut right = i32::MIN;
    let mut bottom = i32::MIN;
    for dir in directions {
        for f in &dir.frames[f0..f1.min(dir.frames.len())] {
            left = left.min(f.box_left);
            top = top.min(f.box_top);
            right = right.max(f.box_left + f.box_width);
            bottom = bottom.max(f.box_top + f.box_height);
        }
    }
    Rect {
        left,
        top,
        width: right - left,
        height: bottom - top,
    }
}

/// Composite `directions` (already in COF draw order, first = bottom) over
/// `rect`, returning `width * height` palette indices, 0 = transparent.
///
/// Each frame's `pixel_data` covers its whole *direction* box (the crate
/// decodes it that way), so painting goes through the direction box offset
/// with bounds clipping — never through the frame box stride.
pub fn composite_indices(
    directions: &[&DccDirection],
    _slot: usize,
    frame: usize,
    rect: Rect,
) -> Vec<u8> {
    let mut canvas = vec![0u8; (rect.width.max(0) as usize) * (rect.height.max(0) as usize)];
    let w = rect.width as usize;
    for dir in directions {
        let Some(d) = dir.frames.get(frame) else {
            continue;
        };
        let dir_w = dir.box_width;
        for y in 0..dir.box_height.max(0) as usize {
            let cy = y as i32 + dir.box_top - rect.top;
            if cy < 0 || cy >= rect.height {
                continue;
            }
            for x in 0..dir_w.max(0) as usize {
                let p = d.pixel_data[x + y * dir_w.max(0) as usize];
                if p == 0 {
                    continue; // transparent: keep what lower layers painted
                }
                let cx = x as i32 + dir.box_left - rect.left;
                if cx < 0 || cx >= rect.width {
                    continue;
                }
                canvas[cx as usize + cy as usize * w] = p;
            }
        }
    }
    canvas
}

/// Map palette indices to RGBA. Index 0 -> alpha 0.
pub fn indices_to_rgba(indices: &[u8], pal: &Palette) -> Vec<u8> {
    let mut out = Vec::with_capacity(indices.len() * 4);
    for &p in indices {
        let c = pal.rgb[p as usize];
        out.extend_from_slice(&[c[0], c[1], c[2], if p == 0 { 0 } else { 255 }]);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direction::DccDirection;
    use crate::frame::DccDirectionFrame;

    fn fake_dir(painted: &[(i32, i32, u8)], box_left: i32, box_top: i32, w: i32, h: i32) -> DccDirection {
        let mut dir = DccDirection::default();
        dir.box_left = box_left;
        dir.box_top = box_top;
        dir.box_width = w;
        dir.box_height = h;
        let mut frame = DccDirectionFrame::default();
        frame.box_left = box_left;
        frame.box_top = box_top;
        frame.box_width = w;
        frame.box_height = h;
        let mut px = vec![0u8; (w * h) as usize];
        for &(x, y, v) in painted {
            px[(x as usize) + (y as usize) * w as usize] = v;
        }
        frame.pixel_data = px;
        dir.frames.push(frame);
        dir
    }

    #[test]
    fn palette_is_bgr_and_pads_short_files() {
        // Stored triple [0x11, 0x22, 0x33] must read R=0x33 G=0x22 B=0x11.
        let mut raw = vec![0x11u8, 0x22, 0x33];
        raw.resize(765, 0); // 768 - 3: simulates the truncated 767-byte file
        let pal = Palette::load(&raw, true);
        assert_eq!(pal.rgb[0], [0x33, 0x22, 0x11]);
        assert_eq!(pal.rgb[255], [0, 0, 0]); // zero padding, not garbage
        let pal_rgb = Palette::load(&raw, false);
        assert_eq!(pal_rgb.rgb[0], [0x11, 0x22, 0x33]);
    }

    #[test]
    fn index_zero_stays_transparent_and_later_layers_win() {
        // Bottom layer paints red at (1,0); top layer paints only at (0,0).
        let bottom = fake_dir(&[(0, 0, 10), (1, 0, 20)], 0, 0, 2, 1);
        let top = fake_dir(&[(0, 0, 30)], 0, 0, 2, 1);
        let rect = frames_union(&[&bottom, &top], 0, 1);
        assert_eq!(rect, Rect { left: 0, top: 0, width: 2, height: 1 });
        let idx = composite_indices(&[&bottom, &top], 0, 0, rect);
        assert_eq!(idx, vec![30, 20]);
        let rgba = indices_to_rgba(&idx, &Palette::load(&[0; 768], true));
        assert_eq!(rgba, vec![0, 0, 0, 255, 0, 0, 0, 255]);
    }

    #[test]
    fn frames_union_spans_layers_and_frames() {
        let a = fake_dir(&[], -10, -20, 4, 4);
        let b = fake_dir(&[], 2, 6, 4, 4);
        let rect = frames_union(&[&a, &b], 0, 1);
        assert_eq!(rect, Rect { left: -10, top: -20, width: 16, height: 30 });
    }

    #[test]
    fn composite_offsets_respect_direction_box_origin() {
        // The direction box is bigger than the frame box; pixel_data is
        // direction-box-sized and the frame sits at (1,1) inside it.
        let mut dir = fake_dir(&[(0, 0, 7)], 1, 1, 3, 3);
        dir.frames[0].box_left = 1;
        dir.frames[0].box_top = 1;
        dir.frames[0].box_width = 1;
        dir.frames[0].box_height = 1;
        let rect = frames_union(&[&dir], 0, 1);
        assert_eq!(rect, Rect { left: 1, top: 1, width: 1, height: 1 });
        let idx = composite_indices(&[&dir], 0, 0, rect);
        assert_eq!(idx, vec![7]);
    }
}
