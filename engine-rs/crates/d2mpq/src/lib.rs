//! # d2mpq
//!
//! Minimal read-only MPQ archive reader for Diablo 2 (v0/v1), written as a
//! byte-faithful Rust port of the validated Python reference
//! `tools/mpq_mini.py` + `tools/blast_pkware.py` (itself derived from Mark
//! Adler's `blast.c`).
//!
//! The crate is WASM-first: [`Archive::open`] takes the raw archive bytes and
//! everything is resolved in memory — no file system access, no C
//! dependencies. Crypto (Storm table, string hash, dword cipher) and the
//! PKWARE DCL decompressor are implemented in-crate; zlib goes through
//! `flate2` (pure-Rust `miniz_oxide` backend) and bzip2 through `bzip2-rs`
//! behind the default `bz2` feature.
//!
//! ```no_run
//! use d2mpq::Archive;
//!
//! let raw = std::fs::read("data/d2data.mpq").unwrap();
//! let archive = Archive::open(raw).unwrap();
//! if archive.has("data\\global\\excel\\armor.txt") {
//!     let bytes = archive.read("data/global/excel/armor.txt").unwrap();
//!     println!("{} bytes", bytes.len());
//! }
//! ```

mod archive;
mod crypto;
mod dcl;
mod error;

pub use archive::{
    Archive, BlockEntry, BlockInfo, HashEntry, FLAG_COMPRESS, FLAG_DELETE, FLAG_ENCRYPTED,
    FLAG_EXISTS, FLAG_IMPLODE, FLAG_KEY_FIX, FLAG_KEY_INDEX_MASK, FLAG_PATCH, FLAG_SECTOR_CRC,
    FLAG_SINGLE_UNIT,
};
pub use crypto::{decrypt_bytes, encrypt_bytes, hash_string};
pub use dcl::blast;
pub use error::D2Error;
