//! Golden tests PROGRESSION (étape 13a — finitions) : la chaîne
//! création de perso → Baal doit être fermée DONNÉES→MOTEUR sur les tables
//! réelles v1.12, pour les 3 difficultés.
//!
//! Méthode tbl_golden.rs : les tables sont relues ici par un parseur
//! INDÉPENDANT (split tabulaire brut) qui fait foi, et chaque sortie moteur
//! est comparée entrée par entrée :
//!
//! * `patch_experience.txt` (99 niveaux × 7 classes + MaxLvl + ExpRatio)
//!   vs [`d2rules::xp::XpTable`] — courbe complète 1→99 par classe ;
//! * `MonLvl.txt` (niveaux 0..110, colonnes AC/TH/HP/DM/XP ± `(N)`/`(H)`)
//!   vs le parser `d2rules::tsv::Tsv` (celui des modules de quêtes) ET vs
//!   l'arithmétique de scaling [`d2rules::World::spawn_monster_rule`] sur
//!   5 niveaux réels (zombie1 lvl 1, Andariel lvl 12, Duriel lvl 22,
//!   Diablo lvl 40, Baal lvl 60) × {Normal, Cauchemar, Enfer} ;
//! * boss scriptés : [`d2rules::quests_actes2::boss_stats`] (Duriel,
//!   Mephisto), [`d2rules::quests_actes45::boss_stats45`] (Izual, Hephasto,
//!   3 Anciens), [`d2rules::quests_finale::finale_stats`] (Diablo, Baal,
//!   5 vagues du trône) — stats + SpawnRule vs patch_monstats/MonLvl ;
//!   Andariel (N)/(H) passe par la MÊME convention SpawnRule (son spawn
//!   acte I vit côté d2game, cf. GAPS) ;
//! * TCs : toute TC de boss/superunique/vague spawnée existe dans
//!   `TreasureClassEx.txt`, avec ses variantes ` (N)`/` (H)` ; règle
//!   retail des variantes verrouillée sur les données (insérer après
//!   « Act N », sinon suffixer) ;
//! * lairs : chaque id de spawn scripté existe dans `patch_levels.txt`
//!   avec ses colonnes MonLvl1/2/3, MonDen ±(N)/(H), MonUMin/Max ±(N)/(H) ;
//! * `patch_difficultylevels.txt` vs [`d2rules::difficulty::embedded`].
//!
//! # GAPS de la 13a — état à la 13a-bis (reprise finale)
//!
//! **CORRIGÉS** (13a-bis, testés ci-dessous) :
//! 1. `world.rs` XP : l'ancien plafond MVP `MAX_LEVEL = 10` et la table
//!    `XP_TABLE` (colonne Barbarian copiée en dur) sont SUPPRIMÉS — le
//!    héros niveaute sur la table réelle 99×7 de `xp.rs` (`xp_threshold`/
//!    `max_level_of`, colonne de la classe), garde-fou
//!    `golden_hero_levels_to_99_on_real_table`.
//! 3. d2game `monsters.rs` : les TC normales NM/Enfer passent par
//!    `Difficulty::tc_variant` (insertion après « Act N ») — l'or normal
//!    ne tombe plus sur des TC absentes.
//! 4. Boss/superuniques NM/Enfer : `World::roll_drop` applique la VARIANTE
//!    retail de la TC au drop (données en main : « Duriel (N) », « Baal
//!    (H) »… existent), garde : variante absente ⇒ base (jamais pire).
//! 5. doc `tc_suffix` corrigée + `tc_variant` testée
//!    (`golden_tc_variant_retail_rule`).
//!
//! **RESTANTS** (signalés, hors périmètre 13a) :
//! 2. `SpawnRule::default()` duplique MonLvl 1 Normal — dorénavant
//!    verrouillé par `golden_spawn_default_is_monlvl_row1` (repli
//!    historique assumé).
//!
//! **FERMÉS à l'étape 21a** (2026-10-25, `src/fidelity_gaps.rs` + doc) :
//! 6. Andariel : API stats d2rules [`d2rules::quests_actes2::andariel_stats`]
//!    (convention `boss_stats`, rang superunique + TC dédiée) — golden
//!    `fidelity_gaps::andariel_stats_golden_real_tables` ; le scope
//!    `boss_stats("andariel") == None` reste verrouillé ci-dessous.
//! 7. Trou de la TABLE v1.12 « Fangskin (H) » : garde `has_tc` documentée
//!    (écart source, pas port) + tests bout-en-bout
//!    `fidelity_gaps::fangskin_hole_guarded_no_panic_fallback` (repli MVP,
//!    pas de panic) et `…fangskin_live_path_drops_real_chain` (le chemin
//!    vivant « Act 2 (H) Super B » dropne réel).
//! 8. Commentaires MonLvl 25/26 : corrigés aux vraies valeurs (100/470,
//!    DM 17) et VERROUILLÉS par `fidelity_gaps::monlvl_25_26_comment_claims`
//!    (les valeurs commentées doivent rester celles des APIs).

use d2rules::quests_actes2::{boss_stats, BOSSES_ACTS23};
use d2rules::quests_actes45::{boss_stats45, BOSSES_ACTS45};
use d2rules::quests_finale::{finale_stats, FINALE_RECORDS};
use d2rules::{difficulty, xp, GameEvent, MonKind, PlayerInput, SpawnRule, World};

use serde_json::json;

// ---------------------------------------------------------------------------
// Tables réelles (mêmes fichiers que le runtime — lecture seule)
// ---------------------------------------------------------------------------

const XP_TXT: &str = include_str!("../../../../assets/patch_experience.txt");
const MONLVL_TXT: &str = include_str!("../../../../assets/MonLvl.txt");
const MONSTATS_TXT: &str = include_str!("../../../../assets/patch_monstats.txt");
const LEVELS_TXT: &str = include_str!("../../../../assets/patch_levels.txt");
const TCE_TXT: &str = include_str!("../../../../assets/TreasureClassEx.txt");
const DIFFLEVELS_TXT: &str = include_str!("../../../../assets/patch_difficultylevels.txt");
const SUPERUNIQUES_TXT: &str = include_str!("../../../../assets/superuniques.txt");

// ---------------------------------------------------------------------------
// Parseur de référence INDÉPENDANT (split tabulaire brut, sans d2rules::tsv)
// ---------------------------------------------------------------------------

/// TSV brut → (en-tête, lignes de cellules).
fn raw_tsv(txt: &str) -> (Vec<String>, Vec<Vec<String>>) {
    let mut lines = txt.lines().filter(|l| !l.trim().is_empty());
    let header: Vec<String> = lines
        .next()
        .expect("en-tête présent")
        .split('\t')
        .map(|c| c.trim().to_string())
        .collect();
    let rows = lines
        .map(|l| l.split('\t').map(|c| c.trim().to_string()).collect())
        .collect();
    (header, rows)
}

fn cell_u(row: &[String], i: usize) -> u32 {
    row.get(i).and_then(|c| c.trim().parse().ok()).unwrap_or(0)
}

fn cell_i(row: &[String], i: usize) -> i32 {
    row.get(i).and_then(|c| c.trim().parse().ok()).unwrap_or(0)
}

/// Colonne d'une table brute (panic assisté : le golden doit dire quoi).
fn col_of(header: &[String], name: &str) -> usize {
    header
        .iter()
        .position(|h| h.eq_ignore_ascii_case(name))
        .unwrap_or_else(|| panic!("colonne {name} absente de la table"))
}

// ---------------------------------------------------------------------------
// 1. XP 1→99 — les 7 classes vs patch_experience.txt
// ---------------------------------------------------------------------------

const CLASSES: [(&str, &str); 7] = [
    ("ama", "Amazon"),
    ("sor", "Sorceress"),
    ("nec", "Necromancer"),
    ("pal", "Paladin"),
    ("bar", "Barbarian"),
    ("dru", "Druid"),
    ("ass", "Assassin"),
];

#[test]
fn golden_xp_full_curve_1_to_99_all_7_classes() {
    let (header, rows) = raw_tsv(XP_TXT);
    let table = xp::XpTable::global();
    assert_eq!(header[0], "Level", "en-tête patch_experience.txt");
    // MaxLvl + 100 lignes de niveaux (0..=99).
    assert_eq!(rows.len(), 101, "MaxLvl + niveaux 0..=99");
    let maxlvl_row = &rows[0];
    let level_rows = &rows[1..];

    for (code, column) in CLASSES {
        let ci = col_of(&header, column);
        let cx = table.class(code).unwrap_or_else(|| panic!("classe {code}"));
        // MaxLvl = 99 pour les 7 classes (ligne MaxLvl de la table).
        assert_eq!(cx.max_level, cell_u(maxlvl_row, ci), "{code} : MaxLvl");
        // Chacune des 100 lignes 0..=99, valeur pour valeur.
        for (row, raw) in level_rows.iter().enumerate() {
            assert_eq!(
                cx.raw_row(row),
                cell_u(raw, ci) as u64,
                "{code} : ligne {row} (seuil niveau {})",
                row + 1
            );
        }
        // Sémantique niveau ⇄ seuil sur TOUTE la courbe : threshold(lvl) =
        // la ligne lvl-1 (la ligne N porte l'XP cumulé pour passer N → N+1).
        assert_eq!(cx.threshold(0), 0, "{code} : niveau 0 ⇒ 0");
        assert_eq!(cx.threshold(1), 0, "{code} : le niveau 1 est gratuit");
        for lvl in 2..=99u32 {
            let t = cx.threshold(lvl);
            assert_eq!(cx.level_for(t), lvl, "{code} : {t} XP ⇒ niveau {lvl}");
            if t > 0 {
                assert_eq!(
                    cx.level_for(t - 1),
                    lvl - 1,
                    "{code} : {} XP ⇒ encore niveau {}",
                    t - 1,
                    lvl - 1
                );
            }
            if lvl < 99 {
                assert_eq!(
                    cx.xp_to_next(lvl, t),
                    cx.threshold(lvl + 1) - t,
                    "{code} : reste pour {lvl}→{}",
                    lvl + 1
                );
            }
        }
        assert_eq!(cx.xp_to_next(99, u64::MAX), 0, "{code} : cap niveau 99");
        assert_eq!(cx.level_for(u64::MAX), 99, "{code} : plafond MaxLvl");
    }
    assert_eq!(table.class_codes().len(), 7, "7 classes en colonne");
}

#[test]
fn golden_xp_landmarks_and_exp_ratio() {
    let t = xp::XpTable::global();
    // Repères historiques (ligne N = XP pour passer N → N+1) — Barbarian.
    let bar = t.class("bar").unwrap();
    for (lvl, want) in [
        (2, 500),
        (3, 1500),
        (4, 3750),
        (10, 57715),
        (11, 72144),
        (50, 47_116_709),
        (99, 3_520_485_254),
    ] {
        assert_eq!(bar.threshold(lvl), want, "seuil niveau {lvl}");
    }
    assert_eq!(bar.raw_row(99), 3_837_739_017, "ligne 99 (inatteignable)");
    // Les 7 courbes sont identiques en v1.12 MAIS chacune lue depuis SA
    // colonne : si une colonne divergeait, golden_xp_full_curve_… le voit ;
    // ici on verrouille le fait de données (utile au MVP world.rs).
    let curves: Vec<Vec<u64>> = CLASSES
        .iter()
        .map(|(c, _)| (0..=99).map(|r| t.class(c).unwrap().raw_row(r)).collect())
        .collect();
    assert!(curves.windows(2).all(|w| w[0] == w[1]), "courbes v1.12 identiques");

    // ExpRatio : colonne partagée, 1024 (ligne 0) → 5 (ligne 99).
    let (header, rows) = raw_tsv(XP_TXT);
    let er = col_of(&header, "ExpRatio");
    for (row, raw) in rows[1..].iter().enumerate() {
        assert_eq!(t.exp_ratio(row as u32), cell_u(raw, er), "ExpRatio ligne {row}");
    }

    // Le héros World (MVP) démarre aligné sur la table réelle : 500 XP
    // pour le niveau 2 (XP_TABLE[0] de world.rs = ligne 1 de la table).
    let w = test_world(json!({ "monsters": [] }));
    assert_eq!(w.hero_stats().xp_to_next, 500, "World MVP aligné sur patch_experience");
}

// ---------------------------------------------------------------------------
// 2. MonLvl — parser moteur (tsv) + arithmétique de scaling N/H du World
// ---------------------------------------------------------------------------

/// Une ligne MonLvl complète : AC/TH/HP/DM/XP pour Normal/(N)/(H).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct MonlvlRow {
    normal: [u32; 5], // AC TH HP DM XP
    nm: [u32; 5],
    hell: [u32; 5],
}

fn monlvl_table() -> std::collections::HashMap<u32, MonlvlRow> {
    let (header, rows) = raw_tsv(MONLVL_TXT);
    let c = |n: &str| col_of(&header, n);
    let cols = [
        [c("AC"), c("TH"), c("HP"), c("DM"), c("XP")],
        [c("AC(N)"), c("TH(N)"), c("HP(N)"), c("DM(N)"), c("XP(N)")],
        [c("AC(H)"), c("TH(H)"), c("HP(H)"), c("DM(H)"), c("XP(H)")],
    ];
    let mut m = std::collections::HashMap::new();
    for r in &rows {
        let lvl = cell_u(r, c("Level"));
        let pick = |set: [usize; 5]| set.map(|i| cell_u(r, i));
        m.insert(lvl, MonlvlRow { normal: pick(cols[0]), nm: pick(cols[1]), hell: pick(cols[2]) });
    }
    m
}

/// La difficulté indexe la ligne : 0 = Normal, 1 = (N), 2 = (H).
fn row_for(row: &MonlvlRow, diff: usize) -> [u32; 5] {
    [row.normal, row.nm, row.hell][diff]
}

#[test]
fn golden_monlvl_engine_parser_matches_raw() {
    // Le parser du runtime (d2rules::tsv, utilisé par les modules de quêtes)
    // doit lire EXACTEMENT la table brute, 111 lignes × 15 colonnes utiles.
    let t = d2rules::tsv::Tsv::parse(MONLVL_TXT).expect("MonLvl.txt parse");
    let (header, rows) = raw_tsv(MONLVL_TXT);
    assert_eq!(t.len(), rows.len(), "111 lignes de niveaux 0..110");
    let cols = [
        "AC", "TH", "HP", "DM", "XP", "AC(N)", "TH(N)", "HP(N)", "DM(N)", "XP(N)", "AC(H)",
        "TH(H)", "HP(H)", "DM(H)", "XP(H)",
    ];
    for i in 0..rows.len() {
        let rec = t.rec(i).unwrap_or_else(|| panic!("ligne {i}"));
        assert_eq!(rec.u("Level"), cell_u(&rows[i], col_of(&header, "Level")), "Level ligne {i}");
        for c in cols {
            assert_eq!(rec.u(c), cell_u(&rows[i], col_of(&header, c)), "{c} ligne {i}");
        }
    }
}

#[test]
fn golden_spawn_default_is_monlvl_row1() {
    // Le SpawnRule::default() « repli historique » de world.rs duplique la
    // ligne MonLvl 1 Normal — on verrouille la duplication sur la table.
    let ml = monlvl_table();
    let d = SpawnRule::default();
    assert_eq!(d.ac, ml[&1].normal[0], "AC MonLvl 1");
    assert_eq!(d.th_pct, ml[&1].normal[1], "TH MonLvl 1");
    assert_eq!(d.hp_pct, ml[&1].normal[2], "HP MonLvl 1");
    assert_eq!(d.dmg_pct, ml[&1].normal[3], "DM MonLvl 1");
    assert_eq!(d.xp_pct, ml[&1].normal[4], "XP MonLvl 1");
}

/// Record brut patch_monstats d'un monstre : les valeurs qui nourrissent le
/// scaling (code sprite, niveaux par difficulté, PV/AC/DM/XP bruts, TC, résists).
#[derive(Clone)]
struct MonRec {
    code: String,
    level: [u32; 3],    // Level, Level(N), Level(H)
    hp: u32,            // minHP (= maxHP pour les boss)
    ac: u32,            // AC brut monstats
    dmg: [u32; 2],      // A1MinD, A1MaxD
    exp: u32,           // Exp
    tc: String,         // TreasureClass1
    res: [[i32; 6]; 3], // ResDm/Ma/Fi/Li/Co/Po × Normal/(N)/(H)
}

fn monstats_table() -> std::collections::HashMap<String, MonRec> {
    let (header, rows) = raw_tsv(MONSTATS_TXT);
    let c = |n: &str| col_of(&header, n);
    let res_cols = [
        ["ResDm", "ResMa", "ResFi", "ResLi", "ResCo", "ResPo"],
        ["ResDm(N)", "ResMa(N)", "ResFi(N)", "ResLi(N)", "ResCo(N)", "ResPo(N)"],
        ["ResDm(H)", "ResMa(H)", "ResFi(H)", "ResLi(H)", "ResCo(H)", "ResPo(H)"],
    ];
    let mut m = std::collections::HashMap::new();
    for r in &rows {
        let id = r[c("Id")].to_ascii_lowercase();
        if id.is_empty() {
            continue;
        }
        m.insert(
            id,
            MonRec {
                code: r[c("Code")].clone(),
                level: [cell_u(r, c("Level")), cell_u(r, c("Level(N)")), cell_u(r, c("Level(H)"))],
                hp: cell_u(r, c("minHP")),
                ac: cell_u(r, c("AC")),
                dmg: [cell_u(r, c("A1MinD")), cell_u(r, c("A1MaxD"))],
                exp: cell_u(r, c("Exp")),
                tc: r[c("TreasureClass1")].clone(),
                res: res_cols.map(|set| set.map(|n| cell_i(r, c(n)))),
            },
        );
    }
    m
}

/// Gabarit monstre de test : valeurs BRUTES d'une ligne patch_monstats
/// (mêmes colonnes que le manifest démo `monsters[]`/`bosses[]`).
fn monster_manifest(rec: &MonRec) -> serde_json::Value {
    json!({
        "monsters": [{
            "code": rec.code,
            "name": rec.code,
            "stats": {
                "level": rec.level[0],
                "hpMin": rec.hp,
                "hp": rec.hp,
                "dmgMin": rec.dmg[0],
                "dmgMax": rec.dmg[1],
                "exp": rec.exp,
                "treasureClass": rec.tc,
            }
        }]
    })
}

/// Grille toute marchable (`false` = marchable, contrat save).
fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
    vec![vec![false; w]; h]
}

fn test_world(manifest: serde_json::Value) -> World {
    World::with_seed(open_grid(24, 16), manifest, 7)
}

/// Stats effectives du monstre 0 lues dans le contrat save (champs
/// `hp/max_hp/dmg_min/dmg_max/th/ac/xp/res` de MonSave).
fn saved_monster(w: &World) -> serde_json::Value {
    let s: serde_json::Value = serde_json::from_str(&w.save()).expect("save JSON");
    s["monsters"][0].clone()
}

/// Golden de scaling : monstre réel × difficulté réelle. La SpawnRule est
/// construite depuis les colonnes MonLvl de la difficulté (exactement la
/// convention d2game/monsters.rs que d2rules applique), le World spawnne,
/// et les stats effectives (save) doivent égaliser table × gabarit brut.
fn assert_scaling(rec: &MonRec, ml: &std::collections::HashMap<u32, MonlvlRow>, diff: usize) {
    let lvl = rec.level[diff];
    let row = ml.get(&lvl).unwrap_or_else(|| panic!("MonLvl ligne {lvl} absente"));
    let f = row_for(row, diff); // AC TH HP DM XP
    let rule = SpawnRule {
        kind: MonKind::Boss,
        hp_pct: f[2],
        dmg_pct: f[3],
        th_pct: f[1],
        xp_pct: f[4],
        ac: f[0],
        treasure_class: Some(rec.tc.clone()),
        res: rec.res[diff],
    };
    let mut w = test_world(monster_manifest(rec));
    w.clear_monsters();
    assert!(w.spawn_monster_rule(&rec.code, 3, 3, rule), "spawn {}", rec.code);
    let s = saved_monster(&w);
    // PV : brut × HP%/100 (hpMin == hpMax ⇒ déterministe) ; sol 1.
    let want_hp = (rec.hp.saturating_mul(f[2]) / 100).max(1);
    assert_eq!(
        s["hp"].as_u64(),
        Some(want_hp as u64),
        "{} diff {diff} : HP = {}×{}/100",
        rec.code,
        rec.hp,
        f[2]
    );
    assert_eq!(s["max_hp"].as_u64(), Some(want_hp as u64));
    // Dégâts : brut × DM%/100, sol 1.
    assert_eq!(
        s["dmg_min"].as_u64(),
        Some((rec.dmg[0].saturating_mul(f[3]) / 100).max(1) as u64),
        "{} diff {diff} : DM min",
        rec.code
    );
    assert_eq!(
        s["dmg_max"].as_u64(),
        Some((rec.dmg[1].saturating_mul(f[3]) / 100).max(1) as u64),
        "{} diff {diff} : DM max",
        rec.code
    );
    // Toucher/AC : colonnes directes de la ligne (pas un %).
    assert_eq!(s["th"].as_u64(), Some(f[1] as u64), "{} diff {diff} : TH direct", rec.code);
    assert_eq!(s["ac"].as_u64(), Some(f[0] as u64), "{} diff {diff} : AC directe", rec.code);
    // XP : brut × XP%/100 (crédité au héros, cf. golden_xp_credit_end_to_end).
    assert_eq!(
        s["xp"].as_u64(),
        Some((rec.exp.saturating_mul(f[4]) / 100) as u64),
        "{} diff {diff} : XP = {}×{}/100",
        rec.code,
        rec.exp,
        f[4]
    );
    // Résists : passe-plat exact de la difficulté du spawn.
    let want_res: Vec<i64> = rec.res[diff].iter().map(|&r| r as i64).collect();
    let got_res: Vec<i64> = s["res"].as_array().expect("res").iter().map(|v| v.as_i64().unwrap()).collect();
    assert_eq!(got_res, want_res, "{} diff {diff} : résists", rec.code);
}

/// Les 5 niveaux réels du golden (mission 13a-1) : lvl 1 (zombie), 12
/// (Andariel), 22 (Duriel), 40 (Diablo), 60 (Baal) — colonnes brutes
/// patch_monstats, gabarit = valeurs d'origine (pas de constantes).
fn progression_cast() -> Vec<MonRec> {
    let ms = monstats_table();
    ["zombie1", "andariel", "duriel", "diablo", "baalcrab"]
        .iter()
        .map(|id| ms[*id].clone())
        .collect()
}

#[test]
fn golden_monster_scaling_N_H_5_real_levels() {
    let ml = monlvl_table();
    for rec in &progression_cast() {
        for diff in 0..3 {
            assert_scaling(rec, &ml, diff);
        }
    }
}

#[test]
fn golden_andariel_normal_vs_nightmare_vs_hell() {
    // Repères littéraux (fichier:ligne:colonne) — au-delà de l'arithmétique
    // générique, les valeurs d'Andariel sont posées noir sur blanc :
    // patch_monstats `andariel` (Level 12/49/75, minHP 2562) × les lignes
    // MonLvl de SON niveau par difficulté — Normal 12 (AC 72, TH 111,
    // HP 40, DM 8, XP 140), Cauchemar 49 (N) (294/518/299/31/2213 bruts,
    // (N) 581/972/968/41/6186), Enfer 75 (H) (450/804/605/46/4917 bruts,
    // (H) 1254/3028/3774/85/37605).
    let ml = monlvl_table();
    let ms = monstats_table();
    let a = &ms["andariel"];
    assert_eq!(a.level, [12, 49, 75], "andariel Level/(N)/(H)");
    assert_eq!(a.hp, 2562, "andariel minHP");
    let want = [
        // (HP effectifs, AC, TH) — Normal (MonLvl 12), Cauchemar (MonLvl 49
        // colonnes (N)), Enfer (MonLvl 75 colonnes (H)) — la ligne suit le
        // Level(N)/(H) du monstre, convention d2game level_for(diff).
        (2562 * 40 / 100, 72, 111),
        (2562 * 968 / 100, 581, 972),
        (2562 * 3774 / 100, 1254, 3028),
    ];
    for (diff, (hp, ac, th)) in want.iter().enumerate() {
        let f = row_for(&ml[&a.level[diff]], diff);
        assert_eq!(f[2], [40, 968, 3774][diff], "MonLvl HP andariel diff {diff}");
        let mut w = test_world(monster_manifest(a));
        w.clear_monsters();
        assert!(w.spawn_monster_rule(
            &a.code,
            3,
            3,
            SpawnRule {
                kind: MonKind::Boss,
                hp_pct: f[2],
                dmg_pct: f[3],
                th_pct: f[1],
                xp_pct: f[4],
                ac: f[0],
                treasure_class: Some(a.tc.clone()),
                res: a.res[diff],
            },
        ));
        let s = saved_monster(&w);
        assert_eq!(s["hp"].as_u64(), Some(*hp as u64), "andariel HP diff {diff}");
        assert_eq!(s["ac"].as_u64(), Some(*ac as u64), "andariel AC diff {diff}");
        assert_eq!(s["th"].as_u64(), Some(*th as u64), "andariel TH diff {diff}");
    }
    // Croissance stricte N << H sur les 5 niveaux réels (le cœur du
    // contrat « Hell difficile ») — PV effectifs et AC.
    for rec in &progression_cast() {
        let hps: Vec<u64> = (0..3)
            .map(|d| {
                let f = row_for(&ml[&rec.level[d]], d);
                (rec.hp.saturating_mul(f[2]) / 100).max(1) as u64
            })
            .collect();
        assert!(hps[0] < hps[1] && hps[1] < hps[2], "{} : HP N/H croissants {hps:?}", rec.code);
        let acs = [
            ml[&rec.level[0]].normal[0],
            ml[&rec.level[1]].nm[0],
            ml[&rec.level[2]].hell[0],
        ];
        assert!(acs[0] < acs[1] && acs[1] < acs[2], "{} : AC N/H croissantes {acs:?}", rec.code);
    }
    // Et l'immunité réelle en Enfer : zombie1 ResCo(H) = 120 ⇒ froid 0.
    assert_eq!(difficulty::apply_resist(999, ms["zombie1"].res[2][4]), 0, "zombie1 immunisé froid en Enfer");
    assert_eq!(difficulty::apply_resist(100, ms["zombie1"].res[0][5]), 50, "zombie1 ResPo Normal 50");
}

#[test]
fn golden_xp_credit_end_to_end_monlvl_row1() {
    // Chaîne complète données → event : un zombie (patch_monstats zombie1,
    // Exp 111) spawné au repli MonLvl 1 Normal (XP ×30/100) crédite
    // EXACTEMENT 33 XP au héros via MonsterDied.
    let ms = monstats_table();
    let z = &ms["zombie1"];
    let mut w = test_world(monster_manifest(z));
    w.clear_monsters();
    // Spawn au contact du héros (clic-attaque = cible la plus proche).
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    let mut ok = false;
    for (x, y) in [(hx + 2, hy), (hx - 2, hy), (hx, hy + 2), (hx, hy - 2), (hx + 1, hy)] {
        if w.is_walkable(x, y) && w.spawn_monster_rule(&z.code, x, y, SpawnRule::default()) {
            ok = true;
            break;
        }
    }
    assert!(ok, "zombie spawné près du héros");
    let want_xp = z.exp * SpawnRule::default().xp_pct / 100;
    assert_eq!(want_xp, 33, "111 × 30/100");
    let mut died_tick = None;
    for t in 0..400 {
        let target = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
        let input = target.map(|(x, y)| PlayerInput::Attack { x, y });
        for ev in w.tick(50, input) {
            if matches!(ev, GameEvent::MonsterDied { .. }) {
                died_tick = Some(t);
            }
        }
        if let Some(t0) = died_tick {
            if t > t0 + 3 {
                break; // crédit XP du tick de mort acquis
            }
        }
    }
    assert!(died_tick.is_some(), "zombie tué en ~20 s de sim");
    assert_eq!(w.hero_stats().xp, want_xp, "XP créditée = Exp × MonLvl.XP/100");
}

// ---------------------------------------------------------------------------
// 3. Boss scriptés — stats réelles vs tables (Normal), tous modules
// ---------------------------------------------------------------------------

#[test]
fn golden_boss_stats_acts23_match_tables() {
    let ml = monlvl_table();
    let ms = monstats_table();
    assert_eq!(BOSSES_ACTS23.len(), 2, "Duriel + Mephisto");
    for rec in BOSSES_ACTS23 {
        let st = boss_stats(rec.id).unwrap_or_else(|| panic!("boss_stats {}", rec.id));
        let m = &ms[rec.id];
        assert_eq!(st.rec, rec);
        assert_eq!(st.level, m.level[0], "{} : Level monstats", rec.id);
        assert_eq!(st.hp, m.hp, "{} : minHP monstats", rec.id);
        assert_eq!(st.exp, m.exp, "{} : Exp monstats", rec.id);
        assert_eq!(st.res, m.res[0], "{} : résists Normal", rec.id);
        let f = row_for(&ml[&m.level[0]], 0);
        assert_eq!(st.rule.hp_pct, f[2], "{} : HP% = MonLvl {}", rec.id, m.level[0]);
        assert_eq!(st.rule.dmg_pct, f[3]);
        assert_eq!(st.rule.th_pct, f[1]);
        assert_eq!(st.rule.xp_pct, f[4]);
        assert_eq!(st.rule.ac, f[0]);
        assert_eq!(
            st.rule.treasure_class.as_deref(),
            Some(m.tc.as_str()),
            "{} : TC1 monstats",
            rec.id
        );
    }
    // Repères du brief de livraison : Duriel (MonLvl 22 : AC 132, TH 221,
    // HP 84) et Mephisto (MonLvl 26 : AC 156, TH 265, HP 106).
    assert_eq!(row_for(&ml[&22], 0), [132, 221, 84, 14, 363], "MonLvl 22");
    assert_eq!(row_for(&ml[&26], 0), [156, 265, 106, 17, 503], "MonLvl 26");
}

#[test]
fn golden_boss_stats_acts45_match_tables() {
    let ml = monlvl_table();
    let ms = monstats_table();
    assert_eq!(BOSSES_ACTS45.len(), 5, "Izual + Hephasto + 3 Anciens");
    for rec in BOSSES_ACTS45 {
        let st = boss_stats45(rec.id).unwrap_or_else(|| panic!("boss_stats45 {}", rec.id));
        let m = &ms[rec.id];
        assert_eq!(st.level, m.level[0]);
        assert_eq!(st.hp, m.hp);
        assert_eq!(st.exp, m.exp, "{} : Exp monstats (0 pour les Anciens)", rec.id);
        let f = row_for(&ml[&m.level[0]], 0);
        assert_eq!(st.rule.hp_pct, f[2], "{} : HP% MonLvl {}", rec.id, m.level[0]);
        assert_eq!(st.rule.ac, f[0]);
        assert_eq!(st.rule.th_pct, f[1]);
        assert_eq!(st.rule.dmg_pct, f[3]);
        assert_eq!(st.rule.xp_pct, f[4]);
        // TC : TreasureClass1 monstats, sinon vide (Anciens) ⇒ None.
        assert_eq!(
            st.rule.treasure_class,
            (!m.tc.is_empty()).then(|| m.tc.clone()),
            "{} : TC effective",
            rec.id
        );
    }
    // Repères MonLvl des boss 4-5 (Izual 29, Hephasto 25, Anciens 37 N).
    assert_eq!(row_for(&ml[&29], 0), [174, 298, 126, 19, 616], "MonLvl 29");
    assert_eq!(row_for(&ml[&25], 0), [150, 254, 100, 16, 470], "MonLvl 25");
}

#[test]
fn golden_finale_stats_match_tables() {
    let ml = monlvl_table();
    let ms = monstats_table();
    let su = superuniques_by_class();
    let tce = treasure_classes();
    assert_eq!(FINALE_RECORDS.len(), 12, "a4 + Anciens + 5 vagues + Baal");
    for rec in FINALE_RECORDS {
        let st = finale_stats(rec.id).unwrap_or_else(|| panic!("finale_stats {}", rec.id));
        let m = &ms[rec.id];
        assert_eq!(st.level, m.level[0], "{} : Level", rec.id);
        assert_eq!(st.hp, m.hp, "{} : minHP", rec.id);
        let f = row_for(&ml[&m.level[0]], 0);
        assert_eq!(st.rule.hp_pct, f[2], "{} : HP% MonLvl {}", rec.id, m.level[0]);
        assert_eq!(st.rule.ac, f[0]);
        assert_eq!(st.rule.th_pct, f[1]);
        assert_eq!(st.rule.dmg_pct, f[3]);
        assert_eq!(st.rule.xp_pct, f[4]);
        // TC effective : superunique (col TC) > monstats TC1 > repli > none.
        let su_tc = su.get(rec.id).map(|r| r.1.clone()).unwrap_or_default();
        let want_tc = if !su_tc.is_empty() {
            Some(su_tc)
        } else if !m.tc.is_empty() {
            Some(m.tc.clone())
        } else {
            None
        };
        assert_eq!(st.tc_effective, want_tc, "{} : TC effective", rec.id);
        assert_eq!(st.rule.treasure_class, st.tc_effective);
        // Toute TC effective existe dans TreasureClassEx (drop jouable).
        if let Some(tc) = &st.tc_effective {
            assert!(tce.contains(tc), "TC « {tc} » ({}) absente de TreasureClassEx", rec.id);
        }
    }
    // Vagues du trône : TC superunique « Act 5 Champ C » (superuniques.txt).
    for w in 0..5 {
        let rec = &FINALE_RECORDS[6 + w];
        assert_eq!(rec.wave, Some(w + 1));
        assert_eq!(
            finale_stats(rec.id).unwrap().tc_effective.as_deref(),
            Some("Act 5 Champ C")
        );
    }
    // Anciens : pas de drop dédié (TC1 et TC superuniques vides).
    for id in ["ancientbarb1", "ancientbarb2", "ancientbarb3"] {
        assert_eq!(finale_stats(id).unwrap().tc_effective, None, "{id} : pas de TC");
    }
    // Repères de niveaux (MonLvl N des boss) : Diablo 40, Baal 60.
    assert_eq!(row_for(&ml[&40], 0), [240, 419, 215, 25, 1297], "MonLvl 40 (Diablo)");
    assert_eq!(row_for(&ml[&60], 0), [360, 639, 418, 37, 3357], "MonLvl 60 (Baal)");
}

// ---------------------------------------------------------------------------
// 4. TCs (N)/(H) — tout ce qui spawn droppe depuis une TC qui existe
// ---------------------------------------------------------------------------

fn treasure_classes() -> std::collections::HashSet<String> {
    raw_tsv(TCE_TXT)
        .1
        .iter()
        .filter_map(|r| r.first().filter(|c| !c.is_empty()).cloned())
        .collect()
}

/// Variante retail d'une TC pour une difficulté : les TC « Act N … »
/// insèrent ` (N)`/` (H)` APRÈS le préfixe d'acte (« Act 1 H2H A » →
/// « Act 1 (N) H2H A »), toute autre la suffixe (« Andariel » →
/// « Andariel (N) »). Verrouillé ci-dessous sur 100 % des TC monstats.
fn tc_variant(tc: &str, suffix: &str) -> String {
    if let Some(rest) = tc.strip_prefix("Act ") {
        if let Some(sp) = rest.find(' ') {
            return format!("Act {}{}{}", &rest[..sp], suffix, &rest[sp..]);
        }
    }
    format!("{tc}{suffix}")
}

/// (Superunique, TC, TC(N), TC(H)) de superuniques.txt, indexés par la
/// colonne Class (= id monstats du port).
fn superuniques_by_class() -> std::collections::HashMap<String, (String, String, String, String)> {
    let (header, rows) = raw_tsv(SUPERUNIQUES_TXT);
    let c = |n: &str| col_of(&header, n);
    let mut m = std::collections::HashMap::new();
    for r in &rows {
        let class = r[c("Class")].to_ascii_lowercase();
        if class.is_empty() {
            continue;
        }
        m.insert(
            class,
            (
                r[c("Superunique")].clone(),
                r[c("TC")].clone(),
                r[c("TC(N)")].clone(),
                r[c("TC(H)")].clone(),
            ),
        );
    }
    m
}

#[test]
fn golden_all_spawned_tcs_exist_with_N_H_variants() {
    let tce = treasure_classes();
    let ms = monstats_table();

    // (a) Boss scriptés : TC1 monstats — base + variantes nommées ` (N) (H)`.
    let boss_ids = ["andariel", "duriel", "mephisto", "izual", "hephasto", "diablo", "baalcrab"];
    for id in boss_ids {
        let tc = ms[id].tc.clone();
        assert!(!tc.is_empty(), "{id} : TC1 présente");
        assert!(tce.contains(&tc), "TC « {tc} » ({id})");
        for sfx in [" (N)", " (H)"] {
            let v = format!("{tc}{sfx}");
            assert!(tce.contains(&v), "TC « {v} » ({id}) : variante difficile absente");
        }
    }

    // (b) TOUTES les TC de monstres de patch_monstats : la base existe ET
    // la variante retail (règle acte-insérée / suffixée) existe, N et H.
    let mut seen = std::collections::HashSet::new();
    for rec in ms.values() {
        if rec.tc.is_empty() || !seen.insert(rec.tc.clone()) {
            continue;
        }
        assert!(tce.contains(&rec.tc), "TC « {} » monstats", rec.tc);
        for sfx in [" (N)", " (H)"] {
            let v = tc_variant(&rec.tc, sfx);
            assert!(tce.contains(&v), "TC « {v} » : variante de « {} » absente", rec.tc);
        }
    }
    assert!(seen.len() >= 60, "couverture TC monstats ({})", seen.len());

    // (c) La règle retail de nommage est BINAIRE sur les données : la forme
    // suffixée « Act 1 H2H A (N) » n'existe PAS (piège documenté GAPS-3).
    assert!(!tce.contains("Act 1 H2H A (N)"), "forme suffixée act : absente de la table");
    assert!(tce.contains("Act 1 (N) H2H A"), "forme insérée act : présente");

    // (d) Superuniques (data-driven d2game) : colonnes TC/TC(N)/TC(H)
    // existent dans TreasureClassEx — SAUF le trou de la table v1.12
    // elle-même : « Fangskin (H) » référencé mais absent (consigné).
    let su = superuniques_by_class();
    let mut checked = 0;
    let mut holes: Vec<String> = Vec::new();
    for (_name, tc, tc_n, tc_h) in su.values() {
        for tc in [tc, tc_n, tc_h] {
            if tc.is_empty() {
                continue;
            }
            checked += 1;
            if !tce.contains(tc) {
                holes.push(tc.clone());
            }
        }
    }
    assert_eq!(checked, 177, "TC superuniques vérifiées : {checked}");
    assert_eq!(holes, vec!["Fangskin (H)".to_string()], "trou de table v1.12 attendu (GAPS)");
}

// ---------------------------------------------------------------------------
// 5. Lairs scriptés — ids vs patch_levels.txt (MonLvl1/2/3, MonDen, MonU)
// ---------------------------------------------------------------------------

#[test]
fn golden_scripted_lairs_exist_in_patch_levels() {
    let (header, rows) = raw_tsv(LEVELS_TXT);
    let ci = col_of(&header, "Id");
    let mut levels = std::collections::HashMap::new();
    for r in &rows {
        levels.insert(cell_u(r, ci), r);
    }
    let mut lairs: Vec<(u32, &str)> = Vec::new();
    for rec in BOSSES_ACTS23 {
        lairs.push((rec.lair, rec.id));
    }
    for rec in BOSSES_ACTS45 {
        lairs.push((rec.lair, rec.id));
    }
    for rec in FINALE_RECORDS {
        lairs.push((rec.lair, rec.id));
    }
    // Den of Evil / Catacombes 4 (quêtes acte I, journal W13) : mêmes ids.
    lairs.push((8, "den-of-evil"));
    lairs.push((37, "catacombs-4"));
    for (id, who) in lairs {
        let r = levels.get(&id).unwrap_or_else(|| panic!("patch_levels : id {id} ({who}) absent"));
        let g = |n: &str| r[col_of(&header, n)].trim().to_string();
        // Colonnes de la difficulté présentes et numériques (0 ou vide
        // légaux — convention tables v1.12 : cellule vide = 0).
        for col in [
            "MonDen", "MonDen(N)", "MonDen(H)", "MonUMin", "MonUMax", "MonUMin(N)", "MonUMax(N)",
            "MonUMin(H)", "MonUMax(H)",
        ] {
            let v = g(col);
            if !v.is_empty() {
                v.parse::<u32>()
                    .unwrap_or_else(|_| panic!("{who} : {col} non numérique ({v})"));
            }
        }
        // Les plans MonLvl1/2/3 (niveau monstre par difficulté) sont là dès
        // qu'il y a une densité de spawn ; 120/132 (Autel, World Stone) sont
        // des zones scriptées sans plan ambiant : MonDen = 0 exactement.
        let den: [u32; 3] = [g("MonDen"), g("MonDen(N)"), g("MonDen(H)")].map(|s| s.parse().unwrap());
        let mlv: [u32; 3] = [g("MonLvl1"), g("MonLvl2"), g("MonLvl3")].map(|s| s.parse().unwrap());
        if den[0] > 0 {
            assert!(mlv[0] > 0, "{who} : MonLvl1 présent (den {})", den[0]);
            assert!(mlv[1] > mlv[0] && mlv[2] > mlv[1], "{who} : MonLvl1<2<3 ({mlv:?})");
        } else {
            assert_eq!(den, [0, 0, 0], "{who} : zone scriptée sans spawn ambiant");
        }
    }
    // Repères de la campagne : repaire de Duriel 73, Chambre de la Pierre
    // du Monde 132 (MonLvl 0/0/0 — spawn scripté, plan ambiant absent).
    assert_eq!(levels[&73][col_of(&header, "Name")], "Act 2 - Duriel's Lair");
    let wstone = levels[&132];
    for col in ["MonLvl1", "MonLvl2", "MonLvl3"] {
        assert_eq!(wstone[col_of(&header, col)].trim(), "0", "World Stone : {col} = 0 (scripté)");
    }
}

// ---------------------------------------------------------------------------
// 6. patch_difficultylevels.txt — pénalités héros/diviseurs monstres
// ---------------------------------------------------------------------------

#[test]
fn golden_difficulty_levels_embedded_matches_raw() {
    let (header, rows) = raw_tsv(DIFFLEVELS_TXT);
    let name_c = col_of(&header, "Name");
    let dl = difficulty::embedded();
    let diffs = [
        difficulty::Difficulty::Normal,
        difficulty::Difficulty::Nightmare,
        difficulty::Difficulty::Hell,
    ];
    for (i, key) in ["Normal", "Nightmare", "Hell"].iter().enumerate() {
        let row = rows
            .iter()
            .find(|r| r[name_c].trim().eq_ignore_ascii_case(key))
            .unwrap_or_else(|| panic!("rangée {key}"));
        let u = |n: &str| cell_u(row, col_of(&header, n));
        let d = dl.row(diffs[i]);
        assert_eq!(d.resist_penalty, cell_i(row, col_of(&header, "ResistPenalty")), "{key}");
        assert_eq!(d.death_exp_penalty, u("DeathExpPenalty"), "{key}");
        assert_eq!(d.monster_skill_bonus, u("MonsterSkillBonus"), "{key}");
        assert_eq!(d.static_field_min, u("StaticFieldMin"), "{key}");
        assert_eq!(d.unique_damage_bonus, u("UniqueDamageBonus"), "{key}");
        assert_eq!(d.hireable_boss_damage_percent, u("HireableBossDamagePercent"), "{key}");
        assert_eq!(d.uber_code_odds, [u("UberCodeOddsNormal"), u("UberCodeOddsGood")], "{key}");
        assert_eq!(d.ultra_code_odds, [u("UltraCodeOddsNormal"), u("UltraCodeOddsGood")], "{key}");
    }
}

// ---------------------------------------------------------------------------
// 7. Garde-fous des correctifs 13a-bis (XP 99 niveaux + variantes TC)
// ---------------------------------------------------------------------------

/// GAPS-b corrigé : le héros `World` niveaute sur la TABLE RÉELLE 99×7
/// (`xp.rs`, colonne de sa classe) — plus aucun plafond MVP à 10. Points
/// de repère pris DANS la table (pas de constantes de test).
#[test]
fn golden_hero_levels_to_99_on_real_table() {
    let t = xp::XpTable::global();
    let mut w = test_world(monster_manifest(&monstats_table()["zombie1"]));
    w.clear_monsters();

    // Palier au-delà de l'ancien plafond MVP : niveau 12.
    let th12 = t.class("bar").unwrap().threshold(12) as u32;
    w.grant_xp(th12);
    let s = w.hero_stats();
    assert_eq!(s.level, 12, "12 = seuil table");
    assert_eq!(s.xp, th12);
    assert_eq!(
        s.xp_to_next as u64,
        t.class("bar").unwrap().threshold(13) - th12 as u64,
        "reste = seuil(13) − seuil(12)"
    );
    // Niveau 50 (au-delà de tout le MVP).
    w.grant_xp((t.class("bar").unwrap().threshold(50) - th12 as u64) as u32);
    assert_eq!(w.hero_stats().level, 50);

    // Plafond : MaxLvl de la table = 99, l'XP excédentaire ne monte plus.
    w.grant_xp(u32::MAX - w.hero_stats().xp);
    assert_eq!(w.hero_stats().level, 99, "cap = MaxLvl table");
    assert_eq!(w.hero_stats().xp_to_next, 0, "cap : plus rien à atteindre");
    w.grant_xp(1_000);
    assert_eq!(w.hero_stats().level, 99, "au-delà du cap : inchangé");

    // Round-trip save v3 : le niveau 99 survit (la save porte level+xp).
    let w2 = World::load(&w.save()).expect("save chargée");
    assert_eq!(w2.hero_stats().level, 99);

    // Événements : chaque palier émet HeroLeveled via le prochain tick.
    let mut w3 = test_world(monster_manifest(&monstats_table()["zombie1"]));
    w3.clear_monsters();
    w3.grant_xp(t.class("bar").unwrap().threshold(5) as u32);
    let mut leveled = Vec::new();
    for ev in w3.tick(50, None) {
        if let GameEvent::HeroLeveled { level } = ev {
            leveled.push(level);
        }
    }
    assert_eq!(leveled, vec![2, 3, 4, 5], "un event par palier franchi");
}

/// GAPS-a corrigé : la composition retail des variantes TC vit dans le
/// MOTEUR (`Difficulty::tc_variant`) — vérifiée ici contre la règle des
/// données pour 100 % des TC monstats + les TC de boss + les TC
/// superuniques (colonnes TC(N)/TC(H) de superuniques.txt = vérité).
#[test]
fn golden_tc_variant_retail_rule() {
    use d2rules::Difficulty;
    let tce = treasure_classes();
    let ms = monstats_table();
    let mut seen = std::collections::HashSet::new();
    for rec in ms.values() {
        if rec.tc.is_empty() || !seen.insert(rec.tc.clone()) {
            continue;
        }
        for (diff, sfx) in [(Difficulty::Nightmare, " (N)"), (Difficulty::Hell, " (H)")] {
            let want = tc_variant(&rec.tc, sfx); // règle indépendante (référence)
            let got = Difficulty::tc_variant(&rec.tc, diff);
            assert_eq!(got, want, "tc_variant({tc:?}, {sfx})", tc = rec.tc);
            assert!(tce.contains(&got), "variante « {got} » absente de TreasureClassEx");
        }
        assert_eq!(Difficulty::tc_variant(&rec.tc, Difficulty::Normal), rec.tc, "Normal = inchangé");
    }
    // TC de boss (nommées) : suffixe final, variantes présentes.
    for boss_tc in ["Andariel", "Duriel", "Mephisto", "Izual", "Haphesto", "Diablo", "Baal", "Blood Raven", "Griswold"] {
        assert_eq!(Difficulty::tc_variant(boss_tc, Difficulty::Hell), format!("{boss_tc} (H)"));
        assert!(tce.contains(&format!("{boss_tc} (H)")), "« {boss_tc} (H) »");
    }
    // Colonnes TC(N)/TC(H) de superuniques.txt : l'engine compose le MÊME
    // nom que la table pour les TC acte (« Act N Super … »).
    let su = superuniques_by_class();
    for (_name, tc, tc_n, tc_h) in su.values() {
        if tc.is_empty() || !tc.starts_with("Act ") {
            continue;
        }
        if !tc_n.is_empty() {
            assert_eq!(
                Difficulty::tc_variant(tc, Difficulty::Nightmare),
                *tc_n,
                "superuniques TC(N) de « {tc} »"
            );
        }
        if !tc_h.is_empty() && tce.contains(tc_h.as_str()) {
            // « Fangskin (H) » : trou de la table v1.12 (GAPS 7) — la
            // colonne TC(H) pointe un nom absent de TreasureClassEx.
            assert_eq!(Difficulty::tc_variant(tc, Difficulty::Hell), *tc_h, "superuniques TC(H) de « {tc} »");
        }
    }
}

/// GAPS-a corrigé, bout en bout : en ENFER, le drop du même monstre (même
/// graine, même gabarit, même combat) passe par la VARIANTE retail de la TC
/// (« Act 1 (H) H2H A » et ses sous-pools 54+, distincts du Normal). Si la
/// variante n'était pas composée, difficultéNormal et Hell produiraient des
/// suites de drops IDENTIQUES (même TC de base, même flux rng). Le repli MVP
/// est exclu de lui-même : la TC de base existe (pas de fallback).
#[test]
fn golden_hell_normal_drop_uses_retail_variant() {
    let mut z = monstats_table()["zombie1"].clone();
    // Niveau HELL sur le gabarit (Level(H) = 67) : mlvl réel de la difficulté.
    z.level[0] = z.level[2];
    // Drops (code, kind) d'un kill complet à la graine `seed`.
    let run = |seed: u64, hell: bool| -> Vec<(String, d2rules::ItemKind)> {
        let mut w = test_world(monster_manifest(&z));
        w.clear_monsters();
        if hell {
            w.set_difficulty(d2rules::Difficulty::Hell);
        }
        let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
        let mut ok = false;
        for (x, y) in
            [(hx + 2, hy), (hx - 2, hy), (hx, hy + 2), (hx, hy - 2), (hx + 1, hy), (hx - 1, hy - 1)]
        {
            if w.is_walkable(x, y) && w.spawn_monster_rule(&z.code, x, y, SpawnRule::default()) {
                ok = true;
                break;
            }
        }
        assert!(ok, "zombie spawné (seed {seed})");
        let mut drops = Vec::new();
        for _ in 0..400 {
            let target = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
            let input = target.map(|(x, y)| PlayerInput::Attack { x, y });
            for ev in w.tick(50, input) {
                if let GameEvent::ItemDropped { item, .. } = ev {
                    drops.push((item.code.clone(), item.kind));
                }
            }
            if w.monsters().is_empty() {
                break;
            }
        }
        drops
    };
    // ~62 % NoDrop + ~13 % or par pick (picks=1, NoDrop 100/160) : ces seeds
    // coïncident naturellement ; les picks Junk/Equip/Good résolvent des
    // chaînes (H) DIFFÉRENTES (pools 54+ vs Normal). Variante non composée
    // ⇒ 0 différence sur tous les seeds.
    let mut differed = 0;
    for seed in 0..40u64 {
        if run(seed, false) != run(seed, true) {
            differed += 1;
        }
    }
    assert!(
        differed >= 5,
        "Hell = Normal sur {differed}/40 seeds : la variante retail « Act 1 (H) H2H A » n'est pas utilisée au drop"
    );
}

