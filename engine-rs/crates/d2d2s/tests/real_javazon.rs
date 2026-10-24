//! Test d'intégration sur le **fichier réel** Javazon.d2s fourni par Justin
//! (vecteur doré `vectors/real/Javazon.d2s`, 2 838 o,
//! SHA-256 `0361b4ad6cac44e1bfb31f7d6c26edab44c2dc3db216b33362b3bbd9bdc5dfc2`).
//!
//! C'est la **VALIDATION RÉELLE** de la campagne (cf.
//! `d2w-integ/docs/d2s-validation.md`) : sauvegarde **D2R v0x61** — stats
//! « gf » + liste 1.10+, items au layout D2R avec codes Huffman. Le fichier
//! est un mule de test édité (toutes les compétences à 20, XP pile au seuil
//! du niveau 99, or rond) : le champ `count` de la liste d'items y est
//! **périmé** (50 déclaré, 76 items réels) — le parseur D2R lit donc les
//! items en continu jusqu'au marqueur suivant.
//!
//! Tout est affirmé depuis le fichier lui-même : en-tête, stats, skills,
//! waypoints, items (codes/qualités/uniques/runewords identifiés par leurs
//! mods), et **round-trip octet-à-octet**.

use d2d2s::{d2r, d2r::QualityD2R, Class, SaveFile};

const GOLDEN: &[u8] = include_bytes!("vectors/real/Javazon.d2s");

/// Somme SHA-256 du vecteur (garde-fou contre toute modification accidentelle).
#[test]
fn golden_vector_is_intact() {
    // SHA-256 calculé une fois pour toutes (sha256sum) — constantes codées.
    let digest = sha256_hex(GOLDEN);
    assert_eq!(
        digest,
        "0361b4ad6cac44e1bfb31f7d6c26edab44c2dc3db216b33362b3bbd9bdc5dfc2"
    );
}

fn sha256_hex(data: &[u8]) -> String {
    // mini-SHA-256 embarqué (pas de dépendance) — implémentation de
    // référence FIPS 180-4, testée contre sha256sum(1).
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
fn real_javazon_parses_strict() {
    // strict : signature + checksum valides exigés.
    let save = SaveFile::parse(GOLDEN).expect("le fichier réel doit passer le parse strict");
    assert_eq!(save.header.name(), "LightningAmazon");
    assert_eq!(save.header.version, 97);
    assert_eq!(save.header.class, Class::Amazon);
    assert_eq!(save.header.level, 99);
}

#[test]
fn real_javazon_header_fields() {
    let save = SaveFile::parse(GOLDEN).unwrap();
    let h = &save.header;
    assert!(h.status.expansion, "personnage d'expansion (0x20)");
    assert!(!h.status.hardcore && !h.status.died && !h.status.ladder);
    assert_eq!(h.progression, 0);
    // difficulté : Normal actif acte 0, NM/Hell inactifs
    assert!(h.difficulty[0].active && h.difficulty[0].act == 0);
    assert!(!h.difficulty[1].active && !h.difficulty[2].active);
    assert!(h.is_v110_plus(), "0x61 >= 0x60");
    assert_eq!(
        d2d2s::version_name(97),
        "D2R / 0x61 (v1.14+ : stats « gf » + liste 1.10+, items Huffman)"
    );
    // marqueurs des sections fixes
    assert!(save.quests.marker_ok() && save.waypoints.marker_ok() && save.npc.marker_ok());
    // waypoints : Rogue Encampment (acte 1) visité au minimum
    let wp_bits = &save.waypoints.raw()[2..];
    assert!(wp_bits[0] & 1 == 1, "waypoint Rogue Encampment posé");
}

#[test]
fn real_javazon_stats_gf_prefixed() {
    let save = SaveFile::parse(GOLDEN).unwrap();
    // format D2R : « gf » + liste 1.10+ (l'« ID 103 » du parseur python =
    // les deux octets « gf » lus comme un ID).
    let s = &save.stats;
    assert_eq!(s.format, d2d2s::StatsFormat::GfPrefixed110);
    // stats du mule édité : caractéristiques hautes, XP pile au seuil 99
    assert_eq!(s.value(0), Some(855)); // strength
    assert_eq!(s.value(1), Some(959)); // energy
    assert_eq!(s.value(2), Some(904)); // dexterity
    assert_eq!(s.value(3), Some(949)); // vitality
    assert_eq!(s.value(4), None, "statpts = 0 → absent du flux");
    assert_eq!(s.value(5), None, "newskills = 0 → absent du flux");
    assert_eq!(s.raw(6), Some(899_097)); // hitpoints ×256
    assert_eq!(s.value(6), Some(3_512));
    assert_eq!(s.value(7), Some(2_837)); // maxhp
    assert_eq!(s.value(9), Some(1_430)); // maxmana
    assert_eq!(s.value(12), Some(99)); // level
    assert_eq!(s.value(13), Some(3_520_485_254)); // XP exacte du niveau 99
    assert_eq!(s.value(14), Some(990_000)); // gold
    assert_eq!(s.value(15), Some(2_500_000)); // goldbank
    // ordre du flux : IDs croissants 0..15 sans 4/5
    let ids: Vec<u16> = s.entries.iter().map(|e| e.id).collect();
    assert_eq!(ids, vec![0, 1, 2, 3, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
}

#[test]
fn real_javazon_skills_all_20() {
    let save = SaveFile::parse(GOLDEN).unwrap();
    assert!(save.skills.marker_ok());
    // mule éditée : les 30 slots Amazon (offset classe 6) sont à 20 points
    for slot in 0..30 {
        assert_eq!(save.skills.level(slot), 20, "slot {slot}");
    }
}

#[test]
fn real_javazon_items_are_d2r() {
    let save = SaveFile::parse(GOLDEN).unwrap();
    let list = save.d2r.as_ref().expect("items D2R présents pour v0x61");
    assert_eq!(list.declared_count, 50, "count PÉRIMÉ du fichier édité");
    // 78 items réels (enfants sertis inclus), lus jusqu'au corpse « JM ».
    assert_eq!(list.items.len(), 78);
    // liste corpse (JM + 0) puis merc/golem vides
    let corpse = save.corpse_d2r.as_ref().expect("marqueur corpse présent");
    assert_eq!(corpse.declared_count, 0);
    assert!(save.merc_d2r.as_ref().map_or(true, |m| m.is_empty()));
    let golem = save.golem_d2r.as_ref().expect("marqueur kf présent");
    assert!(!golem.0, "pas de golem de fer");
}

fn find<'a>(save: &'a SaveFile, code: &str) -> &'a d2r::ItemD2R {
    let list = save.d2r.as_ref().unwrap();
    list.items
        .iter()
        .find(|i| i.code == code)
        .unwrap_or_else(|| panic!("item {code} introuvable"))
}

#[test]
fn real_javazon_equipped_gear() {
    let save = SaveFile::parse(GOLDEN).unwrap();

    // Amulette : Highlord's Wrath (unique ID 276 → ligne 277 des tables)
    let amu = find(&save, "amu");
    assert!(amu.flags.identified() && amu.mode == 1 && amu.pos == 2, "équipée au cou");
    let ext = amu.extended.as_ref().unwrap();
    assert_eq!(ext.quality_data, QualityD2R::Unique(276));
    assert_eq!(ext.ilvl, 99);
    // mods signatures de Highlord's : +1 all skills (127), IAS (93), deadly/lvl
    let ids: Vec<u16> = ext.mods.iter().map(|m| m.id).collect();
    assert!(ids.contains(&127) && ids.contains(&93) && ids.contains(&250));

    // Anneau : Raven Frost (unique ID 275 → ligne 276) — CBF + absorb froid
    let rin = find(&save, "rin");
    assert!(rin.mode == 1 && rin.pos == 6, "équipé anneau droit");
    let ext = rin.extended.as_ref().unwrap();
    assert_eq!(ext.quality_data, QualityD2R::Unique(275));
    let ids: Vec<u16> = ext.mods.iter().map(|m| m.id).collect();
    assert!(ids.contains(&153), "cannot be frozen");
    assert!(ids.contains(&148), "absorb cold");
    // coldmindam (54) = paire 8+9+8 : 15-45 dmg, 100 frames de longueur
    let cold = ext.mods.iter().find(|m| m.id == 54).unwrap();
    assert_eq!(cold.raw & 0xFF, 15, "cold min");
    assert_eq!((cold.raw >> 8) & 0x3FF, 45, "cold max");
    assert_eq!((cold.raw >> 17) & 0xFF, 100, "cold length (frames)");

    // Casque : Griffon's Eye (diadème unique, ID 336 → ligne 337)
    let ci3 = find(&save, "ci3");
    let ext = ci3.extended.as_ref().unwrap();
    assert_eq!(ext.quality_data, QualityD2R::Unique(336));
    let ids: Vec<u16> = ext.mods.iter().map(|m| m.id).collect();
    assert!(ids.contains(&330) && ids.contains(&334), "lightning mastery/pierce");

    // Arme : Titan's Revenge (javelin unique ETH, empilé ×140)
    let amf = find(&save, "amf");
    assert!(amf.flags.ethereal());
    let ext = amf.extended.as_ref().unwrap();
    assert_eq!(ext.quality_data, QualityD2R::Unique(281)); // +1 → Titan's
    assert_eq!(ext.quantity, Some(140));
    assert_eq!(ext.max_durability, Some(6));
    let ids: Vec<u16> = ext.mods.iter().map(|m| m.id).collect();
    assert!(ids.contains(&188), "+2 tab javazon (addskill_tab p2)");
    assert!(ids.contains(&253) && ids.contains(&254), "replenish + extra stack");
}

#[test]
fn real_javazon_runewords() {
    let save = SaveFile::parse(GOLDEN).unwrap();

    // Enigma : Ghost Armor (uui) sup, indestructible, 3 sockets, RWid 59
    // (= Runeword33 + 26 : Jah+Ith+Ber) ; mods : +775 def, +14 LaK, +45% FRW,
    // +1 Téléport (nonclassskill p54), +2 skills, +str/lvl, +MF/lvl.
    let uui = find(&save, "uui");
    let ext = uui.extended.as_ref().unwrap();
    assert!(uui.flags.runeword() && uui.flags.socketed());
    assert_eq!(ext.sockets, Some(3));
    assert_eq!(ext.max_durability, Some(0), "indestructible (Zod)");
    assert_eq!(ext.runeword, Some((59, 5)));
    let rw: Vec<u16> = ext.rw_mods.as_ref().unwrap().iter().map(|m| m.id).collect();
    assert!(rw.contains(&31) && rw.contains(&96) && rw.contains(&97)
        && rw.contains(&127) && rw.contains(&220) && rw.contains(&240));
    let teleport = ext.rw_mods.as_ref().unwrap().iter().find(|m| m.id == 97).unwrap();
    assert_eq!(teleport.param, 54, "skill 54 = Teleport");
    assert_eq!(teleport.value, 1);
    // enfants = les 3 runes Jah/Ith/Ber, juste après l'armure
    let idx = save.d2r.as_ref().unwrap().items.iter().position(|i| i.code == "uui").unwrap();
    let items = &save.d2r.as_ref().unwrap().items;
    assert_eq!(items[idx + 1].code, "r31", "Jah");
    assert_eq!(items[idx + 2].code, "r06", "Ith");
    assert_eq!(items[idx + 3].code, "r30", "Ber");
    for r in &items[idx + 1..idx + 4] {
        assert!(r.flags.simple() && r.mode == 6, "rune sertie (location 6)");
    }

    // Call to Arms : Crystal Sword (crs), RWid 39 (= Runeword13 + 26,
    // Amn+Ral+Mal+Ist+Ohm) + 5 runes enfants.
    let crs = find(&save, "crs");
    let ext = crs.extended.as_ref().unwrap();
    assert_eq!(ext.runeword, Some((39, 5)));
    assert_eq!(ext.sockets, Some(5));
    // +Battle Command/Orders : nonclassskill avec params 146/149/155
    let skills: Vec<u32> = ext
        .rw_mods
        .as_ref()
        .unwrap()
        .iter()
        .filter(|m| m.id == 97)
        .map(|m| m.param)
        .collect();
    assert_eq!(skills, vec![146, 149, 155]);

    // Fortitude : Archon Plate (utp) sup ETH 4 sockets, RWid 67 (=41+26)
    let utp = find(&save, "utp");
    let ext = utp.extended.as_ref().unwrap();
    assert_eq!(ext.runeword, Some((67, 5)));
    assert!(utp.flags.ethereal());
    assert_eq!(ext.sockets, Some(4));
    assert_eq!(ext.max_durability, Some(60), "durabilité 60/60");
    assert_eq!(ext.durability, Some(60));
}

#[test]
fn real_javazon_uniques_and_charms() {
    let save = SaveFile::parse(GOLDEN).unwrap();

    // Annihilus : petite charme unique (ID 381 → ligne 382) — on cherche
    // l'unique parmi les cm1.
    let list = save.d2r.as_ref().unwrap();
    let annihilus = list
        .items
        .iter()
        .find(|i| i.code == "cm1" && i.extended.as_ref().map(|e| e.quality_raw) == Some(7))
        .expect("Annihilus présent");
    let e = annihilus.extended.as_ref().unwrap();
    assert!(matches!(e.quality_data, QualityD2R::Unique(381)));
    let ids: Vec<u16> = e.mods.iter().map(|m| m.id).collect();
    for expected in [0, 1, 2, 3, 39, 41, 43, 45, 85, 127] {
        assert!(ids.contains(&expected), "mod {expected} d'Annihilus");
    }

    // Hellfire Torch : grande charme unique (ID 400 → ligne 401)
    let torch = list
        .items
        .iter()
        .find(|i| i.code == "cm2" && i.extended.as_ref().map(|e| e.quality_raw) == Some(7))
        .expect("Torch présente");
    assert!(matches!(
        torch.extended.as_ref().unwrap().quality_data,
        QualityD2R::Unique(400)
    ));

    // Gheed's Fortune : grande charme unique (ID 359 → ligne 360) :
    // gold/magic/réduction de prix
    let gheed = list
        .items
        .iter()
        .find(|i| i.code == "cm3" && i.extended.as_ref().map(|e| e.quality_raw) == Some(7))
        .expect("Gheed présente");
    let ids: Vec<u16> =
        gheed.extended.as_ref().unwrap().mods.iter().map(|m| m.id).collect();
    assert!(ids.contains(&79) && ids.contains(&80) && ids.contains(&87));

    // 8 petites charmes « Shimmering … of Greed » : +5 à toutes les résistances
    let shimmering = list
        .items
        .iter()
        .filter(|i| i.code == "cm1" && i.extended.as_ref().map(|e| e.quality_raw) == Some(4))
        .count();
    assert_eq!(shimmering, 9);

    // Ceinture Razortail (zvb, ID 243 → ligne 244) : +33% pierce
    let zvb = find(&save, "zvb");
    let ids: Vec<u16> = zvb.extended.as_ref().unwrap().mods.iter().map(|m| m.id).collect();
    assert!(ids.contains(&156));

    // Bottes War Traveler (xtb unique ID 240 → ligne 241) — 2e xtb du flux
    let list = save.d2r.as_ref().unwrap();
    let wt = list
        .items
        .iter()
        .find(|i| i.code == "xtb" && i.extended.as_ref().map(|e| e.quality_raw) == Some(7))
        .expect("War Traveler présent");
    assert!(matches!(wt.extended.as_ref().unwrap().quality_data, QualityD2R::Unique(240)));
}

#[test]
fn real_javazon_round_trips_byte_exact() {
    let save = SaveFile::parse(GOLDEN).unwrap();
    let bytes = save.to_bytes();
    assert_eq!(bytes.len(), GOLDEN.len(), "taille identique");
    assert_eq!(bytes, GOLDEN, "round-trip octet-à-octet du fichier réel");
}

#[test]
fn real_javazon_summary() {
    let save = SaveFile::parse(GOLDEN).unwrap();
    let s = save.to_summary();
    assert_eq!(s.name, "LightningAmazon");
    assert_eq!(s.class_name, "Amazon");
    assert_eq!(s.level, 99);
    assert_eq!(s.experience, Some(3_520_485_254));
    assert_eq!(s.strength, Some(855));
    assert_eq!(s.max_hp, Some(2_837));
    assert_eq!(s.gold, Some(990_000));
    // items D2R comptés via le champ dédié
    assert_eq!(s.items, 78);
}
