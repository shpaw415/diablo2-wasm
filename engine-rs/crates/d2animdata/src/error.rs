//! Error type of the AnimData.d2 loader (mirrors the error paths of the
//! OpenDiablo2 Go `Load`, animdata.go l.121-209).

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimDataError {
    /// Ran out of bytes mid-structure (Go: stream reader `ReadX` errors).
    TooShort {
        need: usize,
        have: usize,
    },
    /// A block declares more than [`crate::MAX_RECORDS_PER_BLOCK`] records
    /// (Go: "more than 67 records in block", animdata.go l.134).
    TooManyRecords {
        block: usize,
        count: u32,
    },
    /// Record name is not NUL-terminated in its 8-byte field
    /// (Go: "name missing null terminator byte", animdata.go l.146).
    MissingNullTerminator {
        block: usize,
        record: u32,
    },
    /// Trailing bytes after the 256 blocks (Go: "unable to parse animation
    /// data: pos != len", animdata.go l.205).
    LengthMismatch {
        pos: usize,
        len: usize,
    },
}

impl fmt::Display for AnimDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AnimDataError::TooShort { need, have } => {
                write!(f, "animdata too short: need {need} bytes, have {have}")
            }
            AnimDataError::TooManyRecords { block, count } => {
                write!(f, "block {block}: more than {} records ({count})", crate::MAX_RECORDS_PER_BLOCK)
            }
            AnimDataError::MissingNullTerminator { block, record } => {
                write!(f, "block {block} record {record}: name missing null terminator byte")
            }
            AnimDataError::LengthMismatch { pos, len } => {
                write!(f, "unable to parse animation data: {pos} != {len}")
            }
        }
    }
}

impl std::error::Error for AnimDataError {}
