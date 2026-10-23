//! Table d'expérience D2 v1.12 complète — socle D5 de `finish-systemes.md`
//! (`patch_experience.txt` : 99 niveaux × 7 classes + MaxLvl + ExpRatio).
//!
//! # Sémantique (table réelle)
//! * La ligne `N` de la table = XP **cumulés** requis pour ATTEINDRE le
//!   niveau `N+1` (ligne 0 = 0 XP = niveau 1 gratuit ; ligne 1 = 500 XP =
//!   niveau 2). `thresholds[i]` reste indexé par la LIGNE `i` du fichier —
//!   [`ClassXp::threshold`] fait le décalage (niveau ⇄ ligne).
//! * `ExpRatio` = colonne de scaling d'XP par ligne (1024 en ligne 0 → 5 en
//!   ligne 99), exposée telle quelle pour les consommateurs futurs.
//! * Les 7 classes ont leurs propres courbes (identiques en v1.12, mais
//!   chacune est lue depuis SA colonne — plus aucune valeur codée en main).

use std::collections::HashMap;
use std::sync::OnceLock;

/// Table XP v1.12 embarquée (102 lignes, patch_d2.mpq).
const XP_TXT: &str = include_str!("../../../../assets/patch_experience.txt");

/// Courbe XP d'une classe.
#[derive(Debug, Clone, Default)]
pub struct ClassXp {
    /// Code canonique 3 lettres (`ama`, `sor`, `nec`, `pal`, `bar`, `dru`, `ass`).
    pub code: String,
    /// Nom de colonne dans la table (`Amazon`…`Assassin`).
    pub column: String,
    /// Niveau maximum (`MaxLvl` = 99 en v1.12).
    pub max_level: u32,
    /// Table brute indexée par LIGNE du fichier : `thresholds[i]` = XP
    /// cumulés pour atteindre le niveau `i + 1` (ligne 0 = 0).
    pub thresholds: Vec<u64>,
}

impl ClassXp {
    /// XP requis pour atteindre `level` (niveau 1 = 0 ; niveau > max ⇒
    /// dernier seuil connu).
    pub fn threshold(&self, level: u32) -> u64 {
        if level == 0 {
            return 0;
        }
        self.thresholds
            .get(level as usize - 1)
            .copied()
            .unwrap_or(*self.thresholds.last().unwrap_or(&0))
    }

    /// Ligne brute de la table (`row` 0..=99), pour les golden tests.
    pub fn raw_row(&self, row: usize) -> u64 {
        self.thresholds.get(row).copied().unwrap_or(0)
    }

    /// Nombre de lignes lues.
    pub fn rows(&self) -> usize {
        self.thresholds.len()
    }

    /// Niveau atteint avec `xp` cumulés (1..=max_level ; xp = 0 ⇒ niveau 1,
    /// le niveau 1 est gratuit comme dans D2).
    pub fn level_for(&self, xp: u64) -> u32 {
        let mut lvl = 1;
        for row in 0..self.max_level.min(self.thresholds.len() as u32) as usize {
            if xp >= self.thresholds[row] {
                lvl = row as u32 + 1;
            } else {
                break;
            }
        }
        lvl.min(self.max_level)
    }

    /// XP restants avant `level + 1` (0 au niveau max).
    pub fn xp_to_next(&self, level: u32, xp: u64) -> u64 {
        if level >= self.max_level {
            return 0;
        }
        self.threshold(level + 1).saturating_sub(xp)
    }
}

/// Table XP complète : 7 classes + la colonne `ExpRatio` par niveau.
#[derive(Debug, Default)]
pub struct XpTable {
    classes: HashMap<String, ClassXp>,
    /// Ordre canonique des codes (colonne de la table).
    order: Vec<String>,
    /// `ExpRatio` par niveau (colonne partagée, scaling d'XP des monstres).
    exp_ratio: Vec<u32>,
}

/// Aliases de classes acceptés en entrée : codes courts, codes héros majuscules
/// (manifest `BA`/`SO`…) et noms complets.
fn class_key(input: &str) -> String {
    let low = input.trim().to_ascii_lowercase();
    match low.as_str() {
        "amazon" | "ama" | "am" => "ama".into(),
        "sorceress" | "sor" | "so" => "sor".into(),
        "necromancer" | "nec" | "ne" => "nec".into(),
        "paladin" | "pal" | "pa" => "pal".into(),
        "barbarian" | "bar" | "ba" => "bar".into(),
        "druid" | "dru" | "dr" => "dru".into(),
        "assassin" | "ass" | "as" => "ass".into(),
        other => other.to_string(),
    }
}

impl XpTable {
    /// Parse un TSV `experience.txt` v1.12 (ligne `MaxLvl` + lignes niveaux).
    pub fn parse(txt: &str) -> XpTable {
        let mut table = XpTable::default();
        let mut rows = txt.lines().filter(|l| !l.trim().is_empty()).map(crate::stats::split_tsv);
        let Some(hdr) = rows.next() else { return table };
        // Colonnes classes : tout sauf `Level` et `ExpRatio`.
        let class_cols: Vec<(usize, String)> = hdr
            .iter()
            .enumerate()
            .filter(|(_, h)| {
                let h = h.to_ascii_lowercase();
                !h.is_empty() && h != "level" && h != "expratio" && !h.starts_with('*')
            })
            .map(|(i, h)| (i, h.clone()))
            .collect();
        let expratio_c = hdr.iter().position(|h| h.eq_ignore_ascii_case("ExpRatio"));
        let cell = |r: &[String], i: usize| -> String {
            r.get(i).cloned().unwrap_or_default()
        };
        for r in rows {
            let key = cell(&r, 0);
            if key.is_empty() {
                continue;
            }
            if key.eq_ignore_ascii_case("MaxLvl") {
                for (i, col) in &class_cols {
                    let code = class_key(col);
                    let lvl: u32 = cell(&r, *i).trim().parse().unwrap_or(99);
                    if let Some(cx) = table.classes.get_mut(&code) {
                        cx.max_level = lvl;
                    }
                }
                continue;
            }
            let Ok(level) = key.trim().parse::<usize>() else { continue };
            if let Some(ec) = expratio_c {
                let v: u32 = cell(&r, ec).trim().parse().unwrap_or(0);
                if table.exp_ratio.len() <= level {
                    table.exp_ratio.resize(level + 1, 0);
                }
                table.exp_ratio[level] = v;
            }
            for (i, col) in &class_cols {
                let code = class_key(col);
                let xp: u64 = cell(&r, *i).trim().parse().unwrap_or(0);
                let entry = table
                    .classes
                    .entry(code.clone())
                    .or_insert_with(|| ClassXp {
                        code: code.clone(),
                        column: col.clone(),
                        max_level: 99,
                        thresholds: Vec::new(),
                    });
                if entry.thresholds.len() <= level {
                    entry.thresholds.resize(level + 1, 0);
                }
                entry.thresholds[level] = xp;
            }
        }
        table.order = ["ama", "sor", "nec", "pal", "bar", "dru", "ass"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        table
    }

    /// Table globale embarquée (parse au premier accès).
    pub fn global() -> &'static XpTable {
        static TABLE: OnceLock<XpTable> = OnceLock::new();
        TABLE.get_or_init(|| XpTable::parse(XP_TXT))
    }

    /// Courbe d'une classe (code court, code héros ou nom complet).
    pub fn class(&self, class: &str) -> Option<&ClassXp> {
        self.classes.get(&class_key(class))
    }

    /// Niveau atteint avec `xp` pour la classe donnée (défaut 1 si classe
    /// inconnue — jamais de panic).
    pub fn level_for(&self, class: &str, xp: u64) -> u32 {
        self.class(class).map_or(1, |c| c.level_for(xp))
    }

    /// XP restants avant le niveau suivant (0 si classe inconnue ou max).
    pub fn xp_to_next(&self, class: &str, level: u32, xp: u64) -> u64 {
        self.class(class).map_or(0, |c| c.xp_to_next(level, xp))
    }

    /// Codes canoniques dans l'ordre de la table.
    pub fn class_codes(&self) -> &[String] {
        &self.order
    }

    /// `ExpRatio` au niveau `level` (0 si hors table).
    pub fn exp_ratio(&self, level: u32) -> u32 {
        self.exp_ratio.get(level as usize).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_seven_classes_max99() {
        let t = XpTable::global();
        assert_eq!(t.class_codes().len(), 7);
        for code in t.class_codes() {
            let c = t.class(code).unwrap();
            assert_eq!(c.max_level, 99, "{code} : MaxLvl 99");
            assert_eq!(c.rows(), 100, "{code} : lignes 0..=99");
        }
    }

    #[test]
    fn thresholds_match_known_values() {
        let t = XpTable::global();
        // Ligne 0 = 0 (niveau 1 gratuit), ligne 1 = 500 (niveau 2)…
        let bar = t.class("bar").unwrap();
        assert_eq!(bar.threshold(1), 0);
        assert_eq!(bar.threshold(2), 500);
        assert_eq!(bar.threshold(3), 1500);
        assert_eq!(bar.threshold(0), 0);
        // Dernière ligne : ~3.83 Md (inatteignable, MaxLvl = 99) pour toutes.
        for code in t.class_codes() {
            assert_eq!(t.class(code).unwrap().raw_row(99), 3_837_739_017);
        }
    }

    #[test]
    fn level_for_and_to_next() {
        let t = XpTable::global();
        assert_eq!(t.level_for("bar", 0), 1);
        assert_eq!(t.level_for("bar", 499), 1);
        assert_eq!(t.level_for("bar", 500), 2);
        assert_eq!(t.level_for("bar", 1499), 2);
        assert_eq!(t.level_for("bar", 1500), 3);
        assert_eq!(t.level_for("bar", u64::MAX), 99, "plafonné à MaxLvl");
        assert_eq!(t.xp_to_next("bar", 1, 0), 500);
        assert_eq!(t.xp_to_next("bar", 1, 200), 300);
        assert_eq!(t.xp_to_next("bar", 2, 500), 1000);
        assert_eq!(t.xp_to_next("bar", 99, u64::MAX), 0);
    }

    #[test]
    fn aliases_and_unknown_classes() {
        let t = XpTable::global();
        for alias in ["BA", "ba", "Barbarian", " barbarian "] {
            assert_eq!(t.level_for(alias, 500), 2, "alias {alias}");
        }
        assert_eq!(t.level_for("inconnu", 500), 1, "classe inconnue ⇒ niveau 1");
        assert_eq!(t.xp_to_next("inconnu", 1, 0), 0);
    }

    #[test]
    fn exp_ratio_column_loaded() {
        let t = XpTable::global();
        assert_eq!(t.exp_ratio(0), 1024);
        assert_eq!(t.exp_ratio(99), 5);
    }
}
