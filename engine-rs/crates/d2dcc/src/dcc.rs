//! DCC file — Diablo 2 character/monster sprite animation.
//!
//! Byte-faithful port of OpenDiablo2 `d2dcc` (dcc.go). A DCC holds 1, 4 or
//! 32 directions; each direction decodes to `frames_per_direction` frames
//! of 8-bit palette indices laid out in the direction box.

use crate::bitmuncher::BitMuncher;
use crate::direction::DccDirection;
use crate::error::DccError;

pub const DCC_SIGNATURE: u8 = 0x74;
const DIRECTION_OFFSET_MULTIPLIER: usize = 8;

/// Port of OpenDiablo2 `DCC` (dcc.go).
#[derive(Debug, Clone, Default)]
pub struct Dcc {
    pub signature: u8,
    pub version: u8,
    pub number_of_directions: usize,
    pub frames_per_direction: usize,
    pub directions: Vec<DccDirection>,
    direction_offsets: Vec<i32>,
}

impl Dcc {
    /// Decode a whole DCC from its raw file bytes.
    pub fn decode(file_data: &[u8]) -> Result<Dcc, DccError> {
        if file_data.is_empty() {
            return Err(DccError::BadSignature(0));
        }

        let mut bm = BitMuncher::new(file_data, 0);

        let signature = bm.get_byte();
        if signature != DCC_SIGNATURE {
            return Err(DccError::BadSignature(signature));
        }

        let mut dcc = Dcc {
            signature,
            version: bm.get_byte(),
            number_of_directions: bm.get_byte() as usize,
            frames_per_direction: bm.get_int32() as usize,
            ..Default::default()
        };

        if bm.get_int32() != 1 {
            return Err(DccError::BadHeaderFlag(0));
        }

        let _total_size_coded = bm.get_int32();

        for _ in 0..dcc.number_of_directions {
            dcc.direction_offsets.push(bm.get_int32());
        }

        for i in 0..dcc.number_of_directions {
            let byte_offset = dcc.direction_offsets[i] as usize * DIRECTION_OFFSET_MULTIPLIER;
            if byte_offset >= file_data.len() * 8 {
                return Err(DccError::Truncated {
                    need: byte_offset / 8 + 1,
                    have: file_data.len(),
                    at: dcc.direction_offsets[i] as usize,
                });
            }
            let mut dir_bm = BitMuncher::new(file_data, byte_offset);
            dcc.directions
                .push(DccDirection::parse(&mut dir_bm, dcc.frames_per_direction)?);
        }

        Ok(dcc)
    }

    /// Byte offset (in the file) of each direction's header.
    pub fn direction_offsets(&self) -> &[i32] {
        &self.direction_offsets
    }
}
