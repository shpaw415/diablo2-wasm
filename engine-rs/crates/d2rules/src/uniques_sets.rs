//! `uniques_sets` — uniques et sets identifiés v1.12 (`uniqueitems.txt`,
//! `setitems.txt`, `sets.txt`).
//!
//! Port Rust des records od2 (`unique_items_record.go`, `set_item_record.go`,
//! `set_record.go`) + sélection de drop D2 réel (réf. exécutable Diablerie
//! `ItemDrop.SelectUniqueForItem`/`SelectSetItem`) :
//!
//! * **index base → candidats** : un unique/set item est candidat si son code
//!   de base (`code`/`item`) correspond au code du drop ;
//! * **porte de niveau** : `ilvl ≥ lvl` du record (sinon dégradé rare, comme
//!   chez Diablerie) ;
//! * **pondération `rarity`** (« plus grand = plus commun », od2) ;
//! * **propriétés** : `prop1-12` (uniques), `prop1-9` + blocs additionnels
//!   `apropNa/Nb` (set items, `add func`), partielles `PCode2-5a/b` et
//!   complètes `FCode1-8` (sets) — descripteurs bruts [`PropDescriptor`]
//!   consommés tels quels par le socle Stat.
//!
//! Zéro IO, déterminisme côté appelant (1 tirage pondéré par sélection).

use std::collections::HashMap;

use crate::item_props::PropDescriptor;
use crate::rng::Rng;

/// Un unique (`uniqueitems.txt` v1.12 — ~403 records, port
/// `unique_items_record.go`, champs utiles au drop).
#[derive(Debug, Clone)]
pub struct UniqueRec {
    /// Clé TBL (`index`, ex. `Gull`).
    pub index: String,
    /// Code de base (`code` → weapons/armor/misc, ex. `dgr`).
    pub code: String,
    /// `enabled` (0 ⇒ record jamais candidat).
    pub enabled: bool,
    /// Niveau du drop (`lvl` — l'ilvl du monstre doit l'atteindre).
    pub level: u32,
    /// Niveau requis du porteur (`lvl req`).
    pub level_req: u32,
    /// Poids de tirage (`rarity`).
    pub rarity: u32,
    /// Propriétés `prop1-12`.
    pub props: Vec<PropDescriptor>,
}

/// Une pièce de set (`setitems.txt` v1.12 — ~128 records).
#[derive(Debug, Clone)]
pub struct SetItemRec {
    /// Clé TBL (`index`, ex. `Civerb's Vestments`).
    pub index: String,
    /// Set parent (`set` → clé `index` de `Sets.txt`).
    pub set: String,
    /// Code de base (`item`).
    pub item_code: String,
    /// Niveau du drop (`lvl`).
    pub level: u32,
    /// Niveau requis (`lvl req`).
    pub level_req: u32,
    /// Poids (`rarity`).
    pub rarity: u32,
    /// Mode des blocs additionnels (`add func` : 0 = props plates).
    pub add_func: i32,
    /// Propriétés propres `prop1-9`.
    pub props: Vec<PropDescriptor>,
    /// Blocs additionnels `apropNa`/`apropNb` (bonus par nombre de pièces).
    pub additional: [Vec<PropDescriptor>; 2],
}

/// Un set complet (`sets.txt` v1.12 — 33 records dont la ligne marqueur
/// « Expansion » des MPQ : bonus partiels + FULL).
#[derive(Debug, Clone)]
pub struct SetRec {
    /// Clé (`index`, reliée par [`SetItemRec::set`]).
    pub index: String,
    /// Version (0 vanilla, 100 LoD).
    pub version: u32,
    /// Niveau du set (`level`).
    pub level: u32,
    /// Bonus partiels variante A (`PCode2a-5a`).
    pub partial_a: Vec<PropDescriptor>,
    /// Bonus partiels variante B (`PCode2b-5b`).
    pub partial_b: Vec<PropDescriptor>,
    /// Bonus set COMPLET (`FCode1-8`).
    pub full: Vec<PropDescriptor>,
}

/// Tables uniques/sets v1.12 embarquées + index base → positions.
#[derive(Debug, Clone, Default)]
pub struct UniquesSets {
    /// Uniques (ordre du fichier).
    pub uniques: Vec<UniqueRec>,
    /// Pièces de set (ordre du fichier).
    pub set_items: Vec<SetItemRec>,
    /// Sets complets (ordre du fichier).
    pub sets: Vec<SetRec>,
    /// Index code de base → positions dans `uniques`.
    by_base_unique: HashMap<String, Vec<usize>>,
    /// Index code de base → positions dans `set_items`.
    by_base_set_item: HashMap<String, Vec<usize>>,
    /// Index clé `index` → position dans `sets`.
    set_by_index: HashMap<String, usize>,
}

/// Lit une cellule TSV (trimée) ; index hors bornes ⇒ vide.
fn cell(r: &[String], i: Option<usize>) -> &str {
    i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("")
}

/// Position de colonne (insensible à la casse).
fn col_of(hdr: &[String], name: &str) -> Option<usize> {
    hdr.iter().position(|h| h.eq_ignore_ascii_case(name))
}

/// Bloc `XN`/`parN`/`minN`/`maxN` (préfixe `prop`, n = 1..=count).
fn prop_block(hdr: &[String], r: &[String], prefix: &str, count: usize) -> Vec<PropDescriptor> {
    (1..=count)
        .filter_map(|n| {
            Some(crate::item_props::descriptor([
                cell(r, col_of(hdr, &format!("{prefix}{n}"))),
                cell(r, col_of(hdr, &format!("par{n}"))),
                cell(r, col_of(hdr, &format!("min{n}"))),
                cell(r, col_of(hdr, &format!("max{n}"))),
            ])?)
        })
        .collect()
}

/// Bloc set items `apropNs`/`aparNs`/`aminNs`/`amaxNs` (suffixe `a`/`b`).
fn set_additional_block(
    hdr: &[String],
    r: &[String],
    suffix: char,
) -> Vec<PropDescriptor> {
    (1..=5)
        .filter_map(|n| {
            Some(crate::item_props::descriptor([
                cell(r, col_of(hdr, &format!("aprop{n}{suffix}"))),
                cell(r, col_of(hdr, &format!("apar{n}{suffix}"))),
                cell(r, col_of(hdr, &format!("amin{n}{suffix}"))),
                cell(r, col_of(hdr, &format!("amax{n}{suffix}"))),
            ])?)
        })
        .collect()
}

/// Parse les trois tables embarquées ; `None` si aucune ligne exploitable
/// (la résolution qualité retombe alors sans uniques/sets).
pub fn parse(unique_txt: &str, set_item_txt: &str, set_txt: &str) -> Option<UniquesSets> {
    let uniques = parse_uniques(unique_txt);
    let set_items = parse_set_items(set_item_txt);
    let sets = parse_sets(set_txt);
    (uniques.len() + set_items.len() > 0).then(|| {
        let mut t = UniquesSets {
            by_base_unique: HashMap::new(),
            by_base_set_item: HashMap::new(),
            set_by_index: HashMap::new(),
            uniques,
            set_items,
            sets,
        };
        for (i, u) in t.uniques.iter().enumerate() {
            if u.enabled {
                t.by_base_unique.entry(u.code.clone()).or_default().push(i);
            }
        }
        for (i, s) in t.set_items.iter().enumerate() {
            t.by_base_set_item.entry(s.item_code.clone()).or_default().push(i);
        }
        for (i, s) in t.sets.iter().enumerate() {
            t.set_by_index.insert(s.index.clone(), i);
        }
        t
    })
}

fn parse_uniques(txt: &str) -> Vec<UniqueRec> {
    let rows = crate::loot::tsv_rows(txt);
    let Some(hdr) = rows.first().cloned() else { return Vec::new() };
    let (Some(index_c), Some(code_c)) = (col_of(&hdr, "index"), col_of(&hdr, "code")) else {
        return Vec::new();
    };
    let num = |r: &[String], name: &str| cell(r, col_of(&hdr, name)).parse::<u32>().unwrap_or(0);
    rows.iter()
        .skip(1)
        .filter(|r| !cell(r, Some(index_c)).is_empty())
        .map(|r| UniqueRec {
            index: cell(r, Some(index_c)).to_string(),
            code: cell(r, Some(code_c)).to_string(),
            enabled: num(r, "enabled") == 1,
            level: num(r, "lvl"),
            level_req: num(r, "lvl req"),
            rarity: num(r, "rarity").max(1),
            props: prop_block(&hdr, r, "prop", 12),
        })
        .collect()
}

fn parse_set_items(txt: &str) -> Vec<SetItemRec> {
    let rows = crate::loot::tsv_rows(txt);
    let Some(hdr) = rows.first().cloned() else { return Vec::new() };
    let (Some(index_c), Some(item_c)) = (col_of(&hdr, "index"), col_of(&hdr, "item")) else {
        return Vec::new();
    };
    let num = |r: &[String], name: &str| cell(r, col_of(&hdr, name)).parse::<u32>().unwrap_or(0);
    rows.iter()
        .skip(1)
        .filter(|r| !cell(r, Some(index_c)).is_empty())
        .map(|r| SetItemRec {
            index: cell(r, Some(index_c)).to_string(),
            set: cell(r, col_of(&hdr, "set")).to_string(),
            item_code: cell(r, Some(item_c)).to_string(),
            level: num(r, "lvl"),
            level_req: num(r, "lvl req"),
            rarity: num(r, "rarity").max(1),
            add_func: cell(r, col_of(&hdr, "add func")).parse().unwrap_or(0),
            props: prop_block(&hdr, r, "prop", 9),
            additional: [
                set_additional_block(&hdr, r, 'a'),
                set_additional_block(&hdr, r, 'b'),
            ],
        })
        .collect()
}

fn parse_sets(txt: &str) -> Vec<SetRec> {
    let rows = crate::loot::tsv_rows(txt);
    let Some(hdr) = rows.first().cloned() else { return Vec::new() };
    let Some(index_c) = col_of(&hdr, "index") else { return Vec::new() };
    let num = |r: &[String], name: &str| cell(r, col_of(&hdr, name)).parse::<u32>().unwrap_or(0);
    // Bonus partiels PCode2-5a / PCode2-5b, complets FCode1-8, PAR LIGNE
    // (chaque set = une ligne ; les colonnes diffèrent d'une ligne à l'autre).
    let partial = |r: &[String], suffix: char| -> Vec<PropDescriptor> {
        (2..=5)
            .filter_map(|n| {
                Some(crate::item_props::descriptor([
                    cell(r, col_of(&hdr, &format!("PCode{n}{suffix}"))),
                    cell(r, col_of(&hdr, &format!("PParam{n}{suffix}"))),
                    cell(r, col_of(&hdr, &format!("PMin{n}{suffix}"))),
                    cell(r, col_of(&hdr, &format!("PMax{n}{suffix}"))),
                ])?)
            })
            .collect()
    };
    let full = |r: &[String]| -> Vec<PropDescriptor> {
        (1..=8)
            .filter_map(|n| {
                Some(crate::item_props::descriptor([
                    cell(r, col_of(&hdr, &format!("FCode{n}"))),
                    cell(r, col_of(&hdr, &format!("FParam{n}"))),
                    cell(r, col_of(&hdr, &format!("FMin{n}"))),
                    cell(r, col_of(&hdr, &format!("FMax{n}"))),
                ])?)
            })
            .collect()
    };
    rows.iter()
        .skip(1)
        .filter(|r| !cell(r, Some(index_c)).is_empty())
        .map(|r| SetRec {
            index: cell(r, Some(index_c)).to_string(),
            version: num(r, "version"),
            level: num(r, "level"),
            partial_a: partial(r, 'a'),
            partial_b: partial(r, 'b'),
            full: full(r),
        })
        .collect()
}

/// Tirage pondéré : retourne l'index choisi dans `weights` (1 tirage [`Rng`]).
fn weighted_pick(rng: &mut Rng, weights: &[u32]) -> Option<usize> {
    let total: u64 = weights.iter().map(|&w| w.max(1) as u64).sum();
    if total == 0 {
        return None;
    }
    let mut pick = rng.below(total as u32) as u64;
    for (i, &w) in weights.iter().enumerate() {
        let w = w.max(1) as u64;
        if pick < w {
            return Some(i);
        }
        pick -= w;
    }
    None
}

impl UniquesSets {
    /// Uniques candidats pour un code de base (positions, ordre du fichier).
    pub fn uniques_for_base(&self, code: &str) -> &[usize] {
        self.by_base_unique.get(code).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Pièces de set candidats pour un code de base (positions).
    pub fn set_items_for_base(&self, code: &str) -> &[usize] {
        self.by_base_set_item.get(code).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Set complet par clé `index`.
    pub fn set_by_index(&self, index: &str) -> Option<&SetRec> {
        self.set_by_index.get(index).map(|&i| &self.sets[i])
    }

    /// Sélectionne l'unique du drop : candidats `enabled` avec
    /// `ilvl ≥ lvl`, pondérés par `rarity` (port Diablerie
    /// `SelectUniqueForItem`). Un tirage [`Rng`] ; `None` ⇒ dégrader rare.
    pub(crate) fn select_unique(&self, rng: &mut Rng, ilvl: u32, code: &str) -> Option<&UniqueRec> {
        let cands: Vec<&UniqueRec> = self
            .uniques_for_base(code)
            .iter()
            .filter_map(|&i| self.uniques.get(i))
            .filter(|u| u.enabled && ilvl >= u.level)
            .collect();
        let weights: Vec<u32> = cands.iter().map(|u| u.rarity).collect();
        weighted_pick(rng, &weights).map(|i| cands[i])
    }

    /// Sélectionne la pièce de set du drop (mêmes règles, `rarity`).
    pub(crate) fn select_set_item(&self, rng: &mut Rng, ilvl: u32, code: &str) -> Option<&SetItemRec> {
        let cands: Vec<&SetItemRec> = self
            .set_items_for_base(code)
            .iter()
            .filter_map(|&i| self.set_items.get(i))
            .filter(|s| ilvl >= s.level)
            .collect();
        let weights: Vec<u32> = cands.iter().map(|s| s.rarity).collect();
        weighted_pick(rng, &weights).map(|i| cands[i])
    }
}

// ---------------------------------------------------------------------------
// Tests — tables réelles embarquées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const UNIQUES: &str = include_str!("../../../../assets/patch_uniqueitems.txt");
    const SETITEMS: &str = include_str!("../../../../assets/patch_setitems.txt");
    const SETS: &str = include_str!("../../../../assets/patch_sets.txt");

    fn tables() -> UniquesSets {
        parse(UNIQUES, SETITEMS, SETS).expect("tables uniques/sets valides")
    }

    /// Golden : les 402/128/33 records réels v1.12 sont parsés (sets.txt
    /// inclut sa ligne marqueur « Expansion », données patch_d2.mpq).
    #[test]
    fn parse_real_record_counts() {
        let t = tables();
        assert_eq!(t.uniques.len(), 402, "uniques (UniqueItems.txt complet)");
        assert_eq!(t.set_items.len(), 128, "set items (SetItems.txt complet)");
        assert_eq!(t.sets.len(), 33, "sets (Sets.txt complet) : {}", t.sets.len());
    }

    /// BRIEF : Gull résolu — dague `dgr`, niveau 6, dmg-min 1 / dmg-max 15
    /// (valeurs exactes), candidat au-dessus de la porte de niveau seulement.
    #[test]
    fn gull_resolves_with_exact_props() {
        let t = tables();
        let gull = t.uniques.iter().find(|u| u.index == "Gull").expect("Gull");
        assert_eq!(gull.code, "dgr");
        assert_eq!(gull.level, 6);
        assert_eq!(gull.rarity, 1);
        assert!(gull.enabled);
        let dmg_max = gull.props.iter().find(|p| p.code == "dmg-max").unwrap();
        assert_eq!((dmg_max.min, dmg_max.max), (15, 15));
        assert!(gull.props.iter().any(|p| p.code == "dmg-min"));
        assert!(gull.props.iter().any(|p| p.code == "mag%"), "Gull = +Magic Find");
        assert!(t.uniques_for_base("dgr").iter().any(|&i| t.uniques[i].index == "Gull"));
        let mut rng = Rng::new(1);
        assert!(t.select_unique(&mut rng, 6, "dgr").is_some(), "ilvl 6 ≥ lvl 6");
        assert!(t.select_unique(&mut rng, 5, "dgr").is_none(), "ilvl 5 < lvl 6");
    }

    /// BRIEF : « The Gnasher » présent et résolu (base `hax` = Hand Axe,
    /// code réel v1.12 — l'unique porte le code de SA base).
    #[test]
    fn the_gnasher_present() {
        let t = tables();
        let g = t
            .uniques
            .iter()
            .find(|u| u.index == "The Gnasher")
            .expect("The Gnasher dans uniqueitems");
        assert_eq!(g.code, "hax");
        assert!(g.enabled);
        assert!(!g.props.is_empty(), "propriétés parsées");
        assert!(t.uniques_for_base("hax").iter().any(
            |&i| t.uniques[i].index == "The Gnasher"
        ));
    }

    /// Sets : « Civerb's Vestments » (SET, clé Sets.txt) relie 3 pièces
    /// (Ward/Icon/Cudgel) et porte les bonus complets v1.12 exacts
    /// (str 15, dmg-undead 200, res-ltng 25) ; chaque ligne de Sets.txt
    /// garde SES bonus (Hsarus ≠ Civerb).
    #[test]
    fn set_items_link_to_full_set_bonuses() {
        let t = tables();
        let parent = t
            .set_by_index("Civerb's Vestments")
            .expect("set Civerb's Vestments dans Sets.txt");
        let pieces: Vec<&SetItemRec> = t
            .set_items
            .iter()
            .filter(|s| s.set == "Civerb's Vestments")
            .collect();
        assert!(pieces.len() >= 3, "Civerb ≥3 pièces : {}", pieces.len());
        assert!(pieces.iter().any(|s| s.item_code == "lrg"), "Ward = Large Shield");
        assert!(pieces.iter().any(|s| s.item_code == "amu"), "Icon = Amulette");
        assert!(pieces.iter().any(|s| s.item_code == "gsc"), "Cudgel = Grand Scepter");
        let codes: Vec<&str> = parent.full.iter().map(|p| p.code.as_str()).collect();
        assert!(codes.contains(&"str"), "full set sans str : {codes:?}");
        assert!(codes.contains(&"dmg-undead"));
        assert!(codes.contains(&"res-ltng"));
        let str_p = parent.full.iter().find(|p| p.code == "str").unwrap();
        assert_eq!((str_p.min, str_p.max), (15, 15));
        // Hsarus' Defense (autre ligne de Sets.txt) porte SES codes à elle.
        let hsarus = t
            .sets
            .iter()
            .find(|s| s.index.starts_with("Hsarus"))
            .expect("Hsarus' Defense");
        let hc: Vec<&str> = hsarus.full.iter().map(|p| p.code.as_str()).collect();
        assert!(hc.contains(&"dmg-max") && hc.contains(&"nofreeze"), "{hc:?}");
        // Les pièces Hsarus (buc = Buckler) indexent vers ce set.
        assert!(t.set_items_for_base("buc").iter().any(|&i| t.set_items[i].set.starts_with("Hsarus")));
    }

    /// Pondération rarity (moteur + table réelle) : les pièces de set v1.12
    /// portent rarity 1, 3 ou 7 (SetItems.txt) ; un poids 7 sort ~7× plus
    /// qu'un poids 1 — on vérifie la table PUIS le moteur sur le couple réel
    /// extrême (7 contre 1), part attendue 7/8 = 0,875.
    #[test]
    fn rarity_weighting_follows_table() {
        let t = tables();
        // Table réelle : au moins une pièce à rarity 7 et une à rarity 1.
        assert!(t.set_items.iter().any(|s| s.rarity == 7), "rareté 7 présente");
        assert!(t.set_items.iter().any(|s| s.rarity == 1), "rareté 1 présente");
        // Moteur : ratio de la table réelle (7:1), même loi que le tirage
        // d'uniques/sets (un tirage pondéré par candidat).
        let mut seven_wins = 0;
        let mut rng = Rng::new(7);
        for _ in 0..20_000 {
            if let Some(i) = weighted_pick(&mut rng, &[7, 1]) {
                if i == 0 {
                    seven_wins += 1;
                }
            }
        }
        let share = seven_wins as f64 / 20_000.0;
        assert!((0.84..=0.91).contains(&share), "part du poids 7 : {share}");
    }
}
