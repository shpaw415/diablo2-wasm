//! État des compétences du héros — équivalent `d2core/d2hero/hero_skill.go`
//! (MVP : skills de DÉPART uniquement, un point de sélection par classe).
//!
//! # Données réelles (`assets/patch_charstats.txt` v1.12, colonne `StartSkill`)
//! La table fait foi : seules DEUX classes démarrent avec une compétence.
//! * Sorceress → `fire bolt` = `Fire Bolt` (Id 36, `charclass=sor`,
//!   `srvmissile=firebolt`, `EType=fire`, niv 1).
//! * Necromancer → `raise skeleton` = `Raise Skeleton` (Id 70,
//!   `charclass=nec`, invocation sans `srvmissile` — repli mêlée/flèche du
//!   moteur).
//! * Amazon, Paladin, Barbarian, Druid, Assassin → `StartSkill` VIDE : aucune
//!   compétence de classe au démarrage (l'hypothèse MVP initiale Bash/Sacrifice
//!   n'est PAS dans la table — corrigée ici).
//!
//! Les dégâts/élément des bolts vivent dans la table `d2missiles`
//! (`dmgSrc=Skill` : firebolt 3–8 feu) — ce module ne porte QUE l'état
//! (code, niveau, missile), conformément à la frontière des crates.
//! Le missile du skill actif alimente `hero_ranged_attack` (world.rs) ;
//! un skill sans missile (mêlée/invocation) ou AUCUN skill retombe sur
//! `arrow` transportant les dégâts d'arme.

use serde::{Deserialize, Serialize};

/// Compétence connue du héros (état minimal : code table + niveau + missile).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeroSkill {
    /// Code `skills.txt` (colonne `skill`, ex. `"Fire Bolt"`).
    pub code: String,
    /// Niveau de la compétence (1 aux skills de départ).
    pub level: u32,
    /// Missile `srvmissile` tiré (id `missiles.txt`, ex. `"firebolt"`) —
    /// `None` = compétence de mêlée (`h2h`), aucun projectile.
    pub missile: Option<String>,
}

impl HeroSkill {
    /// Compétence de mêlée (pas de projectile) ?
    pub fn is_melee(&self) -> bool {
        self.missile.is_none()
    }
}

/// Skills de départ RÉELS par classe — colonne `StartSkill` de
/// `assets/patch_charstats.txt` v1.12 (cf. doc de module) : SOR = Fire Bolt,
/// NE = Raise Skeleton, les 5 autres classes démarrent SANS compétence
/// (`hero_ranged_attack` replie alors sur `arrow` + dégâts d'arme).
pub(crate) fn starting_skills(hero_code: &str) -> Vec<HeroSkill> {
    match hero_code.to_ascii_uppercase().as_str() {
        // sor : `fire bolt` → srvmissile `firebolt` (3–8 feu, table d2missiles)
        "SOR" => vec![HeroSkill {
            code: "Fire Bolt".into(),
            level: 1,
            missile: Some("firebolt".into()),
        }],
        // nec : `raise skeleton` — invocation, aucun srvmissile (repli mêlée) ;
        // l'effet de summon n'est pas câblé au MVP (état seul, frontière crates)
        "NE" => vec![HeroSkill {
            code: "Raise Skeleton".into(),
            level: 1,
            missile: None,
        }],
        // ama / pal / bar / dru / ass : StartSkill vide dans charstats v1.12
        _ => Vec::new(),
    }
}

/// Index du skill sélectionné par défaut : la première compétence de départ —
/// SOR tire `firebolt`, NE invoque (mêlée au MVP), les 5 autres classes n'ont
/// aucun skill (index ignoré, `active_skill()` = None).
pub(crate) fn default_active(hero_code: &str) -> usize {
    let _ = hero_code; // sélection uniforme au MVP (une compétence par classe)
    0
}
