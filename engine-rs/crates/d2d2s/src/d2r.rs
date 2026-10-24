//! Items des sauvegardes **D2R (v0x61 = 97)** — layout validé sur le fichier
//! réel Javazon.d2s (2 838 o, SHA-256 0361b4ad…5dfc2, checksum valide) et
//! croisé avec trois parseurs D2R indépendants (fredimachado/Diablo2R,
//! shadowevil/D2SaveLibrary, simshi/d2savior).
//!
//! # Différences avec le layout 0x60 (items.rs)
//! * **PAS de marqueur « JM » par item** — un seul « JM » + u16 de compte en
//!   tête de liste ; le compte peut être périmé (fichiers édités) : les items
//!   se lisent en continu jusqu'au marqueur suivant (« JM » sain, « jf »,
//!   « kf ») ou la fin. [Le réel : count=50 périmé, 76 items réels.]
//! * En-tête = 32 bits de drapeaux bruts : identified@4, quantity@5,
//!   socketed@11, ear@16, newbie@17, simple/compact@21, ethereal@22,
//!   personalized@24, runeword@26, chronicle@28-29.
//! * Version d'item = 1 bit « haut » + 2 bits (v0x60 : champ inconnu).
//! * Emplacement : mode 3 bits (+ pos 4, col 4, row 4, stored 3 brut ; modes
//!   3/5 = or étendu, col/row 16+16).
//! * **Code type encodé en HUFFMAN** (toujours 4 caractères, table 37
//!   symboles) — c'est lui qui rend le layout 0x60 illisible sur du D2R.
//! * Étendu : gems 3, guid 32, ilvl 7, quality 4, gfx 1+3, class 1+11,
//!   données de qualité (rare/crafted = noms 8+8 + 6×(1+11)), runeword
//!   **12+4** (ID stocké = numéro « RunewordNN » + 26 ; ex. 59 = Enigma),
//!   perso 7 bits/char, realm 1+(4×32), def 11, maxdur 8 (+dur 9 si max>0),
//!   quantité 9 (stackable), sockets 4, set 5×1 bit, liste de mods
//!   principale, listes de set, liste de mods du runeword, chronicle.
//! * Mod = (ID 9 bits, [param `Save Param Bits`], valeur `Save Bits` +
//!   bits des stats appariées [`ItemStatCost::PAIRED`]).
//!
//! Le round-trip du fichier réel est octet-à-octet (test real_javazon).

#![allow(dead_code)]

use crate::bits::{BitReader, BitWriter};
use crate::codes;
use crate::error::D2sError;
use crate::itemstatcost::ItemStatCost;

/// Table Huffman des codes d'item D2R (37 symboles, ordre de D2SaveLibrary).
const HUFF: [(&str, &str); 37] = [
    (" ", "10"),
    ("0", "11111011"),
    ("1", "1111100"),
    ("2", "001100"),
    ("3", "1101101"),
    ("4", "11111010"),
    ("5", "00010110"),
    ("6", "1101111"),
    ("7", "01111"),
    ("8", "000100"),
    ("9", "01110"),
    ("a", "11110"),
    ("b", "0101"),
    ("c", "01000"),
    ("d", "110001"),
    ("e", "110000"),
    ("f", "010011"),
    ("g", "11010"),
    ("h", "00011"),
    ("i", "1111110"),
    ("j", "000101110"),
    ("k", "010010"),
    ("l", "11101"),
    ("m", "01101"),
    ("n", "001101"),
    ("o", "1111111"),
    ("p", "11001"),
    ("q", "11011001"),
    ("r", "11100"),
    ("s", "0010"),
    ("t", "01100"),
    ("u", "00001"),
    ("v", "1101110"),
    ("w", "00000"),
    ("x", "00111"),
    ("y", "0001010"),
    ("z", "11011000"),
];

/// Décode un caractère Huffman depuis `reader`.
fn huffman_decode(reader: &mut BitReader) -> Result<char, D2sError> {
    let mut code = String::new();
    loop {
        let bit = reader.read(1)?;
        code.push(if bit != 0 { '1' } else { '0' });
        if let Some(&(c, _)) = HUFF.iter().find(|(_, v)| *v == code) {
            return Ok(c.chars().next().unwrap());
        }
        if code.len() > 9 {
            return Err(D2sError::BadMarker {
                expected: "code Huffman d'item".into(),
                got: code,
                bit: reader.bit_pos(),
            });
        }
    }
}

/// Encode un caractère en Huffman (None si symbole absent).
fn huffman_encode(c: char) -> Option<&'static str> {
    HUFF.iter().find(|(s, _)| *s == c.to_string()).map(|(_, v)| *v)
}

/// Drapeaux bruts d'un item (32 bits, conservés en entier pour le
/// round-trip ; accesseurs typés sur les bits validés).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(pub u32);

impl Flags {
    pub fn identified(self) -> bool { self.0 & 0x10 != 0 }
    pub fn has_quantity(self) -> bool { self.0 & 0x20 != 0 }
    pub fn socketed(self) -> bool { self.0 & 0x800 != 0 }
    pub fn ear(self) -> bool { self.0 & 0x1_0000 != 0 }
    pub fn newbie(self) -> bool { self.0 & 0x2_0000 != 0 }
    pub fn simple(self) -> bool { self.0 & 0x20_0000 != 0 }
    pub fn ethereal(self) -> bool { self.0 & 0x40_0000 != 0 }
    pub fn personalized(self) -> bool { self.0 & 0x100_0000 != 0 }
    pub fn runeword(self) -> bool { self.0 & 0x400_0000 != 0 }
    pub fn chronicle(self) -> bool { self.0 & 0x1000_0000 != 0 }
    pub fn chronicle_compact(self) -> bool { self.0 & 0x2000_0000 != 0 }
}

/// Données de qualité (bits bruts conservés, sémantique validée).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum QualityD2R {
    /// quality 2 (normal) — rien à lire.
    #[default]
    None,
    /// quality 1 — 3 bits (Cracked/Crude/…).
    Low(u8),
    /// quality 3 — 3 bits (superior).
    Superior(u8),
    /// quality 4 — préfixe puis suffixe (11 bits chacun, 0 = absent).
    Magic { prefix: u16, suffix: u16 },
    /// quality 5 — ID de set 12 bits (le membre réel = ID + 1 dans les
    /// tables, cf. VALIDATION RÉELLE).
    Set(u16),
    /// quality 6/8 — noms 8+8 puis 6×(1 bit + 11 bits).
    Rare {
        name1: u8,
        name2: u8,
        affixes: [Option<u16>; 6],
    },
    /// quality 7 — ID unique 12 bits (membre réel = ID + 1 ; ex. 275 →
    /// ligne 276 = Raven Frost).
    Unique(u16),
}

/// Un mod d'item D2R : ID 9 bits + param (`Save Param Bits`, souvent 0) +
/// valeur (`Save Bits` + bits appariés). `raw` est conservé pour le
/// round-trip ; `value` = raw − Save Add (confort d'affichage).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModD2R {
    pub id: u16,
    /// Couche param (`Save Param Bits`) — ex. skill 54 pour « +1 Téléport ».
    pub param: u32,
    /// Valeur brute lue (param et appariés inclus, round-trip).
    pub raw: u32,
    /// Valeur rechargée ≈ raw − Save Add.
    pub value: i64,
}

/// Bloc « chronicle » (drapeaux 28-29) — bits bruts conservés ; découpage
/// d2savior : ID 16 bits, puis 64 si compact, sinon 32+4+N×64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chronicle {
    /// Bits bruts (LSB-first, bit-packed) — réémis tels quels.
    pub bits: Vec<u8>,
    /// Nb de bits utiles dans [`bits`].
    pub bit_len: usize,
}

/// Item D2R complet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemD2R {
    /// 32 bits de drapeaux bruts.
    pub flags: Flags,
    /// Bit « haut » de la version d'item.
    pub version_high: u8,
    /// 2 bits bas de la version d'item.
    pub version_low: u8,
    /// Mode 3 bits (0 stocké, 1 équipé, 2 ceinture, 3/5 = or étendu).
    pub mode: u8,
    /// Slot équipé 4 bits.
    pub pos: u8,
    /// Colonne 4 bits.
    pub col: u8,
    /// Rangée 4 bits.
    pub row: u8,
    /// Conteneur 3 bits **brut** (convention D2R : valeur − 1).
    pub stored_raw: u8,
    /// Oreille : (classe, niveau, nom).
    pub ear: Option<(u8, u8, String)>,
    /// Code type 3-4 lettres (Huffman, espace final retiré).
    pub code: String,
    /// Étendu — None si simple (bit 21, hors realm) ou oreille.
    pub extended: Option<Box<ExtD2R>>,
}

/// Bloc étendu d'un item D2R.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtD2R {
    /// Nb d'items enfants sertis qui suivent (3 bits).
    pub gems: u8,
    /// GUID 32 bits.
    pub guid: u32,
    /// Item level 7 bits.
    pub ilvl: u8,
    /// Qualité 4 bits brute.
    pub quality_raw: u8,
    /// Graphisme 1+3.
    pub var_gfx: Option<u8>,
    /// Class-specific 1+11.
    pub class_specific: Option<u16>,
    /// Données de qualité.
    pub quality_data: QualityD2R,
    /// Runeword (ID 12 bits + inconnu 4 bits).
    pub runeword: Option<(u16, u8)>,
    /// Nom de personnalisation (7 bits/char jusqu'à 0).
    pub personalized: Option<String>,
    /// Realm data : 1 bit + 4×32 (v0x61).
    pub realm: Option<[u32; 4]>,
    /// Défense brute 11 bits (armure ; affichée = brute − 10).
    pub defense: Option<u16>,
    /// Durabilité max 8 bits (0 = indestructible).
    pub max_durability: Option<u8>,
    /// Durabilité courante 9 bits (si max > 0).
    pub durability: Option<u16>,
    /// Quantité 9 bits (pile).
    pub quantity: Option<u16>,
    /// Nb de sockets 4 bits (si drapeau socketed).
    pub sockets: Option<u8>,
    /// Drapeaux de listes de set (5×1 bit, quality 5).
    pub set_flags: Option<[bool; 5]>,
    /// Liste de mods principale (terminée par 0x1FF).
    pub mods: Vec<ModD2R>,
    /// Listes de set supplémentaires (une par drapeau actif, dans l'ordre).
    pub set_lists: Vec<Vec<ModD2R>>,
    /// Liste de mods du runeword (si drapeau runeword).
    pub rw_mods: Option<Vec<ModD2R>>,
    /// Bloc chronicle brut (drapeaux 28-29).
    pub chronicle: Option<Chronicle>,
}

fn read_mods(reader: &mut BitReader, table: &ItemStatCost) -> Result<Vec<ModD2R>, D2sError> {
    let mut mods = Vec::new();
    loop {
        let id = reader.read(9)?;
        if id >= crate::stats::END_ID as u32 {
            return Ok(mods);
        }
        let (spb, vbits, sa) = table
            .mod_shape(id as u16)
            .map(|(spb, vbits, sa, _vs)| (spb, vbits, sa))
            .map_err(|_| D2sError::UnknownModWidth { id: id as u16, bit: reader.bit_pos() - 9 })?;
        let param = if spb > 0 { reader.read(spb as usize)? } else { 0 };
        let raw = reader.read(vbits as usize)?;
        mods.push(ModD2R { id: id as u16, param, raw, value: raw as i64 - sa as i64 });
    }
}

fn write_mods(mods: &[ModD2R], writer: &mut BitWriter, table: &ItemStatCost) {
    for m in mods {
        writer.write(m.id as u32, 9);
        if let Ok((spb, vbits, _sa)) = table.mod_shape(m.id).map(|(a, b, c, _)| (a, b, c)) {
            if spb > 0 {
                writer.write(m.param & crate::bits::mask(spb as usize), spb as usize);
            }
            writer.write(m.raw & crate::bits::mask(vbits as usize), vbits as usize);
        }
    }
    writer.write(crate::stats::END_ID as u32, 9);
}

fn read_name(reader: &mut BitReader) -> Result<String, D2sError> {
    let mut name = String::new();
    loop {
        let c = reader.read(7)?;
        if c == 0 {
            return Ok(name);
        }
        name.push(c as u8 as char);
    }
}

fn write_name(name: &str, writer: &mut BitWriter) {
    for c in name.chars() {
        writer.write(c as u32 & 0x7F, 7);
    }
    writer.write(0, 7);
}

/// Capture `nbits` bits bruts dans un `BitWriter` (pour les zones
/// conservées sans sémantique : chronicle).
fn capture_bits(reader: &mut BitReader, nbits: usize, out: &mut BitWriter) -> Result<(), D2sError> {
    for _ in 0..nbits {
        let b = reader.read(1)?;
        out.write(b, 1);
    }
    Ok(())
}

/// Parse un item D2R (table embarquée).
pub fn parse_item(reader: &mut BitReader) -> Result<ItemD2R, D2sError> {
    parse_item_with(reader, &ItemStatCost::embedded())
}

/// Parse un item D2R avec une table ItemStatCost explicite.
pub fn parse_item_with(
    reader: &mut BitReader,
    table: &ItemStatCost,
) -> Result<ItemD2R, D2sError> {
    let flags = Flags(reader.read(32)?);
    let version_high = reader.read(1)? as u8;
    let version_low = reader.read(2)? as u8;
    let mode = reader.read(3)? as u8;
    let (pos, col, row, stored_raw) = if mode == 3 || mode == 5 {
        let _col16 = reader.read(16)?;
        let _row16 = reader.read(16)?;
        (0u8, 0u8, 0u8, 0u8)
    } else {
        let pos = reader.read(4)? as u8;
        let col = reader.read(4)? as u8;
        let row = reader.read(4)? as u8;
        let stored = reader.read(3)? as u8;
        (pos, col, row, stored)
    };

    if flags.ear() {
        let cls = reader.read(3)? as u8;
        let level = reader.read(7)? as u8;
        let name = read_name(reader)?;
        reader.align();
        return Ok(ItemD2R {
            flags,
            version_high,
            version_low,
            mode,
            pos,
            col,
            row,
            stored_raw,
            ear: Some((cls, level, name)),
            code: "ear".into(),
            extended: None,
        });
    }

    // Code type : 4 caractères Huffman (le 4e = espace pour les codes 3 lettres).
    let mut code = String::new();
    for _ in 0..4 {
        let c = huffman_decode(reader)?;
        if c != ' ' {
            code.push(c);
        }
    }

    if flags.simple() {
        // compact : drapeau realm (v0x61 : 4×32) puis alignement.
        let realm = if reader.read(1)? != 0 {
            Some([
                reader.read(32)?,
                reader.read(32)?,
                reader.read(32)?,
                reader.read(32)?,
            ])
        } else {
            None
        };
        reader.align();
        return Ok(ItemD2R {
            flags,
            version_high,
            version_low,
            mode,
            pos,
            col,
            row,
            stored_raw,
            ear: None,
            code,
            extended: realm.map(|realm| {
                Box::new(ExtD2R {
                    gems: 0,
                    guid: 0,
                    ilvl: 0,
                    quality_raw: 0,
                    var_gfx: None,
                    class_specific: None,
                    quality_data: QualityD2R::None,
                    runeword: None,
                    personalized: None,
                    realm: Some(realm),
                    defense: None,
                    max_durability: None,
                    durability: None,
                    quantity: None,
                    sockets: None,
                    set_flags: None,
                    mods: Vec::new(),
                    set_lists: Vec::new(),
                    rw_mods: None,
                    chronicle: None,
                })
            }),
        });
    }

    let gems = reader.read(3)? as u8;
    let guid = reader.read(32)?;
    let ilvl = reader.read(7)? as u8;
    let quality_raw = reader.read(4)? as u8;
    let var_gfx = if reader.read(1)? != 0 { Some(reader.read(3)? as u8) } else { None };
    let class_specific =
        if reader.read(1)? != 0 { Some(reader.read(11)? as u16) } else { None };

    let quality_data = match quality_raw {
        1 => QualityD2R::Low(reader.read(3)? as u8),
        3 => QualityD2R::Superior(reader.read(3)? as u8),
        4 => {
            let prefix = reader.read(11)? as u16;
            let suffix = reader.read(11)? as u16;
            QualityD2R::Magic { prefix, suffix }
        }
        5 => QualityD2R::Set(reader.read(12)? as u16),
        6 | 8 => {
            let name1 = reader.read(8)? as u8;
            let name2 = reader.read(8)? as u8;
            let mut affixes = [None; 6];
            for slot in affixes.iter_mut() {
                if reader.read(1)? != 0 {
                    *slot = Some(reader.read(11)? as u16);
                }
            }
            QualityD2R::Rare { name1, name2, affixes }
        }
        7 => QualityD2R::Unique(reader.read(12)? as u16),
        _ => QualityD2R::None,
    };

    let runeword = if flags.runeword() {
        Some((reader.read(12)? as u16, reader.read(4)? as u8))
    } else {
        None
    };
    let personalized = if flags.personalized() { Some(read_name(reader)?) } else { None };
    let realm = if reader.read(1)? != 0 {
        Some([reader.read(32)?, reader.read(32)?, reader.read(32)?, reader.read(32)?])
    } else {
        None
    };

    let is_armor = codes::is_armor(&code);
    let defense = if is_armor { Some(reader.read(11)? as u16) } else { None };
    let (max_durability, durability) = if codes::has_durability(&code) {
        let max = reader.read(8)? as u8;
        let cur = if max > 0 { Some(reader.read(9)? as u16) } else { None };
        (Some(max), cur)
    } else {
        (None, None)
    };
    let quantity = if codes::is_stackable_gear(&code) || codes::is_stackable_misc(&code) {
        Some(reader.read(9)? as u16)
    } else {
        None
    };
    let sockets = if flags.socketed() { Some(reader.read(4)? as u8) } else { None };

    let set_flags = if quality_raw == 5 {
        let mut f = [false; 5];
        for slot in f.iter_mut() {
            *slot = reader.read(1)? != 0;
        }
        Some(f)
    } else {
        None
    };

    let mods = read_mods(reader, table)?;
    let mut set_lists = Vec::new();
    if let Some(flags5) = set_flags {
        for active in flags5 {
            if active {
                set_lists.push(read_mods(reader, table)?);
            }
        }
    }
    let rw_mods = if flags.runeword() { Some(read_mods(reader, table)?) } else { None };

    let chronicle = if flags.chronicle() {
        let mut w = BitWriter::new();
        capture_bits(reader, 16, &mut w)?;
        if flags.chronicle_compact() {
            capture_bits(reader, 64, &mut w)?;
        } else {
            capture_bits(reader, 32, &mut w)?;
            let n = reader.read(4)?;
            w.write(n, 4);
            for _ in 0..n {
                capture_bits(reader, 64, &mut w)?;
            }
        }
        Some(Chronicle { bit_len: w.bit_pos(), bits: w.into_bytes() })
    } else {
        None
    };

    reader.align();
    Ok(ItemD2R {
        flags,
        version_high,
        version_low,
        mode,
        pos,
        col,
        row,
        stored_raw,
        ear: None,
        code,
        extended: Some(Box::new(ExtD2R {
            gems,
            guid,
            ilvl,
            quality_raw,
            var_gfx,
            class_specific,
            quality_data,
            runeword,
            personalized,
            realm,
            defense,
            max_durability,
            durability,
            quantity,
            sockets,
            set_flags,
            mods,
            set_lists,
            rw_mods,
            chronicle,
        })),
    })
}

/// Sérialise un item D2R (réciproque exacte de [`parse_item`]).
pub fn serialize_item(item: &ItemD2R, writer: &mut BitWriter) {
    serialize_item_with(item, writer, &ItemStatCost::embedded());
}

/// Sérialise un item D2R avec table explicite.
pub fn serialize_item_with(
    item: &ItemD2R,
    writer: &mut BitWriter,
    table: &ItemStatCost,
) {
    writer.write(item.flags.0, 32);
    writer.write(item.version_high as u32, 1);
    writer.write(item.version_low as u32, 2);
    writer.write(item.mode as u32, 3);
    if item.mode == 3 || item.mode == 5 {
        writer.write(item.col as u32, 16);
        writer.write(item.row as u32, 16);
    } else {
        writer.write(item.pos as u32, 4);
        writer.write(item.col as u32, 4);
        writer.write(item.row as u32, 4);
        writer.write(item.stored_raw as u32, 3);
    }

    if let Some((cls, level, name)) = &item.ear {
        writer.write(*cls as u32, 3);
        writer.write(*level as u32, 7);
        write_name(name, writer);
        writer.align();
        return;
    }

    // Code Huffman : 4 caractères (espace final pour les codes de 3).
    let mut full = item.code.clone();
    if full.chars().count() == 3 {
        full.push(' ');
    }
    for c in full.chars() {
        match huffman_encode(c) {
            Some(bits) => {
                for b in bits.chars() {
                    writer.write(b as u32 - '0' as u32, 1);
                }
            }
            None => writer.write(0, 8), // hors table : dégradé (jamais vu)
        }
    }

    let ext = match &item.extended {
        Some(ext) => ext,
        None => {
            // simple : drapeau realm + alignement
            if let Some(realm) = item_simple_realm(item) {
                writer.write(1, 1);
                for v in realm {
                    writer.write(v, 32);
                }
            } else {
                writer.write(0, 1);
            }
            writer.align();
            return;
        }
    };

    if item.flags.simple() {
        // simple avec bloc realm conservé (cas rare)
        if let Some(realm) = ext.realm {
            writer.write(1, 1);
            for v in realm {
                writer.write(v, 32);
            }
        } else {
            writer.write(0, 1);
        }
        writer.align();
        return;
    }

    writer.write(ext.gems as u32, 3);
    writer.write(ext.guid, 32);
    writer.write(ext.ilvl as u32, 7);
    writer.write(ext.quality_raw as u32, 4);
    match ext.var_gfx {
        Some(g) => {
            writer.write(1, 1);
            writer.write(g as u32, 3);
        }
        None => writer.write(0, 1),
    }
    match ext.class_specific {
        Some(cs) => {
            writer.write(1, 1);
            writer.write(cs as u32, 11);
        }
        None => writer.write(0, 1),
    }

    match &ext.quality_data {
        QualityD2R::None => {}
        QualityD2R::Low(v) => writer.write(*v as u32, 3),
        QualityD2R::Superior(v) => writer.write(*v as u32, 3),
        QualityD2R::Magic { prefix, suffix } => {
            writer.write(*prefix as u32, 11);
            writer.write(*suffix as u32, 11);
        }
        QualityD2R::Set(id) => writer.write(*id as u32, 12),
        QualityD2R::Rare { name1, name2, affixes } => {
            writer.write(*name1 as u32, 8);
            writer.write(*name2 as u32, 8);
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
        QualityD2R::Unique(id) => writer.write(*id as u32, 12),
    }

    if let Some((id, unk)) = ext.runeword {
        writer.write(id as u32, 12);
        writer.write(unk as u32, 4);
    }
    if let Some(name) = &ext.personalized {
        write_name(name, writer);
    }
    match ext.realm {
        Some(realm) => {
            writer.write(1, 1);
            for v in realm {
                writer.write(v, 32);
            }
        }
        None => writer.write(0, 1),
    }

    if let Some(def) = ext.defense {
        writer.write(def as u32, 11);
    }
    if let Some(max) = ext.max_durability {
        writer.write(max as u32, 8);
        if max > 0 {
            writer.write(ext.durability.unwrap_or(0) as u32, 9);
        }
    }
    if let Some(qty) = ext.quantity {
        writer.write(qty as u32, 9);
    }
    if let Some(sockets) = ext.sockets {
        writer.write(sockets as u32, 4);
    }
    if let Some(flags5) = ext.set_flags {
        for &active in flags5.iter() {
            writer.write(active as u32, 1);
        }
    }
    write_mods(&ext.mods, writer, table);
    for list in &ext.set_lists {
        write_mods(list, writer, table);
    }
    if let Some(rw) = &ext.rw_mods {
        write_mods(rw, writer, table);
    }
    if let Some(chron) = &ext.chronicle {
        for i in 0..chron.bit_len {
            let bit = (chron.bits[i / 8] >> (i % 8)) & 1;
            writer.write(bit as u32, 1);
        }
    }
    writer.align();
}

/// Realm d'un item simple (stocké dans `extended` sans autre champ).
fn item_simple_realm(item: &ItemD2R) -> Option<[u32; 4]> {
    item.extended.as_ref().and_then(|e| e.realm)
}

/// Liste d'items D2R : « JM » + u16 (compte déclaré, possiblement périmé)
/// puis items en continu jusqu'au prochain marqueur (« JM » sain, « jf »,
/// « kf ») ou la fin des données.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D2rItemList {
    /// Compte u16 lu après « JM » (périmé sur les fichiers édités).
    pub declared_count: u16,
    /// Items lus (enfants sertis inclus, à plat).
    pub items: Vec<ItemD2R>,
}

/// Le compte u16 à `pos` ressemble à un compte de liste (sanité < 512).
fn looks_like_count(data: &[u8], pos: usize) -> bool {
    data.len() >= pos + 2
        && ((data[pos] as u16) | ((data[pos + 1] as u16) << 8)) < 0x200
}

/// Parse une liste d'items D2R depuis `reader` (données brutes `data` pour
/// les sondes de marqueurs ; le lecteur doit être aligné sur un octet).
pub fn parse_item_list(
    reader: &mut BitReader,
    data: &[u8],
) -> Result<D2rItemList, D2sError> {
    parse_item_list_with(reader, data, &ItemStatCost::embedded())
}

/// Parse une liste d'items D2R avec table explicite.
pub fn parse_item_list_with(
    reader: &mut BitReader,
    data: &[u8],
    table: &ItemStatCost,
) -> Result<D2rItemList, D2sError> {
    let start = reader.bit_pos();
    let marker = reader.read_ascii(2)?;
    if marker != "JM" {
        return Err(D2sError::BadMarker {
            expected: "JM".into(),
            got: marker,
            bit: start,
        });
    }
    let declared_count = reader.read(16)? as u16;
    let mut items = Vec::new();
    loop {
        let byte_pos = reader.byte_pos();
        if byte_pos >= data.len() {
            break;
        }
        let two = &data[byte_pos..(byte_pos + 2).min(data.len())];
        if two == b"jf" || two == b"kf" {
            break;
        }
        if two == b"JM" && looks_like_count(data, byte_pos + 2) {
            break; // liste suivante (corpse)
        }
        let item = parse_item_with(reader, table)?;
        items.push(item);
    }
    Ok(D2rItemList { declared_count, items })
}

/// Sérialise une liste d'items D2R.
pub fn serialize_item_list(list: &D2rItemList, writer: &mut BitWriter) {
    serialize_item_list_with(list, writer, &ItemStatCost::embedded());
}

/// Sérialise une liste d'items D2R avec table explicite.
pub fn serialize_item_list_with(
    list: &D2rItemList,
    writer: &mut BitWriter,
    table: &ItemStatCost,
) {
    writer.write_ascii("JM");
    writer.write(list.declared_count as u32, 16);
    for item in &list.items {
        serialize_item_with(item, writer, table);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huffman_table_is_prefix_free_and_complete() {
        // chaque code décode vers son symbole
        for (sym, code) in HUFF {
            let mut w = BitWriter::new();
            for b in code.chars() {
                w.write(b as u32 - '0' as u32, 1);
            }
            let packed = w.into_bytes();
            let mut r = BitReader::new(&packed, 0);
            assert_eq!(huffman_decode(&mut r).unwrap(), sym.chars().next().unwrap(), "{sym}");
        }
    }

    #[test]
    fn huffman_known_values() {
        // « hp1 » (potion) : 00011 11001 1111100 10
        let mut w = BitWriter::new();
        for c in "hp1 ".chars() {
            for b in huffman_encode(c).unwrap().chars() {
                w.write(b as u32 - '0' as u32, 1);
            }
        }
        let bytes = w.into_bytes();
        assert_eq!(bytes.len(), 3); // 19 bits → 3 octets
        let mut r = BitReader::new(&bytes, 0);
        let mut out = String::new();
        for _ in 0..4 {
            out.push(huffman_decode(&mut r).unwrap());
        }
        assert_eq!(out, "hp1 ");
    }
}
