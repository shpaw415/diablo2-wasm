//! Difficultés Normal / Nightmare / Hell — W08 (`finish-monde.md`) + K
//! (`finish-systemes.md`).
//!
//! Trois sources réelles v1.12 (assets extraits des MPQ, aucune invention —
//! chaque champ cite son `fichier:colonne`) :
//!
//! * `assets/patch_difficultylevels.txt` (3 records, lignes 2-4) — les
//!   pénalités côté héros et diviseurs côté monstres : `ResistPenalty`
//!   0/−40/−100, `DeathExpPenalty` (XP perdu à la mort) 0/5/10 %,
//!   `MonsterSkillBonus` 0/3/7, `MonsterFreezeDivisor`/`MonsterColdDivisor`/
//!   `AiCurseDivisor` 1/2/4, `LifeStealDivisor`/`ManaStealDivisor` 1/2/3,
//!   `UniqueDamageBonus`/`ChampionDamageBonus` 90/75/66,
//!   `HireableBossDamagePercent` 50/35/25, `MonsterCEDamagePercent` 50/35/20,
//!   `StaticFieldMin` 0/33/50, `UberCodeOddsNormal/Good` 0/0 → 10/20 → 20/40,
//!   `UltraCodeOddsNormal/Good` 0/0 → 0/0 → 30/40.
//! * `assets/MonLvl.txt` (111 lignes, niveaux 0..110) — facteurs effectifs
//!   par niveau de monstre × difficulté : colonnes `AC(N)/(H)`, `TH(N)/(H)`,
//!   `HP(N)/(H)`, `DM(N)/(H)`, `XP(N)/(H)`. Ligne niveau 1 : AC 6/92/147,
//!   TH 8/108/216, HP 7/107/830, DM 2/3/4, XP 30/78/117. Le parseur
//!   colonnes Normal vit côté d2game (`monsters.rs::MonLvl`) ; cette table
//!   des pénalités, elle, est embarquée ICI (consommée par `World` sans IO).
//! * `assets/patch_monstats.txt` — `Level(N)/(H)` (colonnes 33-34 ; ex.
//!   zombie1 = 1/36/67) et les résists par difficulté `ResDm/ResMa/ResFi/
//!   ResLi/ResCo/ResPo` ± `(N)/(H)` (colonnes 130-147 ; ex. zombie1
//!   `ResCo(H)`=120 ⇒ IMMUNITé froid en Enfer, fallen1 `ResFi(H)`=100 ⇒
//!   immunité feu, skeleton1 `ResLi(H)`=100 ⇒ immunité foudre ; convention
//!   tables v1.12 : cellule vide = 0). Les valeurs sont lues par le plan de
//!   spawn d2game et posées sur le monstre via `SpawnRule::res`.
//!
//! Règles de résist v1.12 : une résistance `res` modifie les dégâts
//! élémentaires par ×(100−res)/100 (négatif = amplifié) ; `res ≥ 100`
//! = immunité (0 dégât). `apply_resist` est la seule arithmétique —
//! partagée mêlée (ResDm) et missiles (`DamageType` → index).

use serde::{Deserialize, Serialize};

/// Seuil d'immunité : résist ≥ 100 ⇒ la source élémentaire fait 0 dégât.
pub const RES_IMMUNE_THRESHOLD: i32 = 100;

/// Les 3 difficultés du jeu (`patch_difficultylevels.txt` lignes 2-4 ;
/// Go de référence : `d2enum/difficulty.go`). `Default` = Normal : tout
/// l'existant (constructeurs `World`, saves v1/v2) reste exactement
/// identique — la difficulté est purement additive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Difficulty {
    /// Difficulté 1 (défaut historique).
    #[default]
    Normal,
    /// Difficulté 2 (`MonLvl` colonnes `(N)`).
    Nightmare,
    /// Difficulté 3 (`MonLvl` colonnes `(H)`) — résists négatifs, immunités.
    Hell,
}

impl Difficulty {
    /// Code stable (JSON wasm / save / URL `?diff=`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Difficulty::Normal => "normal",
            Difficulty::Nightmare => "nightmare",
            Difficulty::Hell => "hell",
        }
    }

    /// Résout `?diff=nm|hell|normal…` (insensible à la casse ; inconnu/vide
    /// ⇒ Normal — jamais d'erreur côté page).
    pub fn parse(s: &str) -> Difficulty {
        match s.trim().to_ascii_lowercase().as_str() {
            "nightmare" | "nm" | "n" | "2" => Difficulty::Nightmare,
            "hell" | "h" | "3" => Difficulty::Hell,
            _ => Difficulty::Normal,
        }
    }

    /// Index de difficulté 0/1/2 = rangée `patch_difficultylevels.txt`,
    /// colonnes `(N)`/`(H)` des tables.
    pub fn index(&self) -> usize {
        match self {
            Difficulty::Normal => 0,
            Difficulty::Nightmare => 1,
            Difficulty::Hell => 2,
        }
    }

    /// Suffixe de colonne TSV v1.12 (`""` / `"(N)"` / `"(H)"`).
    pub fn col_suffix(&self) -> &'static str {
        match self {
            Difficulty::Normal => "",
            Difficulty::Nightmare => "(N)",
            Difficulty::Hell => "(H)",
        }
    }

    /// Suffixe brut des TreasureClass v1.12 (`None` en Normal — les records
    /// Normal de TreasureClassEx ne portent PAS de marqueur ; ` (N)`/` (H)`
    /// sinon). Préférer [`Self::tc_variant`] qui compose le nom COMPLET
    /// selon la règle retail de positionnement du marqueur.
    pub fn tc_suffix(&self) -> Option<&'static str> {
        match self {
            Difficulty::Normal => None,
            Difficulty::Nightmare => Some(" (N)"),
            Difficulty::Hell => Some(" (H)"),
        }
    }

    /// Nom de TreasureClass EFFECTIF pour la difficulté — règle retail
    /// v1.12 (`TreasureClassEx.txt`, vérifiée entrée par entrée par le
    /// golden `progression_golden.rs`) :
    ///
    /// * Normal ⇒ nom inchangé (les records de base n'ont pas de marqueur) ;
    /// * TC « Act N … » ⇒ marqueur INSÉRÉ après le préfixe d'acte
    ///   (« Act 1 H2H A » → « Act 1 (N) H2H A », « Act 5 Champ C » →
    ///   « Act 5 (H) Champ C ») ;
    /// * toute autre TC (boss, superunique, « Quill 1 », « Griswold »…)
    ///   ⇒ marqueur SUFFIXÉ en fin de nom (« Andariel » → « Andariel (N) »,
    ///   « Duriel » → « Duriel (H) »).
    ///
    /// Attention : l'existence de la variante n'est PAS garantie par la
    /// table pour toutes les TC (« Fangskin (H) » manque dans
    /// TreasureClassEx v1.12) — l'appelant qui dropne doit garder la base
    /// si la variante est inconnue (cf. `World::roll_drop`).
    pub fn tc_variant(base: &str, diff: Difficulty) -> String {
        match diff.tc_suffix() {
            None => base.to_string(),
            Some(sfx) => {
                if let Some(rest) = base.strip_prefix("Act ") {
                    if let Some(sp) = rest.find(' ') {
                        return format!("Act {}{}{}", &rest[..sp], sfx, &rest[sp..]);
                    }
                }
                format!("{base}{sfx}")
            }
        }
    }

    /// Index de résist monstre (`[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]`,
    /// ordre des colonnes `patch_monstats.txt` 130-135) pour un élément
    /// `d2missiles::DamageType`.
    pub fn resist_index(d: d2missiles::DamageType) -> usize {
        use d2missiles::DamageType as T;
        match d {
            T::Physical => 0, // ResDm
            T::Magic => 1,    // ResMa
            T::Fire => 2,     // ResFi
            T::Lightning => 3, // ResLi
            T::Cold => 4,     // ResCo
            T::Poison => 5,   // ResPo
        }
    }
}

/// Dégâts élémentaires après résist v1.12 : `dmg × (100 − res)/100`
/// (résist négative = amplifié, ex. Hell) ; `res ≥ 100` = immunité ⇒ 0.
/// Entier pur (u64 interne — jamais d'overflow sur des dégâts u32).
pub fn apply_resist(dmg: u32, res: i32) -> u32 {
    if res >= RES_IMMUNE_THRESHOLD {
        return 0;
    }
    ((dmg as u64) * (100 - res as i64).max(0) as u64 / 100) as u32
}

// ---------------------------------------------------------------------------
// patch_difficultylevels.txt — pénalités héros + diviseurs monstres (K3)
// ---------------------------------------------------------------------------

/// Une rangée de `patch_difficultylevels.txt` (une par difficulté).
/// Consommateurs v1.12 : StaticField (héros), vole-de-vie/gel (monstres)…
/// Les champs sans consommateur dans le port aujourd'hui restent exposés
/// (preuve de données, K3 — la DoD « Hell difficile » s'appuiera dessus).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DiffLevelRow {
    /// Pénalité de résistance du héros (`ResistPenalty` : 0/−40/−100).
    pub resist_penalty: i32,
    /// % d'XP PERDUS à la mort du héros (`DeathExpPenalty` : 0/5/10).
    pub death_exp_penalty: u32,
    /// Bonus de niveau de skill des monstres (`MonsterSkillBonus` : 0/3/7).
    pub monster_skill_bonus: u32,
    /// Diviseur de durée de gel infligée aux monstres (`MonsterFreezeDivisor`).
    pub monster_freeze_divisor: u32,
    /// Diviseur de durée de froid (`MonsterColdDivisor`).
    pub monster_cold_divisor: u32,
    /// Diviseur de durée de malédiction (`AiCurseDivisor`).
    pub ai_curse_divisor: u32,
    /// Diviseur de vol de vie (`LifeStealDivisor`).
    pub life_steal_divisor: u32,
    /// Diviseur de vol de mana (`ManaStealDivisor`).
    pub mana_steal_divisor: u32,
    /// +% dégâts des uniques (`UniqueDamageBonus` : 90/75/66).
    pub unique_damage_bonus: u32,
    /// +% dégâts des champions (`ChampionDamageBonus` : 90/75/66).
    pub champion_damage_bonus: u32,
    /// `HireableBossDamagePercent` (50/35/25).
    pub hireable_boss_damage_percent: u32,
    /// `MonsterCEDamagePercent` — CorpseExplosion monstre (50/35/20).
    pub monster_ce_damage_percent: u32,
    /// `StaticFieldMin` — plancher de PV du Static Field (0/33/50).
    pub static_field_min: u32,
    /// `UberCodeOddsNormal/Good` (chances d'uber codes, 0/0 → 10/20 → 20/40).
    pub uber_code_odds: [u32; 2],
    /// `UltraCodeOddsNormal/Good` (0/0 → 0/0 → 30/40).
    pub ultra_code_odds: [u32; 2],
}

/// Les 3 rangées `patch_difficultylevels.txt`, indexées par
/// [`Difficulty::index`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DifficultyLevels {
    rows: [DiffLevelRow; 3],
}

impl DifficultyLevels {
    /// Parse le TSV réel (en-tête nommé + 3 rangées `Normal/Nightmare/Hell`).
    /// `Err` si une rangée manque — l'appelant garde alors les défauts 0
    /// (comportement Normal historique, jamais fatal).
    pub fn parse(tsv: &str) -> Result<DifficultyLevels, String> {
        let mut lines = tsv.lines();
        let header: Vec<&str> = lines.next().unwrap_or("").split('\t').collect();
        let col = |name: &str| header.iter().position(|h| h.trim() == name);
        let need = [
            "Name", "ResistPenalty", "DeathExpPenalty", "MonsterSkillBonus",
            "MonsterFreezeDivisor", "MonsterColdDivisor", "AiCurseDivisor",
            "LifeStealDivisor", "ManaStealDivisor", "UniqueDamageBonus",
            "ChampionDamageBonus", "HireableBossDamagePercent",
            "MonsterCEDamagePercent", "StaticFieldMin", "UberCodeOddsNormal",
            "UberCodeOddsGood", "UltraCodeOddsNormal", "UltraCodeOddsGood",
        ];
        for n in need {
            if col(n).is_none() {
                return Err(format!("patch_difficultylevels.txt : colonne {n} absente"));
            }
        }
        let cell = |cells: &[&str], name: &str| -> i32 {
            let i = col(name).unwrap_or(usize::MAX);
            cells.get(i).and_then(|c| c.trim().parse::<i32>().ok()).unwrap_or(0)
        };
        let mut rows = [DiffLevelRow::default(); 3];
        let mut seen = [false; 3];
        for line in lines {
            let cells: Vec<&str> = line.split('\t').collect();
            let idx = match cells.first().copied().unwrap_or("").trim() {
                "Normal" => 0,
                "Nightmare" => 1,
                "Hell" => 2,
                _ => continue,
            };
            rows[idx] = DiffLevelRow {
                resist_penalty: cell(&cells, "ResistPenalty"),
                death_exp_penalty: cell(&cells, "DeathExpPenalty").max(0) as u32,
                monster_skill_bonus: cell(&cells, "MonsterSkillBonus").max(0) as u32,
                monster_freeze_divisor: cell(&cells, "MonsterFreezeDivisor").max(1) as u32,
                monster_cold_divisor: cell(&cells, "MonsterColdDivisor").max(1) as u32,
                ai_curse_divisor: cell(&cells, "AiCurseDivisor").max(1) as u32,
                life_steal_divisor: cell(&cells, "LifeStealDivisor").max(1) as u32,
                mana_steal_divisor: cell(&cells, "ManaStealDivisor").max(1) as u32,
                unique_damage_bonus: cell(&cells, "UniqueDamageBonus").max(0) as u32,
                champion_damage_bonus: cell(&cells, "ChampionDamageBonus").max(0) as u32,
                hireable_boss_damage_percent: cell(&cells, "HireableBossDamagePercent").max(0) as u32,
                monster_ce_damage_percent: cell(&cells, "MonsterCEDamagePercent").max(0) as u32,
                static_field_min: cell(&cells, "StaticFieldMin").max(0) as u32,
                uber_code_odds: [
                    cell(&cells, "UberCodeOddsNormal").max(0) as u32,
                    cell(&cells, "UberCodeOddsGood").max(0) as u32,
                ],
                ultra_code_odds: [
                    cell(&cells, "UltraCodeOddsNormal").max(0) as u32,
                    cell(&cells, "UltraCodeOddsGood").max(0) as u32,
                ],
            };
            seen[idx] = true;
        }
        if !seen.iter().all(|s| *s) {
            return Err("patch_difficultylevels.txt : rangée Normal/Nightmare/Hell manquante".into());
        }
        Ok(DifficultyLevels { rows })
    }

    /// Rangée d'une difficulté (preuve : ligne `Name` de la table).
    pub fn row(&self, d: Difficulty) -> &DiffLevelRow {
        &self.rows[d.index()]
    }
}

/// Table embarquée (`include_str!`, même modèle que `loot.rs`) — le WASM et
/// les tests consomment la MÊME source réelle, zéro IO.
const DIFFICLEVELS_TXT: &str = include_str!("../../../../assets/patch_difficultylevels.txt");

/// Instance unique parsée une fois (runtime WASM, tests).
pub fn embedded() -> &'static DifficultyLevels {
    static EMBEDDED: std::sync::OnceLock<DifficultyLevels> = std::sync::OnceLock::new();
    EMBEDDED.get_or_init(|| DifficultyLevels::parse(DIFFICLEVELS_TXT).expect("table difficultylevels embarquée valide"))
}

// ---------------------------------------------------------------------------
// Tests — la table réelle + les règles de résist
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Réel `assets/patch_difficultylevels.txt` (lignes 2-4) — chaque valeur
    /// est une preuve fichier:ligne:colonne.
    #[test]
    fn parses_real_difficultylevels() {
        let t = embedded();
        let (n, nm, h) = (t.row(Difficulty::Normal), t.row(Difficulty::Nightmare), t.row(Difficulty::Hell));
        // ResistPenalty : 0 / −40 / −100 (Hell = −100, K3).
        assert_eq!((n.resist_penalty, nm.resist_penalty, h.resist_penalty), (0, -40, -100));
        // DeathExpPenalty : 0 / 5 / 10 % d'XP perdus à la mort.
        assert_eq!((n.death_exp_penalty, nm.death_exp_penalty, h.death_exp_penalty), (0, 5, 10));
        // MonsterSkillBonus : 0 / 3 / 7.
        assert_eq!((n.monster_skill_bonus, nm.monster_skill_bonus, h.monster_skill_bonus), (0, 3, 7));
        // Diviseurs gel/froid/curse : 1 / 2 / 4.
        assert_eq!(n.monster_freeze_divisor, 1);
        assert_eq!(nm.monster_cold_divisor, 2);
        assert_eq!(h.ai_curse_divisor, 4);
        // Vole de vie/mana : 1 / 2 / 3.
        assert_eq!((n.life_steal_divisor, nm.mana_steal_divisor, h.mana_steal_divisor), (1, 2, 3));
        // Bonus dégâts uniques/champions : 90 / 75 / 66.
        assert_eq!((n.unique_damage_bonus, nm.unique_damage_bonus, h.unique_damage_bonus), (90, 75, 66));
        assert_eq!((n.champion_damage_bonus, nm.champion_damage_bonus, h.champion_damage_bonus), (90, 75, 66));
        // StaticFieldMin : 0 / 33 / 50.
        assert_eq!((n.static_field_min, nm.static_field_min, h.static_field_min), (0, 33, 50));
        // Uber/Ultra code odds (NM/Hell seulement).
        assert_eq!(nm.uber_code_odds, [10, 20]);
        assert_eq!(h.uber_code_odds, [20, 40]);
        assert_eq!(h.ultra_code_odds, [30, 40]);
        assert_eq!(n.uber_code_odds, [0, 0]);
    }

    #[test]
    fn parses_str_and_index_and_suffixes() {
        assert_eq!(Difficulty::parse("hell"), Difficulty::Hell);
        assert_eq!(Difficulty::parse("NM"), Difficulty::Nightmare);
        assert_eq!(Difficulty::parse("nightmare"), Difficulty::Nightmare);
        assert_eq!(Difficulty::parse(""), Difficulty::Normal);
        assert_eq!(Difficulty::parse("n'importe quoi"), Difficulty::Normal);
        assert_eq!(Difficulty::Hell.index(), 2);
        assert_eq!(Difficulty::Nightmare.col_suffix(), "(N)");
        assert_eq!(Difficulty::Normal.tc_suffix(), None);
        assert_eq!(Difficulty::Hell.tc_suffix(), Some(" (H)"));
        assert_eq!(Difficulty::default(), Difficulty::Normal);
    }

    /// Règle retail de nommage des variantes TC (13a) — insérée après le
    /// préfixe d'acte pour les TC « Act N … », suffixée sinon ; Normal =
    /// nom inchangé. Vérifiée contre 100 % des TC monstats dans
    /// `progression_golden.rs`.
    #[test]
    fn tc_variant_composes_retail_names() {
        use Difficulty::{Hell, Nightmare, Normal};
        // Normal : inchangé.
        assert_eq!(Difficulty::tc_variant("Act 1 H2H A", Normal), "Act 1 H2H A");
        assert_eq!(Difficulty::tc_variant("Duriel", Normal), "Duriel");
        // TC d'acte : insertion APRÈS « Act N » (jamais en fin de nom).
        assert_eq!(Difficulty::tc_variant("Act 1 H2H A", Nightmare), "Act 1 (N) H2H A");
        assert_eq!(Difficulty::tc_variant("Act 1 H2H A", Hell), "Act 1 (H) H2H A");
        assert_eq!(Difficulty::tc_variant("Act 5 Champ C", Hell), "Act 5 (H) Champ C");
        // TC nommées : suffixe final.
        assert_eq!(Difficulty::tc_variant("Andariel", Nightmare), "Andariel (N)");
        assert_eq!(Difficulty::tc_variant("Duriel", Hell), "Duriel (H)");
        assert_eq!(Difficulty::tc_variant("Quill 1", Nightmare), "Quill 1 (N)");
        // Cas limites : vide, « Act » sans suite (non producteur de TC).
        assert_eq!(Difficulty::tc_variant("", Hell), " (H)");
        assert_eq!(Difficulty::tc_variant("Act", Hell), "Act (H)");
    }

    /// Règle v1.12 : ×(100−res)/100, négatif = amplifié, ≥100 = immunité.
    #[test]
    fn apply_resist_matches_v112() {
        // Résist positive : réduit (fallen1 ResFi(H)=40 → froid… non, feu 40).
        assert_eq!(apply_resist(100, 40), 60);
        // Résist nulle (vide en table) : inchangé.
        assert_eq!(apply_resist(100, 0), 100);
        // Résist négative (Hell) : amplifie ×1,5.
        assert_eq!(apply_resist(100, -50), 150);
        // Immunités réelles : zombie1 ResCo(H)=120, fallen1 ResFi(H)=100,
        // skeleton1 ResLi(H)=100 (patch_monstats.txt) ⇒ 0 dégât.
        assert_eq!(apply_resist(100, 100), 0);
        assert_eq!(apply_resist(100, 120), 0);
        assert_eq!(apply_resist(u32::MAX / 2, 90), u32::MAX / 20, "×10/100 sans overflow");
    }
}
