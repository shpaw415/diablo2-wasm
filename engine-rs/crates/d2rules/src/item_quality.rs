//! `item_quality` — tirage de QUALITÉ d'un drop via `ItemRatio.txt` v1.12.
//!
//! Port Rust de l'algorithme D2 réel (réf. exécutable : `Diablerie/
//! Engine/ItemDrop.cs` `TestQuality`/`SelectItemQuality`, mofr — aligné sur
//! les guides Phrozen Keep `ItemRatio.txt` ; od2 `item_ratio_loader.go`
//! ne fait que CHARGER la table, sans l'exploiter) :
//!
//! 1. **Ligne de ratio** choisie par flags (`version` LoD = 1, `uber` =
//!    item exceptionnel/élite = `code != normcode`, `class_specific`).
//! 2. Pour chaque qualité, DU HAUT VERS LE BAS (unique → set → rare →
//!    magic → hi-quality (supérieur) → normal → low), test INDÉPENDANT :
//!    ```text
//!    chance   = (freq − (ilvl − qlvl) / divisor) × 128
//!    chance  −= chance × facteur_TC / 1024   (colonnes Unique/Set/Rare/Magic
//!                                              de TreasureClassEx, max de la
//!                                              chaîne de sous-TCs)
//!    chance   = max(chance, min)             (colonne Min = plancher)
//!    réussite ⇔ rng[0, chance] < 128          (P = 128 / chance)
//!    ```
//!    Première réussite = qualité ; tout échouer ⇒ `Low`.
//!    (`ilvl` = niveau du monstre, `qlvl` = colonne `level` de la base.)
//! 3. **Magic Find** : `MF effectif = MF × 250|500|600|600 / (MF + facteur)`
//!    (unique/set/rare/magic), `chance × 100 / (100 + MF effectif)` — facteur
//!    0 au MVP (aucune stat MF), paramètre déjà branché.
//!
//! Facteur_TC hors bornes (colonnes vides/absentes) = 0 = aucune amélioration.
//! La qualité `Rare` n'est testée que si la base l'autorise (od2
//! `sanitizeDropModifier` : sinon dégradation naturelle vers `Magic`).
//! Déterministe : exactement un tirage [`Rng`] par test, arrêt au premier
//! succès (ordre fixé : U → S → R → M → HiQ → Normal → Low).

use crate::rng::Rng;

/// Qualité d'un item généré (low = fissuré/bruté… ; crafted = cube, hors MVP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum Quality {
    /// Fissuré/bruté/damagé (échec de tous les tests).
    #[default]
    Low,
    /// Normal (blanc).
    Normal,
    /// Supérieur (HiQuality).
    Superior,
    /// Magique (bleu, 1 prefix et/ou 1 suffix).
    Magic,
    /// Pièce de set (vert).
    Set,
    /// Rare (jaune).
    Rare,
    /// Unique (doré).
    Unique,
}

/// Un triplet `Frequency/Divisor/Min` de `ItemRatio.txt` (port
/// `item_ratio_record.go::DropRatioInfo`, `min` = plancher du chance final).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropRatioInfo {
    /// Fréquence de base (`Unique`, `Rare`, … — « 1 / freq » à ilvl == qlvl).
    pub frequency: i32,
    /// Diviseur de la rampe de niveau (`(ilvl − qlvl) / divisor`).
    pub divisor: i32,
    /// Plancher du chance final (`UniqueMin` 6400…).
    pub min: i32,
}

/// Une ligne de `ItemRatio.txt` (5 lignes réelles v1.12 : 2 classic ignorées,
/// 3 LoD : normal / uber / class-specific).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RatioRow {
    /// 0 = normal, 1 = exceptionnel/élite (`uber`).
    pub uber: bool,
    /// Classe spécifique (colonne `Class Specific`).
    pub class_specific: bool,
    /// Qualité unique.
    pub unique: DropRatioInfo,
    /// Qualité rare.
    pub rare: DropRatioInfo,
    /// Qualité set.
    pub set: DropRatioInfo,
    /// Qualité magique.
    pub magic: DropRatioInfo,
    /// Supérieur (HiQuality — pas de colonne Min).
    pub hi_quality: DropRatioInfo,
    /// Normal (pas de colonne Min).
    pub normal: DropRatioInfo,
}

/// Tables de qualité v1.12 embarquées : 3 lignes LoD indexées
/// `(uber, class_specific)`.
#[derive(Debug, Clone, Copy, Default)]
pub struct QualityTables {
    /// Ligne item normal, non classe.
    pub normal: Option<RatioRow>,
    /// Ligne item exceptionnel/élite.
    pub uber: Option<RatioRow>,
    /// Ligne item de classe.
    pub class_specific: Option<RatioRow>,
    /// Ligne item de classe exceptionnel.
    pub class_specific_uber: Option<RatioRow>,
}

/// Facteurs de qualité cumulés d'une chaîne de TCs (max des colonnes
/// `Unique`/`Set`/`Rare`/`Magic` de `TreasureClassEx.txt` le long de la
/// récursion — port `ItemDrop.Drop` Diablerie, `QualityFactors`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QualityFactors {
    /// Colonne `Unique` max de la chaîne.
    pub unique: i32,
    /// Colonne `Set` max.
    pub set: i32,
    /// Colonne `Rare` max.
    pub rare: i32,
    /// Colonne `Magic` max.
    pub magic: i32,
}

/// Parse `ItemRatio.txt` (embarqué) ; `None` si illisible — la résolution
/// retombe alors sur `Normal` (drop nu, comportement phase 2).
pub fn parse(txt: &str) -> Option<QualityTables> {
    let mut t = QualityTables::default();
    let mut any = false;
    for r in super::loot::tsv_rows(txt).into_iter().skip(1) {
        let cell = |i: usize| r.get(i).map(|s| s.trim()).unwrap_or("");
        let name = cell(0);
        // version = 1 uniquement (LoD) — les 2 lignes classic (version 0)
        // ne s'appliquent pas au port v1.12 (Diablerie `RemoveAll(version==0)`).
        if name.is_empty() || cell(1) != "1" {
            continue;
        }
        let info = |f: usize, d: usize, m: usize| DropRatioInfo {
            frequency: cell(f).parse().unwrap_or(0),
            divisor: cell(d).parse().unwrap_or(1),
            min: cell(m).parse().unwrap_or(0),
        };
        let uber = cell(2) == "1";
        let class = cell(3) == "1";
        // Colonnes : Unique,UniqueDivisor,UniqueMin / Rare… / Set… / Magic… /
        // HiQuality,HiQualityDivisor / Normal,NormalDivisor (pas de Min).
        let row = RatioRow {
            uber,
            class_specific: class,
            unique: info(4, 5, 6),
            rare: info(7, 8, 9),
            set: info(10, 11, 12),
            magic: info(13, 14, 15),
            hi_quality: DropRatioInfo {
                frequency: cell(16).parse().unwrap_or(0),
                divisor: cell(17).parse().unwrap_or(1),
                min: 0,
            },
            normal: DropRatioInfo {
                frequency: cell(18).parse().unwrap_or(0),
                divisor: cell(19).parse().unwrap_or(1),
                min: 0,
            },
        };
        let slot = match (class, uber) {
            (false, false) => &mut t.normal,
            (false, true) => &mut t.uber,
            (true, false) => &mut t.class_specific,
            (true, true) => &mut t.class_specific_uber,
        };
        if slot.is_none() {
            *slot = Some(row);
        }
        any = true;
    }
    any.then_some(t)
}

impl QualityTables {
    fn pick_row(&self, uber: bool, class: bool) -> Option<RatioRow> {
        match (class, uber) {
            (false, false) => self.normal,
            (false, true) => self.uber,
            (true, false) => self.class_specific,
            (true, true) => self.class_specific_uber,
        }
    }

    /// Ligne applicable à une base (`uber` = `code != normcode`, réf.
    /// Diablerie ; `class_specific` = flag type, faux au MVP, écart documenté).
    pub fn row_for(&self, uber: bool, class_specific: bool) -> RatioRow {
        let fallback = RatioRow {
            uber,
            class_specific,
            unique: DropRatioInfo { frequency: 400, divisor: 1, min: 6400 },
            rare: DropRatioInfo { frequency: 100, divisor: 2, min: 3200 },
            set: DropRatioInfo { frequency: 160, divisor: 2, min: 5600 },
            magic: DropRatioInfo { frequency: 34, divisor: 3, min: 192 },
            hi_quality: DropRatioInfo { frequency: 12, divisor: 8, min: 0 },
            normal: DropRatioInfo { frequency: 2, divisor: 2, min: 0 },
        };
        self.pick_row(uber, class_specific).unwrap_or(fallback)
    }
}

/// Un test de qualité isolé (port exact `ItemDrop.TestQuality`) : `true` si la
/// qualité est obtenue. Un seul tirage [`Rng`] par appel.
#[allow(clippy::too_many_arguments)]
pub fn test_quality(
    rng: &mut Rng,
    info: &DropRatioInfo,
    qlvl: u32,
    ilvl: u32,
    quality_factor: i32,
    mf: i32,
    mf_factor: i32,
) -> bool {
    // chance = (freq − (ilvl − qlvl)/divisor) × 128  (division tronquée D2)
    let lvl_gap = ilvl as i64 - qlvl as i64;
    let mut chance = (info.frequency as i64 - lvl_gap / info.divisor.max(1) as i64) * 128;
    // Magic Find : MF effectif = mf·f/(mf+f) puis chance × 100/(100+eff).
    if mf_factor > 0 && mf > 0 {
        let eff = mf * mf_factor / (mf + mf_factor);
        chance = chance * 100 / (100 + eff as i64);
    }
    // Facteur TC : chance − chance·qf/1024 (boost des colonnes TreasureClassEx).
    if quality_factor > 0 {
        chance -= chance * quality_factor.min(1024) as i64 / 1024;
    }
    // Plancher (colonne Min) puis jet : réussite ⇔ tirage < 128 (P = 128/chance).
    let chance = (chance.max(info.min as i64)).max(1) as u32;
    rng.below(chance) < 128
}

/// Résolution complète d'un drop TCE : base (`qlvl`), `ilvl` (niveau du
/// monstre), MF, facteurs TC et autorisation `rare`. Un tirage par test,
/// ordre fixe U → S → R → M → HiQ → Normal → Low (tout échouer = [`Quality::Low`]).
pub fn resolve(
    rng: &mut Rng,
    tables: &QualityTables,
    qlvl: u32,
    ilvl: u32,
    mf: i32,
    factors: &QualityFactors,
    can_be_rare: bool,
) -> Quality {
    let r = tables.row_for(false, false);
    if test_quality(rng, &r.unique, qlvl, ilvl, factors.unique, mf, 250) {
        return Quality::Unique;
    }
    if test_quality(rng, &r.set, qlvl, ilvl, factors.set, mf, 500) {
        return Quality::Set;
    }
    if can_be_rare && test_quality(rng, &r.rare, qlvl, ilvl, factors.rare, mf, 600) {
        return Quality::Rare;
    }
    if test_quality(rng, &r.magic, qlvl, ilvl, factors.magic, mf, 600) {
        return Quality::Magic;
    }
    if test_quality(rng, &r.hi_quality, qlvl, ilvl, 0, 0, 0) {
        return Quality::Superior;
    }
    if test_quality(rng, &r.normal, qlvl, ilvl, 0, 0, 0) {
        return Quality::Normal;
    }
    Quality::Low
}

// ---------------------------------------------------------------------------
// Tests — nombres exacts de la table réelle v1.12 (golden).
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const RATIO: &str = include_str!("../../../../assets/patch_itemratio.txt");

    fn tables() -> QualityTables {
        parse(RATIO).expect("ItemRatio.txt embarqué valide")
    }

    /// Golden : la ligne v1.12 « normal » (Ratio - Monster Level / Ratio
    /// Divisor, Version 1) porte EXACTEMENT les cellules du fichier extrait
    /// (`assets/patch_itemratio.txt`, colonnes alignées sur l'en-tête — le
    /// fichier réel v1.12 a une colonne `Function` en tête) : 400/1/6400,
    /// Rare 100/2/3200, Set 160/2/5600 (quirk authentique), Magic 34/3/192,
    /// HiQuality 12/8, Normal 2/2. LA TABLE FAIT FOI (convention
    /// impl-integration.md) — les valeurs « folklore » 192/12 et 8/2
    /// décrivent un autre arrangement de colonnes, pas ce fichier.
    #[test]
    fn itemratio_real_numbers_normal_row() {
        let r = tables().row_for(false, false);
        assert_eq!(
            r.unique,
            DropRatioInfo { frequency: 400, divisor: 1, min: 6400 }
        );
        assert_eq!(r.rare, DropRatioInfo { frequency: 100, divisor: 2, min: 3200 });
        assert_eq!(r.set, DropRatioInfo { frequency: 160, divisor: 2, min: 5600 });
        assert_eq!(r.magic, DropRatioInfo { frequency: 34, divisor: 3, min: 192 });
        assert_eq!(r.hi_quality, DropRatioInfo { frequency: 12, divisor: 8, min: 0 });
        assert_eq!(r.normal, DropRatioInfo { frequency: 2, divisor: 2, min: 0 });
    }

    /// Golden : lignes uber et class-specific LoD (400/1, 80/3, 120/3, 17/6).
    #[test]
    fn itemratio_real_numbers_uber_and_class_rows() {
        let t = tables();
        let u = t.uber.expect("ligne uber LoD");
        assert_eq!(u.unique.frequency, 400);
        assert_eq!(u.rare.frequency, 100);
        assert_eq!(u.set.frequency, 160);
        assert_eq!(u.magic.frequency, 34);
        assert!(u.uber);
        let c = t.class_specific.expect("ligne class LoD");
        assert_eq!(c.unique.frequency, 240);
        assert_eq!(c.rare.frequency, 80);
        assert_eq!(c.set.frequency, 120);
        assert_eq!(c.magic.frequency, 17);
        let cu = t.class_specific_uber.expect("ligne class uber LoD");
        assert_eq!(cu.unique.frequency, 240);
        // classic (version 0) ignoré : pas de ligne normal si uber=1 absent…
        assert!(t.normal.is_some());
    }

    /// Le test de qualité reproduit les probabilités théoriques : à
    /// ilvl == qlvl, P(unique) = 128/51201 ≈ 1/400 ± bruit statistique.
    #[test]
    fn unique_probability_matches_one_over_frequency() {
        let info = DropRatioInfo { frequency: 400, divisor: 1, min: 6400 };
        let n = 200_000u32;
        let mut hits = 0u32;
        let mut rng = Rng::new(0xD2);
        for _ in 0..n {
            if test_quality(&mut rng, &info, 1, 1, 0, 0, 0) {
                hits += 1;
            }
        }
        let ppm = (hits as u64 * 1_000_000 / n as u64) as u64;
        // 1/400 = 2500 ppm ; tolérance ±15 % (variance binomiale ≈ 11 ppm/√n).
        assert!(
            (2125..=2875).contains(&ppm),
            "P(unique) mesurée {ppm} ppm (attendu ≈ 2500)"
        );
    }

    /// Plancher `min` : mlvl très au-dessus → chance plafonnée à `min`
    /// (6400) ⇒ P ≈ 128/6401 ≈ 2 %, jamais au-delà.
    #[test]
    fn min_column_caps_best_case_probability() {
        let info = DropRatioInfo { frequency: 400, divisor: 1, min: 6400 };
        let n = 100_000u32;
        let mut hits = 0u32;
        let mut rng = Rng::new(0xD2A);
        for _ in 0..n {
            // ilvl ≫ qlvl : chance brute négative, plancher 6400.
            if test_quality(&mut rng, &info, 1, 500, 0, 0, 0) {
                hits += 1;
            }
        }
        let pct = hits as f64 / n as f64 * 100.0;
        assert!((1.5..=2.5).contains(&pct), "P plafonnée mesurée {pct} % (attendu ≈ 2 %)");
    }

    /// Facteur TC saturé (1024, colonne TreasureClassEx max) : `chance` tombe
    /// à 0 PUIS passe sous le plancher `Min` (ordre D2 réel, Diablerie
    /// `TestQuality`) — la qualité n'est donc pas garantie : le chance final =
    /// `Min` (6400) ⇒ P ≈ 128/6400 ≈ 2 %, soit ~8× mieux que sans facteur
    /// (1/400 ≈ 0,25 %). Le plancher protège le pool entier, colonne à 1024
    /// ou non.
    #[test]
    fn tc_factor_1024_caps_at_min_floor() {
        let info = DropRatioInfo { frequency: 400, divisor: 1, min: 6400 };
        let n = 200_000u32;
        let run = |qf: i32| {
            let mut hits = 0u32;
            let mut rng = Rng::new(9);
            for _ in 0..n {
                if test_quality(&mut rng, &info, 1, 1, qf, 0, 0) {
                    hits += 1;
                }
            }
            hits as f64 / n as f64
        };
        let base = run(0);
        let boosted = run(1024);
        // Plancher : P = 128/6400 = 2 % ; tolérance binomiale ±10 %.
        assert!(
            (0.018..=0.022).contains(&boosted),
            "P facteur 1024 mesurée {boosted:.4} (attendu ≈ 0,02 = plancher Min)"
        );
        assert!(boosted > base * 6.0, "le facteur TC doit booster fortement : {base:.4} → {boosted:.4}");
    }

    /// Magic Find réduit le chance (augmente P) : mf=75, facteur 250.
    #[test]
    fn magic_find_increases_probability() {
        let info = DropRatioInfo { frequency: 400, divisor: 1, min: 6400 };
        let n = 200_000u32;
        let run = |mf: i32| {
            let mut hits = 0;
            let mut rng = Rng::new(0xD2B);
            for _ in 0..n {
                if test_quality(&mut rng, &info, 1, 1, 0, mf, 250) {
                    hits += 1;
                }
            }
            hits as f64 / n as f64
        };
        let base = run(0);
        let boosted = run(750);
        // eff = 750·250/1000 = 187 ⇒ ×100/287 ≈ ×2,89 sur P.
        assert!(boosted > base * 2.0, "mf {base:.4} → {boosted:.4}");
        assert!(boosted < 0.02, "plafond raisonnable");
    }

    /// `resolve` : déterministe au seed, qualité valide, et la séquence
    /// fixe consomme un flux identique pour le même seed.
    #[test]
    fn resolve_is_deterministic() {
        let t = tables();
        let mut a = Rng::new(777);
        let mut b = Rng::new(777);
        for qlvl in [1u32, 6, 30] {
            let qa = resolve(&mut a, &t, qlvl, qlvl + 2, 0, &QualityFactors::default(), true);
            let qb = resolve(&mut b, &t, qlvl, qlvl + 2, 0, &QualityFactors::default(), true);
            assert_eq!(qa, qb, "qlvl {qlvl}");
        }
    }

    /// Sans facteur TC ni MF, les uniques restent rares sur 20k drops
    /// (aucune explosion) et le spectre complet peut apparaître.
    #[test]
    fn resolve_distribution_is_sane() {
        let t = tables();
        let mut counts = [0u32; 7];
        let mut rng = Rng::new(0xD2C);
        let n = 20_000;
        for i in 0..n {
            let qlvl = 1 + (i % 20) as u32;
            let q = resolve(&mut rng, &t, qlvl, qlvl + 3, 0, &QualityFactors::default(), true);
            counts[match q {
                Quality::Low => 0,
                Quality::Normal => 1,
                Quality::Superior => 2,
                Quality::Magic => 3,
                Quality::Set => 4,
                Quality::Rare => 5,
                Quality::Unique => 6,
            }] += 1;
        }
        assert!(counts[6] > 0, "aucun unique sur {n} drops");
        assert!(counts[3] > counts[6] * 2, "magic devrait dominer unique");
        assert!(counts[1] > 0 && counts[2] > 0, "normal/supérieur absents");
    }
}
