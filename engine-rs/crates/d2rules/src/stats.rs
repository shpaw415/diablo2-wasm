//! Système de stats générique D2 — port adapté de
//! `opendiablo2/d2core/d2stats` (interfaces `Stat`/`StatList`/`StatValue`) et
//! `d2core/d2stats/diablo2stats/{stat,statlist,stat_value}.go` (socle A1 de
//! `finish-systemes.md`), avec `ItemStatCost.txt` comme **dictionnaire** des
//! stats (id ↔ nom, bits de sauvegarde, champs de description).
//!
//! # Port Rust et écarts assumés
//! * Go = interface `Stat` multi-valeurs avec stringers d'affichage ; ici :
//!   [`Stat`] = nom + valeurs ([`StatValue`], entier 64 bits + règle de
//!   combinaison), [`StatList`] = liste ordonnée avec agrégation — l'affichage
//!   (DescFunc + tables STRING fr) est un autre domaine, seuls les champs de
//!   description du dictionnaire sont exposés.
//! * `Combine` : `Sum` additionne (ex. `strength`), `Static` ne s'additionne
//!   jamais (index de classe/tab de skill, cf. `canBeCombinedWith` Go).
//! * Toutes les opérations sont pures et testables : l'agrégation
//!   équipement→héros sera câblée par l'intégration (`world.rs` intouché).

use std::collections::HashMap;
use std::sync::OnceLock;

/// Table `ItemStatCost.txt` v1.12 embarquée (359 records, patch_d2.mpq).
const ISC_TXT: &str = include_str!("../../../../assets/patch_itemstatcost.txt");

/// Découpe une ligne TSV D2 (cellules trimmées, guillemets retirés — même
/// convention que `loot::parse_rows`).
pub(crate) fn split_tsv(line: &str) -> Vec<String> {
    line.split('\t')
        .map(|c| c.trim().trim_matches('"').to_string())
        .collect()
}

/// Règle de combinaison d'une valeur (Go `ValueCombineType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Combine {
    /// Les valeurs s'additionnent (comportement par défaut).
    #[default]
    Sum,
    /// Valeur d'index/qualificatif : jamais additionnée, doit être égale pour
    /// combiner (ex. index de classe dans « +1 to Paladin Skills »).
    Static,
}

/// Une valeur de stat (Go `StatValue`, entier — les stats D2 sont entières).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct StatValue {
    /// Valeur entière.
    pub value: i64,
    /// Règle de combinaison.
    pub combine: Combine,
}

impl StatValue {
    /// Valeur additionnable.
    pub fn sum(value: i64) -> StatValue {
        StatValue { value, combine: Combine::Sum }
    }
    /// Valeur statique (index, jamais additionnée).
    pub fn static_(value: i64) -> StatValue {
        StatValue { value, combine: Combine::Static }
    }
}

/// Une stat nommée avec ses valeurs — port compact de `diablo2Stat`.
/// Le nom est la clé `ItemStatCost.txt` (`strength`, `item_maxdamage_percent`…).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stat {
    /// Clé du dictionnaire.
    pub name: String,
    /// Valeurs (1 = cas général ; >1 = stats multi-valeurs DescFunc 13-28).
    pub values: Vec<StatValue>,
}

impl Stat {
    /// Stat mono-valeur additionnable.
    pub fn new(name: &str, value: i64) -> Stat {
        Stat { name: name.into(), values: vec![StatValue::sum(value)] }
    }

    /// Somme des valeurs additionnables (les `Static` ne comptent pas dans un
    /// total lu — ex. la valeur principale d'une stat d'affichage).
    pub fn total(&self) -> i64 {
        self.values.iter().filter(|v| v.combine == Combine::Sum).map(|v| v.value).sum()
    }

    /// Deux stats peuvent-elles fusionner ? (port `canBeCombinedWith` Go :
    /// même nom, mêmes valeurs, mêmes règles ; `Static` ⇒ valeurs égales).
    pub fn can_combine_with(&self, other: &Stat) -> bool {
        self.name == other.name
            && self.values.len() == other.values.len()
            && self.values.iter().zip(&other.values).all(|(a, b)| {
                a.combine == b.combine && (a.combine == Combine::Sum || a.value == b.value)
            })
    }

    /// Fusionne `other` DANS self (somme des `Sum`, `Static` inchangés) ;
    /// renvoie false si incompatible (port `Combine` Go, sur place).
    pub fn combine_in_place(&mut self, other: &Stat) -> bool {
        if !self.can_combine_with(other) {
            return false;
        }
        for (a, b) in self.values.iter_mut().zip(&other.values) {
            if a.combine == Combine::Sum {
                a.value = a.value.wrapping_add(b.value);
            }
        }
        true
    }
}

/// Liste de stats — port compact de `Diablo2StatList` (Go) : push/pop,
/// agrégation (`add`), réduction des doublons, clonage.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatList {
    stats: Vec<Stat>,
}

impl StatList {
    /// Liste vide.
    pub fn new() -> StatList {
        StatList { stats: Vec::new() }
    }

    /// Nombre d'entrées.
    pub fn len(&self) -> usize {
        self.stats.len()
    }

    /// Liste vide ?
    pub fn is_empty(&self) -> bool {
        self.stats.is_empty()
    }

    /// Itère les entrées (une stat peut apparaître plusieurs fois si des
    /// valeurs `Static` différentes l'empêchent de fusionner).
    pub fn iter(&self) -> impl Iterator<Item = &Stat> {
        self.stats.iter()
    }

    /// Pousse une stat déjà construite (port `Push`).
    pub fn push(&mut self, stat: Stat) {
        self.stats.push(stat);
    }

    /// Retire et renvoie la dernière stat (port `Pop`).
    pub fn pop(&mut self) -> Option<Stat> {
        self.stats.pop()
    }

    /// Ajoute `value` à la stat `name` : fusionne dans l'entrée mono-valeur
    /// existante si possible, sinon pousse une nouvelle entrée. API principale
    /// pour les affixes/équipement (`stat.id += v`).
    pub fn add(&mut self, name: &str, value: i64) {
        if let Some(existing) = self
            .stats
            .iter_mut()
            .find(|s| s.name.eq_ignore_ascii_case(name) && s.values.len() == 1 && s.values[0].combine == Combine::Sum)
        {
            existing.values[0].value = existing.values[0].value.wrapping_add(value);
        } else {
            self.stats.push(Stat::new(name, value));
        }
    }

    /// Total agrégé d'une stat (somme des valeurs `Sum` de toutes les entrées
    /// portant ce nom) — lecture « équipement→héros ».
    pub fn get(&self, name: &str) -> i64 {
        self.stats
            .iter()
            .filter(|s| s.name.eq_ignore_ascii_case(name))
            .map(|s| s.total())
            .sum()
    }

    /// Première entrée portant ce nom (pour lire les multi-valeurs).
    pub fn stat(&self, name: &str) -> Option<&Stat> {
        self.stats.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Retire toutes les entrées d'une stat, renvoie le total retiré.
    pub fn remove(&mut self, name: &str) -> i64 {
        let before = self.get(name);
        self.stats.retain(|s| !s.name.eq_ignore_ascii_case(name));
        before
    }

    /// Concatène `other` (port `AppendStatList`).
    pub fn append(&mut self, other: &StatList) {
        self.stats.extend(other.stats.iter().cloned());
    }

    /// Clone puis fusionne les stats identiques (port `ReduceStats` Go :
    /// combinaison gloutonne, les incompatibles restent séparées).
    pub fn reduce(&self) -> StatList {
        let mut out = StatList::new();
        for stat in &self.stats {
            if let Some(existing) = out.stats.iter_mut().find(|s| s.can_combine_with(stat)) {
                existing.combine_in_place(stat);
            } else {
                out.stats.push(stat.clone());
            }
        }
        out
    }
}

/// Un record du dictionnaire `ItemStatCost.txt` (colonnes utiles au moteur).
/// Les champs `desc*`/`dgrp*` sont exposés pour l'UI future (DescFunc + TBL fr,
/// autre domaine) ; `save*`/`csv*` pour la persistance (save v3, `.d2s`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatCostRec {
    /// Clé (`Stat`) : `strength`, `hitpoints`, `passive_fire_mastery`…
    pub stat: String,
    /// Id numérique (`ID`) utilisé par les saves binaires D2.
    pub id: i32,
    /// Signé (`Signed`).
    pub signed: bool,
    /// Décalage de valeur à l'affichage (`ValShift`).
    pub val_shift: u32,
    /// Bits de sauvegarde (`Save Bits`), 0 = non sauvé.
    pub save_bits: u32,
    /// Ajout de sauvegarde (`Save Add`).
    pub save_add: i32,
    /// Bits CSV (`CSvBits`) — stat présente dans les CSV de personnage.
    pub csv_bits: u32,
    /// Param CSV (`CSvParam`).
    pub csv_add: i32,
    /// Priorité d'affichage (`descpriority`).
    pub desc_priority: i32,
    /// Fonction d'affichage (`descfunc`, 0 = pas de description).
    pub desc_func: i32,
    /// Position de la valeur (`descval` : 0 cachée, 1 préfixe, 2 suffixe).
    pub desc_val: i32,
    /// Clé string positif (`descstrpos`).
    pub desc_pos: String,
    /// Clé string négatif (`descstrneg`).
    pub desc_neg: String,
    /// Seconde clé string (`descstr2`).
    pub desc_str2: String,
    /// Groupe d'affichage (`dgrp`).
    pub dgrp: i32,
    /// Fonction du groupe (`dgrpfunc`).
    pub dgrp_func: i32,
    /// Valeur du groupe (`dgrpval`).
    pub dgrp_val: i32,
}

/// Dictionnaire des stats — `ItemStatCost.txt` parsé une fois pour toutes
/// ([`StatCostTable::global`]) ou depuis un TSV arbitraire (tests).
#[derive(Debug, Default)]
pub struct StatCostTable {
    by_name: HashMap<String, StatCostRec>,
    by_id: HashMap<i32, String>,
}

impl StatCostTable {
    /// Parse un TSV `ItemStatCost.txt`.
    pub fn parse(txt: &str) -> StatCostTable {
        let rows: Vec<Vec<String>> = txt
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(split_tsv)
            .collect();
        let mut table = StatCostTable::default();
        let Some(hdr) = rows.first() else { return table };
        let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
        let num = |r: &[String], c: Option<usize>| -> i32 {
            c.and_then(|c| r.get(c))
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0)
        };
        for r in rows.iter().skip(1) {
            let cell = |c: Option<usize>| -> String {
                c.and_then(|c| r.get(c)).cloned().unwrap_or_default()
            };
            let stat = cell(col("Stat"));
            if stat.is_empty() || stat.starts_with('*') {
                continue; // colonne d'annotation, pas un record
            }
            let rec = StatCostRec {
                stat: stat.clone(),
                id: num(r, col("ID")),
                signed: num(r, col("Signed")) != 0,
                val_shift: num(r, col("ValShift")).max(0) as u32,
                save_bits: num(r, col("Save Bits")).max(0) as u32,
                save_add: num(r, col("Save Add")),
                csv_bits: num(r, col("CSvBits")).max(0) as u32,
                csv_add: num(r, col("CSvParam")),
                desc_priority: num(r, col("descpriority")),
                desc_func: num(r, col("descfunc")),
                desc_val: num(r, col("descval")),
                desc_pos: cell(col("descstrpos")),
                desc_neg: cell(col("descstrneg")),
                desc_str2: cell(col("descstr2")),
                dgrp: num(r, col("dgrp")),
                dgrp_func: num(r, col("dgrpfunc")),
                dgrp_val: num(r, col("dgrpval")),
            };
            table.by_id.entry(rec.id).or_insert_with(|| stat.clone());
            table.by_name.insert(stat, rec);
        }
        table
    }

    /// Dictionnaire global embarqué (parse au premier accès).
    pub fn global() -> &'static StatCostTable {
        static TABLE: OnceLock<StatCostTable> = OnceLock::new();
        TABLE.get_or_init(|| StatCostTable::parse(ISC_TXT))
    }

    /// Record par clé exacte (insensible à la casse).
    pub fn get(&self, name: &str) -> Option<&StatCostRec> {
        self.by_name.get(&name.to_ascii_lowercase())
    }

    /// Nom de stat par id numérique (saves binaires) ; None si inconnu.
    pub fn name_of_id(&self, id: i32) -> Option<&str> {
        self.by_id.get(&id).map(|s| s.as_str())
    }

    /// Nombre de records chargés.
    pub fn len(&self) -> usize {
        self.by_name.len()
    }

    /// Dictionnaire vide ?
    pub fn is_empty(&self) -> bool {
        self.by_name.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_get_aggregate() {
        let mut l = StatList::new();
        l.add("strength", 5);
        l.add("strength", 10);
        l.add("Strength", 3); // insensible à la casse
        assert_eq!(l.get("strength"), 18);
        assert_eq!(l.len(), 1, "les adds fusionnent dans une seule entrée");
        l.add("hitpoints", 20);
        assert_eq!(l.get("hitpoints"), 20);
        assert_eq!(l.get("unknown"), 0);
    }

    #[test]
    fn static_values_never_sum() {
        let mut a = Stat::new("item_addskill_tab", 2);
        a.values.push(StatValue::static_(1)); // tab paladin
        let mut b = Stat::new("item_addskill_tab", 3);
        b.values.push(StatValue::static_(2)); // tab sorcière
        assert!(!a.can_combine_with(&b), "index statiques différents ⇒ pas de fusion");
        let mut c = Stat::new("item_addskill_tab", 1);
        c.values.push(StatValue::static_(1));
        assert!(a.can_combine_with(&c));
        assert!(a.combine_in_place(&c));
        assert_eq!(a.values[0].value, 3);
        assert_eq!(a.values[1].value, 1, "la valeur statique ne bouge pas");
    }

    #[test]
    fn reduce_merges_like_names_keeps_incompatibles() {
        let mut l = StatList::new();
        l.add("strength", 5);
        l.add("strength", 7);
        l.add("dexterity", 2);
        let r = l.reduce();
        assert_eq!(r.len(), 2);
        assert_eq!(r.get("strength"), 12);
    }

    #[test]
    fn remove_and_append_and_pop() {
        let mut l = StatList::new();
        l.add("life", 50);
        let mut other = StatList::new();
        other.add("mana", 15);
        l.append(&other);
        assert_eq!(l.len(), 2);
        assert_eq!(l.remove("life"), 50);
        assert_eq!(l.len(), 1);
        assert_eq!(l.pop().map(|s| s.name), Some("mana".into()));
        assert!(l.is_empty());
    }

    #[test]
    fn dictionary_global_loads_and_maps() {
        let t = StatCostTable::global();
        assert_eq!(t.len(), 359, "tous les records v1.12 chargés");
        let strength = t.get("strength").expect("strength au dictionnaire");
        assert_eq!(strength.id, 0);
        assert_eq!(t.name_of_id(0), Some("strength"));
        assert!(t.get("INEXISTANTE").is_none());
        assert!(t.name_of_id(9999).is_none());
    }
}
