//! Glue héros ⇄ arbre de compétences — C2/C3/C4 de `finish-systemes.md`
//! (chantier ARBRE DE COMPÉTENCES, 2026-10-23).
//!
//! Pur : aucune dépendance à [`crate::World`] — opère sur la liste
//! [`HeroSkill`] du héros (état existant, déjà persisté dans la save depuis
//! v2) et sur les tables globales [`SkillTreeData`]. Câblé par `world.rs`
//! (blocs additifs) et consommé par d2game.
//!
//! # Règles réelles v1.12 (preuves)
//! * Apprendre = +1 niveau sur un skill de SA classe ([`SkillRecord`],
//!   `patch_skills.txt` col `charclass`), 1 point dépensé par niveau de skill.
//! * Prérequis d'arbre : ≥ 1 point dans chaque `reqskill1-3` (cols 159-161 ;
//!   ex. Fire Ball `reqskill1=Fire Bolt`, ligne Fire Ball).
//! * `reqlevel` (col 153 ; Fire Ball = 12, Meteor = 24) : niveau de PERSO
//!   requis — plafonné au MVP par `MAX_LEVEL` (world.rs, 10).
//! * Plafond d'investissement `maxlvl` (col 154) = 20 pour les 210 skills de
//!   classe ([`SkillTreeData::parse`] : défaut 20 si colonne vide/≤ 0).
//! * Synergies (`EDmgSymPerCalc` col `EDmgSymPerCalc` / `DmgSymPerCalc`) :
//!   formules CalcString évaluées via [`crate::calc`] — ex. Fire Bolt
//!   `(skill('Fire Ball'.blvl)+skill('Meteor'.blvl))*par8` (par8 = 16 ⇒
//!   +16 %/niveau de Fire Ball + Meteor ; Phrozen Keep v1.12).
//! * Base de dégâts du missile = table [`d2missiles`] (`missiles_real.json`,
//!   firebolt 3-8 feu = dégâts officiels niveau 1) — ancre inchangée du
//!   moteur ; la synergie MULTIPLIE cette base (×(1+%/100)). Sans synergie le
//!   comportement est bit-identique à l'existant (zéro régression).
//! * `ELen`/`ELenSymPerCalc` (durées froid/poison), `aurastate`/passives :
//!   EXPOSÉS par [`SkillRecord`] mais NON câblés au MVP (aucun moteur
//!   d'états/buffs — C6/P3) ; aucune durée d'effet à scaler côté missiles.

use std::collections::HashMap;

use crate::skills::HeroSkill;
use crate::skills_tree::{SkillLevels, SkillRecord, SkillTree, SkillTreeData, SkillTreeError, SynergyKind};

/// Code de classe `skills.txt` (col `charclass`) depuis le code héros du
/// manifest (`assets/manifest.json` `heroes`: `BA`, `SOR`, `PA`, `AM`, `AS`,
/// `DZ`, `NE`) — insensible à la casse, aliases complets acceptés ; code
/// inconnu ⇒ minuscule tel quel (le refus vient ensuite de `WrongClass`,
/// jamais de panic).
pub fn class_code(hero_code: &str) -> String {
    match hero_code.trim().to_ascii_uppercase().as_str() {
        "AM" | "AMA" | "AMAZON" => "ama",
        "SOR" | "SO" | "SORCERESS" => "sor",
        "NE" | "NEC" | "NECROMANCER" => "nec",
        "PA" | "PAL" | "PALADIN" => "pal",
        "BA" | "BAR" | "BARBARIAN" => "bar",
        "DZ" | "DRU" | "DRUID" => "dru",
        "AS" | "ASS" | "ASSASSIN" => "ass",
        other => return other.to_ascii_lowercase(),
    }
    .to_string()
}

/// Niveaux du héros vus par les formules ([`SkillLevels`]) : map
/// code-minuscule → points investis, construite depuis la liste du héros.
pub struct HeroLevels<'a> {
    map: HashMap<String, u32>,
    /// Niveau de personnage (résout `ulvl`).
    char_level: u32,
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> HeroLevels<'a> {
    /// Vue depuis les skills connus du héros.
    pub fn new(skills: &'a [HeroSkill], char_level: u32) -> HeroLevels<'a> {
        HeroLevels {
            map: skills
                .iter()
                .map(|s| (s.code.to_ascii_lowercase(), s.level))
                .collect(),
            char_level: char_level.max(1),
            _marker: std::marker::PhantomData,
        }
    }
}

impl SkillLevels for HeroLevels<'_> {
    fn blvl(&self, code: &str) -> u32 {
        self.map.get(&code.to_ascii_lowercase()).copied().unwrap_or(0)
    }
    fn char_level(&self) -> u32 {
        self.char_level
    }
}

/// Apprend (ou monte d'un niveau) le skill `code` pour le héros : valide
/// classe/prérequis/`reqlevel`/plafond `maxlvl`/points disponibles, puis
/// MUTE la liste (incrément, ou création avec le `srvmissile` de la table).
/// Renvoie le NOUVEAU niveau du skill ; `Err` exact (UI arbre) sinon.
///
/// `available_points` = points non dépensés du héros (`HeroStats::skill_points`)
/// — l'appelant décrémente après `Ok` (dépense exactement 1 point).
pub fn learn(
    skills: &mut Vec<HeroSkill>,
    hero_class: &str,
    char_level: u32,
    code: &str,
    available_points: u32,
) -> Result<u32, SkillTreeError> {
    let data = SkillTreeData::global();
    let mut tree = SkillTree::with_data(hero_class, data);
    tree.set_char_level(char_level);
    // Charge les niveaux existants (prérequis et plafond portés par l'état).
    for (c, lvl) in skills.iter().map(|s| (s.code.clone(), s.level)) {
        tree.set_level(&c, lvl);
    }
    tree.grant_points(available_points);
    let new_level = tree.learn(code)?;
    // Sync de la liste du héros : incrément ou création (missile de la table).
    let rec = data.get(code);
    match skills.iter_mut().find(|s| s.code.eq_ignore_ascii_case(code)) {
        Some(s) => s.level = new_level,
        None => skills.push(HeroSkill {
            code: rec.map(|r| r.code.clone()).unwrap_or_else(|| code.to_string()),
            level: new_level,
            missile: rec.and_then(|r| r.srv_missile.clone()),
        }),
    }
    Ok(new_level)
}

/// Type de synergie applicable aux dégâts d'un skill : élémentaire dès que
/// le skill a un `EType` ([`SkillRecord::e_type`]), physique sinon (et pour
/// les carriers d'arme). Kind déterminé depuis la TABLE (preuve par record).
pub fn synergy_kind(rec: &SkillRecord) -> SynergyKind {
    if rec.e_type == crate::skills_tree::ElemType::None {
        SynergyKind::Physical
    } else {
        SynergyKind::Elemental
    }
}

/// Dégâts du missile d'un skill APRÈS synergies : base (dégâts officiels de
/// la table `d2missiles`, niveau 1) × (1 + %synergie/100), arrondi au plancher.
/// `base_*` = `MissileDef::dmg_min/max` du missile du skill ; skill inconnu
/// ⇒ base inchangée (jamais de panic — skill non-table = pas de synergie).
pub fn synergy_scaled_damage(
    skill_code: &str,
    base_min: u32,
    base_max: u32,
    levels: &dyn SkillLevels,
) -> (u32, u32) {
    let data = SkillTreeData::global();
    let Some(rec) = data.get(skill_code) else {
        return (base_min.max(1), base_max.max(base_min.max(1)));
    };
    let pct = data.synergy_percent(rec, synergy_kind(rec), levels);
    let f = 1.0 + pct as f64 / 100.0;
    let mn = ((base_min as f64 * f).floor() as u32).max(1);
    let mx = ((base_max as f64 * f).floor() as u32).max(mn);
    (mn, mx)
}

// ---------------------------------------------------------------------------
// Tests (les golden données réelles : parity_tests.rs / skills_tree.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::skills_tree::NoLevels;

    #[test]
    fn learn_blocks_prereq_reqlevel_and_points() {
        let mut skills: Vec<HeroSkill> = Vec::new();
        // Fire Ball (reqlevel 12, reqskill1=Fire Bolt — patch_skills.txt) :
        // sans prérequis ⇒ MissingPrereq (même avec des points).
        assert_eq!(
            learn(&mut skills, "sor", 30, "Fire Ball", 5),
            Err(SkillTreeError::MissingPrereq {
                skill: "Fire Ball".into(),
                required: "Fire Bolt".into()
            })
        );
        assert!(skills.is_empty(), "état inchangé sur refus");
        // Prérequis appris mais héros niv 1 < reqlevel 12 ⇒ LevelTooLow.
        learn(&mut skills, "sor", 1, "Fire Bolt", 1).unwrap();
        assert_eq!(
            learn(&mut skills, "sor", 1, "Fire Ball", 1),
            Err(SkillTreeError::LevelTooLow { skill: "Fire Ball".into(), required: 12 })
        );
        // 0 point disponible ⇒ NoPoints.
        assert_eq!(learn(&mut skills, "sor", 12, "Fire Ball", 0), Err(SkillTreeError::NoPoints));
        // Tout est réuni ⇒ appris niveau 1 (prérequis non consommé).
        assert_eq!(learn(&mut skills, "sor", 12, "Fire Ball", 1).unwrap(), 1);
        assert_eq!(skills.len(), 2, "Fire Bolt + Fire Ball");
        let fb = skills.iter().find(|s| s.code == "Fire Bolt").unwrap();
        assert_eq!(fb.level, 1, "le prérequis garde son niveau");
        let ball = skills.iter().find(|s| s.code == "Fire Ball").unwrap();
        assert_eq!(ball.missile.as_deref(), Some("fireball"), "srvmissile de la table");
    }

    #[test]
    fn learn_climbs_to_maxlvl_20_and_wrong_class_refused() {
        let mut skills: Vec<HeroSkill> = Vec::new();
        for _ in 0..20 {
            let lvl = learn(&mut skills, "sor", 30, "Fire Bolt", 1).unwrap();
            assert_eq!(lvl, skills.iter().find(|s| s.code == "Fire Bolt").unwrap().level);
        }
        assert_eq!(skills[0].level, 20, "plafond maxlvl 20 (col maxlvl)");
        assert_eq!(
            learn(&mut skills, "sor", 30, "Fire Bolt", 1),
            Err(SkillTreeError::MaxLevel { skill: "Fire Bolt".into(), cap: 20 })
        );
        // Skill d'une autre classe (ou monstre/Attack) ⇒ WrongClass.
        assert!(matches!(
            learn(&mut skills, "sor", 30, "Bash", 1),
            Err(SkillTreeError::WrongClass { .. })
        ));
        // Classe inconnue : aucun skill de classe n'est apprenable.
        assert!(matches!(
            learn(&mut skills, "xxx", 30, "Fire Bolt", 1),
            Err(SkillTreeError::WrongClass { .. })
        ));
    }

    #[test]
    fn hero_levels_feed_formulas() {
        let skills = vec![
            HeroSkill { code: "Fire Bolt".into(), level: 10, missile: Some("firebolt".into()) },
            HeroSkill { code: "Fire Ball".into(), level: 20, missile: Some("fireball".into()) },
        ];
        let lv = HeroLevels::new(&skills, 30);
        assert_eq!(lv.blvl("fire bolt"), 10, "insensible à la casse");
        assert_eq!(lv.blvl("Fire Ball"), 20);
        assert_eq!(lv.blvl("Meteor"), 0);
        assert_eq!(SkillLevels::char_level(&lv), 30);
        // Vide = NoLevels.
        let empty = HeroLevels::new(&[], 1);
        assert_eq!(empty.blvl("Fire Bolt"), 0);
    }

    #[test]
    fn firebolt_synergy_case_reel() {
        // Cas réel du brief : Fire Bolt 10 + Fire Ball 20 + Meteor 20.
        // EDmgSymPerCalc de Fire Bolt = (skill('Fire Ball'.blvl) +
        // skill('Meteor'.blvl)) * par8, par8 = 16 ⇒ (20+20)×16 = 640 % ⇒ ×7,4.
        let skills = vec![
            HeroSkill { code: "Fire Bolt".into(), level: 10, missile: Some("firebolt".into()) },
            HeroSkill { code: "Fire Ball".into(), level: 20, missile: Some("fireball".into()) },
            HeroSkill { code: "Meteor".into(), level: 20, missile: Some("meteor".into()) },
        ];
        let lv = HeroLevels::new(&skills, 30);
        // Base = dégâts officiels niveau 1 de la table d2missiles (3-8 feu).
        let (mn, mx) = synergy_scaled_damage("Fire Bolt", 3, 8, &lv);
        assert_eq!((mn, mx), (22, 59), "3×7.4=22.2⇒22, 8×7.4=59.2⇒59");
        // Sans synergie (héros vierge) : base INCHANGÉE (zéro régression).
        let (mn, mx) = synergy_scaled_damage("Fire Bolt", 3, 8, &NoLevels);
        assert_eq!((mn, mx), (3, 8));
        // Skill hors table : base inchangée, jamais de panic.
        let (mn, mx) = synergy_scaled_damage("N'importe Quoi", 3, 8, &lv);
        assert_eq!((mn, mx), (3, 8));
        // Synergie PHYSIQUE : Bash n'a pas de EDmgSymPerCalc/ETypé ⇒ ×1.
        let (mn, mx) = synergy_scaled_damage("Bash", 3, 8, &lv);
        assert_eq!((mn, mx), (3, 8));
    }

    #[test]
    fn synergy_kind_follows_etype() {
        let d = SkillTreeData::global();
        assert_eq!(synergy_kind(d.get("Fire Bolt").unwrap()), SynergyKind::Elemental);
        assert_eq!(synergy_kind(d.get("Bash").unwrap()), SynergyKind::Physical);
    }

    #[test]
    fn class_codes_match_manifest_heroes() {
        // Clés `heroes` du manifest : BA, SOR, PA, AM, AS, DZ, NE — toutes
        // mappent vers un `charclass` skills.txt existant (30 skills/classe).
        for (hero, class) in [
            ("BA", "bar"), ("SOR", "sor"), ("PA", "pal"), ("AM", "ama"),
            ("AS", "ass"), ("DZ", "dru"), ("NE", "nec"),
            ("ba", "bar"), (" nec ", "nec"), ("Barbarian", "bar"),
        ] {
            assert_eq!(class_code(hero), class, "mapping héros→classe de {hero}");
            let d = SkillTreeData::global();
            assert_eq!(d.class_indices(class).len(), 30, "{class} existe dans la table");
        }
        // Inconnu : jamais de panic, refus par WrongClass côté learn.
        assert_eq!(class_code("ZZZ"), "zzz");
    }
}
