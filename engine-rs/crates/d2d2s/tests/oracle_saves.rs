//! Interop .d2s — étape 19a (PLAN-FIDELITE phase 19) : 14 saves ORACLE.
//!
//! Fichiers produits par le binaire `d2w-oracle/d2s-file-generator` (dépôt
//! GPL-3.0 exécuté HORS arbre — ses octets sont des DONNÉES de validation,
//! aucun code n'est copié ici ; les cas de test du générateur proviennent
//! du jeu réel LoD v1.14d, cf. son README). Chaque save est validé contre
//! ses PARAMÈTRES de génération (classe, mode, niveau, difficulté terminée,
//! or stash) : en-tête, checksum, quêtes, waypoints, NPC, stats, items,
//! puis round-trip octet-à-octet.
//!
//! Divergences tranchées contre notre impl d'avant fix (cf. lib.rs §
//! Divergences) : préfixe « gf » des stats en 0x60 (avant : `UnknownStatId
//! { id: 103 }`) et merc « jf » nu sans « JM count » (avant : `BadMarker
//! { expected: "JM", got: "kf" }`).

#![forbid(unsafe_code)]

use d2d2s::{Class, QuestDifficulty, QuestFlags, SaveFile, StatsFormat};

/// Un save oracle + ses paramètres de génération.
struct Spec {
    bytes: &'static [u8],
    disk: &'static str,
    name: &'static str,
    class: Class,
    hardcore: bool,
    level: u8,
    /// 0 = NONE, 1 = NORMAL, 2 = NIGHTMARE, 3 = HELL (toutes les
    /// difficultés jusqu'à celle-ci sont terminées).
    completions: u8,
    gold: u32,
}

const SPECS: &[Spec] = &[
    Spec { bytes: include_bytes!("vectors/oracle/AmaFresh.d2s"), disk: "AmaFresh.d2s", name: "AmaFresh", class: Class::Amazon, hardcore: false, level: 1, completions: 0, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/SorFresh.d2s"), disk: "SorFresh.d2s", name: "SorFresh", class: Class::Sorceress, hardcore: false, level: 1, completions: 0, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/Nec-FreshG.d2s"), disk: "Nec-FreshG.d2s", name: "Nec-FreshG", class: Class::Necromancer, hardcore: false, level: 1, completions: 0, gold: 123_456 },
    Spec { bytes: include_bytes!("vectors/oracle/PalMidNorm.d2s"), disk: "PalMidNorm.d2s", name: "PalMidNorm", class: Class::Paladin, hardcore: false, level: 30, completions: 1, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/BarMidNorm.d2s"), disk: "BarMidNorm.d2s", name: "BarMidNorm", class: Class::Barbarian, hardcore: false, level: 30, completions: 1, gold: 800 },
    Spec { bytes: include_bytes!("vectors/oracle/DruMidNight.d2s"), disk: "DruMidNight.d2s", name: "DruMidNight", class: Class::Druid, hardcore: false, level: 40, completions: 2, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/AssMidNight.d2s"), disk: "AssMidNight.d2s", name: "AssMidNight", class: Class::Assassin, hardcore: false, level: 40, completions: 2, gold: 424_242 },
    Spec { bytes: include_bytes!("vectors/oracle/AmaHellSix.d2s"), disk: "AmaHellSix.d2s", name: "AmaHellSix", class: Class::Amazon, hardcore: false, level: 60, completions: 3, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/SorHellGold.d2s"), disk: "SorHellGold.d2s", name: "SorHellGold", class: Class::Sorceress, hardcore: false, level: 60, completions: 3, gold: 1_000_000 },
    Spec { bytes: include_bytes!("vectors/oracle/NecHellEnd.d2s"), disk: "NecHellEnd.d2s", name: "NecHellEnd", class: Class::Necromancer, hardcore: false, level: 99, completions: 3, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/PalHellEnd.d2s"), disk: "PalHellEnd.d2s", name: "PalHellEnd", class: Class::Paladin, hardcore: true, level: 99, completions: 3, gold: 0 },
    Spec { bytes: include_bytes!("vectors/oracle/BarHellRich.d2s"), disk: "BarHellRich.d2s", name: "BarHellRich", class: Class::Barbarian, hardcore: true, level: 99, completions: 3, gold: 2_500_000 },
    Spec { bytes: include_bytes!("vectors/oracle/DruHellEnd.d2s"), disk: "DruHellEnd.d2s", name: "DruHellEnd", class: Class::Druid, hardcore: false, level: 99, completions: 3, gold: 555_555 },
    Spec { bytes: include_bytes!("vectors/oracle/AssHellEnd.d2s"), disk: "AssHellEnd.d2s", name: "AssHellEnd", class: Class::Assassin, hardcore: false, level: 99, completions: 3, gold: 0 },
];

/// Checksums SHA-256 des 14 vecteurs (MANIFEST.md) — garde-fou anti-modif.
const GOLDEN_SHA256: &[(&str, &str)] = &[
    ("AmaFresh.d2s", "4b310b4b92dd1dc35180dda73088d12504c0a99a2a874a098af14a2f024bc5be"),
    ("SorFresh.d2s", "9f41ca2407c3a1a7b9ddba11d98b25cd354729647526efe559b3063527c6b1d9"),
    ("Nec-FreshG.d2s", "0fd8ae4cc91ea2c8d9463f04c9c2bab926222bd41eca2e5aa62823cf3dc6b138"),
    ("PalMidNorm.d2s", "63404b96506bbb224a1f2288f1b235e0433fd2934640600c91a59360dc5fe988"),
    ("BarMidNorm.d2s", "5608ecb59af490b5b9a3f1f99662657eec98830ed291be2311088fb6d666e52d"),
    ("DruMidNight.d2s", "0c7bf1da168425f322cff3f7867440cc3b474ee26ff632dd0d49d26895079c5d"),
    ("AssMidNight.d2s", "abce987913ed77465f7cbe8cf3b9dab17b2f173b957a52d50cafe7b0f4596700"),
    ("AmaHellSix.d2s", "0da7ae42f29c7b139271bfc14a852ebcb93e386ddd03039cde6f02513282384d"),
    ("SorHellGold.d2s", "088cda3523514f6704f518b934d4a21fa4665cbfbe90e24c5a921e4ebd2dbccb"),
    ("NecHellEnd.d2s", "24d921319c04b6457cfafc6891d4013a268c19dff2f8a811e14a1636d95ab4f6"),
    ("PalHellEnd.d2s", "f9c9b6df08d062f32ed65e1c94b5ec584c09a2cd43e1ae2202f415163e796a7c"),
    ("BarHellRich.d2s", "880e7f37c2a1b20015ef1ed129cb1a5a54beb4b887d4eb77a2cec982ffcbf88b"),
    ("DruHellEnd.d2s", "e798f724aa8f70f95b073c4f04bd2a08c2f15ea2e6609f792bd75c17213c6773"),
    ("AssHellEnd.d2s", "ef9bfb149f7dc3f791c164ceb8a2b90183aab9d78a853ab6af01cf5c2d41ee94"),
];

// ---------------------------------------------------------------------------
// Modèle des paramètres — bases de classe (données du jeu LoD) + règles
// README de l'oracle : +4 skills / +5 stats / +20 vie par difficulté
// terminée, XP = seuil exact du niveau.
// ---------------------------------------------------------------------------

/// (str, energy, dex, vit, hp base, hp/niveau, mana base, mana/niveau,
///  stamina base, stamina/niveau) — valeurs de départ LoD.
fn bases(class: Class) -> (u32, u32, u32, u32, f32, f32, f32, f32, f32, f32) {
    match class {
        Class::Amazon => (20, 15, 25, 20, 50.0, 2.0, 15.0, 1.5, 84.0, 1.0),
        Class::Sorceress => (10, 35, 25, 10, 40.0, 1.0, 35.0, 2.0, 74.0, 1.0),
        Class::Necromancer => (15, 25, 25, 15, 45.0, 1.5, 25.0, 2.0, 79.0, 1.0),
        Class::Paladin => (25, 15, 20, 25, 55.0, 2.0, 15.0, 1.5, 89.0, 1.0),
        Class::Barbarian => (30, 10, 20, 25, 55.0, 2.0, 10.0, 1.0, 92.0, 1.0),
        Class::Druid => (15, 20, 20, 25, 55.0, 1.5, 20.0, 2.0, 84.0, 1.0),
        Class::Assassin => (20, 25, 20, 20, 50.0, 2.0, 25.0, 1.5, 95.0, 1.25),
        Class::Other(_) => unreachable!("aucun save oracle hors classe"),
    }
}

/// XP exacte du niveau (seuils LoD — les niveaux couverts par les saves).
fn xp_for_level(level: u8) -> u32 {
    match level {
        1 => 0,
        30 => 4_663_553,
        40 => 17_270_791,
        60 => 117_772_849,
        99 => 3_520_485_254,
        _ => unreachable!("niveau oracle non couvert : {level}"),
    }
}

/// Vie max attendue (raw ×256) : base + montées de niveau + 20 par
/// difficulté terminée (« The Golden Bird »).
fn expected_hp_raw(spec: &Spec) -> u32 {
    let (_, _, _, _, hp0, hp_up, _, _, _, _) = bases(spec.class);
    let ups = (spec.level - 1) as f32;
    let comp = spec.completions as f32;
    ((hp0 + ups * hp_up + comp * 20.0) * 256.0) as u32
}

/// Mana max attendu (raw ×256) — aucun bonus par difficulté.
fn expected_mana_raw(spec: &Spec) -> u32 {
    let (_, _, _, _, _, _, mp0, mp_up, _, _) = bases(spec.class);
    let ups = (spec.level - 1) as f32;
    ((mp0 + ups * mp_up) * 256.0) as u32
}

/// Stamina max attendue (raw ×256) — aucun bonus par difficulté.
fn expected_stamina_raw(spec: &Spec) -> u32 {
    let (_, _, _, _, _, _, _, _, st0, st_up) = bases(spec.class);
    let ups = (spec.level - 1) as f32;
    ((st0 + ups * st_up) * 256.0) as u32
}

/// Les 36 champs de quête décodés d'une difficulté, à plat.
fn quest_fields(qd: &QuestDifficulty) -> Vec<QuestFlags> {
    vec![
        qd.warriv_intro,
        qd.journey_act1_act2,
        qd.jerhyn_intro,
        qd.journey_act2_act3,
        qd.hratli_intro,
        qd.journey_act3_act4,
        qd.act4_intro,
        qd.journey_act4_act5,
        qd.cain_act4,
    ]
    .into_iter()
    .chain(qd.act1)
    .chain(qd.act2)
    .chain(qd.act3)
    .chain(qd.act4)
    .chain(qd.act5)
    .collect()
}

// ---------------------------------------------------------------------------
// Intégrité des vecteurs
// ---------------------------------------------------------------------------

/// mini-SHA-256 embarqué (même implémentation FIPS 180-4 que real_javazon.rs,
/// testée contre sha256sum(1)).
fn sha256_hex(data: &[u8]) -> String {
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c,
        0x1f83d9ab, 0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
            (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);
        for i in 0..64 {
            const K: [u32; 64] = [
                0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1,
                0x923f82a4, 0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
                0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786,
                0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
                0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147,
                0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
                0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
                0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
                0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a,
                0x5b9cca4f, 0x682e6ff3, 0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
                0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
            ];
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
        h[5] = h[5].wrapping_add(f);
        h[6] = h[6].wrapping_add(g);
        h[7] = h[7].wrapping_add(hh);
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}

#[test]
fn oracle_vectors_are_intact() {
    for (disk, expected) in GOLDEN_SHA256 {
        let spec = SPECS.iter().find(|s| s.disk == *disk).unwrap();
        assert_eq!(sha256_hex(spec.bytes), *expected, "SHA-256 de {disk}");
    }
}

// ---------------------------------------------------------------------------
// Parse strict + en-tête
// ---------------------------------------------------------------------------

/// Parse strict de chaque save : signature, checksum, taille déclarée,
/// marqueurs de sections, en-tête conforme aux paramètres.
#[test]
fn all_oracle_saves_parse_strict_with_matching_header() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes)
            .unwrap_or_else(|e| panic!("{} : parse strict échoué : {e}", spec.disk));
        let h = &save.header;

        // Signature / version / taille / checksum.
        assert_eq!(h.magic, 0xAA55_AA55, "{}", spec.disk);
        assert_eq!(h.version, 96, "{} : legacy 0x60, PAS D2R", spec.disk);
        assert_eq!(
            d2d2s::version_name(96),
            "v1.10+ (v1.12 = 0x60)",
            "{}",
            spec.disk
        );
        assert_eq!(
            h.declared_size as usize,
            spec.bytes.len(),
            "{} : taille déclarée",
            spec.disk
        );
        assert!(save.checksum_valid_for(spec.bytes), "{} : checksum", spec.disk);

        // Identité + mode + progression.
        assert_eq!(h.name(), spec.name, "{}", spec.disk);
        assert_eq!(h.class, spec.class, "{}", spec.disk);
        assert_eq!(h.level, spec.level, "{}", spec.disk);
        assert!(h.status.expansion, "{} : expansion (LoD)", spec.disk);
        assert_eq!(h.status.hardcore, spec.hardcore, "{}", spec.disk);
        assert!(!h.status.died && !h.status.ladder, "{}", spec.disk);
        assert_eq!(
            h.progression,
            spec.completions * 5,
            "{} : progression 0/5/10/15",
            spec.disk
        );
        assert_ne!(h.timestamp, 0, "{} : horodaté", spec.disk);

        // Difficulté courante : la difficulté SUIVANTE est active, acte 0
        // (Hell reste active après une fin Hell).
        let active = (spec.completions as usize).min(2);
        for (i, entry) in h.difficulty.iter().enumerate() {
            assert_eq!(
                entry.active,
                i == active,
                "{} : difficulté {i} active ?",
                spec.disk
            );
            if entry.active {
                assert_eq!(entry.act, 0, "{} : acte de départ", spec.disk);
            }
        }

        // Marqueurs de toutes les sections fixes.
        assert!(
            save.quests.marker_ok() && save.waypoints.marker_ok() && save.npc.marker_ok(),
            "{}",
            spec.disk
        );
        assert!(save.skills.marker_ok(), "{}", spec.disk);
        // En-têtes inconnus conformes aux valeurs canoniques.
        assert_eq!(save.quests.header_unknown(), &[6, 0, 0, 0, 0x2A, 0x01], "{}", spec.disk);
        assert_eq!(save.waypoints.header_unknown(), &[1, 0, 0, 0, 0x50, 0], "{}", spec.disk);
    }
}

// ---------------------------------------------------------------------------
// Quêtes — toutes complétées jusqu'à la difficulté paramétrée, rien au-delà
// ---------------------------------------------------------------------------

#[test]
fn oracle_quests_match_completed_difficulties() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes).unwrap();
        for d in 0..3u8 {
            let qd = save.quests.difficulty(d as usize).unwrap();
            let fields = quest_fields(&qd);
            assert_eq!(fields.len(), 36, "{} : 36 champs décodés", spec.disk);
            if d < spec.completions {
                // Toutes les quêtes de la difficulté sont complétées (bit 0),
                // SAUF le flag de voyage acte IV→V (champ 7 du layout
                // evidence, +62) : le jeu réel ET l'oracle le laissent à 0
                // même difficulté finie (0x0000 dans le pattern complété,
                // vérifié sur les 14 saves) — notre lecture de cet offset
                // reste brute, aucun impact round-trip.
                for (i, q) in fields.iter().enumerate() {
                    if i == 7 {
                        assert_eq!(
                            q.raw(),
                            0,
                            "{} : difficulté {d}, voyage IV→V laissé à 0 par le jeu",
                            spec.disk
                        );
                        continue;
                    }
                    assert!(
                        q.completed(),
                        "{} : difficulté {d}, champ {i} non complété (raw {:#06x})",
                        spec.disk,
                        q.raw()
                    );
                }
            } else {
                // Au-delà de la difficulté terminée : tout à zéro.
                for (i, q) in fields.iter().enumerate() {
                    assert_eq!(
                        q.raw(),
                        0,
                        "{} : difficulté {d}, champ {i} devrait être vide",
                        spec.disk
                    );
                }
            }
        }
        // Récompenses de quête sources des bonus (dernière difficulté
        // terminée, cf. README oracle) : Den of Evil / Radament's Lair /
        // The Fallen Angel (+4 skills), Lam Esen's Tome (+5 stats),
        // The Golden Bird (+20 vie), Prison of Ice (+10 résistances —
        // bit 7 = parchemin consommé, evidence §3).
        if spec.completions > 0 {
            let last = save.quests.difficulty((spec.completions - 1) as usize).unwrap();
            assert!(last.act1[0].completed(), "{} : Den of Evil", spec.disk);
            assert!(last.act2[0].completed(), "{} : Radament's Lair", spec.disk);
            assert!(last.act4[0].completed(), "{} : The Fallen Angel", spec.disk);
            assert!(last.act3[3].completed(), "{} : Lam Esen's Tome", spec.disk);
            assert!(last.act3[0].completed(), "{} : The Golden Bird", spec.disk);
            assert!(
                last.act5[1].raw() & 0x80 != 0,
                "{} : Prison of Ice (parchemin résistance) — +10 res",
                spec.disk
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Waypoints — bitfields par difficulté
// ---------------------------------------------------------------------------

#[test]
fn oracle_waypoints_match_completed_difficulties() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes).unwrap();
        for d in 0..3usize {
            let wp = save.waypoints.difficulty(d).unwrap();
            if d < spec.completions as usize {
                // Les 39 waypoints découverts (bitfield 5 octets, bits 0-38).
                assert_eq!(
                    wp.0 & 0x7F_FFFF_FFFF,
                    0x7F_FFFF_FFFF,
                    "{} : difficulté {d} devrait avoir les 39 WP",
                    spec.disk
                );
                assert_eq!(wp.0 >> 39, 0, "{} : pas de bits fantômes", spec.disk);
            } else {
                // Difficulté non entamée : seul Rogue Encampment (bit 0).
                assert_eq!(wp.0, 1, "{} : difficulté {d}", spec.disk);
            }
        }
        // Spot-check nommés sur un save HELL : Harrogath (bit 30) actif en
        // Normal (difficulté déjà finie) — l'ordre des bits de WAYPOINT_NAMES.
        if spec.completions == 3 {
            assert_eq!(
                save.waypoints.is_active(0, "Harrogath"),
                Some(true),
                "{}",
                spec.disk
            );
            assert_eq!(
                save.waypoints.is_active(0, "Rogue Encampment"),
                Some(true),
                "{}",
                spec.disk
            );
        }
        // Octet final inexpliqué (krisives : 81 octets) = 1, confirmé oracle.
        assert_eq!(save.waypoints.raw()[80], 1, "{} : octet final", spec.disk);
    }
}

// ---------------------------------------------------------------------------
// NPC — introductions / salutations par difficulté
// ---------------------------------------------------------------------------

#[test]
fn oracle_npc_intro_blocks_match_difficulties() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes).unwrap();
        let raw = save.npc.raw();
        // 6 blocs de 8 octets après "w4" + 0 : intros Normal/NM/Hell puis
        // salutations Normal/NM/Hell. Bloc « fini » = FF×5 puis 00×3.
        for i in 0..6usize {
            let diff_of_block = (i % 3) as u8; // 0=Normal, 1=NM, 2=Hell
            let done = diff_of_block < spec.completions;
            let block = &raw[3 + 8 * i..3 + 8 * i + 8];
            if done {
                assert_eq!(
                    block,
                    &[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0, 0],
                    "{} : bloc NPC {i} devrait être complet",
                    spec.disk
                );
            } else {
                assert!(
                    block.iter().all(|&b| b == 0),
                    "{} : bloc NPC {i} devrait être vide",
                    spec.disk
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Stats — cohérentes avec (classe, niveau, difficulté, or)
// ---------------------------------------------------------------------------

#[test]
fn oracle_stats_match_parameters() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes).unwrap();
        let s = &save.stats;

        // Layout « retail 1.14d » : préfixe « gf » + flux 1.10+ (divergence
        // tranchée — avant fix : UnknownStatId { id: 103 }).
        assert_eq!(
            s.format,
            StatsFormat::GfPrefixed110,
            "{} : stats gf-préfixées",
            spec.disk
        );

        let (str0, ene0, dex0, vit0, _, _, _, _, _, _) = bases(spec.class);
        let ups = spec.level - 1;
        let comp = spec.completions;

        // Entrées triées croissantes (exigence du flux 1.10+).
        let ids: Vec<u16> = s.entries.iter().map(|e| e.id).collect();
        let mut sorted = ids.clone();
        sorted.sort_unstable();
        assert_eq!(ids, sorted, "{} : IDs triés", spec.disk);

        // Présence : statpts/newskills/XP seulement si niveau > 1 ;
        // or = SEULEMENT la banque (id 15), jamais l'inventaire (id 14).
        assert_eq!(s.value(0), Some(str0), "{} : strength", spec.disk);
        assert_eq!(s.value(1), Some(ene0), "{} : energy", spec.disk);
        assert_eq!(s.value(2), Some(dex0), "{} : dexterity", spec.disk);
        assert_eq!(s.value(3), Some(vit0), "{} : vitality", spec.disk);

        if spec.level > 1 {
            // +5 stats et +4 skills par difficulté terminée (README oracle).
            assert_eq!(
                s.value(4),
                Some((ups as u32) * 5 + (comp as u32) * 5),
                "{} : statpts",
                spec.disk
            );
            assert_eq!(
                s.value(5),
                Some((ups as u32) + (comp as u32) * 4),
                "{} : newskills",
                spec.disk
            );
            assert_eq!(s.value(13), Some(xp_for_level(spec.level)), "{} : XP", spec.disk);
        } else {
            assert_eq!(s.value(4), None, "{} : niveau 1, pas de statpts", spec.disk);
            assert_eq!(s.value(5), None, "{} : niveau 1, pas de newskills", spec.disk);
            assert_eq!(s.value(13), None, "{} : niveau 1, pas d'XP", spec.disk);
        }

        // Vie/mana/stamina (raw ×256) — +20 vie par difficulté terminée.
        assert_eq!(s.raw(6), Some(expected_hp_raw(spec)), "{} : hitpoints", spec.disk);
        assert_eq!(s.raw(7), Some(expected_hp_raw(spec)), "{} : maxhp", spec.disk);
        assert_eq!(s.raw(8), Some(expected_mana_raw(spec)), "{} : mana", spec.disk);
        assert_eq!(s.raw(9), Some(expected_mana_raw(spec)), "{} : maxmana", spec.disk);
        assert_eq!(
            s.raw(10),
            Some(expected_stamina_raw(spec)),
            "{} : stamina",
            spec.disk
        );
        assert_eq!(
            s.raw(11),
            Some(expected_stamina_raw(spec)),
            "{} : maxstamina",
            spec.disk
        );

        assert_eq!(s.value(12), Some(spec.level as u32), "{} : level", spec.disk);
        assert_eq!(s.value(14), None, "{} : or inventaire absent", spec.disk);
        if spec.gold > 0 {
            assert!(spec.gold <= 2_500_000, "{} : plafond stash", spec.disk);
            assert_eq!(s.value(15), Some(spec.gold), "{} : or stash", spec.disk);
        } else {
            assert_eq!(s.value(15), None, "{} : pas d'or", spec.disk);
        }
    }
}

// ---------------------------------------------------------------------------
// Items — squelette « retail 1.14d » : JM+0 / JM+0 / jf nu / kf+0
// ---------------------------------------------------------------------------

#[test]
fn oracle_item_lists_match_retail_skeleton() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes).unwrap();
        assert_eq!(
            save.character_items.declared_count, 0,
            "{} : aucun item personnage",
            spec.disk
        );
        let corpse = save.corpse_items.as_ref().expect("corpse list présente");
        assert_eq!(corpse.declared_count, 0, "{} : corpse vide", spec.disk);
        // Merc : « jf » NU (divergence tranchée — avant fix :
        // BadMarker { expected: "JM", got: "kf" }).
        let merc = save.merc_items.as_ref().expect("section merc présente");
        assert_eq!(merc.declared_count, 0, "{} : merc vide", spec.disk);
        assert!(save.merc_bare, "{} : merc en forme nue sans count", spec.disk);
        let golem = save.golem.as_ref().expect("trailer kf présent");
        assert!(!golem.has_golem, "{} : pas de golem", spec.disk);
        assert!(save.d2r.is_none(), "{} : legacy, pas de liste D2R", spec.disk);
    }
}

// ---------------------------------------------------------------------------
// Round-trip — parse → write → octets identiques → parse identique
// ---------------------------------------------------------------------------

#[test]
fn oracle_round_trip_byte_exact_then_reparse_identical() {
    for spec in SPECS {
        let save = SaveFile::parse(spec.bytes)
            .unwrap_or_else(|e| panic!("{} : {e}", spec.disk));
        let written = save.to_bytes();
        assert_eq!(
            written.len(),
            spec.bytes.len(),
            "{} : taille round-trip",
            spec.disk
        );
        assert_eq!(written, spec.bytes, "{} : round-trip OCTET-À-OCTET", spec.disk);
        // Et la relecture des octets réécrits redonne la même structure.
        let back = SaveFile::parse(&written).unwrap();
        assert_eq!(back, save, "{} : re-parse identique", spec.disk);
    }
}

// ---------------------------------------------------------------------------
// Conversion struct interne (pont intégrateur)
// ---------------------------------------------------------------------------

#[test]
fn oracle_summary_conversion_spot_checks() {
    // Niveau 1 sans rien : stats minimales.
    let fresh = SaveFile::parse(
        &SPECS.iter().find(|s| s.disk == "AmaFresh.d2s").unwrap().bytes,
    )
    .unwrap();
    let s = fresh.to_summary();
    assert_eq!(s.name, "AmaFresh");
    assert_eq!(s.class_name, "Amazon");
    assert_eq!(s.level, 1);
    assert_eq!(s.strength, Some(20));
    assert_eq!(s.stat_points, None);
    assert_eq!(s.skill_points, None);
    assert_eq!(s.gold_bank, None);
    assert_eq!(s.max_hp, Some(50));
    assert!(!s.hardcore && s.expansion);

    // Fin HELL HC riche : bonus ×3, or au plafond, hardcore.
    let rich = SaveFile::parse(
        &SPECS.iter().find(|s| s.disk == "BarHellRich.d2s").unwrap().bytes,
    )
    .unwrap();
    let s = rich.to_summary();
    assert_eq!(s.name, "BarHellRich");
    assert_eq!(s.class_name, "Barbarian");
    assert_eq!(s.level, 99);
    assert!(s.hardcore);
    assert_eq!(s.stat_points, Some(98 * 5 + 3 * 5));
    assert_eq!(s.skill_points, Some(98 + 3 * 4));
    assert_eq!(s.gold_bank, Some(2_500_000));
    // 55 + 98×2 + 3×20 = 311 hp.
    assert_eq!(s.max_hp, Some(311));
    assert_eq!(s.experience, Some(3_520_485_254));
    assert_eq!(s.items, 0);
}

