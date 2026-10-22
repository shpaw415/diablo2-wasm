//! MPQ cryptography — StormLib-compatible primitives, ported 1:1 from the
//! validated Python reference `tools/mpq_mini.py` (which in turn follows the
//! classic Storm.dll / StormLib algorithms).
//!
//! * crypt table: 0x500 u32 entries, seed 0x00100001 (built at compile time)
//! * [`hash_string`]: the case-insensitive MPQ path hash (types 0..4)
//! * [`decrypt_bytes`] / [`encrypt_bytes`]: the dword-stream block cipher used
//!   for the hash/block tables and for encrypted file data
//!
//! All arithmetic is the usual 32-bit wrapping arithmetic of the original
//! Storm implementation. The Python reference was validated against real
//! Diablo 2 archives; this module reproduces it bit for bit (see the
//! `#[cfg(test)]` vectors, generated from the Python code).

/// Number of entries in the MPQ crypt table.
const CRYPT_TABLE_LEN: usize = 0x500;

/// The MPQ crypt table, precomputed at compile time.
static CRYPT_TABLE: [u32; CRYPT_TABLE_LEN] = build_crypt_table();

/// StormLib crypt-table generation: for each of the 256 base indices, five
/// derived entries are written at stride 0x100.
const fn build_crypt_table() -> [u32; CRYPT_TABLE_LEN] {
    let mut table = [0u32; CRYPT_TABLE_LEN];
    let mut seed: u32 = 0x0010_0001;
    let mut i = 0usize;
    while i < 0x100 {
        let mut idx = i;
        let mut round = 0;
        while round < 5 {
            seed = ((seed as u64 * 125 + 3) % 0x2AAAAB) as u32;
            let t1 = (seed & 0xFFFF) << 0x10;
            seed = ((seed as u64 * 125 + 3) % 0x2AAAAB) as u32;
            let t2 = seed & 0xFFFF;
            table[idx] = t1 | t2;
            idx += 0x100;
            round += 1;
        }
        i += 1;
    }
    table
}

/// Uppercase a path the way the Python reference does
/// (`s.upper().encode('ascii', 'replace')`): Unicode-aware uppercasing, every
/// resulting non-ASCII byte replaced by `?`.
fn upper_ascii(s: &str) -> Vec<u8> {
    s.chars()
        .flat_map(char::to_uppercase)
        .map(|c| if c.is_ascii() { c as u8 } else { b'?' })
        .collect()
}

/// MPQ string hash (Storm hash_string).
///
/// `typ`: 0 = name1, 1 = name2, 2 = probe start, 3 = file encryption key,
/// 4 = key fix ("(def)" style). Case-insensitive; `/` and `\` are equivalent.
pub fn hash_string(s: &str, typ: u32) -> u32 {
    let mut seed1: u32 = 0x7FED_7FED;
    let mut seed2: u32 = 0xEEEE_EEEE;
    for byte in upper_ascii(s) {
        let byte = if byte == b'/' { b'\\' } else { byte };
        let b32 = byte as u32;
        seed1 = CRYPT_TABLE[((typ << 8) + b32) as usize] ^ seed1.wrapping_add(seed2);
        seed2 = b32
            .wrapping_add(seed1)
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
    }
    seed1
}

/// One round of the key schedule (`key = ((~key << 21) + 0x11111111) | key >> 11`).
const fn key_update(key: u32) -> u32 {
    ((!key).wrapping_shl(0x15).wrapping_add(0x1111_1111)) | (key >> 0x0B)
}

/// Decrypt a little-endian u32 stream in place (returns a new buffer).
///
/// Mirrors `decrypt_bytes` in `mpq_mini.py`: only whole dwords are processed
/// and a trailing partial dword is dropped (MPQ encrypted streams are always
/// dword multiples in practice).
pub fn decrypt_bytes(data: &[u8], mut key: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((data.len() / 4) * 4);
    let mut seed2: u32 = 0xEEEE_EEEE;
    for w in data.as_chunks::<4>().0 {
        let cipher = u32::from_le_bytes(*w);
        seed2 = seed2.wrapping_add(CRYPT_TABLE[0x400 + (key & 0xFF) as usize]);
        let plain = cipher ^ key.wrapping_add(seed2);
        key = key_update(key);
        seed2 = plain
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
        out.extend_from_slice(&plain.to_le_bytes());
    }
    out
}

/// Encrypt a little-endian u32 stream (StormLib `EncryptMpqBlock`, the exact
/// inverse of [`decrypt_bytes`]).
///
/// Like StormLib's writer side, the *plaintext* dword is folded into the
/// seed2 chain (`dwValue32 = DataPointer[i]` is assigned before the XOR in
/// SBaseCommon.cpp), which is precisely what makes this the inverse of
/// [`decrypt_bytes`]. Trailing partial dwords are dropped, as in
/// [`decrypt_bytes`].
pub fn encrypt_bytes(data: &[u8], mut key: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity((data.len() / 4) * 4);
    let mut seed2: u32 = 0xEEEE_EEEE;
    for w in data.as_chunks::<4>().0 {
        let plain = u32::from_le_bytes(*w);
        seed2 = seed2.wrapping_add(CRYPT_TABLE[0x400 + (key & 0xFF) as usize]);
        let cipher = plain ^ key.wrapping_add(seed2);
        key = key_update(key);
        seed2 = plain
            .wrapping_add(seed2)
            .wrapping_add(seed2 << 5)
            .wrapping_add(3);
        out.extend_from_slice(&cipher.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // Reference vectors generated with the validated Python implementation
    // (tools/mpq_mini.py).

    #[test]
    fn hash_string_matches_python() {
        // hash_string(name, [0, 1, 2, 3])
        assert_eq!(
            hash_string("(hash table)", 3),
            0xc3af_3770,
            "hash table key"
        );
        assert_eq!(
            hash_string("(block table)", 3),
            0xec83_b3a3,
            "block table key"
        );
        assert_eq!(
            [
                hash_string("(hash table)", 0),
                hash_string("(hash table)", 1),
                hash_string("(hash table)", 2),
            ],
            [0xf5c5_efe2, 0x58be_dbfc, 0x5389_90ce]
        );
        assert_eq!(
            [
                hash_string("(block table)", 0),
                hash_string("(block table)", 1),
                hash_string("(block table)", 2),
            ],
            [0x9d71_9d2b, 0x68e8_6afe, 0xb67f_061b]
        );
        // '/' and '\' must hash identically; case-insensitive.
        let back = "data\\global\\excel\\armor.txt";
        let fwd = "data/global/excel/armor.txt";
        for t in 0..4u32 {
            assert_eq!(hash_string(back, t), hash_string(fwd, t));
        }
        assert_eq!(
            [
                hash_string(back, 0),
                hash_string(back, 1),
                hash_string(back, 2),
                hash_string(back, 3),
            ],
            [0x6e7f_10ff, 0x1b41_db66, 0x4d24_fcc0, 0x0601_46c3]
        );
    }

    #[test]
    fn decrypt_bytes_matches_python() {
        assert_eq!(
            decrypt_bytes(&[0u8; 16], 0x1234_5678),
            [
                0x12, 0xc0, 0x14, 0x2b, 0x85, 0x4e, 0x05, 0xaa, 0xc6, 0xf6, 0xfa, 0x73, 0xca, 0x4c,
                0x9c, 0xf5
            ]
        );
        assert_eq!(
            decrypt_bytes(
                &[
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c,
                    0x0d, 0x0e, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19,
                    0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f
                ],
                hash_string("(hash table)", 3)
            ),
            [
                0xcc, 0xce, 0x3e, 0x85, 0xa0, 0xf0, 0x0d, 0xea, 0x59, 0x2c, 0xbd, 0x7c, 0x79, 0x53,
                0x66, 0x80, 0x9a, 0x6c, 0xe7, 0x98, 0xac, 0x2a, 0x70, 0xca, 0x7e, 0xe2, 0xd3, 0xc5,
                0x60, 0x12, 0xa9, 0xb8
            ]
        );
        assert_eq!(
            decrypt_bytes(&[1, 2, 3, 4, 5, 6, 7, 8], 0xFFFF_FFFF),
            [0x58, 0x15, 0xf1, 0x65, 0x65, 0x46, 0x4e, 0x82]
        );
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        for key in [0u32, 1, 0x1234_5678, 0xFFFF_FFFF, hash_string("(test)", 3)] {
            let plain: Vec<u8> = (0..64u32)
                .map(|i| (i.wrapping_mul(37) ^ 0x5a) as u8)
                .collect();
            let cipher = encrypt_bytes(&plain, key);
            assert_eq!(cipher.len(), plain.len());
            assert_eq!(decrypt_bytes(&cipher, key), plain);
        }
    }

    #[test]
    fn trailing_partial_dword_is_dropped() {
        // Python parity: only whole dwords are processed.
        let out = decrypt_bytes(&[1, 2, 3, 4, 0xFF], 0x42);
        assert_eq!(out.len(), 4);
        assert_eq!(out, decrypt_bytes(&[1, 2, 3, 4], 0x42));
    }
}
