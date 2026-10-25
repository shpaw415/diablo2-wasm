//! Lecteur/écrivain de bits LSB-first (« IFE » : bit 0 = bit de poids faible
//! du premier octet), convention des sections variables du .d2s.
//!
//! `BitReader` est le miroir exact du `BitReader` Python de `d2s_parse.py`
//! (d2w-re/tools) ; `BitWriter` est le miroir du `BitWriter` de
//! `make_vectors.py` — la boucle lecture → écriture est donc octet-à-octet.

#![allow(dead_code)]

use crate::error::D2sError;

/// Lecteur de bits LSB-first. Les champs multi-bits sont assemblés poids
/// faible d'abord : le i-ème bit lu pèse 2^i.
#[derive(Debug, Clone)]
pub struct BitReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> BitReader<'a> {
    /// Nouveau lecteur sur `data`, départ à `bit_offset`.
    pub fn new(data: &'a [u8], bit_offset: usize) -> Self {
        BitReader { data, pos: bit_offset }
    }

    /// Lit `nbits` bits (LSB d'abord). `Err(PrematureEnd)` au-delà des données.
    pub fn read(&mut self, nbits: usize) -> Result<u32, D2sError> {
        if self.pos + nbits > self.data.len() * 8 {
            return Err(D2sError::PrematureEnd { bit: self.pos, wanted_bits: nbits });
        }
        let mut value: u32 = 0;
        for i in 0..nbits {
            let bit_pos = self.pos + i;
            let byte = self.data[bit_pos / 8];
            value |= (((byte >> (bit_pos % 8)) & 1) as u32) << i;
        }
        self.pos += nbits;
        Ok(value)
    }

    /// Lit `nbits` bits signés (complément à deux) — largeurs de mods à
    /// `Save Add` négatif (evidence.md §8.6).
    pub fn read_signed(&mut self, nbits: usize) -> Result<i32, D2sError> {
        let v = self.read(nbits)?;
        if nbits > 0 && v >= 1 << (nbits - 1) {
            Ok(v as i32 - (1i32 << nbits))
        } else {
            Ok(v as i32)
        }
    }

    /// Lit `nchars` octets via le flux de bits et les interprète en ASCII.
    /// Tout octet > 127 est remplacé par U+FFFD (comportement du parseur de
    /// référence, jamais une erreur).
    pub fn read_ascii(&mut self, nchars: usize) -> Result<String, D2sError> {
        let mut s = String::with_capacity(nchars);
        for _ in 0..nchars {
            let b = self.read(8)?;
            s.push(if b < 0x80 { b as u8 as char } else { '\u{FFFD}' });
        }
        Ok(s)
    }

    /// Lit des caractères 7 bits jusqu'au 0 final (noms d'oreilles,
    /// personnalisation Anya). Retourne (texte, octets bruts).
    pub fn read_name_chars(&mut self) -> Result<(String, Vec<u8>), D2sError> {
        let mut chars = Vec::new();
        let mut raw = Vec::new();
        loop {
            let ch = self.read(7)?;
            if ch == 0 {
                return Ok((chars.into_iter().map(|c| c as u8 as char).collect(), raw));
            }
            if ch > 0x7F {
                return Err(D2sError::BadChar { byte: ch as u8, bit: self.pos - 7 });
            }
            raw.push(ch as u8);
            chars.push(ch as u8 as char);
        }
    }

    /// Position courante en bits.
    pub fn bit_pos(&self) -> usize {
        self.pos
    }

    /// Avance jusqu'au frontière d'octet suivante (alignement de fin d'item).
    pub fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// Position courante en octets (`bit_pos() / 8`).
    pub fn byte_pos(&self) -> usize {
        self.pos / 8
    }

    /// Aperçu des `n` octets à la position courante, sans avancer. `None`
    /// si la position n'est pas alignée sur un octet ou s'il reste moins
    /// de `n` octets. (Sonde du préfixe « gf » des stats, étape 19a.)
    pub fn peek_bytes(&self, n: usize) -> Option<&'a [u8]> {
        if self.pos % 8 != 0 {
            return None;
        }
        let start = self.pos / 8;
        self.data.get(start..start + n)
    }
}

/// Écrivain de bits LSB-first — symétrique exact du `BitWriter` de
/// `make_vectors.py` : les octets poussent au fil de l'eau, [`align`]
/// complète l'octet courant avec des 0.
///
/// [`align`]: BitWriter::align
#[derive(Debug, Default, Clone)]
pub struct BitWriter {
    data: Vec<u8>,
    pos: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        BitWriter { data: Vec::new(), pos: 0 }
    }

    /// Écrit les `nbits` bits de poids faible de `value` (LSB d'abord).
    pub fn write(&mut self, value: u32, nbits: usize) {
        for i in 0..nbits {
            let bit = ((value >> i) & 1) as u8;
            if self.pos / 8 == self.data.len() {
                self.data.push(0);
            }
            if bit != 0 {
                self.data[self.pos / 8] |= bit << (self.pos % 8);
            }
            self.pos += 1;
        }
    }

    /// Écrit une valeur signée (complément à deux sur `nbits`).
    pub fn write_signed(&mut self, value: i32, nbits: usize) {
        let raw = (value as u32) & mask(nbits);
        self.write(raw, nbits);
    }

    /// Écrit une chaîne ASCII octet par octet via le flux de bits.
    pub fn write_ascii(&mut self, text: &str) {
        for b in text.bytes() {
            self.write(b as u32, 8);
        }
    }

    /// Écrit des caractères 7 bits + le 0 final (noms).
    pub fn write_name_chars(&mut self, name: &str) {
        for c in name.chars() {
            self.write(c as u32 & 0x7F, 7);
        }
        self.write(0, 7);
    }

    /// Aligne sur l'octet suivant (padding 0), comme le `align()` Python.
    pub fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
        while self.data.len() < self.pos / 8 {
            self.data.push(0);
        }
    }

    /// Position courante en bits.
    pub fn bit_pos(&self) -> usize {
        self.pos
    }

    /// Consomme le lecteur et rend les octets écrits.
    pub fn into_bytes(self) -> Vec<u8> {
        self.data
    }
}

/// Masque de `nbits` bits de poids faible (0 bits → 0, pas de débordement).
pub(crate) fn mask(nbits: usize) -> u32 {
    if nbits == 0 || nbits >= 32 {
        u32::MAX
    } else {
        (1u32 << nbits) - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_round_trip_symmetry() {
        let cases: &[(u32, usize)] = &[
            (0, 1),
            (1, 1),
            (0x1FF, 9),
            (0xDEADBEEF & 0x3FF, 10),
            (0x55AA55AA & 0xFFFFF, 20),
            (u32::MAX, 32),
            (0x2A, 7),
        ];
        let mut w = BitWriter::new();
        for &(v, n) in cases {
            w.write(v, n);
        }
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        for &(v, n) in cases {
            assert_eq!(r.read(n).unwrap(), v & mask(n), "valeur {} sur {} bits", v, n);
        }
    }

    #[test]
    fn signed_round_trip() {
        let mut w = BitWriter::new();
        w.write_signed(-1, 9);
        w.write_signed(255, 9);
        w.write_signed(-256, 9);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        assert_eq!(r.read_signed(9).unwrap(), -1);
        assert_eq!(r.read_signed(9).unwrap(), 255);
        assert_eq!(r.read_signed(9).unwrap(), -256);
    }

    #[test]
    fn premature_end_is_an_error() {
        let data = [0u8; 2];
        let mut r = BitReader::new(&data, 0);
        assert_eq!(r.read(16).unwrap(), 0);
        assert!(matches!(
            r.read(1),
            Err(D2sError::PrematureEnd { bit: 16, wanted_bits: 1 })
        ));
    }

    #[test]
    fn align_pads_with_zeros() {
        let mut w = BitWriter::new();
        w.write(1, 3);
        w.align();
        assert_eq!(w.bit_pos(), 8);
        assert_eq!(w.into_bytes(), vec![0b0000_0001]);
    }

    #[test]
    fn name_chars_round_trip() {
        let mut w = BitWriter::new();
        w.write_name_chars("Jst");
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        let (name, raw) = r.read_name_chars().unwrap();
        assert_eq!(name, "Jst");
        assert_eq!(raw, b"Jst");
    }
}
