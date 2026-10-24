//! `d2d2s` — lecture/écriture du format de sauvegarde `.d2s` de Diablo II
//! **v1.12** pour le port WASM (crate PURE : pas de rendu, pas d'IO, pas de
//! wasm — `std` seul, aucune dépendance).
//!
//! # Sources et hiérarchie des preuves
//!
//! * **AUTORITÉ** : `d2w-re/evidence-formats/d2s/evidence.md` (campagne RE
//!   d2w-re, étape 2) — offsets, checksum, sections, bits d'items, croisés
//!   avec la table `ItemStatCost.txt` v1.12 **réellement extraite des MPQ**
//!   (embarquée ici : `data/itemstatcost_d2s.tsv`).
//! * **DONNÉE TIERS** : `d2w-re/reference/d2s-format-krisives.md` — utilisée
//!   uniquement là où elle concorde ; divergences tranchées ci-dessous.
//! * **VECTEURS** : `tests/vectors/` — 7 blobs synthétiques (copies fidèles,
//!   SHA-256 vérifiées) générés par `make_vectors.py` depuis l'evidence.
//!
//! # Divergences evidence vs krisives — tranchées (l'evidence gagne)
//!
//! | Sujet | krisives (tiers) | Retenu (evidence) | Pourquoi |
//! |---|---|---|---|
//! | Type d'item bits 76–107 | 4 bits @76 + 24 bits @80 (3 lettres) | **4 ASCII (32 bits)** | trevin + parseurs réels concordants (evidence §8.2 [CITÉ]) ; nos vecteurs écrivent/lisent 4 ASCII (`rin `, `cap `, `cm1 `) et le round-trip octet-à-octet le prouve. |
//! | Nb de sockets (armure/arme sertie) | 3 bits | **4 bits** | trevin + parseurs réels (evidence §8.5) ; le vecteur `socketed_armor` (2 sockets, liste de mods `0x1FF` juste après) ne se lit juste qu'avec 4 bits — à 3 bits le marqueur final est décalé d'un bit. |
//! | Largeur d'un mod | `Save Bits` (+ Param Bits) | **`Save Bits` seule** | table ItemStatCost v1.12 locale [VÉRIFIÉ-LOCAL, 247 stats] ; le vecteur `magic_item` encode strength/maxmana sur 8 bits = `Save Bits` — avec Param Bits le round-trip échoue. |
//!
//! Concordances sans divergence : signature `0xAA55AA55` (octets disque
//! `55 AA 55 AA`), version `0x60` = v1.12, en-tête 765 o, checksum
//! (rotation gauche + octet, champ neutralisé), quêtes @335/298, waypoints
//! @633/81 (bit 0 = Rogue Encampment), NPC @714/51, slots équipés 1–12,
//! conteneurs stockés 1/4/5.
//!
//! # ⚠ Limite assumée
//!
//! **Aucun vrai `.d2s` retail n'a encore validé ce crate** (aucun fichier
//! réel disponible sur le bureau au 2026-10-24). Tout est testé contre les
//! vecteurs SYNTHÉTIQUES de l'evidence ; la validation finale est prévue
//! quand Justin fournira un vrai fichier. Ne pas faire confiance
//! aveuglément aux zones [NON VÉRIFIÉ] de l'evidence (hotkeys `0xFFFF` vs
//! `0x00`, octet final waypoints, section NPC « sketchy »…) — elles sont
//! conservées brutes ici, donc sans risque pour le round-trip.
//!
//! # PAS de branchement dans le save v3
//!
//! Le save JSON v3 du port reste **le défaut** : ce crate expose seulement
//! l'API et la conversion vers une struct interne minimale
//! ([`CharacterSummary`], testée). Guide pour l'intégrateur suivant :
//!
//! ```
//! use d2d2s::SaveFile;
//!
//! // 1. Parser des octets (fichier lu ailleurs — zéro IO dans ce crate).
//! //    let bytes = std::fs::read("mon_personnage.d2s")?;
//! //    let save = SaveFile::parse(&bytes)?;   // strict : signature+checksum requis
//!
//! // 2. Construire une save valide en mémoire (round-trip prouvé par les tests).
//! let mut save = SaveFile::default_v112();
//! save.header.set_name("Sorchar");
//! save.stats.set(0, 45); // strength
//! let bytes = save.to_bytes();            // taille + checksum recalculés
//!
//! // 3. Relire.
//! let back = SaveFile::parse(&bytes).unwrap();
//! assert_eq!(back.header.name(), "Sorchar");
//! assert_eq!(back.stats.value(0), Some(45));
//!
//! // 4. Conversion vers la struct interne minimale (pour d2game/d2wasm).
//! let summary = back.to_summary();
//! assert_eq!(summary.name, "Sorchar");
//! assert_eq!(summary.strength, Some(45));
//! ```
//!
//! # Garantie round-trip
//!
//! `parse` → `to_bytes` restitue **octet pour octet** les fichiers bien
//! formés (testé sur les 4 vecteurs valides) : toutes les zones inconnues
//! (`unk*`, bit 39 des items, apparence, padding) sont conservées.
//! `to_bytes` recalcule taille déclarée et checksum — identiques à l'origine
//! quand le fichier est cohérent.

#![forbid(unsafe_code)]
#![allow(clippy::needless_range_loop)]

pub mod bits;
pub mod checksum;
pub mod codes;
pub mod error;
pub mod header;
pub mod items;
pub mod itemstatcost;
pub mod sections;
pub mod stats;

pub use checksum::compute_checksum;
pub use error::D2sError;
pub use header::{version_name, Class, DifficultyEntry, Header, Status, HEADER_SIZE};
pub use items::{
    Container, EarData, EquipSlot, Item, ItemExtended, ItemHeader, Location, Mod, Quality,
    QualityData, TypeData,
};
pub use sections::{
    Npc, QuestDifficulty, QuestFlags, Quests, WaypointBits, Waypoints, QUESTS_OFFSET,
    QUESTS_SIZE, WAYPOINTS_OFFSET, WAYPOINTS_SIZE, WAYPOINT_NAMES,
};
pub use stats::{Skills, StatEntry, Stats, StatsFormat, END_ID};

use bits::{BitReader, BitWriter};

/// Liste d'items "JM" (caractère, corpse, merc, golem).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemList {
    /// Nombre déclaré dans le flux (u16).
    pub declared_count: u16,
    pub items: Vec<Item>,
}

/// Trailer "kf" du golem de fer (Nécromancien).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GolemTrailer {
    /// Octet flag après "kf" (0 = pas de golem).
    pub has_golem: bool,
    /// Items du golem (si flag).
    pub items: Vec<Item>,
}

/// Sauvegarde complète `.d2s` : en-tête 765 o + sections variables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SaveFile {
    /// En-tête propre (335 premiers octets).
    pub header: Header,
    /// Quêtes (offset 335, 298 o).
    pub quests: Quests,
    /// Waypoints (offset 633, 81 o).
    pub waypoints: Waypoints,
    /// NPC (offset 714, 51 o).
    pub npc: Npc,
    /// Stats (bit-stream depuis 765).
    pub stats: Stats,
    /// Skills ("if" + 30 niveaux).
    pub skills: Skills,
    /// Liste d'items du personnage ("JM").
    pub character_items: ItemList,
    /// Liste corpse (présente seulement si les octets "JM" suivent).
    pub corpse_items: Option<ItemList>,
    /// Liste mercenaire ("jf" + liste, expansion).
    pub merc_items: Option<ItemList>,
    /// Trailer golem de fer ("kf"), si présent.
    pub golem: Option<GolemTrailer>,
}

impl Default for SaveFile {
    fn default() -> Self {
        Self::default_v112()
    }
}

impl SaveFile {
    /// Sauvegarde v1.12 vide et valide (personnage anonyme, aucune stat).
    pub fn default_v112() -> SaveFile {
        SaveFile {
            header: Header::default(),
            quests: Quests::default(),
            waypoints: Waypoints::default(),
            npc: Npc::default(),
            stats: Stats { format: StatsFormat::V110Plus, entries: Vec::new() },
            skills: Skills::default(),
            character_items: ItemList { declared_count: 0, items: Vec::new() },
            corpse_items: None,
            merc_items: None,
            golem: Some(GolemTrailer { has_golem: false, items: Vec::new() }),
        }
    }

    /// Parse strict : signature ET checksum doivent être valides
    /// (comportement du jeu : un checksum faux fait refuser la sauvegarde).
    pub fn parse(data: &[u8]) -> Result<SaveFile, D2sError> {
        Self::parse_inner(data, true)
    }

    /// Parse tolérant : signature/checksum invalides acceptés (outils
    /// d'inspection) — les erreurs STRUCTURELLES restent fatales.
    pub fn parse_lenient(data: &[u8]) -> Result<SaveFile, D2sError> {
        Self::parse_inner(data, false)
    }

    fn parse_inner(data: &[u8], strict: bool) -> Result<SaveFile, D2sError> {
        if data.len() < HEADER_SIZE {
            return Err(D2sError::TooShort { len: data.len() });
        }

        let header = Header::parse(data)?;
        if strict && header.magic != header::MAGIC {
            return Err(D2sError::BadSignature { got: header.magic });
        }
        let stored = header.checksum;
        let computed = compute_checksum(data);
        if strict && stored != computed {
            return Err(D2sError::ChecksumMismatch { stored, computed });
        }

        let quests = Quests::parse(data)?;
        let waypoints = Waypoints::parse(data)?;
        let npc = Npc::parse(data)?;

        // ---- Stats ----
        let mut reader = BitReader::new(data, 765 * 8);
        let stats = Stats::parse(&mut reader, header.version)?;
        reader.align();

        // ---- Skills ----
        let skills_offset = reader.byte_pos();
        let skills = Skills::parse(data, skills_offset)?;
        let mut reader = BitReader::new(data, (skills_offset + 32) * 8);

        // ---- Items ----
        let character_items = parse_item_list(&mut reader)?;
        let byte_pos = reader.byte_pos();

        // Liste corpse : un second "JM" immédiatement après.
        let corpse_items = if has_marker(data, byte_pos, b"JM") {
            Some(parse_item_list(&mut reader)?)
        } else {
            None
        };
        let byte_pos = reader.byte_pos();

        // Mercenaire : "jf" + liste (expansion).
        let merc_items = if has_marker(data, byte_pos, b"jf") {
            reader.read(16)?; // marqueur déjà vérifié
            let list = parse_item_list(&mut reader)?;
            Some(list)
        } else {
            None
        };
        let byte_pos = reader.byte_pos();

        // Golem de fer : "kf" + flag + liste éventuelle.
        let golem = if has_marker(data, byte_pos, b"kf") {
            reader.read(16)?; // marqueur déjà vérifié
            let has_golem = reader.read(8)? != 0;
            let items = if has_golem { parse_item_list(&mut reader)?.items } else { Vec::new() };
            Some(GolemTrailer { has_golem, items })
        } else {
            None
        };

        Ok(SaveFile {
            header,
            quests,
            waypoints,
            npc,
            stats,
            skills,
            character_items,
            corpse_items,
            merc_items,
            golem,
        })
    }

    /// Sérialise la sauvegarde complète : taille déclarée + checksum
    /// recalculés (l'écriture d'une save cohérente). La relecture des octets
    /// d'un fichier bien formé redonne les mêmes octets — cf. tests.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1200);

        // En-tête propre + sections fixes = 765 octets.
        let mut head = self.header.serialize().to_vec();
        head.extend_from_slice(self.quests.raw());
        head.extend_from_slice(self.waypoints.raw());
        head.extend_from_slice(self.npc.raw());
        debug_assert_eq!(head.len(), HEADER_SIZE);
        out.extend_from_slice(&head);

        // Sections variables bit-packées.
        let mut w = BitWriter::new();
        self.stats.serialize(&mut w);
        w.write_ascii("if");
        for i in 0..30 {
            w.write(self.skills.level(i) as u32, 8);
        }
        serialize_item_list(&self.character_items, &mut w);
        if let Some(corpse) = &self.corpse_items {
            serialize_item_list(corpse, &mut w);
        }
        if let Some(merc) = &self.merc_items {
            w.write_ascii("jf");
            serialize_item_list(merc, &mut w);
        }
        if let Some(g) = &self.golem {
            w.write_ascii("kf");
            w.write(g.has_golem as u32, 8);
            if g.has_golem {
                w.write_ascii("JM");
                w.write(g.items.len() as u32 & 0xFFFF, 16);
                for item in &g.items {
                    items::serialize_item(item, &mut w);
                    for child in &item.children {
                        items::serialize_item(child, &mut w);
                    }
                }
            }
        }
        out.extend_from_slice(&w.into_bytes());

        // Taille déclarée + checksum (champ neutralisé pendant le calcul).
        // `to_bytes` est pure : l'en-tête self n'est pas muté.
        let total = out.len() as u32;
        out[0x08..0x0C].copy_from_slice(&total.to_le_bytes());
        out[0x0C..0x10].copy_from_slice(&[0; 4]);
        let sum = compute_checksum(&out);
        out[0x0C..0x10].copy_from_slice(&sum.to_le_bytes());
        out
    }

    /// Checksum recalculé sur les octets sérialisés.
    pub fn computed_checksum(&self) -> u32 {
        compute_checksum(&self.to_bytes())
    }

    /// Le checksum stocké dans l'en-tête est-il valide pour ces octets ?
    /// (à appeler avec les octets D'ORIGINE, pas `to_bytes()`.)
    pub fn checksum_valid_for(&self, data: &[u8]) -> bool {
        data.len() >= HEADER_SIZE && self.header.checksum == compute_checksum(data)
    }

    /// Conversion vers la struct interne minimale — le pont documenté pour
    /// l'intégrateur suivant (d2game/d2wasm). PAS branché dans le save v3.
    pub fn to_summary(&self) -> CharacterSummary {
        let h = &self.header;
        CharacterSummary {
            name: h.name(),
            class: h.class,
            class_name: h.class.name(),
            level: h.level,
            hardcore: h.status.hardcore,
            died: h.status.died,
            expansion: h.status.expansion,
            ladder: h.status.ladder,
            progression: h.progression,
            strength: self.stats.value(0),
            energy: self.stats.value(1),
            dexterity: self.stats.value(2),
            vitality: self.stats.value(3),
            stat_points: self.stats.value(4),
            skill_points: self.stats.value(5),
            max_hp: self.stats.value(7),
            max_mana: self.stats.value(9),
            max_stamina: self.stats.value(11),
            experience: self.stats.value(13),
            gold: self.stats.value(14),
            gold_bank: self.stats.value(15),
            items: count_items(&self.character_items.items)
                + self.corpse_items.as_ref().map(|l| count_items(&l.items)).unwrap_or(0)
                + self.merc_items.as_ref().map(|l| count_items(&l.items)).unwrap_or(0)
                + self.golem.as_ref().map(|g| count_items(&g.items)).unwrap_or(0),
        }
    }
}

/// Struct interne minimale extraite d'un .d2s (mission §3 : « un test de
/// conversion d2s -> struct interne minimale, documenté pour l'intégrateur
/// suivant »). Les stats hp/mana/stamina sont déjà décalées (÷256).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CharacterSummary {
    pub name: String,
    pub class: Class,
    pub class_name: &'static str,
    pub level: u8,
    pub hardcore: bool,
    pub died: bool,
    pub expansion: bool,
    pub ladder: bool,
    pub progression: u8,
    pub strength: Option<u32>,
    pub energy: Option<u32>,
    pub dexterity: Option<u32>,
    pub vitality: Option<u32>,
    pub stat_points: Option<u32>,
    pub skill_points: Option<u32>,
    /// Stat `maxhp` rechargée (raw >> 8).
    pub max_hp: Option<u32>,
    /// Stat `maxmana` rechargée (raw >> 8).
    pub max_mana: Option<u32>,
    /// Stat `maxstamina` rechargée (raw >> 8).
    pub max_stamina: Option<u32>,
    pub experience: Option<u32>,
    pub gold: Option<u32>,
    pub gold_bank: Option<u32>,
    /// Items totaux (listes caractère + corpse + merc + golem, enfants
    /// sertis inclus).
    pub items: usize,
}

fn has_marker(data: &[u8], byte_pos: usize, marker: &[u8]) -> bool {
    data.len() >= byte_pos + marker.len() && &data[byte_pos..byte_pos + marker.len()] == marker
}

/// Parse une liste "JM" + count + items (avec leurs enfants sertis).
fn parse_item_list(reader: &mut BitReader) -> Result<ItemList, D2sError> {
    let start = reader.bit_pos();
    let marker = reader.read_ascii(2)?;
    if marker != "JM" {
        return Err(D2sError::BadMarker { expected: "JM".into(), got: marker, bit: start });
    }
    let declared_count = reader.read(16)? as u16;
    let mut items = Vec::with_capacity(declared_count.min(512) as usize);
    for _ in 0..declared_count {
        let mut item = items::parse_item(reader)?;
        for _ in 0..item.header.children_count {
            item.children.push(items::parse_item(reader)?);
        }
        items.push(item);
    }
    Ok(ItemList { declared_count, items })
}

/// Écrit une liste "JM" + count + items (+ enfants).
fn serialize_item_list(list: &ItemList, writer: &mut BitWriter) {
    writer.write_ascii("JM");
    writer.write(list.items.len() as u32 & 0xFFFF, 16);
    for item in &list.items {
        items::serialize_item(item, writer);
        for child in &item.children {
            items::serialize_item(child, writer);
        }
    }
}

/// Compte récursif (top-level + enfants sertis).
fn count_items(items: &[Item]) -> usize {
    items
        .iter()
        .map(|i| 1 + count_items(&i.children))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_v112_round_trips_through_bytes() {
        let mut save = SaveFile::default_v112();
        save.header.set_name("Sorchar");
        save.stats.set(0, 45);
        let bytes = save.to_bytes();
        let back = SaveFile::parse(&bytes).unwrap();
        assert_eq!(back.header.name(), "Sorchar");
        assert_eq!(back.stats.value(0), Some(45));
        assert!(back.quests.marker_ok() && back.waypoints.marker_ok() && back.npc.marker_ok());
        assert!(back.golem.as_ref().unwrap().has_golem == false);
    }

    #[test]
    fn summary_counts_items_recursively() {
        let mut save = SaveFile::default_v112();
        let charm = Item {
            header: ItemHeader { compact: true, ..ItemHeader::default() },
            extended: None,
            ear: None,
            children: Vec::new(),
        };
        let mut cap = Item {
            header: ItemHeader {
                type_raw: *b"cap ",
                socketed: true,
                children_count: 2,
                ..ItemHeader::default()
            },
            extended: Some(ItemExtended {
                file_id: 1,
                ilvl: 1,
                quality: Quality::Normal,
                graphic: None,
                class_specific: None,
                quality_data: QualityData::None,
                runeword: None,
                personalized_name: None,
                unknown_pre_type_bit: 0,
                type_data: TypeData::Armor {
                    defense_stored: 10,
                    max_durability: 12,
                    durability: 12,
                    sockets: Some(2),
                },
                set_lists: None,
                mods: Vec::new(),
            }),
            ear: None,
            children: vec![charm.clone(), charm],
        };
        cap.header.children_count = 2;
        save.character_items.items.push(cap);
        assert_eq!(count_items(&save.character_items.items), 3);
        assert_eq!(save.to_summary().items, 3);
    }
}
