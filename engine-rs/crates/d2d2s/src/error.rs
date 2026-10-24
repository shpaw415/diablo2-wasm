//! Erreurs du parseur .d2s — toutes structurelles, jamais de panic.
//!
//! Convention reprise du parseur de référence `d2s_parse.py` (d2w-re) :
//! une erreur = le fichier ne peut pas être parsé plus loin, avec l'endroit
//! exact (offset d'octet ou de bit) quand on le connaît.

#![allow(dead_code)]

use std::fmt;

/// Erreur de lecture/écriture d'un fichier .d2s.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D2sError {
    /// Fichier plus court qu'un en-tête (765 octets).
    TooShort { len: usize },
    /// Signature 0xAA55AA55 absente (strict uniquement).
    BadSignature { got: u32 },
    /// Checksum stocké != recalculé (strict uniquement).
    ChecksumMismatch { stored: u32, computed: u32 },
    /// Fin de données prématurée dans une section bit-packée.
    PrematureEnd { bit: usize, wanted_bits: usize },
    /// Marqueur ASCII attendu absent ("JM", "Woo!", "WS", "w4", "if", "gf", "kf", "jf").
    BadMarker {
        expected: String,
        got: String,
        bit: usize,
    },
    /// Section d'octets tronquée (skills, quêtes…).
    SectionTruncated {
        name: &'static str,
        offset: usize,
        needed: usize,
        len: usize,
    },
    /// ID de stat caractère inconnu dans le flux 1.10+ (> 15 ou absent des tables).
    UnknownStatId { id: u16, bit: usize },
    /// ID de mod d'item sans largeur connue (ItemStatCost requis).
    UnknownModWidth { id: u16, bit: usize },
    /// Nom d'item/personnalisé avec un octet hors ASCII.
    BadChar { byte: u8, bit: usize },
}

impl fmt::Display for D2sError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            D2sError::TooShort { len } => {
                write!(f, "fichier plus court qu'un en-tête (765 octets) : {}", len)
            }
            D2sError::BadSignature { got } => write!(
                f,
                "signature 0x{:08X} invalide (attendu 0xAA55AA55, octets 55 AA 55 AA)",
                got
            ),
            D2sError::ChecksumMismatch { stored, computed } => write!(
                f,
                "checksum invalide (stocké 0x{:08X}, calculé 0x{:08X})",
                stored, computed
            ),
            D2sError::PrematureEnd { bit, wanted_bits } => write!(
                f,
                "fin de fichier prématurée (bit {}, demande {} bits)",
                bit, wanted_bits
            ),
            D2sError::BadMarker {
                expected,
                got,
                bit,
            } => write!(f, "marqueur {:?} absent (lu {:?}, bit {})", expected, got, bit),
            D2sError::SectionTruncated {
                name,
                offset,
                needed,
                len,
            } => write!(
                f,
                "section {} tronquée (offset {}, {} octets requis, fichier {})",
                name, offset, needed, len
            ),
            D2sError::UnknownStatId { id, bit } => {
                write!(f, "ID de stat inconnu {} (bit {})", id, bit)
            }
            D2sError::UnknownModWidth { id, bit } => write!(
                f,
                "largeur de mod inconnue pour ID {} (bit {}) — table ItemStatCost requise",
                id, bit
            ),
            D2sError::BadChar { byte, bit } => {
                write!(f, "octet 0x{:02X} hors ASCII dans un nom (bit {})", byte, bit)
            }
        }
    }
}

impl std::error::Error for D2sError {}
