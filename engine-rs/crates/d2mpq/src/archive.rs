//! Read-only MPQ v0/v1 archive reader for Diablo 2.
//!
//! Everything happens in memory ([`Archive::open`] consumes the raw bytes),
//! so the crate builds unchanged for `wasm32-unknown-unknown` and performs no
//! file I/O. The implementation is a byte-faithful port of the validated
//! Python reference `tools/mpq_mini.py`:
//!
//! * header `MPQ\x1a` (optionally wrapped in a `MPQ\x1b` user-data header,
//!   whose u32 field at +8 is the real header offset); base offset 0
//! * hash table key = `hash_string("(hash table)", 3)`, block table key =
//!   `hash_string("(block table)", 3)`; hash entry layout `<IIHHI`
//!   (name1, name2, locale, platform, block index)
//! * free slot = every field 0xFF; probe starts at
//!   `hash_string(name, 0) & (hash_n - 1)`, linear probing
//! * sector-based reads with per-sector decryption (`key` for chunks,
//!   `key - 1` for the sector offset table) when `ENCRYPTED`
//! * `FIX_KEY` adjusts the file key to `(key + offset) ^ fsize`
//! * decompression by mask byte when `COMPRESS` (0x02 zlib, 0x08 bzip2-or-DCL
//!   for Diablo 2), pure DCL stream when `IMPLODE` (no mask byte)

use std::io::Read;

use crate::crypto::{decrypt_bytes, hash_string};
use crate::dcl;
use crate::error::D2Error;

// ----------------------------------------------------------------- MPQ flags

pub const FLAG_EXISTS: u32 = 0x8000_0000;
pub const FLAG_SINGLE_UNIT: u32 = 0x0100_0000;
pub const FLAG_KEY_INDEX_MASK: u32 = 0x0300_0000;
pub const FLAG_KEY_FIX: u32 = 0x0002_0000;
pub const FLAG_PATCH: u32 = 0x1000_0000;
pub const FLAG_DELETE: u32 = 0x0200_0000;
pub const FLAG_SECTOR_CRC: u32 = 0x0400_0000;
pub const FLAG_ENCRYPTED: u32 = 0x0001_0000;
pub const FLAG_COMPRESS: u32 = 0x0000_0200;
pub const FLAG_IMPLODE: u32 = 0x0000_0100;

// -------------------------------------------------------------- table entries

/// Decrypted hash-table entry, layout `<IIHHI`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HashEntry {
    pub name1: u32,
    pub name2: u32,
    pub locale: u16,
    pub platform: u16,
    pub block_index: u32,
}

/// Decrypted block-table entry, layout `<IIII`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockEntry {
    pub offset: u32,
    pub csize: u32,
    pub fsize: u32,
    pub flags: u32,
}

/// User-facing view of a block-table hit (what `d2tool probe` prints).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockInfo {
    pub index: u32,
    pub offset: u32,
    pub csize: u32,
    pub fsize: u32,
    pub flags: u32,
}

// ------------------------------------------------------------------- archive

/// An opened MPQ archive, fully resident in memory.
pub struct Archive {
    data: Vec<u8>,
    hash_table: Vec<HashEntry>,
    block_table: Vec<BlockEntry>,
    header_size: u32,
    archive_size: u32,
    version: u16,
    sector_shift: u16,
    sector_size: u32,
    hi_block_offset64: u64,
}

/// MPQ name normalization: `/` and `\` are equivalent (stored as `\`).
fn normalize_name(name: &str) -> String {
    name.replace('/', "\\")
}

fn u16_at(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn u32_at(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

impl Archive {
    /// Parse and open an archive from raw bytes (header + tables are decoded
    /// eagerly, exactly like the Python reference).
    pub fn open(data: Vec<u8>) -> Result<Archive, D2Error> {
        let mut header_at = 0usize;
        if data.len() < 4 {
            return Err(D2Error::Corrupt("archive too small"));
        }
        if &data[0..4] == b"MPQ\x1b" {
            // User-data wrapper: magic(4) | u32 user-data size | u32 header offset.
            if data.len() < 12 {
                return Err(D2Error::Corrupt("truncated user-data header"));
            }
            header_at = u32_at(&data, 8) as usize;
        }
        // 0x24 bytes cover magic + the eight fixed header fields; the
        // extended (v3+) hi-offset needs 0x48.
        if data.len() < header_at.checked_add(0x24).expect("usize overflow") {
            return Err(D2Error::Corrupt("truncated MPQ header"));
        }
        if &data[header_at..header_at + 4] != b"MPQ\x1a" {
            return Err(D2Error::Corrupt("not an MPQ archive"));
        }
        let header_size = u32_at(&data, header_at + 4);
        let archive_size = u32_at(&data, header_at + 8);
        let version = u16_at(&data, header_at + 12);
        let sector_shift = u16_at(&data, header_at + 14);
        let hash_off = u32_at(&data, header_at + 16);
        let block_off = u32_at(&data, header_at + 20);
        let hash_n = u32_at(&data, header_at + 24);
        let block_n = u32_at(&data, header_at + 28);
        let sector_size = 512u32
            .checked_shl(sector_shift as u32)
            .ok_or(D2Error::Corrupt("sector shift too large"))?;
        // v3+ extended header carries a 64-bit hi-block offset; our archives
        // are v0/v1 with base offset 0, so it is only recorded, never used.
        let hi_block_offset64 = if version >= 3 && header_size >= 0x44 {
            if data.len() < header_at + 0x48 {
                return Err(D2Error::Corrupt("truncated extended MPQ header"));
            }
            let mut raw = [0u8; 8];
            raw.copy_from_slice(&data[header_at + 0x40..header_at + 0x48]);
            u64::from_le_bytes(raw)
        } else {
            0
        };

        let mut archive = Archive {
            data,
            hash_table: Vec::new(),
            block_table: Vec::new(),
            header_size,
            archive_size,
            version,
            sector_shift,
            sector_size,
            hi_block_offset64,
        };
        archive.load_tables(hash_off, hash_n, block_off, block_n)?;
        Ok(archive)
    }

    /// Decrypt and decode the hash and block tables.
    fn load_tables(
        &mut self,
        hash_off: u32,
        hash_n: u32,
        block_off: u32,
        block_n: u32,
    ) -> Result<(), D2Error> {
        let hash_raw = decrypt_bytes(
            self.slice(hash_off as u64, hash_n as u64 * 16)?,
            hash_string("(hash table)", 3),
        );
        self.hash_table = hash_raw
            .as_chunks::<16>()
            .0
            .iter()
            .map(|e| HashEntry {
                name1: u32_at(e, 0),
                name2: u32_at(e, 4),
                locale: u16_at(e, 8),
                platform: u16_at(e, 10),
                block_index: u32_at(e, 12),
            })
            .collect();

        let block_raw = decrypt_bytes(
            self.slice(block_off as u64, block_n as u64 * 16)?,
            hash_string("(block table)", 3),
        );
        self.block_table = block_raw
            .as_chunks::<16>()
            .0
            .iter()
            .map(|e| BlockEntry {
                offset: u32_at(e, 0),
                csize: u32_at(e, 4),
                fsize: u32_at(e, 8),
                flags: u32_at(e, 12),
            })
            .collect();
        Ok(())
    }

    /// Borrow `len` bytes at absolute archive offset `off`, bounds-checked.
    fn slice(&self, off: u64, len: u64) -> Result<&[u8], D2Error> {
        let end = off
            .checked_add(len)
            .ok_or(D2Error::Corrupt("read out of bounds"))?;
        if end > self.data.len() as u64 {
            return Err(D2Error::Corrupt("read out of bounds"));
        }
        Ok(&self.data[off as usize..end as usize])
    }

    /// Raw hash-table probe (Python `MPQ.find`): linear probing from
    /// `hash_string(name, 0) & (hash_n - 1)`; a free slot (locale and
    /// platform both 0xFFFF) terminates the probe; a deleted slot
    /// (`block_index == 0xFFFFFFFF`) is skipped.
    fn find(&self, name: &str) -> Option<u32> {
        if self.hash_table.is_empty() {
            return None;
        }
        let norm = normalize_name(name);
        let h1 = hash_string(&norm, 1);
        let h2 = hash_string(&norm, 2);
        let mask = (self.hash_table.len() as u32) - 1;
        let start = (hash_string(&norm, 0) & mask) as usize;
        for i in 0..self.hash_table.len() {
            let idx = (start + i) & mask as usize;
            let entry = &self.hash_table[idx];
            if entry.locale == 0xFFFF && entry.platform == 0xFFFF {
                return None; // free slot: the name cannot be anywhere else
            }
            if entry.name1 == h1 && entry.name2 == h2 && entry.block_index != 0xFFFF_FFFF {
                return Some(entry.block_index);
            }
        }
        None
    }

    /// Block-table info for `name`, without flag filtering
    /// (deleted / non-existent markers included) — used by `d2tool probe`.
    pub fn block_info(&self, name: &str) -> Option<BlockInfo> {
        let index = self.find(name)?;
        let entry = *self.block_table.get(index as usize)?;
        Some(BlockInfo {
            index,
            offset: entry.offset,
            csize: entry.csize,
            fsize: entry.fsize,
            flags: entry.flags,
        })
    }

    /// True when `name` resolves to a readable file. Names are normalized
    /// (`/` == `\`) and matched through the case-insensitive MPQ hash.
    pub fn has(&self, name: &str) -> bool {
        match self.block_info(name) {
            Some(info) => info.flags & FLAG_EXISTS != 0 && info.flags & FLAG_DELETE == 0,
            None => false,
        }
    }

    /// Encryption key for a file block (Python `MPQ.file_key`).
    fn file_key(&self, name: &str, block: BlockInfo) -> u32 {
        let mut key = hash_string(&normalize_name(name), 3);
        if block.flags & FLAG_KEY_FIX != 0 {
            key = key.wrapping_add(block.offset) ^ block.fsize;
        }
        // Key-index files never occur in Diablo 2 archives, but the Python
        // reference implements the lookup, so it is kept for fidelity.
        let key_index = (block.flags & FLAG_KEY_INDEX_MASK) >> 24;
        if key_index != 0 {
            if let Some(entry) = self.block_table.get(key_index as usize) {
                key = entry.offset;
            }
        }
        key
    }

    /// Decompress one chunk according to the block flags.
    ///
    /// * `IMPLODE`: the whole chunk is a pure PKWARE DCL stream (no mask byte)
    /// * `COMPRESS`: first byte is the method mask — 0x02 zlib, 0x08
    ///   bzip2 (when the `BZh` magic is present) else DCL; any other set bit
    ///   is an [`D2Error::UnsupportedCompression`] (Diablo 2 never uses them)
    fn decompress(&self, data: &[u8], flags: u32) -> Result<Vec<u8>, D2Error> {
        if flags & FLAG_IMPLODE != 0 {
            return dcl::blast(data);
        }
        if flags & FLAG_COMPRESS != 0 {
            if data.is_empty() {
                return Ok(Vec::new());
            }
            let mask = data[0];
            let body = &data[1..];
            if mask == 0 {
                return Ok(body.to_vec());
            }
            let mut out = body.to_vec();
            for bit in 0..8u32 {
                if mask & (1 << bit) == 0 {
                    continue;
                }
                match bit {
                    1 => out = zlib_decompress(&out)?,
                    3 => {
                        // Diablo 2-era Storm accepts 0x08 = bzip2 or DCL;
                        // the bzip2 magic wins when present.
                        if out.starts_with(b"BZh") {
                            out = bz2_decompress(&out)?;
                        } else {
                            out = dcl::blast(&out)?;
                        }
                    }
                    _ => return Err(D2Error::UnsupportedCompression(mask)),
                }
            }
            return Ok(out);
        }
        Ok(data.to_vec())
    }

    /// Read and fully decode the file `name` (sector reassembly, decryption,
    /// decompression), returning its exact uncompressed bytes.
    pub fn read(&self, name: &str) -> Result<Vec<u8>, D2Error> {
        let info = self.block_info(name).ok_or(D2Error::NotFound)?;
        if info.flags & FLAG_EXISTS == 0 || info.flags & FLAG_DELETE != 0 {
            return Err(D2Error::NotFound);
        }
        let flags = info.flags;

        if flags & FLAG_SINGLE_UNIT != 0 {
            let mut chunk = self.slice(info.offset as u64, info.csize as u64)?.to_vec();
            if flags & FLAG_ENCRYPTED != 0 {
                chunk = decrypt_bytes(&chunk, self.file_key(name, info));
            }
            return self.decompress(&chunk, flags);
        }

        // Sector-based storage: a table of n_sectors + 1 u32 offsets (relative
        // to block.offset) precedes the sector data.
        let n_sectors = info.fsize.div_ceil(self.sector_size) as usize;
        let table = self
            .slice(info.offset as u64, (n_sectors as u64 + 1) * 4)?
            .to_vec();
        let table = if flags & FLAG_ENCRYPTED != 0 {
            // The offset table is encrypted with key - 1.
            decrypt_bytes(&table, self.file_key(name, info).wrapping_sub(1))
        } else {
            table
        };
        let mut offs = vec![0u32; n_sectors + 1];
        for (i, off) in offs.iter_mut().enumerate() {
            *off = u32_at(&table, i * 4);
        }

        let mut out = Vec::with_capacity(info.fsize as usize);
        for sector in 0..n_sectors {
            let start = offs[sector] as u64;
            let end = offs[sector + 1] as u64;
            let len = end
                .checked_sub(start)
                .ok_or(D2Error::Corrupt("bad sector offset table"))?;
            let mut chunk = self.slice(info.offset as u64 + start, len)?.to_vec();
            if flags & FLAG_ENCRYPTED != 0 {
                chunk = decrypt_bytes(&chunk, self.file_key(name, info));
            }
            let is_last = sector + 1 == n_sectors;
            // Storm stores a sector *uncompressed* when deflating it would
            // not save the 1-byte mask: the chunk then holds the plain
            // bytes (>= the sector's uncompressed length) with no mask
            // byte. For full sectors that length is the sector size; for
            // the last sector it is only the file bytes that remain
            // (e.g. `pal.dat`: fsize 768, csize 776, table `[8, 776]`,
            // chunk 768 bytes starting `00 00 00 00 00 24 08 18` — stored
            // raw; reading byte 0 as a mask byte 0x00 and emitting the
            // remaining 767 bytes loses the palette's last entry).
            let plain_len = ((info.fsize as u64)
                .saturating_sub(sector as u64 * self.sector_size as u64))
            .min(self.sector_size as u64) as usize;
            let stored_raw = chunk.len() >= plain_len.max(1);
            debug_assert!(is_last || plain_len == self.sector_size as usize);
            let wants_decompress = flags & (FLAG_COMPRESS | FLAG_IMPLODE) != 0;
            if wants_decompress && !stored_raw {
                out.extend_from_slice(&self.decompress(&chunk, flags)?);
            } else {
                out.extend_from_slice(&chunk);
            }
        }
        out.truncate(info.fsize as usize);
        Ok(out)
    }

    // ---------------------------------------------------------- accessors

    /// MPQ format version (0 or 1 for Diablo 2 archives).
    pub fn version(&self) -> u16 {
        self.version
    }

    /// Sector size in bytes (`512 << sector_shift`).
    pub fn sector_size(&self) -> u32 {
        self.sector_size
    }

    /// Raw `sector_shift` header field.
    pub fn sector_shift(&self) -> u16 {
        self.sector_shift
    }

    /// Header size in bytes.
    pub fn header_size(&self) -> u32 {
        self.header_size
    }

    /// Declared archive size in bytes.
    pub fn archive_size(&self) -> u32 {
        self.archive_size
    }

    /// Number of hash-table entries.
    pub fn hash_table_len(&self) -> usize {
        self.hash_table.len()
    }

    /// Number of block-table entries.
    pub fn block_table_len(&self) -> usize {
        self.block_table.len()
    }

    /// v3+ 64-bit hi-block offset (always 0 for Diablo 2 v0/v1 archives).
    pub fn hi_block_offset64(&self) -> u64 {
        self.hi_block_offset64
    }
}

// --------------------------------------------------------------- codecs

fn zlib_decompress(data: &[u8]) -> Result<Vec<u8>, D2Error> {
    let mut decoder = flate2::read::ZlibDecoder::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|_| D2Error::Corrupt("zlib decompression failed"))?;
    Ok(out)
}

#[cfg(feature = "bz2")]
fn bz2_decompress(data: &[u8]) -> Result<Vec<u8>, D2Error> {
    let mut decoder = bzip2_rs::DecoderReader::new(data);
    let mut out = Vec::new();
    decoder
        .read_to_end(&mut out)
        .map_err(|_| D2Error::Corrupt("bzip2 decompression failed"))?;
    Ok(out)
}

#[cfg(not(feature = "bz2"))]
fn bz2_decompress(_data: &[u8]) -> Result<Vec<u8>, D2Error> {
    Err(D2Error::Corrupt(
        "bzip2 payload but crate built without the \"bz2\" feature",
    ))
}
