//! COF file — Composite Object Format: which layers make an animation,
//! in which order to draw them per frame, at what speed.
//!
//! Byte-faithful port of OpenDiablo2 `d2cof` (cof.go / cof_layer.go).
//!
//! Layout (all little-endian, plain bytes):
//! ```text
//! [0]  number of layers
//! [1]  frames per direction
//! [2]  number of directions
//! [3..24]  21 unknown header bytes
//! [24] speed (game ticks per frame basis)
//! [25..28] 3 unknown body bytes
//! layers × 9 bytes: type, shadow, selectable, transparent, draw effect,
//!                   weapon class (4 bytes, zero-padded)
//! frames × 1 byte: frame mode ("animation frames")
//! dirs × frames × layers bytes: composite draw priority per frame
//! ```

use crate::error::DccError;

/// Composite layer type — the D2 token letters used in file names.
/// Port of OpenDiablo2 `d2enum.CompositeType`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u8)]
pub enum CompositeType {
    #[default]
    Head = 0, // HD
    Torso = 1, // TR
    LeftArm = 2, // LA
    RightArm = 3, // RA
    LeftHand = 4, // LH
    RightHand = 5, // RH
    Shield = 6, // SH
    Special1 = 7, // S1
    Special2 = 8, // S2
    Special3 = 9, // S3
    Special4 = 10, // S4
    Special5 = 11, // S5
    Special6 = 12, // S6
    Special7 = 13, // S7
    Special8 = 14, // S8
}

impl CompositeType {
    pub fn from_u8(v: u8) -> Option<CompositeType> {
        if v <= 14 {
            Some(unsafe { std::mem::transmute::<u8, CompositeType>(v) })
        } else {
            None
        }
    }

    /// Two-letter D2 token (HD, TR, LA, RA, LH, RH, SH, S1..S8).
    pub fn token(self) -> &'static str {
        match self {
            CompositeType::Head => "HD",
            CompositeType::Torso => "TR",
            CompositeType::LeftArm => "LA",
            CompositeType::RightArm => "RA",
            CompositeType::LeftHand => "LH",
            CompositeType::RightHand => "RH",
            CompositeType::Shield => "SH",
            CompositeType::Special1 => "S1",
            CompositeType::Special2 => "S2",
            CompositeType::Special3 => "S3",
            CompositeType::Special4 => "S4",
            CompositeType::Special5 => "S5",
            CompositeType::Special6 => "S6",
            CompositeType::Special7 => "S7",
            CompositeType::Special8 => "S8",
        }
    }
}

/// One COF layer record (cof_layer.go `CofLayer`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CofLayer {
    pub layer_type: CompositeType,
    pub shadow: u8,
    pub selectable: bool,
    pub transparent: bool,
    pub draw_effect: u8,
    pub weapon_class: String,
}

/// Port of OpenDiablo2 `COF` (cof.go).
#[derive(Debug, Clone, Default)]
pub struct Cof {
    pub number_of_layers: usize,
    pub frames_per_direction: usize,
    pub number_of_directions: usize,
    pub speed: u8,
    pub unknown_header_bytes: [u8; 21],
    pub unknown_body_bytes: [u8; 3],
    pub layers: Vec<CofLayer>,
    pub animation_frames: Vec<u8>,
    /// `priority[direction][frame]` = composite types in draw order
    /// (first = bottom).
    pub priority: Vec<Vec<Vec<CompositeType>>>,
}

const NUM_HEADER_BYTES: usize = 25; // 3 counts + 21 unknown + speed
const NUM_LAYER_BYTES: usize = 9;

impl Cof {
    /// Parse a COF from its raw file bytes (cof.go `Unmarshal`).
    pub fn parse(data: &[u8]) -> Result<Cof, DccError> {
        if data.len() < NUM_HEADER_BYTES {
            return Err(DccError::CofTooShort {
                need: NUM_HEADER_BYTES,
                have: data.len(),
            });
        }

        let mut cof = Cof {
            number_of_layers: data[0] as usize,
            frames_per_direction: data[1] as usize,
            number_of_directions: data[2] as usize,
            unknown_header_bytes: data[3..24].try_into().unwrap(),
            speed: data[24],
            ..Default::default()
        };

        let mut pos = NUM_HEADER_BYTES;
        if data.len() < pos + 3 {
            return Err(DccError::CofTooShort {
                need: pos + 3,
                have: data.len(),
            });
        }
        cof.unknown_body_bytes = data[pos..pos + 3].try_into().unwrap();
        pos += 3;

        for _ in 0..cof.number_of_layers {
            if data.len() < pos + NUM_LAYER_BYTES {
                return Err(DccError::CofTooShort {
                    need: pos + NUM_LAYER_BYTES,
                    have: data.len(),
                });
            }
            let b = &data[pos..pos + NUM_LAYER_BYTES];
            pos += NUM_LAYER_BYTES;
            let layer = CofLayer {
                layer_type: CompositeType::from_u8(b[0]).unwrap_or_default(),
                shadow: b[1],
                selectable: b[2] > 0,
                transparent: b[3] > 0,
                draw_effect: b[4],
                weapon_class: String::from_utf8_lossy(&b[5..])
                    .trim_end_matches('\0')
                    .trim()
                    .to_string(),
            };
            cof.layers.push(layer);
        }

        if data.len() < pos + cof.frames_per_direction {
            return Err(DccError::CofTooShort {
                need: pos + cof.frames_per_direction,
                have: data.len(),
            });
        }
        cof.animation_frames = data[pos..pos + cof.frames_per_direction].to_vec();
        pos += cof.frames_per_direction;

        let priority_len = cof.frames_per_direction * cof.number_of_directions * cof.number_of_layers;
        if data.len() < pos + priority_len {
            return Err(DccError::CofTooShort {
                need: pos + priority_len,
                have: data.len(),
            });
        }

        let mut idx = pos;
        for _ in 0..cof.number_of_directions {
            let mut per_dir = Vec::with_capacity(cof.frames_per_direction);
            for _ in 0..cof.frames_per_direction {
                let mut per_frame = Vec::with_capacity(cof.number_of_layers);
                for _ in 0..cof.number_of_layers {
                    per_frame.push(CompositeType::from_u8(data[idx]).unwrap_or_default());
                    idx += 1;
                }
                per_dir.push(per_frame);
            }
            cof.priority.push(per_dir);
        }

        Ok(cof)
    }

    /// Draw order of composite types for `direction`/`frame`
    /// (first entry drawn first, i.e. bottom-most).
    pub fn draw_order(&self, direction: usize, frame: usize) -> &[CompositeType] {
        &self.priority[direction][frame]
    }

    /// DCC file-stem for each layer of an animation whose COF token is
    /// `token` (e.g. `BAWL1HS`): `<token><LAYER>` per layer, in COF order.
    ///
    /// The D2 file layout puts layer DCCs at
    /// `data\global\chars\<class>\<layer>\<class><layer>LIT<mode><wclass>.dcc`
    /// (constant `LIT` token) for characters and
    /// `data\global\monsters\<code>\<layer>\<code><layer>LIT<mode><wclass>.dcc`
    /// for monsters — discovered empirically by hash-probing the retail
    /// MPQs (see evidence/DCC-SPEC.md).
    pub fn dcc_stems(&self, token: &str) -> Vec<String> {
        self.layers
            .iter()
            .map(|l| format!("{token}{}", l.layer_type.token()))
            .collect()
    }
}
