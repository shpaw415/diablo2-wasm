//! FIX-CAMPES — validation du camp des Rousses sur les données réelles
//! (TownN1.ds1 + 8 conteneurs DT1), d'après le rapport archivé
//! `docs/rapports-d2w-fix/RAPPORT-FIX-cartes.md`. Auto-skip si les assets
//! du workspace sont absents.

mod common;

use d2game::compose;
use d2game::ds1_real;
use d2game::dt1;

fn root() -> String {
    // CARGO_MANIFEST_DIR = engine-rs/crates/d2game ; assets démo = ../../..
    let r = format!("{}/../../../demo", env!("CARGO_MANIFEST_DIR"));
    if std::path::Path::new(&format!("{r}/assets/map/TownN1.ds1")).exists() {
        r
    } else {
        String::new()
    }
}

fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

fn resolve_container(tg1: &str, level_dt1: &[String]) -> String {
    // même règle que game.rs::resolve_container (règle dossier d'abord)
    let norm = tg1.replace('/', "\\");
    let stem = norm
        .split('\\')
        .filter(|p| !p.is_empty())
        .last()
        .and_then(|base| base.split('.').next())
        .unwrap_or("")
        .to_ascii_lowercase();
    let segs: Vec<String> = norm
        .split('\\')
        .filter(|p| !p.is_empty())
        .map(|p| p.to_ascii_lowercase())
        .collect();
    let prefixes: &[&str] = if segs.iter().any(|s| s == "town") {
        &["town_"]
    } else if segs.iter().any(|s| s == "outdoors") {
        &["out_"]
    } else {
        &[]
    };
    let basenames: Vec<String> = level_dt1
        .iter()
        .map(|d| d.rsplit('/').next().unwrap_or(d).to_ascii_lowercase())
        .collect();
    for pfx in prefixes {
        if let Some(b) = basenames
            .iter()
            .find(|b| b.strip_suffix(".dt1").is_some_and(|s| s.strip_prefix(pfx) == Some(&stem)))
        {
            return b.clone();
        }
    }
    if let Some(b) = basenames.iter().find(|b| b.strip_suffix(".dt1") == Some(&stem)) {
        return b.clone();
    }
    if let Some(b) = basenames
        .iter()
        .find(|b| b.strip_suffix(".dt1").is_some_and(|s| s.ends_with(&stem)))
    {
        return b.clone();
    }
    String::new()
}

#[test]
fn town1_real_data_renders_the_fortified_camp() {
    let root = root();
    if root.is_empty() {
        eprintln!("assets absents — skip");
        return;
    }
    let manifest: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap(),
    )
    .unwrap();
    let level = &manifest["level"];
    let dt1_list: Vec<String> = level["dt1"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_string())
        .collect();
    let ds1_bytes = std::fs::read(format!("{root}/{}", level["ds1"].as_str().unwrap())).unwrap();

    // 1. Schéma réel : (2,1), 6 flux, cellules bit-packées.
    let real = ds1_real::decode_ds1_real(&ds1_bytes).expect("TownN1 doit être au schéma réel v18");
    assert_eq!(real.files.len(), 8);
    assert_eq!(real.w * real.h, 2337);

    // 2. Lecture DT1 appariée + métadonnées (ordre = liste .tg1 du DS1).
    let mut tiles = Vec::new();
    let mut metas = Vec::new();
    let mut fence_start = None;
    for tg1 in &real.files {
        let container = resolve_container(tg1, &dt1_list);
        assert!(!container.is_empty(), "tg1 non résolu : {tg1}");
        let bytes = std::fs::read(format!("{root}/assets/map/{container}")).unwrap();
        let mut paired = dt1::decode_dt1_paired(&bytes).unwrap();
        assert!(!paired.is_empty(), "{container} vide");
        if container == "town_fence.dt1" {
            fence_start = Some(tiles.len());
        }
        tiles.extend(paired.iter().map(|(t, _)| t.clone()));
        metas.extend(paired.drain(..).map(|(_, m)| m));
    }

    // 3. Métadonnées de la première tuile town_fence (valeurs réelles
    //    validées : dir=1, type=1, style=0, seq=14).
    let f0 = &metas[fence_start.expect("town_fence présent dans le pool")];
    assert_eq!((f0.dir, f0.kind, f0.style, f0.seq), (1, 1, 0, 14));

    // 4. Composition réelle : camp fortifié, 0 cellule non résolue.
    let m = compose::compose_real_town(&real, &tiles, &metas, fnv1a("town"));
    assert_eq!(m.stats.unresolved, 0, "toutes les cellules doivent se résoudre");
    // Grille de marche : ratio marchable réaliste pour un camp ouvert
    // (0.55-0.90 — la forêt généralisée écrasait tout).
    let walk_ratio = m.walk.iter().filter(|&&w| w).count() as f64 / m.walk.len() as f64;
    assert!(
        (0.55..=0.90).contains(&walk_ratio),
        "ratio marchable {walk_ratio:.2} hors bornes"
    );
    // La porte-warp (Est, cellule ~49,21 → sous-tuile ~247,107) est praticable
    // à proximité : la sortie du camp existe.
    let warp = compose::find_nearby_walkable(&m.walk, m.grid_w, m.grid_h, 247, 107, 12);
    assert!(warp.is_some(), "aucune tuile marchable près de la porte-warp");

    // 5. Apparition du camp : centroïde des tentes (kind 15), marchable.
    let (sx, sy) = m.camp_spawn.expect("camp_spawn doit exister sur le layout réel");
    assert!(m.walkable(sx, sy), "spawn du camp ({sx},{sy}) non marchable");
    // Dans l'enveloppe des tentes x[24-39] y[8-15] (cellules) élargie :
    let cx = sx / 5;
    let cy = sy / 5;
    assert!(
        (18..=45).contains(&cx) && (2..=21).contains(&cy),
        "spawn ({cx},{cy}) cellule hors de la zone des tentes"
    );
}
