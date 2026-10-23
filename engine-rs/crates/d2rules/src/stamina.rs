//! Endurance (stamina) — EXP-H2 de l'audit `finish-experience.md`.
//!
//! La valeur `stamina` existe dans le manifest (`heroes[code].stats.stamina`,
//! 92 pour le barbare — `patch_charstats.txt` v1.12) mais n'était consommée
//! nulle part : la course était gratuite. Ce module ajoute le drain/régénération
//! CÔTÉ RÈGLES, de façon **additive** (aucun comportement existant modifié en
//! dehors du drain lui-même) :
//!
//! * courir (`run == true`) ET se déplacer épuise l'endurance ;
//! * marcher ou rester immobile régénère (la course à l'arrêt ne draine pas —
//!   même règle que D2 : seul le déplacement en course coûte) ;
//! * à 0, la course est coupée côté appelant ([`World`] remet `run = false`,
//!   comme D2 où le héros repasse en marche ; il faut relâcher/rappuyer Maj) ;
//! * la régénération reprend dès que l'on ne court plus.
//!
//! # Écarts assumés (MVP, constants visibles pour réglage)
//! * D2 module finement le drain (vitesse de course, poids…) ; ici deux taux
//!   constants, calés pour que le barbare (92) tienne ~23 s de course et
//!   régénère plein en ~9 s au repos — l'ordre de grandeur D2 bas niveau.
//! * L'accumulateur sous-unitaire (`milli`) est TRANSITAIRE (jamais sauvegardé)
//!   : la valeur affichée [`HeroStats::stamina`] est persistée, les saves v2
//!   antérieures à ce module chargent avec `stamina = max_stamina = 0` puis
//!   sont reconciliées au gabarit du manifest (`World::reconcile_stamina`,
//!   appelé par `save::load_json`).
//!
//! Zéro IO, fonctions pures testables : [`tick_stamina`] porte toute la règle.

/// Taux de DRAIN en course : milli-points d'endurance par milliseconde.
/// 4.0 milli/ms = 4 points/s ⇒ 92 points ≈ 23 s de course continue (BA).
pub const STAMINA_DRAIN_MILLI_PER_MS: i64 = 4;
/// Taux de RÉGÉNÉRATION : milli-points par milliseconde hors course.
/// 11.0 milli/ms = 11 points/s ⇒ plein en ~8,4 s (ordre de grandeur D2).
pub const STAMINA_REGEN_MILLI_PER_MS: i64 = 11;

/// Un pas d'endurance (pur). `milli` est l'accumulateur sous-unitaire
/// (1000 milli = 1 point, clampé [0, max×1000]) ; `stamina` la valeur
/// affichée/persistée, mise à jour à la fin. Renvoie `true` tant que le héros
/// court ET se déplace AVEC une endurance à zéro — l'appelant ([`World`]) en
/// fait un forçage IDROPOTENT de la marche (`run = false`, comme D2 où le
/// héros épuisé repasse en marche ; Maj relancera la course dès qu'il y a de
/// nouveau de l'endurance).
///
/// * `running && moving` → drain ; tout le reste → régénération.
/// * `max_stamina == 0` (sauvegarde ancienne non reconciliée) → aucun effet
///   (ne divise pas par zéro, ne produit pas d'épuisement fantôme).
pub fn tick_stamina(
    running: bool,
    moving: bool,
    dt_ms: u32,
    max_stamina: u32,
    milli: &mut i64,
    stamina: &mut u32,
) -> bool {
    if max_stamina == 0 {
        return false;
    }
    let max_milli = (max_stamina as i64) * 1000;
    if running && moving {
        *milli -= (dt_ms as i64) * STAMINA_DRAIN_MILLI_PER_MS;
        if *milli < 0 {
            *milli = 0;
        }
    } else {
        *milli += (dt_ms as i64) * STAMINA_REGEN_MILLI_PER_MS;
        if *milli > max_milli {
            *milli = max_milli;
        }
    }
    *stamina = (*milli / 1000) as u32;
    running && moving && *stamina == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 1 s de course draine ~4 points ; la régénération clamp au max.
    #[test]
    fn drain_and_regen_rates() {
        let mut milli = 92_000;
        let mut stamina = 92;
        assert!(!tick_stamina(true, true, 1000, 92, &mut milli, &mut stamina));
        assert_eq!(stamina, 88, "4 points/s en course");
        assert!(!tick_stamina(false, false, 1000, 92, &mut milli, &mut stamina));
        assert_eq!(stamina, 92, "régénération clampée au max");
        assert_eq!(milli, 92_000, "accumulateur clampé à max×1000");
    }

    /// La course sur place ne draine pas (seul le déplacement coûte).
    #[test]
    fn running_in_place_does_not_drain() {
        let mut milli = 92_000;
        let mut stamina = 92;
        tick_stamina(true, false, 5000, 92, &mut milli, &mut stamina);
        assert_eq!(stamina, 92, "course immobile = régénération (clamp au max)");
    }

    /// Épuisement : à zéro en course, le signal reste `true` (l'appelant
    /// force la marche de façon idempotente à chaque tick à zéro).
    #[test]
    fn exhaustion_signal_while_at_zero() {
        let mut milli = 1_000; // 1 point exactement
        let mut stamina = 1;
        assert!(tick_stamina(true, true, 300, 92, &mut milli, &mut stamina), "épuisé ce tick");
        assert_eq!(stamina, 0);
        assert!(tick_stamina(true, true, 300, 92, &mut milli, &mut stamina), "toujours à zéro en course");
        // et la régénération repart dès qu'on ne court plus.
        assert!(!tick_stamina(false, true, 1000, 92, &mut milli, &mut stamina));
        assert!(stamina > 0, "régénéré en marchant");
    }

    /// `max_stamina == 0` (vieille save) : no-op sans panique.
    #[test]
    fn zero_max_is_a_noop() {
        let mut milli = 0;
        let mut stamina = 0;
        assert!(!tick_stamina(true, true, 1000, 0, &mut milli, &mut stamina));
        assert_eq!((milli, stamina), (0, 0));
    }

    /// Intégration [`World`] : la course drainée finit par couper `run`,
    /// le repos régénère, et le respawn remet plein.
    #[test]
    fn world_run_drains_then_regens() {
        use crate::PlayerInput;
        use crate::World;
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../assets/manifest.json")).unwrap();
        let grid = vec![vec![false; 56]; 40];
        let mut w = World::with_seed(grid, manifest, 7);
        let start = w.hero_stats().stamina;
        assert!(start > 0, "stamina initialisée au gabarit (manifest)");
        let tpl_max = w.hero_tpl.stamina;

        // course longue : la valeur descend puis la course est coupée à 0.
        // On relance un MoveTo alterné à chaque tick pour que le héros ne
        // soit JAMAIS arrivé (course continue, comme un joueur qui fuit).
        w.tick(0, Some(PlayerInput::SetRun(true)));
        let mut exhausted = false;
        for i in 0..6000 {
            let target = if i % 2 == 0 { (2, 2) } else { (53, 37) };
            w.tick(50, Some(PlayerInput::MoveTo { x: target.0, y: target.1 }));
            if !w.hero_is_running() {
                exhausted = true;
                break;
            }
        }
        assert!(exhausted, "la course doit être coupée à zéro d'endurance");
        assert_eq!(w.hero_stats().stamina, 0);

        // repos : la régénération repart
        for _ in 0..200 {
            w.tick(50, None);
        }
        assert!(w.hero_stats().stamina > 0, "régénération au repos");

        // mort → respawn : endurance pleine à nouveau (hero est pub(crate),
        // accessible dans le crate pour tester la réinitialisation).
        w.hero.dead_ms = crate::world::RESPAWN_MS - 1;
        w.tick(100, None);
        assert_eq!(w.hero_stats().stamina, tpl_max, "respawn = pleine endurance");
    }
}
