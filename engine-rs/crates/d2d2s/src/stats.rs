//! Stats du personnage — bit-stream à partir de l'octet 765 (evidence.md §6).
//!
//! * **v1.10+ (0x60, notre v1.12)** : flux direct de `(ID 9 bits, valeur =
//!   CSvBits[id] bits)`, IDs triés croissants, terminé par `0x1FF`. Le bit 0
//!   du flux est le bit de poids faible de l'octet 765 (LSB-first / IFE).
//!   Recharge : `value = raw >> ValShift` (hp/mana/stamina stockés ×256).
//! * **v1.09 (0x5C)** : marqueur `"gf"` + u16 bitfield de présence + u32
//!   brut par stat présente (trevin).
//!
//! Largeurs : colonne `CSvBits` de la table ItemStatCost v1.12 RÉELLE
//! embarquée (`itemstatcost_d2s.tsv`) — validation croisée locale 16/16 avec
//! la table de largeurs de nokka (evidence.md §6) ; repli codé en dur
//! identique si l'ID manque de la table.

#![allow(dead_code)]

use crate::bits::{BitReader, BitWriter};
use crate::error::D2sError;
use crate::itemstatcost;

/// ID de fin de flux (stats 1.10+ et mods d'items).
pub const END_ID: u16 = 0x1FF;

/// Largeurs CSvBits des 16 stats de base — repli codé en dur, identique bit
/// pour bit à la colonne CSvBits de la table embarquée (evidence.md §6,
/// validation croisée locale) et à la table nokka citée.
pub const FALLBACK_CSV_BITS: [(u16, u8); 16] = [
    (0, 10),  // strength
    (1, 10),  // energy
    (2, 10),  // dexterity
    (3, 10),  // vitality
    (4, 10),  // statpts
    (5, 8),   // newskills
    (6, 21),  // hitpoints
    (7, 21),  // maxhp
    (8, 21),  // mana
    (9, 21),  // maxmana
    (10, 21), // stamina
    (11, 21), // maxstamina
    (12, 7),  // level
    (13, 32), // experience
    (14, 25), // gold
    (15, 25), // goldbank
];

/// ValShift : stats ÷256 à la lecture (stockées ×256).
pub const FALLBACK_VALSHIFT: [(u16, u8); 6] =
    [(6, 8), (7, 8), (8, 8), (9, 8), (10, 8), (11, 8)];

/// Noms canoniques des 16 stats de base (d2s_parse.py / nokka).
pub const STAT_NAMES: [(u16, &str); 16] = [
    (0, "strength"),
    (1, "energy"),
    (2, "dexterity"),
    (3, "vitality"),
    (4, "statpts"),
    (5, "newskills"),
    (6, "hitpoints"),
    (7, "maxhp"),
    (8, "mana"),
    (9, "maxmana"),
    (10, "stamina"),
    (11, "maxstamina"),
    (12, "level"),
    (13, "experience"),
    (14, "gold"),
    (15, "goldbank"),
];

/// Largeur de sauvegarde d'une stat caractère (CSvBits).
pub fn csv_bits(id: u16) -> Option<u8> {
    if let Some(row) = itemstatcost::embedded().row(id) {
        // CSvBits peut être 0 dans la table pour des stats hors d2s.
        if row.csv_bits > 0 {
            return Some(row.csv_bits);
        }
    }
    FALLBACK_CSV_BITS
        .iter()
        .find(|(i, _)| *i == id)
        .map(|&(_, bits)| bits)
}

/// ValShift d'une stat caractère (0 si absent).
pub fn val_shift(id: u16) -> u8 {
    if let Some(row) = itemstatcost::embedded().row(id) {
        return row.val_shift;
    }
    FALLBACK_VALSHIFT
        .iter()
        .find(|(i, _)| *i == id)
        .map(|&(_, s)| s)
        .unwrap_or(0)
}

/// Nom d'une stat de base.
pub fn stat_name(id: u16) -> Option<&'static str> {
    STAT_NAMES.iter().find(|(i, _)| *i == id).map(|&(_, n)| n)
}

/// Format du flux de stats tel que lu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatsFormat {
    /// 1.10+ (0x60) : flux (ID 9 bits, valeur CSvBits) + 0x1FF.
    V110Plus,
    /// 1.09 (0x5C) : "gf" + bitfield présence u16 + u32 par stat présente.
    Gf109,
    /// D2R (0x61) : marqueur "gf" + flux (ID 9 bits, valeur CSvBits) + 0x1FF.
    /// [VALIDATION RÉELLE 2026-10-24 : Javazon.d2s — les 2 octets `gf`
    /// (0x67 0x66) précèdent la liste ; l'« ID 103 » du parseur python
    /// était ces octets décodés comme un ID. Confirmé par le parseur
    /// D2R fredimachado/Diablo2R (« reader.ReadInt16() // Header »).]
    /// **Étape 19a (2026-10-25)** : ce format n'est PAS propre à la branche
    /// 0x61 — les fichiers 0x60 ÉCRITS PAR LE JEU RÉEL (référence v1.14d de
    /// l'oracle, `Amazon_1_SC_GOLD.d2s`) et les 14 vecteurs oracle ont aussi
    /// le préfixe `gf` devant le flux 1.10+. Le format est donc enregistré
    /// ici dès que le préfixe est présent, quelle que soit la version ;
    /// la branche (0x60/0x61) reste portée par `header.version`.
    GfPrefixed110,
}

/// Une stat lue : `raw` = valeur stockée dans le flux, `value` = recharge
/// (`raw >> ValShift`). Les deux sont conservés pour le round-trip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatEntry {
    pub id: u16,
    pub raw: u32,
    pub value: u32,
}

/// Collection de stats telle que parsée (ordre du flux conservé).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Stats {
    pub format: StatsFormat,
    pub entries: Vec<StatEntry>,
}

impl Stats {
    /// Parse le flux de stats à partir de `bit_offset` (765*8 dans le
    /// fichier), branche choisie par `version`.
    pub fn parse(reader: &mut BitReader, version: u32) -> Result<Stats, D2sError> {
        if version < crate::header::VERSION_V112 {
            Self::parse_gf109(reader)
        } else if version >= crate::header::VERSION_D2R {
            Self::parse_gf_prefixed_110(reader)
        } else {
            Self::parse_v110_legacy(reader)
        }
    }

    /// Branche D2R (0x61) : "gf" + (ID 9 bits, valeur CSvBits) + 0x1FF.
    fn parse_gf_prefixed_110(reader: &mut BitReader) -> Result<Stats, D2sError> {
        let start = reader.bit_pos();
        let marker = reader.read_ascii(2)?;
        if marker != "gf" {
            return Err(D2sError::BadMarker {
                expected: "gf".into(),
                got: marker,
                bit: start,
            });
        }
        let Stats { entries, .. } = Self::parse_v110(reader)?;
        Ok(Stats { format: StatsFormat::GfPrefixed110, entries })
    }

    /// Branche 0x60 : flux 1.10+ avec préfixe « gf » OPTIONNEL.
    ///
    /// Divergence tranchée par les DONNÉES (étape 19a, 2026-10-25) :
    /// l'evidence (vecteurs synthétiques) écrivait le flux 0x60 SANS
    /// préfixe, mais le fichier réel v1.14d (référence de l'oracle
    /// `Amazon_1_SC_GOLD.d2s`, version 0x60 vérifiée à l'octet 4) et les 14
    /// saves oracle commencent par `67 66` = « gf ». Sans préfixe, ces 2
    /// octets décodent en « ID 103 » (0x167|… = 103, cf. erreur
    /// `UnknownStatId { id: 103 }` du sondage d'avant fix) — la sonde est
    /// donc sûre : un flux de stats de personnage commence toujours par un
    /// ID de base ≤ 12 (entrées triées croissantes, str/ene/dex/vit
    /// toujours présentes), jamais 103.
    fn parse_v110_legacy(reader: &mut BitReader) -> Result<Stats, D2sError> {
        if reader.peek_bytes(2) == Some(b"gf") {
            reader.read(16)?;
            let Stats { entries, .. } = Self::parse_v110(reader)?;
            return Ok(Stats { format: StatsFormat::GfPrefixed110, entries });
        }
        Self::parse_v110(reader)
    }

    /// Branche 1.10+ : (ID 9 bits, valeur CSvBits) croissant + 0x1FF.
    fn parse_v110(reader: &mut BitReader) -> Result<Stats, D2sError> {
        let mut entries = Vec::with_capacity(16);
        loop {
            let start = reader.bit_pos();
            let id = reader.read(9)?;
            if id == END_ID as u32 {
                break;
            }
            let nbits = csv_bits(id as u16).ok_or(D2sError::UnknownStatId {
                id: id as u16,
                bit: start,
            })? as usize;
            let raw = reader.read(nbits)?;
            let shift = val_shift(id as u16) as usize;
            entries.push(StatEntry { id: id as u16, raw, value: raw >> shift });
        }
        Ok(Stats { format: StatsFormat::V110Plus, entries })
    }

    /// Branche 1.09 : "gf" + présence u16 + u32 par bit posé.
    fn parse_gf109(reader: &mut BitReader) -> Result<Stats, D2sError> {
        let start = reader.bit_pos();
        let marker = reader.read_ascii(2)?;
        if marker != "gf" {
            return Err(D2sError::BadMarker {
                expected: "gf".into(),
                got: marker,
                bit: start,
            });
        }
        let presence = reader.read(16)?;
        let mut entries = Vec::with_capacity(presence.count_ones() as usize);
        for sid in 0..16u32 {
            if presence >> sid & 1 == 0 {
                continue;
            }
            let raw = reader.read(32)?;
            let shift = val_shift(sid as u16) as usize;
            entries.push(StatEntry { id: sid as u16, raw, value: raw >> shift });
        }
        Ok(Stats { format: StatsFormat::Gf109, entries })
    }

    /// Sérialise dans `writer` (réciproque exacte de [`Stats::parse`] ;
    /// les IDs 1.10+ sont réécrits triés croissants comme l'exige la spec).
    pub fn serialize(&self, writer: &mut BitWriter) {
        match self.format {
            StatsFormat::V110Plus | StatsFormat::GfPrefixed110 => {
                if self.format == StatsFormat::GfPrefixed110 {
                    writer.write_ascii("gf");
                }
                let mut sorted: Vec<&StatEntry> = self.entries.iter().collect();
                sorted.sort_by_key(|e| e.id);
                for e in sorted {
                    let nbits = csv_bits(e.id).unwrap_or(32) as usize;
                    writer.write(e.id as u32, 9);
                    writer.write(e.raw & crate::bits::mask(nbits), nbits);
                }
                writer.write(END_ID as u32, 9);
                writer.align();
            }
            StatsFormat::Gf109 => {
                writer.write_ascii("gf");
                let mut presence: u32 = 0;
                for e in &self.entries {
                    if e.id < 16 {
                        presence |= 1 << e.id;
                    }
                }
                writer.write(presence & 0xFFFF, 16);
                for sid in 0..16u16 {
                    if presence >> sid & 1 == 0 {
                        continue;
                    }
                    if let Some(e) = self.entries.iter().find(|e| e.id == sid) {
                        writer.write(e.raw, 32);
                    }
                }
                writer.align();
            }
        }
    }

    /// Valeur rechargée d'une stat par ID.
    pub fn value(&self, id: u16) -> Option<u32> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.value)
    }

    /// Valeur brute stockée d'une stat par ID.
    pub fn raw(&self, id: u16) -> Option<u32> {
        self.entries.iter().find(|e| e.id == id).map(|e| e.raw)
    }

    /// Démonte / remplace une stat (raw et value cohérents).
    pub fn set(&mut self, id: u16, value: u32) {
        let shift = val_shift(id) as usize;
        let raw = value << shift;
        if let Some(e) = self.entries.iter_mut().find(|e| e.id == id) {
            e.raw = raw;
            e.value = value;
        } else {
            self.entries.push(StatEntry { id, raw, value });
        }
    }
}

/// Section skills : "if" + 30 octets de niveaux (octet i = points dans le
/// i-ème slot de compétence de la classe ; mapping slot→skill ID par offset
/// de classe dans skills.txt : Amazon 6, Sorceress 36, Necromancer 66,
/// Paladin 96, Barbarian 126, Druid 221, Assassin 251).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skills {
    raw: [u8; 32],
}

pub const SKILLS_SIZE: usize = 32;

impl Default for Skills {
    fn default() -> Self {
        let mut raw = [0u8; SKILLS_SIZE];
        raw[0..2].copy_from_slice(b"if");
        Skills { raw }
    }
}

impl Skills {
    /// Depuis 32 octets du fichier (offset = fin du flux stats).
    pub fn parse(data: &[u8], offset: usize) -> Result<Skills, D2sError> {
        if offset + SKILLS_SIZE > data.len() {
            return Err(D2sError::SectionTruncated {
                name: "skills",
                offset,
                needed: SKILLS_SIZE,
                len: data.len(),
            });
        }
        let mut raw = [0u8; SKILLS_SIZE];
        raw.copy_from_slice(&data[offset..offset + SKILLS_SIZE]);
        Ok(Skills { raw })
    }

    /// Octets bruts (round-trip).
    pub fn raw(&self) -> &[u8; SKILLS_SIZE] {
        &self.raw
    }

    /// Le marqueur "if" est-il en place ?
    pub fn marker_ok(&self) -> bool {
        &self.raw[0..2] == b"if"
    }

    /// Niveaux des 30 slots (octets 2..32).
    pub fn levels(&self) -> &[u8] {
        &self.raw[2..]
    }

    /// Points dans le slot i.
    pub fn level(&self, slot: usize) -> u8 {
        self.raw[2 + slot]
    }

    /// Définit les points d'un slot.
    pub fn set_level(&mut self, slot: usize, points: u8) {
        self.raw[2 + slot] = points;
    }

    /// Offset de départ des slots dans skills.txt, par classe (evidence §7).
    pub fn class_slot_base(class: crate::header::Class) -> u16 {
        use crate::header::Class::*;
        match class {
            Amazon => 6,
            Sorceress => 36,
            Necromancer => 66,
            Paladin => 96,
            Barbarian => 126,
            Druid => 221,
            Assassin => 251,
            crate::header::Class::Other(_) => 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_bits_match_local_table_and_nokka() {
        for (id, bits) in FALLBACK_CSV_BITS {
            assert_eq!(csv_bits(id), Some(bits), "id {} CSvBits", id);
        }
        assert_eq!(csv_bits(300), None);
    }

    #[test]
    fn val_shift_hp_is_8() {
        for id in [6u16, 7, 8, 9, 10, 11] {
            assert_eq!(val_shift(id), 8);
        }
        assert_eq!(val_shift(0), 0);
    }

    #[test]
    fn stats_v110_round_trip() {
        // mêmes valeurs que le vecteur minimal
        let values: [(u16, u32); 9] = [
            (0, 20),
            (1, 25),
            (2, 20),
            (3, 25),
            (7, 148),
            (12, 12),
            (13, 123_456),
            (14, 1_000),
            (15, 2_500),
        ];
        let stats = Stats {
            format: StatsFormat::V110Plus,
            entries: values
                .iter()
                .map(|&(id, raw)| StatEntry { id, raw, value: raw >> val_shift(id) as usize })
                .collect(),
        };
        let mut w = BitWriter::new();
        stats.serialize(&mut w);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        let back = Stats::parse(&mut r, crate::header::VERSION_V112).unwrap();
        assert_eq!(back, stats);
        assert_eq!(back.value(0), Some(20));
        assert_eq!(back.value(7), Some(0)); // 148 >> 8
        assert_eq!(back.raw(7), Some(148));
        assert_eq!(back.value(13), Some(123_456));
    }

    #[test]
    fn stats_gf109_round_trip() {
        let stats = Stats {
            format: StatsFormat::Gf109,
            entries: vec![
                StatEntry { id: 0, raw: 20, value: 20 },
                StatEntry { id: 12, raw: 12, value: 12 },
                StatEntry { id: 14, raw: 1_000, value: 1_000 },
            ],
        };
        let mut w = BitWriter::new();
        stats.serialize(&mut w);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        let back = Stats::parse(&mut r, crate::header::VERSION_V109).unwrap();
        assert_eq!(back, stats);
        assert_eq!(back.value(14), Some(1_000));
    }

    #[test]
    fn stats_unknown_id_errors() {
        let mut w = BitWriter::new();
        w.write(300, 9); // pas une stat de base
        w.write(0, 10);
        w.write(END_ID as u32, 9);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        assert!(matches!(
            Stats::parse(&mut r, crate::header::VERSION_V112),
            Err(D2sError::UnknownStatId { id: 300, .. })
        ));
    }

    #[test]
    fn stats_premature_end_errors() {
        let mut w = BitWriter::new();
        w.write(0, 9);
        w.write(20, 10); // pas de 0x1FF — comme truncated.d2s
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        assert!(matches!(
            Stats::parse(&mut r, crate::header::VERSION_V112),
            Err(D2sError::PrematureEnd { .. })
        ));
    }

    #[test]
    fn skills_slots() {
        let mut s = Skills::default();
        assert!(s.marker_ok());
        s.set_level(3, 5);
        s.set_level(8, 3);
        assert_eq!(s.level(3), 5);
        assert_eq!(s.levels()[3], 5);
        assert_eq!(Skills::class_slot_base(crate::header::Class::Paladin), 96);
        assert_eq!(Skills::parse(s.raw(), 0).unwrap(), s);
    }
}
