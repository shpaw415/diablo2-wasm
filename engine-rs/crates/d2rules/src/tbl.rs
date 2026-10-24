//! `tbl` — décodeur des tables de chaînes Diablo 2 (`.tbl`), v1.12 (W18).
//!
//! Port fidèle de `d2w-re/tools/tbl_decode.py` (spéc dans
//! `d2w-re/tables/tbl-fra/evidence.md`, validée sur les 3 tables réelles
//! fr-FR de la release digitale v1.12). Format little-endian :
//!
//! ```text
//! Header 21 o : crc16 u16 | num_strings u16 | num_slots u16 | zéro u16
//!               | version u8 | data_start u32 | num_keys u32 | file_size u32
//! Index map   : num_strings × u16 — index de jeu → position de slot
//! Slots       : num_slots × 17 o — active u8 | str_idx u16 | bucket u32
//!               | key_off u32 | val_off u32 | val_len u16 (avec NUL final)
//! Données     : clé = [key_off, val_off) terminée NUL ;
//!               valeur = [val_off, val_off+val_len) terminée NUL
//! ```
//!
//! * Clés dupliquées légales (105 occurrences de `x` dans patchstring) —
//!   [`Tbl::get`] résout à la PREMIÈRE occurrence en ordre d'index de jeu
//!   (la fonction de hash `bucket` n'est pas reversée, non nécessaire).
//! * Encodage : UTF-8 strict d'abord, repli CP1252 (`U+FFFD` sur octets non
//!   définis) — identique au python ; les 3 tables v1.12 fr sont UTF-8 à 100 %.
//! * Validations = celles du python (version 1, `slots end == data_start`,
//!   nb d'actifs == num_strings, index map permutation des slots actifs,
//!   cadrage NUL) + lectures bornées (`TblError::Truncated` au lieu de paniquer).
//!
//! Pur : aucune IO, aucun traitement — [`Tbl::from_bytes`] consomme des octets.
//! Les 3 tables fr réelles sont embarquées via [`bundled_fra`] (`include_bytes!`
//! de `assets/tbl/`, même modèle que les autres tables v1.12 de d2rules).

use std::collections::HashMap;
use std::fmt;

/// Taille de l'en-tête (21 octets).
const HEADER_SIZE: usize = 0x15;
/// Taille d'un enregistrement de slot (17 octets).
const SLOT_SIZE: usize = 17;

/// Erreurs de décodage TBL (validations du port python + lectures bornées).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TblError {
    /// Fichier trop petit pour l'en-tête.
    TooSmall { len: usize },
    /// Champ version ≠ 1.
    BadVersion { version: u8 },
    /// Fin de la table de slots ≠ `data_start` de l'en-tête.
    LayoutMismatch { slots_end: u32, data_start: u32 },
    /// Offsetting hors du buffer (fichier tronqué/corrompu).
    Truncated { need: usize, have: usize },
    /// Nombre de slots actifs ≠ `num_strings`.
    ActiveMismatch { active: usize, expected: usize },
    /// L'index map n'est pas une permutation des positions actives.
    IndexMapNotPermutation,
    /// Clé ou valeur non terminée par NUL à l'index de jeu donné.
    BadFraming { index: usize },
}

impl fmt::Display for TblError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TblError::TooSmall { len } => write!(f, "tbl : trop petit ({len} o)"),
            TblError::BadVersion { version } => write!(f, "tbl : version inattendue {version}"),
            TblError::LayoutMismatch { slots_end, data_start } => write!(
                f,
                "tbl : incohérence de layout — fin des slots {slots_end} != data_start {data_start}"
            ),
            TblError::Truncated { need, have } => {
                write!(f, "tbl : tronqué — besoin de {need} o, buffer de {have} o")
            }
            TblError::ActiveMismatch { active, expected } => {
                write!(f, "tbl : {active} slots actifs != num_strings {expected}")
            }
            TblError::IndexMapNotPermutation => {
                write!(f, "tbl : l'index map n'est pas une permutation des slots actifs")
            }
            TblError::BadFraming { index } => {
                write!(f, "tbl : cadrage NUL incorrect à l'index de jeu {index}")
            }
        }
    }
}

impl std::error::Error for TblError {}

/// Une table de chaînes décodée. Ordre mémoire = ordre d'index de jeu
/// (celui des TSV/JSON de référence).
#[derive(Debug, Clone)]
pub struct Tbl {
    /// Checksum en-tête (algorithme non reversé — informationnel).
    pub crc16: u16,
    /// Nombre de chaînes actives (== `entries.len()`).
    pub num_strings: u16,
    /// Taille de la table de hash (actifs + supprimés).
    pub num_slots: u16,
    /// Offset absolu du bloc de données.
    pub data_start: u32,
    /// Champ `num_keys` de l'en-tête (≈ clés uniques, indicatif, PAS exact).
    pub header_num_keys: u32,
    /// Champ `file_size` de l'en-tête (auto-cohérence).
    pub file_size_field: u32,
    /// Entrées `(clé, texte)` en ordre d'index de jeu.
    entries: Vec<(String, String)>,
    /// Résolution clé → index de jeu (première occurrence gagne).
    lookup: HashMap<String, usize>,
}

/// Lit un `u16` little-endian borné.
fn read_u16(data: &[u8], off: usize) -> Result<u16, TblError> {
    let end = off.checked_add(2).ok_or(TblError::Truncated { need: usize::MAX, have: data.len() })?;
    if end > data.len() {
        return Err(TblError::Truncated { need: end, have: data.len() });
    }
    Ok(u16::from_le_bytes([data[off], data[off + 1]]))
}

/// Lit un `u32` little-endian borné.
fn read_u32(data: &[u8], off: usize) -> Result<u32, TblError> {
    let end = off.checked_add(4).ok_or(TblError::Truncated { need: usize::MAX, have: data.len() })?;
    if end > data.len() {
        return Err(TblError::Truncated { need: end, have: data.len() });
    }
    Ok(u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]]))
}

/// Décode le texte : UTF-8 strict d'abord, repli CP1252 (`U+FFFD` sur les
/// octets non définis de la page haute) — port de `decode_text` du python.
fn decode_text(raw: &[u8]) -> String {
    match std::str::from_utf8(raw) {
        Ok(s) => s.to_string(),
        Err(_) => raw.iter().map(|&b| cp1252_char(b)).collect(),
    }
}

/// Un octet CP1252 → `char` (0x80-0x9F = page haute, reste = Latin-1 ;
/// 0x81/0x8D/0x8F/0x90/0x9D non définis → `U+FFFD`, comme `errors='replace'`).
fn cp1252_char(b: u8) -> char {
    const HIGH: [Option<char>; 32] = [
        Some('\u{20AC}'), None, Some('\u{201A}'), Some('\u{0192}'),
        Some('\u{201E}'), Some('\u{2026}'), Some('\u{2020}'), Some('\u{2021}'),
        Some('\u{02C6}'), Some('\u{2030}'), Some('\u{0160}'), Some('\u{2039}'),
        Some('\u{0152}'), None, Some('\u{017D}'), None,
        None, Some('\u{2018}'), Some('\u{2019}'), Some('\u{201C}'),
        Some('\u{201D}'), Some('\u{2022}'), Some('\u{2013}'), Some('\u{2014}'),
        Some('\u{02DC}'), Some('\u{2122}'), Some('\u{0161}'), Some('\u{203A}'),
        Some('\u{0153}'), None, Some('\u{017E}'), Some('\u{0178}'),
    ];
    match b {
        0x80..=0x9F => HIGH[(b - 0x80) as usize].unwrap_or('\u{FFFD}'),
        _ => b as char,
    }
}

impl Tbl {
    /// Décode une table `.tbl` depuis ses octets (port de `tbl_decode.parse`).
    pub fn from_bytes(data: &[u8]) -> Result<Tbl, TblError> {
        if data.len() < HEADER_SIZE {
            return Err(TblError::TooSmall { len: data.len() });
        }
        let crc16 = read_u16(data, 0x00)?;
        let num_strings = read_u16(data, 0x02)?;
        let num_slots = read_u16(data, 0x04)?;
        let _zero = read_u16(data, 0x06)?;
        let version = data[0x08];
        let data_start = read_u32(data, 0x09)?;
        let header_num_keys = read_u32(data, 0x0D)?;
        let file_size_field = read_u32(data, 0x11)?;
        if version != 1 {
            return Err(TblError::BadVersion { version });
        }

        let map_off = HEADER_SIZE;
        let slots_off = map_off + 2 * num_strings as usize;
        let data_end = slots_off + SLOT_SIZE * num_slots as usize;
        if data_end as u32 != data_start {
            return Err(TblError::LayoutMismatch {
                slots_end: data_end as u32,
                data_start,
            });
        }
        if data_end > data.len() {
            return Err(TblError::Truncated { need: data_end, have: data.len() });
        }

        // Index map : index de jeu → position de slot.
        let mut index_map = Vec::with_capacity(num_strings as usize);
        for i in 0..num_strings as usize {
            index_map.push(read_u16(data, map_off + 2 * i)?);
        }

        // Table de slots.
        let mut slots = Vec::with_capacity(num_slots as usize);
        for i in 0..num_slots as usize {
            let o = slots_off + SLOT_SIZE * i;
            let active = data[o];
            let _str_idx = read_u16(data, o + 1)?;
            let _bucket = read_u32(data, o + 3)?;
            let key_off = read_u32(data, o + 7)?;
            let val_off = read_u32(data, o + 0xB)?;
            let val_len = read_u16(data, o + 0xF)?;
            slots.push((active, key_off, val_off, val_len));
        }

        // Validations du python : actifs == num_strings, permutation.
        let active_positions: Vec<u16> = slots
            .iter()
            .enumerate()
            .filter(|(_, &(active, ..))| active == 1)
            .map(|(pos, _)| pos as u16)
            .collect();
        if active_positions.len() != num_strings as usize {
            return Err(TblError::ActiveMismatch {
                active: active_positions.len(),
                expected: num_strings as usize,
            });
        }
        let mut sorted_map = index_map.clone();
        sorted_map.sort_unstable();
        let mut sorted_active = active_positions.clone();
        sorted_active.sort_unstable();
        if sorted_map != sorted_active {
            return Err(TblError::IndexMapNotPermutation);
        }

        // Entrées en ordre d'index de jeu, cadrage NUL vérifié.
        let mut entries = Vec::with_capacity(num_strings as usize);
        for (idx, &slot_pos) in index_map.iter().enumerate() {
            let (_active, key_off, val_off, val_len) = slots[slot_pos as usize];
            let key_off = key_off as usize;
            let val_off = val_off as usize;
            let val_len = val_len as usize;
            if val_off > data.len() || key_off > val_off || val_len == 0
                || val_off.checked_add(val_len).map_or(true, |e| e > data.len())
            {
                return Err(TblError::Truncated { need: val_off + val_len, have: data.len() });
            }
            let kraw = &data[key_off..val_off];
            let vraw = &data[val_off..val_off + val_len];
            if kraw.last() != Some(&0) || vraw.last() != Some(&0) {
                return Err(TblError::BadFraming { index: idx });
            }
            entries.push((decode_text(&kraw[..kraw.len() - 1]), decode_text(&vraw[..vraw.len() - 1])));
        }

        // Résolution clé → première occurrence (ordre d'index de jeu) —
        // les clés dupliquées sont légales ; la fonction de hash retail
        // n'étant pas reversée, la première occurrence est notre politique
        // de résolution documentée.
        let mut lookup = HashMap::with_capacity(entries.len());
        for (i, (key, _)) in entries.iter().enumerate() {
            lookup.entry(key.clone()).or_insert(i);
        }

        Ok(Tbl {
            crc16,
            num_strings,
            num_slots,
            data_start,
            header_num_keys,
            file_size_field,
            entries,
            lookup,
        })
    }

    /// Texte de la clé `key` — `None` si absente. En présence de clés
    /// dupliquées, la PREMIÈRE occurrence (ordre d'index de jeu) gagne.
    pub fn get(&self, key: &str) -> Option<String> {
        self.lookup.get(key).map(|&i| self.entries[i].1.clone())
    }

    /// Texte de la chaîne de jeu #`i` (ordre des TSV/JSON de référence).
    pub fn get_index(&self, i: usize) -> Option<String> {
        self.entries.get(i).map(|(_, text)| text.clone())
    }

    /// Clé de la chaîne de jeu #`i`.
    pub fn key_at(&self, i: usize) -> Option<&str> {
        self.entries.get(i).map(|(key, _)| key.as_str())
    }

    /// Nombre de chaînes (== `num_strings`).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Table vide.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Itère `(clé, texte)` en ordre d'index de jeu.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Nombre de clés uniques (méta evidence.md).
    pub fn unique_keys(&self) -> usize {
        self.lookup.len()
    }
}

/// Les 3 tables fr v1.12 réelles embarquées (copies `assets/tbl/`, SHA-256
/// vérifiés dans `assets/tbl/README.md` ; sources MPQ dans l'evidence.md RE).
pub mod bundled {
    /// `data\local\lng\fra\string.tbl` (5 391 chaînes).
    pub const STRING: &[u8] = include_bytes!("../../../../assets/tbl/string.tbl");
    /// `data\local\lng\fra\expansionstring.tbl` (2 818 chaînes).
    pub const EXPANSIONSTRING: &[u8] = include_bytes!("../../../../assets/tbl/expansionstring.tbl");
    /// `data\local\lng\fra\patchstring.tbl` (1 163 chaînes, patch_d2.mpq).
    pub const PATCHSTRING: &[u8] = include_bytes!("../../../../assets/tbl/patchstring.tbl");
}

/// Ensemble de tables avec la priorité de résolution retail :
/// **patchstring > expansionstring > string** (la table chargée en dernier
/// par le jeu gagne — patch > addon > base). Toutes optionnelles : une table
/// absente est simplement sautée.
#[derive(Debug, Clone, Default)]
pub struct TblSet {
    string: Option<Tbl>,
    expansion: Option<Tbl>,
    patch: Option<Tbl>,
}

impl TblSet {
    /// Construit l'ensemble depuis des octets optionnels (`from_bytes` sur
    /// chaque table présente ; une table illisible = `None`, jamais bloquant).
    pub fn from_bytes_opt(
        string: Option<&[u8]>,
        expansion: Option<&[u8]>,
        patch: Option<&[u8]>,
    ) -> TblSet {
        let dec = |b: Option<&[u8]>| b.and_then(|b| Tbl::from_bytes(b).ok());
        TblSet { string: dec(string), expansion: dec(expansion), patch: dec(patch) }
    }

    /// Les 3 tables fr v1.12 embarquées — panique seulement si les octets
    /// embarqués étaient corrompus (impossible : couvert par les golden tests
    /// `tests/tbl_golden.rs` sur les mêmes fichiers).
    pub fn bundled_fra() -> TblSet {
        match Self::from_bytes_opt(
            Some(bundled::STRING),
            Some(bundled::EXPANSIONSTRING),
            Some(bundled::PATCHSTRING),
        ) {
            set if set.string.is_some() && set.expansion.is_some() && set.patch.is_some() => set,
            _ => panic!("tbl : données fr embarquées illisibles (régression golden tests)"),
        }
    }

    /// Résout une clé : patchstring d'abord, puis expansionstring, puis
    /// string.tbl — `None` si la clé manque aux trois (repli appelant).
    pub fn get(&self, key: &str) -> Option<String> {
        self.patch
            .as_ref()
            .and_then(|t| t.get(key))
            .or_else(|| self.expansion.as_ref().and_then(|t| t.get(key)))
            .or_else(|| self.string.as_ref().and_then(|t| t.get(key)))
    }

    /// Table `string.tbl` (base), pour les tests/méta.
    pub fn string(&self) -> Option<&Tbl> {
        self.string.as_ref()
    }

    /// Nombre total de chaînes (somme des tables présentes).
    pub fn len(&self) -> usize {
        self.string.as_ref().map_or(0, |t| t.len())
            + self.expansion.as_ref().map_or(0, |t| t.len())
            + self.patch.as_ref().map_or(0, |t| t.len())
    }

    /// Ensemble vide.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sérialise une table minimale valide (header + index map + slots + data)
    /// pour tester `from_bytes` sans fixture binaire.
    fn build(entries: &[(bool, &str, &str)], extra_deleted: usize) -> Vec<u8> {
        let n = entries.iter().filter(|(a, _, _)| *a).count();
        let s = entries.len() + extra_deleted;
        let mut data = Vec::new();
        // data_start calculé après coup : header 21 + 2n + 17s
        let data_start = (HEADER_SIZE + 2 * n + SLOT_SIZE * s) as u32;
        // chaînes
        let mut strings = Vec::new();
        for (_, k, v) in entries {
            strings.push((k.as_bytes(), v.as_bytes()));
        }
        // offsets absolus : clés/valeurs posées après data_start
        let mut off = data_start as usize;
        let mut offs = Vec::new();
        for (k, v) in &strings {
            let key_off = off as u32;
            off += k.len() + 1;
            let val_off = off as u32;
            off += v.len() + 1;
            offs.push((key_off, val_off, (v.len() + 1) as u16));
        }
        let file_size = off as u32;
        data.extend_from_slice(&0x0000u16.to_le_bytes()); // crc (non vérifié)
        data.extend_from_slice(&(n as u16).to_le_bytes());
        data.extend_from_slice(&(s as u16).to_le_bytes());
        data.extend_from_slice(&0u16.to_le_bytes());
        data.push(1); // version
        data.extend_from_slice(&data_start.to_le_bytes());
        data.extend_from_slice(&(n as u32).to_le_bytes()); // num_keys (indicatif)
        data.extend_from_slice(&file_size.to_le_bytes());
        // index map : les actifs dans l'ordre
        for (pos, (a, _, _)) in entries.iter().enumerate() {
            if *a {
                data.extend_from_slice(&(pos as u16).to_le_bytes());
            }
        }
        // slots
        for (i, ((a, _, _), (key_off, val_off, val_len))) in entries.iter().zip(&offs).enumerate() {
            data.push(if *a { 1 } else { 0 });
            data.extend_from_slice(&(i as u16).to_le_bytes());
            data.extend_from_slice(&(i as u32).to_le_bytes());
            data.extend_from_slice(&key_off.to_le_bytes());
            data.extend_from_slice(&val_off.to_le_bytes());
            data.extend_from_slice(&val_len.to_le_bytes());
        }
        for slot in entries.len()..s {
            data.push(0);
            data.extend_from_slice(&[0u8; 16]); // slot supprimé
            let _ = slot;
        }
        for (k, v) in &strings {
            data.extend_from_slice(k);
            data.push(0);
            data.extend_from_slice(v);
            data.push(0);
        }
        data
    }

    #[test]
    fn roundtrip_minimal() {
        let data = build(&[(true, "Hello", "Bonjour"), (true, "Bye", "Au revoir")], 0);
        let t = Tbl::from_bytes(&data).expect("décodage");
        assert_eq!(t.len(), 2);
        assert_eq!(t.get("Hello").as_deref(), Some("Bonjour"));
        assert_eq!(t.get("Bye").as_deref(), Some("Au revoir"));
        assert_eq!(t.get("inconnu"), None);
        assert_eq!(t.get_index(0).as_deref(), Some("Bonjour"));
        assert_eq!(t.get_index(1).as_deref(), Some("Au revoir"));
        assert_eq!(t.get_index(2), None);
        assert_eq!(t.key_at(0), Some("Hello"));
    }

    #[test]
    fn deleted_slots_and_permutation() {
        // actif, supprimé, actif : l'index map saute le slot 1.
        let data = build(&[(true, "A", "a"), (false, "X", "x"), (true, "B", "b")], 1);
        let t = Tbl::from_bytes(&data).expect("décodage avec slots supprimés");
        assert_eq!(t.len(), 2);
        assert_eq!(t.get_index(0).as_deref(), Some("a"));
        assert_eq!(t.get_index(1).as_deref(), Some("b"));
        assert_eq!(t.get("X"), None); // slot supprimé non résoluble
    }

    #[test]
    fn duplicate_keys_first_wins() {
        let data = build(&[(true, "x", "premier"), (true, "x", "second")], 0);
        let t = Tbl::from_bytes(&data).expect("décodage");
        assert_eq!(t.len(), 2); // les deux entrées existent (index distincts)
        assert_eq!(t.unique_keys(), 1);
        assert_eq!(t.get("x").as_deref(), Some("premier"));
        assert_eq!(t.get_index(1).as_deref(), Some("second")); // accessible par index
    }

    #[test]
    fn cp1252_fallback_and_utf8_strict() {
        // UTF-8 valide décodé tel quel.
        let utf8 = build(&[(true, "k", "café")], 0);
        assert_eq!(Tbl::from_bytes(&utf8).unwrap().get("k").as_deref(), Some("café"));
        // Octet CP1252 0xE9 isolé (invalide UTF-8) → é via repli ; 0x81 non
        // défini → U+FFFD (comme `errors='replace'` côté python).
        let mut cp = build(&[(true, "k", "xx")], 0);
        // La valeur "xx" est la dernière séquence du fichier : x x NUL.
        let n = cp.len();
        cp[n - 3] = 0xE9;
        cp[n - 2] = 0x81;
        let t = Tbl::from_bytes(&cp).expect("décodage CP1252");
        assert_eq!(t.get("k").as_deref(), Some("\u{E9}\u{FFFD}"));
        assert_eq!(decode_text(&[0x80]), "\u{20AC}"); // € page haute
        assert_eq!(decode_text(&[0x41, 0xE9]), "A\u{E9}"); // Latin-1 haut
    }

    #[test]
    fn errors_are_detected() {
        assert!(matches!(Tbl::from_bytes(&[0u8; 4]), Err(TblError::TooSmall { len: 4 })));
        let mut bad = build(&[(true, "A", "a")], 0);
        bad[0x08] = 2; // version
        assert!(matches!(Tbl::from_bytes(&bad), Err(TblError::BadVersion { version: 2 })));
        let mut short = build(&[(true, "A", "a")], 0);
        short.truncate(short.len() - 1); // dernière valeur sans son NUL final
        assert!(matches!(
            Tbl::from_bytes(&short),
            Err(TblError::BadFraming { .. }) | Err(TblError::Truncated { .. })
        ));
    }

    #[test]
    fn tblset_priority_patch_wins() {
        let base = build(&[(true, "Zone", "Base")], 0);
        let patch = build(&[(true, "Zone", "Patch")], 0);
        let set = TblSet::from_bytes_opt(Some(&base), None, Some(&patch));
        assert_eq!(set.get("Zone").as_deref(), Some("Patch"));
        let set2 = TblSet::from_bytes_opt(Some(&base), None, None);
        assert_eq!(set2.get("Zone").as_deref(), Some("Base"));
        assert_eq!(set2.get("absente"), None); // repli appelant documenté
    }
}
