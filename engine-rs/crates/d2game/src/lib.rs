//! d2game — rendering engine for the Diablo 2 browser MVP.
//!
//! Owns everything between "decoded bytes" and "pixels on screen":
//!
//! * [`dt1`] / [`ds1`] — the tile-container and level-layout decoders
//!   (release-v1.12 quirks baked in, ported from the validated prototype
//!   `tools/map_proto2.py`, render proof `assets/map/level-render.png`).
//! * [`compose`] — isometric map compositor producing (a) a palette-index
//!   framebuffer and (b) the boolean walkability grid at sub-tile
//!   resolution, exported for `d2rules`.
//! * [`astar`] — 8-direction pathfinding over that grid (diagonal costs).
//! * [`sprite`] — DCC/COF sprite composition: layers stacked in COF draw
//!   order, direction picked from a 0–15 facing.
//! * [`levels`] — the runtime level-tables reader (W01: levels/LvlTypes/
//!   LvlPrest/LvlMaze/LvlWarp TSV) + the Vis/Warp transition graph (W04).
//! * [`drlg`] — the seeded wilderness DRLG (W02 phase 1: the Blood Moor,
//!   ported from `tools/bloodmoor_build.py`, SplitMix64-seeded per game).
//! * [`game`] — the game loop: `GameCore` consumes the manifest + asset
//!   blobs, ticks the real `d2rules::World`, and renders an RGBA viewport.
//! * [`wasm`] — `#[wasm_bindgen]` glue (only compiled for wasm32).
//!
//! No file I/O outside tests: assets arrive as `Vec<u8>` so the same core
//! runs natively (`cargo test`) and in the browser.

pub mod astar;
pub mod automap; // 9d monde vivant : résolution AutoMap.txt + JSON mini-carte (demo/automap.js)

/// DRLG fin de jeu A4-A5 (étape 10a) — Pandemonium Fortress, Harrogath
/// (townsiege.ds1 réel), Bloody Foothills, Crystalized Cavern (maze),
/// Halls of Vaught (preset ×4 seedé) — `drlg_a45.rs`.
pub mod drlg_a45;
pub mod compose;
pub mod drlg;
pub mod ds1;
/// DS1 complet + section PATH (patrouilles NPC) — port `ds1_path`
/// (`ds1_path.rs`), compagnon additif du décodeur de rendu `ds1`.
pub mod ds1_path;
pub mod dt1;
/// Flags de sous-tuiles DT1 (25/tuile) + grille de collision fine 1/5 —
/// port `dt1_flags` (`dt1_flags.rs`), compagnon additif du décodeur `dt1`.
pub mod dt1_flags;
/// DRLG maze seedé (DrlgType 1, étape 8a) — tombeaux/égouts/sanctuaire A2
/// depuis LvlMaze/LvlTypes réels + DT1 extraits (`drlg_maze.rs`).
pub mod drlg_maze;
/// Élites champion/unique data-driven (étape 9b monde vivant, W09) —
/// catalogue monumod + constantes officielles, tirage seedé, `EliteInfo`
/// JSON (additif au plan de spawn) (`elite_mods.rs`).
pub mod elite_mods;
pub mod game;
pub mod levels;
/// Contenu monstre data-driven (W06-W08 + super-uniques + MonLvl) —
/// tables v1.12 réelles + plan de spawn pur (`monsters.rs`).
pub mod monsters;
/// NPC de ville W16 côté moteur (attach au camp, état JSON UI, talkNpc) —
/// blocs `impl GameCore` additifs (`npc_bridge.rs`).
pub mod npc_bridge;
/// Objets interactifs data-driven 9c (section OBJECTS des DS1, objects.txt +
/// objgroup.txt, coffres TC/shrines/portes/portails) — plan testable au
/// niveau, câblage game.rs/wasm au pas d'intégration (`objects_live.rs`).
pub mod objects_live;
pub mod pal;
/// Palettes d'actes : conteneur runtime `pal.pl2` v1.12 (acts 1-5) et
/// ponts vers les deux chemins de couleur du renderer (`palette/pl2.rs`).
pub mod palette;
pub mod png_mini;
/// Spawn extérieur data-driven (socle W01/A3) — planches mon1-10 de
/// levels.txt + packs MinGrp/MaxGrp de monstats.txt via `d2rules::tsv`
/// (`spawn.rs`), consommé par `LevelDef::from_manifest`.
pub mod spawn;
/// Super-uniques data-driven (étape 9a — monde vivant) : `superuniques.rs`.
pub mod superuniques;
pub mod sprite;
/// Texte fr-FR in-engine (W18) — noms de niveaux/monstres servis depuis les
/// TBL v1.12 réelles (embarquées), repli manifest ; blocs `impl GameCore`
/// additifs (`tbl_names.rs`).
pub mod tbl_names;
/// Consommables 11b côté moteur (wire minimal d2rules::consumables :
/// useConsumable/useBeltKey/beltAutoStore + retour camp via switch_level
/// existant) — blocs `impl GameCore` additifs (`consumables_ui.rs`).
pub mod consumables_ui;
/// Graphe Vis/Warp générique 8c (neighbors tout le jeu, WarpDef depuis la
/// table, entry_points contrat DRLG) — `warp_graph.rs`.
pub mod warp_graph;
/// Waypoints W05 côté moteur (attach, état JSON UI, voyage direct) —
/// blocs `impl GameCore` additifs (`waypoint_travel.rs`).
pub mod waypoint_travel;
pub mod world;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

/// The `World` behind the game loop: the real `d2rules` implementation.
pub use d2rules as world_impl;
