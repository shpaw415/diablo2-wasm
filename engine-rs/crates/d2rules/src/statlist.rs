//! `statlist` — SOCLE Stat/StatList par IDS `ItemStatCost.txt` (A1, étape 7c
//! loot D2 réel).
//!
//! Le module [`crate::stats`] (W01) porte le port od2 nom-par-chaîne
//! (`Stat`/`StatList` + dictionnaire) ; ici on ajoute la moitié « moteur » du
//! socle A1 : une liste de stats **indexée par les ids numériques
//! ItemStatCost** (les clés des saves binaires `.d2s` et du langage
//! `stat.id += v`), avec :
//!
//! * [`StatLine`] — une contribution `(id, valeur)` (l'interface « affix_ids »
//!   : id + valeur, produite par les affixes/uniques/sets) ;
//! * [`StatList`] — map id → valeur agrégée, alimentée par n'importe quel
//!   nombre de [`StatSource`] (base héros, items équipés, buffs…) ;
//! * accesseurs typés pour les stats courantes (str/dex/vit/ene, hp/mana/
//!   stamina, AC, AR, dmg min/max, résistances 4, MF/GF) ;
//! * pont [`prop_stat_lines`] depuis le vocabulaire typé
//!   [`crate::item_props::StatProp`] (affixes roulés).
//!
//! Toutes les opérations sont PURES et déterministes : l'agrégation
//! héros = base + équipement est une simple somme de sources
//! ([`StatList::from_sources`]). Zéro IO, zéro panique.
//!
//! # Sources des ids (tables v1.12 réelles, `patch_itemstatcost.txt`)
//! Chaque constante est revérifiée par un golden test contre le dictionnaire
//! embarqué ([`crate::stats::StatCostTable::global`]) : ex. `strength`=0,
//! `hitpoints`=6, `armorclass`=31, `item_magicbonus`=80.

use std::collections::BTreeMap;

// --- Ids `ItemStatCost.txt` v1.12 (clés du moteur, cf. golden test) ---
/// `strength` (id 0).
pub const STRENGTH: i32 = 0;
/// `energy` (id 1).
pub const ENERGY: i32 = 1;
/// `dexterity` (id 2).
pub const DEXTERITY: i32 = 2;
/// `vitality` (id 3).
pub const VITALITY: i32 = 3;
/// `hitpoints` (id 6) — bonus vie.
pub const HITPOINTS: i32 = 6;
/// `maxhp` (id 7) — vie max (ValShift 8 au retail, affichage %).
pub const MAXHP: i32 = 7;
/// `mana` (id 8) — bonus mana.
pub const MANA: i32 = 8;
/// `maxmana` (id 9).
pub const MAXMANA: i32 = 9;
/// `stamina` (id 10) — bonus endurance.
pub const STAMINA: i32 = 10;
/// `maxstamina` (id 11).
pub const MAXSTAMINA: i32 = 11;
/// `level` (id 12).
pub const LEVEL: i32 = 12;
/// `item_armor_percent` (id 16) — Défense renforcée (ac%).
pub const ARMOR_PERCENT: i32 = 16;
/// `item_maxdamage_percent` (id 17) — Dégâts renforcés (ED, dmg%).
pub const MAXDAMAGE_PERCENT: i32 = 17;
/// `tohit` (id 19) — Toucher (AR) plat.
pub const TOHIT: i32 = 19;
/// `toblock` (id 20) — chances de blocage.
pub const TOBLOCK: i32 = 20;
/// `mindamage` (id 21) — dégâts min (final dérivé côté équipement).
pub const MINDAMAGE: i32 = 21;
/// `maxdamage` (id 22) — dégâts max (final dérivé côté équipement).
pub const MAXDAMAGE: i32 = 22;
/// `armorclass` (id 31) — Défense (final dérivé côté équipement).
pub const ARMORCLASS: i32 = 31;
/// `fireresist` (id 39).
pub const FIRERESIST: i32 = 39;
/// `lightresist` (id 41).
pub const LIGHTRESIST: i32 = 41;
/// `coldresist` (id 43).
pub const COLDRESIST: i32 = 43;
/// `poisonresist` (id 45).
pub const POISONRESIST: i32 = 45;
/// `item_goldbonus` (id 79) — Gold Find (gold%).
pub const GOLDBONUS: i32 = 79;
/// `item_magicbonus` (id 80) — Magic Find (mag%).
pub const MAGICBONUS: i32 = 80;

/// Une contribution de stat : id `ItemStatCost` + valeur. C'est l'interface
/// « affix_ids » (id + valeur) que les affixes/uniques/sets produisent et que
/// [`StatList`] agrège.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub struct StatLine {
    /// Id numérique `ItemStatCost` (ex. [`STRENGTH`] = 0).
    pub id: i32,
    /// Valeur (peut être négative, ex. résistances NM/Hell).
    pub value: i64,
}

impl StatLine {
    /// Contribution `(id, valeur)`.
    pub fn new(id: i32, value: i64) -> StatLine {
        StatLine { id, value }
    }
}

impl From<(i32, i64)> for StatLine {
    fn from((id, value): (i32, i64)) -> StatLine {
        StatLine::new(id, value)
    }
}

/// Source de stats : base héros, item équipé, affixes, buff… Toute chose
/// capable de lister ses contributions `(id, valeur)`.
pub trait StatSource {
    /// Les contributions de cette source (ordre indifférent — la map agrège).
    fn stat_lines(&self) -> Vec<StatLine>;
}

impl StatSource for StatList {
    fn stat_lines(&self) -> Vec<StatLine> {
        self.as_lines()
    }
}

impl StatSource for Vec<StatLine> {
    fn stat_lines(&self) -> Vec<StatLine> {
        self.clone()
    }
}

impl StatSource for &[StatLine] {
    fn stat_lines(&self) -> Vec<StatLine> {
        self.to_vec()
    }
}

impl StatSource for [StatLine] {
    fn stat_lines(&self) -> Vec<StatLine> {
        self.to_vec()
    }
}

impl<const N: usize> StatSource for [StatLine; N] {
    fn stat_lines(&self) -> Vec<StatLine> {
        self.to_vec()
    }
}

/// Liste de stats agrégée par id — la moitié « moteur » du socle A1 :
/// `hero = base + Σ items` est une somme de [`StatSource`].
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StatList {
    /// Map id → valeur agrégée (BTreeMap : itération déterministe par id).
    entries: BTreeMap<i32, i64>,
}

impl StatList {
    /// Liste vide.
    pub fn new() -> StatList {
        StatList { entries: BTreeMap::new() }
    }

    /// Agrège n'importe quel nombre de sources (base héros + items + …).
    pub fn from_sources<S: StatSource>(sources: impl IntoIterator<Item = S>) -> StatList {
        let mut out = StatList::new();
        for s in sources {
            for line in s.stat_lines() {
                out.add(line.id, line.value);
            }
        }
        out
    }

    /// Ajoute `value` à la stat `id` (crée l'entrée si absente).
    pub fn add(&mut self, id: i32, value: i64) {
        *self.entries.entry(id).or_insert(0) += value;
    }

    /// Ajoute toutes les lignes d'une source.
    pub fn add_lines(&mut self, lines: &[StatLine]) {
        for l in lines {
            self.add(l.id, l.value);
        }
    }

    /// Fusionne une autre liste (somme entrée à entrée).
    pub fn merge(&mut self, other: &StatList) {
        for (id, v) in &other.entries {
            self.add(*id, *v);
        }
    }

    /// Écrit la valeur exacte d'une stat (remplace — usage dérivation).
    pub fn set(&mut self, id: i32, value: i64) {
        self.entries.insert(id, value);
    }

    /// Valeur agrégée d'une stat (0 si absente).
    pub fn get(&self, id: i32) -> i64 {
        self.entries.get(&id).copied().unwrap_or(0)
    }

    /// Retire une stat, renvoie sa valeur (0 si absente).
    pub fn remove(&mut self, id: i32) -> i64 {
        self.entries.remove(&id).unwrap_or(0)
    }

    /// Nombre d'entrées.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Liste vide ?
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Itère `(id, valeur)` trié par id (déterministe).
    pub fn iter(&self) -> impl Iterator<Item = (i32, i64)> + '_ {
        self.entries.iter().map(|(k, v)| (*k, *v))
    }

    /// Copie plate en lignes `(id, valeur)` (interface affixes).
    pub fn as_lines(&self) -> Vec<StatLine> {
        self.iter().map(|(id, value)| StatLine { id, value }).collect()
    }

    // --- Accesseurs typés (stats courantes) ---
    /// Force totale (bonus héros + items).
    pub fn strength(&self) -> i64 {
        self.get(STRENGTH)
    }
    /// Énergie totale.
    pub fn energy(&self) -> i64 {
        self.get(ENERGY)
    }
    /// Dextérité totale.
    pub fn dexterity(&self) -> i64 {
        self.get(DEXTERITY)
    }
    /// Vitalité totale.
    pub fn vitality(&self) -> i64 {
        self.get(VITALITY)
    }
    /// Bonus vie.
    pub fn hp(&self) -> i64 {
        self.get(HITPOINTS)
    }
    /// Vie max (stat `maxhp`, ValShift retail).
    pub fn max_hp(&self) -> i64 {
        self.get(MAXHP)
    }
    /// Bonus mana.
    pub fn mana(&self) -> i64 {
        self.get(MANA)
    }
    /// Mana max (stat `maxmana`).
    pub fn max_mana(&self) -> i64 {
        self.get(MAXMANA)
    }
    /// Bonus endurance.
    pub fn stamina(&self) -> i64 {
        self.get(STAMINA)
    }
    /// Endurance max (stat `maxstamina`).
    pub fn max_stamina(&self) -> i64 {
        self.get(MAXSTAMINA)
    }
    /// Niveau (stat `level`).
    pub fn level(&self) -> i64 {
        self.get(LEVEL)
    }
    /// Défense (AC) — final dérivé côté [`crate::equipment`].
    pub fn armor_class(&self) -> i64 {
        self.get(ARMORCLASS)
    }
    /// Défense % (ac% brut, affichage).
    pub fn ac_pct(&self) -> i64 {
        self.get(ARMOR_PERCENT)
    }
    /// Toucher (AR) plat.
    pub fn attack_rating(&self) -> i64 {
        self.get(TOHIT)
    }
    /// Dégâts min de l'arme en main (final dérivé).
    pub fn dmg_min(&self) -> i64 {
        self.get(MINDAMAGE)
    }
    /// Dégâts max de l'arme en main (final dérivé).
    pub fn dmg_max(&self) -> i64 {
        self.get(MAXDAMAGE)
    }
    /// Dégâts % (ED brut, affichage).
    pub fn edmg_pct(&self) -> i64 {
        self.get(MAXDAMAGE_PERCENT)
    }
    /// Chances de blocage.
    pub fn block(&self) -> i64 {
        self.get(TOBLOCK)
    }
    /// Résistance au feu.
    pub fn res_fire(&self) -> i64 {
        self.get(FIRERESIST)
    }
    /// Résistance à la foudre.
    pub fn res_lightning(&self) -> i64 {
        self.get(LIGHTRESIST)
    }
    /// Résistance au froid.
    pub fn res_cold(&self) -> i64 {
        self.get(COLDRESIST)
    }
    /// Résistance au poison.
    pub fn res_poison(&self) -> i64 {
        self.get(POISONRESIST)
    }
    /// Magic Find (mag%).
    pub fn magic_find(&self) -> i64 {
        self.get(MAGICBONUS)
    }
    /// Gold Find (gold%).
    pub fn gold_find(&self) -> i64 {
        self.get(GOLDBONUS)
    }

    /// Borne les stats non signées du moteur de combat à ≥ 0 (AC, dmg, AR) —
    /// les résistances signées ne sont PAS touchées (NM/Hell négatifs réels).
    pub fn clamp_combat(&mut self) {
        for id in [ARMORCLASS, MINDAMAGE, MAXDAMAGE, TOHIT] {
            let v = self.get(id);
            if v < 0 {
                self.set(id, 0);
            }
        }
    }
}

/// Pont [`crate::item_props::StatProp`] → lignes `(id, valeur)`. Une variante
/// produit au moins une ligne ; `res-all` se répartit sur les 4 résistances
/// (sémantique D2) ; les variantes non mappées (skills, lifesteal…) renvoient
/// une liste vide (elles restent portées par les props roulées).
pub fn prop_stat_lines(prop: &crate::item_props::StatProp) -> Vec<StatLine> {
    use crate::item_props::StatProp as P;
    let one = |id: i32, v: i32| vec![StatLine::new(id, v as i64)];
    match *prop {
        P::Ac(v) => one(ARMORCLASS, v),
        P::AcPct(v) => one(ARMOR_PERCENT, v),
        P::EdamPct(v) => one(MAXDAMAGE_PERCENT, v),
        P::DmgMin(v) => one(MINDAMAGE, v),
        P::DmgMax(v) => one(MAXDAMAGE, v),
        P::Str(v) => one(STRENGTH, v),
        P::Dex(v) => one(DEXTERITY, v),
        P::Vit(v) => one(VITALITY, v),
        P::Enr(v) => one(ENERGY, v),
        P::Life(v) => one(HITPOINTS, v),
        P::Mana(v) => one(MANA, v),
        P::ResFire(v) => one(FIRERESIST, v),
        P::ResCold(v) => one(COLDRESIST, v),
        P::ResLightning(v) => one(LIGHTRESIST, v),
        P::ResPoison(v) => one(POISONRESIST, v),
        // res-all : +X % aux QUATRE résistances (D2 réel).
        P::ResAll(v) => vec![
            StatLine::new(FIRERESIST, v as i64),
            StatLine::new(LIGHTRESIST, v as i64),
            StatLine::new(COLDRESIST, v as i64),
            StatLine::new(POISONRESIST, v as i64),
        ],
        P::MagicFind(v) => one(MAGICBONUS, v),
        P::GoldFind(v) => one(GOLDBONUS, v),
        // Convention MVP (héritée de inventory.rs) : att et att% cumulent le
        // même accumulateur AR ; le % reste distinguable dans les props.
        P::AttackRating(v) | P::AttackRatingPct(v) => one(TOHIT, v),
        _ => Vec::new(),
    }
}

/// Convertit une liste de props roulées (affixes d'un item) en lignes
/// `(id, valeur)` — le format « affix_ids » consommé par [`StatList`].
pub fn props_stat_lines(props: &[crate::item_props::RolledProp]) -> Vec<StatLine> {
    let mut out = Vec::new();
    for p in props {
        if let Some(typed) = &p.typed {
            out.extend(prop_stat_lines(typed));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests — ids revérifiés contre la table réelle v1.12 + agrégation pures.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_props::{PropDescriptor, RolledProp};

    /// GOLDEN : chaque constante d'id existe bien dans le dictionnaire
    /// `ItemStatCost.txt` v1.12 embarqué avec EXACTEMENT cet id.
    #[test]
    fn ids_match_itemstatcost_v112() {
        let t = crate::stats::StatCostTable::global();
        let cases: [(&str, i32); 24] = [
            ("strength", STRENGTH),
            ("energy", ENERGY),
            ("dexterity", DEXTERITY),
            ("vitality", VITALITY),
            ("hitpoints", HITPOINTS),
            ("maxhp", MAXHP),
            ("mana", MANA),
            ("maxmana", MAXMANA),
            ("stamina", STAMINA),
            ("maxstamina", MAXSTAMINA),
            ("level", LEVEL),
            ("item_armor_percent", ARMOR_PERCENT),
            ("item_maxdamage_percent", MAXDAMAGE_PERCENT),
            ("tohit", TOHIT),
            ("toblock", TOBLOCK),
            ("mindamage", MINDAMAGE),
            ("maxdamage", MAXDAMAGE),
            ("armorclass", ARMORCLASS),
            ("fireresist", FIRERESIST),
            ("lightresist", LIGHTRESIST),
            ("coldresist", COLDRESIST),
            ("poisonresist", POISONRESIST),
            ("item_goldbonus", GOLDBONUS),
            ("item_magicbonus", MAGICBONUS),
        ];
        for (name, id) in cases {
            let rec = t.get(name).unwrap_or_else(|| panic!("{name} absent du dictionnaire"));
            assert_eq!(rec.id, id, "{name} : id table {} ≠ constante {id}", rec.id);
        }
    }

    /// Agrégation base héros + items = somme des sources (BA niv 1 charstats :
    /// str 30, dex 20, ene 10, vit 25 — lib.rs §données réelles) + anneau.
    #[test]
    fn aggregation_hero_base_plus_items() {
        let base: Vec<StatLine> = vec![
            StatLine::new(STRENGTH, 30),
            StatLine::new(DEXTERITY, 20),
            StatLine::new(ENERGY, 10),
            StatLine::new(VITALITY, 25),
            StatLine::new(HITPOINTS, 55),
            StatLine::new(MANA, 15),
        ];
        let ring: Vec<StatLine> = vec![
            StatLine::new(STRENGTH, 5),
            StatLine::new(MAGICBONUS, 15),
        ];
        let total = StatList::from_sources([base, ring]);
        assert_eq!(total.strength(), 35);
        assert_eq!(total.dexterity(), 20);
        assert_eq!(total.energy(), 10);
        assert_eq!(total.vitality(), 25);
        assert_eq!(total.hp(), 55);
        assert_eq!(total.mana(), 15);
        assert_eq!(total.magic_find(), 15);
        assert_eq!(total.gold_find(), 0);
        assert_eq!(total.len(), 7, "une entrée par id distinct");
    }

    /// add/set/remove/clamp : sémantique de la map, déterminisme d'itération.
    #[test]
    fn map_semantics() {
        let mut l = StatList::new();
        l.add(ARMORCLASS, 10);
        l.add(ARMORCLASS, -30);
        assert_eq!(l.armor_class(), -20);
        l.clamp_combat();
        assert_eq!(l.armor_class(), 0, "AC borné ≥ 0");
        l.add(FIRERESIST, -40);
        l.clamp_combat();
        assert_eq!(l.res_fire(), -40, "résistances signées non bornées (NM/Hell)");
        l.set(MINDAMAGE, 7);
        assert_eq!(l.dmg_min(), 7);
        assert_eq!(l.remove(MINDAMAGE), 7);
        assert_eq!(l.dmg_min(), 0);
        // itération triée par id (déterministe)
        let mut other = StatList::new();
        other.add(MAGICBONUS, 1);
        other.add(STRENGTH, 2);
        let seq: Vec<i32> = other.iter().map(|(id, _)| id).collect();
        assert_eq!(seq, vec![STRENGTH, MAGICBONUS]);
        // as_lines ↔ from_sources round-trip
        let lines = other.as_lines();
        let again = StatList::from_sources([&lines[..], &lines[..]]);
        assert_eq!(again.strength(), 4);
        assert_eq!(again.magic_find(), 2);
    }

    fn prop(code: &str, v: i32) -> RolledProp {
        let desc = PropDescriptor { code: code.into(), param: String::new(), min: v, max: v };
        RolledProp { desc: desc.clone(), value: v, typed: crate::item_props::interpret(&desc, v) }
    }

    /// Pont props → ids : res-all éclate sur 4 résistances, code inconnu ⇒ vide.
    #[test]
    fn bridge_from_rolled_props() {
        let lines = props_stat_lines(&[prop("str", 5), prop("res-all", 10), prop("nofreeze", 1)]);
        let l = StatList::from_sources([lines]);
        assert_eq!(l.strength(), 5);
        assert_eq!(l.res_fire(), 10);
        assert_eq!(l.res_lightning(), 10);
        assert_eq!(l.res_cold(), 10);
        assert_eq!(l.res_poison(), 10);
        assert_eq!(l.len(), 5, "nofreeze ignoré (non mappé)");
    }
}
