//! PKWARE DCL ("implode"/"explode") decompressor.
//!
//! Faithful Rust port of Mark Adler's `blast.c` v1.3 (kept in
//! `evidence/blast.c`), transcribed through the validated Python reference
//! `tools/blast_pkware.py`. The fixed Huffman tables (`LITLEN`, `LENLEN`,
//! `DISTLEN`, `BASE`, `EXTRA`) are reproduced verbatim.
//!
//! The stream layout: two header bytes (literal-coding flag, then
//! log2(dictionary)-6), then an interleaved stream of literals and
//! length/distance pairs terminated by the length code 519. Huffman codes are
//! bit-reversed on the wire, extra bits are not.

use crate::error::D2Error;

const MAXBITS: usize = 13;
/// Sliding-window size (distances never exceed 4 KiB).
const MAXWIN: usize = 4096;

/// Compact bit lengths of the 256 literal codes (98 run-length bytes that
/// expand to exactly 256 entries).
const LITLEN: [u8; 98] = [
    11, 124, 8, 7, 28, 7, 188, 13, 76, 4, 10, 8, 12, 10, 12, 10, 8, 23, 8, //
    9, 7, 6, 7, 8, 7, 6, 55, 8, 23, 24, 12, 11, 7, 9, 11, 12, 6, 7, 22, 5, //
    7, 24, 6, 11, 9, 6, 7, 22, 7, 11, 38, 7, 9, 8, 25, 11, 8, 11, 9, 12, //
    8, 12, 5, 38, 5, 38, 5, 11, 7, 5, 6, 21, 6, 10, 53, 8, 7, 24, 10, 27, //
    44, 253, 253, 253, 252, 252, 252, 13, 12, 45, 12, 45, 12, 61, 12, 45, //
    44, 173,
];
/// Bit lengths of the length codes 0..15.
const LENLEN: [u8; 6] = [2, 35, 36, 53, 38, 23];
/// Bit lengths of the distance codes 0..63.
const DISTLEN: [u8; 7] = [2, 20, 53, 230, 247, 151, 248];
/// Base values for the length codes.
const BASE: [u16; 16] = [3, 2, 4, 5, 6, 7, 8, 9, 10, 12, 16, 24, 40, 72, 136, 264];
/// Extra bits for the length codes.
const EXTRA: [u8; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8];

/// Input bit reader plus output buffer (mirrors `struct state` in blast.c).
/// Bits are consumed LSB-first within each byte.
struct State<'a> {
    data: &'a [u8],
    pos: usize,
    bitbuf: u32,
    bitcnt: i32,
    out: Vec<u8>,
}

impl<'a> State<'a> {
    /// Consume `need` bits from the stream (LSB-first), 0 <= need <= 8.
    fn bits(&mut self, need: u32) -> Result<u32, D2Error> {
        let mut val = self.bitbuf;
        let mut bitcnt = self.bitcnt;
        while bitcnt < need as i32 {
            if self.pos >= self.data.len() {
                return Err(D2Error::Corrupt("dcl: out of input"));
            }
            val |= (self.data[self.pos] as u32) << bitcnt;
            self.pos += 1;
            bitcnt += 8;
        }
        self.bitbuf = val >> need;
        self.bitcnt = bitcnt - need as i32;
        Ok(val & ((1u32 << need) - 1))
    }

    /// Decode one Huffman symbol (bit-reversed canonical code, as in blast.c).
    fn decode(&mut self, count: &[u16], symbol: &[u16]) -> Result<u16, D2Error> {
        let mut bitbuf = self.bitbuf;
        let mut left: i32 = self.bitcnt;
        let mut code: u32 = 0;
        let mut first: u32 = 0;
        let mut index: usize = 0;
        let mut blen: i32 = 1;
        let mut ci: usize = 1;
        loop {
            while left > 0 {
                left -= 1;
                code |= (bitbuf & 1) ^ 1; // stream bits are inverted
                bitbuf >>= 1;
                let cnt = count[ci] as u32;
                ci += 1;
                if code < first + cnt {
                    self.bitbuf = bitbuf;
                    self.bitcnt = (self.bitcnt - blen) & 7;
                    let sym = index + (code - first) as usize;
                    return symbol
                        .get(sym)
                        .copied()
                        .ok_or(D2Error::Corrupt("dcl: bad code"));
                }
                index += cnt as usize;
                first = (first + cnt) << 1;
                code <<= 1;
                blen += 1;
            }
            left = (MAXBITS as i32 + 1) - blen;
            if left == 0 {
                break;
            }
            if self.pos >= self.data.len() {
                return Err(D2Error::Corrupt("dcl: out of input"));
            }
            bitbuf = self.data[self.pos] as u32;
            self.pos += 1;
            if left > 8 {
                left = 8;
            }
        }
        Err(D2Error::Corrupt("dcl: bad code"))
    }
}

/// Expand compact run-length code lengths (`high nibble + 1` repeats of the
/// low nibble) and build the canonical decoding tables. Return values are
/// ignored, exactly as blast.c does for its three fixed tables.
fn construct(rep: &[u8], count: &mut [u16; MAXBITS + 1], symbol: &mut [u16]) {
    let mut length: Vec<u8> = Vec::with_capacity(256);
    for &r in rep {
        let runs = (r >> 4) as usize + 1;
        let len = r & 0x0F;
        length.resize(length.len() + runs, len);
    }
    let n = length.len();
    count.fill(0);
    for &len in &length {
        count[len as usize] += 1;
    }
    if count[0] as usize == n {
        return; // no codes at all; decode() will fail on any input
    }
    let mut left: i32 = 1;
    // Loop style mirrors blast.c's construct(); count is indexed by code length.
    #[allow(clippy::needless_range_loop)]
    for len in 1..=MAXBITS {
        left = (left << 1) - count[len] as i32;
        debug_assert!(left >= 0, "over-subscribed fixed table");
    }
    let mut offs = [0u16; MAXBITS + 2];
    for len in 1..MAXBITS {
        offs[len + 1] = offs[len] + count[len];
    }
    for (sym, &len) in length.iter().enumerate() {
        if len != 0 {
            symbol[offs[len as usize] as usize] = sym as u16;
            offs[len as usize] += 1;
        }
    }
}

/// Decompress a full PKWARE DCL stream, including its 2-byte header.
///
/// This is the whole stream form used by MPQ `IMPLODE` blocks; the Python
/// reference keeps its `first` distance guard permanently set, so any
/// distance reaching before the start of the output is rejected.
pub fn blast(data: &[u8]) -> Result<Vec<u8>, D2Error> {
    let mut s = State {
        data,
        pos: 0,
        bitbuf: 0,
        bitcnt: 0,
        out: Vec::with_capacity(MAXWIN * 4),
    };

    // Fixed decoding tables (identical for every DCL stream).
    let mut litcnt = [0u16; MAXBITS + 1];
    let mut litsym = [0u16; 256];
    let mut lencnt = [0u16; MAXBITS + 1];
    let mut lensym = [0u16; 16];
    let mut distcnt = [0u16; MAXBITS + 1];
    let mut distsym = [0u16; 64];
    construct(&LITLEN, &mut litcnt, &mut litsym);
    construct(&LENLEN, &mut lencnt, &mut lensym);
    construct(&DISTLEN, &mut distcnt, &mut distsym);

    // Header: literal-coding flag, then log2(dictionary) - 6.
    let lit = s.bits(8)?;
    if lit > 1 {
        return Err(D2Error::Corrupt("dcl: bad literal flag"));
    }
    let dict = s.bits(8)?;
    if !(4..=6).contains(&dict) {
        return Err(D2Error::Corrupt("dcl: bad dictionary size"));
    }

    loop {
        if s.bits(1)? == 1 {
            // Length/distance pair.
            let symbol = s.decode(&lencnt, &lensym)?;
            let len = BASE[symbol as usize] as u32 + s.bits(EXTRA[symbol as usize] as u32)?;
            if len == 519 {
                break; // end-of-stream code
            }
            let sym2 = if len == 2 { 2 } else { dict };
            let mut dist = (s.decode(&distcnt, &distsym)? as u32) << sym2;
            dist += s.bits(sym2)? + 1;
            if dist as usize > s.out.len() {
                return Err(D2Error::Corrupt("dcl: distance too far back"));
            }
            // Overlapping copy (length may exceed distance) — byte by byte,
            // exactly like the reference.
            for _ in 0..len {
                let byte = s.out[s.out.len() - dist as usize];
                s.out.push(byte);
            }
        } else {
            // Literal.
            let symbol = if lit == 1 {
                s.decode(&litcnt, &litsym)? as u32
            } else {
                s.bits(8)?
            };
            s.out.push(symbol as u8);
        }
    }
    Ok(s.out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Worked example from the blast.c header comment:
    /// `00 04 82 24 25 8f 80 7f` -> "AIAIAIAIAIAIA".
    #[test]
    fn blast_example_from_blast_c() {
        let stream = [0x00, 0x04, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f];
        assert_eq!(blast(&stream).unwrap(), b"AIAIAIAIAIAIA");
    }

    #[test]
    fn rejects_bad_header_and_truncation() {
        assert_eq!(
            blast(&[0x02, 0x04]),
            Err(D2Error::Corrupt("dcl: bad literal flag"))
        );
        assert_eq!(
            blast(&[0x00, 0x03]),
            Err(D2Error::Corrupt("dcl: bad dictionary size"))
        );
        assert_eq!(blast(&[0x00]), Err(D2Error::Corrupt("dcl: out of input")));
        assert_eq!(blast(&[]), Err(D2Error::Corrupt("dcl: out of input")));
    }

    /// A truncated copy loop must fail cleanly instead of panicking.
    #[test]
    fn truncated_stream_is_an_error_not_a_panic() {
        let stream = [0x00, 0x04, 0x82, 0x24]; // cut mid-stream
        assert!(blast(&stream).is_err());
    }
}
