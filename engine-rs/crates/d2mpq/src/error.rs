//! Error type shared by the whole `d2mpq` crate.

use std::fmt;

/// Everything that can go wrong while opening or reading a MPQ archive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D2Error {
    /// The requested file does not exist in the archive (or is flagged
    /// non-existent / deleted).
    NotFound,
    /// A compression method bit outside of zlib (0x02) / DCL (0x08) was
    /// requested by the chunk mask byte. The payload is the full mask byte.
    UnsupportedCompression(u8),
    /// The archive or one of its streams is malformed. The payload is a
    /// static description of what was found.
    Corrupt(&'static str),
    /// Host I/O failure (only produced by the CLI layer; the library itself
    /// never touches the file system).
    Io(String),
}

impl fmt::Display for D2Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            D2Error::NotFound => write!(f, "file not found in archive"),
            D2Error::UnsupportedCompression(mask) => {
                write!(f, "unsupported compression mask 0x{mask:02x}")
            }
            D2Error::Corrupt(what) => write!(f, "corrupt archive: {what}"),
            D2Error::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for D2Error {}
