//! BitMuncher — LSB-first bit reader over a byte slice.
//!
//! Byte-faithful port of OpenDiablo2 `d2datautils.BitMuncher` (bitmuncher.go):
//! bits are consumed LSB-first within each byte (`offset % 8` selects the bit),
//! multi-bit reads assemble LSB-first (`result |= bit << i`).

#[derive(Clone)]
pub struct BitMuncher<'a> {
    data: &'a [u8],
    offset: usize,
    bits_read: usize,
}

const BYTE_LEN: usize = 8;

impl<'a> BitMuncher<'a> {
    /// Create a muncher starting at bit `offset` of `data`.
    pub fn new(data: &'a [u8], offset: usize) -> Self {
        BitMuncher {
            data,
            offset,
            bits_read: 0,
        }
    }

    /// Copy keeping the current bit offset but resetting `bits_read`
    /// (Go `CopyBitMuncher`). Used to fork sub-bitstreams that share the
    /// same start bit but advance independently.
    pub fn fork(&self) -> BitMuncher<'a> {
        BitMuncher {
            data: self.data,
            offset: self.offset,
            bits_read: 0,
        }
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn set_offset(&mut self, n: usize) {
        self.offset = n;
    }

    pub fn bits_read(&self) -> usize {
        self.bits_read
    }

    pub fn set_bits_read(&mut self, n: usize) {
        self.bits_read = n;
    }

    pub fn get_bit(&mut self) -> u32 {
        let byte = self.data.get(self.offset / BYTE_LEN).copied().unwrap_or(0);
        let result = (byte >> (self.offset % BYTE_LEN)) as u32 & 0x01;
        self.offset += 1;
        self.bits_read += 1;
        result
    }

    pub fn skip_bits(&mut self, bits: usize) {
        self.offset += bits;
        self.bits_read += bits;
    }

    pub fn get_byte(&mut self) -> u8 {
        self.get_bits(BYTE_LEN) as u8
    }

    pub fn get_int32(&mut self) -> i32 {
        let value = self.get_bits(32);
        self.make_signed(value, 32)
    }

    pub fn get_uint32(&mut self) -> u32 {
        self.get_bits(32)
    }

    pub fn get_bits(&mut self, bits: usize) -> u32 {
        if bits == 0 {
            return 0;
        }
        let mut result = 0u32;
        for i in 0..bits {
            result |= self.get_bit() << i;
        }
        result
    }

    pub fn get_signed_bits(&mut self, bits: usize) -> i32 {
        let value = self.get_bits(bits);
        self.make_signed(value, bits)
    }

    /// Two's-complement of a `bits`-wide field. A single bit set means -1.
    pub fn make_signed(&self, value: u32, bits: usize) -> i32 {
        if bits == 0 {
            return 0;
        }
        if bits == 1 {
            return -(value as i32);
        }
        if bits >= 32 {
            return value as i32;
        }
        if value & (1 << (bits - 1)) == 0 {
            return value as i32;
        }
        // Sign-extend: the Go original builds 0xFFFFFFFF and clears the
        // positions where the value has a 0 bit — equivalent to extension.
        (value | !((1u32 << bits) - 1)) as i32
    }
}
