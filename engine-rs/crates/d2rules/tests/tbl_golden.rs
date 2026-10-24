//! Golden tests TBL (W18) — les 3 tables fr v1.12 réelles décodées EXACTEMENT
//! comme les TSV de référence livrés par `d2w-re/tools/tbl_decode.py`
//! (copies or dans `assets/tbl/`, provenance + SHA-256 dans `assets/tbl/README.md`).
//!
//! Comparaison ENTRÉE PAR ENTRÉE : index, clé et texte pour les 9 372 chaînes
//! (5 391 string + 2 818 expansionstring + 1 163 patchstring), plus les méta
//! de l'evidence.md (nb actifs, clés uniques, slots supprimés, CRC observés,
//! UTF-8 à 100 %).

use d2rules::tbl::{Tbl, TblSet, TblError};

/// Fichiers binaires réels (copies brutes `assets/tbl/`).
const STRING_TBL: &[u8] = include_bytes!("../../../../assets/tbl/string.tbl");
const EXPANSIONSTRING_TBL: &[u8] = include_bytes!("../../../../assets/tbl/expansionstring.tbl");
const PATCHSTRING_TBL: &[u8] = include_bytes!("../../../../assets/tbl/patchstring.tbl");
/// TSV de référence `index<TAB>key<TAB>text` (échappements `\t` `\n` `\r` `\\`).
const STRING_TSV: &str = include_str!("../../../../assets/tbl/string.tsv");
const EXPANSIONSTRING_TSV: &str = include_str!("../../../../assets/tbl/expansionstring.tsv");
const PATCHSTRING_TSV: &str = include_str!("../../../../assets/tbl/patchstring.tsv");

/// Déséchappe un champ TSV (port inverse de `esc()` du python).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse le TSV de référence en `Vec<(index, clé, texte)>`.
fn parse_tsv(tsv: &str) -> Vec<(usize, String, String)> {
    let mut rows = Vec::new();
    for (line_no, line) in tsv.lines().enumerate() {
        if line_no == 0 {
            assert_eq!(line, "index\tkey\ttext", "en-tête TSV inattendu");
            continue;
        }
        let mut parts = line.splitn(3, '\t');
        let index: usize = parts.next().expect("colonne index").parse().expect("index numérique");
        let key = unescape(parts.next().expect("colonne key"));
        let text = unescape(parts.next().expect("colonne text"));
        rows.push((index, key, text));
    }
    rows
}

/// Compare une table décodée au TSV de référence, entrée par entrée.
/// Les clés dupliquées sont légales (105 occurrences de `x` dans
/// patchstring) : chaque occurrence est vérifiée par index ; la résolution
/// `get(key)` est vérifiée à la PREMIÈRE occurrence seulement (politique
/// documentée — la hash retail n'est pas reversée).
fn assert_matches_tsv(tbl: &Tbl, tsv: &str, name: &str) {
    let rows = parse_tsv(tsv);
    assert_eq!(tbl.len(), rows.len(), "{name} : nombre de chaînes");
    let mut seen = std::collections::HashSet::new();
    for (i, (index, key, text)) in rows.iter().enumerate() {
        assert_eq!(*index, i, "{name} : index séquentiels");
        assert_eq!(tbl.key_at(i), Some(key.as_str()), "{name} : clé #{i}");
        assert_eq!(
            tbl.get_index(i).as_deref(),
            Some(text.as_str()),
            "{name} : texte #{i} (clé {key:?})"
        );
        if seen.insert(key.clone()) {
            assert_eq!(
                tbl.get(key).as_deref(),
                Some(text.as_str()),
                "{name} : résolution par clé {key:?} (première occurrence)"
            );
        }
    }
}

#[test]
fn golden_string_tbl_matches_tsv() {
    let t = Tbl::from_bytes(STRING_TBL).expect("string.tbl décode");
    assert_matches_tsv(&t, STRING_TSV, "string.tbl");
}

#[test]
fn golden_expansionstring_tbl_matches_tsv() {
    let t = Tbl::from_bytes(EXPANSIONSTRING_TBL).expect("expansionstring.tbl décode");
    assert_matches_tsv(&t, EXPANSIONSTRING_TSV, "expansionstring.tbl");
}

#[test]
fn golden_patchstring_tbl_matches_tsv() {
    let t = Tbl::from_bytes(PATCHSTRING_TBL).expect("patchstring.tbl décode");
    assert_matches_tsv(&t, PATCHSTRING_TSV, "patchstring.tbl");
}

#[test]
fn golden_total_9372_strings() {
    let total = Tbl::from_bytes(STRING_TBL).unwrap().len()
        + Tbl::from_bytes(EXPANSIONSTRING_TBL).unwrap().len()
        + Tbl::from_bytes(PATCHSTRING_TBL).unwrap().len();
    assert_eq!(total, 9372, "total de chaînes fr v1.12 (mission W18)");
}

/// Méta evidence.md : chaînes, clés uniques, slots supprimés, CRC observés.
#[test]
fn golden_meta_matches_evidence() {
    let cases: [(&[u8], u16, usize, usize, u16); 3] = [
        // (fichier, chaînes, clés uniques, slots supprimés, crc16 observé)
        (STRING_TBL, 5391, 5099, 2, 10699),
        (EXPANSIONSTRING_TBL, 2818, 2788, 1, 14531),
        (PATCHSTRING_TBL, 1163, 1058, 8, 493),
    ];
    for (data, strings, unique, deleted, crc) in cases {
        let t = Tbl::from_bytes(data).expect("décodage");
        assert_eq!(t.num_strings, strings);
        assert_eq!(t.len(), strings as usize);
        assert_eq!(t.unique_keys(), unique, "clés uniques");
        assert_eq!((t.num_slots as usize) - (t.len()), deleted, "slots supprimés");
        assert_eq!(t.crc16, crc, "crc16 en-tête (informationnel)");
        assert!(t.data_start > 0);
        // Auto-cohérence : file_size == taille réelle (les 3 tables v1.12).
        assert_eq!(t.file_size_field as usize, data.len());
    }
}

/// Exemples d'evidence.md + clés consommées par le branchement W18.
#[test]
fn golden_spot_checks_fr() {
    let patch = Tbl::from_bytes(PATCHSTRING_TBL).unwrap();
    assert_eq!(patch.get_index(0).as_deref(), Some("Coupe-gorge de Bartuc"));
    assert_eq!(patch.get("Cutthroat1").as_deref(), Some("Coupe-gorge de Bartuc"));

    let set = TblSet::bundled_fra();
    // Niveaux (clés = colonne LevelName de patch_levels.txt v1.12) :
    assert_eq!(set.get("Rogue Encampment").as_deref(), Some("Le Camp des Rogues"));
    assert_eq!(set.get("Blood Moor").as_deref(), Some("La Lande sanglante"));
    // Monstres (clés = colonne NameStr de patch_monstats.txt = `name` manifest) :
    assert_eq!(set.get("Fallen").as_deref(), Some("Déchu"));
    assert_eq!(set.get("Zombie").as_deref(), Some("Zombie"));
    assert_eq!(set.get("QuillRat").as_deref(), Some("Rat-épic"));
    assert_eq!(set.get("Skeleton").as_deref(), Some("Squelette"));
    assert_eq!(set.get("FallenShaman").as_deref(), Some("Chaman Déchu"));
    assert_eq!(set.get("Brute").as_deref(), Some("Brute"));
    assert_eq!(set.get("Andariel").as_deref(), Some("Andarielle"));
    // Super-unique (noms mon_names W10) :
    assert_eq!(set.get("Bishibosh").as_deref(), Some("Bishibosh"));
    // Clé absente → None → repli appelant (documenté, testé).
    assert_eq!(set.get("LevelNameQuiNExistePas"), None);
    assert_eq!(set.get("ZKZKR"), None); // un code brut ne résout jamais
}

/// UTF-8 à 100 % sur les 3 tables (evidence.md) : toute chaîne décodée est
// du Rust `str` valide — le repli CP1252 n'a jamais été nécessaire. On
// vérifie le compteur implicite : re-décoder en UTF-8 strict réussit pour
/// les 9 372 chaînes.
#[test]
fn golden_utf8_everywhere() {
    for (data, name) in [
        (STRING_TBL, "string"),
        (EXPANSIONSTRING_TBL, "expansionstring"),
        (PATCHSTRING_TBL, "patchstring"),
    ] {
        let t = Tbl::from_bytes(data).expect(name);
        for (k, v) in t.iter() {
            // Un `str` Rust EST de l'UTF-8 valide par construction ; on
            // vérifie simplement qu'aucun U+FFFD de repli n'est présent.
            assert!(!k.contains('\u{FFFD}') && !v.contains('\u{FFFD}'),
                "{name} : repli CP1252 déclenché (clé {k:?}) — attendu UTF-8 100 %");
        }
    }
}

/// Le décodeur refuse proprement des entrées dégradées (jamais de panique).
#[test]
fn rejects_degraded_inputs() {
    assert!(matches!(Tbl::from_bytes(b""), Err(TblError::TooSmall { len: 0 })));
    assert!(matches!(Tbl::from_bytes(&[0u8; 20]), Err(TblError::TooSmall { len: 20 })));
    let mut bad_version = STRING_TBL.to_vec();
    bad_version[0x08] = 9;
    assert!(matches!(Tbl::from_bytes(&bad_version), Err(TblError::BadVersion { version: 9 })));
    // Troncature brutale : erreur bornée, pas de panic.
    let cut = &STRING_TBL[..STRING_TBL.len() / 2];
    assert!(Tbl::from_bytes(cut).is_err());
    let mut corrupt = STRING_TBL.to_vec();
    let n = corrupt.len();
    corrupt[n - 1] = 0xFF; // dernier octet de la dernière valeur : plus de NUL
    assert!(
        matches!(Tbl::from_bytes(&corrupt), Err(TblError::BadFraming { .. })),
        "cadrage NUL cassé détecté (l'index exact dépend de l'ordre des slots)"
    );
}
