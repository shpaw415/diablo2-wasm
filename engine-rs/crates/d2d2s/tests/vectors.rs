//! Tests d'intégration du crate `d2d2s` contre les 7 vecteurs SYNTHÉTIQUES
//! de la campagne RE (copies fidèles de
//! `d2w-re/evidence-formats/d2s/vectors/` — SHA-256 vérifiées, cf.
//! `vectors/MANIFEST.md`).
//!
//! Contrat (mission d2w-integ étape 4) : les bons passent, `bad_checksum` /
//! `corrupt_magic` / `truncated` échouent PROPREMENT (erreur typée, jamais
//! de panic), et la réécriture round-trip est octet-à-octet.

#![forbid(unsafe_code)]

use d2d2s::{
    compute_checksum, Container, D2sError, Location, Quality, QualityData, SaveFile,
    StatsFormat, TypeData, HEADER_SIZE,
};

const MINIMAL: &[u8] = include_bytes!("vectors/minimal_v0x60.d2s");
const BAD_CHECKSUM: &[u8] = include_bytes!("vectors/bad_checksum.d2s");
const CORRUPT_MAGIC: &[u8] = include_bytes!("vectors/corrupt_magic.d2s");
const V109: &[u8] = include_bytes!("vectors/version_1_09_gf.d2s");
const MAGIC_ITEM: &[u8] = include_bytes!("vectors/magic_item.d2s");
const SOCKETED_ARMOR: &[u8] = include_bytes!("vectors/socketed_armor.d2s");
const TRUNCATED: &[u8] = include_bytes!("vectors/truncated.d2s");

/// Checksums dorés stockés dans l'en-tête de chaque vecteur (champ 0x0C,
/// lu indépendamment — preuve que le calcul Rust retombe dessus).
const GOLDEN_CHECKSUMS: &[(&[u8], u32)] = &[
    (MINIMAL, 0xE73D_F9D5),
    (V109, 0xAFF4_3121),
    (MAGIC_ITEM, 0x932D_2517),
    (SOCKETED_ARMOR, 0x59A4_1D0B),
    (TRUNCATED, 0xD4B8_58B8),
];

// ---------------------------------------------------------------------------
// Checksums dorés
// ---------------------------------------------------------------------------

#[test]
fn golden_checksums_recomputed_from_scratch() {
    // Chaque checksum stocké à 0x0C est retrouvé par le calcul Rust
    // (rotation gauche + octet, champ neutralisé) — preuve indépendante.
    for (data, expected) in GOLDEN_CHECKSUMS {
        assert_eq!(compute_checksum(data), *expected, "checksum doré");
    }
}

// ---------------------------------------------------------------------------
// minimal_v0x60.d2s — le bon passe
// ---------------------------------------------------------------------------

#[test]
fn minimal_parses_strictly() {
    let save = SaveFile::parse(MINIMAL).expect("minimal doit passer en strict");
    assert_eq!(save.header.magic, 0xAA55_AA55);
    assert_eq!(save.header.version, 96);
    assert_eq!(d2d2s::version_name(96), "v1.10+ (v1.12 = 0x60)");
    assert_eq!(save.header.name(), "Testchar");
    assert_eq!(save.header.class, d2d2s::Class::Paladin);
    assert_eq!(save.header.level, 12);
    assert!(save.header.status.expansion);
    assert!(!save.header.status.hardcore);
    assert_eq!(save.header.map_id, 0x0BAD_F00D);
    assert!(save.header.difficulty[0].active && save.header.difficulty[0].act == 0);

    // Sections fixes en place.
    assert!(save.quests.marker_ok());
    assert!(save.waypoints.marker_ok());
    assert!(save.npc.marker_ok());
    // trevin : en-tête quêtes `06 00 00 00 2A 01` (298 = taille de section).
    assert_eq!(save.quests.header_unknown(), &[6, 0, 0, 0, 0x2A, 0x01]);
    assert_eq!(save.waypoints.header_unknown(), &[1, 0, 0, 0, 0x50, 0]);
}

#[test]
fn minimal_stats_reload_identically() {
    let save = SaveFile::parse(MINIMAL).unwrap();
    assert_eq!(save.stats.format, StatsFormat::V110Plus);
    assert_eq!(save.stats.entries.len(), 9);
    // valeurs écrites par make_vectors.py (brutes et rechargées)
    assert_eq!(save.stats.value(0), Some(20)); // strength
    assert_eq!(save.stats.value(1), Some(25)); // energy
    assert_eq!(save.stats.value(2), Some(20)); // dexterity
    assert_eq!(save.stats.value(3), Some(25)); // vitality
    assert_eq!(save.stats.raw(7), Some(148)); // maxhp stocké ×256
    assert_eq!(save.stats.value(7), Some(0)); // 148 >> 8
    assert_eq!(save.stats.value(12), Some(12)); // level
    assert_eq!(save.stats.value(13), Some(123_456)); // experience
    assert_eq!(save.stats.value(14), Some(1_000)); // gold
    assert_eq!(save.stats.value(15), Some(2_500)); // goldbank
}

#[test]
fn minimal_skills_items_and_trailer() {
    let save = SaveFile::parse(MINIMAL).unwrap();
    assert!(save.skills.marker_ok());
    assert_eq!(save.skills.level(3), 5);
    assert_eq!(save.skills.level(8), 3);
    assert_eq!(save.character_items.declared_count, 0);
    assert!(save.character_items.items.is_empty());
    assert!(save.corpse_items.is_none(), "pas de liste corpse dans minimal");
    assert!(save.merc_items.is_none());
    let golem = save.golem.as_ref().expect("trailer kf présent");
    assert!(!golem.has_golem);
}

#[test]
fn minimal_round_trips_byte_for_byte() {
    let save = SaveFile::parse(MINIMAL).unwrap();
    assert_eq!(save.to_bytes(), MINIMAL);
}

#[test]
fn minimal_summary_conversion() {
    // Mission §3 : conversion d2s -> struct interne minimale, pour
    // l'intégrateur suivant (PAS branché dans le save v3).
    let summary = SaveFile::parse(MINIMAL).unwrap().to_summary();
    assert_eq!(summary.name, "Testchar");
    assert_eq!(summary.class_name, "Paladin");
    assert_eq!(summary.level, 12);
    assert_eq!(summary.strength, Some(20));
    assert_eq!(summary.energy, Some(25));
    assert_eq!(summary.dexterity, Some(20));
    assert_eq!(summary.vitality, Some(25));
    assert_eq!(summary.max_hp, Some(0)); // 148 >> 8
    assert_eq!(summary.experience, Some(123_456));
    assert_eq!(summary.gold, Some(1_000));
    assert_eq!(summary.gold_bank, Some(2_500));
    assert_eq!(summary.items, 0);
    assert!(summary.expansion && !summary.hardcore);
}

// ---------------------------------------------------------------------------
// bad_checksum.d2s / corrupt_magic.d2s — échouent proprement
// ---------------------------------------------------------------------------

#[test]
fn bad_checksum_fails_cleanly_in_strict() {
    assert_eq!(
        SaveFile::parse(BAD_CHECKSUM),
        Err(D2sError::ChecksumMismatch { stored: 0xDEAD_BEEF, computed: 0xE73D_F9D5 })
    );
}

#[test]
fn bad_checksum_lenient_parses_but_flags_checksum() {
    let save = SaveFile::parse_lenient(BAD_CHECKSUM).expect("structure valide sinon");
    assert!(!save.checksum_valid_for(BAD_CHECKSUM));
    // et la réécriture RÉPARE le checksum
    let fixed = save.to_bytes();
    assert_eq!(compute_checksum(&fixed), compute_checksum(MINIMAL));
    assert!(SaveFile::parse(&fixed).is_ok());
}

#[test]
fn corrupt_magic_fails_cleanly_in_strict() {
    assert_eq!(
        SaveFile::parse(CORRUPT_MAGIC),
        Err(D2sError::BadSignature { got: 0x1234_5678 })
    );
}

#[test]
fn corrupt_magic_lenient_reveals_both_faults() {
    // La signature a été écrasée SANS recalculer le checksum : les deux
    // sont invalides (lecture indépendante, MANIFEST : « checksum
    // recalculé différent »).
    let save = SaveFile::parse_lenient(CORRUPT_MAGIC).unwrap();
    assert_eq!(save.header.magic, 0x1234_5678);
    assert!(!save.checksum_valid_for(CORRUPT_MAGIC));
}

// ---------------------------------------------------------------------------
// truncated.d2s — échoue proprement au milieu du flux de stats
// ---------------------------------------------------------------------------

#[test]
fn truncated_fails_premature_end_in_stats_stream() {
    // Le checksum du tronqué est VALIDE (recalculé à la fabrication) :
    // l'échec vient donc du flux de stats, après 765*8 + 19 bits
    // (ID 9 bits + valeur 10 bits écrits, puis plus rien).
    match SaveFile::parse(TRUNCATED) {
        Err(D2sError::PrematureEnd { bit, wanted_bits }) => {
            assert_eq!(bit, 765 * 8 + 19);
            assert_eq!(wanted_bits, 9);
        }
        other => panic!("attendu PrematureEnd, obtenu {:?}", other.err()),
    }
}

#[test]
fn too_short_file_is_an_error() {
    assert_eq!(SaveFile::parse(&MINIMAL[..HEADER_SIZE - 1]), Err(D2sError::TooShort { len: 764 }));
    assert_eq!(SaveFile::parse(&[]), Err(D2sError::TooShort { len: 0 }));
}

// ---------------------------------------------------------------------------
// version_1_09_gf.d2s — branche "gf" 1.09
// ---------------------------------------------------------------------------

#[test]
fn v109_gf_stats_branch() {
    let save = SaveFile::parse(V109).expect("v1.09 doit passer en strict");
    assert_eq!(save.header.version, 92);
    assert_eq!(d2d2s::version_name(92), "v1.09");
    assert_eq!(save.stats.format, StatsFormat::Gf109);
    assert_eq!(save.stats.value(0), Some(20)); // strength
    assert_eq!(save.stats.value(12), Some(12)); // level
    assert_eq!(save.stats.value(14), Some(1_000)); // gold
    assert_eq!(save.stats.value(1), None); // absent du bitfield
    assert_eq!(save.skills.level(3), 2);
    assert_eq!(save.character_items.declared_count, 0);
    assert_eq!(save.to_bytes(), V109, "round-trip octet-à-octet");
}

// ---------------------------------------------------------------------------
// magic_item.d2s — item étendu magic avec mods
// ---------------------------------------------------------------------------

#[test]
fn magic_item_parses_completely() {
    let save = SaveFile::parse(MAGIC_ITEM).expect("magic_item doit passer en strict");
    let items = &save.character_items.items;
    assert_eq!(save.character_items.declared_count, 1);
    assert_eq!(items.len(), 1);
    let ring = &items[0];
    assert_eq!(ring.type_code(), "rin");
    assert!(ring.header.identified);
    assert!(!ring.header.compact && !ring.header.ear);
    assert_eq!(ring.header.location, Location::Stored);
    assert_eq!(ring.header.container, Container::Inventory);
    let ext = ring.extended.as_ref().unwrap();
    assert_eq!(ext.file_id, 0xDEAD_BEEF);
    assert_eq!(ext.ilvl, 45);
    assert_eq!(ext.quality, Quality::Magic);
    assert_eq!(ext.quality_data, QualityData::Magic { prefix: 5, suffix: 3 });
    assert!(ext.graphic.is_none() && ext.class_specific.is_none());
    // mods [9 et 0] (MANIFEST) — largeurs Save Bits réelles :
    // strength/maxmana = 8 bits chacune (table locale v1.12).
    assert_eq!(ext.mods.len(), 2);
    assert_eq!(ext.mods[0].id, 9);
    assert_eq!(ext.mods[0].bits, 8);
    assert_eq!(ext.mods[0].value, 100);
    assert_eq!(ext.mods[1].id, 0);
    assert_eq!(ext.mods[1].bits, 8);
    assert_eq!(ext.mods[1].value, 60);
    // corpse présent mais VIDE (liste JM + 0), trailer kf.
    let corpse = save.corpse_items.as_ref().expect("liste corpse présente");
    assert_eq!(corpse.declared_count, 0);
    assert!(!save.golem.as_ref().unwrap().has_golem);
    assert_eq!(save.stats.value(0), Some(30));
    assert_eq!(save.stats.value(12), Some(12));
}

#[test]
fn magic_item_round_trips_byte_for_byte() {
    let save = SaveFile::parse(MAGIC_ITEM).unwrap();
    assert_eq!(save.to_bytes(), MAGIC_ITEM);
}

// ---------------------------------------------------------------------------
// socketed_armor.d2s — armure sertie + 2 gemmes enfants
// ---------------------------------------------------------------------------

#[test]
fn socketed_armor_parses_with_children() {
    let save = SaveFile::parse(SOCKETED_ARMOR).expect("socketed_armor doit passer en strict");
    let items = &save.character_items.items;
    assert_eq!(items.len(), 1);
    let cap = &items[0];
    assert_eq!(cap.type_code(), "cap");
    assert!(cap.header.socketed);
    assert_eq!(cap.header.children_count, 2);
    assert_eq!(cap.children.len(), 2, "2 gemmes enfants rattachées");
    let ext = cap.extended.as_ref().unwrap();
    assert_eq!(ext.file_id, 0xCAFE_D00D);
    assert_eq!(ext.ilvl, 20);
    assert_eq!(ext.quality, Quality::Normal);
    match &ext.type_data {
        TypeData::Armor { defense_stored, max_durability, durability, sockets } => {
            // MANIFEST : « cap defense=33 sockets=2 » (33 = valeur stockée ;
            // la base réelle discutée « defense + 10 » reste un brut conservé).
            assert_eq!(*defense_stored, 33);
            assert_eq!(*max_durability, 22);
            assert_eq!(*durability, 22);
            assert_eq!(*sockets, Some(2), "4 bits (divergence krisives 3 bits tranchée)");
        }
        other => panic!("type_data attendu Armor, obtenu {:?}", other),
    }
    assert!(ext.mods.is_empty());
    // gemmes : charmes compacts de 111 bits, alignés sur l'octet.
    for kid in &cap.children {
        assert_eq!(kid.type_code(), "cm1");
        assert!(kid.header.compact);
        assert!(kid.extended.is_none());
    }
    assert_eq!(save.stats.value(0), Some(20));
    assert_eq!(save.to_bytes(), SOCKETED_ARMOR, "round-trip octet-à-octet");
}

// ---------------------------------------------------------------------------
// Robustesse API publique
// ---------------------------------------------------------------------------

#[test]
fn all_good_vectors_strict_parse_and_round_trip() {
    for (name, data) in [
        ("minimal_v0x60", MINIMAL),
        ("version_1_09_gf", V109),
        ("magic_item", MAGIC_ITEM),
        ("socketed_armor", SOCKETED_ARMOR),
    ] {
        let save = SaveFile::parse(data).unwrap_or_else(|e| panic!("{} : {}", name, e));
        assert_eq!(save.to_bytes(), data, "{} : round-trip octet-à-octet", name);
        assert!(save.checksum_valid_for(data), "{} : checksum doré", name);
    }
}

#[test]
fn quests_and_waypoints_views_on_minimal() {
    let save = SaveFile::parse(MINIMAL).unwrap();
    // Personnage neuf : aucune quête complétée, aucun waypoint.
    for d in 0..3 {
        let qd = save.quests.difficulty(d).unwrap();
        assert!(!qd.warriv_intro.completed());
        assert!(qd.act1.iter().all(|q| q.raw() == 0));
        assert!(qd.act5.iter().all(|q| q.raw() == 0));
        let wp = save.waypoints.difficulty(d).unwrap();
        assert!(!wp.is_set(0), "bit 0 = Rogue Encampment éteint");
        assert_eq!(wp.0, 0);
    }
    assert_eq!(save.waypoints.is_active(0, "Rogue Encampment"), Some(false));
}
