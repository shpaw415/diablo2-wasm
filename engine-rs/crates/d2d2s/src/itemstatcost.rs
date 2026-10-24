//! Table ItemStatCost v1.12 RÉELLE — embarquée en compile-time
//! (`data/itemstatcost_d2s.tsv`, extraction des colonnes ID/Stat/CSvBits/
//! ValShift/Save Bits/Save Add/Save Param Bits depuis
//! `d2w-integ/assets/patch_itemstatcost.txt`, 40 109 o,
//! SHA-256 ee7fa82b…8375, 359 records).
//!
//! Rôles (evidence.md §6 et §8.6, **corrigés §VALIDATION RÉELLE 2026-10-24**
//! sur Javazon.d2s, fichier D2R v0x61 réel fourni par Justin) :
//! * **CSvBits / ValShift** : largeurs et décalage des stats caractère
//!   (validation croisée locale 16/16 avec nokka ; confirmée sur le réel).
//! * **Save Bits / Save Param Bits / Save Add** : largeurs des mods d'item.
//!   ⚠ CORRECTION (le fichier réel invalide l'ancienne règle « paires
//!   min/max par nom ») : la largeur d'un mod = `Save Bits` **+
//!   `Save Param Bits`** (couche param, lue AVANT la valeur — ex.
//!   item_nonclassskill 97 : param 9 bits = skill, valeur 6 bits = niveau ;
//!   item_charged_skill 204 : 16+16) **+ `Save Bits` des stats appariées**
//!   [`PAIRED`] (ex. coldmindam 54 = 8+9+8 avec coldmaxdam 55 et
//!   coldlength 56). nokka et d2savior concordent ; l'Enigma du réel
//!   (+775 def, +14 life/kill, +45% FRW, +1 Téléport p54, +2 skills,
//!   +str/lvl, +MF/lvl) ne se lit que sous cette règle.

#![allow(dead_code)]

use std::sync::OnceLock;

/// Une ligne de la table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IscRow {
    pub id: u16,
    /// Nom de la stat (`strength`, `mindamage`…).
    pub stat: &'static str,
    /// Bits de sauvegarde **Character Save** (stats du .d2s).
    pub csv_bits: u8,
    /// Décalage de recharge (hp/mana/stamina ÷256).
    pub val_shift: u8,
    /// Bits de sauvegarde des **mods d'item** (valeur).
    pub save_bits: u8,
    /// Offset négatif des mods (valeur chargée = stockée − Save Add).
    pub save_add: u32,
    /// Bits de **param** des mods (couche lue avant la valeur : skill,
    /// classe…). 0 = pas de param. — colonne ajoutée §VALIDATION RÉELLE.
    pub save_param_bits: u8,
}

/// Table en mémoire (359 records).
pub struct ItemStatCost {
    rows: Vec<IscRow>,
    by_id: std::collections::HashMap<u16, usize>,
}

const EMBEDDED_TSV: &str = include_str!("../data/itemstatcost_d2s.tsv");

fn load_embedded() -> ItemStatCost {
    let mut rows = Vec::new();
    for line in EMBEDDED_TSV.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        let mut f = line.split('\t');
        let id: u16 = f.next().unwrap_or("0").parse().unwrap_or(0);
        let stat: &str = f.next().unwrap_or("");
        let csv_bits: u8 = f.next().unwrap_or("0").parse().unwrap_or(0);
        let val_shift: u8 = f.next().unwrap_or("0").parse().unwrap_or(0);
        let save_bits: u8 = f.next().unwrap_or("0").parse().unwrap_or(0);
        let save_add: u32 = f.next().unwrap_or("0").parse().unwrap_or(0);
        let save_param_bits: u8 = f.next().unwrap_or("0").parse().unwrap_or(0);
        rows.push(IscRow { id, stat, csv_bits, val_shift, save_bits, save_add, save_param_bits });
    }
    let mut by_id = std::collections::HashMap::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        by_id.insert(row.id, i);
    }
    ItemStatCost { rows, by_id }
}

static TABLE: OnceLock<ItemStatCost> = OnceLock::new();

/// Table embarquée (chargée une fois, zéro IO runtime) — raccourci de
/// [`ItemStatCost::embedded`].
pub fn embedded() -> &'static ItemStatCost {
    ItemStatCost::embedded()
}

impl ItemStatCost {
    /// Table embarquée (chargée une fois, zéro IO runtime).
    pub fn embedded() -> &'static ItemStatCost {
        TABLE.get_or_init(load_embedded)
    }

    /// Nombre de records chargés.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// La table est-elle vide (échec d'embarquement — impossible en pratique) ?
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Ligne par ID.
    pub fn row(&self, id: u16) -> Option<&IscRow> {
        self.by_id.get(&id).map(|&i| &self.rows[i])
    }

    /// Stats appariées : après la valeur de l'ID, on lit `Save Bits` de
    /// chaque apparié (correction §VALIDATION RÉELLE — nokka + d2savior
    /// concordent ; l'ancienne règle « min/max par nom » est invalide :
    /// elle ratait coldlength/poisonlength et la paire 17→18).
    pub const PAIRED: &[(u16, &[u16])] = &[
        (17, &[18]),          // maxdamage_percent → mindamage_percent
        (48, &[49]),          // firemindam → firemaxdam
        (50, &[51]),          // lightmindam → lightmaxdam
        (52, &[53]),          // magicmindam → magicmaxdam
        (54, &[55, 56]),      // coldmindam → maxdam + length
        (57, &[58, 59]),      // poisonmindam → maxdam + length
    ];

    /// Bits appariés d'un ID (Σ Save Bits des appariés, 0 si aucun).
    fn paired_bits(&self, id: u16) -> u8 {
        Self::PAIRED
            .iter()
            .find(|(i, _)| *i == id)
            .map(|(_, ps)| {
                ps.iter().map(|&p| self.row(p).map(|r| r.save_bits).unwrap_or(0)).sum()
            })
            .unwrap_or(0)
    }

    /// Shape d'un mod : (bits param, bits valeur, save_add, val_shift).
    /// `Err(())` si l'ID est absent ou sans `Save Bits` (non sérialisable).
    pub fn mod_shape(&self, id: u16) -> Result<(u8, u8, u32, u8), ()> {
        let row = self.row(id).ok_or(())?;
        if row.save_bits == 0 {
            return Err(());
        }
        Ok((
            row.save_param_bits,
            row.save_bits + self.paired_bits(id),
            row.save_add,
            row.val_shift,
        ))
    }

    /// Largeur totale d'un mod d'item : `Save Bits` **+ `Save Param Bits`**
    /// **+ bits des appariés** ([`Self::PAIRED`]) — correction §VALIDATION
    /// RÉELLE ; paires min/max = min + max de l'ID suivant, même base.
    /// (Rétrocompat : l'ancienne signature (bits, save_add, val_shift).)
    pub fn mod_width(&self, id: u16) -> Option<(u8, u32, u8)> {
        let (spb, vbits, sa, vs) = self.mod_shape(id).ok()?;
        Some((spb + vbits, sa, vs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_size_matches_extraction() {
        let t = ItemStatCost::embedded();
        assert_eq!(t.len(), 359, "359 records dans patch_itemstatcost.txt v1.12");
        assert!(!t.is_empty());
    }

    /// Validation croisée evidence.md §6 : CSvBits == table nokka, 16/16.
    #[test]
    fn csv_bits_nokka_cross_validation() {
        let t = ItemStatCost::embedded();
        let expected = [
            (0, 10), (1, 10), (2, 10), (3, 10), (4, 10), (5, 8),
            (6, 21), (7, 21), (8, 21), (9, 21), (10, 21), (11, 21),
            (12, 7), (13, 32), (14, 25), (15, 25),
        ];
        for (id, bits) in expected {
            assert_eq!(t.row(id).unwrap().csv_bits, bits, "CSvBits id {}", id);
        }
    }

    /// Evidence §8.6 **corrigée §VALIDATION RÉELLE** : shapes réelles de la
    /// table locale (param + valeur + appariés).
    #[test]
    fn mod_widths_from_local_table() {
        let t = ItemStatCost::embedded();
        // exemples simples (sans param, sans apparié)
        assert_eq!(t.row(19).map(|r| (r.stat, r.save_bits)), Some(("tohit", 10)));
        assert_eq!(t.row(31).map(|r| (r.stat, r.save_bits)), Some(("armorclass", 11)));
        assert_eq!(t.mod_shape(19), Ok((0, 10, 0, 0))); // tohit
        assert_eq!(t.mod_shape(0), Ok((0, 8, 32, 0)));   // strength
        assert_eq!(t.mod_shape(9), Ok((0, 8, 32, 8)));   // maxmana
        // paires appariées [VALIDATION RÉELLE] — plus de fusion « min/max par nom »
        assert_eq!(t.mod_shape(21), Ok((0, 6, 0, 0)));  // mindamage SEUL = 6 bits
        assert_eq!(t.mod_shape(22), Ok((0, 7, 0, 0)));  // maxdamage SEUL = 7 bits
        assert_eq!(t.mod_shape(48), Ok((0, 17, 0, 0))); // fire = 8+9
        assert_eq!(t.mod_shape(50), Ok((0, 16, 0, 0))); // light = 6+10
        assert_eq!(t.mod_shape(54), Ok((0, 25, 0, 0))); // cold = 8+9+8 (length)
        assert_eq!(t.mod_shape(57), Ok((0, 29, 0, 0))); // poison = 10+10+9
        // param bits [VALIDATION RÉELLE : +1 Téléport p54 dans l'Enigma]
        assert_eq!(t.mod_shape(97), Ok((9, 6, 0, 0))); // nonclassskill
        assert_eq!(t.mod_shape(204), Ok((16, 16, 0, 0))); // charged_skill
        // Save Bits nuls → pas de mod sérialisable
        assert_eq!(t.mod_shape(4), Err(())); // statpts : Save Bits = 0
        assert_eq!(t.mod_shape(103), Err(())); // skill_bypass_undead : 0 (le
        // « ID 103 » du parseur python = les octets « gf » du v0x61 réel !)
        assert_eq!(t.mod_shape(5000), Err(()));
        // compat : mod_width = spb + valeur
        assert_eq!(t.mod_width(97), Some((15, 0, 0)));
        assert_eq!(t.mod_width(54), Some((25, 0, 0)));
    }
}
