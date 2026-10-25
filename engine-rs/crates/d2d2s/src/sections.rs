//! Sections fixes de l'en-tête : quêtes (335, 298 o), waypoints (633, 81 o),
//! NPC (714, 51 o) — evidence.md §3–§5.
//!
//! Stratégie round-trip : les octets bruts de chaque section sont conservés
//! intégralement (aucune zone « inconnue » n'est perdue) et une vue typée est
//! décodée à la demande. Sérialiser = recopier les octets : round-trip
//! octet-à-octet garanti par construction.

#![allow(dead_code)]

use crate::error::D2sError;

// ---------------------------------------------------------------------------
// Quêtes — offset 335, 298 octets [CITÉ, trevin détaillé + nokka concordant]
// ---------------------------------------------------------------------------

/// Taille de la section quêtes.
pub const QUESTS_SIZE: usize = 298;
/// Offset absolu des quêtes.
pub const QUESTS_OFFSET: usize = 335;
/// Nombre de difficultés (Normal, Cauchemar, Enfer).
pub const DIFFICULTIES: usize = 3;
/// Taille d'une structure de difficulté (96 o).
pub const QUEST_DIFF_SIZE: usize = 96;

/// Marqueur "Woo!" (octets 57 6F 6F 21).
pub const QUESTS_MARKER: &[u8; 4] = b"Woo!";

/// Bits génériques d'un champ quête u16 (trevin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct QuestFlags(pub u16);

impl QuestFlags {
    /// bit 0 — quête complétée.
    pub fn completed(self) -> bool {
        self.0 & 1 != 0
    }
    /// bit 1 — prérequis fait, récompense à prendre.
    pub fn reward_pending(self) -> bool {
        self.0 & 0b10 != 0
    }
    /// bit 2 — donnée par PNJ.
    pub fn given_by_npc(self) -> bool {
        self.0 & 0b100 != 0
    }
    /// bit 12 — animation de fin vue.
    pub fn end_animation_seen(self) -> bool {
        self.0 & (1 << 12) != 0
    }
    /// bit 13 — complétée dans la partie courante (effacée au reload).
    pub fn completed_this_game(self) -> bool {
        self.0 & (1 << 13) != 0
    }
    /// Bits bruts (spécifiques par quête, cf. trevin : Den of Evil bit 4
    /// entrée, Search for Cain bit 10 niveau vache, Prison of Ice bit 7
    /// parchemin résistance consommé…).
    pub fn raw(self) -> u16 {
        self.0
    }
}

/// Quêtes d'une difficulté, décodées depuis les offsets relatifs de trevin :
/// intro d'acte (Warriv/Jerhyn/Hratli…), 6 quêtes par acte (3 en acte IV),
/// voyages entre actes, Cain acte IV.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestDifficulty {
    /// +0 — intro Warriv acte I.
    pub warriv_intro: QuestFlags,
    /// +2..14 — 6 quêtes acte I.
    pub act1: [QuestFlags; 6],
    /// +14 — voyage I→II.
    pub journey_act1_act2: QuestFlags,
    /// +16 — intro Jerhyn acte II.
    pub jerhyn_intro: QuestFlags,
    /// +18..30 — 6 quêtes acte II.
    pub act2: [QuestFlags; 6],
    /// +30 — voyage II→III.
    pub journey_act2_act3: QuestFlags,
    /// +32 — intro Hratli acte III.
    pub hratli_intro: QuestFlags,
    /// +34..46 — 6 quêtes acte III.
    pub act3: [QuestFlags; 6],
    /// +46 — voyage III→IV.
    pub journey_act3_act4: QuestFlags,
    /// +48 — intro acte IV.
    pub act4_intro: QuestFlags,
    /// +50..56 — 3 quêtes acte IV (6 octets alloués).
    pub act4: [QuestFlags; 3],
    /// +62 — voyage IV→V.
    pub journey_act4_act5: QuestFlags,
    /// +64 — Cain / acte IV.
    pub cain_act4: QuestFlags,
    /// +70..82 — 6 quêtes acte V.
    pub act5: [QuestFlags; 6],
}

fn rd16(raw: &[u8], off: usize) -> QuestFlags {
    QuestFlags(u16::from_le_bytes([raw[off], raw[off + 1]]))
}

impl QuestDifficulty {
    /// Décode une structure de 96 octets.
    pub fn parse(raw: &[u8; QUEST_DIFF_SIZE]) -> QuestDifficulty {
        let act = |off: usize| -> [QuestFlags; 6] {
            core::array::from_fn(|i| rd16(raw, off + 2 * i))
        };
        QuestDifficulty {
            warriv_intro: rd16(raw, 0),
            act1: act(2),
            journey_act1_act2: rd16(raw, 14),
            jerhyn_intro: rd16(raw, 16),
            act2: act(18),
            journey_act2_act3: rd16(raw, 30),
            hratli_intro: rd16(raw, 32),
            act3: act(34),
            journey_act3_act4: rd16(raw, 46),
            act4_intro: rd16(raw, 48),
            act4: core::array::from_fn(|i| rd16(raw, 50 + 2 * i)),
            journey_act4_act5: rd16(raw, 62),
            cain_act4: rd16(raw, 64),
            act5: act(70),
        }
    }
}

// ---------------------------------------------------------------------------
// Waypoints — offset 633, 81 octets [CITÉ, trevin + krisives concordants]
// ---------------------------------------------------------------------------

/// Taille de la section waypoints.
pub const WAYPOINTS_SIZE: usize = 81;
/// Offset absolu des waypoints.
pub const WAYPOINTS_OFFSET: usize = 633;
/// Marqueur "WS".
pub const WAYPOINTS_MARKER: &[u8; 2] = b"WS";

/// Noms des 39 waypoints, ordre des bits 0–38 [CITÉ, ordre communautaire] :
/// actes I–III 9 WP chacun (bits 0–26), acte IV 3 WP (bits 27–29),
/// acte V 9 WP (bits 30–38). Bit 0 = Rogue Encampment (krisives).
pub const WAYPOINT_NAMES: [&str; 39] = [
    // Acte I (bits 0–8)
    "Rogue Encampment",
    "Cold Plains",
    "Stony Field",
    "Dark Wood",
    "Black Marsh",
    "Outer Cloister",
    "Barracks",
    "Catacombs",
    "Monastery Gate",
    // Acte II (bits 9–17) — Halls of the Dead n'a PAS de waypoint.
    "Lut Gholein",
    "Sewers Level 2",
    "Dry Hills",
    "Far Oasis",
    "Lost City",
    "Ancient Tunnels",
    "Palace Cellar Level 1",
    "Arcane Sanctuary",
    "Canyon of the Magi",
    // Acte III (bits 18–26)
    "Kurast Docks",
    "Spider Forest",
    "Great Marsh",
    "Flayer Jungle",
    "Lower Kurast",
    "Kurast Bazaar",
    "Upper Kurast",
    "Travincal",
    "Durance of Hate Level 2",
    // Acte IV (bits 27–29)
    "Pandemonium Fortress",
    "City of the Damned",
    "River of Flame",
    // Acte V (bits 30–38)
    "Harrogath",
    "Frigid Highlands",
    "Arreat Plateau",
    "Crystalline Passage",
    "Halls of Pain",
    "Glacial Trail",
    "Frozen Tundra",
    "Ancient's Way",
    "Worldstone Keep Level 2",
];

/// Waypoints d'une difficulté : bitfield 40 bits (5 octets, LSB d'abord).
/// Bit n = waypoint n activé ; le 1er WP de chaque acte est activé dès
/// l'entrée (krisives).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WaypointBits(pub u64);

impl WaypointBits {
    /// Décode 5 octets LSB-first.
    pub fn from_bytes(bytes: &[u8; 5]) -> WaypointBits {
        let mut v: u64 = 0;
        for (i, &b) in bytes.iter().enumerate() {
            v |= (b as u64) << (8 * i);
        }
        WaypointBits(v)
    }

    /// Encode vers 5 octets LSB-first.
    pub fn to_bytes(self) -> [u8; 5] {
        let mut out = [0u8; 5];
        for (i, b) in out.iter_mut().enumerate() {
            *b = ((self.0 >> (8 * i)) & 0xFF) as u8;
        }
        out
    }

    /// Waypoint `index` (0–38) activé ?
    pub fn is_set(self, index: usize) -> bool {
        index < 40 && self.0 >> index & 1 != 0
    }

    /// Active/désactive le waypoint `index`.
    pub fn set(&mut self, index: usize, on: bool) {
        if index < 40 {
            if on {
                self.0 |= 1 << index;
            } else {
                self.0 &= !(1 << index);
            }
        }
    }

    /// Nom du waypoint `index` (cf. [`WAYPOINT_NAMES`]).
    pub fn name(index: usize) -> Option<&'static str> {
        WAYPOINT_NAMES.get(index).copied()
    }
}

// ---------------------------------------------------------------------------
// NPC — offset 714, 51 octets [CITÉ, trevin « sketchy » assumé]
// ---------------------------------------------------------------------------

/// Taille de la section NPC.
pub const NPC_SIZE: usize = 51;
/// Offset absolu de la section NPC.
pub const NPC_OFFSET: usize = 714;
/// Marqueur "w4".
pub const NPC_MARKER: &[u8; 2] = b"w4";

// ---------------------------------------------------------------------------
// Conteneurs bruts (round-trip) —
// ---------------------------------------------------------------------------

/// Quêtes brutes : "Woo!" (4) + inconnu (6, trevin : toujours
/// `06 00 00 00 2A 01` = 298 = taille de section) + 3×96 o difficultés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quests {
    raw: [u8; QUESTS_SIZE],
}

impl Default for Quests {
    fn default() -> Self {
        let mut raw = [0u8; QUESTS_SIZE];
        raw[0..4].copy_from_slice(QUESTS_MARKER);
        raw[4..10].copy_from_slice(&[0x06, 0x00, 0x00, 0x00, 0x2A, 0x01]);
        Quests { raw }
    }
}

impl Quests {
    /// Depuis une structure brute de 298 octets.
    pub fn from_raw(raw: [u8; QUESTS_SIZE]) -> Quests {
        Quests { raw }
    }

    /// Depuis le fichier complet (offset 335).
    pub fn parse(data: &[u8]) -> Result<Quests, D2sError> {
        if data.len() < QUESTS_OFFSET + QUESTS_SIZE {
            return Err(D2sError::SectionTruncated {
                name: "quests",
                offset: QUESTS_OFFSET,
                needed: QUESTS_SIZE,
                len: data.len(),
            });
        }
        let mut raw = [0u8; QUESTS_SIZE];
        raw.copy_from_slice(&data[QUESTS_OFFSET..QUESTS_OFFSET + QUESTS_SIZE]);
        Ok(Quests { raw })
    }

    /// Octets bruts (round-trip).
    pub fn raw(&self) -> &[u8; QUESTS_SIZE] {
        &self.raw
    }

    /// Le marqueur "Woo!" est-il en place ?
    pub fn marker_ok(&self) -> bool {
        &self.raw[0..4] == QUESTS_MARKER
    }

    /// Les 6 octets d'en-tête inconnus (trevin : `06 00 00 00 2A 01`).
    pub fn header_unknown(&self) -> &[u8] {
        &self.raw[4..10]
    }

    /// Quêtes décodées d'une difficulté (0 = Normal, 1 = Cauchemar, 2 = Enfer).
    pub fn difficulty(&self, index: usize) -> Option<QuestDifficulty> {
        if index >= DIFFICULTIES {
            return None;
        }
        let base = 10 + index * QUEST_DIFF_SIZE;
        let mut buf = [0u8; QUEST_DIFF_SIZE];
        buf.copy_from_slice(&self.raw[base..base + QUEST_DIFF_SIZE]);
        Some(QuestDifficulty::parse(&buf))
    }
}

/// Waypoints bruts : "WS" (2) + inconnu (6, krisives : `01 00 00 00 50 00`)
/// + 3×(2 inconnus + bitfield 5 o + 17 inconnus) + 1 octet final inexpliqué
/// (krisives annonce 81 octets au total).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waypoints {
    raw: [u8; WAYPOINTS_SIZE],
}

impl Default for Waypoints {
    fn default() -> Self {
        let mut raw = [0u8; WAYPOINTS_SIZE];
        raw[0..2].copy_from_slice(WAYPOINTS_MARKER);
        raw[2..8].copy_from_slice(&[0x01, 0x00, 0x00, 0x00, 0x50, 0x00]);
        for d in 0..3 {
            let base = 8 + d * 24;
            raw[base] = 0x02;
            raw[base + 1] = 0x01;
        }
        // Octet 80 : toujours 1 en retail (saves oracle 19a/19c —
        // krisives l'annonçait « inexpliqué », l'oracle le pose à 1 dans
        // TOUTES ses saves, même fraîches). Lecture : conservé brut.
        raw[80] = 1;
        Waypoints { raw }
    }
}

impl Waypoints {
    /// Depuis une structure brute de 81 octets.
    pub fn from_raw(raw: [u8; WAYPOINTS_SIZE]) -> Waypoints {
        Waypoints { raw }
    }

    /// Depuis le fichier complet (offset 633).
    pub fn parse(data: &[u8]) -> Result<Waypoints, D2sError> {
        if data.len() < WAYPOINTS_OFFSET + WAYPOINTS_SIZE {
            return Err(D2sError::SectionTruncated {
                name: "waypoints",
                offset: WAYPOINTS_OFFSET,
                needed: WAYPOINTS_SIZE,
                len: data.len(),
            });
        }
        let mut raw = [0u8; WAYPOINTS_SIZE];
        raw.copy_from_slice(&data[WAYPOINTS_OFFSET..WAYPOINTS_OFFSET + WAYPOINTS_SIZE]);
        Ok(Waypoints { raw })
    }

    /// Octets bruts (round-trip).
    pub fn raw(&self) -> &[u8; WAYPOINTS_SIZE] {
        &self.raw
    }

    /// Le marqueur "WS" est-il en place ?
    pub fn marker_ok(&self) -> bool {
        &self.raw[0..2] == WAYPOINTS_MARKER
    }

    /// Les 6 octets inconnus après "WS" (krisives : `01 00 00 00 50 00`).
    pub fn header_unknown(&self) -> &[u8] {
        &self.raw[2..8]
    }

    /// Bitfield d'une difficulté (0 = Normal, 1 = Cauchemar, 2 = Enfer).
    pub fn difficulty(&self, index: usize) -> Option<WaypointBits> {
        if index >= DIFFICULTIES {
            return None;
        }
        let base = 8 + index * 24;
        let mut b = [0u8; 5];
        b.copy_from_slice(&self.raw[base + 2..base + 7]);
        Some(WaypointBits::from_bytes(&b))
    }

    /// Écrit le bitfield d'une difficulté (les inconnus de structure sont
    /// conservés).
    pub fn set_difficulty(&mut self, index: usize, bits: WaypointBits) -> bool {
        if index >= DIFFICULTIES {
            return false;
        }
        let base = 8 + index * 24;
        self.raw[base + 2..base + 7].copy_from_slice(&bits.to_bytes());
        true
    }

    /// Waypoint `name` activé en difficulté `difficulty` ?
    pub fn is_active(&self, difficulty: usize, name: &str) -> Option<bool> {
        let idx = WAYPOINT_NAMES.iter().position(|&n| n == name)?;
        self.difficulty(difficulty).map(|b| b.is_set(idx))
    }
}

/// NPC bruts : "w4" (2) + 1 inconnu + 5 o bitfield introductions Normal
/// (Warriv/Charsi/Kashya/Akara/Gheed ; Greiz/Meshif/Geglash/Lysander/Fara/
/// Drognan ; Alkor/Asheara/Cain/Elzix ; Malah/Anya/Natalya/Meshif/Ormus ;
/// Cain/Qual-Kehk/Nihlathak) + 3 inconnus + 8 o intro NM + 8 Hell + 4 o
/// « bienvenue au retour » + 4 inconnus + 8 o salutations NM + 8 Hell.
/// **Partiellement décodé** (trevin « sketchy ») — tout est conservé brut.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Npc {
    raw: [u8; NPC_SIZE],
}

impl Default for Npc {
    fn default() -> Self {
        let mut raw = [0u8; NPC_SIZE];
        raw[0..2].copy_from_slice(NPC_MARKER);
        Npc { raw }
    }
}

impl Npc {
    /// Depuis une structure brute de 51 octets.
    pub fn from_raw(raw: [u8; NPC_SIZE]) -> Npc {
        Npc { raw }
    }

    /// Depuis le fichier complet (offset 714).
    pub fn parse(data: &[u8]) -> Result<Npc, D2sError> {
        if data.len() < NPC_OFFSET + NPC_SIZE {
            return Err(D2sError::SectionTruncated {
                name: "npc",
                offset: NPC_OFFSET,
                needed: NPC_SIZE,
                len: data.len(),
            });
        }
        let mut raw = [0u8; NPC_SIZE];
        raw.copy_from_slice(&data[NPC_OFFSET..NPC_OFFSET + NPC_SIZE]);
        Ok(Npc { raw })
    }

    /// Octets bruts (round-trip).
    pub fn raw(&self) -> &[u8; NPC_SIZE] {
        &self.raw
    }

    /// Le marqueur "w4" est-il en place ?
    pub fn marker_ok(&self) -> bool {
        &self.raw[0..2] == NPC_MARKER
    }

    /// Bitfield 5 octets des introductions Normal (octets 3..8).
    pub fn intro_bits_normal(&self) -> u64 {
        let mut v: u64 = 0;
        for (i, &b) in self.raw[3..8].iter().enumerate() {
            v |= (b as u64) << (8 * i);
        }
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quest_flags_bits() {
        let q = QuestFlags((1 << 0) | (1 << 2) | (1 << 13));
        assert!(q.completed() && q.given_by_npc() && q.completed_this_game());
        assert!(!q.reward_pending() && !q.end_animation_seen());
    }

    #[test]
    fn quest_difficulty_layout() {
        // Structure de 96 o : positions relatives trevin.
        let mut raw = [0u8; 96];
        for (i, off) in [0usize, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32, 34, 36, 38, 40, 42, 44, 46, 48, 50, 52, 54, 62, 64, 70, 72, 74, 76, 78, 80, 82]
            .iter()
            .enumerate()
        {
            raw[*off] = (i + 1) as u8;
        }
        let q = QuestDifficulty::parse(&raw);
        assert_eq!(q.warriv_intro.0, 1);
        assert_eq!(q.act1[0].0, 2);
        assert_eq!(q.act1[5].0, 7);
        assert_eq!(q.journey_act1_act2.0, 8);
        assert_eq!(q.jerhyn_intro.0, 9);
        assert_eq!(q.act2[5].0, 15);
        assert_eq!(q.act4.len(), 3);
        assert_eq!(q.act4[0].0, 26);
        assert_eq!(q.journey_act4_act5.0, 29);
        assert_eq!(q.cain_act4.0, 30);
        assert_eq!(q.act5[0].0, 31);
        assert_eq!(q.act5[5].0, 36);
    }

    #[test]
    fn waypoint_bits_lsb_first() {
        // bit 0 = octet 0 bit 0 ; bit 8 = octet 1 bit 0.
        let bytes = [0b0000_0001, 0b0000_0010, 0, 0, 0];
        let b = WaypointBits::from_bytes(&bytes);
        assert!(b.is_set(0)); // Rogue Encampment
        assert!(b.is_set(9)); // Lut Gholein
        assert!(!b.is_set(1));
        assert_eq!(b.to_bytes(), bytes);
        assert_eq!(WaypointBits::name(0), Some("Rogue Encampment"));
        assert_eq!(WaypointBits::name(27), Some("Pandemonium Fortress"));
        assert_eq!(WaypointBits::name(38), Some("Worldstone Keep Level 2"));
        assert_eq!(WaypointBits::name(39), None);
    }

    #[test]
    fn quests_defaults_and_decode() {
        let q = Quests::default();
        assert!(q.marker_ok());
        assert_eq!(q.header_unknown(), &[6, 0, 0, 0, 0x2A, 0x01]);
        let d0 = q.difficulty(0).unwrap();
        assert!(!d0.act1[0].completed());
        assert!(q.difficulty(3).is_none());
        // round-trip brut
        assert_eq!(Quests::from_raw(*q.raw()), q);
    }

    #[test]
    fn waypoints_set_and_decode() {
        let mut w = Waypoints::default();
        assert!(w.marker_ok());
        assert_eq!(w.header_unknown(), &[1, 0, 0, 0, 0x50, 0]);
        let mut bits = w.difficulty(1).unwrap();
        assert!(!bits.is_set(0));
        bits.set(0, true);
        w.set_difficulty(1, bits);
        assert_eq!(w.is_active(1, "Rogue Encampment"), Some(true));
        assert_eq!(w.is_active(0, "Rogue Encampment"), Some(false));
        assert_eq!(w.is_active(1, "Nulle Part"), None);
        // les inconnus de structure {02 01} sont intacts
        assert_eq!(&w.raw()[8..10], &[0x02, 0x01]);
        assert_eq!(Waypoints::from_raw(*w.raw()), w);
    }

    #[test]
    fn npc_defaults() {
        let n = Npc::default();
        assert!(n.marker_ok());
        assert_eq!(n.intro_bits_normal(), 0);
        assert_eq!(Npc::from_raw(*n.raw()), n);
    }
}
