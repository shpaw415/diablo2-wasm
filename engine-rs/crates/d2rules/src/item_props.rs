//! `item_props` — descripteurs de propriétés d'items (affixes, uniques, sets).
//!
//! Port Rust de `d2records/property_descriptor.go` (Code/Param/Min/Max) :
//! chaque colonne `propN`/`parN`/`minN`/`maxN` des tables v1.12
//! (`uniqueitems`, `setitems`, `sets`, `magicprefix`, `magicsuffix`,
//! `automagic`) devient un [`PropDescriptor`] brut. Le tirage de la valeur
//! (`min..=max`, D2 tire à la GÉNÉRATION de l'item) produit un
//! [`RolledProp`] portant l'interprétation typée [`StatProp`] — le socle
//! `Stat`/`ItemStatCost` (chantier d'un autre membre) consommera cette liste.
//!
//! Codes couverts en typé : le vocabulaire réel des tables v1.12
//! (dmg%, ac%, ac, dmg-min/max, str/dex/vit/enr, hp, mana, res-*,
//! mag%, gold%, att/att%, lifesteal…). Code inconnu ⇒ [`StatProp::Generic`]
//! (jamais perdu, juste non interprété).
//!
//! Zéro IO, zéro panique : cellule absente ⇒ `None`.

use crate::rng::Rng;

/// Descripteur brut d'une propriété (port direct `property_descriptor.go`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PropDescriptor {
    /// Code propriété (`properties.txt` v1.12 : `dmg%`, `ac%`, `res-fire`…).
    pub code: String,
    /// Paramètre brut (id de skill, de state… ; souvent vide).
    pub param: String,
    /// Borne basse du tirage.
    pub min: i32,
    /// Borne haute du tirage.
    pub max: i32,
}

/// Valeur tirée d'un descripteur + interprétation typée.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RolledProp {
    /// Descripteur source (pour la re-génération et l'audit).
    pub desc: PropDescriptor,
    /// Valeur tirée dans `min..=max` (déterministe au seed du [`Rng`]).
    pub value: i32,
    /// Interprétation typée ([`None`] = code non couvert, reste en `Generic`).
    pub typed: Option<StatProp>,
}

/// Liste typée des stats d'item (socle du futur `Stat`/`ItemStatCost`).
/// Les variantes couvrent les codes réellement présents dans les tables
/// v1.12 embarquées ; tout le reste tombe dans [`StatProp::Generic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StatProp {
    /// Défense plate (`ac`).
    Ac(i32),
    /// Défense % (`ac%`, affixes « Sturdy »… — s'applique à la base AC).
    AcPct(i32),
    /// Dégâts % (`dmg%` — Enhanced Damage, arme en main).
    EdamPct(i32),
    /// Dégâts min plats (`dmg-min`).
    DmgMin(i32),
    /// Dégâts max plats (`dmg-max`).
    DmgMax(i32),
    /// Force (`str`).
    Str(i32),
    /// Dextérité (`dex`).
    Dex(i32),
    /// Vitalité (`vit`).
    Vit(i32),
    /// Énergie (`enr`).
    Enr(i32),
    /// Vie max (`hp`).
    Life(i32),
    /// Mana max (`mana`).
    Mana(i32),
    /// Résistance feu (`res-fire`).
    ResFire(i32),
    /// Résistance froid (`res-cold`).
    ResCold(i32),
    /// Résistance foudre (`res-ltng`).
    ResLightning(i32),
    /// Résistance poison (`res-pois`).
    ResPoison(i32),
    /// Résistance à tout (`res-all`).
    ResAll(i32),
    /// Magic Find (`mag%`).
    MagicFind(i32),
    /// Gold Find (`gold%`).
    GoldFind(i32),
    /// Toucher plat (`att`).
    AttackRating(i32),
    /// Toucher % (`att%`).
    AttackRatingPct(i32),
    /// Vol de vie % (`lifesteal`).
    LifeSteal(i32),
    /// Vol de mana % (`manasteal`).
    ManaSteal(i32),
    /// +skills tous (`allskills`).
    AllSkills(i32),
    /// +skill unique (param = code skill, `skill`).
    Skill,
    /// +skills d'un onglet (param = 1..6, `skilltab`).
    SkillTab,
    /// Durabilité (`dur`).
    Durability(i32),
    /// Rayon de lumière (`light`).
    LightRadius(i32),
    /// Code non interprété (resté brut, jamais perdu).
    Generic,
}

/// Tire la valeur d'un descripteur (`min..=max` inclusif, ordre D2 : à la
/// génération) et résout l'interprétation typée.
pub fn roll(desc: &PropDescriptor, rng: &mut Rng) -> RolledProp {
    let (lo, hi) = (desc.min.min(desc.max), desc.min.max(desc.max));
    let value = rng.range_i32(lo, hi);
    let typed = interpret(desc, value);
    RolledProp { desc: desc.clone(), value, typed }
}

/// Interprétation typée d'un descripteur avec sa valeur déjà tirée.
/// `param` peut porter l'identifiant (skill/onglet) — conservé dans le tuple
/// pour les codes paramétrés.
pub fn interpret(desc: &PropDescriptor, value: i32) -> Option<StatProp> {
    use StatProp::*;
    Some(match desc.code.as_str() {
        "ac" => Ac(value),
        "ac%" => AcPct(value),
        "dmg%" => EdamPct(value),
        "dmg-min" => DmgMin(value),
        "dmg-max" => DmgMax(value),
        "str" => Str(value),
        "dex" => Dex(value),
        "vit" => Vit(value),
        "enr" => Enr(value),
        "hp" => Life(value),
        "mana" => Mana(value),
        "res-fire" => ResFire(value),
        "res-cold" => ResCold(value),
        "res-ltng" => ResLightning(value),
        "res-pois" => ResPoison(value),
        "res-all" => ResAll(value),
        "mag%" => MagicFind(value),
        "gold%" => GoldFind(value),
        "att" => AttackRating(value),
        "att%" => AttackRatingPct(value),
        "lifesteal" => LifeSteal(value),
        "manasteal" => ManaSteal(value),
        "allskills" => AllSkills(value),
        "skill" => Skill,
        "skilltab" => SkillTab,
        "dur" => Durability(value),
        "light" => LightRadius(value),
        _ => return None,
    })
}

/// Lit une cellule `propN`/`parN`/`minN`/`maxN` ; `(code vide ou "0")` ⇒ None.
pub(crate) fn descriptor(cells: [&str; 4]) -> Option<PropDescriptor> {
    let code = cells[0].trim();
    if code.is_empty() || code == "0" {
        return None;
    }
    let parse = |s: &str| s.trim().parse::<i32>().unwrap_or(0);
    Some(PropDescriptor {
        code: code.to_string(),
        param: cells[1].trim().to_string(),
        min: parse(cells[2]),
        max: parse(cells[3]),
    })
}

// ---------------------------------------------------------------------------
// Tests — tables réelles embarquées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// Le tirage est inclusif et déterministe au seed.
    #[test]
    fn roll_is_deterministic_and_bounded() {
        let d = PropDescriptor {
            code: "ac%".into(),
            param: String::new(),
            min: 20,
            max: 30,
        };
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..50 {
            let ra = roll(&d, &mut a);
            let rb = roll(&d, &mut b);
            assert_eq!(ra, rb);
            assert!((20..=30).contains(&ra.value));
            assert_eq!(ra.typed, Some(StatProp::AcPct(ra.value)));
        }
    }

    /// Valeurs fixes (min == max, ex. Gull `dmg-max 15 15`) et bornes inversées.
    #[test]
    fn roll_fixed_and_inverted_bounds() {
        let mut rng = Rng::new(7);
        let fixed = PropDescriptor {
            code: "dmg-max".into(),
            param: String::new(),
            min: 15,
            max: 15,
        };
        assert_eq!(roll(&fixed, &mut rng).value, 15);
        let inv = PropDescriptor {
            code: "hp".into(),
            param: String::new(),
            min: 25,
            max: 10,
        };
        let r = roll(&inv, &mut rng);
        assert!((10..=25).contains(&r.value), "bornes inversées gérées");
    }

    /// Codes inconnus ⇒ `Generic` (valeur conservée), cellules vides ⇒ None.
    #[test]
    fn unknown_codes_fall_back_to_generic() {
        assert_eq!(interpret(&desc("nofreeze", 1), 1), None);
        let d = desc("dmg-undead", 200);
        let mut rng = Rng::new(1);
        assert_eq!(roll(&d, &mut rng).typed, None);
        assert!(descriptor(["", "", "1", "2"]).is_none());
        assert!(descriptor(["0", "", "1", "2"]).is_none());
    }

    /// Le vocabulaire principal des tables v1.12 est bien typé.
    #[test]
    fn real_v112_codes_are_typed() {
        let cases = [
            ("dmg%", StatProp::EdamPct(0)),
            ("ac%", StatProp::AcPct(0)),
            ("ac", StatProp::Ac(0)),
            ("str", StatProp::Str(0)),
            ("res-all", StatProp::ResAll(0)),
            ("mag%", StatProp::MagicFind(0)),
            ("lifesteal", StatProp::LifeSteal(0)),
            ("skilltab", StatProp::SkillTab),
        ];
        for (code, want) in cases {
            assert_eq!(interpret(&desc(code, 0), 0), Some(want), "code {code}");
        }
    }

    fn desc(code: &str, v: i32) -> PropDescriptor {
        PropDescriptor { code: code.into(), param: String::new(), min: v, max: v }
    }
}
