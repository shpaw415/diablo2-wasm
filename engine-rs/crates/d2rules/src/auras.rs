//! `auras` — Étape 16a (PLAN-FIDELITE phase 16) : COMPORTEMENTS DES AURAS DE
//! PALADIN ET MALÉDICTIONS DE NÉCROMANCIEN.
//!
//! # Données réelles v1.12 (la table fait foi — chaque golden cite sa ligne)
//! * `assets/patch_skills.txt` — colonnes `aurastate` (auras) /
//!   `auratargetstate` (malédictions), `aurarangecalc` (rayon `ln12` =
//!   Param1 + (n-1)×Param2 yards), `auralencalc` (durée des malédictions
//!   `ln34` = Param3 + (n-1)×Param4 frames), `aurastat1-6` +
//!   `aurastatcalc1-6` (stats portées), coûts `mana`/`lvlmana`.
//!   Ces colonnes n'étaient PAS parsées par [`crate::skills_tree`] : ce
//!   module les lit par-dessus la même table embarquée
//!   ([`crate::skills_tree::SKILLS_TXT`], zéro copie) — les Params/calcs/
//!   courbes EMin continuent de venir du record [`SkillRecord`] global.
//! * `d2w-re/tables/items-ui/states.txt` (183 états) — les 24 états portés
//!   existent : auras `aura=1` (might id 33, prayer 34, holyfire 35, thorns
//!   36, defiance 37, conviction 28, fanaticism 49, sanctuary 47…),
//!   malédictions `curse=1` (amplifydamage id 9, dimvision 23, weaken 19,
//!   ironmaiden 55, terror 56, attract 57, confuse 59, decrepify 60,
//!   lowerresist 61).
//! * `assets/patch_itemstatcost.txt` — ids des stats d'aura résolus par le
//!   dictionnaire [`crate::stats::StatCostTable`] (golden : damagepercent 25,
//!   thorns_percent 131, skill_armor_percent 171, damageresist 36…).
//!
//! # Portée 16a (brief)
//! * **15 auras de paladin** : Holy Fire, Holy Shock, Sanctuary (dégâts par
//!   pulse dans le rayon, résists/immunités réels + synergies
//!   `EDmgSymPerCalc`), Prayer (soin par pulse = courbe EMin), Meditation
//!   (régén mana), Thorns (renvoi de dégâts), Defiance (défense %), Might /
//!   Concentration / Fanaticism (dégâts % du héros), Resist
//!   Fire/Cold/Lightning + Salvation (stats au socle `statlist`), Conviction
//!   (aura ENNEMIE : -AC/-res dans le rayon). Toggle on/off, UNE SEULE
//!   active à la fois (swap), durée infinie tant qu'active, coupée à la mort
//!   du héros, coût mana réel au cast.
//! * **9 malédictions de nécromancien** : Amplify Damage, Dim Vision, Weaken
//!   (« Weep » du brief), Iron Maiden, Terror, Attract, Confuse, Decrepify,
//!   Lower Resist — zone (`aurarange`), durée `auralen` par niveau, UNE par
//!   monstre (recast remplace), effets branchés combat : résists effectifs
//!   ([`World::monster_effective_res`]), dégâts monstre scalés (Weaken/
//!   Decrepify), renvois (Iron Maiden), FSM (Dim Vision = plus d'aggro,
//!   Terror = fuite, Attract/Confuse = in-fighting).
//! * **Socle statlist (mission ③)** : [`ActiveAura::stat_lines`] /
//!   [`ActiveCurse::stat_lines`] produisent les `(id ItemStatCost, valeur)`
//!   des `aurastat` — même interface que [`crate::shrines::ShrineBuff`] :
//!   l'agrégation héros = base + équipement + aura passe par
//!   `StatList::from_sources`.
//!
//! # Conventions MVP documentées (écarts, zéro donnée inventée)
//! * **Pulse = 1 s** ([`AURA_PULSE_MS`] = 25 frames × 40 ms, le rythme de
//!   refresh `srvdofunc` 65/66) : dégâts Holy Fire/Holy Shock/Sanctuary,
//!   soin Prayer (courbe EMin — la stat `hitpoints`/`edns` de la table vaut
//!   0, ELen de Prayer vide : golden cohérent skills_tree), régén Meditation.
//! * **Déterminisme** : les jets de dégâts de pulse ne consomment du RNG que
//!   si l'aura est active ET qu'un monstre non-immunisé est dans le rayon
//!   (même contrat additif que les mods de coup 12c — les chemins historiques
//!   gardent leurs suites de graines) ; les pulses n'émettent AUCUN event
//!   (même silence que les DoT 12c).
//! * **Sanctuary** frappe tous les monstres du rayon (le flag undead
//!   n'existe pas dans les gabarits du moteur) ; **Holy Shock** ne fait que
//!   le pulse de rayon (le +dégâts foudre de mêlée v1.12 passe par les
//!   missiles, hors port) ; **Fanaticism** : la composante `attackrate`
//!   reste au socle statlist (pas de système de vitesse d'attaque).
//! * **Meditation** : régén MVP `max(1, max_mana × ln34 / 40 000)` par pulse
//!   (pas de socle de régén mana dans le moteur) ; la stat
//!   `manarecoverybonus` reste exposée au socle statlist.
//! * Malédictions : appliquées à tous les vivants du rayon, pas d'immunité
//!   de boss ; frappe monstre↔monstre directe (pas de jet de toucher,
//!   simplification MVP) ; **Life Tap** hors portée 16a (absent du brief).
//! * **Transitoire** : l'aura active et les malédictions ne sont JAMAIS
//!   persistées (comme `HitMods`/DoT 12c) — vides au chargement de save.

use crate::calc::{CalcString, EvalContext};
use crate::difficulty::{self, RES_IMMUNE_THRESHOLD};
use crate::skills_hero::HeroLevels;
use crate::skills_tree::{
    ElemType, NoLevels, SkillEvalCtx, SkillLevels, SkillRecord, SkillTreeData, SynergyKind,
};
use crate::statlist::StatLine;
use crate::world::{MonState, World, ATTACK_RANGE, MONSTER_ATK_CD_MS, REPATH_MS, WANDER_RADIUS};
use crate::GameEvent;
use d2missiles::DamageType;
use std::collections::HashMap;
use std::sync::OnceLock;

/// 40 ms par frame D2 (25 ips) — conversions des durées `auralen` (identique
/// à `combat_mods::FRAME_MS_D2`).
pub const D2_FRAME_MS: u64 = 40;

/// Période des effets d'aura : 1 000 ms = 25 frames (refresh `srvdofunc`).
pub const AURA_PULSE_MS: u32 = 25 * 40;

// ---------------------------------------------------------------------------
// Skills implémentés (brief 16a) — codes `skills.txt` exacts
// ---------------------------------------------------------------------------

/// Les 15 auras de paladin implémentées (`patch_skills.txt`, `charclass=pal`).
pub const PALADIN_AURAS: &[&str] = &[
    "Prayer",
    "Resist Fire",
    "Holy Fire",
    "Thorns",
    "Defiance",
    "Resist Cold",
    "Resist Lightning",
    "Concentration",
    "Holy Shock",
    "Sanctuary",
    "Meditation",
    "Fanaticism",
    "Conviction",
    "Salvation",
    "Might",
];

/// Les 9 malédictions de nécromancien implémentées (`charclass=nec`).
pub const NECRO_CURSES: &[&str] = &[
    "Amplify Damage",
    "Dim Vision",
    "Weaken",
    "Iron Maiden",
    "Terror",
    "Attract",
    "Confuse",
    "Decrepify",
    "Lower Resist",
];

// ---------------------------------------------------------------------------
// Erreurs
// ---------------------------------------------------------------------------

/// Erreurs d'activation d'aura / lancement de malédiction (diagnostiques UI).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuraError {
    /// Code absent de `skills.txt`.
    UnknownSkill(String),
    /// Le héros n'a aucun point dans ce skill.
    NotKnown(String),
    /// Le skill n'est ni une aura ni une malédiction implémentée en 16a.
    NotAnAura(String),
    /// Mana insuffisant au cast (`mana` + `lvlmana`×(n-1), plancher
    /// `minmana` — [`SkillTreeData::mana_cost_at`]).
    NotEnoughMana { cost: u32, have: u32 },
}

impl std::fmt::Display for AuraError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuraError::UnknownSkill(s) => write!(f, "skill inconnu : {s}"),
            AuraError::NotKnown(s) => write!(f, "{s} n'est pas appris"),
            AuraError::NotAnAura(s) => write!(f, "{s} n'est pas une aura/malédiction 16a"),
            AuraError::NotEnoughMana { cost, have } => {
                write!(f, "mana insuffisant : {cost} requis, {have} disponibles")
            }
        }
    }
}

impl std::error::Error for AuraError {}

// ---------------------------------------------------------------------------
// Colonnes d'aura (parser additif sur la table embarquée partagée)
// ---------------------------------------------------------------------------

/// Colonnes d'aura d'une ligne `skills.txt` — parsées ICI (additif, la
/// table brute est partagée avec [`crate::skills_tree`], zéro copie).
#[derive(Debug, Default, Clone)]
pub(crate) struct AuraCols {
    /// `aurarangecalc` (ex. `ln12`).
    pub range: Option<CalcString>,
    /// `auralencalc` (durée des malédictions, ex. `ln34`).
    pub len: Option<CalcString>,
    /// `auratargetstate` (malédictions — ex. `amplifydamage`).
    pub target_state: String,
    /// Paires `aurastatN` / `aurastatcalcN` (N = 1..6), ordre de la table.
    pub stats: Vec<(String, Option<CalcString>)>,
}

/// Colonnes d'aura par code (parse au premier accès ; lenient — table
/// illisible ⇒ vide, jamais de panic).
pub(crate) fn aura_cols(code: &str) -> Option<&'static AuraCols> {
    static COLS: OnceLock<HashMap<String, AuraCols>> = OnceLock::new();
    let map = COLS.get_or_init(|| {
        let mut map: HashMap<String, AuraCols> = HashMap::new();
        let mut rows = crate::skills_tree::SKILLS_TXT
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(crate::stats::split_tsv);
        let Some(hdr) = rows.next() else { return map };
        let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
        let c_code = col("skill");
        let c_range = col("aurarangecalc");
        let c_len = col("auralencalc");
        // 16c (réparation minimale de l'arbre partagé) : champ `target_state`
        // documenté sur `AuraCols` — lit sa colonne `auratargetstate`.
        let c_target = col("auratargetstate");
        let c_stat: Vec<Option<usize>> =
            (1..=6).map(|n| col(&format!("aurastat{n}"))).collect();
        let c_calc: Vec<Option<usize>> =
            (1..=6).map(|n| col(&format!("aurastatcalc{n}"))).collect();
        for r in rows {
            let Some(code) = c_code.and_then(|c| r.get(c)) else { continue };
            if code.is_empty() || code.starts_with('*') {
                continue;
            }
            let cell = |c: Option<usize>| -> String {
                c.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default()
            };
            let calc = |c: Option<usize>| -> Option<CalcString> {
                let v = cell(c);
                (!v.is_empty()).then(|| CalcString::parse(&v))
            };
            let stats = (0..6)
                .map(|i| (cell(c_stat[i]), calc(c_calc[i])))
                .filter(|(name, _)| !name.is_empty())
                .collect();
            map.insert(
                code.to_ascii_lowercase(),
                AuraCols { range: calc(c_range), len: calc(c_len), target_state: cell(c_target), stats },
            );
        }
        map
    });
    map.get(&code.to_ascii_lowercase())
}

/// Canonise un code de skill via la table (casse réelle de `skills.txt`).
fn canon(code: &str) -> String {
    SkillTreeData::global()
        .get(code)
        .map(|r| r.code.clone())
        .unwrap_or_else(|| code.to_string())
}

// ---------------------------------------------------------------------------
// Contexte d'évaluation (formules aurastat/aurarange/auralen)
// ---------------------------------------------------------------------------

/// Paire Param d'un record (`ln34` = Param3 + (n-1)×Param4) — port local du
/// helper privé de skills_tree.
fn param_pair(rec: &SkillRecord, a: usize, b: usize, lvl: u32) -> i64 {
    let base = rec.params[a.saturating_sub(1)];
    let incr = rec.params[b.saturating_sub(1)];
    base + (lvl.max(1) as i64 - 1) * incr
}

/// Contexte d'évaluation des formules d'aura/malédiction : record courant à
/// SON niveau + niveaux du héros (formules inter-skills comme
/// `skill('Resist Fire'.blvl)` de Resist Fire L102 ou
/// `skill('Prayer'.edns)` de Meditation L122 — même sémantique que
/// `FullEvalCtx` de skills_tree).
struct AuraEvalCtx<'a> {
    inner: SkillEvalCtx<'a>,
}

impl<'a> AuraEvalCtx<'a> {
    fn new(rec: &'a SkillRecord, own_lvl: u32, levels: &'a dyn SkillLevels) -> AuraEvalCtx<'a> {
        AuraEvalCtx { inner: SkillEvalCtx::new(rec, own_lvl, levels) }
    }
}

impl EvalContext for AuraEvalCtx<'_> {
    fn skill_level(&mut self, skill: &str, qualifier: &str) -> i64 {
        if skill.eq_ignore_ascii_case(&self.inner.rec.code) {
            return self.inner.skill_level(skill, qualifier);
        }
        // Skill ÉTRANGER : niveaux du héros + record des tables.
        let lvl = self.inner.levels.blvl(skill);
        let Some(rec) = SkillTreeData::global().get(skill) else { return 0 };
        match qualifier {
            "blvl" | "lvl" | "" => lvl as i64,
            q @ ("edmn" | "edmx" | "edns") => {
                let data = SkillTreeData::global();
                let lvl = lvl.max(1);
                match q {
                    "edmn" => data.elem_damage_at(rec, lvl).0,
                    "edmx" => data.elem_damage_at(rec, lvl).1,
                    _ => data.elem_len_at(rec, lvl),
                }
            }
            q if q.starts_with("par") => {
                let n: usize = q[3..].parse().unwrap_or(0);
                if (1..=8).contains(&n) {
                    rec.params[n - 1]
                } else {
                    0
                }
            }
            q if q.starts_with("ln") || q.starts_with("dm") => {
                let digits = &q[2..];
                if digits.len() == 2 {
                    if let (Ok(a), Ok(b)) =
                        (digits[..1].parse::<usize>(), digits[1..].parse::<usize>())
                    {
                        if (1..=8).contains(&a) && (1..=8).contains(&b) {
                            return param_pair(rec, a, b, lvl);
                        }
                    }
                }
                0
            }
            _ => lvl as i64,
        }
    }

    fn stat_value(&mut self, stat: &str, qualifier: &str) -> i64 {
        self.inner.stat_value(stat, qualifier)
    }

    fn missile_prop(&mut self, missile: &str, qualifier: &str) -> i64 {
        self.inner.missile_prop(missile, qualifier)
    }

    fn bare(&mut self, name: &str) -> i64 {
        self.inner.bare(name)
    }
}

/// Évalue une formule du record à `lvl` (niveaux héros pour les synergies).
fn eval_calc(
    rec: &SkillRecord,
    lvl: u32,
    calc: &CalcString,
    levels: &dyn SkillLevels,
) -> i64 {
    calc.eval(&mut AuraEvalCtx::new(rec, lvl, levels))
}

/// Tokens `bare` connus de l'évaluateur — tout autre mot nu (ex. `toht`,
/// Fanaticism L124) est un token D2 SANS équivalent évalué : la stat est
/// IGNORÉE plutôt que fausse (le placeholder `1` du langage Go écrirait une
/// donnée inventée ; documenté au contrat de [`ActiveAura::stat_pairs`]).
const KNOWN_BARE_TOKENS: &[&str] = &["lvl", "sklvl", "ulvl", "edmn", "edmx", "edns"];

/// `true` si la formule n'est pas un mot nu hors liste (parN/lnXX/dmXX,
/// min(...), skill(...)… sont évaluables).
fn calc_is_resolvable(calc: &CalcString) -> bool {
    let src = calc.source().trim();
    if !src.chars().all(|c| c.is_ascii_alphabetic()) {
        return true;
    }
    KNOWN_BARE_TOKENS.contains(&src)
}

/// Évalue les `aurastatN` × `aurastatcalcN` du record `code` (paires
/// nom/valeur, ordre de la table ; tokens D2 non résolus ignorés).
fn stat_pairs_of(code: &str, rec: &SkillRecord, level: u32, levels: &dyn SkillLevels) -> Vec<(String, i64)> {
    let Some(cols) = aura_cols(code) else { return Vec::new() };
    cols.stats
        .iter()
        .filter(|(_, calc)| calc.as_ref().map(calc_is_resolvable).unwrap_or(false))
        .filter_map(|(name, calc)| calc.as_ref().map(|c| (name.clone(), eval_calc(rec, level, c, levels))))
        .collect()
}

/// Résout des paires (nom, valeur) en ids `ItemStatCost` v1.12 (noms absents
/// du dictionnaire ignorés — jamais de panic).
fn stat_lines_of(pairs: &[(String, i64)]) -> Vec<StatLine> {
    let dict = crate::stats::StatCostTable::global();
    pairs
        .iter()
        .filter_map(|(name, v)| dict.get(name).map(|r| StatLine::new(r.id, *v)))
        .collect()
}

// ---------------------------------------------------------------------------
// Aura active (paladin)
// ---------------------------------------------------------------------------

/// Aura de paladin ACTIVE (une seule à la fois — swap au toggle). Durée
/// infinie tant qu'active ; coupée à la mort du héros ; non persistée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveAura {
    /// Code `skills.txt` (ex. `"Holy Fire"`).
    pub skill: String,
    /// Points investis (niveau du skill).
    pub level: u32,
}

impl ActiveAura {
    /// Construit si `code` est une des 15 auras implémentées.
    pub fn new(code: &str, level: u32) -> Option<ActiveAura> {
        PALADIN_AURAS
            .iter()
            .any(|c| c.eq_ignore_ascii_case(code))
            .then(|| ActiveAura { skill: canon(code), level: level.max(1) })
    }

    /// Record table du skill.
    pub fn rec(&self) -> &'static SkillRecord {
        SkillTreeData::global().get(&self.skill).expect("aura 16a = record table")
    }

    /// Rayon réel (`aurarangecalc`, `ln12` yards — 1 tuile = 1 yard).
    pub fn radius(&self) -> f32 {
        let rec = self.rec();
        aura_cols(&self.skill)
            .and_then(|c| c.range.as_ref())
            .map(|calc| eval_calc(rec, self.level, calc, &NoLevels).max(0) as f32)
            .unwrap_or(0.0)
    }

    /// Aura ENNEMIE ? (stats toutes ≤ 0 — Conviction L125 : -AC/-res aux
    /// monstres du rayon). Générique, piloté par la donnée.
    pub fn is_enemy_aura(&self) -> bool {
        let pairs = self.stat_pairs(&NoLevels);
        !pairs.is_empty() && pairs.iter().all(|(_, v)| *v <= 0)
    }

    /// Paire (élément, EMin, EMax) des auras de DÉGÂTS par pulse —
    /// Holy Fire (feu, L104 : 2/6 + [1,2,3,5,7]), Holy Shock (foudre, L120 :
    /// 1/10 + courbe EMax), Sanctuary (magie, L121 : 8/16 + courbes).
    /// Courbes réelles `EMin`/`EMax` + `EMinLev`/`EMaxLev` ; `None` pour les
    /// auras sans dégâts (Prayer soigne via [`Self::heal_per_pulse`]).
    pub fn pulse_damage(&self) -> Option<(DamageType, i64, i64)> {
        let rec = self.rec();
        let elem = match rec.e_type {
            ElemType::Fire => DamageType::Fire,
            ElemType::Lightning => DamageType::Lightning,
            ElemType::Magic => DamageType::Magic,
            _ => return None,
        };
        if rec.e_min == 0 && rec.e_max == 0 {
            return None;
        }
        let (mn, mx) = SkillTreeData::global().elem_damage_at(rec, self.level);
        Some((elem, mn.max(0), mx.max(mn).max(0)))
    }

    /// Soin par pulse — Prayer L101 uniquement (courbe EMin = 2 +
    /// [1,1,2,2,3]/niveau ; le `hitpoints`=`edns` de la table vaut 0, ELen
    /// vide — la quantité soignée vient de la courbe de dégâts du record,
    /// lecture D2 du srvdofunc 65).
    pub fn heal_per_pulse(&self) -> Option<i64> {
        if self.skill != "Prayer" {
            return None;
        }
        let (mn, _) = SkillTreeData::global().elem_damage_at(self.rec(), self.level);
        Some(mn.max(0))
    }

    /// Synergie élémentaire de l'aura (EDmgSymPerCalc — Holy Fire L104 :
    /// Resist Fire×18 + Salvation×6 ; Holy Shock idem foudre ; Sanctuary :
    /// Cleansing×7) en %.
    pub fn synergy_percent(&self, levels: &dyn SkillLevels) -> i64 {
        SkillTreeData::global().synergy_percent(self.rec(), SynergyKind::Elemental, levels)
    }

    /// Stats portées (`aurastat1-6` × `aurastatcalc1-6`) — paires (nom,
    /// valeur) évaluées au niveau de l'aura (niveaux héros pour les formules
    /// inter-skills). Ex. Might L100 → `[("damagepercent", 40 + 10×(n-1))]`.
    pub fn stat_pairs(&self, levels: &dyn SkillLevels) -> Vec<(String, i64)> {
        stat_pairs_of(&self.skill, self.rec(), self.level, levels)
    }

    /// Pont socle `statlist` (mission ③) : les mêmes stats résolues en ids
    /// `ItemStatCost` — agrégeable via `StatList::from_sources` (même
    /// interface que [`crate::shrines::ShrineBuff::stat_lines`]). Les noms
    /// absents du dictionnaire v1.12 sont ignorés (jamais de panic).
    pub fn stat_lines(&self, levels: &dyn SkillLevels) -> Vec<StatLine> {
        stat_lines_of(&self.stat_pairs(levels))
    }

    /// Somme d'une stat nommée portée par l'aura (0 si absente).
    pub fn stat_sum(&self, name: &str, levels: &dyn SkillLevels) -> i64 {
        self.stat_pairs(levels)
            .iter()
            .filter(|(n, _)| n == name)
            .map(|(_, v)| *v)
            .sum()
    }
}

// ---------------------------------------------------------------------------
// Malédiction (nécromancien)
// ---------------------------------------------------------------------------

/// Comportement d'une malédiction (`auratargetstate` — states.txt `curse=1`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurseKind {
    /// `amplifydamage` (id 9) — ResDm -100 % (dégâts physiques ×2).
    AmplifyDamage,
    /// `dimvision` (id 23) — aveuglé : plus d'aggro (errance seule).
    DimVision,
    /// `weaken` (id 19) — dégâts du monstre -33 %.
    Weaken,
    /// `ironmaiden` (id 55) — renvoi 200 % (+25/niv) des dégâts infligés.
    IronMaiden,
    /// `terror` (id 56) — fuite (pattern panique Fallen).
    Terror,
    /// `attract` (id 57) — les autres monstres attaquent la cible.
    Attract,
    /// `confuse` (id 59) — idem Attract au MVP (monstres s'entre-attaquent).
    Confuse,
    /// `decrepify` (id 60) — dégâts -50 %, vitesse -50 %, ResDm -50 %.
    Decrepify,
    /// `lowerresist` (id 61) — res feu/foudre/froid/POISON -25 -70×(n-1) %
    /// (aurastat1-4 de la table v1.12).
    LowerResist,
}

impl CurseKind {
    /// Résolution DATA-DRIVEN depuis la colonne `auratargetstate` de la
    /// table (ex. `amplifydamage` — états `curse=1` de states.txt). Les
    /// `auratargetstate` des auras de paladin (might, prayer…) et des skills
    /// sans malédiction tombent dans `_` ⇒ `None`.
    pub fn of_code(code: &str) -> Option<CurseKind> {
        let cols = aura_cols(code)?;
        Some(match cols.target_state.as_str() {
            "amplifydamage" => CurseKind::AmplifyDamage,
            "dimvision" => CurseKind::DimVision,
            "weaken" => CurseKind::Weaken,
            "ironmaiden" => CurseKind::IronMaiden,
            "terror" => CurseKind::Terror,
            "attract" => CurseKind::Attract,
            "confuse" => CurseKind::Confuse,
            "decrepify" => CurseKind::Decrepify,
            "lowerresist" => CurseKind::LowerResist,
            _ => return None,
        })
    }

    /// Code `skills.txt` canonique de la malédiction.
    pub fn code(self) -> &'static str {
        match self {
            CurseKind::AmplifyDamage => "Amplify Damage",
            CurseKind::DimVision => "Dim Vision",
            CurseKind::Weaken => "Weaken",
            CurseKind::IronMaiden => "Iron Maiden",
            CurseKind::Terror => "Terror",
            CurseKind::Attract => "Attract",
            CurseKind::Confuse => "Confuse",
            CurseKind::Decrepify => "Decrepify",
            CurseKind::LowerResist => "Lower Resist",
        }
    }
}

/// Malédiction ACTIVE sur un monstre (`MonData::curse`) — UNE par monstre
/// (recast = remplacement), durée `auralen` décrémentée au tick, transitoire
/// (jamais persistée).
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveCurse {
    /// Code `skills.txt`.
    pub skill: String,
    /// Points investis.
    pub level: u32,
    /// Rayon d'application (`aurarangecalc` au cast, yards).
    pub radius: f32,
    /// Durée restante en ms (`auralencalc` frames × 40 ms).
    pub remaining_ms: u64,
}

impl ActiveCurse {
    /// Construit si `code` est une des 9 malédictions ; durée/rayon réels du
    /// niveau donné.
    pub fn new(code: &str, level: u32) -> Option<ActiveCurse> {
        CurseKind::of_code(code).map(|_| ActiveCurse {
            skill: canon(code),
            level: level.max(1),
            radius: curse_radius(code, level.max(1)),
            remaining_ms: curse_duration_ms(code, level.max(1)),
        })
    }

    /// Famille de comportement.
    pub fn kind(&self) -> CurseKind {
        CurseKind::of_code(&self.skill).expect("malédiction 16a")
    }

    /// Un pas de temps simulé. `true` tant que la malédiction est active.
    pub fn tick(&mut self, dt_ms: u32) -> bool {
        self.remaining_ms = self.remaining_ms.saturating_sub(dt_ms as u64);
        self.remaining_ms > 0
    }

    /// Malédiction active ?
    pub fn is_active(&self) -> bool {
        self.remaining_ms > 0
    }

    /// Record table du skill.
    pub fn rec(&self) -> &'static SkillRecord {
        SkillTreeData::global().get(&self.skill).expect("malédiction 16a = record table")
    }

    /// Stats portées (`aurastat1-3` des malédictions — Amplify L68
    /// `damageresist` -par5, Weaken L74 `damagepercent` -par5, Decrepify L89
    /// ×3, Lower Resist L93 res feu/foudre/froid -dm56 ; les malédictions
    /// sans `aurastat` (Dim Vision/Terror/Confuse/Attract/IM) sont vides —
    /// leurs effets sont des comportements FSM/renvoi, pas des stats).
    pub fn stat_pairs(&self) -> Vec<(String, i64)> {
        stat_pairs_of(&self.skill, self.rec(), self.level, &NoLevels)
    }

    /// Pont socle `statlist` (mission ③), même contrat que l'aura.
    pub fn stat_lines(&self) -> Vec<StatLine> {
        stat_lines_of(&self.stat_pairs())
    }

    /// Somme d'une stat nommée portée par la malédiction (0 si absente).
    pub fn stat_sum(&self, name: &str) -> i64 {
        self.stat_pairs().iter().filter(|(n, _)| n == name).map(|(_, v)| *v).sum()
    }

    /// Renvoi Iron Maiden (`calc1` = `ln56` = 200 + 25×(n-1) %, L78) —
    /// 0 pour les autres malédictions.
    pub fn iron_maiden_pct(&self) -> i64 {
        if self.kind() != CurseKind::IronMaiden {
            return 0;
        }
        let rec = self.rec();
        rec.calcs[0]
            .as_ref()
            .map(|c| eval_calc(rec, self.level, c, &NoLevels))
            .unwrap_or(0)
            .max(0)
    }
}

/// Rayon au cast (`aurarangecalc` — `ln12` yards).
fn curse_radius(code: &str, level: u32) -> f32 {
    let rec = SkillTreeData::global().get(code);
    match (rec, aura_cols(code).and_then(|c| c.range.as_ref())) {
        (Some(rec), Some(calc)) => eval_calc(rec, level, calc, &NoLevels).max(0) as f32,
        _ => 0.0,
    }
}

/// Durée au cast (`auralencalc` — `ln34` frames × 40 ms).
fn curse_duration_ms(code: &str, level: u32) -> u64 {
    let rec = SkillTreeData::global().get(code);
    match (rec, aura_cols(code).and_then(|c| c.len.as_ref())) {
        (Some(rec), Some(calc)) => {
            (eval_calc(rec, level, calc, &NoLevels).max(0) as u64) * D2_FRAME_MS
        }
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Branchement World — état + API publique (toggle / cast / lectures)
// ---------------------------------------------------------------------------

/// État d'aura du monde : l'aura de paladin ACTIVE (transitoire) +
/// accumulateur du pulse 1 s. Non persisté (vides au chargement de save).
#[derive(Debug, Clone, Default)]
pub struct AuraState {
    /// Aura active (`None` = aucune).
    pub active: Option<ActiveAura>,
    /// Accumulateur ms du pulse (privé — avance dans [`tick`]).
    pulse_acc: u32,
}

/// Applique `pct` % (signé) : `v × (100 + pct) / 100`, part entière.
fn scaled(v: u32, pct: i64) -> u32 {
    ((v as i64 * (100 + pct).max(0)) / 100) as u32
}

/// Ajoute une stat de résist nommée au slot correspondant
/// `[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]`.
fn add_res_stat(res: &mut [i32; 6], name: &str, v: i64) {
    let slot = match name {
        "damageresist" => Some(0),
        "fireresist" => Some(2),
        "lightresist" => Some(3),
        "coldresist" => Some(4),
        "poisonresist" => Some(5),
        _ => None,
    };
    if let Some(s) = slot {
        res[s] = (res[s] as i64 + v).clamp(-1000, 1000) as i32;
    }
}

/// Transition de mort d'un monstre (identique au bloc DoT 12c de
/// `update_monsters` — anim DD, retrait à la fin de l'anim).
fn kill_monster(w: &mut World, i: usize) {
    let dat = &mut w.mon_dat[i];
    dat.state = MonState::Dying;
    dat.dying_ms = 0;
    dat.anim_ms = 0;
    dat.path.clear();
    let ent = &mut w.mon_ent[i];
    ent.anim = crate::AnimState::DD;
    ent.anim_frame = 0;
}

impl World {
    // --- Lectures ---------------------------------------------------------

    /// Aura de paladin active (lecture — HUD).
    pub fn active_aura(&self) -> Option<&ActiveAura> {
        self.auras.active.as_ref()
    }

    /// Malédiction portée par le monstre `index` (lecture). `None` hors bornes.
    pub fn monster_curse(&self, index: usize) -> Option<&ActiveCurse> {
        self.mon_dat.get(index).and_then(|d| d.curse.as_ref())
    }

    /// % de dégâts ajoutés au héros par l'aura active (`damagepercent` —
    /// Might L100 `ln34` = 40+10×(n-1), Concentration L115 = 60+15×(n-1),
    /// Fanaticism L124 `ln56/2` = (50+17×(n-1))/2).
    pub fn hero_damage_pct(&self) -> i64 {
        self.auras
            .active
            .as_ref()
            .map(|a| a.stat_sum("damagepercent", &NoLevels))
            .unwrap_or(0)
            .max(0)
    }

    /// % de défense ajoutés au héros (`skill_armor_percent` POSITIF —
    /// Defiance L106 `ln34` = 70+10×(n-1) ; le négatif de Conviction est
    /// côté ennemis).
    pub fn hero_defense_pct(&self) -> i64 {
        let Some(a) = &self.auras.active else { return 0 };
        if a.is_enemy_aura() {
            return 0;
        }
        a.stat_sum("skill_armor_percent", &NoLevels).max(0)
    }

    /// % de renvoi Thorns (`thorns_percent` L105 `ln34` = 250 + 40×(n-1)).
    pub fn hero_thorns_pct(&self) -> i64 {
        self.auras
            .active
            .as_ref()
            .map(|a| a.stat_sum("thorns_percent", &NoLevels))
            .unwrap_or(0)
            .max(0)
    }

    /// Résists EFFECTIFS du monstre `index` : base de spawn + malédiction
    /// (`damageresist`/`fireresist`/`lightresist`/`coldresist`/
    /// `poisonresist`) + aura ennemie Conviction dans le rayon (L125 :
    /// `-min(ln34,150)` sur feu/froid/foudre). Ordre
    /// `[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]`.
    pub fn monster_effective_res(&self, index: usize) -> [i32; 6] {
        let Some(d) = self.mon_dat.get(index) else { return [0; 6] };
        let mut res = d.res;
        if let Some(c) = &d.curse {
            for (name, v) in c.stat_pairs() {
                add_res_stat(&mut res, &name, v);
            }
        }
        if let Some(a) = &self.auras.active {
            if a.is_enemy_aura() {
                let e = &self.mon_ent[index];
                if crate::world::dist(e.x - self.hero_ent.x, e.y - self.hero_ent.y) <= a.radius() {
                    for (name, v) in a.stat_pairs(&NoLevels) {
                        add_res_stat(&mut res, &name, v);
                    }
                }
            }
        }
        res
    }

    /// AC EFFECTIF du monstre `index` (Conviction L125 : `-dm56` =
    /// 40 + 100×(n-1) en négatif, plancher 0 ; autres auras/malédictions :
    /// inchangé).
    pub fn monster_effective_ac(&self, index: usize) -> u32 {
        let Some(d) = self.mon_dat.get(index) else { return 0 };
        let mut ac = d.ac as i64;
        if let Some(a) = &self.auras.active {
            if a.is_enemy_aura() {
                let e = &self.mon_ent[index];
                if crate::world::dist(e.x - self.hero_ent.x, e.y - self.hero_ent.y) <= a.radius() {
                    ac += a.stat_sum("skill_armor_percent", &NoLevels);
                }
            }
        }
        ac.max(0) as u32
    }

    /// Vitesse du monstre réduite par malédiction ? (Decrepify L89 :
    /// `velocitypercent` -50 % — appliqué ×1/2, même règle que le froid 12c.)
    pub(crate) fn monster_slowed(&self, index: usize) -> bool {
        self.mon_dat
            .get(index)
            .and_then(|d| d.curse.as_ref())
            .map(|c| c.stat_sum("velocitypercent") <= -50)
            .unwrap_or(false)
    }

    // --- Actions ----------------------------------------------------------

    /// Active/coupe l'aura `code` (toggle) — UNE SEULE active : activer une
    /// autre aura SWAP (l'ancienne se coupe, règle D2). Le héros doit
    /// CONNAÎTRE le skill (≥ 1 point) et payer le coût mana de la table au
    /// cast (`mana` + `lvlmana`×(n-1) — la plupart des auras coûtent 0).
    /// Renvoie `Ok(true)` si l'aura est désormais active, `Ok(false)` si elle
    /// vient d'être coupée ; état inchangé sur `Err`.
    pub fn toggle_aura(&mut self, code: &str) -> Result<bool, AuraError> {
        let level = self
            .hero
            .skills
            .iter()
            .find(|s| s.code.eq_ignore_ascii_case(code) && s.level > 0)
            .map(|s| s.level)
            .ok_or_else(|| {
                if SkillTreeData::global().get(code).is_none() {
                    AuraError::UnknownSkill(canon(code))
                } else {
                    AuraError::NotKnown(canon(code))
                }
            })?;
        let Some(aura) = ActiveAura::new(code, level) else {
            return Err(AuraError::NotAnAura(canon(code)));
        };
        // Déjà active → OFF.
        if let Some(a) = &self.auras.active {
            if a.skill.eq_ignore_ascii_case(&aura.skill) {
                self.auras.active = None;
                return Ok(false);
            }
        }
        // Coût mana réel de la table (Prayer 16 + 3/niv, Sanctuary 1, autres 0).
        let cost = SkillTreeData::global().mana_cost_at(aura.rec(), level).max(0) as u32;
        if self.hero.stats.mana < cost {
            return Err(AuraError::NotEnoughMana { cost, have: self.hero.stats.mana });
        }
        self.hero.stats.mana -= cost;
        self.auras.active = Some(aura); // swap implicite (l'ancienne est remplacée)
        Ok(true)
    }

    /// Lance la malédiction `code` au point `(x, y)` (tuiles continues) :
    /// TOUS les vivants du rayon `aurarange` reçoivent la malédiction (une
    /// seule par monstre — recast remplace), durée `auralen` du niveau.
    /// Coût mana réel de la table, débité même si aucun monstre n'est touché
    /// (le cast a bien eu lieu). Renvoie les indices maudits (ordre de la
    /// table interne, déterministe).
    pub fn cast_curse(&mut self, x: f32, y: f32, code: &str) -> Result<Vec<usize>, AuraError> {
        let level = self
            .hero
            .skills
            .iter()
            .find(|s| s.code.eq_ignore_ascii_case(code) && s.level > 0)
            .map(|s| s.level)
            .ok_or_else(|| {
                if SkillTreeData::global().get(code).is_none() {
                    AuraError::UnknownSkill(canon(code))
                } else {
                    AuraError::NotKnown(canon(code))
                }
            })?;
        if CurseKind::of_code(code).is_none() {
            return Err(AuraError::NotAnAura(canon(code)));
        }
        let rec = SkillTreeData::global().get(code).expect("malédiction validée = record");
        let cost = SkillTreeData::global().mana_cost_at(rec, level).max(0) as u32;
        if self.hero.stats.mana < cost {
            return Err(AuraError::NotEnoughMana { cost, have: self.hero.stats.mana });
        }
        let curse = ActiveCurse::new(code, level).expect("malédiction validée");
        self.hero.stats.mana -= cost;
        let radius = curse.radius;
        let mut hit = Vec::new();
        for i in 0..self.mon_dat.len() {
            if self.mon_dat[i].state == MonState::Dying {
                continue;
            }
            let e = &self.mon_ent[i];
            if crate::world::dist(e.x - x, e.y - y) <= radius {
                self.mon_dat[i].curse = Some(curse.clone());
                hit.push(i);
            }
        }
        Ok(hit)
    }

    // --- Hooks internes (appelés par world.rs, additifs) -------------------

    /// Défense du héros après aura (Defiance — [`World::hero_defense_pct`]).
    pub(crate) fn scale_hero_defense(&self, def: u32) -> u32 {
        scaled(def, self.hero_defense_pct())
    }

    /// Dégâts du héros après aura (Might / Concentration / Fanaticism).
    pub(crate) fn scale_hero_damage(&self, dmg: u32) -> u32 {
        scaled(dmg, self.hero_damage_pct())
    }

    /// Dégâts du monstre `i` après malédiction (Weaken L74 -33 % / Decrepify
    /// L89 -50 % — `damagepercent` négatif ; Amplify/autres : inchangés).
    pub(crate) fn scale_monster_damage(&self, i: usize, dmg: u32) -> u32 {
        let pct = self
            .mon_dat
            .get(i)
            .and_then(|d| d.curse.as_ref())
            .map(|c| c.stat_sum("damagepercent"))
            .unwrap_or(0);
        scaled(dmg, pct)
    }

    /// Renvoi de dégâts quand le monstre `i` touche le héros en mêlée :
    /// Thorns (aura, `thorns_percent` % des dégâts infligés) + Iron Maiden
    /// (malédiction, `ln56` %, L78). Percentages MULTIPLICATIFS (250 % du
    /// coup = ×2,5 — contrairement aux bonus `damagepercent` additifs) ;
    /// peut abattre le monstre (même transition de mort que les DoT 12c).
    pub(crate) fn reflect_on_hero_hit(&mut self, i: usize, dealt: u32) {
        if self.mon_dat[i].state == MonState::Dying || dealt == 0 {
            return;
        }
        let mut pct = self.hero_thorns_pct();
        if let Some(c) = self.mon_dat[i].curse.as_ref() {
            pct += c.iron_maiden_pct();
        }
        if pct <= 0 {
            return;
        }
        let refl = ((dealt as i64 * pct) / 100).max(0) as u32;
        if refl == 0 {
            return;
        }
        self.mon_dat[i].hp = self.mon_dat[i].hp.saturating_sub(refl);
        if self.mon_dat[i].hp == 0 {
            kill_monster(self, i);
        }
    }

    /// L'aura se coupe à la mort du héros (états perdus — respawn nu).
    pub(crate) fn on_respawn(&mut self) {
        self.auras.active = None;
        self.auras.pulse_acc = 0;
    }
}

// ---------------------------------------------------------------------------
// Tick global : pulse des auras (dégâts / soin / régén)
// ---------------------------------------------------------------------------

/// Tick des auras (appelé par `World::tick` APRÈS `update_monsters` — les
/// indices ne bougent pas : les pulses n'émettent AUCUN event, même contrat
/// silencieux que les DoT 12c).
pub(crate) fn tick(w: &mut World, dt: u32, _ev: &mut Vec<GameEvent>) {
    if w.hero.dead_ms > 0 {
        return;
    }
    let Some(aura) = w.auras.active.clone() else { return };
    w.auras.pulse_acc = w.auras.pulse_acc.saturating_add(dt);
    if w.auras.pulse_acc < AURA_PULSE_MS {
        return;
    }
    w.auras.pulse_acc %= AURA_PULSE_MS;
    let levels = HeroLevels::new(&w.hero.skills, w.hero.stats.level);

    // Valeurs dépendant des niveaux du héros, calculées AVANT toute mutation
    // (le boucle de dégâts mute `w` — le prêt immuable de `levels` finit ici).
    let syn = aura.synergy_percent(&levels).max(0);
    let med_bonus = if aura.skill == "Meditation" {
        aura.stat_sum("manarecoverybonus", &levels).max(0) as u64
    } else {
        0
    };

    // 1) Dégâts par pulse (Holy Fire / Holy Shock / Sanctuary) — monstres du
    //    rayon, résists EFFECTIFS (malédictions + Conviction incluses), les
    //    immuns (res ≥ 100) sont sautés SANS tirage RNG (déterminisme).
    if let Some((elem, emin, emax)) = aura.pulse_damage() {
        let radius = aura.radius();
        let (hx, hy) = (w.hero_ent.x, w.hero_ent.y);
        for i in 0..w.mon_dat.len() {
            if w.mon_dat[i].state == MonState::Dying || w.mon_dat[i].hp == 0 {
                continue;
            }
            let e = &w.mon_ent[i];
            if crate::world::dist(e.x - hx, e.y - hy) > radius {
                continue;
            }
            let res = w.monster_effective_res(i);
            let r = res[difficulty::Difficulty::resist_index(elem)];
            if r >= RES_IMMUNE_THRESHOLD {
                continue; // immunisé : le type est sauté (0 dégât, zéro RNG)
            }
            let roll = w.rng.range_u32(emin as u32, emax.max(emin) as u32) as u64;
            let dmg = (roll * (100 + syn) as u64 / 100) as u32;
            let dmg = difficulty::apply_resist(dmg, r);
            if dmg == 0 {
                continue;
            }
            w.mon_dat[i].hp = w.mon_dat[i].hp.saturating_sub(dmg);
            if w.mon_dat[i].hp == 0 {
                kill_monster(w, i);
            }
        }
    }

    // 2) Prayer (L101) : soin par pulse — courbe EMin (2 + [1,1,2,2,3]/niv),
    //    plafonné à max_life (jamais de sur-soin).
    if let Some(heal) = aura.heal_per_pulse() {
        let heal = heal as u32;
        if heal > 0 {
            let s = &mut w.hero.stats;
            s.life = (s.life + heal).min(s.max_life);
        }
    }

    // 3) Meditation (L122) : régén mana MVP (`max(1, max_mana × ln34/40 000)`
    //    par pulse — cf. doc module) ; plafonné à max_mana.
    if med_bonus > 0 {
        let regen = ((w.hero.stats.max_mana as u64 * med_bonus) / 40_000).max(1) as u32;
        let s = &mut w.hero.stats;
        s.mana = (s.mana + regen).min(s.max_mana);
    }
}

// ---------------------------------------------------------------------------
// Tick par monstre : malédictions (comportements FSM)
// ---------------------------------------------------------------------------

/// Errance locale (le `wander_target` de world.rs est privé au module) —
/// home ± [`WANDER_RADIUS`], marchable de préférence, 8 essais.
fn wander_target_local(w: &mut World, home: (f32, f32)) -> (i32, i32) {
    let hx = home.0.floor() as i32;
    let hy = home.1.floor() as i32;
    for _ in 0..8 {
        let x = hx + w.rng.range_i32(-(WANDER_RADIUS as i32), WANDER_RADIUS as i32);
        let y = hy + w.rng.range_i32(-(WANDER_RADIUS as i32), WANDER_RADIUS as i32);
        if w.is_walkable(x, y) {
            return (x, y);
        }
    }
    (hx, hy)
}

/// Déplacement + anim génériques (vitesse ×1/2 si Decrepify — même règle
/// que le froid 12c).
fn move_and_anim(w: &mut World, i: usize, dt: u32) {
    if !w.mon_dat[i].path.is_empty() {
        let base = w.mon_dat[i].speed;
        let speed = if w.monster_slowed(i) { base / 2.0 } else { base };
        crate::world::advance_along_path(&mut w.mon_ent[i], &mut w.mon_dat[i].path, speed, dt);
    }
    let st = if !w.mon_dat[i].path.is_empty() { crate::AnimState::WL } else { crate::AnimState::NU };
    let anim_ms = &mut w.mon_dat[i].anim_ms;
    crate::world::set_anim(&mut w.mon_ent[i], st, anim_ms);
    crate::world::advance_anim(&mut w.mon_ent[i], anim_ms, dt, true);
}

/// Errance forcée (Dim Vision L73) — même mécanique que la branche Idle de
/// `update_monsters`, JAMAIS d'aggro tant que la malédiction dure.
fn blind_wander(w: &mut World, i: usize, dt: u32) {
    if w.mon_dat[i].path.is_empty() {
        w.mon_dat[i].wander_ms = w.mon_dat[i].wander_ms.saturating_sub(dt);
        if w.mon_dat[i].wander_ms == 0 {
            let ht = (w.mon_ent[i].x.floor() as i32, w.mon_ent[i].y.floor() as i32);
            let home = w.mon_dat[i].home;
            let target = wander_target_local(w, home);
            if let Some(p) = w.path_between(ht, target) {
                if !p.is_empty() {
                    w.mon_dat[i].path = p;
                }
            }
            w.mon_dat[i].wander_ms = w.rng.range_u32(1500, 4500);
        }
    }
    move_and_anim(w, i, dt);
}

/// Fuite (Terror L79) — 3 tuiles à l'opposé du héros, re-path périodique
/// (pattern panique Fallen de ai_behaviors).
fn flee(w: &mut World, i: usize, dt: u32) {
    w.mon_dat[i].repath_ms = w.mon_dat[i].repath_ms.saturating_sub(dt);
    if w.mon_dat[i].path.is_empty() || w.mon_dat[i].repath_ms == 0 {
        let (mx, my) = (w.mon_ent[i].x, w.mon_ent[i].y);
        let (dx, dy) = (mx - w.hero_ent.x, my - w.hero_ent.y);
        let d = crate::world::dist(dx, dy).max(0.001);
        let tile = ((mx + dx / d * 3.0).floor() as i32, (my + dy / d * 3.0).floor() as i32);
        if let Some(p) = w.path_between((mx.floor() as i32, my.floor() as i32), tile) {
            w.mon_dat[i].path = p;
        }
        w.mon_dat[i].repath_ms = REPATH_MS;
    }
    move_and_anim(w, i, dt);
}

/// Cible in-fighting : un monstre vivant `j ≠ i` maudit Attract/Confuse dans
/// le rayon de SA malédiction (premier en ordre de table — déterministe).
fn attract_target(w: &World, i: usize) -> Option<usize> {
    for (j, d) in w.mon_dat.iter().enumerate() {
        if j == i || d.state == MonState::Dying {
            continue;
        }
        if let Some(c) = &d.curse {
            if matches!(c.kind(), CurseKind::Attract | CurseKind::Confuse) {
                let e = &w.mon_ent[j];
                let di = &w.mon_ent[i];
                if crate::world::dist(e.x - di.x, e.y - di.y) <= c.radius {
                    return Some(j);
                }
            }
        }
    }
    None
}

/// Poursuite/frappe du monstre `i` CONTRE le monstre `j` (Attract/Confuse) —
/// même FSM que la poursuite du héros ; frappe DIRECTE (pas de jet de
/// toucher monstre↔monstre, simplification MVP documentée).
fn infight(w: &mut World, i: usize, j: usize, dt: u32) {
    let d = crate::world::dist(w.mon_ent[i].x - w.mon_ent[j].x, w.mon_ent[i].y - w.mon_ent[j].y);
    if d <= ATTACK_RANGE {
        w.mon_dat[i].path.clear();
        let anim_ms = &mut w.mon_dat[i].anim_ms;
        crate::world::set_anim(&mut w.mon_ent[i], crate::AnimState::A1, anim_ms);
        crate::world::advance_anim(&mut w.mon_ent[i], anim_ms, dt, false);
        w.mon_dat[i].atk_cd_ms = w.mon_dat[i].atk_cd_ms.saturating_sub(dt);
        if w.mon_dat[i].atk_cd_ms == 0 {
            let dmg = w.rng.range_u32(w.mon_dat[i].dmg_min, w.mon_dat[i].dmg_max).max(1);
            w.mon_dat[j].hp = w.mon_dat[j].hp.saturating_sub(dmg);
            w.mon_dat[i].atk_cd_ms = MONSTER_ATK_CD_MS;
            w.mon_dat[i].anim_ms = 0; // rejoue le swing A1
            if w.mon_dat[j].hp == 0 {
                kill_monster(w, j);
            }
        }
    } else {
        w.mon_dat[i].repath_ms = w.mon_dat[i].repath_ms.saturating_sub(dt);
        if w.mon_dat[i].path.is_empty() || w.mon_dat[i].repath_ms == 0 {
            let mt = (w.mon_ent[i].x.floor() as i32, w.mon_ent[i].y.floor() as i32);
            let jt = (w.mon_ent[j].x.floor() as i32, w.mon_ent[j].y.floor() as i32);
            if let Some(p) = w.path_between(mt, jt) {
                w.mon_dat[i].path = p;
            }
            w.mon_dat[i].repath_ms = REPATH_MS;
        }
        move_and_anim(w, i, dt);
    }
}

/// Contrôle d'un monstre par les malédictions (appelé par `update_monsters`
/// APRÈS le bloc DoT — les DoT continuent de courir). Décrémente TOUJOURS la
/// durée de la malédiction propre au monstre. Renvoie `true` quand la
/// malédiction PREND LA MAIN ce frame (Dim Vision / Terror / in-fighting
/// Attract/Confuse), `false` = flux historique strictement inchangé.
pub(crate) fn tick_monster(w: &mut World, i: usize, dt: u32, _ev: &mut Vec<GameEvent>) -> bool {
    if let Some(c) = w.mon_dat[i].curse.as_mut() {
        if !c.tick(dt) {
            w.mon_dat[i].curse = None; // expiration propre
        }
    }
    if w.mon_dat[i].state == MonState::Dying {
        return false;
    }
    let own = w.mon_dat[i].curse.as_ref().map(|c| c.kind());
    match own {
        Some(CurseKind::DimVision) => {
            blind_wander(w, i, dt);
            return true;
        }
        Some(CurseKind::Terror) => {
            flee(w, i, dt);
            return true;
        }
        _ => {}
    }
    if let Some(j) = attract_target(w, i) {
        infight(w, i, j, dt);
        return true;
    }
    false
}

// ---------------------------------------------------------------------------
// Tests — golden par skill (chaîne cast → effet → expiration), table fait foi
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PlayerInput;
    use crate::skills_tree::SkillTree;
    use crate::statlist::{COLDRESIST, FIRERESIST, LIGHTRESIST};

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");

    /// Numéro de ligne (1-based) du record dans patch_skills.txt — la citation
    /// « L<n> » des goldens est VÉRIFIÉE par ce helper (même principe que
    /// skills_tree::tests::line_of).
    fn line_of(code: &str) -> usize {
        crate::skills_tree::SKILLS_TXT
            .lines()
            .position(|l| crate::stats::split_tsv(l).first().map(|c| c == code).unwrap_or(false))
            .map(|i| i + 1)
            .unwrap_or(0)
    }

    fn world(hero: &str) -> World {
        let manifest: serde_json::Value = serde_json::from_str(MANIFEST).unwrap();
        let grid = vec![vec![false; 64]; 64];
        World::with_seed_and_hero(grid, manifest, 7, hero)
    }

    /// Prérequis d'arbre v1.12 des skills 16a (patch_skills.txt reqskill1-3).
    fn prereqs_of(code: &str) -> &'static [&'static str] {
        match code {
            "Holy Fire" | "Blessed Aim" => &["Might"],
            "Holy Freeze" => &["Holy Fire"],
            "Cleansing" => &["Prayer"],
            "Concentration" => &["Blessed Aim"],
            "Holy Shock" => &["Holy Freeze"],
            "Sanctuary" => &["Thorns", "Holy Freeze"],
            "Meditation" => &["Cleansing"],
            "Fanaticism" => &["Concentration"],
            "Conviction" => &["Sanctuary"],
            "Weaken" | "Iron Maiden" => &["Amplify Damage"],
            "Terror" => &["Weaken"],
            "Confuse" => &["Dim Vision"],
            "Attract" => &["Confuse"],
            "Life Tap" => &["Iron Maiden"],
            "Decrepify" => &["Terror"],
            "Lower Resist" => &["Life Tap", "Decrepify"],
            _ => &[],
        }
    }

    /// Apprend `code` (prérequis d'abord, 1 point chacun) avec `points` niveaux.
    fn learn_chain(w: &mut World, code: &str, points: u32) {
        for pre in prereqs_of(code) {
            learn_chain(w, pre, 1);
        }
        for _ in 0..points {
            w.learn_skill(code).unwrap();
        }
    }

    /// Héros `hero` niveau 30 (reqlevel des auras profondes) avec les skills
    /// `mains` appris (chaînes de prérequis résolues automatiquement).
    fn hero_with(hero: &str, mains: &[(&str, u32)]) -> World {
        let mut w = world(hero);
        w.hero.stats.level = 30;
        w.hero.stats.skill_points = 90;
        for (c, l) in mains {
            learn_chain(&mut w, c, *l);
        }
        w
    }

    fn spawn_at(w: &mut World, code: &str, x: i32, y: i32) -> usize {
        assert!(w.spawn_monster(code, x, y), "spawn {code} ({x},{y})");
        w.mon_dat.len() - 1
    }

    /// Monstre ENCAISSEUR (×200 PV — survit aux pulses/renvois des tests).
    fn spawn_tank(w: &mut World, x: i32, y: i32) -> usize {
        let rule = crate::world::SpawnRule { hp_pct: 20_000, ..Default::default() };
        assert!(w.spawn_monster_rule("ZM", x, y, rule), "spawn tank ({x},{y})");
        w.mon_dat.len() - 1
    }

    /// Premier coup MÊLÉ du héros sur un zombie adjacent (helper 12c).
    fn first_melee_hit(w: &mut World) -> u32 {
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        spawn_tank(w, htx + 1, hty);
        for _ in 0..400 {
            let t = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
            let ev = w.tick(50, t.map(|(x, y)| PlayerInput::Attack { x, y }));
            if let Some(GameEvent::HeroAttacked { dmg, .. }) =
                ev.iter().find(|e| matches!(e, GameEvent::HeroAttacked { .. }))
            {
                return *dmg;
            }
        }
        panic!("aucun coup porté en 20 s");
    }

    /// Première frappe du MONSTRE sur le héros (zombie adjacent, héros passif).
    fn first_monster_hit(w: &mut World) -> u32 {
        for _ in 0..400 {
            let ev = w.tick(50, None);
            if let Some(GameEvent::HeroHit { dmg, .. }) =
                ev.iter().find(|e| matches!(e, GameEvent::HeroHit { .. }))
            {
                return *dmg;
            }
        }
        panic!("aucune frappe du monstre en 20 s");
    }

    // -------------------------------------------------------------------
    // GOLDEN table — auras paladin (15) : lignes, états, rayons, stats
    // -------------------------------------------------------------------

    #[test]
    fn golden_table_paladin_auras_lignes_etats_rayons() {
        let d = SkillTreeData::global();
        for (code, line) in [
            ("Might", 100),
            ("Prayer", 101),
            ("Resist Fire", 102),
            ("Holy Fire", 104),
            ("Thorns", 105),
            ("Defiance", 106),
            ("Resist Cold", 107),
            ("Resist Lightning", 112),
            ("Concentration", 115),
            ("Holy Shock", 120),
            ("Sanctuary", 121),
            ("Meditation", 122),
            ("Fanaticism", 124),
            ("Conviction", 125),
            ("Salvation", 127),
        ] {
            assert_eq!(line_of(code), line, "{code} : ligne citée {line} invalide");
            let rec = d.get(code).expect(code);
            assert_eq!(rec.class, "pal", "{code} : charclass pal");
            assert!(!rec.aura_state.is_empty(), "{code} : aurastate présent");
            assert_eq!(PALADIN_AURAS.iter().filter(|c| **c == code).count(), 1);
            assert!(ActiveAura::new(code, 1).is_some(), "{code} : implémenté 16a");
        }
        assert_eq!(PALADIN_AURAS.len(), 15, "15 auras au brief 16a");
        // Rayons `aurarangecalc` = ln12 réels : Holy Fire 6+1/niv (L104),
        // Might 16+2 (L100), Fanaticism 11+1 (L124), Sanctuary 5+1 (L121),
        // Conviction 20+0 (L125 — P2=0 : rayon fixe).
        let r = |code: &str, lvl: u32| ActiveAura::new(code, lvl).unwrap().radius();
        assert_eq!(r("Holy Fire", 1), 6.0);
        assert_eq!(r("Holy Fire", 5), 10.0);
        assert_eq!(r("Might", 1), 16.0);
        assert_eq!(r("Might", 3), 20.0);
        assert_eq!(r("Fanaticism", 1), 11.0);
        assert_eq!(r("Sanctuary", 1), 5.0);
        assert_eq!(r("Conviction", 20), 20.0);
        // Hors liste : Fire Bolt n'est pas une aura.
        assert!(ActiveAura::new("Fire Bolt", 1).is_none());
    }

    #[test]
    fn golden_stats_auras_valeurs_reelles() {
        let a = |code: &str, lvl: u32| ActiveAura::new(code, lvl).unwrap();
        // L100 Might : damagepercent ln34 = 40 + 10×(n-1).
        assert_eq!(a("Might", 1).stat_pairs(&NoLevels), vec![("damagepercent".into(), 40)]);
        assert_eq!(a("Might", 10).stat_sum("damagepercent", &NoLevels), 130);
        // L105 Thorns : thorns_percent ln34 = 250 + 40×(n-1).
        assert_eq!(a("Thorns", 1).stat_sum("thorns_percent", &NoLevels), 250);
        assert_eq!(a("Thorns", 5).stat_sum("thorns_percent", &NoLevels), 410);
        // L106 Defiance : skill_armor_percent ln34 = 70 + 10×(n-1).
        assert_eq!(a("Defiance", 1).stat_sum("skill_armor_percent", &NoLevels), 70);
        // L102 Resist Fire : fireresist dm34 = 35 + 150×(n-1) ;
        // maxfireresist = skill('Resist Fire'.blvl) — 0 sans niveau héros,
        // 1 avec (HeroLevels — formule inter-skills réelle).
        let rf = a("Resist Fire", 1);
        assert_eq!(rf.stat_sum("fireresist", &NoLevels), 35);
        assert_eq!(rf.stat_sum("maxfireresist", &NoLevels), 0);
        let skills = vec![crate::HeroSkill { code: "Resist Fire".into(), level: 1, missile: None }];
        let lv = HeroLevels::new(&skills, 30);
        assert_eq!(rf.stat_sum("maxfireresist", &lv), 1);
        // L127 Salvation : 3 résists dm34 = 50 + 120×(n-1).
        let sv = a("Salvation", 1);
        for s in ["fireresist", "coldresist", "lightresist"] {
            assert_eq!(sv.stat_sum(s, &NoLevels), 50, "{s}");
        }
        // L115 Concentration : damagepercent ln34 = 60+15×(n-1) + par5=20.
        let co = a("Concentration", 1);
        assert_eq!(co.stat_sum("damagepercent", &NoLevels), 60);
        assert_eq!(co.stat_sum("skill_concentration", &NoLevels), 20);
        // L124 Fanaticism : attackrate dm34 = 10+40×(n-1), damagepercent
        // ln56/2 = (50+17×(n-1))/2 ; item_tohit_percent (token D2 `toht`)
        // est IGNORÉE — token sans évaluateur, stat sautée plutôt que fausse
        // (garde KNOWN_BARE_TOKENS, doc module).
        let fa = a("Fanaticism", 1);
        assert_eq!(fa.stat_sum("attackrate", &NoLevels), 10);
        assert_eq!(fa.stat_sum("damagepercent", &NoLevels), 25, "50/2");
        assert_eq!(
            fa.stat_pairs(&NoLevels).iter().find(|(n, _)| n == "item_tohit_percent"),
            None,
            "toht : stat ignorée, pas de placeholder 1"
        );
        // L125 Conviction : AURA ENNEMIE — -dm56 AC (40+100×(n-1)) +
        // -min(ln34,150) sur feu/froid/foudre (30+5×(n-1), plafond 150 au 25).
        let cv = a("Conviction", 1);
        assert!(cv.is_enemy_aura(), "Conviction = aura ennemie");
        assert_eq!(cv.stat_sum("skill_armor_percent", &NoLevels), -40);
        for s in ["fireresist", "coldresist", "lightresist"] {
            assert_eq!(cv.stat_sum(s, &NoLevels), -30, "{s}");
        }
        assert_eq!(a("Conviction", 25).stat_sum("fireresist", &NoLevels), -150);
        assert!(!a("Might", 1).is_enemy_aura(), "auras positives = côté héros");
        // L122 Meditation : manarecoverybonus ln34 = 300+25×(n-1) ;
        // hitpoints = skill('Prayer'.edns) = 0 (ELen de Prayer VIDE v1.12 —
        // Meditation ne soigne pas, cohérent avec le golden skills_tree).
        let me = a("Meditation", 1);
        assert_eq!(me.stat_sum("manarecoverybonus", &NoLevels), 300);
        assert_eq!(me.stat_sum("hitpoints", &NoLevels), 0);
        // L121 Sanctuary : item_undeaddamage_percent ln34 = 150+30×(n-1),
        // item_undead_tohit ln56 = 100+50×(n-1), skill_bypass_undead = 1.
        let sa = a("Sanctuary", 1);
        assert_eq!(sa.stat_sum("item_undeaddamage_percent", &NoLevels), 150);
        assert_eq!(sa.stat_sum("item_undead_tohit", &NoLevels), 100);
        assert_eq!(sa.stat_sum("skill_bypass_undead", &NoLevels), 1);
        // L101 Prayer : heal = courbe EMin (2 au niv 1, 9 au niv 8, 10 au
        // niv 9 — valeurs PARSÉES par l'engine, split_tsv fait foi).
        assert_eq!(a("Prayer", 1).heal_per_pulse(), Some(2));
        assert_eq!(a("Prayer", 8).heal_per_pulse(), Some(9), "2+7×1");
        assert_eq!(a("Prayer", 9).heal_per_pulse(), Some(10), "palier 9-15 de la table parsée");
        assert_eq!(a("Might", 1).heal_per_pulse(), None);
    }

    #[test]
    fn golden_pulses_degats_courbes_et_synergies() {
        // L104 Holy Fire : EMin 2 / EMax 6, courbes [1,2,3,5,7] ;
        // EDmgSymPerCalc = Resist Fire × par8(18) + Salvation × par7(6).
        let hf = |l| ActiveAura::new("Holy Fire", l).unwrap();
        assert_eq!(hf(1).pulse_damage(), Some((DamageType::Fire, 2, 6)));
        assert_eq!(hf(8).pulse_damage(), Some((DamageType::Fire, 9, 13)), "2+7, 6+7");
        let t0 = SkillTree::for_class("pal");
        assert_eq!(hf(1).synergy_percent(t0.levels()), 0);
        // L120 Holy Shock : EMin 1 (SANS courbe), EMax 10 + [6,8,10,12,15].
        let hs = |l| ActiveAura::new("Holy Shock", l).unwrap();
        assert_eq!(hs(1).pulse_damage(), Some((DamageType::Lightning, 1, 10)));
        assert_eq!(hs(5).pulse_damage(), Some((DamageType::Lightning, 1, 34)), "10+6×4");
        // L121 Sanctuary : EMin 8 + [4,4,5,5,6], EMax 16 + [4,5,6,6,7] (magie).
        let sa = |l| ActiveAura::new("Sanctuary", l).unwrap();
        assert_eq!(sa(1).pulse_damage(), Some((DamageType::Magic, 8, 16)));
        assert_eq!(sa(5).pulse_damage(), Some((DamageType::Magic, 24, 32)));
        // Auras sans dégâts : Might / Prayer → None (Prayer soigne).
        assert_eq!(ActiveAura::new("Might", 1).unwrap().pulse_damage(), None);
        assert_eq!(ActiveAura::new("Prayer", 1).unwrap().pulse_damage(), None);
        // Synergie réelle : Resist Fire 5 + Salvation 2 → 5×18 + 2×6 = 102 %.
        let mut t = SkillTree::for_class("pal");
        t.set_level("Resist Fire", 5);
        t.set_level("Salvation", 2);
        assert_eq!(hf(1).synergy_percent(t.levels()), 102);
    }

    /// Mission ③ (statlist) : les stats d'aura/malédiction résolvent vers des
    /// ids `ItemStatCost` v1.12 RÉELS (golden dictionnaire) — même interface
    /// que ShrineBuff::stat_lines.
    #[test]
    fn stat_lines_resolvent_ids_itemstatcost_reels() {
        let expected: &[(&str, i32)] = &[
            ("damagepercent", 25),
            ("hitpoints", 6),
            ("manarecoverybonus", 27),
            ("damageresist", 36),
            ("fireresist", FIRERESIST),
            ("maxfireresist", 40),
            ("lightresist", LIGHTRESIST),
            ("maxlightresist", 42),
            ("coldresist", COLDRESIST),
            ("maxcoldresist", 44),
            ("velocitypercent", 67),
            ("attackrate", 68),
            ("skill_bypass_undead", 103),
            ("item_tohit_percent", 119),
            ("item_undeaddamage_percent", 122),
            ("item_undead_tohit", 124),
            ("thorns_percent", 131),
            ("skill_concentration", 164),
            ("skill_armor_percent", 171),
        ];
        for (name, id) in expected {
            let rec = crate::stats::StatCostTable::global()
                .get(name)
                .unwrap_or_else(|| panic!("{name} absent du dictionnaire"));
            assert_eq!(rec.id, *id, "{name} : id table {} ≠ {id}", rec.id);
        }
        // Les auras AVEC aurastat produisent des stat_lines NON VIDES (aucun
        // nom orphelin) ; Holy Fire (L104) / Holy Shock (L120) n'ont PAS de
        // colonnes aurastat — leurs dégâts passent par les courbes EMin/EMax
        // ([`ActiveAura::pulse_damage`]), stat_lines vides = table fait foi.
        for code in PALADIN_AURAS {
            let a = ActiveAura::new(code, 1).unwrap();
            if code == &"Holy Fire" || code == &"Holy Shock" {
                continue;
            }
            assert!(!a.stat_lines(&NoLevels).is_empty(), "{code} : stat_lines vides");
        }
        assert!(ActiveAura::new("Holy Fire", 1).unwrap().stat_lines(&NoLevels).is_empty());
        assert!(ActiveAura::new("Holy Shock", 1).unwrap().stat_lines(&NoLevels).is_empty());
        // Malédictions AVEC aurastat : non vides ; celles SANS (effets FSM,
        // colonnes vides en v1.12) : vides — la table fait foi.
        for code in ["Amplify Damage", "Weaken", "Decrepify", "Lower Resist"] {
            assert!(
                !ActiveCurse::new(code, 1).unwrap().stat_lines().is_empty(),
                "{code} : stat_lines vides"
            );
        }
        for code in ["Dim Vision", "Terror", "Confuse", "Attract", "Iron Maiden"] {
            assert!(
                ActiveCurse::new(code, 1).unwrap().stat_lines().is_empty(),
                "{code} : aucune aurastat en v1.12"
            );
        }
    }

    // -------------------------------------------------------------------
    // GOLDEN table — malédictions nécromancien (9)
    // -------------------------------------------------------------------

    #[test]
    fn golden_table_maledictions_lignes_durees_rayons_couts() {
        let d = SkillTreeData::global();
        // Citations de ligne épinglées + `auratargetstate` RÉEL de la table
        // (colonne lue par ce module — states.txt `curse=1` correspondants).
        for (code, line, target) in [
            ("Amplify Damage", 68, "amplifydamage"),
            ("Dim Vision", 73, "dimvision"),
            ("Weaken", 74, "weaken"),
            ("Iron Maiden", 78, "ironmaiden"),
            ("Terror", 79, "terror"),
            ("Confuse", 83, "confuse"),
            ("Attract", 88, "attract"),
            ("Decrepify", 89, "decrepify"),
            ("Lower Resist", 93, "lowerresist"),
        ] {
            assert_eq!(line_of(code), line, "{code} : ligne citée {line} invalide");
            let rec = d.get(code).expect(code);
            assert_eq!(rec.class, "nec");
            assert_eq!(
                aura_cols(code).expect("colonnes d'aura").target_state,
                target,
                "{code} : auratargetstate réel"
            );
            assert!(ActiveCurse::new(code, 1).is_some(), "{code} : implémenté 16a");
        }
        assert_eq!(NECRO_CURSES.len(), 9, "9 malédictions au brief 16a");
        // Durées `auralencalc` = ln34 (frames) : Amp 200+75/niv, Dim Vision
        // 175+50, Weaken 350+60, IM 300+60, Terror 200+25, Confuse 250+50,
        // Attract 300+90, Decrepify 100+15, Lower Resist 500+50 (×40 ms).
        let ms = |code: &str, lvl: u32| ActiveCurse::new(code, lvl).unwrap().remaining_ms;
        assert_eq!(ms("Amplify Damage", 1), 200 * 40);
        assert_eq!(ms("Amplify Damage", 5), (200 + 75 * 4) * 40);
        assert_eq!(ms("Dim Vision", 1), 175 * 40);
        assert_eq!(ms("Weaken", 1), 350 * 40);
        assert_eq!(ms("Iron Maiden", 1), 300 * 40);
        assert_eq!(ms("Terror", 1), 200 * 40);
        assert_eq!(ms("Confuse", 1), 250 * 40);
        assert_eq!(ms("Attract", 1), 300 * 40);
        assert_eq!(ms("Decrepify", 1), 100 * 40);
        assert_eq!(ms("Lower Resist", 3), (500 + 50 * 2) * 40);
        // Rayons `aurarangecalc` = ln12 (yards) : Amp 3+1/niv, Confuse 6+1,
        // Attract 9+0, Decrepify 6+0, IM 7+0, Lower Resist 7+1.
        let r = |code: &str, lvl: u32| ActiveCurse::new(code, lvl).unwrap().radius;
        assert_eq!(r("Amplify Damage", 1), 3.0);
        assert_eq!(r("Amplify Damage", 4), 6.0);
        assert_eq!(r("Confuse", 1), 6.0);
        assert_eq!(r("Attract", 20), 9.0, "P2=0 : fixe");
        assert_eq!(r("Decrepify", 1), 6.0);
        assert_eq!(r("Iron Maiden", 1), 7.0);
        assert_eq!(r("Lower Resist", 1), 7.0);
        // Coûts mana réels (cast) : 4/9/4/5/7/13/17/11/22.
        let cost = |code: &str| d.mana_cost_at(d.get(code).unwrap(), 1);
        assert_eq!(cost("Amplify Damage"), 4);
        assert_eq!(cost("Dim Vision"), 9);
        assert_eq!(cost("Weaken"), 4);
        assert_eq!(cost("Iron Maiden"), 5);
        assert_eq!(cost("Terror"), 7);
        assert_eq!(cost("Confuse"), 13);
        assert_eq!(cost("Attract"), 17);
        assert_eq!(cost("Decrepify"), 11);
        assert_eq!(cost("Lower Resist"), 22);
    }

    #[test]
    fn golden_stats_maledictions_valeurs_reelles() {
        let c = |code: &str, lvl: u32| ActiveCurse::new(code, lvl).unwrap();
        // L68 Amplify Damage : damageresist -par5 = -100 (physique ×2).
        assert_eq!(c("Amplify Damage", 1).stat_sum("damageresist"), -100);
        // L74 Weaken : damagepercent -par5 = -33.
        assert_eq!(c("Weaken", 1).stat_sum("damagepercent"), -33);
        // L89 Decrepify : velocitypercent/damagepercent/damageresist = -50.
        let dec = c("Decrepify", 1);
        assert_eq!(dec.stat_sum("velocitypercent"), -50);
        assert_eq!(dec.stat_sum("damagepercent"), -50);
        assert_eq!(dec.stat_sum("damageresist"), -50);
        // L93 Lower Resist : LES QUATRE résists -dm56 = -25 -70×(n-1) —
        // `aurastat1-4` = feu/foudre/froid/POISON (la table fait foi : le
        // poison EST réduit en v1.12, colonne aurastat4=poisonresist).
        let lr = |l| c("Lower Resist", l);
        for s in ["fireresist", "lightresist", "coldresist", "poisonresist"] {
            assert_eq!(lr(1).stat_sum(s), -25, "{s}");
            assert_eq!(lr(3).stat_sum(s), -165, "{s}");
        }
        // L78 Iron Maiden : calc1 ln56 = 200 + 25×(n-1) % de renvoi.
        assert_eq!(c("Iron Maiden", 1).iron_maiden_pct(), 200);
        assert_eq!(c("Iron Maiden", 5).iron_maiden_pct(), 300);
        assert_eq!(c("Amplify Damage", 1).iron_maiden_pct(), 0, "renvoi = IM seul");
        // Décroissance : tick consomme la durée, expiration propre.
        let mut cur = c("Amplify Damage", 1);
        assert!(cur.tick(7_999));
        assert!(!cur.tick(1), "8000 ms consommées");
        assert!(!cur.tick(1000), "rester expiré");
    }

    // -------------------------------------------------------------------
    // Toggle : une seule active, swap, mana, erreurs
    // -------------------------------------------------------------------

    #[test]
    fn toggle_erreurs_inconnu_non_appris_pas_une_aura_pas_de_mana() {
        let mut w = world("PA");
        assert_eq!(
            w.toggle_aura("N'importe Quoi"),
            Err(AuraError::UnknownSkill("N'importe Quoi".into()))
        );
        // Le PA ne connaît pas Fire Bolt ⇒ NotKnown (la vérification « connu »
        // précède « est une aura ») ; la SOR le connaît ⇒ NotAnAura.
        assert_eq!(w.toggle_aura("Fire Bolt"), Err(AuraError::NotKnown("Fire Bolt".into())));
        let mut ws = world("SOR");
        assert_eq!(ws.toggle_aura("Fire Bolt"), Err(AuraError::NotAnAura("Fire Bolt".into())));
        // Aura pal non apprise.
        assert_eq!(w.toggle_aura("Might"), Err(AuraError::NotKnown("Might".into())));
        // Apprise mais mana insuffisant (Prayer = 16, PA démarre à 15).
        w.hero.stats.skill_points = 5;
        w.learn_skill("Prayer").unwrap();
        assert_eq!(w.toggle_aura("Prayer"), Err(AuraError::NotEnoughMana { cost: 16, have: 15 }));
        assert!(w.active_aura().is_none(), "état inchangé sur refus");
    }

    #[test]
    fn toggle_swap_une_seule_active_et_cout_mana_table() {
        let mut w = hero_with("PA", &[("Holy Fire", 1), ("Might", 1), ("Prayer", 1)]);
        let mana0 = w.hero_stats().mana;
        // Holy Fire : coût 0 (col mana vide) → actif.
        assert_eq!(w.toggle_aura("Holy Fire"), Ok(true));
        assert_eq!(w.active_aura().unwrap().skill, "Holy Fire");
        assert_eq!(w.hero_stats().mana, mana0);
        // Swap : Might remplace Holy Fire (UNE seule active — règle D2).
        assert_eq!(w.toggle_aura("Might"), Ok(true));
        assert_eq!(w.active_aura().unwrap().skill, "Might");
        // Toggle off.
        assert_eq!(w.toggle_aura("Might"), Ok(false));
        assert!(w.active_aura().is_none());
        // Prayer charge 16 mana (coût réel de la table L101).
        w.hero.stats.mana = 50;
        assert_eq!(w.toggle_aura("Prayer"), Ok(true));
        assert_eq!(w.hero_stats().mana, 50 - 16);
    }

    // -------------------------------------------------------------------
    // Pulses : Holy Fire / Holy Shock / Sanctuary / Prayer / Meditation
    // -------------------------------------------------------------------

    #[test]
    fn holy_fire_pulse_degats_rayon_et_kill() {
        let mut w = hero_with("PA", &[("Holy Fire", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let near = spawn_tank(&mut w, htx + 3, hty); // dans 6 yards (L104)
        let far = spawn_tank(&mut w, htx + 20, hty); // hors rayon
        assert_eq!(w.toggle_aura("Holy Fire"), Ok(true));
        let (hp_near0, _) = w.monster_hp(near).unwrap();
        let (hp_far0, _) = w.monster_hp(far).unwrap();
        for _ in 0..30 {
            w.tick(100, None); // 3 pulses (1 s chacun)
        }
        let (hp_near1, _) = w.monster_hp(near).unwrap();
        let (hp_far1, _) = w.monster_hp(far).unwrap();
        let total = hp_near0 - hp_near1;
        assert!((6..=18).contains(&total), "3 pulses × 2..6 = 6..18, got {total}");
        assert_eq!(hp_far0, hp_far1, "hors rayon : aucun dégât");
        // KILL : un Fallen (1-4 PV) adjacent meurt aux pulses → MonsterDied.
        let mut wk = hero_with("PA", &[("Holy Fire", 1)]);
        let (sx, sy) = wk.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        wk.clear_monsters();
        spawn_at(&mut wk, "FA", htx + 2, hty);
        wk.toggle_aura("Holy Fire").unwrap();
        let mut died = false;
        for _ in 0..40 {
            if wk.tick(100, None).iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })) {
                died = true;
                break;
            }
        }
        assert!(died, "Holy Fire a abattu le Fallen en ≤ 4 s");
    }

    #[test]
    fn holy_fire_immunise_et_synergie_resist_fire_salvation() {
        // Immunisé feu (res 100) : type sauté, aucun tirage (déterminisme).
        let mut w = hero_with("PA", &[("Holy Fire", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let rule = crate::world::SpawnRule {
            hp_pct: 20_000,
            res: [0, 0, 100, 0, 0, 0],
            ..Default::default()
        };
        assert!(w.spawn_monster_rule("ZM", htx + 2, hty, rule));
        let idx = w.mon_dat.len() - 1;
        assert_eq!(w.toggle_aura("Holy Fire"), Ok(true));
        let (hp0, _) = w.monster_hp(idx).unwrap();
        for _ in 0..20 {
            w.tick(100, None);
        }
        assert_eq!(w.monster_hp(idx).unwrap().0, hp0, "immunité feu : 0 dégât");
        // Synergie réelle : Resist Fire 5 + Salvation 2 → +102 % (2..6 →
        // 4..12 par pulse ; 10 pulses → 40..120).
        let mut w2 = hero_with("PA", &[("Holy Fire", 1), ("Resist Fire", 5), ("Salvation", 2)]);
        let (sx, sy) = w2.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w2.clear_monsters();
        let idx = spawn_tank(&mut w2, htx + 2, hty);
        w2.toggle_aura("Holy Fire").unwrap();
        let (hp0, _) = w2.monster_hp(idx).unwrap();
        for _ in 0..100 {
            w2.tick(100, None); // 10 pulses (1 par seconde)
        }
        let total = hp0 - w2.monster_hp(idx).unwrap().0;
        assert!((40..=120).contains(&total), "10 pulses × 4..12 (×2.02), got {total}");
    }

    #[test]
    fn holy_shock_foudre_et_sanctuary_magie_pulsent() {
        for (code, elem_slot) in [("Holy Shock", 3usize), ("Sanctuary", 1)] {
            let mut w = hero_with("PA", &[(code, 1)]);
            let (sx, sy) = w.hero_spawn();
            let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
            w.clear_monsters();
            let mut res = [0; 6];
            res[elem_slot] = 100; // immunisé à l'élément de l'aura
            let rule = crate::world::SpawnRule {
                hp_pct: 20_000,
                res,
                ..Default::default()
            };
            assert!(w.spawn_monster_rule("ZM", htx + 2, hty, rule));
            let idx = w.mon_dat.len() - 1;
            w.toggle_aura(code).unwrap();
            let (hp0, _) = w.monster_hp(idx).unwrap();
            for _ in 0..10 {
                w.tick(100, None);
            }
            assert_eq!(w.monster_hp(idx).unwrap().0, hp0, "{code} : immunisé ⇒ 0");
            // Non-immunisé : le pulse fait mal (EMin 1 / 8 par pulse min).
            let idx2 = spawn_tank(&mut w, htx + 4, hty);
            let (hp20, _) = w.monster_hp(idx2).unwrap();
            for _ in 0..10 {
                w.tick(100, None);
            }
            assert!(w.monster_hp(idx2).unwrap().0 < hp20, "{code} : pulse visible");
        }
    }

    #[test]
    fn prayer_soigne_par_pulse_et_plafonne() {
        let mut w = world("PA");
        w.hero.stats.level = 30;
        w.hero.stats.skill_points = 5;
        w.learn_skill("Prayer").unwrap();
        w.hero.stats.mana = 50;
        w.hero.stats.life = 10;
        w.clear_monsters(); // aucun monstre : le soin est mesuré à l'isolé
        assert_eq!(w.toggle_aura("Prayer"), Ok(true));
        assert_eq!(w.hero_stats().mana, 50 - 16, "cast = 16 mana (table L101)");
        for _ in 0..10 {
            w.tick(100, None);
        }
        assert_eq!(w.hero_stats().life, 12, "1 pulse en 1 s → +2 (EMin L101)");
        for _ in 0..90 {
            w.tick(100, None);
        }
        assert_eq!(w.hero_stats().life, 30, "10 pulses × +2");
        // Plafond : jamais au-dessus de max_life (pas de sur-soin).
        w.hero.stats.life = w.hero_stats().max_life - 1;
        for _ in 0..10 {
            w.tick(100, None);
        }
        assert_eq!(w.hero_stats().life, w.hero_stats().max_life);
        // Niveau 8 : +9 par pulse (2 + 7×1 — courbe [1,1,2,2,3] L101).
        let mut w8 = world("PA");
        w8.hero.stats.level = 30;
        w8.hero.stats.skill_points = 30;
        learn_chain(&mut w8, "Prayer", 8);
        w8.hero.stats.mana = 200;
        w8.hero.stats.life = 10;
        w8.clear_monsters();
        w8.toggle_aura("Prayer").unwrap();
        for _ in 0..10 {
            w8.tick(100, None);
        }
        assert_eq!(w8.hero_stats().life, 19, "1 pulse × 9 au niveau 8");
    }

    #[test]
    fn meditation_regen_mana_par_pulse() {
        let mut w = hero_with("PA", &[("Meditation", 1)]);
        w.hero.stats.mana = 0;
        assert_eq!(w.toggle_aura("Meditation"), Ok(true), "coût 0 (table L122)");
        for _ in 0..10 {
            w.tick(100, None);
        }
        assert_eq!(w.hero_stats().mana, 1, "1 pulse → +1 (régén MVP, doc module)");
        for _ in 0..90 {
            w.tick(100, None);
        }
        assert!(w.hero_stats().mana >= 10, "régén continue");
        assert!(w.hero_stats().mana <= w.hero_stats().max_mana, "plafonné max mana");
    }

    // -------------------------------------------------------------------
    // Auras de combat : dégâts héros, Conviction, Defiance
    // -------------------------------------------------------------------

    #[test]
    fn might_booste_les_degats_hero_mul_table() {
        // Même graine : coup de base D → ⌊D × (100+40)/100⌋ avec Might L100.
        let base = first_melee_hit(&mut hero_with("PA", &[("Might", 1)]));
        let mut w = hero_with("PA", &[("Might", 1)]);
        assert_eq!(w.toggle_aura("Might"), Ok(true));
        let boosted = first_melee_hit(&mut w);
        assert_eq!(boosted, base * 140 / 100, "Might L1 = +40 % (ln34, L100)");
    }

    #[test]
    fn concentration_et_fanaticism_boostent_aucune_cumulable() {
        // Même graine : base sans aura → ×1.6 (Concentration L115, niveau 1)
        // / ×1.25 (Fanaticism L124, ln56/2 = 50/2, niveau 1) — trois mondes ;
        // le swap prouve l'unicité de l'aura active. (La chaîne de
        // Fanaticism apprend Concentration comme prérequis — les trois mondes
        // partagent donc le même arbre, niveaux identiques.)
        let base = first_melee_hit(&mut hero_with("PA", &[("Fanaticism", 1)]));
        let mut wc = hero_with("PA", &[("Fanaticism", 1)]);
        wc.toggle_aura("Concentration").unwrap();
        assert_eq!(wc.hero_damage_pct(), 60, "Concentration L1 = ln34");
        assert_eq!(first_melee_hit(&mut wc), base * 160 / 100, "Concentration +60 %");
        let mut wf = hero_with("PA", &[("Fanaticism", 1)]);
        wf.toggle_aura("Fanaticism").unwrap();
        assert_eq!(wf.active_aura().unwrap().skill, "Fanaticism");
        assert_eq!(wf.hero_damage_pct(), 25, "Fanaticism L1 = ln56/2");
        assert_eq!(first_melee_hit(&mut wf), base * 125 / 100, "Fanaticism +25 %");
    }

    #[test]
    fn conviction_reduit_ac_et_res_des_monstres_dans_le_rayon() {
        let mut w = hero_with("PA", &[("Conviction", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let mk_rule = || crate::world::SpawnRule {
            hp_pct: 20_000,
            ac: 200,
            res: [0, 0, 50, 0, 0, 0],
            ..Default::default()
        };
        assert!(w.spawn_monster_rule("ZM", htx + 3, hty, mk_rule())); // dans 20 yards
        let near = w.mon_dat.len() - 1;
        assert!(w.spawn_monster_rule("ZM", htx + 30, hty, mk_rule())); // hors rayon
        let far = w.mon_dat.len() - 1;
        // Sans aura : inchangé.
        assert_eq!(w.monster_effective_ac(near), 200);
        assert_eq!(w.monster_effective_res(near)[2], 50);
        // L125 : AC -dm56 = -40, res -min(ln34,150) = -30 (niveau 1).
        w.toggle_aura("Conviction").unwrap();
        assert_eq!(w.monster_effective_ac(near), 160);
        assert_eq!(w.monster_effective_res(near)[2], 20);
        assert_eq!(w.monster_effective_ac(far), 200, "hors rayon : inchangé");
        assert_eq!(w.monster_effective_res(far)[2], 50);
        // Niveau 20 (plafond maxlvl) : res -min(ln34,150) = -125, AC planché
        // à 0 (dm56 = 40+100×19 = 1940).
        let mut w20 = hero_with("PA", &[("Conviction", 20)]);
        let (sx, sy) = w20.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w20.clear_monsters();
        assert!(w20.spawn_monster_rule("ZM", htx + 2, hty, mk_rule()));
        w20.toggle_aura("Conviction").unwrap();
        assert_eq!(w20.monster_effective_ac(0), 0, "plancher 0");
        assert_eq!(w20.monster_effective_res(0)[2], -75, "50 - 125");
    }

    #[test]
    fn defiance_booste_la_defense_hero() {
        let mut w = hero_with("PA", &[("Defiance", 1)]);
        assert_eq!(w.hero_defense_pct(), 0);
        w.toggle_aura("Defiance").unwrap();
        assert_eq!(w.hero_defense_pct(), 70, "L106 ln34 = 70 + 10×(n-1)");
        assert_eq!(w.scale_hero_defense(100), 170);
        // Conviction n'est PAS une aura défensive du héros (stats ennemies).
        let mut wc = hero_with("PA", &[("Conviction", 1)]);
        wc.toggle_aura("Conviction").unwrap();
        assert_eq!(wc.hero_defense_pct(), 0);
    }

    // -------------------------------------------------------------------
    // Malédictions : cast, effets combat, comportements, expiration
    // -------------------------------------------------------------------

    #[test]
    fn cast_curse_zone_duree_expiration_et_mana() {
        let mut w = hero_with("NE", &[("Amplify Damage", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let near = spawn_tank(&mut w, htx + 2, hty); // dans 3 yards (L68)
        let far = spawn_tank(&mut w, htx + 15, hty); // hors rayon
        let mana0 = w.hero_stats().mana;
        let hit = w.cast_curse(htx as f32 + 2.5, hty as f32 + 0.5, "Amplify Damage").unwrap();
        assert_eq!(hit, vec![near], "seul le monstre du rayon est maudit");
        assert_eq!(w.hero_stats().mana, mana0 - 4, "cast = 4 mana (table L68)");
        // Effet : ResDm -100 sur la cible, rien hors zone.
        assert_eq!(w.monster_effective_res(near)[0], -100);
        assert_eq!(w.monster_effective_res(far)[0], 0);
        // Durée 200 frames = 8000 ms → expiration propre après 8,1 s.
        assert_eq!(w.monster_curse(near).unwrap().remaining_ms, 8_000);
        for _ in 0..81 {
            w.tick(100, None);
        }
        assert!(w.monster_curse(near).is_none(), "expiration");
        assert_eq!(w.monster_effective_res(near)[0], 0, "res restaurée");
    }

    #[test]
    fn amplify_double_les_degats_physiques_hero() {
        // Même monde, même RNG : coup de base D (res 0) → 2×D sous Amplify
        // Damage (damageresist -100 → apply_resist ×2 dans apply_combat_mods).
        // Le curse est posé ENTRE les deux coups sur le MÊME encaisseur —
        // pas de clear (il perdrait la malédiction), pas de monde neuf (le
        // flux RNG divergerait et le ×2 exact serait du hasard, cf. union
        // 15+16).
        // Deux mondes au MÊME seed et au MÊME enchaînement d opérations
        // jusqu au coup mesuré (spawn → tick attack) : le seul écart est la
        // malédiction posée sur l encaisseur du monde 2 APRÈS son spawn —
        // le roll de dégâts est donc identique, ×2 par ResDm -100.
        let d0 = first_melee_hit(&mut hero_with("NE", &[("Amplify Damage", 1)]));
        let mut w = hero_with("NE", &[("Amplify Damage", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        spawn_tank(&mut w, htx + 1, hty);
        w.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Amplify Damage").unwrap();
        let mut d1 = 0;
        for _ in 0..400 {
            let t = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
            let ev = w.tick(50, t.map(|(x, y)| PlayerInput::Attack { x, y }));
            if let Some(GameEvent::HeroAttacked { dmg, .. }) =
                ev.iter().find(|e| matches!(e, GameEvent::HeroAttacked { .. }))
            {
                d1 = *dmg;
                break;
            }
        }
        assert_ne!(d1, 0, "un coup a dû porter");
        assert_eq!(d1, d0 * 2, "ResDm -100 % ⇒ dégâts physiques ×2");
    }

    #[test]
    fn curse_replace_curse_et_erreurs() {
        let mut w = hero_with("NE", &[("Amplify Damage", 1), ("Dim Vision", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let m = spawn_tank(&mut w, htx + 1, hty);
        w.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Amplify Damage").unwrap();
        assert_eq!(w.monster_curse(m).unwrap().kind(), CurseKind::AmplifyDamage);
        // Recast : REMPLACE (une seule malédiction par monstre) + coût DV 9.
        let mana0 = w.hero_stats().mana;
        w.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Dim Vision").unwrap();
        assert_eq!(w.monster_curse(m).unwrap().kind(), CurseKind::DimVision);
        assert_eq!(w.hero_stats().mana, mana0 - 9);
        // Erreurs : pas apprise / pas une malédiction (même connu) / mana vide.
        assert_eq!(
            w.cast_curse(0.0, 0.0, "Decrepify"),
            Err(AuraError::NotKnown("Decrepify".into()))
        );
        assert_eq!(
            w.cast_curse(0.0, 0.0, "Raise Skeleton"),
            Err(AuraError::NotAnAura("Raise Skeleton".into()))
        );
        w.hero.stats.mana = 0;
        assert_eq!(
            w.cast_curse(0.0, 0.0, "Dim Vision"),
            Err(AuraError::NotEnoughMana { cost: 9, have: 0 })
        );
    }

    #[test]
    fn weaken_et_decrepify_affaiblissent_le_monstre() {
        // Baseline (même graine, même tank adjacent) : le zombie frappe D.
        let d0 = {
            let mut w = hero_with("NE", &[("Weaken", 1), ("Decrepify", 1)]);
            let (sx, sy) = w.hero_spawn();
            let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
            w.clear_monsters();
            spawn_tank(&mut w, htx + 1, hty);
            first_monster_hit(&mut w)
        };
        // Weaken L74 : dégâts ×(100-33)/100.
        let mut ww = hero_with("NE", &[("Weaken", 1)]);
        let (sx, sy) = ww.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        ww.clear_monsters();
        spawn_tank(&mut ww, htx + 1, hty);
        ww.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Weaken").unwrap();
        assert_eq!(first_monster_hit(&mut ww), d0 * 67 / 100, "Weaken -33 %");
        // Decrepify L89 : dégâts ×1/2 + ResDm -50 + vitesse ×1/2.
        let mut wd = hero_with("NE", &[("Decrepify", 1)]);
        let (sx, sy) = wd.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        wd.clear_monsters();
        spawn_tank(&mut wd, htx + 1, hty);
        wd.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Decrepify").unwrap();
        assert_eq!(first_monster_hit(&mut wd), d0 / 2, "Decrepify -50 %");
        assert!(wd.monster_slowed(0), "velocitypercent -50 ⇒ vitesse ×1/2");
        assert_eq!(wd.monster_effective_res(0)[0], -50, "ResDm -50");
    }

    #[test]
    fn lower_resist_amplifie_les_degats_elementaires() {
        // L93 : les QUATRE résists -25 -70×(n-1) (aurastat1-4 = feu/foudre/
        // froid/poison) — vérifiés sur les résists effectifs consommés par
        // apply_resist (missiles/pulses).
        let mut w = hero_with("NE", &[("Lower Resist", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let rule = crate::world::SpawnRule {
            hp_pct: 20_000,
            res: [0, 0, 50, 50, 50, 75],
            ..Default::default()
        };
        assert!(w.spawn_monster_rule("ZM", htx + 2, hty, rule));
        w.cast_curse(htx as f32 + 2.5, hty as f32 + 0.5, "Lower Resist").unwrap();
        let res = w.monster_effective_res(0);
        assert_eq!(res[2], 25, "feu 50 - 25");
        assert_eq!(res[3], 25, "foudre 50 - 25");
        assert_eq!(res[4], 25, "froid 50 - 25");
        assert_eq!(res[5], 50, "poison 75 - 25 (aurastat4, v1.12)");
        // Un coup feu 100 : 50 avant / 75 après (apply_resist réel).
        let avant = difficulty::apply_resist(100, 50);
        let apres = difficulty::apply_resist(100, res[2]);
        assert_eq!((avant, apres), (50, 75), "Lower Resist amplifie le feu");
    }

    #[test]
    fn dim_vision_bloque_l_aggro() {
        // Baseline (même graine) : le zombie adjacent attaque le héros.
        let base_hit = first_monster_hit(&mut hero_with("NE", &[("Dim Vision", 1)]));
        assert!(base_hit > 0);
        // Aveuglé : AUCUNE frappe en 5 s (durée L73 = 7 000 ms > 5 000).
        let mut w = hero_with("NE", &[("Dim Vision", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        spawn_tank(&mut w, htx + 1, hty);
        w.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Dim Vision").unwrap();
        for _ in 0..100 {
            let ev = w.tick(50, None);
            assert!(
                !ev.iter().any(|e| matches!(e, GameEvent::HeroHit { .. })),
                "Dim Vision : le monstre n'aggro plus"
            );
        }
    }

    #[test]
    fn terror_met_le_monstre_en_fuite() {
        let mut w = hero_with("NE", &[("Terror", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        spawn_tank(&mut w, htx + 1, hty);
        w.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Terror").unwrap();
        let d0 = crate::world::dist(
            w.monsters()[0].x - w.hero().x,
            w.monsters()[0].y - w.hero().y,
        );
        let mut hit = false;
        for _ in 0..60 {
            let ev = w.tick(50, None);
            hit |= ev.iter().any(|e| matches!(e, GameEvent::HeroHit { .. }));
        }
        let d1 = crate::world::dist(
            w.monsters()[0].x - w.hero().x,
            w.monsters()[0].y - w.hero().y,
        );
        assert!(d1 > d0, "le monstre fuit ({} → {} tuiles)", d0, d1);
        assert!(!hit, "Terror : aucune frappe en 3 s (durée L79 = 8 000 ms)");
    }

    #[test]
    fn attract_declenche_l_infighting() {
        let mut w = hero_with("NE", &[("Attract", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let a = spawn_tank(&mut w, htx + 2, hty); // maudit (rayon L88 = 9)
        let _b = spawn_tank(&mut w, htx + 4, hty); // dans le rayon de A
        w.cast_curse(htx as f32 + 2.5, hty as f32 + 0.5, "Attract").unwrap();
        assert_eq!(w.monster_curse(a).unwrap().kind(), CurseKind::Attract);
        let (hp_a0, _) = w.monster_hp(a).unwrap();
        for _ in 0..200 {
            w.tick(50, None); // 10 s : A et B (tous deux dans le rayon du cast)
            // se traquent mutuellement (in-fighting Attract/Confuse).
        }
        let (hp_a1, _) = w.monster_hp(a).unwrap();
        assert!(hp_a0 > hp_a1, "B a attaqué A (in-fighting Attract) : {} → {}", hp_a0, hp_a1);
    }

    #[test]
    fn iron_maiden_et_thorns_renvoient_les_degats() {
        // Iron Maiden L78 : le monstre encaisse 200 % de ce qu'il inflige.
        let mut w = hero_with("NE", &[("Iron Maiden", 1)]);
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        let m = spawn_tank(&mut w, htx + 1, hty);
        w.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Iron Maiden").unwrap();
        let (hp0, _) = w.monster_hp(m).unwrap();
        let d = first_monster_hit(&mut w);
        let (hp1, _) = w.monster_hp(m).unwrap();
        assert_eq!(hp0 - hp1, d * 200 / 100, "renvoi IM = 200 % des dégâts infligés");
        // Thorns L105 : l'aura renvoie 250 % (héros PA).
        let mut wt = hero_with("PA", &[("Thorns", 1)]);
        wt.toggle_aura("Thorns").unwrap();
        let (sxt, syt) = wt.hero_spawn();
        let (htx, hty) = (sxt.floor() as i32, syt.floor() as i32);
        wt.clear_monsters();
        let m = spawn_tank(&mut wt, htx + 1, hty);
        let (hp0, _) = wt.monster_hp(m).unwrap();
        let d = first_monster_hit(&mut wt);
        let (hp1, _) = wt.monster_hp(m).unwrap();
        assert_eq!(hp0 - hp1, d * 250 / 100, "renvoi Thorns = 250 % (ln34 L105)");
    }

    #[test]
    fn aura_coupee_a_la_mort_et_transitoire_en_save() {
        // Mort du héros : l'aura se coupe (états perdus au respawn).
        let mut w = hero_with("PA", &[("Might", 1)]);
        w.toggle_aura("Might").unwrap();
        assert!(w.active_aura().is_some());
        let (sx, sy) = w.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        w.clear_monsters();
        spawn_tank(&mut w, htx + 1, hty);
        w.hero.stats.life = 1;
        let mut died = false;
        for _ in 0..100 {
            if w.tick(50, None).iter().any(|e| matches!(e, GameEvent::HeroDied)) {
                died = true;
                break;
            }
        }
        assert!(died, "le zombie a abattu le héros à 1 PV");
        for _ in 0..50 {
            w.tick(50, None); // respawn (2 000 ms)
        }
        assert!(w.active_aura().is_none(), "aura coupée au respawn");
        // Save/load : l'aura et les malédictions sont TRANSITOIRES.
        let mut wn = hero_with("NE", &[("Amplify Damage", 1)]);
        let (sx, sy) = wn.hero_spawn();
        let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
        wn.clear_monsters();
        let m = spawn_tank(&mut wn, htx + 1, hty);
        wn.cast_curse(htx as f32 + 1.5, hty as f32 + 0.5, "Amplify Damage").unwrap();
        assert!(wn.monster_curse(m).is_some());
        let loaded = World::load(&wn.save()).expect("save/load valide");
        assert!(loaded.monster_curse(m).is_none(), "malédiction non persistée");
        assert!(loaded.active_aura().is_none());
    }

    #[test]
    fn determinisme_meme_graine_traces_identiques() {
        let script = |w: &mut World| {
            let (sx, sy) = w.hero_spawn();
            let (htx, hty) = (sx.floor() as i32, sy.floor() as i32);
            w.clear_monsters();
            let a = spawn_tank(w, htx + 2, hty);
            let b = spawn_tank(w, htx + 20, hty);
            w.toggle_aura("Holy Fire").unwrap();
            for _ in 0..50 {
                w.tick(100, None); // 5 pulses
            }
            (
                w.monster_hp(a).map(|(h, _)| h),
                w.monster_hp(b).map(|(h, _)| h),
                w.hero_stats().life,
                w.monsters().iter().map(|m| (m.x, m.y)).collect::<Vec<_>>(),
            )
        };
        let a = script(&mut hero_with("PA", &[("Holy Fire", 1)]));
        let b = script(&mut hero_with("PA", &[("Holy Fire", 1)]));
        assert_eq!(a, b, "même graine ⇒ traces identiques (pulses compris)");
        // Et un script SANS aura reste bit-identique au flux historique.
        let sans = |w: &mut World| {
            w.clear_monsters();
            for _ in 0..50 {
                w.tick(100, None);
            }
            (w.hero_stats().life, w.monsters().iter().map(|m| (m.x, m.y)).collect::<Vec<_>>())
        };
        assert_eq!(sans(&mut world("PA")), sans(&mut world("PA")));
    }
}
