//! Interop .d2s — étape 19c (PLAN-FIDELITE phase 19) : ÉCRITURE durcie
//! contre les saves ORACLE (layout « retail 1.14d »).
//!
//! Méthode (cf. `docs/d2s-export-interop.md`) : 3 saves oracle proches de
//! nos profils (Barbare niveau 1 Normal / 30 en Cauchemar / 99 en Enfer)
//! sont DIFFÉES section par section avec nos exports équivalents
//! ([`World::export_d2s`], stats miroirs des fichiers oracle). Les écarts
//! de LAYOUT trouvés sont corrigés dans `save_d2s.rs` (flux stats « gf »,
//! zeros omis, squelette de listes `JM/JM0/jf nu/kf`, unks 0xFF, octet
//! final waypoints, bit 0 par difficulté, progression ×5) — ces tests
//! vérouillent le layout doré résultat + la stabilité du round-trip.
//!
//! Les octets oracle sont des DONNÉES de validation (générateur GPL-3.0
//! exécuté hors arbre, aucun code copié — cf. `d2d2s/tests/vectors/
//! MANIFEST.md`). SHA-256 garantis par `d2d2s/tests/oracle_saves.rs`
//! (14 saves du coéquipier) et le MANIFEST pour les nôtres.

#![forbid(unsafe_code)]

use super::world::{INV_SLOTS, World, xp_threshold};
use crate::Difficulty;
use d2d2s::{Class, SaveFile, StatsFormat};
use serde_json::json;

/// Saves oracle 19c (BA, SC, or 0, nom `Testba`, ts/map_id 0).
const ORACLE_FRESH: &[u8] = include_bytes!("../../d2d2s/tests/vectors/oracle/Bar19cFresh.d2s");
const ORACLE_NIGHT: &[u8] = include_bytes!("../../d2d2s/tests/vectors/oracle/Bar19cNight.d2s");
const ORACLE_HELL: &[u8] = include_bytes!("../../d2d2s/tests/vectors/oracle/Bar19cHell.d2s");

/// Profils miroirs : valeurs décodées des fichiers oracle (python LSB-first).
struct Profile {
    oracle: &'static [u8],
    level: u32,
    xp: u32,
    skill_points: u32,
    /// Points de stats non dépensés (stat 4, décodé de l'oracle).
    stat_points: u32,
    life: u32,
    mana: u32,
    stamina: u32,
    difficulty: Difficulty,
    /// Taille oracle (notre export = même taille, layout aligné).
    oracle_size: usize,
    /// Offset du marqueur skills « if » (fin du flux stats).
    skills_off: usize,
    progression_oracle: u8,
}

const PROFILES: &[Profile] = &[
    Profile {
        oracle: ORACLE_FRESH,
        level: 1,
        xp: 0,
        skill_points: 0,
        stat_points: 0,
        life: 55,
        mana: 10,
        stamina: 92,
        difficulty: Difficulty::Normal,
        oracle_size: 848,
        skills_off: 803,
        progression_oracle: 0,
    },
    Profile {
        oracle: ORACLE_NIGHT,
        level: 30,
        xp: 4_663_553,
        skill_points: 33,
        stat_points: 150,
        life: 133,
        mana: 39,
        stamina: 121,
        difficulty: Difficulty::Nightmare,
        oracle_size: 857,
        skills_off: 812,
        progression_oracle: 5,
    },
    Profile {
        oracle: ORACLE_HELL,
        level: 99,
        xp: 3_520_485_254,
        skill_points: 110,
        stat_points: 505,
        life: 311,
        mana: 108,
        stamina: 190,
        difficulty: Difficulty::Hell,
        oracle_size: 857,
        skills_off: 812,
        progression_oracle: 15,
    },
];

/// Squelette de fin retail : `JM+0 / JM+0 (corpse) / jf nu (merc) / kf+0`.
const SKELETON_ITEMS: &[u8] = &[
    0x4A, 0x4D, 0x00, 0x00, 0x4A, 0x4D, 0x00, 0x00, 0x6A, 0x66, 0x6B, 0x66, 0x00,
];

/// Monde BA minimal avec les stats du profil (miroir oracle : attrs BA
/// 30/10/20/25 via `patch_charstats.txt`, vie/mana/endurance du fichier).
fn mirror_world(p: &Profile) -> World {
    let manifest = json!({ "heroes": { "BA": {} } });
    let grid = vec![vec![false; 20]; 20];
    let mut w = World::with_seed_and_hero(grid, manifest, 42, "BA");
    {
        let s = &mut w.players[0].data.stats;
        s.level = p.level;
        s.xp = p.xp;
        s.xp_to_next = 0;
        s.strength = 30;
        s.dexterity = 20;
        s.max_life = p.life;
        s.life = p.life;
        s.max_mana = p.mana;
        s.mana = p.mana;
        s.max_stamina = p.stamina;
        s.stamina = p.stamina;
        s.gold = 0;
        s.skill_points = p.skill_points;
        s.stat_points = p.stat_points;
    }
    w.players[0].data.skills.clear();
    w.players[0].data.inv = vec![None; INV_SLOTS];
    w.set_difficulty(p.difficulty);
    w
}

#[test]
fn oracle_saves_have_the_retail_layout() {
    for p in PROFILES {
        let save = SaveFile::parse(p.oracle).expect("save oracle : parse strict");
        assert_eq!(save.header.class, Class::Barbarian);
        assert_eq!(save.header.version, 96, "0x60");
        // gf + flux 1.10+
        assert_eq!(save.stats.format, StatsFormat::GfPrefixed110, "stats gf");
        assert_eq!(&p.oracle[765..767], b"gf", "marqueur gf à l'octet 765");
        // sections fixes aux offsets canoniques
        assert_eq!(&p.oracle[335..339], b"Woo!");
        assert_eq!(&p.oracle[633..635], b"WS");
        assert_eq!(&p.oracle[714..716], b"w4");
        // squelette final JM/JM0/jf nu/kf0
        let tail = &p.oracle[p.oracle.len() - 13..];
        assert_eq!(tail, SKELETON_ITEMS, "squelette items retail");
        // offset skills = taille - 13 (squelette) - 32 (section skills)
        assert_eq!(p.oracle.len() - 45, p.skills_off, "offset skills");
        assert_eq!(&p.oracle[p.skills_off..p.skills_off + 2], b"if");
        // octet final waypoints = 1 + bit 0 Rogue Encampment partout
        assert_eq!(p.oracle[633 + 80], 1, "octet final waypoints");
        for d in 0..3 {
            let bits = save.waypoints.difficulty(d).unwrap();
            assert!(bits.is_set(0), "Rogue Encampment actif en difficulté {d}");
        }
        let _ = save.header.progression;
    }
}

#[test]
fn our_export_adopts_the_retail_layout() {
    for p in PROFILES {
        let w = mirror_world(p);
        let bytes = w.export_d2s("Testba").expect("export");
        // 1) même taille que l'oracle : layout identique au.octet près des
        //    données que le save v3 ne porte pas (quêtes/NPC/WP par diff).
        assert_eq!(
            bytes.len(),
            p.oracle_size,
            "taille alignée retail pour le niveau {}",
            p.level
        );
        // 2) lecture STRICTE de nos propres exports (checksum inclus).
        let save = SaveFile::parse(&bytes).expect("rélecture stricte");
        assert_eq!(save.stats.format, StatsFormat::GfPrefixed110, "stats gf");
        assert_eq!(&bytes[765..767], b"gf");
        // 3) squelette + offset skills identiques à l'oracle.
        assert_eq!(&bytes[bytes.len() - 13..], SKELETON_ITEMS);
        assert_eq!(bytes.len() - 45, p.skills_off);
        assert_eq!(&bytes[p.skills_off..p.skills_off + 2], b"if");
        // 4) en-tête : unks retail + progression ×5.
        assert_eq!(&bytes[0x29..0x2B], &[16, 30], "unk_29 retail");
        assert_eq!(&bytes[0x34..0x38], &[0xFF; 4], "unk_34 retail");
        assert_eq!(&bytes[0x88..0xA8], &[0xFF; 32], "apparence retail");
        // Notre approximation : 5 × difficulté courante (0/5/10) — le 15
        // retail « Enfer terminé » n'est pas exprimable (cf. doc).
        assert_eq!(bytes[0x25], 5 * p.difficulty.index() as u8);
        if p.level == 1 {
            assert_eq!(bytes[0x25], p.progression_oracle);
        }
        // 5) waypoints : octet final 1 + bit 0 dans les 3 enregistrements.
        assert_eq!(bytes[633 + 80], 1);
        for d in 0..3 {
            let bits = save.waypoints.difficulty(d).unwrap();
            assert!(bits.is_set(0), "bit 0 actif en difficulté {d}");
        }
        // 6) stats : zeros omis comme retail (goldbank absente ; statpts
        //    présente ssi > 0 — miroir oracle : 0 / 150 / 505).
        if p.stat_points == 0 {
            assert!(save.stats.value(4).is_none(), "statpts 0 omis");
        } else {
            assert_eq!(save.stats.value(4), Some(p.stat_points), "statpts");
        }
        assert!(save.stats.value(15).is_none(), "goldbank 0 omis");
        assert_eq!(save.stats.value(0), Some(30), "strength");
        assert_eq!(save.stats.value(7), Some(p.life), "maxhp");
        // 7) skills vides (profil sans compétence) : section nulle, comme
        //    l'oracle EMPTY_SKILLS.
        assert!(save.skills.raw()[2..32].iter().all(|&b| b == 0));
        let _ = save;
    }
}

#[test]
fn export_import_export_cycle_is_byte_stable_per_profile() {
    for p in PROFILES {
        let w = mirror_world(p);
        let b1 = w.export_d2s("Testba").unwrap();
        let mut w2 = mirror_world(p);
        let report = w2.import_d2s(&b1).expect("import de notre export");
        assert_eq!(report.class, "BA");
        let b2 = w2.export_d2s("Testba").unwrap();
        assert_eq!(b2, b1, "export→import→export stable (niveau {})", p.level);
        let mut w3 = mirror_world(p);
        w3.import_d2s(&b2).expect("second import");
        let b3 = w3.export_d2s("Testba").unwrap();
        assert_eq!(b3, b1, "cycle de 3 stable");
        // L'état mappé suit le profil (niveau/xp plafonnés aux tables).
        let s = w2.hero_stats();
        assert_eq!(s.level, p.level);
        assert_eq!(s.xp, xp_threshold("BA", p.level));
        assert_eq!(s.strength, 30);
        assert_eq!(s.max_life, p.life);
        assert_eq!(s.skill_points, p.skill_points);
    }
}

#[test]
fn imported_oracle_save_round_trips_through_our_pipeline() {
    // Une save ORACLE traverse notre chaîne : import (lecture stricte) →
    // export (écriture retail) → rélecture stricte. Preuve d'interop dans
    // les DEUX sens avec le même layout.
    for p in PROFILES {
        let mut w = mirror_world(p);
        let report = w.import_d2s(p.oracle).expect("oracle importé");
        assert_eq!(report.name, "Testba");
        assert_eq!(report.level, p.level);
        let ours = w.export_d2s("Testba").unwrap();
        let back = SaveFile::parse(&ours).expect("notre export relisible");
        assert_eq!(back.header.class, Class::Barbarian);
        assert_eq!(back.stats.format, StatsFormat::GfPrefixed110);
        assert_eq!(back.stats.value(12), Some(p.level), "level");
        // Et stable : re-import de NOTRE export → même état mappé.
        let mut w2 = mirror_world(p);
        let r2 = w2.import_d2s(&ours).unwrap();
        assert_eq!(r2.level, report.level);
        assert_eq!(w2.hero_stats().skill_points, w.hero_stats().skill_points);
    }
}

#[test]
fn exported_items_are_starter_gear_identified() {
    // Mission 19c : « items avec starter gear identified » — un inventaire
    // non vide exporte des items identifiés au bit starter (33) posé.
    let mut w = mirror_world(&PROFILES[0]);
    w.players[0].data.inv[0] = Some(crate::ItemDef {
        code: "hax".into(),
        name: "Hand Axe".into(),
        kind: crate::ItemKind::Weapon,
        amount: 0,
    });
    w.players[0].data.inv[10] = Some(crate::ItemDef {
        code: "hp1".into(),
        name: "Lesser Healing Potion".into(),
        kind: crate::ItemKind::Potion,
        amount: 0,
    });
    let bytes = w.export_d2s("Testba").unwrap();
    let save = SaveFile::parse(&bytes).unwrap();
    assert_eq!(save.character_items.items.len(), 2);
    for it in &save.character_items.items {
        assert!(it.header.identified, "identifié");
        assert!(it.header.starter, "bit starter (33) posé");
        assert!(it.header.unk_39 == 1, "bit 39 retail (=1)");
    }
    assert!(save.character_items.items.iter().any(|i| i.type_code() == "hax"));
}

#[test]
fn zero_stats_are_omitted_like_retail_but_gold_survives() {
    // Retail : or 0 → stat 14 absente ; or > 0 → présente (or porté).
    let mut w = mirror_world(&PROFILES[0]);
    w.players[0].data.stats.gold = 0;
    let bytes = w.export_d2s("Testba").unwrap();
    let save = SaveFile::parse(&bytes).unwrap();
    assert!(save.stats.value(14).is_none(), "or 0 omis");
    assert!(save.stats.value(5).is_none(), "newskills 0 omis");

    w.players[0].data.stats.gold = 123;
    w.players[0].data.stats.skill_points = 2;
    let bytes = w.export_d2s("Testba").unwrap();
    let save = SaveFile::parse(&bytes).unwrap();
    assert_eq!(save.stats.value(14), Some(123), "or présent si > 0");
    assert_eq!(save.stats.value(5), Some(2), "newskills présent si > 0");
}
