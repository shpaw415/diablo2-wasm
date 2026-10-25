//! En-tête fixe du .d2s — partie propre (335 octets, offsets absolus
//! 0x0000–0x014E) + accès typé aux inconnus, sans perte pour le round-trip.
//!
//! Source : evidence.md §1 [CITÉ, concordance krisives + nokka + trevin],
//! croisée avec la référence krisives (DONNÉE TIERS) — concordance totale sur
//! les offsets ; les zones `unk*` sont conservées octet par octet.

#![allow(dead_code)]

use crate::error::D2sError;

/// Taille de la partie fixe de l'en-tête (jusqu'aux quêtes).
pub const HEADER_PROPER_SIZE: usize = 335;
/// Taille totale de l'en-tête fixe (quêtes+waypoints+NPC inclus).
pub const HEADER_SIZE: usize = 765;
/// Signature `0xAA55AA55` (octets disque `55 AA 55 AA`).
pub const MAGIC: u32 = 0xAA55_AA55;

/// Version = 0x60 (96), celle de v1.10+ — notre cible v1.12.
pub const VERSION_V112: u32 = 96;
/// Version 92 = v1.09 (stats au format "gf").
pub const VERSION_V109: u32 = 92;
/// Version 0x61 (97) = Diablo II Resurrected (v1.14+) : stats « gf » + liste
/// 1.10+, items au layout D2R (codes Huffman). [VALIDATION RÉELLE]
pub const VERSION_D2R: u32 = 97;

/// Nom de version lisible (evidence.md §1).
pub fn version_name(version: u32) -> &'static str {
    match version {
        71 => "v1.00-v1.06",
        87 => "v1.07 / expansion v1.08",
        89 => "v1.08 standard",
        92 => "v1.09",
        96 => "v1.10+ (v1.12 = 0x60)",
        97 => "D2R / 0x61 (v1.14+ : stats « gf » + liste 1.10+, items Huffman)",
        _ => "inconnue",
    }
}

/// Classe de personnage (octet 0x28).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Class {
    #[default]
    Amazon,
    Sorceress,
    Necromancer,
    Paladin,
    Barbarian,
    Druid,
    Assassin,
    /// Valeur hors table — conservée pour le round-trip.
    Other(u8),
}

impl Class {
    pub fn from_u8(v: u8) -> Class {
        match v {
            0 => Class::Amazon,
            1 => Class::Sorceress,
            2 => Class::Necromancer,
            3 => Class::Paladin,
            4 => Class::Barbarian,
            5 => Class::Druid,
            6 => Class::Assassin,
            other => Class::Other(other),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Class::Amazon => 0,
            Class::Sorceress => 1,
            Class::Necromancer => 2,
            Class::Paladin => 3,
            Class::Barbarian => 4,
            Class::Druid => 5,
            Class::Assassin => 6,
            Class::Other(v) => v,
        }
    }

    /// Nom canonique anglais (table strings du jeu).
    pub fn name(self) -> &'static str {
        match self {
            Class::Amazon => "Amazon",
            Class::Sorceress => "Sorceress",
            Class::Necromancer => "Necromancer",
            Class::Paladin => "Paladin",
            Class::Barbarian => "Barbarian",
            Class::Druid => "Druid",
            Class::Assassin => "Assassin",
            Class::Other(_) => "inconnue",
        }
    }
}

/// Bits du champ status (0x0024) [CITÉ, 3 sources concordantes].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Status {
    /// bit 2 — mort définitive à la prochaine mort (Hardcore).
    pub hardcore: bool,
    /// bit 3 — déjà mort au moins une fois (reste posé).
    pub died: bool,
    /// bit 5 — personnage d'expansion (Lord of Destruction).
    pub expansion: bool,
    /// bit 6 — Ladder (nokka seul explicite).
    pub ladder: bool,
    /// bits 0, 1, 4, 7 — inconnus, conservés.
    pub unknown: u8,
}

impl Status {
    pub fn from_u8(v: u8) -> Status {
        Status {
            hardcore: v & 0x04 != 0,
            died: v & 0x08 != 0,
            expansion: v & 0x20 != 0,
            ladder: v & 0x40 != 0,
            unknown: v & 0x5B,
        }
    }

    pub fn to_u8(self) -> u8 {
        let mut v = self.unknown & 0x5B;
        if self.hardcore {
            v |= 0x04;
        }
        if self.died {
            v |= 0x08;
        }
        if self.expansion {
            v |= 0x20;
        }
        if self.ladder {
            v |= 0x40;
        }
        v
    }
}

/// Difficulté courante (un des 3 octets 0xA8–0xAA) : bit 7 = active,
/// bits 0–2 = acte (0–4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DifficultyEntry {
    pub active: bool,
    pub act: u8,
    /// Bits 3–6 inconnus, conservés.
    pub unknown: u8,
}

impl DifficultyEntry {
    pub fn from_u8(v: u8) -> DifficultyEntry {
        DifficultyEntry { active: v & 0x80 != 0, act: v & 0x07, unknown: (v >> 3) & 0x0F }
    }

    pub fn to_u8(self) -> u8 {
        (self.act & 0x07) | ((self.unknown & 0x0F) << 3) | if self.active { 0x80 } else { 0 }
    }
}

/// En-tête propre du .d2s (335 premiers octets). Toutes les zones inconnues
/// sont conservées (`unk*`) pour garantir le round-trip octet-à-octet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    /// 0x0000 — signature 0xAA55AA55.
    pub magic: u32,
    /// 0x0004 — version (96 = v1.10+, notre v1.12 ; 92 = v1.09).
    pub version: u32,
    /// 0x0008 — taille du fichier déclarée.
    pub declared_size: u32,
    /// 0x000C — checksum stocké.
    pub checksum: u32,
    /// 0x0010 — active weapon hérité (0/1), non utilisé 1.10+.
    pub active_weapon: u32,
    /// 0x0014 — nom ASCII brut (16 octets, padding 0x00).
    pub name_raw: [u8; 16],
    /// 0x0024 — status.
    pub status: Status,
    /// 0x0025 — progression (titres).
    pub progression: u8,
    /// 0x0026 — inconnu (2 o).
    pub unk_26: [u8; 2],
    /// 0x0028 — classe.
    pub class: Class,
    /// 0x0029 — inconnu (2 o, trevin : {16,30} observés).
    pub unk_29: [u8; 2],
    /// 0x002B — niveau (écran sélection ; = stat `level`).
    pub level: u8,
    /// 0x002C — inconnu (4 o).
    pub unk_2c: [u8; 4],
    /// 0x0030 — horodatage dernière sauvegarde (unix).
    pub timestamp: u32,
    /// 0x0034 — inconnu (4 o).
    pub unk_34: [u8; 4],
    /// 0x0038 — 16 hotkeys (0xFFFF si vide selon trevin, 0x00 selon nokka —
    /// divergence non tranchable sans vrai fichier ; conservées brutes).
    pub hotkeys: [u32; 16],
    /// 0x0078 — skill bouton gauche.
    pub left_skill: u32,
    /// 0x007C — skill bouton droit.
    pub right_skill: u32,
    /// 0x0080 — skill gauche (swap armes).
    pub left_skill_swap: u32,
    /// 0x0084 — skill droit (swap armes).
    pub right_skill_swap: u32,
    /// 0x0088 — apparence menu sélection (32 o).
    pub appearance: [u8; 32],
    /// 0x00A8 — difficulté courante Normal/NM/Hell.
    pub difficulty: [DifficultyEntry; 3],
    /// 0x00AB — map ID (seed liée au fichier .map).
    pub map_id: u32,
    /// 0x00AF — inconnu (2 o).
    pub unk_af: [u8; 2],
    /// 0x00B1 — merc : mort ?
    pub merc_dead: u16,
    /// 0x00B3 — merc : seed/ID (0 = jamais eu).
    pub merc_id: u32,
    /// 0x00B7 — merc : index nom (table strings).
    pub merc_name_index: u16,
    /// 0x00B9 — merc : type (difficulté/acte/élément encodés).
    pub merc_type: u16,
    /// 0x00BB — merc : expérience.
    pub merc_experience: u32,
    /// 0x00BF — inconnu (144 o).
    pub unk_bf: [u8; 144],
}

impl Default for Header {
    fn default() -> Self {
        Header {
            magic: MAGIC,
            version: VERSION_V112,
            declared_size: 0,
            checksum: 0,
            active_weapon: 0,
            name_raw: [0; 16],
            status: Status::default(),
            progression: 0,
            unk_26: [0; 2],
            class: Class::default(),
            // Écriture « retail 1.14d » (étape 19c : saves oracle du
            // générateur vérifiées jeu réel, octets 0x29–0x2A = `10 1E`) —
            // la LECTURE conserve les octets bruts, ce défaut ne concerne
            // que les fichiers NOUVEAUX (export save_d2s).
            unk_29: [16, 30],
            level: 1,
            unk_2c: [0; 4],
            timestamp: 0,
            // 0x34–0x37 : 0xFF en retail (saves oracle 19a/19c).
            unk_34: [0xFF; 4],
            hotkeys: [0xFFFF; 16],
            left_skill: 0,
            right_skill: 0,
            left_skill_swap: 0,
            right_skill_swap: 0,
            // 0x88–0xA7 : apparence = 0xFF en retail (saves oracle
            // 19a/19c) — conservé brut à la lecture.
            appearance: [0xFF; 32],
            difficulty: [DifficultyEntry::default(); 3],
            map_id: 0,
            unk_af: [0; 2],
            merc_dead: 0,
            merc_id: 0,
            merc_name_index: 0,
            merc_type: 0,
            merc_experience: 0,
            unk_bf: [0; 144],
        }
    }
}

fn rd_u32(data: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

fn wr_u32(out: &mut [u8], off: usize, v: u32) {
    out[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn rd_u16(data: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([data[off], data[off + 1]])
}

fn wr_u16(out: &mut [u8], off: usize, v: u16) {
    out[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

impl Header {
    /// Nom du héros (ASCII jusqu'au 0, jamais plus de 15 chars).
    pub fn name(&self) -> String {
        let end = self.name_raw.iter().position(|&b| b == 0).unwrap_or(16);
        String::from_utf8_lossy(&self.name_raw[..end]).into_owned()
    }

    /// Change le nom (padding 0x00, tronque à 16).
    pub fn set_name(&mut self, name: &str) {
        self.name_raw = [0u8; 16];
        for (i, &b) in name.as_bytes().iter().take(16).enumerate() {
            self.name_raw[i] = b;
        }
    }

    /// Parse les 335 premiers octets de `data`.
    pub fn parse(data: &[u8]) -> Result<Header, D2sError> {
        if data.len() < HEADER_PROPER_SIZE {
            return Err(D2sError::TooShort { len: data.len() });
        }
        let mut name_raw = [0u8; 16];
        name_raw.copy_from_slice(&data[0x14..0x24]);
        let mut unk_26 = [0u8; 2];
        unk_26.copy_from_slice(&data[0x26..0x28]);
        let mut unk_29 = [0u8; 2];
        unk_29.copy_from_slice(&data[0x29..0x2B]);
        let mut unk_2c = [0u8; 4];
        unk_2c.copy_from_slice(&data[0x2C..0x30]);
        let mut unk_34 = [0u8; 4];
        unk_34.copy_from_slice(&data[0x34..0x38]);
        let mut hotkeys = [0u32; 16];
        for (i, hk) in hotkeys.iter_mut().enumerate() {
            *hk = rd_u32(data, 0x38 + 4 * i);
        }
        let mut appearance = [0u8; 32];
        appearance.copy_from_slice(&data[0x88..0xA8]);
        let mut unk_af = [0u8; 2];
        unk_af.copy_from_slice(&data[0xAF..0xB1]);
        let mut unk_bf = [0u8; 144];
        unk_bf.copy_from_slice(&data[0xBF..0x14F]);
        Ok(Header {
            magic: rd_u32(data, 0x00),
            version: rd_u32(data, 0x04),
            declared_size: rd_u32(data, 0x08),
            checksum: rd_u32(data, 0x0C),
            active_weapon: rd_u32(data, 0x10),
            name_raw,
            status: Status::from_u8(data[0x24]),
            progression: data[0x25],
            unk_26,
            class: Class::from_u8(data[0x28]),
            unk_29,
            level: data[0x2B],
            unk_2c,
            timestamp: rd_u32(data, 0x30),
            unk_34,
            hotkeys,
            left_skill: rd_u32(data, 0x78),
            right_skill: rd_u32(data, 0x7C),
            left_skill_swap: rd_u32(data, 0x80),
            right_skill_swap: rd_u32(data, 0x84),
            appearance,
            difficulty: [
                DifficultyEntry::from_u8(data[0xA8]),
                DifficultyEntry::from_u8(data[0xA9]),
                DifficultyEntry::from_u8(data[0xAA]),
            ],
            map_id: rd_u32(data, 0xAB),
            unk_af,
            merc_dead: rd_u16(data, 0xB1),
            merc_id: rd_u32(data, 0xB3),
            merc_name_index: rd_u16(data, 0xB7),
            merc_type: rd_u16(data, 0xB9),
            merc_experience: rd_u32(data, 0xBB),
            unk_bf,
        })
    }

    /// Sérialise les 335 octets (réciproque exacte de [`parse`]).
    ///
    /// [`parse`]: Header::parse
    pub fn serialize(&self) -> [u8; HEADER_PROPER_SIZE] {
        let mut out = [0u8; HEADER_PROPER_SIZE];
        wr_u32(&mut out, 0x00, self.magic);
        wr_u32(&mut out, 0x04, self.version);
        wr_u32(&mut out, 0x08, self.declared_size);
        wr_u32(&mut out, 0x0C, self.checksum);
        wr_u32(&mut out, 0x10, self.active_weapon);
        out[0x14..0x24].copy_from_slice(&self.name_raw);
        out[0x24] = self.status.to_u8();
        out[0x25] = self.progression;
        out[0x26..0x28].copy_from_slice(&self.unk_26);
        out[0x28] = self.class.to_u8();
        out[0x29..0x2B].copy_from_slice(&self.unk_29);
        out[0x2B] = self.level;
        out[0x2C..0x30].copy_from_slice(&self.unk_2c);
        wr_u32(&mut out, 0x30, self.timestamp);
        out[0x34..0x38].copy_from_slice(&self.unk_34);
        for (i, &hk) in self.hotkeys.iter().enumerate() {
            wr_u32(&mut out, 0x38 + 4 * i, hk);
        }
        wr_u32(&mut out, 0x78, self.left_skill);
        wr_u32(&mut out, 0x7C, self.right_skill);
        wr_u32(&mut out, 0x80, self.left_skill_swap);
        wr_u32(&mut out, 0x84, self.right_skill_swap);
        out[0x88..0xA8].copy_from_slice(&self.appearance);
        out[0xA8] = self.difficulty[0].to_u8();
        out[0xA9] = self.difficulty[1].to_u8();
        out[0xAA] = self.difficulty[2].to_u8();
        wr_u32(&mut out, 0xAB, self.map_id);
        out[0xAF..0xB1].copy_from_slice(&self.unk_af);
        wr_u16(&mut out, 0xB1, self.merc_dead);
        wr_u32(&mut out, 0xB3, self.merc_id);
        wr_u16(&mut out, 0xB7, self.merc_name_index);
        wr_u16(&mut out, 0xB9, self.merc_type);
        wr_u32(&mut out, 0xBB, self.merc_experience);
        out[0xBF..0x14F].copy_from_slice(&self.unk_bf);
        out
    }

    /// La version utilise-t-elle le flux de stats 1.10+ (0x60) ?
    pub fn is_v110_plus(&self) -> bool {
        self.version >= VERSION_V112
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trip_synthetic() {
        let mut h = Header::default();
        h.set_name("Testchar");
        h.status = Status::from_u8(0x20 | 0x40);
        h.progression = 3;
        h.class = Class::Paladin;
        h.level = 12;
        h.timestamp = 1_761_234_567;
        h.map_id = 0x0BADF00D;
        h.difficulty[0] = DifficultyEntry::from_u8(0x80);
        h.hotkeys[3] = 0;
        h.unk_bf[7] = 0x42; // un inconnu non nul
        let bytes = h.serialize();
        let h2 = Header::parse(&bytes).unwrap();
        assert_eq!(h, h2);
    }

    #[test]
    fn status_bits() {
        let s = Status::from_u8(0x2C); // HC + died + expansion
        assert!(s.hardcore && s.died && s.expansion && !s.ladder);
        assert_eq!(s.to_u8(), 0x2C);
        // bits inconnus préservés (0x01, 0x02, 0x10)
        let s2 = Status::from_u8(0x53);
        assert_eq!(s2.to_u8(), 0x53);
    }

    #[test]
    fn difficulty_bits() {
        let d = DifficultyEntry::from_u8(0x80 | 3);
        assert!(d.active && d.act == 3);
        assert_eq!(d.to_u8(), 0x83);
    }

    #[test]
    fn too_short_is_an_error() {
        assert_eq!(
            Header::parse(&[0u8; 100]),
            Err(D2sError::TooShort { len: 100 })
        );
    }
}
