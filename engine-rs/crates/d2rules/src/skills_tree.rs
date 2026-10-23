//! Arbre de compétences D2 complet — socle C1→C4 de `finish-systemes.md`
//! (`skills.txt` v1.12 : 210 skills de classe + Attack/Kick + skills monstres).
//!
//! # Modèle (port des colonnes utiles, données réelles embarquées)
//! * [`SkillRecord`] : prérequis (`reqskill1-3`), niveau requis (`reqlevel`),
//!   plafond (`maxlvl` — 20 pour les 210 skills de classe), missiles
//!   (`srvmissile a/b/c`), dégâts physiques (Min/Max + courbes `*LevDam1-5`),
//!   dégâts élémentaires (`EType/EMin/EMax` + courbes + `ELen`), synergies
//!   (`DmgSymPerCalc`/`EDmgSymPerCalc` évaluées via [`crate::calc`]), mana,
//!   toucher, Param1-8, calcs bruts (calc1-4, prgcalc1-3, passives…).
//! * Courbes par niveau : formule itérative D2 — palier niv 1 = base, puis
//!   `+LevDam1` par niveau 2-8, `+LevDam2` 9-15, `+LevDam3` 16-22, `+LevDam4`
//!   23-28, `+LevDam5` 29+ (ancrage golden : Fire Bolt niv 8 = 27-33 feu).
//! * [`SkillTree`] : état d'un héros (points investis par skill, points
//!   non dépensés) + API pure `learn`/`can_learn`/`synergy_factor` — le
//!   câblage `world.rs` sera fait par l'intégration (module interdit ici).
//! * `ln12/ln34/ln56/ln78`/`dmXX` des formules = **paires Param** :
//!   `lnAB` ⇒ `ParamA + (niveau-1) × ParamB` (ancrage : Conviction
//!   `min(ln34,150)` = `min(30+5·(n-1),150)` = −30 %→−150 % résists D2).
//! * 4 skills par classe marqués jouables MVP ([`SkillTreeData::is_mvp`]) ;
//!   les 3 skills de départ de `skills.rs` restent valides.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::calc::{CalcString, EvalContext};
use crate::stats::split_tsv;

/// Table skills v1.12 embarquée (356 records, patch_d2.mpq).
pub(crate) const SKILLS_TXT: &str = include_str!("../../../../assets/patch_skills.txt");
/// Table skilldesc v1.12 (221 records : position d'arbre, icône, clés strings).
pub(crate) const SKILLDESC_TXT: &str = include_str!("../../../../assets/patch_skilldesc.txt");

/// Codes des 4 skills jouables MVP par classe (choix documenté : comportements
/// simples — missile direct, mêlée modifiée, aura/buff élémentaire — à câbler
/// par l'intégration ; le reste de l'arbre est apprenable et calculé).
const MVP_SKILLS: &[(&str, [&str; 4])] = &[
    ("ama", ["Fire Arrow", "Cold Arrow", "Poison Javelin", "Jab"]),
    ("sor", ["Fire Bolt", "Ice Bolt", "Fire Ball", "Lightning"]),
    ("nec", ["Teeth", "Bone Spear", "Poison Dagger", "Bone Armor"]),
    ("pal", ["Sacrifice", "Holy Bolt", "Holy Fire", "Prayer"]),
    ("bar", ["Bash", "Stun", "Double Swing", "Leap"]),
    ("dru", ["Firestorm", "Molten Boulder", "Arctic Blast", "Twister"]),
    ("ass", ["Fire Trauma", "Claw Mastery", "Dragon Talon", "Shock Field"]),
];

/// Type élémentaire d'un skill (`EType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ElemType {
    /// Aucun (`EType` vide).
    #[default]
    None,
    /// `fire`
    Fire,
    /// `ltng`
    Lightning,
    /// `cold`
    Cold,
    /// `pois`
    Poison,
    /// `mag`
    Magic,
    /// `stun`
    Stun,
}

impl ElemType {
    fn parse(s: &str) -> ElemType {
        match s.trim().to_ascii_lowercase().as_str() {
            "fire" => ElemType::Fire,
            "ltng" => ElemType::Lightning,
            "cold" => ElemType::Cold,
            "pois" => ElemType::Poison,
            "mag" | "magi" => ElemType::Magic,
            "stun" => ElemType::Stun,
            _ => ElemType::None,
        }
    }
}

/// Record d'un skill (`skills.txt`), colonnes utiles au moteur.
#[derive(Debug, Clone, Default)]
pub struct SkillRecord {
    /// Code table (colonne `skill`, ex. `"Fire Bolt"`).
    pub code: String,
    /// Id numérique (`Id`).
    pub id: u32,
    /// Classe (`charclass` : `ama`…`ass`) ; vide = Attack/Kick/monstres.
    pub class: String,
    /// Clé `skilldesc.txt` (libellés/position d'arbre).
    pub skilldesc: String,
    /// Niveau de personnage requis (`reqlevel`).
    pub req_level: u32,
    /// Plafond d'investissement (`maxlvl`, 20 pour les skills de classe).
    pub max_level: u32,
    /// Prérequis force/dex/int/vit (`reqstr/reqdex/reqint/reqvit`).
    pub req_str: u32,
    /// Prérequis dextérité.
    pub req_dex: u32,
    /// Prérequis intelligence.
    pub req_int: u32,
    /// Prérequis vitalité.
    pub req_vit: u32,
    /// Prérequis d'arbre (`reqskill1-3`, ≥ 1 point chacun).
    pub req_skills: Vec<String>,
    /// Missile serveur (`srvmissile`, ex. `firebolt`) ; None = mêlée/autres.
    pub srv_missile: Option<String>,
    /// Missiles additionnels a/b/c.
    pub srv_missile_a: Option<String>,
    /// Missile additionnel b.
    pub srv_missile_b: Option<String>,
    /// Missile additionnel c.
    pub srv_missile_c: Option<String>,
    /// Bonus de toucher (`ToHit`).
    pub to_hit: i64,
    /// Toucher par niveau (`LevToHit`).
    pub lev_to_hit: i64,
    /// Toucher dynamique (`ToHitCalc`).
    pub to_hit_calc: Option<CalcString>,
    /// Décalage de dégâts (`HitShift`, 8 = 256 → dégâts ×2^shift/256).
    pub hit_shift: u8,
    /// Part des dégâts d'arme (`SrcDam`, %×256 ; 0 = dégâts propres).
    pub src_dam: i32,
    /// Dégâts physiques min (`MinDam`) et courbe (`MinLevDam1-5`).
    pub min_dam: i64,
    /// Courbe min (5 paliers).
    pub min_lev_dam: [i64; 5],
    /// Dégâts physiques max (`MaxDam`).
    pub max_dam: i64,
    /// Courbe max.
    pub max_lev_dam: [i64; 5],
    /// Synergie dégâts physiques (`DmgSymPerCalc`).
    pub dmg_sym_per_calc: Option<CalcString>,
    /// Élément (`EType`).
    pub e_type: ElemType,
    /// Dégâts élémentaires min (`EMin`) et courbe (`EMinLev1-5`).
    pub e_min: i64,
    /// Courbe élément min.
    pub e_min_lev: [i64; 5],
    /// Dégâts élémentaires max (`EMax`).
    pub e_max: i64,
    /// Courbe élément max.
    pub e_max_lev: [i64; 5],
    /// Synergie dégâts élémentaires (`EDmgSymPerCalc`).
    pub edmg_sym_per_calc: Option<CalcString>,
    /// Durée d'effet de base (frames, `ELen` — poison/froid).
    pub e_len: i64,
    /// Courbe de durée (`ELevLen1-3`).
    pub e_lev_len: [i64; 3],
    /// Synergie de durée (`ELenSymPerCalc`).
    pub e_len_sym_per_calc: Option<CalcString>,
    /// Coût mana de base (`mana`).
    pub mana: i64,
    /// Mana par niveau (`lvlmana`).
    pub lvl_mana: i64,
    /// Plancher de mana (`minmana`).
    pub min_mana: i64,
    /// Mana de démarrage (`startmana`).
    pub start_mana: i64,
    /// Délai de cast en frames (`delay`).
    pub delay: i64,
    /// Délai de re-cast partagé (`perdelay`, frames) — nombre OU formule
    /// CalcString (`patch_skills.txt` col `perdelay` : auras = constante `50`,
    /// Thunder Storm = `(100-dm56)*par4/100+par3`). `None` = aucun délai ;
    /// évaluation à la demande dans un [`SkillEvalCtx`] (niveau du skill).
    pub per_delay: Option<CalcString>,
    /// Param1-8 (pilotent `parN`/`lnXX` des formules).
    pub params: [i64; 8],
    /// Formules calc1-4.
    pub calcs: [Option<CalcString>; 4],
    /// Formules prgcalc1-3.
    pub prg_calcs: [Option<CalcString>; 3],
    /// État d'aura (`aurastate`, ex. `might`).
    pub aura_state: String,
    /// État passif (`passivestate`).
    pub passive_state: String,
    /// Stats passives 1-3 (`passivestat1-3`).
    pub passive_stats: [String; 3],
    /// Formules passives 1-3 (`passivecalc1-3`).
    pub passive_calcs: [Option<CalcString>; 3],
}

impl SkillRecord {
    /// Skill de classe jouable (peut recevoir des points) ?
    pub fn is_class_skill(&self) -> bool {
        !self.class.is_empty()
    }
}

/// Lookup des niveaux de skills (contexte d'évaluation des formules).
pub trait SkillLevels {
    /// Points investis dans `code` (0 si aucun).
    fn blvl(&self, code: &str) -> u32;
    /// Niveau du personnage (pour `ulvl`).
    fn char_level(&self) -> u32 {
        1
    }
}

impl SkillLevels for HashMap<String, u32> {
    fn blvl(&self, code: &str) -> u32 {
        self.get(&code.to_ascii_lowercase()).copied().unwrap_or(0)
    }
}

/// Niveaux vides (aucun point investi).
pub struct NoLevels;
impl SkillLevels for NoLevels {
    fn blvl(&self, _code: &str) -> u32 {
        0
    }
}

/// Contexte d'évaluation des formules d'UN skill — implémente
/// [`EvalContext`] avec le record courant + les niveaux du héros.
pub struct SkillEvalCtx<'a> {
    /// Skill PROPRIÉTAIRE de la formule (résout `parN`/`lnXX`/`lvl`).
    pub rec: &'a SkillRecord,
    /// Niveau du skill propriétaire.
    pub own_lvl: u32,
    /// Niveaux des autres skills (synergies).
    pub levels: &'a dyn SkillLevels,
    /// Stats du héros (résout `stat('x')` ; défaut vide).
    pub stats: Option<&'a crate::stats::StatList>,
    /// Missile associé au calcul (résout `miss('x')` — défaut 1).
    pub missile_l: i64,
}

impl<'a> SkillEvalCtx<'a> {
    /// Contexte minimal : record + niveau + niveaux héros.
    pub fn new(rec: &'a SkillRecord, own_lvl: u32, levels: &'a dyn SkillLevels) -> Self {
        SkillEvalCtx { rec, own_lvl, levels, stats: None, missile_l: 1 }
    }

    /// Paire Param d'une autre record (`skill('X'.ln12)`).
    fn param_pair(rec: &SkillRecord, a: usize, b: usize, lvl: u32) -> i64 {
        let base = rec.params[a.saturating_sub(1)];
        let incr = rec.params[b.saturating_sub(1)];
        base + (lvl.max(1) as i64 - 1) * incr
    }
}

impl EvalContext for SkillEvalCtx<'_> {
    fn skill_level(&mut self, skill: &str, qualifier: &str) -> i64 {
        let lvl = self.levels.blvl(skill);
        match qualifier {
            // blvl = points investis ; lvl = points + bonus équipement (les
            // +skills d'objet n'existent pas encore : identiques, documenté).
            "blvl" | "lvl" | "" => lvl as i64,
            // skill('X'.parN) : le ParamN du record référencé.
            q if q.starts_with("par") => {
                let n: usize = q[3..].parse().unwrap_or(0);
                if (1..=8).contains(&n) {
                    self.self_or_foreign(skill).map(|r| r.params[n - 1]).unwrap_or(0)
                } else {
                    0
                }
            }
            // skill('X'.ln12) : la paire du record référencé, à SON niveau.
            q if q.starts_with("ln") || q.starts_with("dm") => {
                let digits = &q[2..];
                if digits.len() == 2 {
                    let a: usize = digits[..1].parse().unwrap_or(0);
                    let b: usize = digits[1..].parse().unwrap_or(0);
                    if (1..=8).contains(&a) && (1..=8).contains(&b) {
                        match self.self_or_foreign(skill) {
                            Some(r) => Self::param_pair(r, a, b, lvl),
                            None => 0,
                        }
                    } else {
                        0
                    }
                } else {
                    0
                }
            }
            _ => lvl as i64,
        }
    }

    fn stat_value(&mut self, stat: &str, _qualifier: &str) -> i64 {
        self.stats.map(|s| s.get(stat)).unwrap_or(0)
    }

    fn missile_prop(&mut self, _missile: &str, _qualifier: &str) -> i64 {
        self.missile_l
    }

    fn bare(&mut self, name: &str) -> i64 {
        match name {
            "lvl" | "sklvl" => self.own_lvl as i64,
            "ulvl" => self.levels.char_level() as i64,
            _ => {
                // parN / lnXX / dmXX du skill propriétaire.
                if let Some(n) = name.strip_prefix("par") {
                    if let Ok(n) = n.parse::<usize>() {
                        if (1..=8).contains(&n) {
                            return self.rec.params[n - 1];
                        }
                    }
                    return 1;
                }
                for prefix in ["ln", "dm"] {
                    if let Some(digits) = name.strip_prefix(prefix) {
                        if digits.len() == 2 {
                            if let (Ok(a), Ok(b)) =
                                (digits[..1].parse::<usize>(), digits[1..].parse::<usize>())
                            {
                                if (1..=8).contains(&a) && (1..=8).contains(&b) {
                                    return Self::param_pair(self.rec, a, b, self.own_lvl);
                                }
                            }
                        }
                        return 1;
                    }
                }
                1
            }
        }
    }
}

impl SkillEvalCtx<'_> {
    fn self_or_foreign(&self, skill: &str) -> Option<&SkillRecord> {
        if skill.eq_ignore_ascii_case(&self.rec.code) {
            Some(self.rec)
        } else {
            None // le contexte minimal n'a pas accès aux autres records ;
                  // SkillTreeData::eval_ctx fournit la variante complète.
        }
    }
}

/// Table skilldesc : position d'arbre + icône + clés strings fr (les chaînes
/// elles-mêmes = tables STRING/TBL, autre domaine — exposées en clés).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SkillDescRec {
    /// Clé (`skilldesc`, ex. `firebolt`).
    pub code: String,
    /// Onglet de l'arbre (`SkillPage`, 1-3).
    pub page: u32,
    /// Rangée dans l'onglet (`SkillRow`).
    pub row: u32,
    /// Colonne dans l'onglet (`SkillColumn`).
    pub column: u32,
    /// Icône dans `skillicon.dc6` (`IconCel`).
    pub icon_cel: u32,
    /// Clé string du nom (`str name`).
    pub str_name: String,
    /// Clé string courte (`str short`).
    pub str_short: String,
    /// Clé string longue (`str long`).
    pub str_long: String,
}

/// Tables chargées : skills + skilldesc — immuable, partagée ([`global`]).
#[derive(Debug, Default)]
pub struct SkillTreeData {
    records: Vec<SkillRecord>,
    by_code: HashMap<String, usize>,
    class_index: HashMap<String, Vec<usize>>,
    descs: Vec<SkillDescRec>,
    desc_by_code: HashMap<String, usize>,
}

impl SkillTreeData {
    /// Parse un TSV `skills.txt` v1.12.
    pub fn parse(txt: &str) -> SkillTreeData {
        let mut data = SkillTreeData::default();
        let mut rows = txt.lines().filter(|l| !l.trim().is_empty()).map(split_tsv);
        let Some(hdr) = rows.next() else { return data };
        let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
        let cell = |r: &[String], c: Option<usize>| -> String {
            c.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default()
        };
        let num = |r: &[String], c: Option<usize>| -> i64 {
            cell(r, c).parse().unwrap_or(0)
        };
        let opt_str = |v: String| -> Option<String> {
            let v = v.trim();
            (!v.is_empty() && !v.eq_ignore_ascii_case("none")).then(|| v.to_string())
        };
        let c_code = col("skill");
        let c_id = col("Id");
        let c_class = col("charclass");
        let c_desc = col("skilldesc");
        let c_arr: Vec<Option<usize>> =
            ["reqskill1", "reqskill2", "reqskill3"].iter().map(|c| col(c)).collect();
        for r in rows {
            let code = cell(&r, c_code);
            if code.is_empty() || code.starts_with('*') {
                continue;
            }
            let class = cell(&r, c_class).to_ascii_lowercase();
            let req_skills: Vec<String> = c_arr
                .iter()
                .filter_map(|c| opt_str(cell(&r, *c)))
                .collect();
            let calcs: [Option<CalcString>; 4] =
                ["calc1", "calc2", "calc3", "calc4"]
                    .iter()
                    .map(|c| col(c))
                    .map(|c| {
                        let v = cell(&r, c);
                        (!v.is_empty()).then(|| CalcString::parse(&v))
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
            let prg_calcs: [Option<CalcString>; 3] =
                ["prgcalc1", "prgcalc2", "prgcalc3"]
                    .iter()
                    .map(|c| col(c))
                    .map(|c| {
                        let v = cell(&r, c);
                        (!v.is_empty()).then(|| CalcString::parse(&v))
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
            let passive_stats: [String; 3] = ["passivestat1", "passivestat2", "passivestat3"]
                .iter()
                .map(|c| cell(&r, col(c)))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let passive_calcs: [Option<CalcString>; 3] =
                ["passivecalc1", "passivecalc2", "passivecalc3"]
                    .iter()
                    .map(|c| col(c))
                    .map(|c| {
                        let v = cell(&r, c);
                        (!v.is_empty()).then(|| CalcString::parse(&v))
                    })
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap();
            let params: [i64; 8] = (1..=8)
                .map(|n| col(&format!("Param{n}")))
                .map(|c| num(&r, c))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let lev = |name: &str| -> [i64; 5] {
                (1..=5)
                    .map(|n| col(&format!("{name}{n}")))
                    .map(|c| num(&r, c))
                    .collect::<Vec<_>>()
                    .try_into()
                    .unwrap()
            };
            let e_lev_len: [i64; 3] = (1..=3)
                .map(|n| col(&format!("ELevLen{n}")))
                .map(|c| num(&r, c))
                .collect::<Vec<_>>()
                .try_into()
                .unwrap();
            let rec = SkillRecord {
                code,
                id: num(&r, c_id).max(0) as u32,
                class,
                skilldesc: cell(&r, c_desc),
                req_level: num(&r, col("reqlevel")).max(0) as u32,
                max_level: {
                    let m = num(&r, col("maxlvl"));
                    if m <= 0 { 20 } else { m as u32 }
                },
                req_str: num(&r, col("reqstr")).max(0) as u32,
                req_dex: num(&r, col("reqdex")).max(0) as u32,
                req_int: num(&r, col("reqint")).max(0) as u32,
                req_vit: num(&r, col("reqvit")).max(0) as u32,
                req_skills,
                srv_missile: opt_str(cell(&r, col("srvmissile"))),
                srv_missile_a: opt_str(cell(&r, col("srvmissilea"))),
                srv_missile_b: opt_str(cell(&r, col("srvmissileb"))),
                srv_missile_c: opt_str(cell(&r, col("srvmissilec"))),
                to_hit: num(&r, col("ToHit")),
                lev_to_hit: num(&r, col("LevToHit")),
                to_hit_calc: opt_str(cell(&r, col("ToHitCalc")))
                    .map(|v| CalcString::parse(&v)),
                hit_shift: num(&r, col("HitShift")).clamp(0, 31) as u8,
                src_dam: num(&r, col("SrcDam")) as i32,
                min_dam: num(&r, col("MinDam")),
                min_lev_dam: lev("MinLevDam"),
                max_dam: num(&r, col("MaxDam")),
                max_lev_dam: lev("MaxLevDam"),
                dmg_sym_per_calc: opt_str(cell(&r, col("DmgSymPerCalc")))
                    .map(|v| CalcString::parse(&v)),
                e_type: ElemType::parse(&cell(&r, col("EType"))),
                e_min: num(&r, col("EMin")),
                e_min_lev: lev("EMinLev"),
                e_max: num(&r, col("EMax")),
                e_max_lev: lev("EMaxLev"),
                edmg_sym_per_calc: opt_str(cell(&r, col("EDmgSymPerCalc")))
                    .map(|v| CalcString::parse(&v)),
                e_len: num(&r, col("ELen")),
                e_lev_len,
                e_len_sym_per_calc: opt_str(cell(&r, col("ELenSymPerCalc")))
                    .map(|v| CalcString::parse(&v)),
                mana: num(&r, col("mana")),
                lvl_mana: num(&r, col("lvlmana")),
                min_mana: num(&r, col("minmana")),
                start_mana: num(&r, col("startmana")),
                delay: num(&r, col("delay")),
                per_delay: opt_str(cell(&r, col("perdelay")))
                    .map(|v| CalcString::parse(&v)),
                params,
                calcs,
                prg_calcs,
                aura_state: cell(&r, col("aurastate")),
                passive_state: cell(&r, col("passivestate")),
                passive_stats,
                passive_calcs,
            };
            let key = rec.code.to_ascii_lowercase();
            if rec.is_class_skill() {
                data.class_index.entry(rec.class.clone()).or_default().push(data.records.len());
            }
            data.by_code.insert(key, data.records.len());
            data.records.push(rec);
        }
        data
    }

    /// Parse le TSV `skilldesc.txt` (arbre UI).
    pub fn parse_skilldesc(txt: &str) -> Vec<SkillDescRec> {
        let mut rows = txt.lines().filter(|l| !l.trim().is_empty()).map(split_tsv);
        let Some(hdr) = rows.next() else { return Vec::new() };
        let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
        let cell = |r: &[String], c: Option<usize>| -> String {
            c.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default()
        };
        let num = |r: &[String], c: Option<usize>| -> u32 {
            cell(r, c).parse().unwrap_or(0)
        };
        rows.map(|r| SkillDescRec {
            code: cell(&r, col("skilldesc")),
            page: num(&r, col("SkillPage")),
            row: num(&r, col("SkillRow")),
            column: num(&r, col("SkillColumn")),
            icon_cel: num(&r, col("IconCel")),
            str_name: cell(&r, col("str name")),
            str_short: cell(&r, col("str short")),
            str_long: cell(&r, col("str long")),
        })
        .filter(|d| !d.code.is_empty() && !d.code.starts_with('*'))
        .collect()
    }

    /// Tables globales embarquées (parse au premier accès).
    pub fn global() -> &'static SkillTreeData {
        static DATA: OnceLock<SkillTreeData> = OnceLock::new();
        DATA.get_or_init(|| {
            let mut d = SkillTreeData::parse(SKILLS_TXT);
            d.descs = SkillTreeData::parse_skilldesc(SKILLDESC_TXT);
            d.desc_by_code = d
                .descs
                .iter()
                .enumerate()
                .map(|(i, rec)| (rec.code.to_ascii_lowercase(), i))
                .collect();
            d
        })
    }

    /// Record par code (insensible à la casse).
    pub fn get(&self, code: &str) -> Option<&SkillRecord> {
        self.by_code.get(&code.to_ascii_lowercase()).map(|&i| &self.records[i])
    }

    /// Tous les records (ordre du fichier).
    pub fn records(&self) -> &[SkillRecord] {
        &self.records
    }

    /// Skills de classe apprenables (indices dans [`records`]), ordre du fichier.
    pub fn class_indices(&self, class: &str) -> &[usize] {
        self.class_index
            .get(&class.to_ascii_lowercase())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Skill marqué jouable MVP ?
    pub fn is_mvp(&self, code: &str) -> bool {
        let Some(rec) = self.get(code) else { return false };
        MVP_SKILLS
            .iter()
            .filter(|(c, _)| *c == rec.class)
            .any(|(_, skills)| skills.iter().any(|s| s.eq_ignore_ascii_case(&rec.code)))
    }

    /// Description d'arbre (skilldesc) d'un code de skill.
    pub fn desc_of(&self, code: &str) -> Option<&SkillDescRec> {
        let rec = self.get(code)?;
        self.desc_by_code
            .get(&rec.skilldesc.to_ascii_lowercase())
            .map(|&i| &self.descs[i])
    }

    /// Courbe itérative D2 : base + incrément du palier à chaque niveau.
    fn curve(base: i64, levs: &[i64; 5], lvl: u32) -> i64 {
        let mut v = base;
        for l in 2..=lvl {
            v += match l {
                2..=8 => levs[0],
                9..=15 => levs[1],
                16..=22 => levs[2],
                23..=28 => levs[3],
                _ => levs[4],
            };
        }
        v
    }

    /// Dégâts physiques du skill à `lvl` (base au niveau 0/1).
    pub fn damage_at(&self, rec: &SkillRecord, lvl: u32) -> (i64, i64) {
        (
            Self::curve(rec.min_dam, &rec.min_lev_dam, lvl),
            Self::curve(rec.max_dam, &rec.max_lev_dam, lvl),
        )
    }

    /// Dégâts élémentaires du skill à `lvl` (min, max).
    pub fn elem_damage_at(&self, rec: &SkillRecord, lvl: u32) -> (i64, i64) {
        (
            Self::curve(rec.e_min, &rec.e_min_lev, lvl),
            Self::curve(rec.e_max, &rec.e_max_lev, lvl),
        )
    }

    /// Durée d'effet (frames) à `lvl` (ELen + ELevLen1-3, même itération).
    pub fn elem_len_at(&self, rec: &SkillRecord, lvl: u32) -> i64 {
        let mut v = rec.e_len;
        for l in 2..=lvl {
            v += match l {
                2..=8 => rec.e_lev_len[0],
                9..=15 => rec.e_lev_len[1],
                _ => rec.e_lev_len[2],
            };
        }
        v
    }

    /// Coût mana à `lvl` : `mana + (lvl-1) × lvlmana`, planché à `minmana`.
    pub fn mana_cost_at(&self, rec: &SkillRecord, lvl: u32) -> i64 {
        let raw = rec.mana + (lvl.max(1) as i64 - 1) * rec.lvl_mana;
        raw.max(rec.min_mana)
    }

    /// % de synergie brut (`DmgSymPerCalc`/`EDmgSymPerCalc`) — formule réelle
    /// évaluée dans le contexte (niveaux héros + Params du skill).
    pub fn synergy_percent(
        &self,
        rec: &SkillRecord,
        kind: SynergyKind,
        levels: &dyn SkillLevels,
    ) -> i64 {
        let calc = match kind {
            SynergyKind::Physical => rec.dmg_sym_per_calc.as_ref(),
            SynergyKind::Elemental => rec.edmg_sym_per_calc.as_ref(),
        };
        let Some(calc) = calc else { return 0 };
        let own_lvl = levels.blvl(&rec.code);
        let mut ctx = FullEvalCtx { data: self, inner: SkillEvalCtx::new(rec, own_lvl, levels) };
        calc.eval(&mut ctx)
    }
}

/// Type de synergie : dégâts physiques ou élémentaires.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SynergyKind {
    /// `DmgSymPerCalc`
    Physical,
    /// `EDmgSymPerCalc`
    Elemental,
}

/// Contexte complet : comme [`SkillEvalCtx`] mais `skill('Autre'.…)` résout
/// dans les tables (Params/paires du record référencé).
pub struct FullEvalCtx<'a, 's> {
    data: &'a SkillTreeData,
    inner: SkillEvalCtx<'s>,
}

impl EvalContext for FullEvalCtx<'_, '_> {
    fn skill_level(&mut self, skill: &str, qualifier: &str) -> i64 {
        if skill.eq_ignore_ascii_case(&self.inner.rec.code) {
            return self.inner.skill_level(skill, qualifier);
        }
        let lvl = self.inner.levels.blvl(skill);
        let Some(rec) = self.data.get(skill) else { return 0 };
        match qualifier {
            "blvl" | "lvl" | "" => lvl as i64,
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
                    let a: usize = digits[..1].parse().unwrap_or(0);
                    let b: usize = digits[1..].parse().unwrap_or(0);
                    if (1..=8).contains(&a) && (1..=8).contains(&b) {
                        SkillEvalCtx::param_pair(rec, a, b, lvl.max(1))
                    } else {
                        0
                    }
                } else {
                    0
                }
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

/// Erreurs d'apprentissage (diagnostiques UI arbre).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillTreeError {
    /// Code inconnu des tables.
    UnknownSkill(String),
    /// Pas un skill de la classe du héros (ou skill monstre/Attack).
    WrongClass { skill: String, hero_class: String },
    /// Un prérequis n'a pas de point.
    MissingPrereq { skill: String, required: String },
    /// Plafond d'investissement atteint (`maxlvl`, 20).
    MaxLevel { skill: String, cap: u32 },
    /// Plus aucun point de skill à dépenser.
    NoPoints,
    /// Prérequis de niveau de personnage non atteint (`reqlevel`).
    LevelTooLow { skill: String, required: u32 },
}

impl std::fmt::Display for SkillTreeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillTreeError::UnknownSkill(s) => write!(f, "skill inconnu : {s}"),
            SkillTreeError::WrongClass { skill, hero_class } => {
                write!(f, "{skill} n'est pas un skill de classe {hero_class}")
            }
            SkillTreeError::MissingPrereq { skill, required } => {
                write!(f, "{skill} exige 1 point dans {required}")
            }
            SkillTreeError::MaxLevel { skill, cap } => {
                write!(f, "{skill} est au plafond ({cap})")
            }
            SkillTreeError::NoPoints => write!(f, "aucun point de skill disponible"),
            SkillTreeError::LevelTooLow { skill, required } => {
                write!(f, "{skill} demande le niveau {required}")
            }
        }
    }
}

impl std::error::Error for SkillTreeError {}

/// État d'arbre d'un héros : niveaux par skill + points. API pure
/// (`learn`/`can_learn`/`synergy_factor`) — l'intégration câble `world.rs`.
#[derive(Debug, Clone)]
pub struct SkillTree<'a> {
    data: &'a SkillTreeData,
    class: String,
    /// Niveau du personnage (porte `reqlevel` des skills ; 1 par défaut).
    char_level: u32,
    levels: HashMap<String, u32>,
    unspent: u32,
}

impl SkillTree<'static> {
    /// Arbre du héros sur les tables globales embarquées.
    pub fn for_class(hero_class: &str) -> SkillTree<'static> {
        SkillTree::with_data(hero_class, SkillTreeData::global())
    }
}

impl<'a> SkillTree<'a> {
    /// Arbre sur des tables arbitraires (tests).
    pub fn with_data(hero_class: &str, data: &'a SkillTreeData) -> SkillTree<'a> {
        SkillTree {
            data,
            class: hero_class.to_ascii_lowercase(),
            char_level: 1,
            levels: HashMap::new(),
            unspent: 0,
        }
    }

    /// Classe du héros (code court).
    pub fn class(&self) -> &str {
        &self.class
    }

    /// Niveau de personnage connu de l'arbre (porte `reqlevel`).
    pub fn char_level(&self) -> u32 {
        self.char_level
    }

    /// Met à jour le niveau du personnage (montée de niveau).
    pub fn set_char_level(&mut self, level: u32) {
        self.char_level = level.max(1);
    }

    /// Points de skill non dépensés.
    pub fn unspent_points(&self) -> u32 {
        self.unspent
    }

    /// Accorde des points (1/niveau de perso + récompenses de quêtes).
    pub fn grant_points(&mut self, n: u32) {
        self.unspent = self.unspent.saturating_add(n);
    }

    /// Points investis au total (somme des niveaux).
    pub fn points_spent(&self) -> u32 {
        self.levels.values().sum()
    }

    /// Points investis dans `code` (0 si aucun).
    pub fn level_of(&self, code: &str) -> u32 {
        self.levels.get(&code.to_ascii_lowercase()).copied().unwrap_or(0)
    }

    /// Force un niveau (chargement de sauvegarde ; plafonné au `maxlvl`).
    pub fn set_level(&mut self, code: &str, level: u32) {
        let cap = self.data.get(code).map(|r| r.max_level).unwrap_or(0);
        let lvl = level.min(cap);
        if lvl == 0 {
            self.levels.remove(&code.to_ascii_lowercase());
        } else {
            self.levels.insert(code.to_ascii_lowercase(), lvl);
        }
    }

    /// Niveaux (pour la sauvegarde).
    pub fn levels(&self) -> &HashMap<String, u32> {
        &self.levels
    }

    /// Peut-on mettre un point dans `code` ? Renvoie l'erreur exacte sinon.
    pub fn can_learn(&self, code: &str) -> Result<(), SkillTreeError> {
        let Some(rec) = self.data.get(code) else {
            return Err(SkillTreeError::UnknownSkill(code.into()));
        };
        if !rec.is_class_skill() || !rec.class.eq_ignore_ascii_case(&self.class) {
            return Err(SkillTreeError::WrongClass {
                skill: rec.code.clone(),
                hero_class: self.class.clone(),
            });
        }
        for req in &rec.req_skills {
            if self.level_of(req) == 0 {
                return Err(SkillTreeError::MissingPrereq {
                    skill: rec.code.clone(),
                    required: req.clone(),
                });
            }
        }
        if self.level_of(code) >= rec.max_level {
            return Err(SkillTreeError::MaxLevel { skill: rec.code.clone(), cap: rec.max_level });
        }
        if self.char_level < rec.req_level {
            return Err(SkillTreeError::LevelTooLow {
                skill: rec.code.clone(),
                required: rec.req_level,
            });
        }
        if self.unspent == 0 {
            return Err(SkillTreeError::NoPoints);
        }
        Ok(())
    }

    /// Dépense 1 point dans `code` ; renvoie le nouveau niveau.
    pub fn learn(&mut self, code: &str) -> Result<u32, SkillTreeError> {
        self.can_learn(code)?;
        let key = code.to_ascii_lowercase();
        let lvl = self.levels.entry(key).or_insert(0);
        *lvl += 1;
        self.unspent -= 1;
        Ok(*lvl)
    }

    /// Skill connu (≥ 1 point) ?
    pub fn knows(&self, code: &str) -> bool {
        self.level_of(code) > 0
    }

    /// % de synergie d'un skill (via [`SkillTreeData::synergy_percent`] avec
    /// les niveaux de CE héros).
    pub fn synergy_percent(&self, code: &str, kind: SynergyKind) -> i64 {
        match self.data.get(code) {
            Some(rec) => self.data.synergy_percent(rec, kind, &self.levels),
            None => 0,
        }
    }

    /// Multiplicateur de synergie : `1 + %/100` (1.0 sans synergie) — le
    /// facteur appliqué aux dégâts du skill.
    pub fn synergy_factor(&self, code: &str, kind: SynergyKind) -> f64 {
        1.0 + self.synergy_percent(code, kind) as f64 / 100.0
    }

    /// Dégâts du skill à SON niveau actuel (physiques, élémentaires, type,
    /// durée) — base × courbe, SANS synergie (appliquer [`synergy_factor`]).
    pub fn current_damage(&self, code: &str) -> ((i64, i64), (i64, i64), ElemType, i64) {
        let Some(rec) = self.data.get(code) else {
            return ((0, 0), (0, 0), ElemType::None, 0);
        };
        let lvl = self.level_of(code);
        let phys = self.data.damage_at(rec, lvl);
        let elem = self.data.elem_damage_at(rec, lvl);
        let len = self.data.elem_len_at(rec, lvl);
        (phys, elem, rec.e_type, len)
    }

    /// Coût mana du skill à son niveau actuel.
    pub fn mana_cost(&self, code: &str) -> i64 {
        match self.data.get(code) {
            Some(rec) => self.data.mana_cost_at(rec, self.level_of(code)),
            None => 0,
        }
    }

    /// Missile serveur du skill (`srvmissile`) — câblage tir d2missiles.
    pub fn missile_of(&self, code: &str) -> Option<String> {
        self.data.get(code).and_then(|r| r.srv_missile.clone())
    }

    /// Contexte d'évaluation complet pour CE héros (record + niveaux).
    pub fn eval_ctx<'s>(&'s self, rec: &'a SkillRecord) -> FullEvalCtx<'a, 's> {
        FullEvalCtx {
            data: self.data,
            inner: SkillEvalCtx::new(rec, self.level_of(&rec.code), &self.levels),
        }
    }

    /// Tables sous-jacentes.
    pub fn data(&self) -> &'a SkillTreeData {
        self.data
    }
}

// ---------------------------------------------------------------------------
// Tests unitaires comportementaux (les golden données réelles : parity_tests.rs)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> &'static SkillTreeData {
        SkillTreeData::global()
    }

    #[test]
    fn tables_load_210_class_skills() {
        let d = data();
        let class_count = d
            .records()
            .iter()
            .filter(|r| r.is_class_skill())
            .count();
        assert_eq!(class_count, 210, "30 skills × 7 classes");
        for class in ["ama", "sor", "nec", "pal", "bar", "dru", "ass"] {
            assert_eq!(d.class_indices(class).len(), 30, "30 skills {class}");
        }
    }

    #[test]
    fn firebolt_record_and_curve() {
        let d = data();
        let fb = d.get("Fire Bolt").unwrap();
        assert_eq!(fb.class, "sor");
        assert_eq!(fb.id, 36);
        assert_eq!(fb.srv_missile.as_deref(), Some("firebolt"));
        assert_eq!(fb.e_type, ElemType::Fire);
        assert_eq!(fb.max_level, 20);
        assert!(fb.req_skills.is_empty());
        // Courbe EMin 6 / +3 (2-8) / +4 (9-15) : ancrages Phrozen Keep.
        // Côté max : 12 / +3 / +6.
        assert_eq!(d.elem_damage_at(fb, 1), (6, 12));
        assert_eq!(d.elem_damage_at(fb, 8), (27, 33));
        assert_eq!(d.elem_damage_at(fb, 9), (31, 39));
    }

    #[test]
    fn learn_requires_prereq_points_and_cap() {
        let mut t = SkillTree::for_class("SOR");
        t.set_char_level(30);
        // Fire Ball exige Fire Bolt.
        assert_eq!(
            t.can_learn("Fire Ball"),
            Err(SkillTreeError::MissingPrereq {
                skill: "Fire Ball".into(),
                required: "Fire Bolt".into()
            })
        );
        // 1 point : juste de quoi apprendre Fire Bolt — Fire Ball doit alors
        // être refusé faute de point (branche NoPoints).
        t.grant_points(1);
        assert_eq!(t.learn("Fire Bolt").unwrap(), 1);
        assert_eq!(t.unspent_points(), 0);
        assert_eq!(
            t.can_learn("Fire Ball"),
            Err(SkillTreeError::NoPoints),
            "plus de point ⇒ refus"
        );
        t.grant_points(1);
        assert_eq!(t.learn("Fire Ball").unwrap(), 1, "prérequis OK ⇒ appris");
        assert!(t.knows("Fire Bolt") && t.knows("Fire Ball"));
        // Autre classe ⇒ refus (ex. Bash barbare).
        assert!(matches!(
            t.can_learn("Bash"),
            Err(SkillTreeError::WrongClass { .. })
        ));
        // Plafond 20.
        for _ in 0..19 {
            t.grant_points(1);
            t.learn("Fire Bolt").unwrap();
        }
        assert_eq!(t.level_of("Fire Bolt"), 20);
        t.grant_points(1);
        assert_eq!(
            t.can_learn("Fire Bolt"),
            Err(SkillTreeError::MaxLevel { skill: "Fire Bolt".into(), cap: 20 })
        );
    }

    #[test]
    fn reqlevel_gates_learning() {
        let mut t = SkillTree::for_class("sor");
        t.set_char_level(1);
        t.grant_points(5);
        // Fire Bolt reqlevel 1 : OK.
        assert!(t.can_learn("Fire Bolt").is_ok());
        // Fire Ball reqlevel 12 : refus tant que le héros est niv 1.
        assert_eq!(
            t.can_learn("Fire Ball"),
            Err(SkillTreeError::MissingPrereq { skill: "Fire Ball".into(), required: "Fire Bolt".into() })
        );
        t.learn("Fire Bolt").unwrap();
        assert_eq!(
            t.can_learn("Fire Ball"),
            Err(SkillTreeError::LevelTooLow { skill: "Fire Ball".into(), required: 12 })
        );
        t.set_char_level(12);
        assert!(t.can_learn("Fire Ball").is_ok());
    }

    #[test]
    fn firebolt_synergy_factor() {
        let mut t = SkillTree::for_class("SOR");
        t.set_level("Fire Bolt", 10);
        assert_eq!(t.synergy_percent("Fire Bolt", SynergyKind::Elemental), 0);
        t.set_level("Fire Ball", 20);
        t.set_level("Meteor", 20);
        // (20+20) × par8(16) = 640 % ⇒ ×7,4 (golden cross-check calc).
        assert_eq!(t.synergy_percent("Fire Bolt", SynergyKind::Elemental), 640);
        assert!((t.synergy_factor("Fire Bolt", SynergyKind::Elemental) - 7.4).abs() < 1e-9);
        // Sans synergie ⇒ ×1.0.
        assert!((t.synergy_factor("Fire Bolt", SynergyKind::Physical) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ln_param_pairs_conviction() {
        let d = data();
        let conv = d.get("Conviction").unwrap();
        // Params : P3 = 30 (base), P4 = 5 (/niveau) ; plafond 150 ⇒ niv 25.
        let calc = CalcString::parse("min(ln34,150)");
        let mut n25 = SkillEvalCtx::new(conv, 25, &NoLevels);
        let mut n1 = SkillEvalCtx::new(conv, 1, &NoLevels);
        let mut n30 = SkillEvalCtx::new(conv, 30, &NoLevels);
        assert_eq!(calc.eval(&mut n25), 150, "plafond 150 atteint au niv 25");
        assert_eq!(calc.eval(&mut n1), 30, "niveau 1 = 30 %");
        assert_eq!(calc.eval(&mut n30), 150);
    }

    #[test]
    fn mana_cost_and_missile() {
        let mut t = SkillTree::for_class("sor");
        t.set_level("Fire Bolt", 1);
        assert_eq!(t.mana_cost("Fire Bolt"), 5);
        t.set_level("Fire Ball", 20);
        assert_eq!(t.mana_cost("Fire Ball"), 10 + 19, "10 + 19×1 lvlmana");
        assert_eq!(t.missile_of("Fire Ball").as_deref(), Some("fireball"));
        assert_eq!(t.missile_of("Bash"), None);
    }

    #[test]
    fn perdelay_parse_and_eval() {
        let d = data();
        // Auras : `perdelay` = constante 50 frames (patch_skills.txt col.
        // `perdelay`, ligne Might/Prayer/Resist Fire…).
        let might = d.get("Might").unwrap();
        let calc = might.per_delay.clone().expect("Might porte perdelay=50");
        assert_eq!(calc.eval(&mut SkillEvalCtx::new(might, 1, &NoLevels)), 50);
        // Formule réelle : Thunder Storm `(100-dm56)*par4/100+par3` — parse ET
        // évalue (Params 5/6/4/3 du record, dm56 à son niveau).
        let ts = d.get("Thunder Storm").unwrap();
        let ts_calc = ts.per_delay.clone().expect("Thunder Storm porte une formule perdelay");
        assert!(
            ts_calc.eval(&mut SkillEvalCtx::new(ts, 1, &NoLevels)) > 0,
            "formule perdelay évaluable"
        );
        // Fire Bolt : aucun perdelay (colonne vide).
        assert!(d.get("Fire Bolt").unwrap().per_delay.is_none());
    }

    #[test]
    fn mvp_flags_and_desc_lookup() {
        let d = data();
        assert!(d.is_mvp("Fire Bolt"));
        assert!(d.is_mvp("Bash"));
        assert!(!d.is_mvp("Vengeance"), "hors liste MVP");
        let desc = d.desc_of("Fire Bolt").expect("skilldesc firebolt");
        // Table réelle v1.12 : `fire bolt` → SkillPage 1 (l'onglet Feu de la
        // sorcière EST la page 1 de skilldesc.txt ; 2 = Foudre, 3 = Froid).
        assert_eq!(desc.page, 1, "onglet Feu de la sorcière = page 1");
        assert_eq!((desc.row, desc.column), (1, 2), "place dans l'arbre Feu");
        assert!(!desc.str_name.is_empty(), "clé string du nom présente");
    }

    #[test]
    fn unknown_code_is_safe() {
        let t = SkillTree::for_class("bar");
        assert_eq!(t.can_learn("N'importe Quoi"), Err(SkillTreeError::UnknownSkill("N'importe Quoi".into())));
        assert_eq!(t.level_of("N'importe Quoi"), 0);
        assert!(matches!(t.current_damage("N'importe Quoi"), ((0, 0), (0, 0), ElemType::None, 0)));
        assert_eq!(t.mana_cost("N'importe Quoi"), 0);
        assert!((t.synergy_factor("N'importe Quoi", SynergyKind::Elemental) - 1.0).abs() < 1e-9);
    }
}
