//! État des compétences du héros — équivalent `d2core/d2hero/hero_skill.go`
//! (MVP : skills de DÉPART uniquement, un point de sélection par classe).
//!
//! # Données réelles (`assets/patch_skills.txt` v1.12)
//! * `Fire Bolt` (Id 36, `charclass=sor`, `srvmissile=firebolt`, `EType=fire`,
//!   `h2h=none`) — skill de départ sorcière, niv 1.
//! * `Bash` (Id 126, `charclass=bar`, mêlée `h2h`, aucun `srvmissile`) —
//!   skill de départ barbare, niv 1.
//! * `Sacrifice` (Id 96, `charclass=pal`, mêlée `h2h`, aucun `srvmissile`) —
//!   skill de départ paladin, niv 1.
//!
//! Les dégâts/élément des bolts vivent dans la table `d2missiles`
//! (`dmgSrc=Skill` : firebolt 3–8 feu) — ce module ne porte QUE l'état
//! (code, niveau, missile), conformément à la frontière des crates.
//! Le missile du skill actif alimente `hero_ranged_attack` (world.rs) ;
//! un skill sans missile (mêlée) retombe sur `arrow`.

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

/// Skills de départ réels par classe (`hero_state_factory.go:111-148` :
/// une compétence de classe niv 1 ; classes non couvertes ⇒ vide — le tir
/// garde `arrow` en fallback).
pub(crate) fn starting_skills(hero_code: &str) -> Vec<HeroSkill> {
    match hero_code.to_ascii_uppercase().as_str() {
        // sor : Fire Bolt → srvmissile `firebolt` (3–8 feu, table d2missiles)
        "SOR" => vec![HeroSkill {
            code: "Fire Bolt".into(),
            level: 1,
            missile: Some("firebolt".into()),
        }],
        // bar : Bash — mêlée pure (`h2h`), bonus dégâts non câblés au MVP
        "BA" => vec![HeroSkill { code: "Bash".into(), level: 1, missile: None }],
        // pal : Sacrifice — mêlée pure (`h2h`), % dégâts non câblés au MVP
        "PA" => vec![HeroSkill {
            code: "Sacrifice".into(),
            level: 1,
            missile: None,
        }],
        _ => Vec::new(),
    }
}

/// Index du skill sélectionné par défaut : la première (et seule au MVP)
/// compétence de départ — SOR tire `firebolt`, BA/PA attaque en mêlée.
pub(crate) fn default_active(hero_code: &str) -> usize {
    let _ = hero_code; // sélection uniforme au MVP (une compétence par classe)
    0
}
