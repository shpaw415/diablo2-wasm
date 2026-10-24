//! Items du .d2s — liste "JM", en-tête bit-packé 111 bits, bloc étendu,
//! qualité, prefix/suffix, runeword, personnalisation, données de type,
//! sertissage et mods (evidence.md §8).
//!
//! ## Divergences tranchées (notre evidence gagne — cf. lib.rs)
//! * **Type 76–107** : 4 ASCII (trevin + parseurs réels ; krisives : 4+24
//!   bits) — retenu 32 bits, prouvé par les vecteurs (`rin `, `cap `, `cm1 `).
//! * **Sockets** : 4 bits (trevin + vecteur socketed_armor ; krisives : 3
//!   bits = champ graphisme) — avec 3 bits la liste de mods qui suit du
//!   vecteur se lit faux.
//! * **Largeur des mods** : `Save Bits` seule de ItemStatCost v1.12 (table
//!   locale [VÉRIFIÉ-LOCAL]) ; krisives dit « Save Bits (+ Param Bits) ».
//!
//! Champs inconnus (`unk*`, bit 39…) conservés pour le round-trip
//! octet-à-octet ; la classification armure/arme vient des codes réels
//! v1.12 embarqués ([`crate::codes`]).

#![allow(dead_code)]

use crate::bits::{BitReader, BitWriter};
use crate::codes;
use crate::error::D2sError;
use crate::itemstatcost;

/// ID de fin de liste de mods.
pub const MOD_END_ID: u16 = 0x1FF;

/// Emplacement parent (bits 58–60).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Location {
    /// 0 — stocké (inventaire/cube/stash).
    #[default]
    Stored,
    /// 1 — équipé.
    Equipped,
    /// 2 — ceinture.
    Belt,
    /// 4 — curseur.
    Cursor,
    /// 6 — sertie dans un item.
    SocketedInItem,
    /// Autre valeur — conservée.
    Other(u8),
}

impl Location {
    pub fn from_u8(v: u8) -> Location {
        match v {
            0 => Location::Stored,
            1 => Location::Equipped,
            2 => Location::Belt,
            4 => Location::Cursor,
            6 => Location::SocketedInItem,
            other => Location::Other(other),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Location::Stored => 0,
            Location::Equipped => 1,
            Location::Belt => 2,
            Location::Cursor => 4,
            Location::SocketedInItem => 6,
            Location::Other(v) => v,
        }
    }

    /// Nom anglais canonique (d2s_parse.py).
    pub fn name(self) -> &'static str {
        match self {
            Location::Stored => "stored",
            Location::Equipped => "equipped",
            Location::Belt => "belt",
            Location::Cursor => "cursor",
            Location::SocketedInItem => "socketed",
            Location::Other(_) => "?",
        }
    }
}

/// Slot équipé (bits 61–64) — 1 casque … 11/12 swap (krisives + trevin).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct EquipSlot(pub u8);

impl EquipSlot {
    pub const HEAD: EquipSlot = EquipSlot(1);
    pub const NECK: EquipSlot = EquipSlot(2);
    pub const TORSO: EquipSlot = EquipSlot(3);
    pub const WEAPON_RIGHT: EquipSlot = EquipSlot(4);
    pub const WEAPON_LEFT: EquipSlot = EquipSlot(5);
    pub const RING_RIGHT: EquipSlot = EquipSlot(6);
    pub const RING_LEFT: EquipSlot = EquipSlot(7);
    pub const BELT: EquipSlot = EquipSlot(8);
    pub const FEET: EquipSlot = EquipSlot(9);
    pub const HANDS: EquipSlot = EquipSlot(10);
    pub const ALT_WEAPON_RIGHT: EquipSlot = EquipSlot(11);
    pub const ALT_WEAPON_LEFT: EquipSlot = EquipSlot(12);

    pub fn name(self) -> &'static str {
        match self.0 {
            1 => "head",
            2 => "neck",
            3 => "torso",
            4 => "weapon_right",
            5 => "weapon_left",
            6 => "ring_right",
            7 => "ring_left",
            8 => "belt",
            9 => "feet",
            10 => "hands",
            11 => "alt_weapon_right",
            12 => "alt_weapon_left",
            _ => "?",
        }
    }
}

/// Conteneur (bits 73–75).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Container {
    /// 1 — inventaire.
    #[default]
    Inventory,
    /// 4 — cube Horadrim.
    Cube,
    /// 5 — coffre (stash).
    Stash,
    /// Autre valeur — conservée.
    Other(u8),
}

impl Container {
    pub fn from_u8(v: u8) -> Container {
        match v {
            1 => Container::Inventory,
            4 => Container::Cube,
            5 => Container::Stash,
            other => Container::Other(other),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Container::Inventory => 1,
            Container::Cube => 4,
            Container::Stash => 5,
            Container::Other(v) => v,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Container::Inventory => "inventory",
            Container::Cube => "cube",
            Container::Stash => "stash",
            Container::Other(_) => "?",
        }
    }
}

/// Qualité (bits 150–153, bloc étendu).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Quality {
    /// 1 — low quality (Crude/Cracked/Damaged/Low Quality).
    Low,
    /// 2 — normal.
    #[default]
    Normal,
    /// 3 — supérieure.
    High,
    /// 4 — magique.
    Magic,
    /// 5 — set.
    Set,
    /// 6 — rare.
    Rare,
    /// 7 — unique.
    Unique,
    /// 8 — crafted.
    Crafted,
    /// Autre valeur — conservée.
    Other(u8),
}

impl Quality {
    pub fn from_u8(v: u8) -> Quality {
        match v {
            1 => Quality::Low,
            2 => Quality::Normal,
            3 => Quality::High,
            4 => Quality::Magic,
            5 => Quality::Set,
            6 => Quality::Rare,
            7 => Quality::Unique,
            8 => Quality::Crafted,
            other => Quality::Other(other),
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            Quality::Low => 1,
            Quality::Normal => 2,
            Quality::High => 3,
            Quality::Magic => 4,
            Quality::Set => 5,
            Quality::Rare => 6,
            Quality::Unique => 7,
            Quality::Crafted => 8,
            Quality::Other(v) => v,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Quality::Low => "low",
            Quality::Normal => "normal",
            Quality::High => "high",
            Quality::Magic => "magic",
            Quality::Set => "set",
            Quality::Rare => "rare",
            Quality::Unique => "unique",
            Quality::Crafted => "crafted",
            Quality::Other(_) => "?",
        }
    }
}

/// Données de qualité (evidence.md §8.4).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QualityData {
    #[default]
    None,
    /// quality 1 — 3 bits : 0 Crude, 1 Cracked, 2 Damaged, 3 Low Quality.
    Low(u8),
    /// quality 3 — 3 bits inconnus (superior).
    SuperiorUnknown(u8),
    /// quality 4 — prefix puis suffix (11 bits chacun, 0 = absent ;
    /// index dans magicprefix/magicsuffix).
    Magic { prefix: u16, suffix: u16 },
    /// quality 5 — ID du set (12 bits ; le membre est déduit du type).
    Set(u16),
    /// quality 6/8 — nom rare (2×8 bits, IDs rareprefix/raresuffix) + 6
    /// affixes optionnels (flag 1 bit + ID 11 bits).
    Rare {
        name_prefix_id: u8,
        name_suffix_id: u8,
        affixes: [Option<u16>; 6],
    },
    /// quality 7 — ID unique (12 bits).
    Unique(u16),
}

/// Données type-spécifiques (evidence.md §8.5) — classification par codes
/// réels armor.txt/weapons.txt v1.12 embarqués.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TypeData {
    /// Armure : défense stockée 10 bits (la base réelle est discutée —
    /// trevin « defense + 10 » ; valeur brute conservée) + durabilité
    /// max/actuelle + sockets si sertie.
    Armor {
        defense_stored: u16,
        max_durability: u8,
        durability: u8,
        sockets: Option<u8>,
    },
    /// Arme : durabilité max/actuelle + sockets si sertie.
    Weapon { max_durability: u8, durability: u8, sockets: Option<u8> },
    /// Pile (flèches, potions, clés, tomes) : quantité 9 bits ; les tomes
    /// portent 5 bits inconnus avant.
    Stack { tome_unknown: Option<u8>, quantity: u16 },
    /// Rien (misc : anneaux, amulettes, bijoux, charms…).
    #[default]
    None,
}

/// Un mod d'item : `(ID 9 bits, valeur)` — largeur = colonne `Save Bits` de
/// ItemStatCost v1.12 (paires min/max = largeur min + largeur max, ID
/// suivant). La valeur est signée ; `save_add`/`val_shift` sont exposés mais
/// **non appliqués** (sémantique à confirmer sur un vrai .d2s — evidence
/// §8.6 : « à confirmer », aligné sur le parseur de référence).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mod {
    pub id: u16,
    /// Largeur effective lue/écrite (paires min/max incluses).
    pub bits: u8,
    /// Valeur signée stockée dans le flux.
    pub value: i32,
    /// Colonne `Save Add` (exposée, non appliquée).
    pub save_add: u32,
    /// Colonne `ValShift` (exposée, non appliquée).
    pub val_shift: u8,
}

/// En-tête d'item — bits 0–111, tous les inconnus conservés.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemHeader {
    /// bit 20 — identifié.
    pub identified: bool,
    /// bit 27 — sertissable/sertie.
    pub socketed: bool,
    /// bit 29 — ramassé depuis la dernière save.
    pub fresh: bool,
    /// bit 32 — oreille de joueur.
    pub ear: bool,
    /// bit 33 — gear de départ.
    pub starter: bool,
    /// bit 37 — compact (l'item s'arrête à 111 bits).
    pub compact: bool,
    /// bit 38 — éthéré.
    pub ethereal: bool,
    /// bit 40 — personnalisé (par Anya).
    pub personalized: bool,
    /// bit 42 — runeword.
    pub runeword: bool,
    /// bits 58–60 — emplacement parent.
    pub location: Location,
    /// bits 61–64 — slot équipé.
    pub equip_slot: EquipSlot,
    /// bits 65–68 — colonne (inventaire 0–9, stash 0–5, cube 0–3).
    pub column: u8,
    /// bits 69–71 — rangée.
    pub row: u8,
    /// bits 73–75 — conteneur.
    pub container: Container,
    /// bits 76–107 — code type brut 4 octets (`amu `, `2hs `…). La découpe
    /// 4 ASCII de l'evidence est retenue (divergence krisives 4+24 tranchée).
    pub type_raw: [u8; 4],
    /// bits 108–110 — nb d'items enfants sertis qui suivent.
    pub children_count: u8,
    // --- inconnus conservés pour le round-trip ---
    /// bits 16–19.
    pub unk_16: u8,
    /// bits 21–26.
    pub unk_21: u8,
    /// bit 28.
    pub unk_28: u8,
    /// bits 30–31.
    pub unk_30: u8,
    /// bits 34–36.
    pub unk_34: u8,
    /// bit 39 — observé = 1 (trevin).
    pub unk_39: u8,
    /// bit 41.
    pub unk_41: u8,
    /// bits 43–57.
    pub unk_43: u16,
    /// bit 72.
    pub unk_72: u8,
}

impl Default for ItemHeader {
    fn default() -> Self {
        ItemHeader {
            identified: true,
            socketed: false,
            fresh: false,
            ear: false,
            starter: false,
            compact: false,
            ethereal: false,
            personalized: false,
            runeword: false,
            location: Location::Stored,
            equip_slot: EquipSlot(0),
            column: 0,
            row: 0,
            container: Container::Inventory,
            type_raw: *b"cm1 ",
            children_count: 0,
            unk_16: 0,
            unk_21: 0,
            unk_28: 0,
            unk_30: 0,
            unk_34: 0,
            unk_39: 1, // observé = 1
            unk_41: 0,
            unk_43: 0,
            unk_72: 0,
        }
    }
}

impl ItemHeader {
    /// Code 3 lettres (le champ brut, espaces/NUL de droite retirés).
    pub fn type_code(&self) -> String {
        String::from_utf8_lossy(&self.type_raw)
            .trim_end_matches(['\x00', ' '])
            .to_string()
    }
}

/// Oreille de joueur (bit 32) — classe, niveau, nom du joueur défunct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EarData {
    pub class: crate::header::Class,
    pub level: u8,
    pub name: String,
}

/// Bloc étendu d'un item (si !compact && !ear) — evidence.md §8.3–§8.6.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemExtended {
    /// bits 111–142 — ID unique anti-dupe.
    pub file_id: u32,
    /// bits 143–149 — item level.
    pub ilvl: u8,
    /// bits 150–153 — qualité.
    pub quality: Quality,
    /// graphisme alternatif (flag 1 bit + index 3 bits).
    pub graphic: Option<u8>,
    /// class-specific (flag 1 bit + 11 bits).
    pub class_specific: Option<u16>,
    /// données de qualité.
    pub quality_data: QualityData,
    /// runeword : (index 12 bits, inconnu 4 bits — =5 observé).
    pub runeword: Option<(u16, u8)>,
    /// personnalisation Anya (7 bits/char jusqu'à 0).
    pub personalized_name: Option<String>,
    /// bit inconnu avant les données de type (=1 sur les tomes, trevin).
    pub unknown_pre_type_bit: u8,
    /// données type-spécifiques.
    pub type_data: TypeData,
    /// items de set : nb de listes de mods (5 bits, 1 ou 3).
    pub set_lists: Option<u8>,
    /// mods magiques (ID 9 bits + valeur Save Bits, fin 0x1FF).
    pub mods: Vec<Mod>,
}

/// Un item complet : en-tête + étendu + enfants sertis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Item {
    pub header: ItemHeader,
    /// `None` si compact (bit 37) ou oreille (bit 32).
    pub extended: Option<ItemExtended>,
    /// Oreille : classe/niveau/nom (aucun bloc étendu).
    pub ear: Option<EarData>,
    /// Gemmes/runes/jewels insérés (structures immédiatement après).
    pub children: Vec<Item>,
}

impl Item {
    /// Code 3 lettres du type.
    pub fn type_code(&self) -> String {
        self.header.type_code()
    }

    /// Liste des mods (vide si compact/ear).
    pub fn mods(&self) -> &[Mod] {
        self.extended.as_ref().map(|e| e.mods.as_slice()).unwrap_or(&[])
    }
}

fn rd_type_data(reader: &mut BitReader, code: &str, socketed: bool) -> Result<TypeData, D2sError> {
    if codes::is_armor(code) {
        let defense_stored = reader.read(10)? as u16;
        let max_durability = reader.read(8)? as u8;
        let durability = reader.read(8)? as u8;
        let sockets = if socketed { Some(reader.read(4)? as u8) } else { None };
        Ok(TypeData::Armor { defense_stored, max_durability, durability, sockets })
    } else if codes::is_weapon(code) {
        let max_durability = reader.read(8)? as u8;
        let durability = reader.read(8)? as u8;
        let sockets = if socketed { Some(reader.read(4)? as u8) } else { None };
        Ok(TypeData::Weapon { max_durability, durability, sockets })
    } else if codes::is_stackable(code) {
        let tome_unknown = if codes::TOME_CODES.contains(&code) {
            Some(reader.read(5)? as u8)
        } else {
            None
        };
        let quantity = reader.read(9)? as u16;
        Ok(TypeData::Stack { tome_unknown, quantity })
    } else {
        // Codes hors armor/weapons/piles connus : aucun champ lu
        // (désynchronisation possible — limite assumée, cf. lib.rs).
        Ok(TypeData::None)
    }
}

fn wr_type_data(writer: &mut BitWriter, td: &TypeData) {
    match td {
        TypeData::Armor { defense_stored, max_durability, durability, sockets } => {
            writer.write(*defense_stored as u32, 10);
            writer.write(*max_durability as u32, 8);
            writer.write(*durability as u32, 8);
            if let Some(s) = sockets {
                writer.write(*s as u32, 4);
            }
        }
        TypeData::Weapon { max_durability, durability, sockets } => {
            writer.write(*max_durability as u32, 8);
            writer.write(*durability as u32, 8);
            if let Some(s) = sockets {
                writer.write(*s as u32, 4);
            }
        }
        TypeData::Stack { tome_unknown, quantity } => {
            if let Some(t) = tome_unknown {
                writer.write(*t as u32, 5);
            }
            writer.write(*quantity as u32, 9);
        }
        TypeData::None => {}
    }
}

/// Parse un item complet dans le flux de bits (evidence.md §8.2–§8.6 ;
/// miroir de `parse_item` de d2s_parse.py).
pub fn parse_item(reader: &mut BitReader) -> Result<Item, D2sError> {
    let start = reader.bit_pos();
    let marker = reader.read_ascii(2)?;
    if marker != "JM" {
        return Err(D2sError::BadMarker { expected: "JM".into(), got: marker, bit: start });
    }

    let unk_16 = reader.read(4)? as u8; // bits 16-19
    let identified = reader.read(1)? != 0; // bit 20
    let unk_21 = reader.read(6)? as u8; // bits 21-26
    let socketed = reader.read(1)? != 0; // bit 27
    let unk_28 = reader.read(1)? as u8; // bit 28
    let fresh = reader.read(1)? != 0; // bit 29
    let unk_30 = reader.read(2)? as u8; // bits 30-31
    let ear = reader.read(1)? != 0; // bit 32
    let starter = reader.read(1)? != 0; // bit 33
    let unk_34 = reader.read(3)? as u8; // bits 34-36
    let compact = reader.read(1)? != 0; // bit 37
    let ethereal = reader.read(1)? != 0; // bit 38
    let unk_39 = reader.read(1)? as u8; // bit 39
    let personalized = reader.read(1)? != 0; // bit 40
    let unk_41 = reader.read(1)? as u8; // bit 41
    let runeword = reader.read(1)? != 0; // bit 42
    let unk_43 = reader.read(15)? as u16; // bits 43-57
    let location = Location::from_u8(reader.read(3)? as u8); // bits 58-60
    let equip_slot = EquipSlot(reader.read(4)? as u8); // bits 61-64
    let column = reader.read(4)? as u8; // bits 65-68
    let row = reader.read(3)? as u8; // bits 69-71
    let unk_72 = reader.read(1)? as u8; // bit 72
    let container = Container::from_u8(reader.read(3)? as u8); // bits 73-75
    let mut type_raw = [0u8; 4];
    let type_str = reader.read_ascii(4)?; // bits 76-107
    type_raw.copy_from_slice(type_str.as_bytes());
    let children_count = reader.read(3)? as u8; // bits 108-110

    let header = ItemHeader {
        identified,
        socketed,
        fresh,
        ear,
        starter,
        compact,
        ethereal,
        personalized,
        runeword,
        location,
        equip_slot,
        column,
        row,
        container,
        type_raw,
        children_count,
        unk_16,
        unk_21,
        unk_28,
        unk_30,
        unk_34,
        unk_39,
        unk_41,
        unk_43,
        unk_72,
    };

    // Oreille : classe 3 bits, niveau 7 bits, nom 7 bits/char, align.
    if ear {
        let cls = reader.read(3)?;
        let level = reader.read(7)? as u8;
        let (name, _) = reader.read_name_chars()?;
        reader.align();
        return Ok(Item {
            header,
            extended: None,
            ear: Some(EarData {
                class: crate::header::Class::from_u8(cls as u8),
                level,
                name,
            }),
            children: Vec::new(),
        });
    }

    // Compact : fin à 111 bits.
    if compact {
        reader.align();
        return Ok(Item { header, extended: None, ear: None, children: Vec::new() });
    }

    // ---- Bloc étendu (evidence §8.3) ----
    let file_id = reader.read(32)?;
    let ilvl = reader.read(7)? as u8;
    let quality_raw = reader.read(4)? as u8;
    let quality = Quality::from_u8(quality_raw);
    let graphic = if reader.read(1)? != 0 { Some(reader.read(3)? as u8) } else { None };
    let class_specific = if reader.read(1)? != 0 { Some(reader.read(11)? as u16) } else { None };

    let quality_data = match quality_raw {
        1 => QualityData::Low(reader.read(3)? as u8),
        3 => QualityData::SuperiorUnknown(reader.read(3)? as u8),
        4 => QualityData::Magic {
            prefix: reader.read(11)? as u16,
            suffix: reader.read(11)? as u16,
        },
        5 => QualityData::Set(reader.read(12)? as u16),
        6 | 8 => {
            let name_prefix_id = reader.read(8)? as u8;
            let name_suffix_id = reader.read(8)? as u8;
            let mut affixes: [Option<u16>; 6] = [None; 6];
            for slot in affixes.iter_mut() {
                if reader.read(1)? != 0 {
                    *slot = Some(reader.read(11)? as u16);
                }
            }
            QualityData::Rare { name_prefix_id, name_suffix_id, affixes }
        }
        7 => QualityData::Unique(reader.read(12)? as u16),
        _ => QualityData::None,
    };

    let runeword_data = if runeword {
        Some((reader.read(12)? as u16, reader.read(4)? as u8))
    } else {
        None
    };

    let personalized_name = if personalized {
        Some(reader.read_name_chars()?.0)
    } else {
        None
    };

    let unknown_pre_type_bit = reader.read(1)? as u8;

    let code = header.type_code();
    let type_data = rd_type_data(reader, &code, socketed)?;

    // Items de set : 5 bits = nb de listes de mods (après les données type).
    let set_lists = if quality == Quality::Set { Some(reader.read(5)? as u8) } else { None };

    let mods = parse_mod_list(reader)?;

    reader.align(); // padding de fin d'item — obligatoire pour l'alignement
                    // de la structure suivante (trevin, implémenté et testé
                    // dans le parseur de référence)

    Ok(Item {
        header,
        extended: Some(ItemExtended {
            file_id,
            ilvl,
            quality,
            graphic,
            class_specific,
            quality_data,
            runeword: runeword_data,
            personalized_name,
            unknown_pre_type_bit,
            type_data,
            set_lists,
            mods,
        }),
        ear: None,
        children: Vec::new(),
    })
}

/// Sérialise un item (réciproque exacte de [`parse_item`]).
pub fn serialize_item(item: &Item, writer: &mut BitWriter) {
    let h = &item.header;
    writer.write_ascii("JM"); // 0-15
    writer.write(h.unk_16 as u32, 4); // 16-19
    writer.write(h.identified as u32, 1); // 20
    writer.write(h.unk_21 as u32, 6); // 21-26
    writer.write(h.socketed as u32, 1); // 27
    writer.write(h.unk_28 as u32, 1); // 28
    writer.write(h.fresh as u32, 1); // 29
    writer.write(h.unk_30 as u32, 2); // 30-31
    writer.write(h.ear as u32, 1); // 32
    writer.write(h.starter as u32, 1); // 33
    writer.write(h.unk_34 as u32, 3); // 34-36
    writer.write(h.compact as u32, 1); // 37
    writer.write(h.ethereal as u32, 1); // 38
    writer.write(h.unk_39 as u32, 1); // 39
    writer.write(h.personalized as u32, 1); // 40
    writer.write(h.unk_41 as u32, 1); // 41
    writer.write(h.runeword as u32, 1); // 42
    writer.write(h.unk_43 as u32, 15); // 43-57
    writer.write(h.location.to_u8() as u32, 3); // 58-60
    writer.write(h.equip_slot.0 as u32, 4); // 61-64
    writer.write(h.column as u32, 4); // 65-68
    writer.write(h.row as u32, 3); // 69-71
    writer.write(h.unk_72 as u32, 1); // 72
    writer.write(h.container.to_u8() as u32, 3); // 73-75
    writer.write_ascii(&String::from_utf8_lossy(&h.type_raw)); // 76-107
    writer.write(h.children_count as u32, 3); // 108-110

    if h.ear {
        if let Some(ear) = &item.ear {
            writer.write(ear.class.to_u8() as u32, 3);
            writer.write(ear.level as u32, 7);
            writer.write_name_chars(&ear.name);
        }
        writer.align();
        return;
    }
    if h.compact {
        writer.align();
        return;
    }

    let ext = item.extended.as_ref().expect("item non compact sans bloc étendu");
    writer.write(ext.file_id, 32);
    writer.write(ext.ilvl as u32, 7);
    writer.write(ext.quality.to_u8() as u32, 4);
    if let Some(g) = ext.graphic {
        writer.write(1, 1);
        writer.write(g as u32, 3);
    } else {
        writer.write(0, 1);
    }
    if let Some(cs) = ext.class_specific {
        writer.write(1, 1);
        writer.write(cs as u32, 11);
    } else {
        writer.write(0, 1);
    }

    match &ext.quality_data {
        QualityData::None => {}
        QualityData::Low(v) => writer.write(*v as u32, 3),
        QualityData::SuperiorUnknown(v) => writer.write(*v as u32, 3),
        QualityData::Magic { prefix, suffix } => {
            writer.write(*prefix as u32, 11);
            writer.write(*suffix as u32, 11);
        }
        QualityData::Set(id) => writer.write(*id as u32, 12),
        QualityData::Rare { name_prefix_id, name_suffix_id, affixes } => {
            writer.write(*name_prefix_id as u32, 8);
            writer.write(*name_suffix_id as u32, 8);
            for slot in affixes {
                match slot {
                    Some(id) => {
                        writer.write(1, 1);
                        writer.write(*id as u32, 11);
                    }
                    None => writer.write(0, 1),
                }
            }
        }
        QualityData::Unique(id) => writer.write(*id as u32, 12),
    }

    if let Some((idx, unk)) = ext.runeword {
        writer.write(idx as u32, 12);
        writer.write(unk as u32, 4);
    }
    if let Some(name) = &ext.personalized_name {
        writer.write_name_chars(name);
    }
    writer.write(ext.unknown_pre_type_bit as u32, 1);
    wr_type_data(writer, &ext.type_data);
    if let Some(lists) = ext.set_lists {
        writer.write(lists as u32, 5);
    }
    serialize_mod_list(&ext.mods, writer);
    writer.align();
}

/// Parse une liste de mods : `(ID 9 bits, valeur Save Bits[+max])` jusqu'à
/// `0x1FF` (evidence.md §8.6 — largeurs [VÉRIFIÉ-LOCAL] depuis ItemStatCost).
pub fn parse_mod_list(reader: &mut BitReader) -> Result<Vec<Mod>, D2sError> {
    let table = itemstatcost::embedded();
    let mut mods = Vec::new();
    loop {
        let start = reader.bit_pos();
        let id = reader.read(9)?;
        if id == MOD_END_ID as u32 {
            return Ok(mods);
        }
        let (bits, save_add, val_shift) =
            table.mod_width(id as u16).ok_or(D2sError::UnknownModWidth {
                id: id as u16,
                bit: start,
            })?;
        let value = reader.read_signed(bits as usize)?;
        mods.push(Mod { id: id as u16, bits, value, save_add, val_shift });
    }
}

/// Sérialise une liste de mods (0x1FF final, sans alignement — l'appelant
/// aligne après, comme le parseur).
pub fn serialize_mod_list(mods: &[Mod], writer: &mut BitWriter) {
    for m in mods {
        writer.write(m.id as u32, 9);
        writer.write_signed(m.value, m.bits as usize);
    }
    writer.write(MOD_END_ID as u32, 9);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Item compact minimal (charme) — 111 bits + align.
    #[test]
    fn compact_charm_round_trip() {
        let item = Item {
            header: ItemHeader::default(), // compact = false par défaut !
            extended: None,
            ear: None,
            children: Vec::new(),
        };
        // on force compact comme le générateur de vecteurs
        let mut item = item;
        item.header.compact = true;
        item.header.identified = true;

        let mut w = BitWriter::new();
        serialize_item(&item, &mut w);
        w.align();
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 14); // 111 bits -> 14 octets
        let mut r = BitReader::new(&bytes, 0);
        let back = parse_item(&mut r).unwrap();
        assert_eq!(back, item);
        assert_eq!(back.type_code(), "cm1");
        assert!(back.extended.is_none());
    }

    /// Anneau magic — mêmes champs que le vecteur magic_item.d2s.
    #[test]
    fn magic_ring_round_trip() {
        let item = Item {
            header: ItemHeader { type_raw: *b"rin ", ..ItemHeader::default() },
            extended: Some(ItemExtended {
                file_id: 0xDEAD_BEEF,
                ilvl: 45,
                quality: Quality::Magic,
                graphic: None,
                class_specific: None,
                quality_data: QualityData::Magic { prefix: 5, suffix: 3 },
                runeword: None,
                personalized_name: None,
                unknown_pre_type_bit: 0,
                type_data: TypeData::None, // "rin" hors armor/weapons/piles
                set_lists: None,
                mods: vec![
                    Mod { id: 9, bits: 8, value: 100, save_add: 32, val_shift: 8 },
                    Mod { id: 0, bits: 8, value: 60, save_add: 32, val_shift: 0 },
                ],
            }),
            ear: None,
            children: Vec::new(),
        };
        let mut w = BitWriter::new();
        serialize_item(&item, &mut w);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        let back = parse_item(&mut r).unwrap();
        assert_eq!(back, item);
        let ext = back.extended.unwrap();
        assert_eq!(
            ext.quality_data,
            QualityData::Magic { prefix: 5, suffix: 3 }
        );
        assert_eq!(ext.mods.len(), 2);
        assert_eq!(ext.mods[0].id, 9);
        assert_eq!(ext.mods[0].value, 100);
        assert_eq!(ext.mods[1].id, 0);
        assert_eq!(ext.mods[1].value, 60);
        assert_eq!(r.bit_pos() % 8, 0, "item aligné sur l'octet");
    }

    /// Casque sertis avec gemmes — mêmes champs que socketed_armor.d2s.
    #[test]
    fn socketed_cap_with_children_round_trip() {
        let charm = Item {
            header: ItemHeader { compact: true, ..ItemHeader::default() },
            extended: None,
            ear: None,
            children: Vec::new(),
        };
        let cap = Item {
            header: ItemHeader {
                type_raw: *b"cap ",
                socketed: true,
                children_count: 2,
                ..ItemHeader::default()
            },
            extended: Some(ItemExtended {
                file_id: 0xCAFE_D00D,
                ilvl: 20,
                quality: Quality::Normal,
                graphic: None,
                class_specific: None,
                quality_data: QualityData::None,
                runeword: None,
                personalized_name: None,
                unknown_pre_type_bit: 0,
                type_data: TypeData::Armor {
                    defense_stored: 33,
                    max_durability: 22,
                    durability: 22,
                    sockets: Some(2),
                },
                set_lists: None,
                mods: Vec::new(),
            }),
            ear: None,
            children: vec![charm.clone(), charm],
        };
        let mut w = BitWriter::new();
        serialize_item(&cap, &mut w);
        for c in &cap.children {
            serialize_item(c, &mut w);
        }
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        let back = parse_item(&mut r).unwrap();
        let kid1 = parse_item(&mut r).unwrap();
        let kid2 = parse_item(&mut r).unwrap();
        assert_eq!(back.header.type_code(), "cap");
        assert_eq!(back.header.children_count, 2);
        match &back.extended.as_ref().unwrap().type_data {
            TypeData::Armor { defense_stored, durability, sockets, .. } => {
                assert_eq!(*defense_stored, 33);
                assert_eq!(*durability, 22);
                assert_eq!(*sockets, Some(2));
            }
            other => panic!("type_data inattendu : {:?}", other),
        }
        assert_eq!(kid1, charm_original());
        assert_eq!(kid2, charm_original());
    }

    fn charm_original() -> Item {
        Item {
            header: ItemHeader { compact: true, ..ItemHeader::default() },
            extended: None,
            ear: None,
            children: Vec::new(),
        }
    }

    #[test]
    fn ear_round_trip() {
        let ear = Item {
            header: ItemHeader { ear: true, ..ItemHeader::default() },
            extended: None,
            ear: Some(EarData {
                class: crate::header::Class::Barbarian,
                level: 42,
                name: "Conan".into(),
            }),
            children: Vec::new(),
        };
        let mut w = BitWriter::new();
        serialize_item(&ear, &mut w);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        let back = parse_item(&mut r).unwrap();
        assert_eq!(back.ear.as_ref().unwrap().name, "Conan");
        assert_eq!(back.ear.as_ref().unwrap().level, 42);
        assert_eq!(back, ear);
    }

    #[test]
    fn rare_quality_round_trip() {
        let item = Item {
            header: ItemHeader { type_raw: *b"cap ", ..ItemHeader::default() },
            extended: Some(ItemExtended {
                file_id: 1,
                ilvl: 60,
                quality: Quality::Rare,
                graphic: None,
                class_specific: None,
                quality_data: QualityData::Rare {
                    name_prefix_id: 33,
                    name_suffix_id: 77,
                    affixes: [Some(4), None, Some(511), None, None, None],
                },
                runeword: None,
                personalized_name: None,
                unknown_pre_type_bit: 0,
                type_data: TypeData::Armor {
                    defense_stored: 100,
                    max_durability: 24,
                    durability: 24,
                    sockets: None,
                },
                set_lists: None,
                mods: Vec::new(),
            }),
            ear: None,
            children: Vec::new(),
        };
        let mut w = BitWriter::new();
        serialize_item(&item, &mut w);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        assert_eq!(parse_item(&mut r).unwrap(), item);
    }

    #[test]
    fn bad_marker_is_an_error() {
        let mut w = BitWriter::new();
        w.write_ascii("XX");
        w.write(0, 16);
        let bytes = w.into_bytes();
        let mut r = BitReader::new(&bytes, 0);
        assert!(matches!(
            parse_item(&mut r),
            Err(D2sError::BadMarker { expected, got, .. })
                if expected == "JM" && got == "XX"
        ));
    }
}
