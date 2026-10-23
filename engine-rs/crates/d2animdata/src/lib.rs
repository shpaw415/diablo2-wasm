//! d2animdata — Diablo 2 `AnimData.d2`: the authoritative frames-per-direction,
//! speed and per-frame trigger events for every animation token in the game.
//!
//! Byte-faithful Rust port of the OpenDiablo2 Go package `d2animdata`
//! (animdata.go / block.go / record.go / events.go / hash.go — reference in
//! `/home/agent/workspace/opendiablo2/d2common/d2fileformats/d2animdata`).
//! No file I/O — the crate builds unchanged for `wasm32-unknown-unknown`.
//!
//! File layout (little-endian, v1.12 retail file = 570 304 bytes):
//! ```text
//! 256 blocks; block = u32 record_count (≤ 67), then records:
//!   record (160 B) = name 8 B (NUL-terminated) + u32 frames_per_direction
//!                  + u16 speed + 2 pad + 144 event bytes
//! ```
//! A record's name IS its block key: `hash_name(name)` (sum of uppercase
//! bytes mod 256) places it, so a token lookup is O(block) — unit-tested on
//! the real file where the relation holds for all 3 558 records.
//!
//! Why this matters (audit `docs/parity/od2-features-formats.md` #19): the
//! COF header does NOT reliably carry frames/speed — e.g. the retail
//! `FAWLHTH.COF` stores a speed byte of 0 while AnimData.d2 says 256
//! (25 FPS). The game's timing values come from here.
//!
//! ```no_run
//! use d2animdata::AnimationData;
//!
//! let bytes = std::fs::read("AnimData.d2").unwrap();
//! let ad = AnimationData::load(&bytes).unwrap();
//! if let Some(rec) = ad.get_record("BAWL1HS") {
//!     println!("{} frames at {} fps", rec.frames_per_direction, rec.fps());
//! }
//! ```

mod error;
mod record;

pub use error::AnimDataError;
pub use record::{AnimationDataRecord, AnimationEvent};

/// Number of blocks in the file (animdata.go `numBlocks`).
pub const NUM_BLOCKS: usize = 256;
/// Hard cap of records per block (animdata.go `maxRecordsPerBlock`).
pub const MAX_RECORDS_PER_BLOCK: u32 = 67;
const BYTE_COUNT_NAME: usize = 8; // animdata.go byteCountName
const BYTE_COUNT_SPEED_PADDING: usize = 2; // animdata.go byteCountSpeedPadding
/// Per-frame trigger event slots (animdata.go `numEvents`).
pub const NUM_EVENTS: usize = 144;

/// Fixed record size: name + fpd + speed + padding + events = 160 B.
const RECORD_SIZE: usize = BYTE_COUNT_NAME + 4 + 2 + BYTE_COUNT_SPEED_PADDING + NUM_EVENTS;

/// hash.go `hashName`: sum of the uppercase ASCII bytes, mod
/// [`NUM_BLOCKS`]. Case-insensitive by construction ("bawl1hs" == "BAWL1HS").
pub fn hash_name(name: &str) -> u8 {
    let hash: u32 = name
        .bytes()
        .map(|b| b.to_ascii_uppercase() as u32)
        .sum();
    (hash % NUM_BLOCKS as u32) as u8
}

/// One block: record count as stored, then the records (block.go `block`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Block {
    pub record_count: u32,
    pub records: Vec<AnimationDataRecord>,
}

/// Where a named record lives: `(block index, record index)` — the Rust
/// stand-in for the Go `map[string][]*AnimationDataRecord` of pointers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RecordRef(u16, u16);

/// Representation of the binary data from `data\global\AnimData.d2`
/// (animdata.go `AnimationData`).
#[derive(Debug, Clone, Default)]
pub struct AnimationData {
    blocks: Vec<Block>,
    /// name → all homonym records, in file order (Go `entries`).
    entries: std::collections::HashMap<String, Vec<RecordRef>>,
    total_records: usize,
}

impl AnimationData {
    /// Load (parse) the data — animdata.go `Load`. Strictly validates like
    /// the Go version: ≤ 67 records per block, NUL-terminated names, and the
    /// whole input must be consumed exactly.
    pub fn load(data: &[u8]) -> Result<AnimationData, AnimDataError> {
        let mut pos = 0usize;
        let mut blocks = Vec::with_capacity(NUM_BLOCKS);
        let mut entries: std::collections::HashMap<String, Vec<RecordRef>> =
            std::collections::HashMap::new();
        let mut total = 0usize;

        for block_idx in 0..NUM_BLOCKS {
            let cntb = data
                .get(pos..pos + 4)
                .ok_or(AnimDataError::TooShort { need: pos + 4, have: data.len() })?;
            let record_count = u32::from_le_bytes(cntb.try_into().unwrap());
            pos += 4;

            if record_count > MAX_RECORDS_PER_BLOCK {
                return Err(AnimDataError::TooManyRecords { block: block_idx, count: record_count });
            }

            let mut records = Vec::with_capacity(record_count as usize);
            for record_idx in 0..record_count {
                let rec = data.get(pos..pos + RECORD_SIZE).ok_or(AnimDataError::TooShort {
                    need: pos + RECORD_SIZE,
                    have: data.len(),
                })?;
                pos += RECORD_SIZE;

                // animdata.go l.146: the 8-byte name must be NUL-terminated.
                if rec[BYTE_COUNT_NAME - 1] != 0 {
                    return Err(AnimDataError::MissingNullTerminator {
                        block: block_idx,
                        record: record_idx,
                    });
                }
                let name_bytes: Vec<u8> = rec[..BYTE_COUNT_NAME].iter().copied().filter(|&b| b != 0).collect();
                let name = String::from_utf8_lossy(&name_bytes).into_owned();

                let frames_per_direction =
                    u32::from_le_bytes(rec[8..12].try_into().unwrap());
                let speed = u16::from_le_bytes(rec[12..14].try_into().unwrap());
                let events: [u8; NUM_EVENTS] = rec[16..RECORD_SIZE].try_into().unwrap();

                let record = AnimationDataRecord { name, frames_per_direction, speed, events };
                entries.entry(record.name.clone()).or_default().push(RecordRef(
                    block_idx as u16,
                    record_idx as u16,
                ));
                records.push(record);
                total += 1;
            }

            blocks.push(Block { record_count, records });
        }

        // animdata.go l.205: every byte must have been consumed.
        if pos != data.len() {
            return Err(AnimDataError::LengthMismatch { pos, len: data.len() });
        }

        Ok(AnimationData { blocks, entries, total_records: total })
    }

    /// All homonym records with this name, in file order (Go `GetRecords`) —
    /// the file has multiple records with the same name but different values.
    pub fn get_records(&self, name: &str) -> Vec<&AnimationDataRecord> {
        match self.entries.get(name) {
            Some(refs) => refs
                .iter()
                .map(|&RecordRef(b, r)| &self.blocks[b as usize].records[r as usize])
                .collect(),
            None => Vec::new(),
        }
    }

    /// Single record by name — when several records share the name, the LAST
    /// one wins (Go `GetRecord`).
    pub fn get_record(&self, name: &str) -> Option<&AnimationDataRecord> {
        self.get_records(name).into_iter().last()
    }

    /// Token lookup through the file's own hash placement: block =
    /// [`hash_name`]`(token)`, then the matching records inside that block.
    /// Yields the same records as [`AnimationData::get_records`] (tested on
    /// the real file) — this is the path the game itself uses.
    pub fn get_by_token(&self, token: &str) -> Vec<&AnimationDataRecord> {
        self.blocks[hash_name(token) as usize]
            .records
            .iter()
            .filter(|r| r.name.eq_ignore_ascii_case(token))
            .collect()
    }

    /// Number of distinct names (Go `GetRecordsCount` — counts the entries
    /// map, i.e. unique names, NOT total records; see
    /// [`AnimationData::total_records`]).
    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    /// Total records across all blocks (homonyms included).
    pub fn total_records(&self) -> usize {
        self.total_records
    }

    /// All record names, sorted (Go `GetRecordNames` — map order there is
    /// randomized by Go; sorted is deterministic).
    pub fn record_names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(|s| s.as_str()).collect();
        names.sort_unstable();
        names
    }

    /// The 256 blocks, in file order.
    pub fn blocks(&self) -> &[Block] {
        &self.blocks
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic 256-block buffer with a single record in block 0.
    fn one_record(name: &str, fpd: u32, speed: u16, events: &[u8]) -> Vec<u8> {
        assert!(name.len() <= 7);
        let mut d = Vec::new();
        d.extend_from_slice(&1u32.to_le_bytes()); // block 0: 1 record
        let mut n = name.as_bytes().to_vec();
        n.resize(BYTE_COUNT_NAME, 0);
        d.extend_from_slice(&n);
        d.extend_from_slice(&fpd.to_le_bytes());
        d.extend_from_slice(&speed.to_le_bytes());
        d.extend_from_slice(&[0, 0]);
        let mut ev = [0u8; NUM_EVENTS];
        for (i, e) in events.iter().enumerate() {
            ev[i] = *e;
        }
        d.extend_from_slice(&ev);
        // 255 empty blocks
        d.extend_from_slice(&[0; 4 * (NUM_BLOCKS - 1)]);
        d
    }

    #[test]
    fn hash_name_matches_reference_values() {
        // Values independently computed (sum of uppercase bytes mod 256) and
        // verified against the real file's block placement (Python dump).
        assert_eq!(hash_name("BANUHTH"), 10);
        assert_eq!(hash_name("FAWLHTH"), 14);
        assert_eq!(hash_name("ZMWLHTH"), 46);
        assert_eq!(hash_name("SIWLHTH"), 35);
        assert_eq!(hash_name("BAWL1HS"), 242);
        // Case-insensitive (Go uppercases before summing).
        assert_eq!(hash_name("bawl1hs"), hash_name("BAWL1HS"));
    }

    #[test]
    fn parses_synthetic_record_with_events() {
        let d = one_record("BAXXHTH", 13, 168, &[0, 1, 0, 3]);
        let ad = AnimationData::load(&d).unwrap();
        assert_eq!(ad.total_records(), 1);
        assert_eq!(ad.entry_count(), 1);
        assert_eq!(ad.blocks().len(), NUM_BLOCKS);
        let r = ad.get_record("BAXXHTH").unwrap();
        assert_eq!(r.frames_per_direction, 13);
        assert_eq!(r.speed, 168);
        assert_eq!((r.fps() * 1_000.0).round() / 1_000.0, 16.406);
        assert_eq!((r.frame_duration_ms() * 1_000.0).round() / 1_000.0, 60.952);
        let trig: Vec<_> = r.triggered_events().collect();
        assert_eq!(trig, vec![(1, AnimationEvent::Attack), (3, AnimationEvent::Sound)]);
        assert_eq!(r.event(1), AnimationEvent::Attack);
        assert_eq!(r.event(0), AnimationEvent::None);
        assert_eq!(r.event(NUM_EVENTS), AnimationEvent::None); // out of range
    }

    #[test]
    fn get_record_last_homonym_wins() {
        // Go TestAnimationData_GetRecord: with three homonyms, GetRecord
        // returns the LAST one.
        let mut d = Vec::new();
        d.extend_from_slice(&3u32.to_le_bytes());
        for (fpd, speed) in [(1u32, 1u16), (2, 2), (3, 3)] {
            let mut n = b"AAAAAAA".to_vec();
            n.resize(BYTE_COUNT_NAME, 0);
            d.extend_from_slice(&n);
            d.extend_from_slice(&fpd.to_le_bytes());
            d.extend_from_slice(&speed.to_le_bytes());
            d.extend_from_slice(&[0; 2 + NUM_EVENTS]);
        }
        d.extend_from_slice(&[0; 4 * (NUM_BLOCKS - 1)]);
        let ad = AnimationData::load(&d).unwrap();
        assert_eq!(ad.get_records("AAAAAAA").len(), 3);
        assert_eq!(ad.get_record("AAAAAAA").unwrap().speed, 3);
        assert!(ad.get_record("MISSING").is_none());
        assert!(ad.get_records("MISSING").is_empty());
    }

    #[test]
    fn rejects_more_than_max_records_per_block() {
        let load_err = |d: &[u8]| match AnimationData::load(d) {
            Err(e) => e,
            Ok(_) => panic!("expected a load error"),
        };
        let mut d = 68u32.to_le_bytes().to_vec();
        d.extend_from_slice(&[0; 68 * RECORD_SIZE]);
        d.extend_from_slice(&[0; 4 * (NUM_BLOCKS - 1)]);
        assert_eq!(load_err(&d), AnimDataError::TooManyRecords { block: 0, count: 68 });
    }

    #[test]
    fn rejects_name_without_null_terminator() {
        let load_err = |d: &[u8]| match AnimationData::load(d) {
            Err(e) => e,
            Ok(_) => panic!("expected a load error"),
        };
        let mut d = 1u32.to_le_bytes().to_vec();
        d.extend_from_slice(b"ABCDEFGH"); // 8 bytes, no NUL
        d.extend_from_slice(&[0; RECORD_SIZE - BYTE_COUNT_NAME]);
        d.extend_from_slice(&[0; 4 * (NUM_BLOCKS - 1)]);
        assert_eq!(
            load_err(&d),
            AnimDataError::MissingNullTerminator { block: 0, record: 0 }
        );
    }

    #[test]
    fn rejects_trailing_bytes_and_truncation() {
        let load_err = |d: &[u8]| match AnimationData::load(d) {
            Err(e) => e,
            Ok(_) => panic!("expected a load error"),
        };
        // Trailing junk after the 256 blocks → LengthMismatch (Go l.205).
        let mut d = vec![0; 4 * NUM_BLOCKS];
        d.push(0xAB);
        assert_eq!(
            load_err(&d),
            AnimDataError::LengthMismatch { pos: 4 * NUM_BLOCKS, len: 4 * NUM_BLOCKS + 1 }
        );

        // Cut mid-record → TooShort.
        let d = one_record("BAXXHTH", 1, 1, &[]); // 4 + 160 + 1020 bytes
        assert_eq!(
            load_err(&d[..4 + 160 - 1]),
            AnimDataError::TooShort { need: 4 + 160, have: 4 + 160 - 1 }
        );
        assert_eq!(load_err(&d[..3]), AnimDataError::TooShort { need: 4, have: 3 });
    }
}
