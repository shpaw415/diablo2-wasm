//! Golden tests record-à-record contre les tables v1.12 réelles (pattern
//! `shortcut-extrait-mpq-hash-d2`) : les dumps de `tests/golden/` ont été
//! générés par un parseur Python INDÉPENDANT (csv + évaluateur maison) AVANT
//! d'écrire le Rust. Le test compare chaque record/champ : toute divergence
//! parseur Rust ⇄ table v1.12 est un échec.
//!
//! Sources (patch_d2.mpq, extraites 2026-10-22) :
//! * `golden_xp.txt` — 99 niveaux × 7 classes + MaxLvl + ExpRatio.
//! * `golden_skills.txt` — 210 skills de classe × 82 colonnes modélisées.
//! * `golden_calc.txt` — 40 formules réelles évaluées (évaluateur Python ;
//!   17 scénarios étape 12a : synergies citées ligne, edmn/edns, ln/dm).
//! * `golden_dmg_curve.txt` — courbes de dégâts aux paliers 1/2/8/9/15/16/22/
//!   23/28/29/30 (formule itérative Python indépendante).
//! * `golden_isc.txt` — 359 records ItemStatCost (dictionnaire stats).
//! * `golden_skilldesc.txt` — 221 records skilldesc (arbre UI).

use crate::calc::CalcString;
use crate::skills_tree::{ElemType, SkillEvalCtx, SkillLevels, SkillTreeData};
use crate::stats::{split_tsv, StatCostTable};
use crate::xp::XpTable;

use std::collections::HashMap;

const GOLDEN_XP: &str = include_str!("../tests/golden/golden_xp.txt");
const GOLDEN_SKILLS: &str = include_str!("../tests/golden/golden_skills.txt");
const GOLDEN_CALC: &str = include_str!("../tests/golden/golden_calc.txt");
const GOLDEN_CURVE: &str = include_str!("../tests/golden/golden_dmg_curve.txt");
const GOLDEN_ISC: &str = include_str!("../tests/golden/golden_isc.txt");
const GOLDEN_SKILLDESC: &str = include_str!("../tests/golden/golden_skilldesc.txt");

fn data() -> &'static SkillTreeData {
    SkillTreeData::global()
}

// ---------------------------------------------------------------------------
// XP — chaque (classe, niveau) du golden vs la table parsée
// ---------------------------------------------------------------------------

#[test]
fn golden_xp_record_par_record() {
    let t = XpTable::global();
    let mut checked = 0;
    for line in GOLDEN_XP.lines().filter(|l| !l.is_empty() && !l.starts_with('#')) {
        let c = split_tsv(line);
        match c[0].as_str() {
            "MAXLVL" => {
                assert_eq!(t.class(&c[1]).unwrap().max_level.to_string(), c[2], "MaxLvl {c:?}");
            }
            code => {
                let lvl: u32 = c[1].parse().unwrap();
                let xp: u64 = c[2].parse().unwrap();
                let class = t.class(code).unwrap_or_else(|| panic!("classe {code} absente"));
                // Le golden capture la LIGNE BRUTE `lvl` du fichier (ligne N =
                // XP pour passer du niveau N au N+1) : comparer via `raw_row`,
                // l'accesseur documenté pour les goldens (`threshold` applique
                // le décalage niveau ⇄ ligne : niveau 1 gratuit = ligne 0).
                assert_eq!(class.raw_row(lvl as usize), xp, "{code} niv {lvl} : seuil");
                assert_eq!(t.exp_ratio(lvl).to_string(), c[3], "ExpRatio niv {lvl}");
            }
        }
        checked += 1;
    }
    assert_eq!(checked, 707, "99 niveaux × 7 classes + 7 MaxLvl");
}

#[test]
fn xp_99_coherent_sur_les_7_colonnes() {
    let t = XpTable::global();
    for code in t.class_codes() {
        let c = t.class(code).unwrap();
        // Croissance strictement croissante niveau 1→99.
        for lvl in 1..99 {
            assert!(c.threshold(lvl + 1) > c.threshold(lvl), "{code} niv {lvl}");
        }
        // Level-from-xp et xp_to_next cohérents sur tous les seuils.
        for lvl in 1..=99 {
            let xp = c.threshold(lvl);
            assert_eq!(c.level_for(xp), lvl, "{code} seuil niv {lvl}");
            if lvl < 99 {
                assert_eq!(c.xp_to_next(lvl, xp), c.threshold(lvl + 1) - xp);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Skills — 210 records × 82 champs, champ à champ
// ---------------------------------------------------------------------------

/// Champs modélisés, dans l'ordre du golden (généré depuis la même liste).
const FIELDS: &[&str] = &[
    "skill", "Id", "charclass", "skilldesc", "reqlevel", "maxlvl", "reqstr", "reqdex", "reqint",
    "reqvit", "reqskill1", "reqskill2", "reqskill3", "srvmissile", "srvmissilea", "srvmissileb",
    "srvmissilec", "ToHit", "LevToHit", "ToHitCalc", "HitShift", "SrcDam", "MinDam",
    "MinLevDam1", "MinLevDam2", "MinLevDam3", "MinLevDam4", "MinLevDam5", "MaxDam", "MaxLevDam1",
    "MaxLevDam2", "MaxLevDam3", "MaxLevDam4", "MaxLevDam5", "DmgSymPerCalc", "EType", "EMin",
    "EMinLev1", "EMinLev2", "EMinLev3", "EMinLev4", "EMinLev5", "EMax", "EMaxLev1", "EMaxLev2",
    "EMaxLev3", "EMaxLev4", "EMaxLev5", "EDmgSymPerCalc", "ELen", "ELevLen1", "ELevLen2",
    "ELevLen3", "ELenSymPerCalc", "delay", "mana", "lvlmana", "minmana", "startmana", "aurastate",
    "passivestate", "passivestat1", "passivecalc1", "passivestat2", "passivecalc2",
    "passivestat3", "passivecalc3", "Param1", "Param2", "Param3", "Param4", "Param5", "Param6",
    "Param7", "Param8", "calc1", "calc2", "calc3", "calc4", "prgcalc1", "prgcalc2", "prgcalc3",
];

/// Valeur brute du champ `name` du record (même convention que le golden :
/// cellule vide = chaîne vide).
fn skill_field(rec: &crate::skills_tree::SkillRecord, name: &str) -> String {
    let calc = |c: &Option<CalcString>| -> String {
        c.as_ref().map(|c| c.source().to_string()).unwrap_or_default()
    };
    let opt = |o: &Option<String>| -> String { o.clone().unwrap_or_default() };
    fn lev(n: &str, a: &[i64]) -> String {
        a[n.parse::<usize>().unwrap() - 1].to_string()
    }
    match name {
        "skill" => rec.code.clone(),
        "Id" => rec.id.to_string(),
        "charclass" => rec.class.clone(),
        "skilldesc" => rec.skilldesc.clone(),
        "reqlevel" => rec.req_level.to_string(),
        "maxlvl" => rec.max_level.to_string(),
        "reqstr" => rec.req_str.to_string(),
        "reqdex" => rec.req_dex.to_string(),
        "reqint" => rec.req_int.to_string(),
        "reqvit" => rec.req_vit.to_string(),
        "reqskill1" => rec.req_skills.first().cloned().unwrap_or_default(),
        "reqskill2" => rec.req_skills.get(1).cloned().unwrap_or_default(),
        "reqskill3" => rec.req_skills.get(2).cloned().unwrap_or_default(),
        "srvmissile" => opt(&rec.srv_missile),
        "srvmissilea" => opt(&rec.srv_missile_a),
        "srvmissileb" => opt(&rec.srv_missile_b),
        "srvmissilec" => opt(&rec.srv_missile_c),
        "ToHit" => rec.to_hit.to_string(),
        "LevToHit" => rec.lev_to_hit.to_string(),
        "ToHitCalc" => calc(&rec.to_hit_calc),
        "HitShift" => rec.hit_shift.to_string(),
        "SrcDam" => rec.src_dam.to_string(),
        "MinDam" => rec.min_dam.to_string(),
        "MaxDam" => rec.max_dam.to_string(),
        "EMin" => rec.e_min.to_string(),
        "EMax" => rec.e_max.to_string(),
        "ELen" => rec.e_len.to_string(),
        "delay" => rec.delay.to_string(),
        "mana" => rec.mana.to_string(),
        "lvlmana" => rec.lvl_mana.to_string(),
        "minmana" => rec.min_mana.to_string(),
        "startmana" => rec.start_mana.to_string(),
        "aurastate" => rec.aura_state.clone(),
        "passivestate" => rec.passive_state.clone(),
        "passivestat1" => rec.passive_stats[0].clone(),
        "passivestat2" => rec.passive_stats[1].clone(),
        "passivestat3" => rec.passive_stats[2].clone(),
        "EType" => etype_str(rec.e_type),
        "DmgSymPerCalc" => calc(&rec.dmg_sym_per_calc),
        "EDmgSymPerCalc" => calc(&rec.edmg_sym_per_calc),
        "ELenSymPerCalc" => calc(&rec.e_len_sym_per_calc),
        "passivecalc1" => calc(&rec.passive_calcs[0]),
        "passivecalc2" => calc(&rec.passive_calcs[1]),
        "passivecalc3" => calc(&rec.passive_calcs[2]),
        "calc1" => calc(&rec.calcs[0]),
        "calc2" => calc(&rec.calcs[1]),
        "calc3" => calc(&rec.calcs[2]),
        "calc4" => calc(&rec.calcs[3]),
        "prgcalc1" => calc(&rec.prg_calcs[0]),
        "prgcalc2" => calc(&rec.prg_calcs[1]),
        "prgcalc3" => calc(&rec.prg_calcs[2]),
        other => {
            if let Some(n) = other.strip_prefix("Param") {
                return rec.params[n.parse::<usize>().unwrap() - 1].to_string();
            }
            if let Some(n) = other.strip_prefix("MinLevDam") {
                return lev(n, &rec.min_lev_dam);
            }
            if let Some(n) = other.strip_prefix("MaxLevDam") {
                return lev(n, &rec.max_lev_dam);
            }
            if let Some(n) = other.strip_prefix("EMinLev") {
                return lev(n, &rec.e_min_lev);
            }
            if let Some(n) = other.strip_prefix("EMaxLev") {
                return lev(n, &rec.e_max_lev);
            }
            if let Some(n) = other.strip_prefix("ELevLen") {
                return lev(n, &rec.e_lev_len);
            }
            panic!("champ golden non mappé : {other}");
        }
    }
}

/// Code de colonne `EType` correspondant à l'enum.
fn etype_str(e: ElemType) -> String {
    match e {
        ElemType::None => "",
        ElemType::Fire => "fire",
        ElemType::Lightning => "ltng",
        ElemType::Cold => "cold",
        ElemType::Poison => "pois",
        ElemType::Magic => "mag",
        ElemType::Stun => "stun",
    }
    .to_string()
}

#[test]
fn golden_skills_210_records_champ_a_champ() {
    let mut lines = GOLDEN_SKILLS.lines();
    let header = split_tsv(lines.next().unwrap());
    assert_eq!(header, FIELDS, "l'entête golden et FIELDS doivent rester synchronisés");
    let mut count = 0;
    for line in lines {
        let row = split_tsv(line);
        let code = row[0].clone();
        let rec = data()
            .get(&code)
            .unwrap_or_else(|| panic!("skill {code} absent du parseur Rust"));
        for (i, name) in FIELDS.iter().enumerate() {
            let expected = row.get(i).cloned().unwrap_or_default();
            let actual = skill_field(rec, name);
            assert_eq!(actual, expected, "{code}.{name}");
        }
        count += 1;
    }
    assert_eq!(count, 210, "210 skills de classe");
}

// ---------------------------------------------------------------------------
// CalcString — formules réelles évaluées par le parseur Python indépendant
// ---------------------------------------------------------------------------

/// Niveaux de skills pour le contexte d'éval des formules golden.
struct Blvls(HashMap<String, u32>);

impl SkillLevels for Blvls {
    fn blvl(&self, code: &str) -> u32 {
        self.0.get(&code.to_ascii_lowercase()).copied().unwrap_or(0)
    }
}

#[test]
fn golden_calc_formules_reelles() {
    let mut checked = 0;
    for line in GOLDEN_CALC.lines().skip(1).filter(|l| !l.is_empty()) {
        let c = split_tsv(line);
        let (own, own_lvl, formula, blvls_raw, expected, note) =
            (&c[0], c[1].parse::<u32>().unwrap(), &c[2], &c[3], c[4].parse::<i64>().unwrap(), &c[5]);
        let rec = data().get(own).unwrap_or_else(|| panic!("skill {own} absent ({note})"));
        let mut map = HashMap::new();
        if !blvls_raw.is_empty() {
            for pair in blvls_raw.split(';') {
                let (k, v) = pair.split_once('=').unwrap();
                map.insert(k.to_ascii_lowercase(), v.parse::<u32>().unwrap());
            }
        }
        let calc = CalcString::parse(formula);
        assert!(calc.error().is_none(), "{note} : parse Rust échoué : {:?}", calc.error());
        let levels = Blvls(map);
        let mut ctx = SkillEvalCtx::new(rec, own_lvl, &levels);
        let got = calc.eval(&mut ctx);
        assert_eq!(got, expected, "{note} : {formula}");
        checked += 1;
    }
    assert_eq!(checked, 40, "40 scénarios de formules réelles (23 + 17 étape 12a)");
}

// ---------------------------------------------------------------------------
// Courbes de dégâts — formule itérative validée aux paliers
// ---------------------------------------------------------------------------

#[test]
fn golden_dmg_curves_aux_paliers() {
    let mut checked = 0;
    let mut skills = std::collections::BTreeSet::new();
    for line in GOLDEN_CURVE.lines().skip(1).filter(|l| !l.is_empty()) {
        let c = split_tsv(line);
        let rec = data().get(&c[0]).unwrap_or_else(|| panic!("skill {} absent", c[0]));
        let lvl: u32 = c[1].parse().unwrap();
        assert_eq!(
            data().damage_at(rec, lvl),
            (c[2].parse().unwrap(), c[3].parse().unwrap()),
            "{} niv {} : physiques",
            c[0],
            lvl
        );
        assert_eq!(
            data().elem_damage_at(rec, lvl),
            (c[4].parse().unwrap(), c[5].parse().unwrap()),
            "{} niv {} : élémentaires",
            c[0],
            lvl
        );
        assert_eq!(etype_str(rec.e_type), c[6], "{} : EType", c[0]);
        assert_eq!(data().elem_len_at(rec, lvl).to_string(), c[7], "{} niv {} : ELen", c[0], lvl);
        skills.insert(c[0].to_string());
        checked += 1;
    }
    assert_eq!(skills.len(), 85, "85 skills à courbe de dégâts");
    assert_eq!(checked, 935, "85 skills × 11 paliers");
}

// ---------------------------------------------------------------------------
// ItemStatCost — dictionnaire complet
// ---------------------------------------------------------------------------

#[test]
fn golden_itemstatcost_359_records() {
    let table = StatCostTable::global();
    assert_eq!(table.len(), 359);
    let mut count = 0;
    for line in GOLDEN_ISC.lines().skip(1).filter(|l| !l.is_empty()) {
        let c = split_tsv(line);
        let rec = table
            .get(&c[0])
            .unwrap_or_else(|| panic!("stat {} absente du dictionnaire", c[0]));
        let eq = |actual: &str, i: usize, what: &str| {
            assert_eq!(actual, c[i], "{}.{what}", c[0]);
        };
        eq(&rec.id.to_string(), 1, "ID");
        eq(&(rec.signed as i32).to_string(), 2, "Signed");
        eq(&rec.val_shift.to_string(), 3, "ValShift");
        eq(&rec.save_bits.to_string(), 4, "Save Bits");
        eq(&rec.save_add.to_string(), 5, "Save Add");
        eq(&rec.csv_bits.to_string(), 6, "CSvBits");
        eq(&rec.desc_priority.to_string(), 7, "descpriority");
        eq(&rec.desc_func.to_string(), 8, "descfunc");
        eq(&rec.desc_val.to_string(), 9, "descval");
        eq(&rec.desc_pos, 10, "descstrpos");
        eq(&rec.desc_neg, 11, "descstrneg");
        eq(&rec.desc_str2, 12, "descstr2");
        eq(&rec.dgrp.to_string(), 13, "dgrp");
        eq(&rec.dgrp_func.to_string(), 14, "dgrpfunc");
        eq(&rec.dgrp_val.to_string(), 15, "dgrpval");
        count += 1;
    }
    assert_eq!(count, 359);
    // Accès par id (saves binaires).
    assert_eq!(table.name_of_id(0), Some("strength"));
    // Accès par id (saves binaires).
    assert_eq!(table.name_of_id(0), Some("strength"));
}

// ---------------------------------------------------------------------------
// skilldesc — arbre UI
// ---------------------------------------------------------------------------

#[test]
fn golden_skilldesc_221_records() {
    let descs = SkillTreeData::parse_skilldesc(crate::skills_tree::SKILLDESC_TXT);    assert_eq!(descs.len(), 221);
    let mut count = 0;
    for line in GOLDEN_SKILLDESC.lines().skip(1).filter(|l| !l.is_empty()) {
        let c = split_tsv(line);
        let rec = descs
            .iter()
            .find(|d| d.code == c[0])
            .unwrap_or_else(|| panic!("skilldesc {} absent", c[0]));
        assert_eq!(rec.page.to_string(), c[1], "{}.page", c[0]);
        assert_eq!(rec.row.to_string(), c[2], "{}.row", c[0]);
        assert_eq!(rec.column.to_string(), c[3], "{}.column", c[0]);
        assert_eq!(rec.icon_cel.to_string(), c[4], "{}.icon", c[0]);
        assert_eq!(rec.str_name, c[5], "{}.str name", c[0]);
        assert_eq!(rec.str_short, c[6], "{}.str short", c[0]);
        assert_eq!(rec.str_long, c[7], "{}.str long", c[0]);
        count += 1;
    }
    assert_eq!(count, 221);
}

/// Le lien skills.txt → skilldesc.txt est complet pour les 210 skills.
#[test]
fn skilldesc_resolu_pour_les_210_skills() {
    let d = data();
    let mut count = 0;
    for rec in d.records().iter().filter(|r| r.is_class_skill()) {
        assert!(
            !rec.skilldesc.is_empty(),
            "{} : colonne skilldesc vide",
            rec.code
        );
        let desc = d.desc_of(&rec.code).unwrap_or_else(|| panic!("{} : pas de record skilldesc", rec.code));
        assert!(desc.page >= 1 && desc.page <= 3, "{} : page d'arbre invalide", rec.code);
        count += 1;
    }
    assert_eq!(count, 210);
}
