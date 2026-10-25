//! Zones spéciales (étape 20b, PLAN-FIDÉLITÉ phase 20, diablo-2-wasm).
//!
//! Deux niveaux secrets montés sur le pipeline DS1 EXISTANT (chemin réel
//! v18 → [`crate::ds1_real`] → `compose::compose_real_town`), sans toucher
//! au cœur du moteur — le même schéma additif que `drlg_a45.rs`/`cube_ui.rs` :
//!
//! | id levels.txt | niveau | DrlgType | montage |
//! |---|---|---|---|
//! | 38 | Tristram | 2 preset | DS1 réel v1.12 **figé**, projeté (2,2)→(2,1) |
//! | 39 | Moo Moo Farm | 3 wilderness | canevas **généré** (fermes à clôtures) |
//!
//! NB : le « maze exact » prévu au même plan est REPORTÉ — l'exactitude
//! retail est couverte côté wilderness par `drlg_exact.rs` (étape 20a,
//! verdict chiffré dans `crates/d2game/REPORT.md` §20a).
//!
//! # 1. Tristram — preset figé réel
//!
//! Extraction MPQ LECTURE SEULE (StormLib, `tools/stormlib_read.py`) :
//! * `data\global\tiles\act1\tristram\tri_town4.ds1` (d2exp.mpq, listfile)
//!   → `assets/map/tristram.ds1` — 62 475 o, md5_8 `62162f44` ;
//! * `data\global\tiles\ACT1\Tristram\town.dt1` (d2data.mpq, sonde par
//!   hash — listfile strippé à 102 o) → `assets/map/tristram_town.dt1`
//!   — 1 560 481 o, md5_8 `d5792185`.
//!
//! Le brief citait `data\global\tiles\ACT1\Tristram\tristram.ds1` : ce
//! chemin N'EXISTE PAS dans la release digitale v1.12 (sonde sur les 4
//! MPQ = None) — la release porte la carte sous `tri_town4.ds1`, taille
//! 43×48 stockée (+1 = 44×49 réelles, piège v18), EXACTEMENT les
//! SizeX/SizeY du record levels.txt id 38 (43×48) — la bonne carte.
//!
//! En-tête réel : v18, **2 murs + 2 sols = 7 flux**. Le pipeline réel du
//! moteur ([`crate::ds1_real::decode_ds1_real`]) n'accepte que le schéma
//! complet (2,1) — 6 flux. [`tristram_engine_ds1`] projette donc le
//! preset vers (2,1) SANS PERTE DE MARCHABILITÉ :
//! * `wall1/orient1`, `wall2/orient2` : repris tels quels (valeurs brutes
//!   bit-packées prop/seq/style conservées) ;
//! * `floor` = `floor1` (+ `floor2` uniquement dans les cellules vides —
//!   ici 0 : les 33 cellules de `floor2` sont des variations de sol
//!   superposées, écart documenté) ;
//! * `shadow` : repris (jamais dessiné, comme le jeu).
//!
//! Répartition réelle des orientations (preuves de tests) : 426 murs
//! (palissades 1/2 : 283, arbres 14 : 120, feu 12 : 2, porte-warp 10 :
//! 1 — LE portail rouge de retour, cellule (34,16)), sol 2 156 cellules.
//! Le portail rouge sert de warp retour vers la Lande sanglante (bloc
//! manifest `warp`, cercle à (172,82) en sous-tuiles).
//!
//! # 2. Portail des vaches — chaîne moteur
//!
//! Recette réelle v1.12 (`patch_cubemain.txt` ligne 4 : « 1 Wirt's leg +
//! 1 town portal book -> portal to cow lvl ») — items VÉRIFIÉS dans les
//! tables réelles : `leg` = Wirt's Leg (`patch_weapons.txt`, code `leg`,
//! type club) et `tbk` = Town Portal Book (`patch_misc.txt`, code `tbk`).
//! Le hook cube existe depuis l'étape 11a (`d2rules::cube` L4 — sortie
//! non-item `portal: Some("Cow Portal")`, testée par golden) ; le
//! branchement gameplay manquant (« branchement gameplay restant » dans
//! `cube_ui.rs`) est fait ici, côté moteur, en additif :
//! [`GameCore::open_portal_from_transmute`] consomme la réponse JSON de
//! `cube_transmute` et ouvre le niveau 39.
//!
//! Moo Moo Farm (levels.txt id 39 : DrlgType 3, LvlType 2, 80×80,
//! planche mon1-3 = hellbovine, MonDen 800) n'a pas de DS1 figé dans la
//! release — le canevas est GÉNÉRÉ ([`moomoofarm_engine_ds1`]) : prairie
//! (tuiles réelles LvlType 2), anneau de roche, 4 enclos clôturés avec
//! portes (tuiles réelles town_fence, kinds 1/2), bosquets (kind 14),
//! taches de pierres, et la tuile porte-warp (kind 10, warp.dt1) du
//! portail rouge à l'entrée. Rolls via [`crate::drlg_exact::D2Seed`] —
//! le RNG retail exact porté par l'étape 20a. Les vaches spawnent par le
//! chemin historique du moteur (codes `HELLBOVINE` du bloc manifest —
//! les planches `mon1-3` réelles du record ne portent pas de sprite
//! extrait : les entités existent et combatennent, le dessin attend
//! l'extraction du DCC bovine, comme tout sprite non extrait).
//!
//! # 3. Montage / jouabilité
//!
//! * [`tristram_manifest_block`] / [`moomoofarm_manifest_block`] : les
//!   blocs `levels` du manifeste (briques de données, fusionnés dans
//!   `assets/manifest.json` et `demo/assets/manifest.json`) ;
//! * [`mount_special_levels`] : fusion programmatique (tests/hôtes) ;
//! * les DS1 « engine » (`assets/map/tristram-engine.ds1`,
//!   `assets/map/moomoofarm-engine.ds1`) sont les octets projeté/généré
//!   écrits une fois — le test d'or du module garantit octet-pour-octet
//!   que le fichier embarqué/produit reste le fichier servi (aucun
//!   drift), même contrat que bloodmoor.ds1 (généré par outil).
//! * Portail Cairn Stones → Tristram : [`GameCore::open_tristram_portal`]
//!   (proximité de l'ancre requise — cf. note écart : le repère visuel
//!   des pierres du DRLG seedé reste à poser) + [`GameCore::tristram_portal_json`]
//!   pour l'UI.
//!
//! # Écarts assumés (annoncés)
//! 1. `floor2` de Tristram : 33 cellules de variation de sol non
//!    superposées (le pipeline réel porte un seul flux de sol) — 1,5 %
//!    des 2 156 cellules, zéro impact collision.
//! 2. Ombre de Tristram vide dans la release (0 cellule) — reprise telle
//!    quelle, jamais dessinée (comportement retail).
//! 3. Ancre Cairn Stones : position moteur fixe (160,160) — le repère
//!    des pierres n'est pas encore posé par le DRLG wilderness (20a) ;
//!    le portail exige la proximité, pas un objet cliquable.
//! 4. Vaches : entités sans sprite (DCC bovine non extrait) — spawn,
//!    IA et combat au contrat ; le rendu suivra l'extraction du sprite.
//! 5. Le portail des vaches est à sens unique au niveau moteur (retour
//!    par la tuile porte-warp non branchée — la tuile existe sur le
//!    canevas, le warp retour attend le branchement objets portails).

use std::collections::HashMap;

use serde_json::{json, Value};

use crate::dt1::{Dt1Meta, Dt1Tile};

/// Id levels.txt de Tristram (Act 1 - Tristram, DrlgType 2, LvlType 11).
pub const TRISTRAM: u32 = 38;
/// Id levels.txt de Moo Moo Farm (Act 1 - Moo Moo Farm, DrlgType 3, LvlType 2).
pub const MOOMOOFARM: u32 = 39;

/// Clé manifeste du niveau Tristram.
pub const TRISTRAM_KEY: &str = "tristram";
/// Clé manifeste du niveau Moo Moo Farm.
pub const MOOMOOFARM_KEY: &str = "moomoofarm";

/// Asset DS1 projeté (2,1) servi au pipeline réel pour Tristram.
pub const TRISTRAM_ENGINE_ASSET: &str = "assets/map/tristram-engine.ds1";
/// Asset DS1 généré (2,1) servi au pipeline réel pour Moo Moo Farm.
pub const MOOMOOFARM_ENGINE_ASSET: &str = "assets/map/moomoofarm-engine.ds1";

/// Ancre du portail rouge des Cairn Stones (Lande sanglante, sous-tuiles)
/// — cf. écart 3 : repère visuel restant.
pub const CAIRN_PORTAL_AT: (f32, f32) = (160.0, 160.0);
/// Rayon d'ouverture du portail des Cairn Stones (sous-tuiles).
pub const CAIRN_PORTAL_R: f32 = 8.0;
/// Point d'entrée dans Tristram (centre de la carte, snap marchable).
pub const TRISTRAM_SPAWN: (f32, f32) = (128.0, 128.0);
/// Point d'entrée dans la ferme (centre, entre les enclos).
pub const MOOMOOFARM_SPAWN: (f32, f32) = (200.0, 200.0);

/// DS1 réel v1.12 (tri_town4.ds1, d2exp.mpq) — octets intacts.
static TRISTRAM_REAL: &[u8] = include_bytes!("../../../../assets/map/tristram.ds1");

/// `patch_weapons.txt` v1.12 (code `leg` = Wirt's Leg — preuve de test).
const PATCH_WEAPONS: &str = include_str!("../../../../assets/patch_weapons.txt");
/// `patch_misc.txt` v1.12 (code `tbk` = Town Portal Book — preuve de test).
const PATCH_MISC: &str = include_str!("../../../../assets/patch_misc.txt");
/// `patch_levels.txt` (records 38/39 — preuves de tests).
const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");

// ---------------------------------------------------------------------------
// Lecteur DS1 v18 générique (tous schémas numWalls/numFloors)
// ---------------------------------------------------------------------------

/// Un DS1 v18 décodé sans hypothèse sur (numWalls, numFloors) — les flux
/// sont repris en valeurs BRUTES (bit-pack prop/seq/style préservé).
struct V18Special {
    w: usize,
    h: usize,
    files: Vec<String>,
    /// Flux [wall, orient] alternés (2·numWalls flux).
    walls: Vec<Vec<i32>>,
    orients: Vec<Vec<i32>>,
    /// Flux de sols (numFloors flux).
    floors: Vec<Vec<i32>>,
    /// Flux d'ombre (jamais dessiné, repris tel quel).
    shadow: Vec<i32>,
}

fn parse_v18_special(name: &str, d: &[u8]) -> Result<V18Special, String> {
    let i32_at = |at: usize| -> Result<i32, String> {
        d.get(at..at + 4)
            .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .ok_or_else(|| format!("v18 {name} : tronqué @{at}"))
    };
    let version = i32_at(0)?;
    if version != 18 {
        return Err(format!("v18 {name} : version {version} != 18"));
    }
    let w = i32_at(4)?.max(0) as usize + 1; // piège v18 : réel = stocké + 1
    let h = i32_at(8)?.max(0) as usize + 1;
    let file_count = i32_at(20)?.max(0) as usize;
    let mut p = 24usize;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let end = d
            .get(p..)
            .and_then(|rest| rest.iter().position(|&b| b == 0))
            .ok_or_else(|| format!("v18 {name} : liste de fichiers tronquée"))?
            + p;
        files.push(String::from_utf8_lossy(&d[p..end]).into_owned());
        p = end + 1;
    }
    let num_walls = i32_at(p)?.max(0) as usize;
    let num_floors = i32_at(p + 4)?.max(0) as usize;
    p += 8;
    let n = w * h;
    let stream = |i: usize| -> Result<Vec<i32>, String> {
        let base = p + i * 4 * n;
        let mut cells = Vec::with_capacity(n);
        for k in 0..n {
            cells.push(i32_at(base + 4 * k)?);
        }
        Ok(cells)
    };
    let mut walls = Vec::with_capacity(num_walls);
    let mut orients = Vec::with_capacity(num_walls);
    for i in 0..num_walls {
        walls.push(stream(2 * i)?);
        orients.push(stream(2 * i + 1)?);
    }
    let mut floors = Vec::with_capacity(num_floors);
    for i in 0..num_floors {
        floors.push(stream(2 * num_walls + i)?);
    }
    let shadow = stream(2 * num_walls + num_floors)?;
    Ok(V18Special { w, h, files, walls, orients, floors, shadow })
}

/// Sérialise un DS1 v18 schéma complet (2,1) — 6 flux — port du §5 de
/// `tools/bloodmoor_build.py` (même layout binaire : en-tête 6×i32, liste
/// de fichiers, tag (2,1), puis 6 flux de W·H u32 little-endian).
#[allow(clippy::too_many_arguments)]
fn write_ds1_v21(
    w: usize,
    h: usize,
    files: &[String],
    wall1: &[i32],
    orient1: &[i32],
    wall2: &[i32],
    orient2: &[i32],
    floor: &[i32],
    shadow: &[i32],
) -> Vec<u8> {
    let n = w * h;
    assert_eq!(
        (wall1.len(), orient1.len(), wall2.len(), orient2.len(), floor.len(), shadow.len()),
        (n, n, n, n, n, n),
        "write_ds1_v21 : flux de tailles inégales"
    );
    let mut out = Vec::with_capacity(24 + files.iter().map(|f| f.len() + 1).sum::<usize>() + 8 + 24 * n);
    out.extend_from_slice(&18i32.to_le_bytes());
    out.extend_from_slice(&((w as i32) - 1).to_le_bytes());
    out.extend_from_slice(&((h as i32) - 1).to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&0i32.to_le_bytes());
    out.extend_from_slice(&(files.len() as i32).to_le_bytes());
    for f in files {
        out.extend_from_slice(f.as_bytes());
        out.push(0);
    }
    out.extend_from_slice(&2i32.to_le_bytes());
    out.extend_from_slice(&1i32.to_le_bytes());
    for layer in [wall1, orient1, wall2, orient2, floor, shadow] {
        for &v in layer {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
    out
}

/// Les 7 fichiers tg1 embarqués du DS1 réel (ordre = espace d'indices) —
/// résolus par le bloc manifeste tristram (mêmes stems).
pub const TRISTRAM_FILES: [&str; 7] = [
    "\\d2\\data\\global\\tiles\\act1\\barracks\\warp.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\objects.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\river.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\stonewall.dt1",
    "\\d2\\data\\global\\tiles\\act1\\town\\floor.dt1",
    "\\d2\\data\\global\\tiles\\act1\\tristram\\town.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\treegroups.dt1",
];

/// Projette le preset réel Tristram (2,2) vers le schéma complet (2,1)
/// consommé par le pipeline réel du moteur — cf. doc de module. FONCTION
/// PURE : mêmes octets en entrée ⇒ mêmes octets en sortie.
pub fn tristram_engine_ds1() -> Result<Vec<u8>, String> {
    let v18 = parse_v18_special("tristram", TRISTRAM_REAL)?;
    if v18.walls.len() != 2 || v18.floors.len() != 2 {
        return Err(format!(
            "tristram : schéma inattendu ({}w+{}f, attendu 2w+2f)",
            v18.walls.len(),
            v18.floors.len()
        ));
    }
    // Sol : floor1 dominant ; floor2 uniquement dans les blancs (écart 1).
    let n = v18.w * v18.h;
    let mut floor = v18.floors[0].clone();
    for (i, &f2) in v18.floors[1].iter().enumerate() {
        if i < n && (floor[i] & 0xFF) == 0 && (f2 & 0xFF) != 0 {
            floor[i] = f2;
        }
    }
    Ok(write_ds1_v21(
        v18.w,
        v18.h,
        &v18.files,
        &v18.walls[0],
        &v18.orients[0],
        &v18.walls[1],
        &v18.orients[1],
        &floor,
        &v18.shadow,
    ))
}

// ---------------------------------------------------------------------------
// Moo Moo Farm — génération du canevas (niveau 39, DrlgType 3)
// ---------------------------------------------------------------------------

/// Banque de tuiles de la ferme (tuiles réelles extraites, LvlType 2) :
/// (fichier local assets/map, tg1 embarqué dans le DS1 écrit). L'ordre =
/// espace d'indices ; `counts` sépare les plages par conteneur.
pub const MOOMOOFARM_BANK: [(&str, &str); 6] = [
    ("out_treegroups.dt1", "\\d2\\data\\global\\tiles\\act1\\outdoors\\treegroups.dt1"),
    ("out_stonewall.dt1", "\\d2\\data\\global\\tiles\\act1\\outdoors\\stonewall.dt1"),
    ("floor.dt1", "\\d2\\data\\global\\tiles\\act1\\town\\floor.dt1"),
    ("out_stones.dt1", "\\d2\\data\\global\\tiles\\act1\\outdoors\\stones.dt1"),
    ("town_fence.dt1", "\\d2\\data\\global\\tiles\\act1\\town\\fence.dt1"),
    ("warp.dt1", "\\d2\\data\\global\\tiles\\act1\\barracks\\warp.dt1"),
];

/// Cellule de mur bit-packée (schéma réel : prop bits 0-7, seq 8-13,
/// style 20-25) + code d'orientation séparé.
fn wall_cell(m: &Dt1Meta, prop: u8) -> i32 {
    (prop as i32) | ((m.seq & 0x3F) << 8) | ((m.style & 0x3F) << 20)
}

/// Candidats `(kind, style, seq) → [(index global, meta)]` sur les tuiles
/// de mur — même clé que `compose_real_town` (la règle du jeu).
fn wall_groups(metas: &[Dt1Meta]) -> HashMap<(i32, i32, i32), Vec<usize>> {
    let mut g: HashMap<_, Vec<usize>> = HashMap::new();
    for (k, m) in metas.iter().enumerate() {
        if m.kind != 0 {
            g.entry((m.kind, m.style, m.seq)).or_default().push(k);
        }
    }
    g
}

/// Candidats `(style, seq)` des sols (kind 0).
fn floor_groups(metas: &[Dt1Meta]) -> HashMap<(i32, i32), Vec<usize>> {
    let mut g: HashMap<_, Vec<usize>> = HashMap::new();
    for (k, m) in metas.iter().enumerate() {
        if m.kind == 0 {
            g.entry((m.style, m.seq)).or_default().push(k);
        }
    }
    g
}

/// Choix data-driven d'un groupe : plus grand groupe de `keys` respectant
/// le filtre `want` (jamais de hard-code de style/seq — la table fait foi).
fn largest_group<'a, K: Copy + Ord + std::hash::Hash>(
    groups: &'a HashMap<K, Vec<usize>>,
    range: std::ops::Range<usize>,
    want: impl Fn(&K) -> bool,
    what: &str,
) -> Result<(K, &'a Vec<usize>), String> {
    // Déterminisme : plus grand groupe, égalité tranchée par la clé
    // minimale (l'ordre d'itération d'une HashMap n'est pas stable).
    let mut best: Option<(K, &Vec<usize>)> = None;
    for (k, v) in groups.iter() {
        if !want(k) || !v.first().is_some_and(|&i| range.contains(&i)) {
            continue;
        }
        let better = match best {
            None => true,
            Some((bk, bv)) => v.len() > bv.len() || (v.len() == bv.len() && *k < bk),
        };
        if better {
            best = Some((*k, v));
        }
    }
    best.map(|(k, v)| (k, v))
        .ok_or_else(|| format!("moomoofarm : aucun groupe {what} dans la banque"))
}

/// Génère le canevas Moo Moo Farm (niveau 39, 80×80 du record) et le
/// sérialise en DS1 v18 schéma (2,1) — pipeline réel au chargement.
///
/// `tilesets`/`metas` = concaténation de [`MOOMOOFARM_BANK`] DANS L'ORDRE
/// ; `counts` = tuiles par conteneur (délimite les plages data-driven :
/// arbres/roche/grass/pierres/clôtures/warp). Rolls retail exacts via
/// [`crate::drlg_exact::D2Seed`] (étape 20a).
pub fn moomoofarm_engine_ds1(
    seed: u32,
    tilesets: &[Dt1Tile],
    metas: &[Dt1Meta],
    counts: &[usize],
) -> Result<Vec<u8>, String> {
    if counts.len() != MOOMOOFARM_BANK.len() {
        return Err(format!(
            "moomoofarm : {} comptes de conteneurs pour {} fichiers de banque",
            counts.len(),
            MOOMOOFARM_BANK.len()
        ));
    }
    if tilesets.len() != counts.iter().sum::<usize>() || metas.len() != tilesets.len() {
        return Err("moomoofarm : tuiles/metas/comptes désynchronisés".into());
    }
    let rec = crate::drlg_maze::level_record(MOOMOOFARM)
        .ok_or_else(|| format!("moomoofarm : niveau {MOOMOOFARM} absent de levels.txt"))?;
    if rec.drlg_type != 3 || rec.level_type != 2 {
        return Err(format!(
            "moomoofarm : record inattendu (DrlgType {}, LvlType {})",
            rec.drlg_type, rec.level_type
        ));
    }
    let (w, h) = (rec.size_x.max(8) as usize, rec.size_y.max(8) as usize);
    let n = w * h;

    // Plages par conteneur (ordre de la banque).
    let mut bases = Vec::with_capacity(counts.len() + 1);
    let mut acc = 0usize;
    for c in counts {
        bases.push(acc);
        acc += c;
    }
    bases.push(acc);
    let range = |i: usize| bases[i]..bases[i + 1];
    let [trees_r, rock_r, grass_r, stones_r, fence_r, warp_r] =
        [0usize, 1, 2, 3, 4, 5].map(range);

    let walls = wall_groups(metas);
    let floors = floor_groups(metas);

    // Groupes data-driven : clôtures (kinds 1/2 palissades réelles), bosquets
    // (kind 14), roche de bord (plus grand groupe de murs du conteneur),
    // porte-warp (kind 10), prairie (plus grand sol du conteneur floor),
    // pierres au sol (plus grand sol du conteneur stones).
    let (fence_key, fence_tiles) =
        largest_group(&walls, fence_r.clone(), |(k, _, _)| (1..=2).contains(k), "clôture (kind 1/2)")?;
    let (tree_key, tree_tiles) = largest_group(&walls, trees_r.clone(), |(k, _, _)| *k == 14, "arbre (kind 14)")?;
    let (rock_key, rock_tiles) = largest_group(&walls, rock_r.clone(), |_| true, "roche de bord")?;
    let (warp_key, warp_tiles) = largest_group(&walls, warp_r.clone(), |(k, _, _)| *k == 10, "porte-warp (kind 10)")?;
    let (grass_key, grass_tiles) = largest_group(&floors, grass_r.clone(), |_| true, "prairie")?;
    let (stone_key, stone_tiles) = largest_group(&floors, stones_r.clone(), |_| true, "pierres au sol")?;

    // --- canevas ------------------------------------------------------------
    let mut rng = crate::drlg_exact::D2Seed::new(seed);
    let mut floor = vec![0i32; n];
    let mut wall = vec![0i32; n];
    let mut orient = vec![0i32; n];

    let mut set_floor = |floor: &mut Vec<i32>, x: usize, y: usize, tiles: &[usize], salt: u8| {
        let k = tiles[(salt as usize) % tiles.len()];
        let m = &metas[k];
        let prop = floor_group_rank(&floors, (m.style, m.seq), k);
        floor[y * w + x] = (prop as i32) | ((m.seq & 0x3F) << 8) | ((m.style & 0x3F) << 20);
    };
    let mut set_wall = |wall: &mut Vec<i32>, orient: &mut Vec<i32>, x: usize, y: usize, key: (i32, i32, i32), tiles: &[usize], salt: u8| {
        let k = tiles[(salt as usize) % tiles.len()];
        wall[y * w + x] = wall_cell(&metas[k], prop_rank_of(tiles, k));
        orient[y * w + x] = key.0;
    };
    let _ = &mut set_floor;
    let _ = &mut set_wall;

    // Prairie partout (variation brassée par le RNG retail).
    for y in 0..h {
        for x in 0..w {
            set_floor(&mut floor, x, y, grass_tiles, rng.limited(64) as u8);
        }
    }

    // Anneau de roche (bord 2 tuiles — scellé).
    for y in 0..h {
        for x in 0..w {
            if x < 2 || y < 2 || x >= w - 2 || y >= h - 2 {
                set_wall(&mut wall, &mut orient, x, y, rock_key, rock_tiles, rng.limited(64) as u8);
            }
        }
    }

    // 4 enclos clôturés (périmètres) avec portes de 5 tuiles côté centre.
    let pens = [(12usize, 12usize, 30usize, 26usize), (48, 12, 66, 26), (12, 52, 30, 66), (48, 52, 66, 66)];
    let mut fence_cells = 0usize;
    for (pi, &(x0, y0, x1, y1)) in pens.iter().enumerate() {
        for y in y0..=y1 {
            for x in x0..=x1 {
                let on_edge = x == x0 || x == x1 || y == y0 || y == y1;
                if !on_edge {
                    continue;
                }
                // Porte : 5 tuiles au milieu du côté tourné vers le centre.
                let gate = match pi {
                    0 | 1 => y == y1 && (x0 + 7..x0 + 12).contains(&x), // sud
                    _ => y == y0 && (x0 + 7..x0 + 12).contains(&x),     // nord
                };
                if gate {
                    continue;
                }
                set_wall(&mut wall, &mut orient, x, y, fence_key, fence_tiles, fence_cells as u8);
                fence_cells += 1;
            }
        }
    }

    // Bosquets (arbres kind 14) — jamais sur un enclos, le ring ni l'entrée.
    for _ in 0..160 {
        let (x, y) = ((rng.limited((w - 8) as i32) as usize) + 4, (rng.limited((h - 8) as i32) as usize) + 4);
        let in_pen = pens.iter().any(|&(x0, y0, x1, y1)| {
            (x0.saturating_sub(1)..=x1 + 1).contains(&x) && (y0.saturating_sub(1)..=y1 + 1).contains(&y)
        });
        let near_entry = (x as i32 - w as i32 / 2).abs() < 5 && y > h - 14;
        if in_pen || near_entry || orient[y * w + x] != 0 {
            continue;
        }
        set_wall(&mut wall, &mut orient, x, y, tree_key, tree_tiles, rng.limited(64) as u8);
    }

    // Taches de pierres au sol (variation de prairie, jamais dans un enclos).
    for _ in 0..60 {
        let (x, y) = ((rng.limited((w - 6) as i32) as usize) + 3, (rng.limited((h - 6) as i32) as usize) + 3);
        let in_pen = pens.iter().any(|&(x0, y0, x1, y1)| (x0..=x1).contains(&x) && (y0..=y1).contains(&y));
        if in_pen || (x < 3 || y < 3 || x >= w - 3 || y >= h - 3) {
            continue;
        }
        set_floor(&mut floor, x, y, stone_tiles, rng.limited(64) as u8);
    }

    // Portail rouge (tuile porte-warp kind 10) à l'entrée sud du centre.
    let (px, py) = (w / 2, h - 8);
    set_wall(&mut wall, &mut orient, px, py, warp_key, warp_tiles, 0);

    let files: Vec<String> = MOOMOOFARM_BANK.iter().map(|(_, tg1)| tg1.to_string()).collect();
    Ok(write_ds1_v21(w, h, &files, &wall, &orient, &vec![0i32; n], &vec![0i32; n], &floor, &vec![0i32; n]))
}

/// Rang d'un sol `k` dans son groupe `(style, seq)` (le `prop` du jeu).
fn floor_group_rank(
    floors: &HashMap<(i32, i32), Vec<usize>>,
    key: (i32, i32),
    k: usize,
) -> u8 {
    floors
        .get(&key)
        .and_then(|v| v.iter().position(|&i| i == k))
        .unwrap_or(0) as u8
}

/// Rang d'une tuile dans sa liste de groupe (le `prop` du jeu).
fn prop_rank_of(group: &[usize], k: usize) -> u8 {
    group.iter().position(|&i| i == k).unwrap_or(0) as u8
}

// ---------------------------------------------------------------------------
// Blocs manifeste + fusion
// ---------------------------------------------------------------------------

/// Bloc `levels.tristram` du manifeste — DS1 projeté, 7 DT1 (ordre = liste
/// embarquée du DS1 réel), palette acte I, monstres retail (skeleton2/
/// goatman2/fallenshaman2 → codes sprites extraits), warp retour vers la
/// Lande sanglante posé SUR la tuile porte-warp du DS1 (cellule (34,16)
/// → sous-tuiles (172,82)).
pub fn tristram_manifest_block() -> Value {
    json!({
        "name": "Tristram",
        "nameDisplay": "Tristram (zone spéciale)",
        "levelsId": TRISTRAM,
        "ds1": TRISTRAM_ENGINE_ASSET,
        "dt1": [
            "assets/map/warp.dt1",
            "assets/map/out_objects.dt1",
            "assets/map/out_river.dt1",
            "assets/map/out_stonewall.dt1",
            "assets/map/floor.dt1",
            "assets/map/tristram_town.dt1",
            "assets/map/out_treegroups.dt1",
        ],
        "palette": "assets/map/pal_act1_pl2.dat",
        "monsters": ["SK", "GM", "FS"],
        "warp": {
            "to": "bloodmoor",
            "at": [172, 82],
            "r": 4,
            "entry": [CAIRN_PORTAL_AT.0, CAIRN_PORTAL_AT.1],
            "note": "tuile porte-warp du DS1 réel (cellule 34,16) — retour Lande sanglante, ancre Cairn Stones",
        },
        "generated": {
            "step": "20b",
            "note": "DS1 réel v1.12 (tri_town4.ds1, d2exp.mpq, md5_8 62162f44) projeté (2,2)→(2,1) — pipeline ds1_real/compose_real_town ; tristram-engine.ds1 = octets projetés (test d'or)",
        },
    })
}

/// Bloc `levels.moomoofarm` du manifeste — canevas généré (fermes à
/// clôtures), monstres retail hell bovine (codes du record mon1-3 ; les
/// sprites DCC bovine restent à extraire — entités sans rendu).
pub fn moomoofarm_manifest_block() -> Value {
    json!({
        "name": "Moo Moo Farm",
        "nameDisplay": "La Ferme des vaches (secrète)",
        "levelsId": MOOMOOFARM,
        "ds1": MOOMOOFARM_ENGINE_ASSET,
        "dt1": [
            "assets/map/out_treegroups.dt1",
            "assets/map/out_stonewall.dt1",
            "assets/map/floor.dt1",
            "assets/map/out_stones.dt1",
            "assets/map/town_fence.dt1",
            "assets/map/warp.dt1",
        ],
        "palette": "assets/map/pal_act1_pl2.dat",
        // 12 packs de vaches (planche réelle mon1-3 = hellbovine ; les
        // sprites DCC bovine restent à extraire — entités sans rendu).
        "monsters": ["HELLBOVINE", "HELLBOVINE", "HELLBOVINE", "HELLBOVINE", "HELLBOVINE",
                     "HELLBOVINE", "HELLBOVINE", "HELLBOVINE", "HELLBOVINE", "HELLBOVINE",
                     "HELLBOVINE", "HELLBOVINE"],
        "generated": {
            "step": "20b",
            "note": "canevas généré (niveau 39 DrlgType 3, 80×80, RNG retail D2Seed) : 4 enclos clôturés + porte-warp (portail rouge) ; chaîne d'ouverture = cube L4 (leg + tbk → Cow Portal) → GameCore::open_portal_from_transmute",
        },
    })
}

/// Insère les deux blocs spéciaux dans le manifeste (fusion additive —
/// les blocs existants ne sont jamais modifiés).
pub fn mount_special_levels(manifest: &mut Value) -> Result<(), String> {
    let obj = manifest
        .get_mut("levels")
        .and_then(|l| l.as_object_mut())
        .ok_or_else(|| "manifest : pas de bloc levels".to_string())?;
    obj.insert(TRISTRAM_KEY.to_string(), tristram_manifest_block());
    obj.insert(MOOMOOFARM_KEY.to_string(), moomoofarm_manifest_block());
    Ok(())
}

// ---------------------------------------------------------------------------
// Ponts GameCore (additifs — même pattern que npc_bridge/cube_ui)
// ---------------------------------------------------------------------------

impl crate::game::GameCore {
    /// État du portail rouge des Cairn Stones (UI) : position, rayon,
    /// proximité du héros. PUR — lisible avant `finalize`.
    pub fn tristram_portal_json(&self) -> String {
        let near = self
            .world
            .as_ref()
            .map(|w| {
                let h = w.hero();
                (h.x - CAIRN_PORTAL_AT.0).hypot(h.y - CAIRN_PORTAL_AT.1) <= CAIRN_PORTAL_R
            })
            .unwrap_or(false);
        json!({
            "level": TRISTRAM_KEY,
            "x": CAIRN_PORTAL_AT.0,
            "y": CAIRN_PORTAL_AT.1,
            "r": CAIRN_PORTAL_R,
            "heroNear": near,
            "note": "portail rouge des Cairn Stones → Tristram (niveau 38)",
        })
        .to_string()
    }

    /// Ouvre le portail rouge des Cairn Stones → Tristram (niveau 38).
    /// Le héros doit être à portée de l'ancre ([`CAIRN_PORTAL_AT`]) —
    /// `Err` fr lisible sinon (jamais de téléport silencieux).
    pub fn open_tristram_portal(&mut self) -> Result<crate::game::LevelSwitchInfo, String> {
        let hero = self.world.as_ref().ok_or("finalize first")?.hero();
        let d = (hero.x - CAIRN_PORTAL_AT.0).hypot(hero.y - CAIRN_PORTAL_AT.1);
        if d > CAIRN_PORTAL_R {
            return Err(format!(
                "Le portail des Cairn Stones est trop loin ({:.0} sous-tuiles).",
                d
            ));
        }
        self.switch_level(TRISTRAM_KEY, TRISTRAM_SPAWN.0, TRISTRAM_SPAWN.1)
    }

    /// Branchement gameplay du hook cube (étape 11a) : consomme la réponse
    /// JSON de [`crate::game::GameCore::cube_transmute`] — sur « Cow Portal »
    /// (recette L4 réelle : `leg` + `tbk`, objets consommés par le cube),
    /// ouvre Moo Moo Farm (niveau 39). `Err` fr pour tout autre résultat
    /// (aucune recette, autre portail, JSON illisible).
    pub fn open_portal_from_transmute(
        &mut self,
        transmute_json: &str,
    ) -> Result<crate::game::LevelSwitchInfo, String> {
        let v: Value = serde_json::from_str(transmute_json)
            .map_err(|e| format!("Réponse cube illisible : {e}"))?;
        let portal = v
            .get("portal")
            .and_then(|p| p.as_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if portal.contains("cow") {
            self.switch_level(MOOMOOFARM_KEY, MOOMOOFARM_SPAWN.0, MOOMOOFARM_SPAWN.1)
        } else if portal.is_empty() {
            Err("Aucun portail dans la transmutation (recette item ou échec).".into())
        } else {
            Err(format!(
                "Portail « {} » sans destination moteur (restant).",
                v.get("portal").and_then(|p| p.as_str()).unwrap_or("?")
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — données réelles, pipeline réel, chaîne moteur bout-en-bout
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dt1::decode_dt1_paired;
    use crate::game::GameCore;

    /// Racine du workspace (d2w-drlg) — `None` hors checkout complet.
    fn ws_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .map(|p| p.to_path_buf())
            .expect("racine workspace")
    }

    fn read_asset(rel: &str) -> Vec<u8> {
        std::fs::read(ws_root().join(rel))
            .unwrap_or_else(|e| panic!("{rel} : {e} (asset absent)"))
    }

    /// Écrit l'asset s'il est absent, sinon retourne son contenu — le test
    /// d'or garantit ensuite l'égalité octet-pour-octet (aucun drift).
    fn write_if_missing(rel: &str, bytes: &[u8]) {
        let path = ws_root().join(rel);
        if !path.exists() {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::write(&path, bytes).unwrap();
        }
    }

    /// Manifeste réel + blocs spéciaux fusionnés (texte JSON).
    fn special_manifest() -> String {
        let mut manifest: Value =
            serde_json::from_str(include_str!("../../../../assets/manifest.json")).unwrap();
        mount_special_levels(&mut manifest).unwrap();
        manifest.to_string()
    }

    /// Tilesets + metas + comptes de la banque ferme (fichiers réels).
    fn farm_bank() -> (Vec<Dt1Tile>, Vec<Dt1Meta>, Vec<usize>) {
        let mut tiles = Vec::new();
        let mut metas = Vec::new();
        let mut counts = Vec::new();
        for (local, _) in MOOMOOFARM_BANK {
            let bytes = read_asset(&format!("assets/map/{local}"));
            let mut paired = decode_dt1_paired(&bytes)
                .unwrap_or_else(|e| panic!("{local} : {e:?}"));
            counts.push(paired.len());
            tiles.extend(paired.iter().map(|(t, _)| t.clone()));
            metas.extend(paired.drain(..).map(|(_, m)| m));
        }
        (tiles, metas, counts)
    }

    /// Fournit tous les assets des niveaux spéciaux + bloodmoor + tables.
    fn provide_all(core: &mut GameCore, farm_ds1: &[u8]) {
        core.provide("assets/patch_levels.txt", read_asset("assets/patch_levels.txt"));
        core.provide("assets/map/pal_act1_pl2.dat", read_asset("assets/map/pal_act1_pl2.dat"));
        // bloodmoor (portail Cairn + chaîne vaches)
        core.provide("assets/map/bloodmoor.ds1", read_asset("assets/map/bloodmoor.ds1"));
        for f in [
            "out_treegroups.dt1",
            "out_stonewall.dt1",
            "floor.dt1",
            "out_stones.dt1",
            "out_river.dt1",
            "bloodmoor_cliff2.dt1",
            "bloodmoor_corner.dt1",
            "bloodmoor_cliff1.dt1",
        ] {
            core.provide(&format!("assets/map/{f}"), read_asset(&format!("assets/map/{f}")));
        }
        // tristram (blocs + DT1 réels)
        core.provide(TRISTRAM_ENGINE_ASSET, tristram_engine_ds1().unwrap());
        for f in [
            "warp.dt1",
            "out_objects.dt1",
            "out_river.dt1",
            "out_stonewall.dt1",
            "tristram_town.dt1",
            "out_treegroups.dt1",
            "town_fence.dt1",
        ] {
            core.provide(&format!("assets/map/{f}"), read_asset(&format!("assets/map/{f}")));
        }
        // ferme
        core.provide(MOOMOOFARM_ENGINE_ASSET, farm_ds1.to_vec());
    }

    fn bfs_reach(walk: &[bool], gw: usize, gh: usize, start: (usize, usize)) -> usize {
        let mut seen = vec![false; gw * gh];
        let (sx, sy) = crate::compose::find_nearby_walkable(walk, gw, gh, start.0, start.1, 24)
            .unwrap_or_else(|| panic!("aucune marche proche de ({},{})", start.0, start.1));
        seen[sy * gw + sx] = true;
        let mut q = std::collections::VecDeque::new();
        q.push_back((sx, sy));
        let mut count = 1usize;
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= gw as i64 || ny >= gh as i64 {
                    continue;
                }
                let ni = ny as usize * gw + nx as usize;
                if !seen[ni] && walk[ni] {
                    seen[ni] = true;
                    count += 1;
                    q.push_back((nx as usize, ny as usize));
                }
            }
        }
        count
    }

    /// Zone connexe depuis un point, mesurée sur la carte RÉELLEMENT
    /// chargée par le moteur (fenêtres publiques `collision()`/`grid_size()`).
    fn bfs_reach_core(g: &GameCore, start: (usize, usize)) -> usize {
        let (gw, gh) = g.grid_size();
        bfs_reach(
            &g.collision().iter().map(|&b| b != 0).collect::<Vec<_>>(),
            gw as usize,
            gh as usize,
            start,
        )
    }

    // --- données réelles ------------------------------------------------------

    /// Les items de la recette L4 existent dans les tables v1.12 réelles :
    /// `leg` (Wirt's Leg, patch_weapons) et `tbk` (Town Portal Book,
    /// patch_misc) — et le hook cube (étape 11a) produit bien « Cow Portal ».
    #[test]
    fn cow_recipe_items_are_real_table_codes() {
        // patch_weapons.txt : ligne « Wirt's Leg … code leg » (colonne 4).
        let leg_row = PATCH_WEAPONS
            .lines()
            .find(|l| l.starts_with("Wirt's Leg\t"))
            .expect("Wirt's Leg absent de patch_weapons.txt");
        let leg_code = leg_row.split('\t').nth(3).expect("colonne code");
        assert_eq!(leg_code, "leg", "code réel de Wirt's Leg");
        // patch_misc.txt : ligne « Town Portal Book » portant le code tbk.
        let tbk_row = PATCH_MISC
            .lines()
            .find(|l| l.starts_with("Town Portal Book\t"))
            .expect("Town Portal Book absent de patch_misc.txt");
        assert!(tbk_row.contains("\ttbk\t"), "code tbk sur la ligne real : {tbk_row}");
        // Le hook cube réel (chemin public du contrat) : leg + tbk → Cow
        // Portal (sortie non-item, items consommés) — via World, comme le
        // gameplay (`World::cube_transmute_slots`, étape 11a).
        let manifest: Value =
            serde_json::from_str(include_str!("../../../../assets/manifest.json")).unwrap();
        let mut w = d2rules::World::with_seed(vec![vec![false; 32]; 32], manifest, 7);
        let save = w.save();
        let mut sv: Value = serde_json::from_str(&save).unwrap();
        sv["hero"]["inv"][0] = json!({ "code": "leg", "name": "Wirt's Leg", "kind": "Weapon", "amount": 0 });
        sv["hero"]["inv"][1] = json!({ "code": "tbk", "name": "Town Portal Book", "kind": "Misc", "amount": 0 });
        w = d2rules::World::load(&sv.to_string())
            .map_err(|_| "world : save patchée illisible".to_string())
            .unwrap();
        let out = w
            .cube_transmute_slots(&[0, 1], 7)
            .expect("L4 doit matcher leg+tbk");
        let ov: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(ov["ok"], true);
        assert_eq!(ov["line"], 4, "recette L4 du fichier réel");
        assert_eq!(ov["portal"], "Cow Portal");
        assert_eq!(ov["consumed"], json!([0, 1]));
        assert!(ov["item"].is_null(), "portail = sortie non-item");
    }

    /// Records levels.txt des deux zones (preuves de table).
    #[test]
    fn special_levels_match_release_tables() {
        let tables = crate::levels::LevelTables::parse(PATCH_LEVELS).unwrap();
        let t = tables.get(TRISTRAM).expect("record 38");
        assert_eq!((t.drlg_type, t.level_type), (2, 11), "Tristram : preset, LvlType 11");
        assert_eq!((t.size_x, t.size_y), (43, 48), "Taille table = DS1 stocké");
        assert_eq!(t.waypoint, 255, "Tristram : pas de waypoint (255 = aucun)");
        let m = tables.get(MOOMOOFARM).expect("record 39");
        assert_eq!((m.drlg_type, m.level_type), (3, 2), "Ferme : wilderness LvlType 2");
        assert_eq!((m.size_x, m.size_y), (80, 80), "Ferme : 80×80");
        assert_eq!(m.mon, ["hellbovine", "hellbovine", "hellbovine"], "planche mon1-3");
        assert_eq!(m.waypoint, 255, "Ferme : pas de waypoint (255 = aucun)");
        assert_eq!(m.level_name, "Moo Moo Farm");
    }

    /// En-tête du DS1 réel (preuves octets) : 44×49 (43+1, 48+1), 7 fichiers
    /// embarqués, schéma 2w+2f, ombre vide.
    #[test]
    fn tristram_real_bytes_proofs() {
        let v18 = parse_v18_special("tristram", TRISTRAM_REAL).unwrap();
        assert_eq!((v18.w, v18.h), (44, 49), "44×49 = SizeX/Y+1 (piège v18)");
        assert_eq!(v18.walls.len(), 2);
        assert_eq!(v18.floors.len(), 2);
        assert_eq!(v18.files.len(), 7);
        for (f, expected) in v18.files.iter().zip(TRISTRAM_FILES) {
            assert_eq!(f, expected, "fichier embarqué réel");
        }
        // Distribution réelle des orientations (comptée sur les octets) :
        // 431 murs — 286 palissades (1/2), 122 arbres (14), 2 feus (12),
        // 1 porte-warp (10) : LE portail rouge.
        let count = |o: &Vec<i32>, k: i32| o.iter().filter(|v| (**v & 0xFF) as i32 == k && **v != 0).count();
        let (o1, o2) = (&v18.orients[0], &v18.orients[1]);
        assert_eq!(count(o1, 1) + count(o1, 2) + count(o2, 1) + count(o2, 2), 286);
        assert_eq!(count(o1, 14) + count(o2, 14), 122);
        assert_eq!(count(o1, 12) + count(o2, 12), 2);
        assert_eq!(count(o1, 10) + count(o2, 10), 1, "une seule porte-warp (portail rouge)");
        let warp_cell = o1
            .iter()
            .position(|v| (*v & 0xFF) as i32 == 10 && *v != 0)
            .expect("porte-warp en flux 1");
        assert_eq!((warp_cell % v18.w, warp_cell / v18.w), (34, 16), "cellule du portail rouge");
        assert!(v18.shadow.iter().all(|v| *v == 0), "ombre vide dans la release");
        // Sol dense : 2 156 cellules (floor1 ; floor2 n'en comble aucune).
        let f1_nz = v18.floors[0].iter().filter(|v| (**v & 0xFF) != 0).count();
        assert_eq!(f1_nz, 2_156);
        let fills = v18
            .floors[0]
            .iter()
            .zip(&v18.floors[1])
            .filter(|(a, b)| (**a & 0xFF) == 0 && (**b & 0xFF) != 0)
            .count();
        assert_eq!(fills, 0, "floor2 ne comble aucun blanc (écart 1 : 33 cellules superposées)");
    }

    /// Projection (2,2)→(2,1) : relue par le lecteur réel du moteur, sols
    /// et murs intégralement conservés, octets d'or stables.
    #[test]
    fn tristram_projection_roundtrip_and_golden_asset() {
        let engine = tristram_engine_ds1().unwrap();
        // Relecture par le PIPELINE RÉEL du moteur.
        let real = crate::ds1_real::decode_ds1_real(&engine)
            .unwrap_or_else(|e| panic!("engine ds1 : {e:?}"));
        assert_eq!((real.w, real.h), (44, 49));
        assert_eq!(real.files.len(), 7);
        assert_eq!(
            real.floor.iter().filter(|c| c.prop != 0).count(),
            2_156,
            "sol intégralement conservé"
        );
        let orients_nz = real
            .orient1
            .iter()
            .chain(&real.orient2)
            .filter(|v| **v != 0)
            .count();
        assert_eq!(orients_nz, 431, "426 murs flux 1 + 5 murs flux 2");
        assert_eq!(
            real.orient1.iter().filter(|v| **v == 10).count(),
            1,
            "le portail rouge survit à la projection"
        );
        // Test d'or : l'asset servi au pipeline = ces octets exacts.
        write_if_missing("assets/map/tristram-engine.ds1", &engine);
        assert_eq!(
            read_asset("assets/map/tristram-engine.ds1"),
            engine,
            "tristram-engine.ds1 dérivé du DS1 réel — pas de drift"
        );
    }

    // --- ferme ----------------------------------------------------------------

    /// Génération de la ferme : 80×80, déterministe, sensible à la graine,
    /// enclos/warp/présents, bords scellés — relue par le pipeline réel.
    #[test]
    fn moomoofarm_generation_proofs_and_golden_asset() {
        let (tiles, metas, counts) = farm_bank();
        let a = moomoofarm_engine_ds1(20261025, &tiles, &metas, &counts).unwrap();
        let b = moomoofarm_engine_ds1(20261025, &tiles, &metas, &counts).unwrap();
        assert_eq!(a, b, "déterministe (même graine ⇒ mêmes octets)");
        let c = moomoofarm_engine_ds1(20261026, &tiles, &metas, &counts).unwrap();
        assert_ne!(a, c, "graine différente ⇒ carte différente");
        // Test d'or : l'asset servi = la graine figée du module.
        write_if_missing("assets/map/moomoofarm-engine.ds1", &a);
        assert_eq!(
            read_asset("assets/map/moomoofarm-engine.ds1"),
            a,
            "moomoofarm-engine.ds1 (graine 20261025) — pas de drift"
        );
        // Relecture par le pipeline réel + structure.
        let real = crate::ds1_real::decode_ds1_real(&a).unwrap();
        assert_eq!((real.w, real.h), (80, 80));
        // Bords scellés : l'anneau est en murs (orient != 0).
        let is_wall = |x: usize, y: usize| real.orient1[y * 80 + x] != 0;
        for i in 0..80 {
            assert!(is_wall(i, 0) && is_wall(i, 79) && is_wall(0, i) && is_wall(79, i), "bord {i}");
        }
        // Portail rouge présent (kind 10) à l'entrée sud du centre.
        assert_eq!(real.orient1[72 * 80 + 40], 10, "tuile porte-warp en (40,72)");
        // Clôtures : densité d'enclos (4 périmètres ≈ 4×(19·2+15·2-5) ≈ 220).
        let fences = real
            .orient1
            .iter()
            .filter(|v| {
                let k = **v & 0xFF;
                k == 1 || k == 2
            })
            .count();
        assert!(fences >= 180, "{fences} cellules de clôture");
        // Prairie : sol dense (le canevas est une prairie, pas un labyrinthe).
        let floor_nz = real.floor.iter().filter(|c| c.prop != 0).count();
        assert!(floor_nz >= 80 * 80 - 700, "{floor_nz} cellules de sol");
    }

    // --- montage moteur : Tristram ---------------------------------------------

    /// Fidélité de composition (même appariement que le moteur) : le DS1
    /// projeté + les 7 DT1 réels → `compose_real_town` — TOUTES les cellules
    /// du preset trouvent leur candidat `(kind,style,seq)` (0 non résolu),
    /// 426+5 murs dessinés, le portail rouge bloque sa cellule.
    #[test]
    fn tristram_composition_fidelity() {
        let engine = tristram_engine_ds1().unwrap();
        let real = crate::ds1_real::decode_ds1_real(&engine).unwrap();
        let mut tiles = Vec::new();
        let mut metas = Vec::new();
        for local in [
            "warp.dt1",
            "out_objects.dt1",
            "out_river.dt1",
            "out_stonewall.dt1",
            "floor.dt1",
            "tristram_town.dt1",
            "out_treegroups.dt1",
        ] {
            let bytes = read_asset(&format!("assets/map/{local}"));
            let mut paired = decode_dt1_paired(&bytes).unwrap_or_else(|e| panic!("{local} : {e:?}"));
            tiles.extend(paired.iter().map(|(t, _)| t.clone()));
            metas.extend(paired.drain(..).map(|(_, m)| m));
        }
        let map = crate::compose::compose_real_town(&real, &tiles, &metas, 1);
        assert_eq!(map.stats.unresolved, 0, "cellules sans candidat — fidélité du preset");
        assert!(map.stats.drawn > 2_000, "{} tuiles dessinées", map.stats.drawn);
        // Le portail rouge (cellule (34,16)) est marqué HIDDEN dans le DS1
        // retail : le jeu y pose un portail DYNAMIQUE (quest), la cellule est
        // réservée mais marchable — le bloc tristram y ancre le warp retour.
        let warp_i = 16 * 44 + 34;
        assert_eq!(real.orient1[warp_i], 10, "porte-warp reservée en (34,16)");
        assert!(map.walk[82 * map.grid_w + 172], "cellule réservée marchable (portail dynamique retail)");
        // Les murs (kinds 1-15) bloquent ; la ville reste dense et ouverte.
        let ratio = map.walk.iter().filter(|w| **w).count() as f32 / map.walk.len() as f32;
        assert!((0.5..0.95).contains(&ratio), "ville dense marchable : {ratio:.2}");
    }

    /// Tristram monté sur le pipeline réel : GameCore boote DIRECTEMENT le
    /// niveau (manifeste fusionné), carte composée et marchable (fenêtres
    /// publiques du moteur), monstres retail spawnés, warp retour réel.
    #[test]
    fn tristram_mounts_and_is_walkable() {
        let mut g = GameCore::with_level(&special_manifest(), "BA", TRISTRAM_KEY).unwrap();
        provide_all(&mut g, &moomoofarm_asset_bytes());
        g.finalize().unwrap();

        // Niveau chargé : clé + id de table.
        let info: Value = serde_json::from_str(&g.level_info_json()).unwrap();
        assert_eq!(info["key"], TRISTRAM_KEY);
        let def = g.level.as_ref().expect("niveau chargé");
        assert_eq!(def.levels_id, Some(TRISTRAM));
        assert_eq!(def.ds1, TRISTRAM_ENGINE_ASSET);

        // Composition réelle du moteur : ~2 300+ tuiles posées (sols+murs
        // du preset, cf. tristram_composition_fidelity pour le 0 non résolu).
        let stats = g.compose_stats();
        assert!(stats.drawn > 2_000, "{} tuiles dessinées", stats.drawn);

        // Marchable : une grande zone connexe depuis le spawn (centre).
        let (gw, gh) = g.grid_size();
        assert_eq!((gw, gh), (220, 245), "grille = 44×49 tuiles × 5 sous-tuiles");
        let ratio = g.collision().iter().filter(|&&b| b != 0).count() as f32 / (gw * gh) as f32;
        assert!((0.10..0.90).contains(&ratio), "ratio de marche {ratio:.2}");
        let reach = bfs_reach_core(&g, (TRISTRAM_SPAWN.0 as usize, TRISTRAM_SPAWN.1 as usize));
        assert!(reach > 4_000, "zone connexe de {reach} sous-tuiles depuis le spawn");

        // Monstres retail (bloc manifeste).
        let codes: Vec<String> = serde_json::from_str(&g.monster_codes_json()).unwrap();
        assert_eq!(codes, ["SK", "GM", "FS"], "codes sprites extraits du bloc");
        // Portail de retour : posé sur la tuile warp réelle (172,82).
        let warp = def.warp.as_ref().expect("warp retour du bloc tristram");
        assert_eq!(warp.to, "bloodmoor");
        assert_eq!(warp.at, (172.0, 82.0));
    }

    // --- montage moteur : Moo Moo Farm ------------------------------------------

    fn moomoofarm_asset_bytes() -> Vec<u8> {
        let (tiles, metas, counts) = farm_bank();
        let bytes = moomoofarm_engine_ds1(20261025, &tiles, &metas, &counts).unwrap();
        write_if_missing("assets/map/moomoofarm-engine.ds1", &bytes);
        bytes
    }

    /// La ferme montée : GameCore charge le niveau 39, les vaches hell
    /// bovine spawnent (codes du bloc — planche réelle hellbovine), le
    /// centre du camp est marchable.
    #[test]
    fn moomoofarm_mounts_and_spawns_hell_bovines() {
        let mut g = GameCore::with_level(&special_manifest(), "BA", MOOMOOFARM_KEY).unwrap();
        provide_all(&mut g, &moomoofarm_asset_bytes());
        g.finalize().unwrap();

        let def = g.level.as_ref().expect("niveau chargé");
        assert_eq!(def.levels_id, Some(MOOMOOFARM));
        assert_eq!(def.ds1, MOOMOOFARM_ENGINE_ASSET);
        // Composition réelle du moteur : prairie dense (sols + clôtures).
        let stats = g.compose_stats();
        assert!(stats.drawn > 5_000, "{} tuiles dessinées", stats.drawn);

        let codes: Vec<String> = serde_json::from_str(&g.monster_codes_json()).unwrap();
        assert!(codes.len() >= 8, "{} vaches spawnées", codes.len());
        assert!(
            codes.iter().all(|c| c == "HELLBOVINE"),
            "toutes les vaches : {codes:?}"
        );
        let reach = bfs_reach_core(&g, (MOOMOOFARM_SPAWN.0 as usize, MOOMOOFARM_SPAWN.1 as usize));
        assert!(reach > 8_000, "prairie connexe de {reach} sous-tuiles");
    }

    // --- portails ---------------------------------------------------------------

    /// Portail rouge des Cairn Stones : trop loin ⇒ erreur fr lisible ; à
    /// portée ⇒ LevelSwitch vers Tristram, carte marchable.
    #[test]
    fn tristram_red_portal_from_cairn_stones() {
        let mut g = GameCore::with_level(&special_manifest(), "BA", "bloodmoor").unwrap();
        provide_all(&mut g, &moomoofarm_asset_bytes());
        g.finalize().unwrap();

        // JSON d'état lisible (UI) avant toute action.
        let portal: Value = serde_json::from_str(&g.tristram_portal_json()).unwrap();
        assert_eq!(portal["level"], TRISTRAM_KEY);
        assert_eq!(portal["heroNear"], false);

        // Trop loin : refus propre.
        g.world.as_mut().unwrap().place_hero_at(40.0, 40.0);
        let err = g.open_tristram_portal().unwrap_err();
        assert!(err.contains("trop loin"), "{err}");

        // À portée de l'ancre Cairn Stones : ouverture → Tristram.
        g.world.as_mut().unwrap().place_hero_at(CAIRN_PORTAL_AT.0, CAIRN_PORTAL_AT.1);
        let portal: Value = serde_json::from_str(&g.tristram_portal_json()).unwrap();
        assert_eq!(portal["heroNear"], true);
        let info = g.open_tristram_portal().unwrap();
        assert_eq!(info.key, TRISTRAM_KEY);
        assert!(!info.name.is_empty(), "nom affiché (TBL fr) : {}", info.name);
        let def = g.level.as_ref().unwrap();
        assert_eq!(def.levels_id, Some(TRISTRAM));
        // Arrivé dans Tristram : la carte est jouable (zone connexe).
        assert!(bfs_reach_core(&g, (TRISTRAM_SPAWN.0 as usize, TRISTRAM_SPAWN.1 as usize)) > 4_000);
    }

    /// Chaîne des vaches BOUT-EN-BOUT au niveau moteur : Wirt's Leg +
    /// Town Portal Book dans l'inventaire (items réels des tables) →
    /// `cube_transmute` (hook L4, items consommés) → `open_portal_from_
    /// transmute` → niveau 39 Moo Moo Farm, vaches hell bovine spawnées.
    #[test]
    fn cow_portal_end_to_end_at_engine_level() {
        let mut g = GameCore::with_level(&special_manifest(), "BA", "bloodmoor").unwrap();
        provide_all(&mut g, &moomoofarm_asset_bytes());
        g.finalize().unwrap();

        // 1. Les items réels entrent dans l'inventaire du héros (save v3 :
        //    patch des slots 0/1 puis rechargement — les codes sont ceux des
        //    tables réelles, prouvés par cow_recipe_items_are_real_table_codes).
        let save = g.world.as_ref().unwrap().save();
        let mut save_v: Value = serde_json::from_str(&save).unwrap();
        save_v["hero"]["inv"][0] = json!({
            "code": "leg", "name": "Wirt's Leg", "kind": "Weapon", "amount": 0
        });
        save_v["hero"]["inv"][1] = json!({
            "code": "tbk", "name": "Town Portal Book", "kind": "Misc", "amount": 0
        });
        g.load_world(&save_v.to_string()).expect("save patchée rechargée");
        let inv: Value = serde_json::from_str(&g.inventory_json()).unwrap();
        assert_eq!(inv[0]["code"], "leg");
        assert_eq!(inv[1]["code"], "tbk");

        // 2. HOOK (cube.rs L4, étape 11a) : la transmutation ouvre le portail
        //    et CONSOMME les deux items (patte + tome, comme le retail).
        let out = g.cube_transmute(r#"{"slots":[0,1],"seed":7}"#).expect("L4 doit matcher");
        let out_v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(out_v["ok"], true);
        assert_eq!(out_v["line"], 4);
        assert_eq!(out_v["portal"], "Cow Portal");
        assert_eq!(out_v["consumed"], json!([0, 1]));
        let inv: Value = serde_json::from_str(&g.inventory_json()).unwrap();
        assert!(inv[0].is_null() && inv[1].is_null(), "items consommés");

        // 3. EVENT → MAP : le branchement gameplay ouvre le niveau 39.
        let info = g.open_portal_from_transmute(&out).expect("portail des vaches");
        assert_eq!(info.key, MOOMOOFARM_KEY);
        assert!(!info.name.is_empty(), "nom affiché : {}", info.name);
        let def = g.level.as_ref().unwrap();
        assert_eq!(def.levels_id, Some(MOOMOOFARM));

        // 4. SPAWN : vaches hell bovine (moo) sur une prairie jouable.
        let codes: Vec<String> = serde_json::from_str(&g.monster_codes_json()).unwrap();
        assert!(codes.len() >= 8, "{} vaches", codes.len());
        assert!(codes.iter().all(|c| c == "HELLBOVINE"), "{codes:?}");
        assert!(bfs_reach_core(&g, (MOOMOOFARM_SPAWN.0 as usize, MOOMOOFARM_SPAWN.1 as usize)) > 8_000);
        // Le héros est bien posé (snap marchable) dans la ferme.
        let hero = g.world.as_ref().unwrap().hero();
        let (gw, _) = g.grid_size();
        let (tx, ty) = (hero.x as usize, hero.y as usize);
        assert!(
            g.collision()[ty * gw as usize + tx] != 0,
            "héros sur tuile marchable ({tx},{ty})"
        );
    }

    /// Rejets propres du branchement portail : réponse sans portail, autre
    /// portail, JSON illisible — jamais de changement de niveau silencieux.
    #[test]
    fn open_portal_rejects_non_cow_transmutes() {
        let mut g = GameCore::with_level(&special_manifest(), "BA", "bloodmoor").unwrap();
        provide_all(&mut g, &moomoofarm_asset_bytes());
        g.finalize().unwrap();
        let before = g.level.as_ref().unwrap().key.clone();

        // Transmutation sans recette → pas de portail.
        let err = g.open_portal_from_transmute(r#"{"ok":false,"message":"Aucune recette…"}"#).unwrap_err();
        assert!(err.contains("Aucun portail"), "{err}");
        // JSON illisible.
        assert!(g.open_portal_from_transmute("pas du json").unwrap_err().contains("illisible"));
        // Portail non câblé (Pandemonium Portal L150 — restant).
        let err = g
            .open_portal_from_transmute(r#"{"ok":true,"line":150,"portal":"Pandemonium Portal"}"#)
            .unwrap_err();
        assert!(err.contains("sans destination"), "{err}");
        // Le niveau courant n'a pas bougé.
        assert_eq!(g.level.as_ref().unwrap().key, before);
    }

    /// La banque de tuiles ferme est REALISTE : chaque groupe requis
    /// (clôtures 1/2, arbres 14, porte-warp 10) existe dans les DT1 réels
    /// — la sélection reste data-driven (jamais de hard-code de style).
    #[test]
    fn farm_bank_groups_are_data_driven() {
        let (_, metas, counts) = farm_bank();
        let walls = wall_groups(&metas);
        let mut bases = vec![0usize];
        for c in &counts {
            bases.push(bases.last().unwrap() + c);
        }
        let fence = walls
            .iter()
            .filter(|((k, _, _), v)| (1..=2).contains(k) && v.first().is_some_and(|&i| (bases[4]..bases[5]).contains(&i)));
        let trees = walls
            .iter()
            .filter(|((k, _, _), v)| *k == 14 && v.first().is_some_and(|&i| (bases[0]..bases[1]).contains(&i)));
        let warp = walls
            .iter()
            .filter(|((k, _, _), v)| *k == 10 && v.first().is_some_and(|&i| (bases[5]..bases[6]).contains(&i)));
        assert!(fence.count() > 0, "clôtures kind 1/2 dans town_fence");
        assert!(trees.count() > 0, "arbres kind 14 dans treegroups");
        assert!(warp.count() > 0, "porte-warp kind 10 dans warp.dt1");
    }


}

