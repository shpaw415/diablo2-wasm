//! Étape 15a — sprites/animations MONSTRES (PLAN-FIDELITE phase 15).
//!
//! Golden sur les données RÉELLES extraites des MPQ v1.12 :
//!
//! 1. **Tokens spawnés ⊆ manifest** — l'union exacte des monstres que le port
//!    fait apparaître aujourd'hui est couverte par un bloc manifest
//!    `monsters`/`bosses` :
//!    * plans data-driven `monsters.rs` — planches `mon1..mon10` des niveaux
//!      du port (ids levels.txt 2/55/65/74/110/113/124) → `patch_monstats.txt`
//!      colonne `Code` ;
//!    * super-uniques dont la classe est dans une planche (fin de
//!      `plan_level_spawns_diff`) — dérivés de `superuniques.txt` ;
//!    * boss scriptés réellement spawnés par d2rules (quests_actes2 →
//!      DU/MP, quests_actes45 → 22/5P/0D/0F/0E, quests_finale → DI + vagues
//!      FS/GY/HP/DM/43 + 42) et kills événementiels acte I-II (CR/RD/SU/FW).
//! 2. **Fichiers non vides + décodables + composables** — chaque bloc mode de
//!    chaque token : COF parse (frames/dirs ≥ 1), chaque couche DCC décode
//!    (≥ 4 directions sauf DD 1-dir réel de DI/42), et `SpriteSet::compose`
//!    (le pipeline de sprite.rs) rend 4 directions de jeu avec des tailles/
//!    offsets non dégénérés.
//! 3. **Une tuile monstre par acte** — planche PNG (4 directions × 3 frames,
//!    palette PL2 réelle de l'acte) écrite dans docs/sprites-15a/.
//!
//! Choix data-driven documenté : blocs manifest `monsters`/`bosses` (le
//! registre existant des sprites, consommé tel quel par
//! `GameCore::finalize` → `build_entity_sets` → `SpriteSet::compose`), pas
//! une table embarquée ; codes UPPER (F0/IP/M5/OS/XX) car `resolve_code`
//! compare le code monstats upcasé aux clés de la map sprites.

mod common;

use d2game::monsters::{MonStats, SuperUniques};
use d2game::sprite::{SpriteSet, sprite_palette};
use d2dcc::{Cof, Dcc};
use serde_json::Value;
use std::collections::BTreeSet;

/// Ids levels.txt des niveaux du port avec planche (manifest `levels`,
/// champs `levelsId` ; town 1/103/109 sans planche = zones sûres).
const PORT_LEVELS_WITH_PLATES: [u32; 7] = [2, 55, 65, 74, 110, 113, 124];

/// Golden littéral des codes planches (dérivé une fois des tables réelles ;
/// le test data-driven ci-dessous le revalide contre les tables courantes).
const PLATE_CODES_GOLDEN: [&str; 22] = [
    "ZM", "FA", "SI", // 2 Blood Moor
    "SK", "SC", // 55 Stony Tomb 1
    "ZM", "SR", "MM", "SK", // 65 Ancient Tunnels
    "WR", "GM", "VA", // 74 Arcane Sanctuary
    "XX", "M5", "OS", "IP", "SK", "SI", "BH", // 110 Bloody Foothills
    "F0", "0B", "L3", // 113 Crystalline Cavern 1 (+ 124 : XX/0B/45 ci-dessous)
];

/// Codes planches de la 124 Halls of Vaught (découpe du golden littéral).
const PLATE_124_GOLDEN: [&str; 3] = ["XX", "0B", "45"];

/// Boss scriptés (spawn réel d2rules) + kills événementiels (acte I-II) —
/// codes sprite monstats ; AN est le boss de quête a1q6 (manifest bosses).
const QUEST_CODES: [&str; 18] = [
    "AN", // a1q6 Andariel
    "CR", "RD", "SU", "FW", // kills événementiels a1q2/a2q1/a2q3/a2q4
    "DU", "MP", // a2q5/a3q6 scriptés (quests_actes2)
    "22", "5P", "0D", "0F", "0E", // a4 Izual/Hephasto/Anciens (quests_actes45)
    "DI", "42", "43", "GY", "HP", "DM", // a4q2/a5 Baal + vagues (quests_finale)
];

fn manifest_value(root: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap(),
    )
    .unwrap()
}

/// Codes (UPPER) des blocs manifest `monsters` + `bosses`.
fn manifest_codes(manifest: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for key in ["monsters", "bosses"] {
        if let Some(list) = manifest.get(key).and_then(|m| m.as_array()) {
            for e in list {
                if let Some(c) = e.get("code").and_then(|c| c.as_str()) {
                    out.insert(c.to_ascii_uppercase());
                }
            }
        }
    }
    out
}

/// Union data-driven des codes spawnés : planches des niveaux du port
/// (`patch_levels.txt` mon1..mon10 → `patch_monstats.txt` Code) + classes de
/// super-uniques posées sur ces planches + codes de quêtes.
fn spawned_codes(root: &str) -> BTreeSet<String> {
    let monstats = MonStats::parse(&std::fs::read_to_string(format!(
        "{root}/assets/patch_monstats.txt"
    ))
    .unwrap())
    .unwrap();
    let levels = d2game::levels::LevelTables::parse(
        &std::fs::read_to_string(format!("{root}/assets/patch_levels.txt")).unwrap(),
    )
    .unwrap();
    let sup = SuperUniques::parse(
        &std::fs::read_to_string(format!("{root}/assets/superuniques.txt")).unwrap(),
    )
    .unwrap();

    let mut plates: BTreeSet<String> = BTreeSet::new();
    let mut plate_ids: Vec<Vec<String>> = Vec::new();
    for id in PORT_LEVELS_WITH_PLATES {
        let rec = levels.get(id).unwrap_or_else(|| panic!("id {id} absent de patch_levels"));
        plate_ids.push(rec.mon.clone());
        for tok in &rec.mon {
            let r = monstats
                .get(tok)
                .unwrap_or_else(|| panic!("token {tok} absent de patch_monstats"));
            plates.insert(r.code.to_ascii_uppercase());
        }
    }
    // Super-uniques réellement posés : classe DANS la planche d'un niveau du
    // port (même règle que plan_level_spawns_diff) — même code que la classe.
    let mut sus = BTreeSet::new();
    for su in &sup.recs {
        if plate_ids.iter().any(|mon| mon.iter().any(|t| *t == su.class)) {
            let r = monstats
                .get(&su.class)
                .unwrap_or_else(|| panic!("SU {} : classe {} absente", su.name, su.class));
            sus.insert(r.code.to_ascii_uppercase());
        }
    }
    assert!(
        sus.is_subset(&plates),
        "SU hors planches ? {sus:?} vs {plates:?}"
    );

    let mut all = plates.clone();
    for c in QUEST_CODES {
        all.insert(c.to_string());
    }
    all
}

/// Littéral complet attendu (planche 19 + 124 3 + quêtes 18, dédupliqué).
fn spawned_codes_golden() -> BTreeSet<String> {
    let mut s = BTreeSet::new();
    for c in PLATE_CODES_GOLDEN.iter().chain(PLATE_124_GOLDEN.iter()).chain(QUEST_CODES.iter()) {
        s.insert(c.to_string());
    }
    s
}

// ---------------------------------------------------------------------------
// 1. Tokens spawnés couverts par le manifest
// ---------------------------------------------------------------------------

#[test]
fn golden_spawned_tokens_match_tables() {
    let Some(root) = common::ws_root() else { return };
    assert_eq!(
        spawned_codes(&root),
        spawned_codes_golden(),
        "l'union planches+SUs+quêtes dérivée des tables a dérivé — mettre le golden à jour"
    );
}

#[test]
fn every_spawned_token_has_a_manifest_block() {
    let Some(root) = common::ws_root() else { return };
    let manifest = manifest_value(&root);
    let codes = manifest_codes(&manifest);
    let golden = spawned_codes_golden();
    let missing: Vec<_> = golden.difference(&codes).collect();
    assert!(
        missing.is_empty(),
        "tokens spawnés sans bloc manifest : {missing:?} (manifest : {codes:?})"
    );
}

/// Chaque bloc mode des tokens spawnés est structurellement complet :
/// COF + au moins une couche DCC, tous les chemins `assets/…` existent.
#[test]
fn spawned_tokens_blocks_are_well_formed() {
    let Some(root) = common::ws_root() else { return };
    let manifest = manifest_value(&root);
    let golden = spawned_codes_golden();
    let mut checked = 0usize;
    for key in ["monsters", "bosses"] {
        for e in manifest[key].as_array().unwrap() {
            let code = e["code"].as_str().unwrap().to_ascii_uppercase();
            if !golden.contains(&code) {
                continue; // gabarits hors périmètre 15a (compat)
            }
            let modes = e.get("modes").and_then(|m| m.as_object())
                .unwrap_or_else(|| panic!("{code}: pas de bloc modes"));
            assert!(!modes.is_empty(), "{code}: bloc modes vide");
            for (tag, b) in modes {
                let cof = b["cof"].as_str()
                    .unwrap_or_else(|| panic!("{code}:{tag}: pas de cof"));
                let raw = std::fs::read(format!("{root}/{cof}"))
                    .unwrap_or_else(|e| panic!("{code}:{tag}: {cof} illisible : {e}"));
                assert!(!raw.is_empty(), "{code}:{tag}: {cof} vide");
                let layers = b["layers"].as_object().expect("layers objet");
                assert!(!layers.is_empty(), "{code}:{tag}: aucune couche sprite");
                for (tok, p) in layers {
                    let p = p.as_str().unwrap();
                    let bytes = std::fs::read(format!("{root}/{p}"))
                        .unwrap_or_else(|e| panic!("{code}:{tag}/{tok}: {p} illisible : {e}"));
                    assert!(!bytes.is_empty(), "{code}:{tag}/{tok}: {p} vide");
                    // Deux formats de couches dans le retail v1.12 : DCC
                    // (standard) et DC6 (Mephisto, cas spécial volant —
                    // aucun DCC n'existe pour ses composantes).
                    if p.ends_with(".dcc") {
                        Dcc::decode(&bytes).unwrap_or_else(|e| panic!("{code}:{tag}/{tok}: {p}: {e:?}"));
                    } else if p.ends_with(".dc6") {
                        d2dc6::Dc6::decode(&bytes).unwrap_or_else(|e| panic!("{code}:{tag}/{tok}: {p}: {e}"));
                    } else {
                        panic!("{code}:{tag}/{tok}: extension inattendue {p}");
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 350, "peu de fichiers vérifiés ({checked}) — manifest tronqué ?");
}

// ---------------------------------------------------------------------------
// 2. Le pipeline sprite.rs/compose consomme les blocs tels quels
// ---------------------------------------------------------------------------

/// Set de sprites d'un bloc manifest mode, fichiers lus depuis `root`.
/// Les couches .dc6 (Mephisto) sont vérifiées d2dc6 mais PAS chargées dans
/// le SpriteSet : le pipeline sprite.rs/compose est DCC-only (choix 15a
/// documenté — branchement DC6 hors scope).
fn build_set(root: &str, code: &str, tag: &str, block: &Value) -> Option<SpriteSet> {
    let cof_path = block["cof"].as_str().unwrap();
    let cof = Cof::parse(&std::fs::read(format!("{root}/{cof_path}")).unwrap())
        .unwrap_or_else(|e| panic!("{cof_path}: {e:?}"));
    let mut set = SpriteSet::new(cof);
    let mut any_dcc = false;
    for (tok, p) in block["layers"].as_object().unwrap() {
        let p = p.as_str().unwrap();
        let bytes = std::fs::read(format!("{root}/{p}")).unwrap();
        if p.ends_with(".dc6") {
            // preuve que la donnée décode (d2dc6) ; non consommée par compose
            d2dc6::Dc6::decode(&bytes)
                .unwrap_or_else(|e| panic!("{code}:{tag}/{tok}: {p}: {e}"));
            continue;
        }
        let dcc = Dcc::decode(&bytes).unwrap_or_else(|e| panic!("{p}: {e:?}"));
        set.add_layer(tok, dcc);
        any_dcc = true;
    }
    let _ = (code, tag);
    any_dcc.then_some(set)
}

/// Toutes les couches DCC du set peignent 0 px (frame 0, direction du slot
/// 0) — stubs retail vides (DD des Anciens 0D/0E/0F, vague 43 : monstres
/// sans cadavre par design).
fn dcc_layers_all_blank(set: &SpriteSet) -> bool {
    set.layers.values().all(|d| {
        d.directions[0]
            .frames
            .iter()
            .all(|f| f.pixel_data.iter().all(|&b| b == 0))
    })
}

#[test]
fn compose_pipeline_consumes_every_spawned_mode() {
    let Some(root) = common::ws_root() else { return };
    let manifest = manifest_value(&root);
    let golden = spawned_codes_golden();
    let pal = sprite_palette(&std::fs::read(format!(
        "{root}/assets/map/pal_act1_pl2.dat"
    ))
    .unwrap());
    let mut modes_checked = 0usize;
    let mut dc6_only = Vec::new();
    for key in ["monsters", "bosses"] {
        for e in manifest[key].as_array().unwrap() {
            let code = e["code"].as_str().unwrap().to_ascii_uppercase();
            if !golden.contains(&code) {
                continue;
            }
            for (tag, b) in e["modes"].as_object().unwrap() {
                let Some(set) = build_set(&root, &code, tag, b) else {
                    // MP (Mephisto) : couches DC6 uniquement — vérifiées
                    // d2dc6 ci-dessus, compose DCC-only ne peut pas les
                    // consommer (choix 15a documenté au bloc manifest).
                    dc6_only.push(format!("{code}:{tag}"));
                    continue;
                };
                assert!(set.frames() >= 1, "{code}:{tag}: 0 frame");
                // 4 directions de jeu minimales (facing 0/4/8/12 — les DCC
                // monstres réels portent 8/16 dirs ; DD de DI/42 = 1 dir
                // réelle, slot 0 pour tous les facings).
                for facing in [0u8, 4, 8, 12] {
                    let Some(sf) = set.compose(facing, 0, &pal) else {
                        panic!("{code}:{tag}: compose(facing {facing}) = None");
                    };
                    assert!(sf.width > 0 && sf.height > 0, "{code}:{tag}: canvas vide");
                    assert!(
                        (sf.width as i64 * sf.height as i64) < 4_000_000,
                        "{code}:{tag}: canvas dégénéré {}x{}", sf.width, sf.height
                    );
                    let lit = sf.rgba.chunks_exact(4).filter(|p| p[3] > 0).count();
                    if lit == 0 {
                        // Toléré UNIQUEMENT pour les stubs DD vides du retail
                        // (donnée réelle : couches qui peignent 0 px).
                        assert!(
                            tag == "DD" && dcc_layers_all_blank(&set),
                            "{code}:{tag}: facing {facing} vide ({lit} px) et pas un stub DD retail"
                        );
                    } else {
                        assert!(lit > 8, "{code}:{tag}: facing {facing} quasi vide ({lit} px)");
                    }
                }
                modes_checked += 1;
            }
        }
    }
    assert!(
        modes_checked >= 150,
        "peu de modes composés ({modes_checked}) — manifest tronqué ?"
    );
    // MP : 5 modes DC6-only documentés, pas plus (toute nouvelle entrée
    // DC6-only doit être qualifiée avant d'être ajoutée ici).
    assert_eq!(
        dc6_only.len(),
        5,
        "modes DC6-only inattendus : {dc6_only:?}"
    );
    assert!(
        dc6_only.iter().all(|k| k.starts_with("MP:")),
        "DC6-only hors MP : {dc6_only:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Une tuile monstre par acte (palette PL2 réelle de l'acte)
// ---------------------------------------------------------------------------

/// (acte, code, mode, palette PL2) — les monstres couvrent les 5 actes du
/// port : planche a1 (Blood Moor), a2 (Stony Tomb), quête a3 (Endugu),
/// quête a4 (Diablo, Sanctuaire du Chaos), a5 (Baal, Chambre de la Pierre).
const ACT_TILES: [(&str, &str, &str, &str); 5] = [
    ("acte-I", "ZM", "WL", "assets/map/pal_act1_pl2.dat"),
    ("acte-II", "SK", "WL", "assets/map/pal_act2_pl2.dat"),
    ("acte-III", "FW", "SC", "assets/map/pal_act3_pl2.dat"),
    ("acte-IV", "DI", "A1", "assets/map/pal_act4_pl2.dat"),
    ("acte-V", "42", "WL", "assets/map/pal_act5_pl2.dat"),
];

#[test]
fn monster_tile_render_per_act() {
    let Some(root) = common::ws_root() else { return };
    let manifest = manifest_value(&root);
    let out_dir = format!("{root}/docs/sprites-15a");
    std::fs::create_dir_all(&out_dir).unwrap();

    // bloc (monsters|bosses) par code
    let find = |code: &str| -> Value {
        for key in ["monsters", "bosses"] {
            for e in manifest[key].as_array().unwrap() {
                if e["code"].as_str().unwrap().eq_ignore_ascii_case(code) {
                    return e.clone();
                }
            }
        }
        panic!("code {code} absent du manifest");
    };

    for (act, code, mode, pal_path) in ACT_TILES {
        let ent = find(code);
        let block = &ent["modes"][mode];
        assert!(
            !block.is_null(),
            "{act}: {code} sans mode {mode} (modes : {:?})",
            ent["modes"].as_object().map(|m| m.keys().collect::<Vec<_>>())
        );
        let set = build_set(&root, code, mode, block)
            .unwrap_or_else(|| panic!("{act}: {code}{mode} sans couche DCC (DC6-only ?)"));
        let pal = sprite_palette(&std::fs::read(format!("{root}/{pal_path}")).unwrap());

        // grille 4 directions × 3 frames (tuile monstre de l'acte)
        let facings = [0u8, 4, 8, 12];
        let frames = 3usize;
        let (cw, ch) = (256usize, 256usize);
        let mut rgba = vec![0u8; cw * facings.len() * ch * frames * 4];
        for y in 0..ch * frames {
            for x in 0..cw * facings.len() {
                let o = (y * cw * facings.len() + x) * 4;
                let v = if (x / 16 + y / 16) % 2 == 0 { 62 } else { 48 };
                rgba[o] = v;
                rgba[o + 1] = v;
                rgba[o + 2] = v + 10;
                rgba[o + 3] = 255;
            }
        }
        let mut total_lit = 0usize;
        for (fi, facing) in facings.iter().enumerate() {
            for frame in 0..frames {
                let Some(sf) = set.compose(*facing, frame, &pal) else {
                    panic!("{act}: {code}{mode} compose(facing {facing}, f{frame}) = None");
                };
                let base_x = fi * cw + (cw - sf.width as usize).min(cw) / 2;
                let base_y = frame * ch + ch - (sf.height as usize).min(ch);
                for y in 0..sf.height as usize {
                    for x in 0..sf.width as usize {
                        let si = (y * sf.width as usize + x) * 4;
                        if sf.rgba[si + 3] == 0 {
                            continue;
                        }
                        let (dx, dy) = (base_x + x, base_y + y);
                        if dx >= cw * facings.len() || dy >= ch * frames {
                            continue;
                        }
                        let o = (dy * cw * facings.len() + dx) * 4;
                        rgba[o..o + 4].copy_from_slice(&sf.rgba[si..si + 4]);
                        if sf.rgba[si + 3] > 0 {
                            total_lit += 1;
                        }
                    }
                }
            }
        }
        assert!(
            total_lit > 2_000,
            "{act}: tuile {code}{mode} quasi vide ({total_lit} px opaques)"
        );
        let path = format!("{out_dir}/monstre-{act}.png");
        d2game::png_mini::write_png_rgba(
            &path,
            (cw * facings.len()) as u32,
            (ch * frames) as u32,
            &rgba,
        )
        .unwrap();
        println!("{act}: {code}{mode} → {path} ({total_lit} px opaques)");
    }
}

// ---------------------------------------------------------------------------
// 4. Boot réel : finalize construit les sets de TOUS les tokens spawnés
// ---------------------------------------------------------------------------

#[test]
fn finalize_builds_sprite_sets_for_every_spawned_token() {
    let Some(root) = common::ws_root() else { return };
    let manifest = manifest_value(&root);
    let mut core = d2game::game::GameCore::new(
        &serde_json::to_string(&manifest).unwrap(),
        "BA",
    )
    .unwrap();
    // provide-all (comme full_playthrough) : tout asset "assets/…" existant.
    let mut paths: Vec<String> = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => {
                if s.starts_with("assets/") {
                    out.push(s.clone());
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(&manifest, &mut paths);
    for p in [
        "assets/patch_levels.txt",
        "assets/patch_monstats.txt",
        "assets/monumod.txt",
        "assets/superuniques.txt",
        "assets/MonLvl.txt",
    ] {
        paths.push(p.to_string());
    }
    for p in paths {
        if let Ok(bytes) = std::fs::read(format!("{root}/{p}")) {
            core.provide(&p, bytes);
        }
    }
    core.finalize().unwrap();

    // Base + TOUS les modes déclarés au manifest, par token spawné (GM sans
    // DD, SU sans A1 : les clés attendues suivent le bloc, pas une liste
    // fixe — les fichiers absents des MPQ ne sont pas un échec).
    let golden = spawned_codes_golden();
    let mut missing = Vec::new();
    for key in ["monsters", "bosses"] {
        for e in manifest[key].as_array().unwrap() {
            let code = e["code"].as_str().unwrap().to_ascii_uppercase();
            if !golden.contains(&code) {
                continue;
            }
            if core.sprite_set(&code).is_none() {
                missing.push(code.clone());
            }
            for tag in e["modes"].as_object().unwrap().keys() {
                let k = format!("{code}:{tag}");
                if core.sprite_set(&k).is_none() {
                    missing.push(k);
                }
            }
        }
    }
    assert!(
        missing.is_empty(),
        "sets manquants après finalize : {missing:?} (notes : {})",
        core.sprite_notes_json()
    );
}
