//! Table ItemStatCost v1.12 RÉELLE — embarquée en compile-time
//! (`data/itemstatcost_d2s.tsv`, extraction des colonnes ID/Stat/CSvBits/
//! ValShift/Save Bits/Save Add depuis `d2w-integ/assets/patch_itemstatcost.txt`,
//! 40 109 o, SHA-256 ee7fa82b…8375, 359 records).
//!
//! Rôles (evidence.md §6 et §8.6) :
//! * **CSvBits / ValShift** : largeurs et décalage des stats caractère
//!   (validation croisée locale 16/16 avec nokka).
//! * **Save Bits / Save Add** : largeurs des mods d'item ; paires min/max =
//!   largeur de la stat min + celle de la stat max (ID suivant, même base).
//!   Ex. mindamage (6) + maxdamage (7) = 13 bits.

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
    /// Bits de sauvegarde des **mods d'item**.
    pub save_bits: u8,
    /// Offset négatif des mods (valeur chargée = stockée − Save Add).
    pub save_add: u32,
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
        rows.push(IscRow { id, stat, csv_bits, val_shift, save_bits, save_add });
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

    /// Largeur effective d'un mod d'item : `Save Bits` (paires min/max =
    /// min + max de l'ID suivant), + `Save Add`/`ValShift` exposés.
    ///
    /// Règle du parseur de référence (`mod_widths` de d2s_parse.py) :
    /// stat contenant `min` + ID+1 contenant `max` même base → somme.
    pub fn mod_width(&self, id: u16) -> Option<(u8, u32, u8)> {
        let row = self.row(id)?;
        if row.save_bits == 0 {
            return None;
        }
        if let Some(rest) = row.stat.split_once("min") {
            let base = rest.0.to_string() + rest.1;
            if let Some(next) = self.row(id + 1) {
                if next.save_bits > 0 {
                    if let Some(nrest) = next.stat.split_once("max") {
                        let nbase = nrest.0.to_string() + nrest.1;
                        if base == nbase {
                            return Some((
                                row.save_bits + next.save_bits,
                                row.save_add,
                                row.val_shift,
                            ));
                        }
                    }
                }
            }
        }
        Some((row.save_bits, row.save_add, row.val_shift))
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

    /// Evidence §8.6 : largeurs de mods réelles de la table locale.
    #[test]
    fn mod_widths_from_local_table() {
        let t = ItemStatCost::embedded();
        // exemples cités dans evidence.md §8.6
        assert_eq!(t.row(19).map(|r| (r.stat, r.save_bits)), Some(("tohit", 10)));
        assert_eq!(t.row(21).map(|r| (r.stat, r.save_bits)), Some(("mindamage", 6)));
        assert_eq!(t.row(22).map(|r| (r.stat, r.save_bits)), Some(("maxdamage", 7)));
        assert_eq!(t.row(31).map(|r| (r.stat, r.save_bits)), Some(("armorclass", 11)));
        // paires min/max → largeur somme
        assert_eq!(t.mod_width(21), Some((13, 0, 0))); // mindamage+maxdamage
        assert_eq!(t.mod_width(23), Some((13, 0, 0))); // secondary_*
        assert_eq!(t.mod_width(48), Some((17, 0, 0))); // firemindam+firemaxdam
        // stats simples
        assert_eq!(t.mod_width(0), Some((8, 32, 0)));  // strength : 8 bits
        assert_eq!(t.mod_width(9), Some((8, 32, 8)));  // maxmana : 8 bits
        // Save Bits nuls → pas de mod sérialisable
        assert_eq!(t.mod_width(4), None); // statpts : Save Bits = 0
        assert_eq!(t.mod_width(5000), None);
    }
}
