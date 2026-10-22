//! Errors for the DCC / COF decoders.

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DccError {
    /// File signature is not 0x74 ('t').
    BadSignature(u8),
    /// The fixed header value after frames-per-direction must be 1.
    BadHeaderFlag(i32),
    /// Truncated file: needed `need` bytes at bit offset `at`, had `have`.
    Truncated { need: usize, have: usize, at: usize },
    /// `optional bytes per frame` bit count is non-zero (unsupported by retail data).
    OptionalBytesUnsupported,
    /// A frame declares itself bottom-up (unsupported by retail data).
    BottomUpFrame,
    /// A sub-bitstream read more/fewer bits than its declared size.
    BitstreamSizeMismatch {
        stream: &'static str,
        expected: usize,
        read: usize,
    },
    /// COF file too short / malformed.
    CofTooShort { need: usize, have: usize },
}

impl fmt::Display for DccError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DccError::BadSignature(s) => write!(f, "DCC signature expected 0x74, got {s:#04x}"),
            DccError::BadHeaderFlag(v) => write!(f, "DCC header flag expected 1, got {v}"),
            DccError::Truncated { need, have, at } => {
                write!(f, "truncated DCC: need {need} bytes at bit {at}, have {have}")
            }
            DccError::OptionalBytesUnsupported => {
                write!(f, "optional bytes per frame is not supported")
            }
            DccError::BottomUpFrame => write!(f, "bottom-up frames are not supported"),
            DccError::BitstreamSizeMismatch {
                stream,
                expected,
                read,
            } => write!(
                f,
                "{stream} bitstream: declared {expected} bits, read {read}"
            ),
            DccError::CofTooShort { need, have } => {
                write!(f, "COF too short: need {need} bytes, have {have}")
            }
        }
    }
}

impl std::error::Error for DccError {}
