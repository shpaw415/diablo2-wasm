//! Golden étape 15b — modes COF additifs des 7 classes (SC/BL/GH/S1-S4/KK/
//! TH/A2) extraits des MPQ release v1.12 dans `assets/dcc/` (lecture SEULE
//! des données ; l'extraction vivait dans `tools/extract_modes_15b.py`).
//!
//! Pour chaque entrée `heroes[code].modes[MODE]` des 2 manifests :
//! 1. le COF existe et décode, ses comptes (frames/dirs) collent au manifest,
//! 2. chaque couche listée existe et DÉCODE, chaque frame de la direction 0
//!    porte au moins un pixel peint (« frame décodée non vide »),
//! 3. `missingLayers` est EXACT dans les deux sens (couche du COF sans
//!    fichier local non marquée = fail ; marquée sans raison = fail),
//! 4. les deux manifests (assets/ + demo/assets/) portent la même matrice.

use d2dcc::{Cof, Dcc};
use serde_json::Value;
use std::path::Path;

fn ws_root() -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap()
        .to_string_lossy()
        .into_owned()
}

/// (héros manifest, token de fichiers) — wclass de départ par classe
/// (charstats item1 ; BL sorcier en HTH, A2 barbare en 2HS, TH amazone en
/// 1HT — sonde tools/modes-15b-probe.json).
const MATRIX: [(&str, &str); 7] = [
    ("BA", "BA"),
    ("SOR", "SO"),
    ("PA", "PA"),
    ("AM", "AM"),
    ("AS", "AI"),
    ("DZ", "DZ"),
    ("NE", "NE"),
];

/// Modes 15b attendus PAR héros (mode, wc du COF) — la matrice livrée.
const EXPECTED: &[(&str, &[(&str, &str)])] = &[
    ("BA", &[("SC", "1HS"), ("BL", "1HS"), ("GH", "1HS"), ("S1", "1HS"), ("KK", "1HS"), ("A2", "2HS")]),
    ("SOR", &[("SC", "STF"), ("BL", "HTH"), ("GH", "STF"), ("S1", "STF"), ("KK", "STF")]),
    ("PA", &[("SC", "1HS"), ("BL", "1HS"), ("GH", "1HS"), ("S1", "1HS"), ("KK", "1HS")]),
    ("AM", &[("SC", "1HT"), ("BL", "1HT"), ("GH", "1HT"), ("S1", "1HT"), ("KK", "1HT"), ("TH", "1HT")]),
    ("AS", &[("SC", "HT1"), ("BL", "HT1"), ("GH", "HT1"), ("S1", "HT1"), ("S2", "HT1"), ("S3", "HT1"), ("S4", "HT1"), ("KK", "HT1")]),
    ("DZ", &[("SC", "1HS"), ("BL", "1HS"), ("GH", "1HS"), ("S1", "1HS"), ("KK", "1HS")]),
    ("NE", &[("SC", "1HS"), ("BL", "1HS"), ("GH", "1HS"), ("S1", "1HS"), ("KK", "1HS")]),
];

fn manifest(root: &str, rel: &str) -> Value {
    let text = std::fs::read_to_string(format!("{root}/{rel}")).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// Modes 15b (pas NU/WL/RN/A1/DD historiques — déjà couverts ailleurs).
fn is_15b(tag: &str) -> bool {
    matches!(tag, "SC" | "BL" | "GH" | "S1" | "S2" | "S3" | "S4" | "KK" | "TH" | "A2")
}

#[test]
fn modes_15b_matrice_complete_dans_les_deux_manifests() {
    let root = ws_root();
    for rel in ["assets/manifest.json", "demo/assets/manifest.json"] {
        let m = manifest(&root, rel);
        for (hero, modes) in EXPECTED {
            let block = &m.pointer(&format!("/heroes/{hero}/modes")).unwrap();
            for (tag, wc) in *modes {
                let entry = block.get(*tag).unwrap_or_else(|| panic!("{rel} {hero}/{tag} absent"));
                let cof = entry["cof"].as_str().unwrap();
                let expected_cof = format!("assets/dcc/{}{tag}{wc}.COF", hero_code(*hero));
                assert_eq!(cof, &expected_cof, "{rel} {hero}/{tag}: mauvais COF");
                assert!(Path::new(&root).join(&cof).is_file(), "{cof} absent");
                // couches présentes + missingLayers sans fichier local
                for (_, p) in entry["layers"].as_object().unwrap() {
                    assert!(Path::new(&root).join(p.as_str().unwrap()).is_file(), "{p} absent");
                }
                for t in entry["missingLayers"].as_array().unwrap() {
                    let t = t.as_str().unwrap();
                    assert!(
                        !Path::new(&root).join(format!("assets/dcc/{}{}{}.dcc", hero_code(*hero), tag, t)).is_file(),
                        "{hero}/{tag}: {t} marquée manquante mais le fichier existe"
                    );
                }
            }
        }
    }
}

fn hero_code(hero: &str) -> &'static str {
    MATRIX.iter().find(|(h, _)| *h == hero).map(|(_, c)| *c).unwrap()
}

#[test]
fn modes_15b_cof_decode_et_comptes_manifest() {
    let root = ws_root();
    let m = manifest(&root, "assets/manifest.json");
    for (hero, modes) in EXPECTED {
        for (tag, _) in *modes {
            let entry = m.pointer(&format!("/heroes/{hero}/modes/{tag}")).unwrap();
            let raw = std::fs::read(Path::new(&root).join(entry["cof"].as_str().unwrap())).unwrap();
            let cof = Cof::parse(&raw).unwrap();
            assert_eq!(
                cof.frames_per_direction,
                entry["frames"].as_u64().unwrap() as usize,
                "{hero}/{tag}: frames COF ≠ manifest"
            );
            assert_eq!(
                cof.number_of_directions,
                entry["dirs"].as_u64().unwrap() as usize,
                "{hero}/{tag}: dirs COF ≠ manifest"
            );
        }
    }
}

#[test]
fn modes_15b_frame_decodee_non_vide_par_mode() {
    let root = ws_root();
    let m = manifest(&root, "assets/manifest.json");
    for (hero, modes) in EXPECTED {
        for (tag, _) in *modes {
            let entry = m.pointer(&format!("/heroes/{hero}/modes/{tag}")).unwrap();
            // couche TR (torse toujours présent) : fenêtre sur le mode entier
            let tr = entry["layers"].get("TR").unwrap_or_else(|| panic!("{hero}/{tag}: TR absente"));
            let bytes = std::fs::read(Path::new(&root).join(tr.as_str().unwrap())).unwrap();
            let dcc = Dcc::decode(&bytes).unwrap_or_else(|e| panic!("{hero}/{tag} TR: {e:?}"));
            let painted: usize = dcc.directions[0]
                .frames
                .iter()
                .map(|f| f.pixel_data.iter().filter(|&&b| b != 0).count())
                .sum();
            assert!(painted > 0, "{hero}/{tag}: direction 0 sans pixel peint");
            let cof =
                Cof::parse(&std::fs::read(Path::new(&root).join(entry["cof"].as_str().unwrap()))
                    .unwrap())
                .unwrap();
            // comptes DCC ↔ COF : les couches corps en HTH (piège §3 du
            // skill — ex. AIS1 TR hth = 15 frames pour un COF de 17) peuvent
            // être PLUS COURTES que le COF au retail : l'engin clampe
            // (`SpriteSet::compose`), jamais de débordement. On exige donc
            // <= et au moins un pixel peint (exigence 15b).
            assert!(
                dcc.frames_per_direction <= cof.frames_per_direction,
                "{hero}/{tag}: frames DCC TR > COF ({} > {})",
                dcc.frames_per_direction, cof.frames_per_direction
            );
        }
    }
}

#[test]
fn modes_15b_missing_layers_exact_dans_les_deux_sens() {
    let root = ws_root();
    let m = manifest(&root, "assets/manifest.json");
    for (hero, modes) in EXPECTED {
        let code = hero_code(hero);
        for (tag, _) in *modes {
            let entry = m.pointer(&format!("/heroes/{hero}/modes/{tag}")).unwrap();
            let cof = Cof::parse(&std::fs::read(Path::new(&root).join(entry["cof"].as_str().unwrap())).unwrap())
                .unwrap();
            let cof_tokens: Vec<String> = cof
                .layers
                .iter()
                .map(|l| l.layer_type.token().to_string())
                .collect();
            let missing: Vec<String> = entry["missingLayers"]
                .as_array()
                .unwrap()
                .iter()
                .map(|t| t.as_str().unwrap().to_string())
                .collect();
            let layer_keys: Vec<String> = entry["layers"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect();
            for t in &cof_tokens {
                let local = Path::new(&root).join(format!("assets/dcc/{code}{tag}{t}.dcc"));
                if local.is_file() {
                    assert!(
                        layer_keys.contains(t),
                        "{hero}/{tag}: {t} présente au COF+disque mais pas dans layers"
                    );
                    assert!(!missing.contains(t), "{hero}/{tag}: {t} marquée manquante à tort");
                } else {
                    assert!(
                        missing.contains(t),
                        "{hero}/{tag}: {t} absente du disque mais non marquée"
                    );
                }
            }
            // couches hors-COF (LG) : fichier disque ⇒ présentes dans layers
            let lg = Path::new(&root).join(format!("assets/dcc/{code}{tag}LG.dcc"));
            if lg.is_file() {
                assert!(
                    layer_keys.contains(&"LG".to_string()),
                    "{hero}/{tag}: LG hors-COF au disque mais absente de layers"
                );
            }
        }
    }
}

#[test]
fn modes_15b_seulement_pas_de_regression_historique() {
    // Les modes historiques NU/WL/RN/A1/DD des 7 héros restent intacts.
    let root = ws_root();
    let m = manifest(&root, "assets/manifest.json");
    for hero in ["BA", "SOR", "PA", "AM", "AS", "DZ", "NE"] {
        let modes = m.pointer(&format!("/heroes/{hero}/modes")).unwrap();
        for tag in ["NU", "WL", "A1", "DD"] {
            let e = modes
                .get(tag)
                .unwrap_or_else(|| panic!("{hero}/{tag} historique disparu"));
            assert!(Path::new(&root).join(e["cof"].as_str().unwrap()).is_file(), "{hero}/{tag}");
        }
    }
}

#[test]
fn modes_15b_tous_tags_manifest_sont_conus() {
    // Aucune clé fantôme dans `modes` (le moteur ignore ce qu'il ne connaît
    // pas — build_entity_sets) : les 2 manifests ne listent que NU..A2.
    let root = ws_root();
    for rel in ["assets/manifest.json", "demo/assets/manifest.json"] {
        let m = manifest(&root, rel);
        for hero in ["BA", "SOR", "PA", "AM", "AS", "DZ", "NE"] {
            for tag in m
                .pointer(&format!("/heroes/{hero}/modes"))
                .unwrap()
                .as_object()
                .unwrap()
                .keys()
            {
                assert!(
                    matches!(tag.as_str(), "NU" | "WL" | "RN" | "A1" | "DD" | "SC" | "BL" | "GH" | "S1" | "S2" | "S3" | "S4" | "KK" | "TH" | "A2"),
                    "{rel} {hero}: tag modes inconnu `{tag}`"
                );
            }
        }
    }
}
