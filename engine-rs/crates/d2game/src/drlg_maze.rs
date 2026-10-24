//! DRLG maze seedé — DrlgType 1 (étape 8a, actes 2+ : tombeaux/égouts/
//! sanctuaire arcane A2, caves A1).
//!
//! Port Rust du générateur MAZE (DrlgType 1) du projet diablo-2-wasm.
//! Référence Go lecture seule : `opendiablo2` ne code PAS le maze
//! (`d2mapgen/map_generator.go` ne porte que l'overworld acte I) — il donne
//! en revanche les colonnes réelles de LvlMaze.txt
//! (`d2records/level_maze_loader.go:11-19` : Name/Level/Rooms/Rooms(N)/
//! Rooms(H)/SizeX/SizeY) et leur sémantique
//! (`d2records/level_maze_record.go:24-26` : « SizeX/SizeY = the size in
//! the X/Y direction of any component ds1 map section » — chaque room est
//! un bloc SizeX×SizeY). La disposition des rooms est donc la nôtre
//! (arbre couvrant seedé, réutilise [`crate::drlg::plan_maze`] déjà testé),
//! mais TOUTES les données sont celles de la release v1.12 :
//!
//! * `LvlMaze.txt` (82 records) : nombre de rooms + taille d'un bloc ;
//! * `LvlTypes.txt` (File 1..32) : les DT1 disponibles du type de niveau —
//!   chaque banque ci-dessous est un sous-ensemble VÉRIFIÉ contre ces
//!   colonnes (les niveaux maze n'ont pas de LvlPrest, donc pas de
//!   Dt1Mask par room : la banque joue ce rôle, cf. [`bank_for_level`]) ;
//! * les DT1 acte 1-2 extraits des MPQ LECTURE SEULE
//!   (`data\global\tiles\ACT{1,2}\…` via `tools/stormlib_read.py`,
//!   copies dans `assets/map/act*_*.dt1`).
//!
//! Pipeline (un seul chemin de code, comme la Blood Moor) :
//! 1. tables réelles embarquées ([`PATCH_LEVELS`], [`LVL_TYPES`],
//!    [`LVL_MAZE`]) → record du niveau + grille de maze ;
//! 2. plan de rooms seedé ([`crate::drlg::plan_maze`]) sur une grille
//!    ⌈√rooms⌉ (+1 de mou pour la connexité) — chaque room un bloc
//!    SizeX×SizeY, ouvertures réciproques = arbre couvrant ;
//! 3. canevas gw·SizeX × gh·SizeY : sol réel brassé par le RNG partout
//!    (pool des tuiles `is_floor` de la banque), rooms évidées (marge
//!    [`INSET`]), couloirs de 3 tuiles entre rooms connectées ;
//! 4. roche = tuile mur seedée (pool `!is_floor`) sur toute cellule non
//!    évidée — bords de carte scellés ;
//! 5. [`Ds1`] synthétique v18 (6 couches tag 2,1, fichiers [`MazeBank`])
//!    consommé tel quel par `compose::compose_seeded` (collision + rendu).
//!
//! Contrainte d'encodage DS1 : `p1` est un u8 (indice famille) — l'espace
//! unifié de chaque banque reste ≤ 256 tuiles (vérifié par test sur les
//! DT1 réels). Garanties testées : même graine ⇒ même carte, graine
//! différente ⇒ carte différente, toutes les rooms connectées (BFS des
//! centres, au niveau tuiles ET au niveau grille de marche composée),
//! sol 100 % rempli, 0 indice hors bornes.

use crate::ds1::Ds1;
use crate::dt1::Dt1Tile;
use crate::levels::{parse_lvl_maze, parse_lvl_types, LevelTables};

/// `patch_levels.txt` (levels.txt final v1.12) — record DrlgType/LevelType.
const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");
/// `LvlTypes.txt` — File 1..32 : les DT1 disponibles par type de niveau.
const LVL_TYPES: &str = include_str!("../../../../assets/LvlTypes.txt");
/// `LvlMaze.txt` — Rooms + SizeX/SizeY par niveau maze.
const LVL_MAZE: &str = include_str!("../../../../assets/LvlMaze.txt");

// ---------------------------------------------------------------------------
// Banques de tuiles (LvlTypes → DT1 extraits des MPQ)
// ---------------------------------------------------------------------------

/// Un fichier DT1 d'une banque :
/// * `stem` — le stem dans les colonnes `File N` de `LvlTypes.txt`
///   (`Act2/Tomb/Tomb.dt1` → `tomb`) — la vérification data-driven ;
/// * `local` — le conteneur extrait dans `assets/map/`
///   (`tools/stormlib_read.py` ← `data\global\tiles\…`) ;
/// * `tg1` — le chemin embarqué dans le [`Ds1`] généré (résolu par
///   `game.rs::resolve_container` vers `local` via la liste `dt1` du bloc
///   manifest — même stem).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BankFile {
    pub stem: &'static str,
    pub local: &'static str,
    pub tg1: &'static str,
}

const fn bf(stem: &'static str, local: &'static str, tg1: &'static str) -> BankFile {
    BankFile { stem, local, tg1 }
}

/// Une banque de tuiles maze : sous-ensemble STRUCTUREL des `File N` du
/// LvlType du niveau (sols + murs), ordre figé = ordre de concaténation
/// des tilesets. Toutes les banques tiennent en ≤ 256 tuiles (espace p1
/// u8 du DS1 — testé sur les DT1 réels).
#[derive(Debug, Clone, Copy)]
pub struct MazeBank {
    /// Id `LevelType` du level record (colonne `LevelType`).
    pub level_type: u32,
    pub name: &'static str,
    pub files: &'static [BankFile],
}

/// LvlType 17 « Act 2 - Tomb » — tombeaux A2 (Stony Tomb, Halls of the
/// Dead, Tal Rasha…) : 186 tuiles réelles (104 Tomb, 11 Columns, 41
/// Things, 6 Stairs, 24 Tombsteps).
pub const BANK_TOMB: MazeBank = MazeBank {
    level_type: 17,
    name: "Act 2 - Tomb",
    files: &[
        bf("tomb", "act2_tomb_tomb.dt1", "\\d2\\data\\global\\tiles\\act2\\tomb\\tomb.dt1"),
        bf("columns", "act2_tomb_columns.dt1", "\\d2\\data\\global\\tiles\\act2\\tomb\\columns.dt1"),
        bf("things", "act2_tomb_things.dt1", "\\d2\\data\\global\\tiles\\act2\\tomb\\things.dt1"),
        bf("stairs", "act2_tomb_stairs.dt1", "\\d2\\data\\global\\tiles\\act2\\tomb\\stairs.dt1"),
        bf("tombsteps", "act2_tomb_tombsteps.dt1", "\\d2\\data\\global\\tiles\\act2\\tomb\\tombsteps.dt1"),
    ],
};

/// LvlType 13 « Act 2 - Sewer » — égouts A2 + Ancient Tunnels : 110 tuiles
/// (41 Walls, 32 Chamb, 15 Items, 22 Radament).
pub const BANK_SEWER: MazeBank = MazeBank {
    level_type: 13,
    name: "Act 2 - Sewer",
    files: &[
        bf("walls", "act2_sewer_walls.dt1", "\\d2\\data\\global\\tiles\\act2\\sewer\\walls.dt1"),
        bf("chamb", "act2_sewer_chamb.dt1", "\\d2\\data\\global\\tiles\\act2\\sewer\\chamb.dt1"),
        bf("items", "act2_sewer_items.dt1", "\\d2\\data\\global\\tiles\\act2\\sewer\\items.dt1"),
        bf("radament", "act2_sewer_radament.dt1", "\\d2\\data\\global\\tiles\\act2\\sewer\\radament.dt1"),
    ],
};

/// LvlType 19 « Act 2 - Arcane » — Sanctuaire Arcane : 81 tuiles.
pub const BANK_ARCANE: MazeBank = MazeBank {
    level_type: 19,
    name: "Act 2 - Arcane",
    files: &[bf(
        "sanctuary",
        "act2_arcane_sanctuary.dt1",
        "\\d2\\data\\global\\tiles\\act2\\arcane\\sanctuary.dt1",
    )],
};

/// LvlType 3 « Act 1 - Cave » — Den of Evil et caves A1 : 212 tuiles.
pub const BANK_CAVE: MazeBank = MazeBank {
    level_type: 3,
    name: "Act 1 - Cave",
    files: &[bf("cave", "act1_cave.dt1", "\\d2\\data\\global\\tiles\\act1\\caves\\cave.dt1")],
};

/// Les banques supportées (DrlgType 1 actes I-II dans l'état actuel des
/// extraits ; LvlTypes 4-12/20+ attendent leurs DT1 acte 3-5).
pub const BANKS: [MazeBank; 4] = [BANK_TOMB, BANK_SEWER, BANK_ARCANE, BANK_CAVE];

/// Stem minuscule d'un chemin LvlTypes (`Act2/Tomb/Tomb.dt1` → `tomb`).
fn stem_of(path: &str) -> &str {
    path.rsplit(['/', '\\'])
        .next()
        .unwrap_or(path)
        .split('.')
        .next()
        .unwrap_or("")
}

/// Banque d'un niveau : record levels.txt (`DrlgType` 1 exigé) → LvlType →
/// banque dont CHAQUE fichier est vérifié présent dans les colonnes
/// `File N` du LvlType (sélection data-driven, pas un hard-code aveugle).
pub fn bank_for_level(level_id: u32) -> Option<&'static MazeBank> {
    let rec = level_record(level_id)?;
    if rec.drlg_type != 1 {
        return None;
    }
    let types = parse_lvl_types(LVL_TYPES).ok()?;
    let lt = types.iter().find(|t| t.id == rec.level_type)?;
    let bank = BANKS.iter().find(|b| b.level_type == rec.level_type)?;
    // Vérification LvlTypes : chaque stem de la banque est un File N réel.
    for f in bank.files {
        if !lt.files.iter().any(|p| stem_of(p).eq_ignore_ascii_case(f.stem)) {
            return None;
        }
    }
    Some(bank)
}

/// Record levels.txt du niveau (`None` si id inconnu).
pub fn level_record(level_id: u32) -> Option<crate::levels::LevelRecord> {
    LevelTables::parse(PATCH_LEVELS).ok()?.get(level_id).cloned()
}

/// Record LvlMaze du niveau (`None` si la grille n'existe pas).
pub fn maze_record(level_id: u32) -> Option<crate::levels::LvlMazeRecord> {
    parse_lvl_maze(LVL_MAZE)
        .ok()?
        .into_iter()
        .find(|m| m.level == level_id)
}

/// Ce niveau est-il générable par ce module ? (record DrlgType 1 + grille
/// LvlMaze + banque LvlTypes vérifiée.)
pub fn supports(level_id: u32) -> bool {
    maze_record(level_id).is_some() && bank_for_level(level_id).is_some()
}

// ---------------------------------------------------------------------------
// Génération
// ---------------------------------------------------------------------------

/// Marge de roche autour du sol d'une room (bord interne de chaque bloc).
const INSET: usize = 2;
/// Demi-largeur des couloirs : 3 tuiles de large (centre ±1).
const CORRIDOR_HALF: usize = 1;

/// Grille de rooms : ⌈√rooms⌉, +1 mou dès 3 rooms (la pose seedée de
/// [`crate::drlg::plan_maze`] exige un voisin déjà posé — le mou garantit
/// la connexité sans relâcher la contrainte).
fn plan_grid(rooms: u32) -> u32 {
    let side = (rooms.max(1) as f64).sqrt().ceil() as u32;
    side + u32::from(rooms >= 3)
}

/// Statistiques de génération (notes du moteur + tests).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MazeStats {
    /// Rooms planifiées (posées = `Rooms` de LvlMaze).
    pub rooms: usize,
    /// Couloirs creusés (une arête d'arbre couvrant = un couloir).
    pub corridors: usize,
    /// Cellules de sol évidées.
    pub floor_cells: usize,
    /// Cellules de roche (tuile mur).
    pub wall_cells: usize,
}

/// Centre (tuile) d'une room du plan.
pub fn room_center(room: &crate::drlg::MazeRoom, sx: usize, sy: usize) -> (usize, usize) {
    (room.gx as usize * sx + sx / 2, room.gy as usize * sy + sy / 2)
}

/// Rejoue le plan de rooms du niveau (même graine que [`generate`] —
/// déterministe) : utile aux tests/callers pour localiser les rooms.
pub fn plan_for(level_id: u32, seed: u64) -> Result<crate::drlg::MazePlan, String> {
    let maze = maze_record(level_id)
        .ok_or_else(|| format!("drlg maze : niveau {level_id} sans record LvlMaze"))?;
    let side = plan_grid(maze.rooms);
    let plan_rec = crate::levels::LvlMazeRecord { size_x: side, size_y: side, ..maze };
    crate::drlg::plan_maze(&plan_rec, seed, &[0])
}

/// Génère le niveau maze `level_id` seedé — FONCTION PURE : les tables
/// viennent des TSV embarquées, les tuiles du paramètre `tilesets`
/// (concaténation des conteneurs de la banque DANS L'ORDRE
/// [`MazeBank::files`] — `game.rs::build_maze_tiles` produit exactement
/// cet ordre). Retourne le [`Ds1`] synthétique + les stats ; erreur
/// explicite sur toute donnée incohérente (jamais de carte invalide
/// silencieuse).
pub fn generate(level_id: u32, seed: u64, tilesets: &[Dt1Tile]) -> Result<(Ds1, MazeStats), String> {
    let rec = level_record(level_id)
        .ok_or_else(|| format!("drlg maze : niveau {level_id} absent de levels.txt"))?;
    if rec.drlg_type != 1 {
        return Err(format!(
            "drlg maze : niveau {level_id} ({}) DrlgType {} != 1",
            rec.name, rec.drlg_type
        ));
    }
    let maze = maze_record(level_id)
        .ok_or_else(|| format!("drlg maze : niveau {level_id} sans record LvlMaze"))?;
    let bank = bank_for_level(level_id)
        .ok_or_else(|| format!("drlg maze : niveau {level_id} ({}) sans banque supportée", rec.name))?;
    if tilesets.len() > 256 {
        return Err(format!(
            "drlg maze : espace de tuiles {} > 256 (p1 du DS1 est un u8)",
            tilesets.len()
        ));
    }

    // --- pools réels : sols / murs de la banque -----------------------------
    // Piège DS1 : p1 == 0 encode la CELLULE VIDE — la tuile d'indice 0 de
    // l'espace concaténé est donc inutilisable (trou de roche / sol manquant)
    // ; elle est exclue des deux pools.
    let floors: Vec<usize> =
        (1..tilesets.len()).filter(|&i| tilesets[i].is_floor).collect();
    let walls: Vec<usize> =
        (1..tilesets.len()).filter(|&i| !tilesets[i].is_floor).collect();
    if floors.is_empty() || walls.is_empty() {
        return Err(format!(
            "drlg maze : banque « {} » sans sol ({}) ou sans mur ({})",
            bank.name,
            floors.len(),
            walls.len()
        ));
    }

    // --- plan de rooms seedé (arbre couvrant, réutilise drlg::plan_maze) ----
    let (sx, sy) = (
        (maze.size_x as usize).max(2 * INSET + 2),
        (maze.size_y as usize).max(2 * INSET + 2),
    );
    let plan = plan_for(level_id, seed)?;
    if plan.rooms.len() != maze.rooms as usize {
        return Err(format!(
            "drlg maze : {} rooms posées au lieu de {} (grille saturée ?)",
            plan.rooms.len(),
            maze.rooms
        ));
    }
    let (w, h) = (plan.grid_w as usize * sx, plan.grid_h as usize * sy);

    let mut rng = crate::drlg::SplitMix64::new(seed);

    // --- canevas : sol réel partout (brassé par le RNG) ----------------------
    let mut layers: [Vec<i32>; 6] = std::array::from_fn(|_| vec![0i32; w * h]);
    for cell in layers[4].iter_mut() {
        let idx = floors[rng.below(floors.len())];
        *cell = idx as i32; // p1 = idx (≤ 255), p2/p3/orient nuls
    }

    // --- masque de sol : rooms évidées + couloirs ----------------------------
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
        for y in (y0 + INSET)..(y0 + sy - INSET) {
            for x in (x0 + INSET)..(x0 + sx - INSET) {
                carve(x, y);
            }
        }
    }
    // Couloirs : une arête = un tunnel 3 tuiles entre les deux aires de sol
    // (à travers les marges de roche). Chaque arête posée une fois (E/S).
    let mut corridors = 0usize;
    for room in &plan.rooms {
        let (cx, cy) = room_center(room, sx, sy);
        if room.open_e {
            let x0 = room.gx as usize * sx + sx - INSET;
            for x in x0..(x0 + 2 * INSET) {
                for y in (cy - CORRIDOR_HALF)..=(cy + CORRIDOR_HALF) {
                    carve(x, y);
                }
            }
            corridors += 1;
        }
        if room.open_s {
            let y0 = room.gy as usize * sy + sy - INSET;
            for y in y0..(y0 + 2 * INSET) {
                for x in (cx - CORRIDOR_HALF)..=(cx + CORRIDOR_HALF) {
                    carve(x, y);
                }
            }
            corridors += 1;
        }
    }

    // --- roche : tuile mur seedée sur toute cellule non évidée ---------------
    let mut wall_cells = 0usize;
    for (i, cell) in layers[1].iter_mut().enumerate() {
        if !open[i] {
            *cell = walls[rng.below(walls.len())] as i32;
            wall_cells += 1;
        }
    }

    // --- validations ---------------------------------------------------------
    if layers[4].iter().any(|v| (*v & 0xFF) == 0) {
        return Err("drlg maze : sol incomplet".into());
    }
    for li in 0..6 {
        for &v in &layers[li] {
            if (v & 0xFF) as usize >= tilesets.len() {
                return Err(format!(
                    "drlg maze : indice de tuile {} hors espace ({})",
                    v & 0xFF,
                    tilesets.len()
                ));
            }
        }
    }
    // Connexité : BFS des cellules de sol, tous les centres de rooms
    // atteints depuis le premier (aucune île — la marchabilité des chunks).
    let mut seen = vec![false; w * h];
    let mut queue = std::collections::VecDeque::new();
    let (sx0, sy0) = room_center(&plan.rooms[0], sx, sy);
    seen[sy0 * w + sx0] = true;
    queue.push_back((sx0, sy0));
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
        let (cx, cy) = room_center(room, sx, sy);
        if !seen[cy * w + cx] {
            return Err(format!(
                "drlg maze : room ({},{}) inatteignable (plan non connexe)",
                room.gx, room.gy
            ));
        }
    }
    // Bords de carte scellés (roche sur le pourtour).
    for y in 0..h {
        for x in [0usize, w - 1] {
            if open[y * w + x] {
                return Err("drlg maze : bord de carte ouvert (E/O)".into());
            }
        }
    }
    for x in 0..w {
        for y in [0usize, h - 1] {
            if open[y * w + x] {
                return Err("drlg maze : bord de carte ouvert (N/S)".into());
            }
        }
    }

    let stats = MazeStats { rooms: plan.rooms.len(), corridors, floor_cells, wall_cells };
    let ds1 = Ds1 {
        version: 18,
        w,
        h,
        files: bank.files.iter().map(|f| f.tg1.to_string()).collect(),
        layers: layers.to_vec(),
    };
    Ok((ds1, stats))
}

// ---------------------------------------------------------------------------
// Tests — vraies tables, vrais DT1 extraits, déterminisme, marchabilité
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::ComposedMap;
    use crate::dt1::decode_dt1;

    /// Racine du workspace (d2w-actes) — `None` hors checkout complet.
    fn ws_root() -> Option<std::path::PathBuf> {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .map(|p| p.to_path_buf())
    }

    /// Tilesets RÉELS d'une banque (DT1 extraits des MPQ dans
    /// `assets/map/`), dans l'ordre [`MazeBank::files`].
    fn real_bank_tiles(bank: &MazeBank) -> Option<Vec<Dt1Tile>> {
        let root = ws_root()?;
        let mut tilesets = Vec::new();
        for f in bank.files {
            let bytes = std::fs::read(root.join("assets/map").join(f.local))
                .unwrap_or_else(|e| panic!("{} : {e} (extrait MPQ absent)", f.local));
            let tiles = decode_dt1(&bytes).unwrap_or_else(|e| panic!("{} : {e:?}", f.local));
            assert!(!tiles.is_empty(), "{} : 0 tuile", f.local);
            tilesets.extend(tiles);
        }
        Some(tilesets)
    }

    // --- tables réelles -------------------------------------------------------

    #[test]
    fn maze_tables_cover_real_levels_and_reject_others() {
        // Les niveaux maze des tables v1.12 réelles : Stony Tomb 1 (55,
        // LvlType 17), Ancient Tunnels (65, type 13), Arcane Sanctuary
        // (74, type 19), Den of Evil (8, type 3).
        for id in [8u32, 55, 65, 74] {
            let rec = level_record(id).unwrap_or_else(|| panic!("{id} : record absent"));
            assert_eq!(rec.drlg_type, 1, "{id} : {}", rec.name);
            let maze = maze_record(id).unwrap_or_else(|| panic!("{id} : LvlMaze absent"));
            assert!(maze.rooms >= 1, "{id} : rooms {}", maze.rooms);
            let bank = bank_for_level(id).unwrap_or_else(|| panic!("{id} : banque absente"));
            assert_eq!(bank.level_type, rec.level_type, "{id}");
            assert!(supports(id), "{id} non supporté");
        }
        // Colonnes LvlMaze réelles (Go level_maze_loader.go:14-18).
        let t65 = maze_record(65).unwrap();
        assert_eq!((t65.rooms, t65.size_x, t65.size_y), (10, 12, 12));
        let t55 = maze_record(55).unwrap();
        assert_eq!((t55.rooms, t55.size_x, t55.size_y), (4, 16, 16));
        let t74 = maze_record(74).unwrap();
        assert_eq!((t74.rooms, t74.size_x, t74.size_y), (61, 12, 12));
        // Rejets propres : wilderness (2), preset (1), maze sans banque
        // (Barracks 28, LvlType 7 non bancarisé).
        assert!(!supports(2) && bank_for_level(2).is_none());
        assert!(!supports(1) && bank_for_level(1).is_none());
        assert_eq!(level_record(28).unwrap().drlg_type, 1);
        assert!(!supports(28) && bank_for_level(28).is_none());
    }

    #[test]
    fn real_banks_fit_u8_tile_space_and_have_both_kinds() {
        for bank in &BANKS {
            let Some(tilesets) = real_bank_tiles(bank) else {
                panic!("DT1 de « {} » absents de assets/map (extraction MPQ)", bank.name);
            };
            assert!(
                tilesets.len() <= 256,
                "banque {} : {} tuiles > espace u8",
                bank.name,
                tilesets.len()
            );
            let floors = tilesets.iter().filter(|t| t.is_floor).count();
            let walls = tilesets.iter().filter(|t| !t.is_floor).count();
            assert!(floors >= 8, "banque {} : {floors} sols", bank.name);
            assert!(walls >= 8, "banque {} : {walls} murs", bank.name);
        }
    }

    // --- génération seedée ----------------------------------------------------

    #[test]
    fn tomb_and_tunnels_are_deterministic_and_seed_sensitive() {
        for id in [55u32, 65] {
            let tiles = real_bank_tiles(bank_for_level(id).unwrap()).unwrap();
            let (a, sa) = generate(id, 42, &tiles).unwrap();
            let (b, sb) = generate(id, 42, &tiles).unwrap();
            assert_eq!(a.layers, b.layers, "niveau {id} : même graine ⇒ mêmes couches");
            assert_eq!(sa, sb);
            let (c, _) = generate(id, 43, &tiles).unwrap();
            assert_ne!(a.layers, c.layers, "niveau {id} : graine différente ⇒ carte différente");
            // La graine de partie se combine à la clé de niveau (drlg).
            assert_ne!(
                crate::drlg::level_seed(7, "ancienttunnels"),
                crate::drlg::level_seed(7, "tombf1")
            );
        }
    }

    #[test]
    fn generated_mazes_are_sane() {
        // (id, rooms attendues, taille de bloc LvlMaze)
        for (id, rooms, sx, sy) in [(55u32, 4usize, 16usize, 16usize), (65, 10, 12, 12)] {
            let tiles = real_bank_tiles(bank_for_level(id).unwrap()).unwrap();
            let (ds1, stats) = generate(id, 20261024, &tiles).unwrap();
            assert_eq!(ds1.version, 18);
            let maze = maze_record(id).unwrap();
            let side = plan_grid(maze.rooms) as usize;
            assert_eq!((ds1.w, ds1.h), (side * sx, side * sy), "niveau {id}");
            assert_eq!(stats.rooms, rooms, "toutes les rooms de LvlMaze posées");
            // Arbre couvrant : ≥ rooms-1 arêtes (couloirs creusés).
            assert!(stats.corridors >= stats.rooms - 1, "niveau {id} : {stats:?}");
            // Sol 100 % rempli, 0 indice hors bornes (validateur interne).
            assert!(ds1.layers[4].iter().all(|v| (*v & 0xFF) != 0), "niveau {id} : sol plein");
            // Le sol évidé est minoritaire (un labyrinthe, pas une plaine)…
            let ratio = stats.floor_cells as f32 / (ds1.w * ds1.h) as f32;
            assert!(
                (0.15..0.75).contains(&ratio),
                "niveau {id} : ratio de sol {ratio:.2} hors plafond de maze"
            );
            // …et la roche le recouvre ailleurs (murs en couche 1).
            assert_eq!(
                ds1.layers[1].iter().filter(|v| (**v & 0xFF) != 0).count(),
                stats.wall_cells,
                "niveau {id} : une tuile mur par cellule de roche"
            );
            // Ombre vide (comme les presets bloodmoor).
            assert!(ds1.layers[5].iter().all(|v| *v == 0));
        }
    }

    #[test]
    fn generate_rejects_bad_inputs() {
        let tiles = real_bank_tiles(&BANK_SEWER).unwrap();
        // Id inconnu / non maze.
        assert!(generate(9999, 1, &tiles).is_err());
        assert!(generate(2, 1, &tiles).is_err(), "Blood Moor = DrlgType 3");
        // Espace de tuiles rétréci : indices hors bornes détectés.
        assert!(generate(65, 1, &tiles[..3]).is_err());
    }

    // --- composition (pipeline réel) + marchabilité des chunks ----------------

    /// Marchabilité « des chunks » : chaque centre de room (plan rejoué
    /// avec la même graine) a une sous-tuile marchable à proximité, et le
    /// BFS sur la grille de marche composée les atteint TOUS.
    fn assert_rooms_reachable(
        map: &ComposedMap,
        id: u32,
        seed: u64,
        sx: usize,
        sy: usize,
    ) {
        let plan = plan_for(id, seed).unwrap();
        let snaps: Vec<(usize, usize)> = plan
            .rooms
            .iter()
            .map(|room| {
                let (cx, cy) = room_center(room, sx, sy);
                let (gx, gy) = (cx * 5 + 2, cy * 5 + 2);
                crate::compose::find_nearby_walkable(&map.walk, map.grid_w, map.grid_h, gx, gy, 12)
                    .unwrap_or_else(|| panic!("centre de room ({gx},{gy}) sans marche à proximité"))
            })
            .collect();
        let mut reach = vec![false; map.grid_w * map.grid_h];
        let (x0, y0) = snaps[0];
        reach[y0 * map.grid_w + x0] = true;
        let mut q = std::collections::VecDeque::new();
        q.push_back((x0, y0));
        while let Some((x, y)) = q.pop_front() {
            for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= map.grid_w as i64 || ny >= map.grid_h as i64 {
                    continue;
                }
                let ni = ny as usize * map.grid_w + nx as usize;
                if !reach[ni] && map.walk[ni] {
                    reach[ni] = true;
                    q.push_back((nx as usize, ny as usize));
                }
            }
        }
        for (i, &(x, y)) in snaps.iter().enumerate() {
            assert!(reach[y * map.grid_w + x], "room {i} ({x},{y}) inatteignable à pied");
        }
    }

    #[test]
    fn composed_tomb_and_tunnels_have_walkable_chunks() {
        for id in [55u32, 65] {
            let tiles = real_bank_tiles(bank_for_level(id).unwrap()).unwrap();
            let (ds1, _) = generate(id, 7, &tiles).unwrap();
            let map = crate::compose::compose_seeded(&ds1, &tiles, 1);
            let ratio = map.walk.iter().filter(|w| **w).count() as f32 / map.walk.len() as f32;
            assert!(
                (0.15..0.75).contains(&ratio),
                "niveau {id} : ratio de marche {ratio:.2}"
            );
            // Les pixels de sol sont peints (pas une carte noire).
            assert!(map.idx.iter().any(|v| *v != 0), "niveau {id} : framebuffer vide");
            let maze = maze_record(id).unwrap();
            assert_rooms_reachable(&map, id, 7, maze.size_x as usize, maze.size_y as usize);
        }
    }

    #[test]
    fn composed_arcane_sanctuary_is_walkable() {
        // 61 rooms (LvlMaze réel) — test isolé (canvas 108×108, buffers de
        // compose volumineux).
        let id = 74u32;
        let tiles = real_bank_tiles(bank_for_level(id).unwrap()).unwrap();
        let (ds1, stats) = generate(id, 7, &tiles).unwrap();
        assert_eq!(stats.rooms, 61);
        let map = crate::compose::compose_seeded(&ds1, &tiles, 1);
        let ratio = map.walk.iter().filter(|w| **w).count() as f32 / map.walk.len() as f32;
        assert!((0.10..0.60).contains(&ratio), "ratio arcane {ratio:.2}");
        let maze = maze_record(id).unwrap();
        assert_rooms_reachable(&map, id, 7, maze.size_x as usize, maze.size_y as usize);
    }

    #[test]
    fn plan_grid_keeps_connectivity_slack() {
        assert_eq!(plan_grid(0), 1);
        assert_eq!(plan_grid(1), 1, "Den of Evil : une room, pas de mou");
        assert_eq!(plan_grid(2), 2);
        assert_eq!(plan_grid(4), 3, "mou dès 3 rooms (pose seedée à voisin requis)");
        assert_eq!(plan_grid(10), 4 + 1);
        assert_eq!(plan_grid(61), 8 + 1);
    }
}
