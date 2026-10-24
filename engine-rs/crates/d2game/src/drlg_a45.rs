//! DRLG fin de jeu A4-A5 (étape 10a — campagne diablo-2-wasm, arbre d2w-fin).
//!
//! Cinq niveaux de fin de jeu, montés sur le modèle des DRLG existants
//! ([`crate::drlg`] wilderness/preset W02, [`crate::drlg_maze`] maze 8a) :
//!
//! | id levels.txt | LevelName (table v1.12) | DrlgType | génération |
//! |---|---|---|---|
//! | 103 | The Pandemonium Fortress | 2 preset | DS1 preset réel `Fortress.ds1` (LvlPrest Def 797) |
//! | 109 | Harrogath | 2 preset | DS1 ville réel `townsiege.ds1` (81×49) + warps sortants |
//! | 110 | Bloody Foothills | 3 wilderness | canevas seedé 240×48 (SizeX/Y de la table) |
//! | 113 | Crystalized Cavern Level 1 (« Crystalline Passage ») | 1 maze | plan de rooms seedé (LvlMaze 10 rooms 16²) |
//! | 124 | Halls of Vaught | 2 preset | DS1 preset réel `Nihl{E,N,S,W}.ds1` (LvlPrest Def 864), variante seedée |
//!
//! # Sources (MPQ LECTURE SEULE → `assets/map/`)
//!
//! Extraction `tools/a45_extract.py` (`stormlib_read.py`, priorité
//! patch_d2 > d2exp > d2data) : les DT1 des LvlTypes 26/29/30/32/33, les
//! presets DS1 ci-dessus, les palettes PL2 (actes IV-V, cf. MAP-SPEC §7).
//! Harrogath : `townsiege.ds1` v1.12 déjà extrait par la campagne d2w-re
//! (`evidence-formats/ds1-path/evidence.md` §1 : v18, 81×49, acte V).
//! NB LvlPrest Def 863 nomme `townWest.ds1` (41×41, la moitié ouest jouée
//! en retail, SizeX/Y 40×40 de la table) ; l'étape 10a monte la carte
//! étendue `townsiege.ds1` — les deux sont des Harrogath v1.12 réels.
//!
//! # Modèle de couches v18 (le piège des villes/presets)
//!
//! Un DS1 v18 porte `numWalls`/`numFloors` puis `2·numWalls + numFloors + 1`
//! flux de W×H cellules (murs ALTERNÉS mur/orientation, puis sols, puis
//! ombre — `ds1-path/evidence.md` §2). Le décodeur historique
//! [`crate::ds1::decode_ds1`] lit 6 flux plats : juste pour les villes A1
//! (2w+1f), où le flux d'orientation tombe sur la couche 1 et ses petites
//! valeurs peignent des tuiles du PREMIER conteneur (town_floor : des
//! sols, donc inoffensives). townsiege/fortress/nihl ont 7 ou 11 flux
//! (et `expansion\siege` en tête de liste = des MURS) : la lecture naïve y
//! bloquerait des cellules de ville. Ce module lit donc l'en-tête complet
//! ([`parse_v18`]) et projette fidèlement vers le modèle 6 couches du
//! moteur ([`to_engine_layers`]) : murs → L0/L1 (convention TownN1),
//! sols → L4 (les sols surnuméraires ne remplissent que les blancs),
//! ombre → abandonnée (flags, pas des tuiles — L5 reste vide comme sur
//! toutes les cartes générées du moteur).
//!
//! # Espaces d'indices
//!
//! Presets (103/109/124) : les cellules réelles sont reprises TELLES QUELLES
//! (pas de remap) — l'ordre des fichiers de la banque = l'ordre embarqué du
//! DS1, exactement comme le chemin figé historique du town. Générés
//! (110/113) : p1 = indice direct dans la banque, plafond u8 = 256 tuiles
//! ([`USABLE_TILES`]) — la banque siège générée est un sous-ensemble
//! structurel de LvlType 30 (248 tuiles sur 779 ; la Blood Moor fait de
//! même avec 8 conteneurs choisis sur son LvlType).
//!
//! Garanties testées : déterminisme par graine, sensibilité à la graine,
//! dimensions vs tables (SizeX/Y exacts pour 110, piège v18 stocké+1 pour
//! les presets, grille LvlMaze pour 113), sol plein, 0 indice hors bornes,
//! marchabilité composée et ancres de warp sur tuile marchable.

use crate::ds1::Ds1;
use crate::dt1::Dt1Tile;
use crate::levels::{parse_lvl_prest, parse_lvl_types, LevelTables};

/// `patch_levels.txt` (levels.txt final v1.12).
const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");
/// `LvlTypes.txt` — File 1..32 : les DT1 disponibles par type de niveau.
const LVL_TYPES: &str = include_str!("../../../../assets/LvlTypes.txt");
/// `LvlPrest.txt` — presets DRLG (Def, DS1 référencés, Dt1Mask).
const LVL_PREST: &str = include_str!("../../../../assets/LvlPrest.txt");

// ---------------------------------------------------------------------------
// Ids de la campagne (levels.txt v1.12 vérifiés par tests)
// ---------------------------------------------------------------------------

/// The Pandemonium Fortress (acte IV, ville de départ).
pub const PANDEMONIUM: u32 = 103;
/// Harrogath (acte V, ville — townsiege.ds1).
pub const HARROGATH: u32 = 109;
/// Bloody Foothills (acte V, wilderness de siège).
pub const BLOODY_FOOTHS: u32 = 110;
/// Crystalized Cavern Level 1 — la « Crystalline Passage » (acte V, maze).
pub const CRYSTALLINE: u32 = 113;
/// Halls of Vaught (acte V, salle de boss Nihlathak, preset ×4).
pub const HALLS_VAUGHT: u32 = 124;

// ---------------------------------------------------------------------------
// Banques de tuiles (LvlTypes → DT1 extraits des MPQ)
// ---------------------------------------------------------------------------

/// Plafond u8 de l'espace p1 d'un DS1 généré (cellule vide = p1 0).
pub const USABLE_TILES: usize = 256;

/// Un fichier DT1 d'une banque :
/// * `stem` — le stem dans les colonnes `File N` de `LvlTypes.txt`
///   (vérification data-driven, [`bank_for_level`]) ;
/// * `local` — le conteneur extrait dans `assets/map/` ;
/// * `tg1` — le chemin embarqué dans le [`Ds1`] (résolu par
///   `game.rs::resolve_container` vers `local` via la liste `dt1` du bloc) ;
/// * `extra` — hors LvlTypes (fichiers act1 embarqués par Fortress.ds1 mais
///   absents de la liste du LvlType 26) : exemptés de la vérification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct A45File {
    pub stem: &'static str,
    pub local: &'static str,
    pub tg1: &'static str,
    pub extra: bool,
}

const fn af(stem: &'static str, local: &'static str, tg1: &'static str) -> A45File {
    A45File { stem, local, tg1, extra: false }
}

/// Variante `extra` : fichier embarqué par le DS1 preset hors LvlTypes.
const fn afx(stem: &'static str, local: &'static str, tg1: &'static str) -> A45File {
    A45File { stem, local, tg1, extra: true }
}

/// Une banque A4-A5 : ordre figé = espace d'indices. Pour les presets
/// (103/109/124) l'ordre = celui des fichiers embarqués du DS1 réel ; pour
/// les niveaux générés (110/113) un sous-ensemble ≤ [`USABLE_TILES`].
#[derive(Debug, Clone, Copy)]
pub struct A45Bank {
    /// Id `LevelType` du level record.
    pub level_type: u32,
    pub name: &'static str,
    pub files: &'static [A45File],
}

/// LvlType 26 « Act 4 - Town » — Pandemonium Fortress (103). Ordre = liste
/// embarquée de Fortress.ds1 (mesa floor/inv_wall/stairs, fort
/// foregate/plaza/interior/exterior, puis les 2 fichiers act1 embarqués).
pub const BANK_FORT: A45Bank = A45Bank {
    level_type: 26,
    name: "Act 4 - Town",
    files: &[
        af("floor", "a4mesa_floor.dt1", "\\d2\\data\\global\\tiles\\act4\\mesa\\floor.tg1"),
        af("inv_wall", "a4mesa_invwall.dt1", "\\d2\\data\\global\\tiles\\act4\\mesa\\inv_wall.tg1"),
        af("stairs", "a4mesa_stairs.dt1", "\\d2\\data\\global\\tiles\\act4\\mesa\\stairs.tg1"),
        af("foregate", "a4fort_forgate.dt1", "\\d2\\data\\global\\tiles\\act4\\fort\\foregate.tg1"),
        af("plaza", "a4fort_plaza.dt1", "\\d2\\data\\global\\tiles\\act4\\fort\\plaza.tg1"),
        af("interior", "a4fort_interior.dt1", "\\d2\\data\\global\\tiles\\act4\\fort\\interior.tg1"),
        af("exterior", "a4fort_exterior.dt1", "\\d2\\data\\global\\tiles\\act4\\fort\\exterior.tg1"),
        afx("warp", "warp.dt1", "\\d2\\data\\global\\tiles\\act1\\barracks\\warp.tg1"),
        afx("inviswal", "inviswal.dt1", "\\d2\\data\\global\\tiles\\act1\\barracks\\inviswal.tg1"),
    ],
};

/// LvlType 29 — Harrogath figé (109) : les 10 fichiers embarqués de
/// townsiege.ds1 (tous du tileset siège A5 ; trees.dt1 n'y est pas posé).
/// NB : cinq d'entre eux (rocks/camp/trench/ascendors/spike) appartiennent
/// au LvlType 30 « Siege » et non à la liste du LvlType 29 de la ville —
/// ils sont embarqués par le DS1 réel hors LvlTypes (marqués `extra`).
pub const BANK_TOWNSIEGE: A45Bank = A45Bank {
    level_type: 29,
    name: "Act 5 - Town",
    files: &[
        afx("rocks", "a5s_rocks.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\rocks.dt1"),
        afx("camp", "a5s_camp.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\camp.dt1"),
        af("fence", "a5s_fence.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\fence.dt1"),
        af("rockclifffloor", "a5s_rockclifffloor.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\rockclifffloor.dt1"),
        af("rockcliff", "a5s_rockcliff.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\rockcliff.dt1"),
        afx("trench", "a5s_trench.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\trench.dt1"),
        afx("ascendors", "a5s_ascendors.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\ascendors.dt1"),
        af("ground", "a5s_ground.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\ground.dt1"),
        afx("spike", "a5s_spike.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\spike.dt1"),
        af("shrub", "a5s_shrub.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\shrub.dt1"),
    ],
};

/// LvlType 30 « Act 5 - Siege » — Bloody Foothills GÉNÉRÉ (110). La banque
/// LvlType réelle fait 11 fichiers / 779 tuiles : hors espace u8 d'un DS1.
/// Sous-ensemble structurel (comme la Blood Moor et ses 8 conteneurs) :
/// sols neige `ground` (74 sols), sols-falaises `rockclifffloor` (92 sols),
/// parois `rocks` (28 murs) = 248 tuiles.
pub const BANK_SIEGE_WILD: A45Bank = A45Bank {
    level_type: 30,
    name: "Act 5 - Siege",
    files: &[
        af("ground", "a5s_ground.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\ground.dt1"),
        af("rockclifffloor", "a5s_rockclifffloor.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\rockclifffloor.dt1"),
        af("rocks", "a5s_rocks.dt1", "\\d2\\data\\global\\tiles\\expansion\\siege\\rocks.dt1"),
    ],
};

/// LvlType 33 « Act 5 - Ice Caves » — Crystalized Cavern 1 (113) : un seul
/// fichier réel `Expansion\icecave\interior.dt1` (288 tuiles — les pools
/// générés restent dans l'espace u8 via [`USABLE_TILES`]).
pub const BANK_ICE: A45Bank = A45Bank {
    level_type: 33,
    name: "Act 5 - Ice Caves",
    files: &[af(
        "interior",
        "a5ice_interior.dt1",
        "\\d2\\data\\global\\tiles\\expansion\\icecave\\interior.dt1",
    )],
};

/// LvlType 32 « Act 5 - Temple » — Halls of Vaught (124), Dt1Mask=1 :
/// `Expansion\wildtemple\interior.dt1` (173 tuiles) + `entrance.dt1`
/// (198 tuiles, marqué `extra`) — les presets Nihl adressent des indices
/// jusqu'à 209 > 173, la banque réelle du preset dépasse donc la liste
/// LvlTypes 32 v1.12 (ordre : interior d'abord = liste embarquée du DS1).
pub const BANK_WILDTEMPLE: A45Bank = A45Bank {
    level_type: 32,
    name: "Act 5 - Temple",
    files: &[
        af(
            "interior",
            "a5wt_interior.dt1",
            "\\d2\\data\\global\\tiles\\expansion\\wildtemple\\interior.dt1",
        ),
        afx(
            "entrance",
            "a5wt_entrance.dt1",
            "\\d2\\data\\global\\tiles\\expansion\\wildtemple\\entrance.dt1",
        ),
    ],
};

/// Stem minuscule d'un chemin LvlTypes (`Act4/Fort/Plaza.dt1` → `plaza`).
fn stem_of(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or("")
}

/// Banque d'un niveau : record levels.txt → LvlType → banque dont chaque
/// fichier est soit un `File N` réel du LvlType, soit marqué `extra`
/// (embarqué par le DS1 preset hors LvlTypes — vérifié par tests).
pub fn bank_for_level(level_id: u32) -> Option<&'static A45Bank> {
    let rec = level_record(level_id)?;
    let bank = match level_id {
        PANDEMONIUM => &BANK_FORT,
        HARROGATH => &BANK_TOWNSIEGE,
        BLOODY_FOOTHS => &BANK_SIEGE_WILD,
        CRYSTALLINE => &BANK_ICE,
        HALLS_VAUGHT => &BANK_WILDTEMPLE,
        _ => return None,
    };
    if bank.level_type != rec.level_type {
        return None;
    }
    let types = parse_lvl_types(LVL_TYPES).ok()?;
    let lt = types.iter().find(|t| t.id == rec.level_type)?;
    for f in bank.files {
        let known = lt.files.iter().any(|p| stem_of(p).eq_ignore_ascii_case(f.stem));
        if !known && !f.extra {
            return None;
        }
    }
    Some(bank)
}

/// Record levels.txt du niveau (`None` si id inconnu).
pub fn level_record(level_id: u32) -> Option<crate::levels::LevelRecord> {
    LevelTables::parse(PATCH_LEVELS).ok()?.get(level_id).cloned()
}

/// Records LvlPrest du niveau (les presets candidats du DRLG retail —
/// Fortress : 1 ; Nihlathak : 4 orientations ; Harrogath : townWest).
pub fn prest_records(level_id: u32) -> Vec<crate::levels::LvlPrestRecord> {
    parse_lvl_prest(LVL_PREST)
        .map(|v| v.into_iter().filter(|p| p.level_id == level_id).collect())
        .unwrap_or_default()
}

/// Ce niveau est-il générable par ce module ? (record levels.txt + banque
/// LvlTypes vérifiée + données preset/maze présentes selon le cas.)
pub fn supports(level_id: u32) -> bool {
    if bank_for_level(level_id).is_none() {
        return false;
    }
    match level_id {
        PANDEMONIUM | HARROGATH => prest_records(level_id).iter().all(|p| !p.files.is_empty()),
        BLOODY_FOOTHS => level_record(level_id).is_some_and(|r| r.drlg_type == 3),
        CRYSTALLINE => crate::drlg_maze::maze_record(level_id).is_some(),
        // LvlPrest Def : UN record « Act 5 - Temple Final Room » dont la
        // colonne Files porte les 4 orientations NihlE/N/S/W.ds1.
        HALLS_VAUGHT => {
            prest_records(level_id).iter().any(|p| p.files.len() == NIHL.len())
        }
        _ => false,
    }
}

/// Ce niveau est-il SANS repli figé ? (les 5 niveaux de l'étape 10a : les
/// presets réels portent 7 ou 11 flux v18 que [`crate::ds1::decode_ds1`]
/// ne sait pas lire — la seule voie correcte est [`generate`] ; échec DRLG
/// = fatal, jamais de carte invalide silencieuse — même politique que le
/// maze de l'étape 8a.)
pub fn generated_only(level_id: u32) -> bool {
    matches!(level_id, PANDEMONIUM | HARROGATH | BLOODY_FOOTHS | CRYSTALLINE | HALLS_VAUGHT)
}

// ---------------------------------------------------------------------------
// Lecteur DS1 v18 complet (tous les flux, murs dédoublés de leur orientation)
// ---------------------------------------------------------------------------

/// Un DS1 v18 décodé SANS hypothèse sur numWalls/numFloors : murs (flux
/// WALL uniquement — les flux d'orientation sont sautés), sols, et la
/// liste de fichiers (espace d'indices de référence).
struct V18 {
    w: usize,
    h: usize,
    files: Vec<String>,
    /// Les `numWalls` flux de murs (sans leurs flux d'orientation).
    walls: Vec<Vec<i32>>,
    /// Les `numFloors` flux de sols.
    floors: Vec<Vec<i32>>,
}

fn parse_v18(name: &str, d: &[u8]) -> Result<V18, String> {
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
    for i in 0..num_walls {
        walls.push(stream(2 * i)?); // les flux d'orientation (2i+1) sont sautés
    }
    let mut floors = Vec::with_capacity(num_floors);
    for i in 0..num_floors {
        floors.push(stream(2 * num_walls + i)?);
    }
    Ok(V18 { w, h, files, walls, floors })
}

/// Projection vers le modèle 6 couches du moteur (cf. doc de module) :
/// L0 = mur 1 (+ sols surnuméraires dans les blancs), L1 = murs 2..N
/// fusionnés, L2/L3 vides, L4 = sol 1, L5 vide (ombre abandonnée).
fn to_engine_layers(v18: &V18) -> [Vec<i32>; 6] {
    let n = v18.w * v18.h;
    let mut layers: [Vec<i32>; 6] = std::array::from_fn(|_| vec![0i32; n]);
    if let Some(w1) = v18.walls.first() {
        layers[0].copy_from_slice(w1);
    }
    // Murs 2..N : fusion non nulle par-dessus (pièces spéciales).
    for w in v18.walls.iter().skip(1) {
        for (dst, &src) in layers[1].iter_mut().zip(w) {
            if src != 0 {
                *dst = src;
            }
        }
    }
    let mut floors = v18.floors.iter();
    if let Some(f1) = floors.next() {
        layers[4].copy_from_slice(f1);
    }
    // Sols surnuméraires : uniquement là où le sol principal est vide.
    for f in floors {
        for i in 0..n {
            if layers[4][i] & 0xFF == 0 && f[i] != 0 {
                layers[4][i] = f[i];
            }
        }
    }
    layers
}

// ---------------------------------------------------------------------------
// Presets réels embarqués (extraits des MPQ, octets intacts)
// ---------------------------------------------------------------------------

static FORTRESS: &[u8] = include_bytes!("../../../../assets/map/presets_a45/fortress.ds1");
static NIHL: [(&str, &[u8]); 4] = [
    ("NihlE", include_bytes!("../../../../assets/map/presets_a45/nihl_e.ds1")),
    ("NihlN", include_bytes!("../../../../assets/map/presets_a45/nihl_n.ds1")),
    ("NihlS", include_bytes!("../../../../assets/map/presets_a45/nihl_s.ds1")),
    ("NihlW", include_bytes!("../../../../assets/map/presets_a45/nihl_w.ds1")),
];
static TOWNSIEGE: &[u8] = include_bytes!("../../../../assets/map/townsiege.ds1");

// ---------------------------------------------------------------------------
// Génération
// ---------------------------------------------------------------------------

/// Statistiques de génération (journal/notes du moteur + tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct A45Stats {
    pub level_id: u32,
    /// `"preset"` (103/109/124), `"wilderness"` (110), `"maze"` (113).
    pub kind: &'static str,
    /// Variante preset tirée (124 : 0..4 = Nihl E/N/S/W ; sinon 0).
    pub variant: usize,
    pub variants: usize,
    /// Cellules ouvertes (marchables au niveau DS1) ; presets : cellules de
    /// sol du niveau.
    pub floor_cells: usize,
    /// Cellules de mur posées (générés uniquement).
    pub wall_cells: usize,
}

/// Génère le niveau A4-A5 `level_id` seedé — FONCTION PURE : les tables
/// viennent des TSV embarquées, les tuiles du paramètre `tilesets`
/// (concaténation des fichiers de la banque DANS L'ORDRE
/// [`A45Bank::files`] — `game.rs::build_a45_tiles` produit cet ordre).
pub fn generate(level_id: u32, seed: u64, tilesets: &[Dt1Tile]) -> Result<(Ds1, A45Stats), String> {
    let bank = bank_for_level(level_id)
        .ok_or_else(|| format!("drlg a45 : niveau {level_id} sans banque supportée"))?;
    match level_id {
        PANDEMONIUM => generate_preset(level_id, FORTRESS, 0, tilesets),
        HARROGATH => generate_preset(level_id, TOWNSIEGE, 0, tilesets),
        HALLS_VAUGHT => {
            let mut rng = crate::drlg::SplitMix64::new(seed);
            let variant = rng.below(NIHL.len());
            generate_preset(level_id, NIHL[variant].1, variant, tilesets)
        }
        BLOODY_FOOTHS => generate_siege(level_id, seed, tilesets),
        CRYSTALLINE => generate_ice_maze(level_id, seed, tilesets),
        _ => Err(format!("drlg a45 : niveau {level_id} hors campagne A4-A5 ({})", bank.name)),
    }
}

/// Montage preset : le DS1 réel v1.12 est décodé, projeté vers le modèle 6
/// couches ([`to_engine_layers`]) et repris SANS remap d'indices (les
/// fichiers de la banque = l'ordre embarqué du DS1 — le contrat du chemin
/// figé historique). Pour Halls of Vaught la variante (E/N/S/W) est tirée
/// par la graine avant l'appel.
fn generate_preset(
    level_id: u32,
    bytes: &[u8],
    variant: usize,
    tilesets: &[Dt1Tile],
) -> Result<(Ds1, A45Stats), String> {
    let bank = bank_for_level(level_id).ok_or("drlg a45 : banque absente")?;
    let v18 = parse_v18(bank.name, bytes)?;
    // Chaque fichier embarqué du DS1 doit résoudre dans la banque (les
    // banques à fichiers `extra` peuvent dépasser la liste embarquée —
    // cf. BANK_WILDTEMPLE : entrance.dt1 requise par les indices 173+).
    let stems: Vec<&str> = bank.files.iter().map(|f| f.stem).collect();
    for f in &v18.files {
        let stem = stem_of(f).to_ascii_lowercase();
        if !stems.iter().any(|s| s.eq_ignore_ascii_case(&stem)) {
            return Err(format!(
                "drlg a45 niveau {level_id} : fichier embarqué « {f} » hors banque (désynchronisée)"
            ));
        }
    }
    let layers = to_engine_layers(&v18);
    let floor_cells = layers[4].iter().filter(|v| (**v & 0xFF) != 0).count();
    if floor_cells == 0 {
        return Err(format!("drlg a45 niveau {level_id} : preset sans sol"));
    }
    validate_cells(level_id, &layers, tilesets)?;
    let stats = A45Stats {
        level_id,
        kind: "preset",
        variant,
        variants: preset_variant_count(level_id),
        floor_cells,
        wall_cells: 0,
    };
    let ds1 = Ds1 {
        version: 18,
        w: v18.w,
        h: v18.h,
        files: v18.files,
        layers: layers.to_vec(),
    };
    Ok((ds1, stats))
}

fn preset_variant_count(level_id: u32) -> usize {
    match level_id {
        HALLS_VAUGHT => 4,
        _ => 1,
    }
}

/// Indices de tuiles utilisables des pools générés : l'espace p1 d'un DS1
/// est un u8 — au-delà de [`USABLE_TILES`] (interior.dt1 = 288 tuiles
/// réelles), les tuiles sont inadressables par une cellule générée.
fn pools(tilesets: &[Dt1Tile]) -> Result<(Vec<usize>, Vec<usize>), String> {
    let usable = tilesets.len().min(USABLE_TILES);
    let floors: Vec<usize> = (1..usable).filter(|&i| tilesets[i].is_floor).collect();
    let walls: Vec<usize> = (1..usable).filter(|&i| !tilesets[i].is_floor).collect();
    if floors.len() < 8 || walls.len() < 8 {
        return Err(format!(
            "drlg a45 : pools insuffisants ({} sols, {} murs sur {} tuiles)",
            floors.len(),
            walls.len(),
            usable
        ));
    }
    Ok((floors, walls))
}

/// Profondeur de l'anneau de bord rocheux des niveaux générés.
const RING: usize = 2;

/// Nombre de cellules de l'anneau de bord `RING` tuiles d'une carte w×h.
fn border_ring(w: usize, h: usize) -> usize {
    2 * (w * RING + (h - 2 * RING) * RING)
}

/// Wildnerness de siège (110, Bloody Foothills) : canevas SizeX×SizeY de la
/// table (240×48), sol réel brassé par le RNG, anneau rocheux de bord,
/// pierres dispersées seedées, et galeries d'entrée E/O garanties ouvertes
/// (les ancres de warp du manifest tombent dedans quelle que soit la graine).
fn generate_siege(level_id: u32, seed: u64, tilesets: &[Dt1Tile]) -> Result<(Ds1, A45Stats), String> {
    let rec = level_record(level_id).ok_or("drlg a45 : record absent")?;
    if rec.drlg_type != 3 {
        return Err(format!("drlg a45 : niveau {level_id} DrlgType {} != 3", rec.drlg_type));
    }
    let (floors, walls) = pools(tilesets)?;
    let min_side = (2 * RING + 2) as u32;
    let (w, h) = (rec.size_x.max(min_side) as usize, rec.size_y.max(min_side) as usize);
    let mut rng = crate::drlg::SplitMix64::new(seed);
    let mut layers: [Vec<i32>; 6] = std::array::from_fn(|_| vec![0i32; w * h]);

    // Sol : tuiles réelles brassées partout.
    for cell in layers[4].iter_mut() {
        *cell = floors[rng.below(floors.len())] as i32;
    }
    // Anneau de bord rocheux + pierres dispersées + galeries d'entrée
    // ouvertes (aucune pierre posée dans les colonnes de galerie).
    let mut wall_cells = 0usize;
    for y in 0..h {
        for x in 0..w {
            let border = x < RING || y < RING || x >= w - RING || y >= h - RING;
            let gallery = (RING..RING + 6).contains(&x) || (w - RING - 6..w - RING).contains(&x);
            let rock = border || (!gallery && rng.f01() < 0.06);
            if rock {
                layers[1][y * w + x] = walls[rng.below(walls.len())] as i32;
                wall_cells += 1;
            }
        }
    }
    validate_generated(level_id, &layers, w, h, tilesets)?;
    let ds1 = Ds1 {
        version: 18,
        w,
        h,
        files: bank_files(level_id)?,
        layers: layers.to_vec(),
    };
    Ok((
        ds1,
        A45Stats {
            level_id,
            kind: "wilderness",
            variant: 0,
            variants: 1,
            floor_cells: w * h - wall_cells,
            wall_cells,
        },
    ))
}

/// Maze de glace (113, Crystalized Cavern 1) : plan de rooms seedé
/// ([`crate::drlg_maze::plan_for`] — LvlMaze 10 rooms de 16×16), sol/murs
/// réels de la banque, salles évidées + couloirs 3 tuiles + DEUX galeries
/// d'entrée traversantes E/O reliées aux rooms extrêmes (les ancres de
/// warp du manifest tombent dedans quelle que soit la graine).
fn generate_ice_maze(level_id: u32, seed: u64, tilesets: &[Dt1Tile]) -> Result<(Ds1, A45Stats), String> {
    let maze = crate::drlg_maze::maze_record(level_id)
        .ok_or_else(|| format!("drlg a45 : niveau {level_id} sans record LvlMaze"))?;
    let (floors, walls) = pools(tilesets)?;
    let plan = crate::drlg_maze::plan_for(level_id, seed)?;
    if plan.rooms.len() != maze.rooms as usize {
        return Err(format!(
            "drlg a45 : {} rooms posées au lieu de {} (LvlMaze)",
            plan.rooms.len(),
            maze.rooms
        ));
    }
    let (sx, sy) = (maze.size_x as usize, maze.size_y as usize);
    let (w, h) = (plan.grid_w as usize * sx, plan.grid_h as usize * sy);
    let inset = RING;
    let half = 1usize; // couloirs de 3 tuiles
    let mut rng = crate::drlg::SplitMix64::new(seed);
    let mut layers: [Vec<i32>; 6] = std::array::from_fn(|_| vec![0i32; w * h]);
    for cell in layers[4].iter_mut() {
        *cell = floors[rng.below(floors.len())] as i32;
    }
    // Masque de l'ouvert : salles évidées, couloirs, galeries, connecteurs.
    let mut open = vec![false; w * h];
    let mut floor_cells = 0usize;
    let mut carve = |x: usize, y: usize| {
        if !open[y * w + x] {
            open[y * w + x] = true;
            floor_cells += 1;
        }
    };
    for room in &plan.rooms {
        let (x0, y0) = (room.gx as usize * sx, room.gy as usize * sy);
        for y in (y0 + inset)..(y0 + sy - inset) {
            for x in (x0 + inset)..(x0 + sx - inset) {
                carve(x, y);
            }
        }
    }
    for room in &plan.rooms {
        let (cx, cy) = crate::drlg_maze::room_center(room, sx, sy);
        if room.open_e {
            for x in (room.gx as usize * sx + sx - inset)..(room.gx as usize * sx + sx + inset) {
                for y in (cy - half)..=(cy + half) {
                    carve(x, y);
                }
            }
        }
        if room.open_s {
            for y in (room.gy as usize * sy + sy - inset)..(room.gy as usize * sy + sy + inset) {
                for x in (cx - half)..=(cx + half) {
                    carve(x, y);
                }
            }
        }
    }
    // Galeries d'entrée E/O traversantes (DANS l'anneau interne).
    let mid = h / 2;
    for y in (mid - 6).max(inset)..((mid + 6).min(h - inset)) {
        for x in inset..(inset + 5) {
            carve(x, y);
        }
        for x in (w - inset - 5)..(w - inset) {
            carve(x, y);
        }
    }
    // Connecteurs en L : la galerie ouest rejoint la room la plus à
    // l'ouest, la galerie est la plus à l'est (le plan est un arbre
    // couvrant : les deux extrêmes sont donc reliées entre elles à travers
    // tout le maze). Horizontale au rang médian de la galerie, puis
    // verticale dans la colonne du centre de room — les rooms ne sont pas
    // forcément au rang médian (graine 42 : room extrême ouest en (2,0)).
    let west = plan.rooms.iter().min_by_key(|r| (r.gx, r.gy)).unwrap();
    let (wx, wy) = crate::drlg_maze::room_center(west, sx, sy);
    for x in (inset + 5)..=wx {
        carve(x, mid);
    }
    for y in wy.min(mid)..=wy.max(mid) {
        carve(wx, y);
    }
    let east = plan.rooms.iter().max_by_key(|r| (r.gx, r.gy)).unwrap();
    let (ex, ey) = crate::drlg_maze::room_center(east, sx, sy);
    for x in ex..(w - inset - 5) {
        carve(x, mid);
    }
    for y in ey.min(mid)..=ey.max(mid) {
        carve(ex, y);
    }
    // Roche réelle sur toute cellule non évidée.
    let mut wall_cells = 0usize;
    for (i, cell) in layers[1].iter_mut().enumerate() {
        if !open[i] {
            *cell = walls[rng.below(walls.len())] as i32;
            wall_cells += 1;
        }
    }
    validate_ice(level_id, &layers, w, h, &open, &plan, sx, sy, tilesets)?;
    let ds1 = Ds1 {
        version: 18,
        w,
        h,
        files: bank_files(level_id)?,
        layers: layers.to_vec(),
    };
    Ok((
        ds1,
        A45Stats {
            level_id,
            kind: "maze",
            variant: 0,
            variants: 1,
            floor_cells,
            wall_cells,
        },
    ))
}

/// Liste tg1 de la banque du niveau (fichiers du [`Ds1`] généré).
fn bank_files(level_id: u32) -> Result<Vec<String>, String> {
    let bank = bank_for_level(level_id).ok_or("drlg a45 : banque absente")?;
    Ok(bank.files.iter().map(|f| f.tg1.to_string()).collect())
}

/// Validation commune : tout indice de tuile référencé tombe dans l'espace
/// concaténé de la banque.
fn validate_cells(level_id: u32, layers: &[Vec<i32>; 6], tilesets: &[Dt1Tile]) -> Result<(), String> {
    for (li, layer) in layers.iter().enumerate() {
        for (i, &v) in layer.iter().enumerate() {
            let (p1, p2, _, _) = crate::ds1::cell_fields(v);
            if p1 != 0 && crate::ds1::cell_tile_index(p1, p2) >= tilesets.len() {
                return Err(format!(
                    "drlg a45 niveau {level_id} : indice de tuile {} hors espace ({}) — couche {li} cellule {i}",
                    crate::ds1::cell_tile_index(p1, p2),
                    tilesets.len()
                ));
            }
        }
    }
    Ok(())
}

/// Validations des niveaux générés : sol plein, indices dans l'espace,
/// anneau de bord rocheux scellé (les galeries vivent DANS l'anneau
/// interne, jamais sur le bord lui-même).
fn validate_generated(
    level_id: u32,
    layers: &[Vec<i32>; 6],
    w: usize,
    h: usize,
    tilesets: &[Dt1Tile],
) -> Result<(), String> {
    if layers[4].iter().any(|v| (*v & 0xFF) == 0) {
        return Err(format!("drlg a45 niveau {level_id} : sol incomplet"));
    }
    validate_cells(level_id, layers, tilesets)?;
    for y in 0..h {
        for x in [0usize, 1, w - 2, w - 1] {
            if layers[1][y * w + x] == 0 {
                return Err(format!("drlg a45 niveau {level_id} : bord de carte ouvert ({x},{y})"));
            }
        }
    }
    for x in 0..w {
        for y in [0usize, 1, h - 2, h - 1] {
            if layers[1][y * w + x] == 0 {
                return Err(format!("drlg a45 niveau {level_id} : bord de carte ouvert ({x},{y})"));
            }
        }
    }
    Ok(())
}

/// Validations spécifiques du maze : toutes les rooms atteignables depuis
/// la galerie ouest (BFS de l'ouvert — aucune île, quel que soit le plan).
fn validate_ice(
    level_id: u32,
    layers: &[Vec<i32>; 6],
    w: usize,
    h: usize,
    open: &[bool],
    plan: &crate::drlg::MazePlan,
    sx: usize,
    sy: usize,
    tilesets: &[Dt1Tile],
) -> Result<(), String> {
    validate_generated(level_id, layers, w, h, tilesets)?;
    let mut seen = vec![false; w * h];
    let mut queue = std::collections::VecDeque::new();
    let start = 2 + 2 + (h / 2) * w; // cœur de la galerie ouest
    seen[start] = true;
    queue.push_back((start % w, start / w));
    while let Some((x, y)) = queue.pop_front() {
        for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let ni = ny as usize * w + nx as usize;
            if !seen[ni] && open[ni] {
                seen[ni] = true;
                queue.push_back((nx as usize, ny as usize));
            }
        }
    }
    for room in &plan.rooms {
        let (cx, cy) = crate::drlg_maze::room_center(room, sx, sy);
        if !seen[cy * w + cx] {
            return Err(format!(
                "drlg a45 niveau {level_id} : room ({},{}) inatteignable depuis les galeries",
                room.gx, room.gy
            ));
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Tests — vraies tables, vrais DT1 extraits, déterminisme, marchabilité
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::ComposedMap;
    use crate::dt1::decode_dt1;

    /// Racine du workspace (d2w-fin) — `None` hors checkout complet.
    fn ws_root() -> Option<std::path::PathBuf> {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .map(|p| p.to_path_buf())
    }

    /// Tilesets RÉELS d'une banque (DT1 extraits des MPQ), ordre bank.
    fn real_bank_tiles(bank: &A45Bank) -> Vec<Dt1Tile> {
        let root = ws_root().expect("workspace d2w-fin introuvable");
        let mut tilesets = Vec::new();
        for f in bank.files {
            let bytes = std::fs::read(root.join("assets/map").join(f.local))
                .unwrap_or_else(|e| panic!("{} : {e} (extrait MPQ absent)", f.local));
            let tiles = decode_dt1(&bytes).unwrap_or_else(|e| panic!("{} : {e:?}", f.local));
            assert!(!tiles.is_empty(), "{} : 0 tuile", f.local);
            tilesets.extend(tiles);
        }
        tilesets
    }

    // --- tables réelles -------------------------------------------------------

    /// Les 5 niveaux A4-A5 : records levels.txt, LvlPrest, LvlMaze, banques.
    #[test]
    fn a45_tables_match_release_data() {
        // (id, DrlgType, LevelType, SizeX×SizeY de la table)
        for (id, drlg, lt, sx, sy) in [
            (PANDEMONIUM, 2u8, 26u32, 32u32, 24u32),
            (HARROGATH, 2, 29, 40, 40),
            (BLOODY_FOOTHS, 3, 30, 240, 48),
            (CRYSTALLINE, 1, 33, 200, 200),
            (HALLS_VAUGHT, 2, 32, 84, 84),
        ] {
            let rec = level_record(id).unwrap_or_else(|| panic!("{id} : record absent"));
            assert_eq!((rec.drlg_type, rec.level_type), (drlg, lt), "{id}");
            assert_eq!((rec.size_x, rec.size_y), (sx, sy), "{id} : taille table");
            assert!(supports(id), "{id} non supporté");
            let bank = bank_for_level(id).unwrap_or_else(|| panic!("{id} : banque absente"));
            assert_eq!(bank.level_type, lt, "{id}");
        }
        // Noms retail (LevelName v1.12).
        assert_eq!(level_record(PANDEMONIUM).unwrap().level_name, "The Pandemonium Fortress");
        assert_eq!(level_record(HARROGATH).unwrap().level_name, "Harrogath");
        assert_eq!(level_record(BLOODY_FOOTHS).unwrap().level_name, "Bloody Foothills");
        assert_eq!(level_record(CRYSTALLINE).unwrap().level_name, "Crystalized Cavern Level 1");
        assert_eq!(level_record(HALLS_VAUGHT).unwrap().level_name, "Halls of Vaught");
        // LvlPrest réel : Fortress 1 preset (Def 797, Fortress.ds1, mask 7
        // bits) ; Vaught 1 record dont Files = les 4 orientations ;
        // Harrogath 1 preset (townWest, Def 863).
        let fort = prest_records(PANDEMONIUM);
        assert_eq!(fort.len(), 1);
        assert!(
            fort[0].files.iter().any(|f| f.to_ascii_lowercase().ends_with("fortress.ds1")),
            "{:?}",
            fort[0].files
        );
        assert_eq!(fort[0].dt1_mask, 127, "Dt1Mask LvlPrest 103 = 7 bits");
        let nihl = prest_records(HALLS_VAUGHT);
        assert_eq!(nihl.len(), 1, "un record LvlPrest pour 124");
        assert_eq!(nihl[0].files.len(), 4, "Files = NihlE/N/S/W.ds1");
        assert_eq!(nihl[0].dt1_mask, 1);
        // L'ordre statique des variantes = l'ordre Files du record.
        for (f, (stem, _)) in nihl[0].files.iter().zip(NIHL) {
            assert!(
                f.to_ascii_lowercase().ends_with(&format!("{}.ds1", stem.to_ascii_lowercase())),
                "{f} vs {stem}"
            );
        }
        assert_eq!(prest_records(HARROGATH).len(), 1, "Def 863 = townWest.ds1");
        // LvlMaze 113 : 10 rooms de 16×16 (grille réelle v1.12).
        let maze = crate::drlg_maze::maze_record(CRYSTALLINE).unwrap();
        assert_eq!((maze.rooms, maze.size_x, maze.size_y), (10, 16, 16));
        // Rejets propres : niveaux hors campagne.
        assert!(!supports(2) && bank_for_level(2).is_none(), "Blood Moor = étape W02");
        assert!(!supports(40) && bank_for_level(40).is_none(), "Lut Gholein hors 10a");
        assert!(
            [PANDEMONIUM, HARROGATH, BLOODY_FOOTHS, CRYSTALLINE, HALLS_VAUGHT]
                .iter()
                .all(|&id| generated_only(id)),
            "les 5 niveaux 10a : échec DRLG fatal (pas de repli figé)"
        );
    }

    // --- lecteur v18 ----------------------------------------------------------

    #[test]
    fn v18_reader_parses_real_presets() {
        // townsiege.ds1 (Harrogath) : 81×49, 2 murs + 2 sols — preuves
        // d2w-re evidence-formats/ds1-path/evidence.md §1.
        let t = parse_v18("townsiege", TOWNSIEGE).unwrap();
        assert_eq!((t.w, t.h), (81, 49));
        assert_eq!(t.walls.len(), 2);
        assert_eq!(t.floors.len(), 2);
        assert_eq!(t.files.len(), 10, "fichiers siège embarqués");
        assert!(t.floors[0].iter().filter(|v| (**v & 0xFF) != 0).count() > 2_000, "sol de ville dense");
        // Fortress.ds1 (Pandemonium, LvlPrest Def 797) : 33×25 = SizeX/Y+1.
        let f = parse_v18("fortress", FORTRESS).unwrap();
        assert_eq!((f.w, f.h), (33, 25));
        assert_eq!((f.walls.len(), f.floors.len()), (4, 2), "4w+2f = 11 flux");
        assert_eq!(f.files.len(), 9, "7 tuiles fort/mesa + warp + inviswal");
        // Nihl*.ds1 (Vaught, Def 864) : 85×85 = 84+1, 2w+2f, un seul DT1.
        for (name, bytes) in NIHL {
            let n = parse_v18(name, bytes).unwrap();
            assert_eq!((n.w, n.h), (85, 85), "{name}");
            assert_eq!((n.walls.len(), n.floors.len()), (2, 2), "{name}");
            assert_eq!(n.files.len(), 1, "{name}");
            assert!(n.floors[0].iter().filter(|v| (**v & 0xFF) != 0).count() > 1_000, "{name} : sol dense");
        }
    }

    #[test]
    fn engine_layers_projection_is_sane() {
        // La projection garde le mur 1 en L0 (convention TownN1), fusionne
        // les murs suivants en L1, et NE COPIE JAMAIS un flux d'orientation
        // (les petites valeurs bloqueraient des cellules de ville).
        let v18 = parse_v18("townsiege", TOWNSIEGE).unwrap();
        let layers = to_engine_layers(&v18);
        assert_eq!(layers.len(), 6);
        let nz = |l: &[i32]| l.iter().filter(|v| (**v & 0xFF) != 0).count();
        assert_eq!(nz(&layers[0]), nz(&v18.walls[0]), "L0 = mur 1 intégral");
        assert_eq!(nz(&layers[1]), nz(&v18.walls[1]), "L1 = mur 2");
        assert!(layers[2..4].iter().all(|l| nz(l) == 0), "L2/L3 vides");
        assert_eq!(nz(&layers[4]), nz(&v18.floors[0]), "L4 = sol 1");
        assert_eq!(nz(&layers[5]), 0, "ombre abandonnée (L5 vide)");
    }

    // --- génération seedée ----------------------------------------------------

    #[test]
    fn presets_are_deterministic() {
        let fort = real_bank_tiles(&BANK_FORT);
        let (a, sa) = generate(PANDEMONIUM, 42, &fort).unwrap();
        let (b, _) = generate(PANDEMONIUM, 42, &fort).unwrap();
        assert_eq!(a.layers, b.layers, "même graine ⇒ mêmes couches");
        assert_eq!(sa.kind, "preset");
        assert_eq!(sa.variants, 1, "Fortress : un seul preset (Def 797)");
        assert_eq!((a.w, a.h), (33, 25), "33×25 = SizeX/Y+1 (piège v18)");
        let town = real_bank_tiles(&BANK_TOWNSIEGE);
        let (h, sh) = generate(HARROGATH, 42, &town).unwrap();
        let (h2, _) = generate(HARROGATH, 43, &town).unwrap();
        assert_eq!(h.layers, h2.layers, "Harrogath : ville figée, graine sans effet");
        assert_eq!((h.w, h.h), (81, 49), "townsiege.ds1 réel");
        assert_eq!(sh.kind, "preset");
        assert!(sh.floor_cells > 2_000, "{} cellules de sol", sh.floor_cells);
    }

    #[test]
    fn halls_of_vaught_seeds_pick_real_variants() {
        let tiles = real_bank_tiles(&BANK_WILDTEMPLE);
        assert_eq!(tiles.len(), 371, "interior (173) + entrance (198) — banque réelle Nihl");
        // 16 graines : au moins 3 variantes distinctes visitées (4 dispo).
        let mut seen = std::collections::BTreeSet::new();
        let mut maps = std::collections::BTreeSet::new();
        for seed in 0..16u64 {
            let (ds1, stats) = generate(HALLS_VAUGHT, seed, &tiles).unwrap();
            assert_eq!((ds1.w, ds1.h), (85, 85), "85×85 = SizeX/Y+1 (piège v18)");
            assert_eq!(stats.variants, 4, "NihlE/N/S/W (LvlPrest Def 864)");
            seen.insert(stats.variant);
            maps.insert(ds1.layers);
        }
        assert!(seen.len() >= 3, "variantes visitées : {seen:?}");
        assert!(maps.len() >= 3, "des cartes différentes selon la graine");
        // Déterminisme graine par graine.
        let (a, sa) = generate(HALLS_VAUGHT, 7, &tiles).unwrap();
        let (b, sb) = generate(HALLS_VAUGHT, 7, &tiles).unwrap();
        assert_eq!((a.layers, sa), (b.layers, sb));
    }

    #[test]
    fn bloody_foothills_matches_table_size_and_is_deterministic() {
        let tiles = real_bank_tiles(&BANK_SIEGE_WILD);
        assert_eq!(tiles.len(), 248, "sous-ensemble structurel ≤ espace u8");
        let (a, sa) = generate(BLOODY_FOOTHS, 42, &tiles).unwrap();
        let (b, sb) = generate(BLOODY_FOOTHS, 42, &tiles).unwrap();
        assert_eq!(a.layers, b.layers, "même graine ⇒ même carte");
        assert_eq!(sa, sb);
        let (c, _) = generate(BLOODY_FOOTHS, 43, &tiles).unwrap();
        assert_ne!(a.layers, c.layers, "graine différente ⇒ carte différente");
        let rec = level_record(BLOODY_FOOTHS).unwrap();
        assert_eq!((a.w, a.h), (rec.size_x as usize, rec.size_y as usize), "SizeX/Y exacts (240×48)");
        assert_eq!(sa.kind, "wilderness");
        assert_eq!(sa.wall_cells, a.layers[1].iter().filter(|v| (**v & 0xFF) != 0).count());
        assert_eq!(sa.floor_cells + sa.wall_cells, a.w * a.h, "chaque cellule est sol ou roche");
        assert!(sa.floor_cells > sa.wall_cells, "une plaine de siège, pas un mur");
    }

    #[test]
    fn crystalline_maze_is_deterministic_and_connected() {
        let tiles = real_bank_tiles(&BANK_ICE);
        assert_eq!(tiles.len(), 288, "icecave interior.dt1 réel");
        let (a, sa) = generate(CRYSTALLINE, 42, &tiles).unwrap();
        let (b, sb) = generate(CRYSTALLINE, 42, &tiles).unwrap();
        assert_eq!(a.layers, b.layers);
        assert_eq!(sa, sb);
        let (c, _) = generate(CRYSTALLINE, 43, &tiles).unwrap();
        assert_ne!(a.layers, c.layers);
        // Dimensions : grille LvlMaze (plan_grid(10) = 5) × bloc 16² ; les
        // 200×200 de levels.txt sont l'emprise retail (convention
        // drlg_maze de l'étape 8a).
        assert_eq!((a.w, a.h), (80, 80));
        assert!(a.w * a.h <= 200 * 200, "dans l'emprise retail SizeX/Y");
        assert_eq!(sa.kind, "maze");
        assert_eq!(sa.floor_cells + sa.wall_cells, a.w * a.h, "chaque cellule est ouverte ou roche");
        // Toutes les rooms du plan sont posées (LvlMaze Rooms=10).
        let plan = crate::drlg_maze::plan_for(CRYSTALLINE, 42).unwrap();
        assert_eq!(plan.rooms.len(), 10);
    }

    #[test]
    fn generate_rejects_bad_inputs() {
        let tiles = real_bank_tiles(&BANK_SIEGE_WILD);
        assert!(generate(2, 1, &tiles).is_err(), "Blood Moor hors campagne");
        assert!(generate(999, 1, &tiles).is_err(), "id inconnu");
        // Espace de tuiles rétréci : pools insuffisants détectés.
        assert!(generate(CRYSTALLINE, 1, &tiles[..3]).is_err());
    }

    // --- composition (pipeline réel) + marchabilité des ancres ----------------

    /// Ancres de warp du manifest (sous-tiles) par niveau — l'aller ET le
    /// point d'arrivée doivent tomber sur une tuile marchable composée,
    /// POUR TOUTE variante seedée (vérifié sur les 4 Nihl en amont).
    fn anchors_of(id: u32) -> Vec<(usize, usize)> {
        match id {
            HARROGATH => vec![(375, 110), (360, 110)],
            BLOODY_FOOTHS => vec![(15, 120), (40, 120), (1185, 120)],
            CRYSTALLINE => vec![(388, 200), (15, 200)],
            // Halls of Vaught : preset temple sparse (canvas partiellement
            // rempli, cf. ratio 0.01-0.5) — ancres relevées stables sur les
            // seeds 1/3/7/42 (sonde walkable±12), l'ancre (390,210) initiale
            // tombait dans un trou local du preset pour le seed 7.
            HALLS_VAUGHT => vec![(200, 50), (200, 75)],
            PANDEMONIUM => vec![(80, 60)],
            _ => Vec::new(),
        }
    }

    fn assert_anchors_walkable(map: &ComposedMap, id: u32) {
        for (x, y) in anchors_of(id) {
            assert!(
                crate::compose::find_nearby_walkable(&map.walk, map.grid_w, map.grid_h, x, y, 12)
                    .is_some(),
                "{id} : ancre ({x},{y}) sans marche à proximité"
            );
        }
    }

    #[test]
    fn composed_presets_are_walkable() {
        // (id, banque, ratio de marche attendu — les presets temple/ville
        // Nihl ne remplissent qu'une partie du canevas 85×85 : le reste
        // reste vide, hors marche, comme figé en retail.)
        for (id, bank, lo, hi) in [
            (PANDEMONIUM, &BANK_FORT, 0.05f32, 0.95f32),
            (HARROGATH, &BANK_TOWNSIEGE, 0.3, 0.95),
            (HALLS_VAUGHT, &BANK_WILDTEMPLE, 0.01, 0.5),
        ] {
            let tiles = real_bank_tiles(bank);
            let (ds1, _) = generate(id, 7, &tiles).unwrap();
            let map = crate::compose::compose_seeded(&ds1, &tiles, 1);
            let ratio = map.walk.iter().filter(|w| **w).count() as f32 / map.walk.len() as f32;
            assert!((lo..hi).contains(&ratio), "niveau {id} : ratio de marche {ratio:.2}");
            assert!(map.idx.iter().any(|v| *v != 0), "niveau {id} : framebuffer vide");
            assert_anchors_walkable(&map, id);
        }
    }

    #[test]
    fn composed_bloody_foothills_is_walkable() {
        let tiles = real_bank_tiles(&BANK_SIEGE_WILD);
        let (ds1, _) = generate(BLOODY_FOOTHS, 7, &tiles).unwrap();
        let map = crate::compose::compose_seeded(&ds1, &tiles, 1);
        assert_eq!((map.grid_w, map.grid_h), (240 * 5, 48 * 5), "grille = SizeX/Y × 5 sous-tiles");
        let ratio = map.walk.iter().filter(|w| **w).count() as f32 / map.walk.len() as f32;
        assert!((0.5..0.95).contains(&ratio), "plaine de siège jouable : {ratio:.2}");
        assert!(map.idx.iter().any(|v| *v != 0), "framebuffer vide");
        assert_anchors_walkable(&map, BLOODY_FOOTHS);
    }

    #[test]
    fn composed_crystalline_is_walkable() {
        let tiles = real_bank_tiles(&BANK_ICE);
        let (ds1, _) = generate(CRYSTALLINE, 7, &tiles).unwrap();
        let map = crate::compose::compose_seeded(&ds1, &tiles, 1);
        let ratio = map.walk.iter().filter(|w| **w).count() as f32 / map.walk.len() as f32;
        assert!((0.15..0.75).contains(&ratio), "maze de glace jouable : {ratio:.2}");
        assert!(map.idx.iter().any(|v| *v != 0), "framebuffer vide");
        assert_anchors_walkable(&map, CRYSTALLINE);
    }
}

#[cfg(test)]
mod dbg_tmp {
    #[test]
    fn dbg_rooms() {
        let plan = crate::drlg_maze::plan_for(super::CRYSTALLINE, 42).unwrap();
        for r in &plan.rooms {
            println!("room gx={} gy={} open(n,e,s,w)=({},{},{},{})", r.gx, r.gy, r.open_n, r.open_e, r.open_s, r.open_w);
        }
        println!("grid {}x{}", plan.grid_w, plan.grid_h);
    }
}
