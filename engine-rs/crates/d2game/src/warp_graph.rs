//! Graphe de transitions Vis/Warp GÉNÉRIQUE (8c, campagne d2w-actes) —
//! résolution depuis les tables réelles v1.12 (`patch_levels.txt` colonnes
//! `Vis0-7`/`Warp0-7` + `LvlWarp.txt`) pour TOUT LE JEU, au lieu du seul
//! couple de warps du manifest (town ↔ Blood Moor, 2 blocs `warp`).
//!
//! # Ce que ce module ajoute (8c-1 / 8c-3)
//!
//! * [`WarpResolver::neighbors`] — les voisins d'un niveau pour les 137
//!   records de la table (graphe non orienté [`WarpGraph`], cf. `levels.rs`
//!   W04), avec noms/acte/types LvlWarp résolus. `patch_levels.txt` porte
//!   **94 paires de warps non orientées uniques** (golden test plus bas) —
//!   le manifest n'en porte qu'une.
//! * [`WarpDef::from_table`] — adaptation du `WarpDef` historique (bloc
//!   manifest `warp` de `game.rs`) pour LIRE LA TABLE : `to` devient la clé
//!   `"level:{id}"` déjà résolue par `GameCore::resolve_level_def` (W04),
//!   la géométrie vient de LvlWarp + taille de grille. Fonctionne pour les
//!   niveaux SANS bloc manifest (Den of Evil, toutes les caves/mazes…).
//! * [`WarpResolver::entry_points`] — contrat DRLG (8c-3) : points d'entrée
//!   candidats d'un niveau généré (maze/preset en cours chez les coéquipiers),
//!   depuis LvlWarp (`SelectX/Y`, `ExitWalkX/Y`, `Direction`) ou repli coins
//!   de grille. Contrat simple : ils brancheront.
//!
//! # Preuves (tables réelles embarquées, golden tests)
//!
//! * `assets/patch_levels.txt` (v1.12) — ex. Den of Evil (id 8) `Vis0=2
//!   Warp0=4` (« Act 1 Cave Up ») ; Lut Gholein (id 40) `Vis2/3=47`,
//!   `Vis4=50` ; Canyon of the Magi (46) → 7 tombes (66-72) ; le town (1)
//!   n'a AUCUN slot Vis (sa transition reste portée par le manifest).
//! * `assets/LvlWarp.txt` (89 records) — `SelectX/Y` + `SelectDX/DY` =
//!   rectangle de sélection relatif à l'ancre du warp dans le DS1 ;
//!   `ExitWalkX/Y` = déplacement du héros après traversée ; `Direction`
//!   vaut `b` (78), `l` (5) ou `r` (5).
//!
//! # Convention de position (assumée, documentée)
//!
//! Les tables ne portent PAS la position absolue des warps dans le niveau
//! (elle vit dans les objets DS1, posés par le DRLG). Le contrat expose donc
//! des points candidats en **cellules de grille d2rules** (1 tuile
//! `SizeX/SizeY` = [`CELLS_PER_TILE`] cellules, cf. waypoint (144.5, 51.5)
//! du town) : bord de grille correspondant à `Direction` (`b` sud, `t` nord,
//! `l` ouest, `r` est — 2 cellules dedans), sinon coin nord-ouest. Le DRLG
//! remplace ces candidats par ses ancres réelles sans changer l'API.

use crate::game::WarpDef;
use crate::levels::{parse_lvl_warp, LevelTables, LvlWarpRecord, WarpGraph};

/// Facteur grille : 1 tuile levels.txt (`SizeX`/`SizeY`) = 5 cellules de la
/// grille d2rules — l'unité des positions warp/waypoint du manifest (ex.
/// sortie Est du camp à x=282 pour 56 tuiles × 5 = 280).
pub const CELLS_PER_TILE: f32 = 5.0;

/// Rayon de zone warp conventionnel du port (les 2 blocs `warp` du manifest
/// portent `r: 4`).
pub const WARP_RADIUS: f32 = 4.0;

/// Profondeur (cellules) du point `at` depuis le bord de grille.
pub const BORDER_INSET: f32 = 2.0;
/// Profondeur (cellules) du point d'arrivée `entry` — hors de la zone warp
/// (|15 − 2| = 13 > `WARP_RADIUS`), même convention que le manifest
/// (« arrivée … hors zone », ex. at (137,147) → entry (137,162)).
pub const ENTRY_INSET: f32 = 15.0;

/// Un voisin de graphe (8c-1) — enrichi depuis les tables, tout le jeu.
#[derive(Debug, Clone, PartialEq)]
pub struct Neighbor {
    /// Id levels.txt du voisin.
    pub id: u32,
    /// Nom technique (`Act 2 - Sewer 1 A`).
    pub name: String,
    /// Nom affichable EN (`LevelName`, ex. `Lut Gholein`) — repli `name`.
    pub level_name: String,
    /// Acte retail 1..=5 (colonne `Act` + 1).
    pub act: u8,
    /// Slot Vis d'origine de la déclaration (0-7).
    pub slot: u8,
    /// Type LvlWarp du lien (-1 si la table n'en déclare pas).
    pub warp_type: i32,
    /// Nom LvlWarp du type (`Act 1 Cave Up`), `""` si type absent/inconnu.
    pub warp_name: String,
}

/// Source d'un point d'entrée candidat (8c-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntrySource {
    /// Bord de grille déduit de la `Direction` du type LvlWarp du lien.
    LvlWarpBorder,
    /// Lien sans type LvlWarp exploitable (`Warp_i` < 0 ou inconnu) : coin
    /// nord-ouest de grille ([`GRID_CORNER_INSET`], [`GRID_CORNER_INSET`]).
    GridCorner,
}

/// Profondeur (cellules) du repli coin de grille.
pub const GRID_CORNER_INSET: f32 = 2.0;

/// Point d'entrée candidat d'un niveau (contrat DRLG 8c-3).
#[derive(Debug, Clone, PartialEq)]
pub struct EntryPoint {
    /// Niveau voisin relié par ce point.
    pub to: u32,
    /// Slot Vis d'origine (0-7) — plusieurs slots peuvent décrire des warps
    /// physiques distincts en retail (wilderness : 4 bords) ; dédoublonnés
    /// ici par (voisin, type, position), premier slot gardé.
    pub slot: u8,
    /// Type LvlWarp du lien (-1 si aucun).
    pub warp_type: i32,
    /// Nom LvlWarp du type ("" si inconnu).
    pub warp_name: String,
    /// Direction d'entrée du type (`b`/`l`/`r`/`t`, "" si inconnue).
    pub direction: String,
    /// Point candidat en cellules de grille d2rules (dans les bornes).
    pub pos: (f32, f32),
    /// Centre du rectangle de sélection LvlWarp (`SelectX/Y + DX/DY/2`) —
    /// REPÈRE RELATIF à l'ancre de warp que le DRLG posera (offset de clic).
    pub select_center: Option<(f32, f32)>,
    /// Déplacement après traversée (`ExitWalkX/Y`) — même repère relatif.
    pub exit_walk: Option<(i32, i32)>,
    /// D'où vient ce candidat (bord LvlWarp ou coin de grille).
    pub source: EntrySource,
}

/// Résolveur de graphe enrichi : tables + graphe W04 + géométrie LvlWarp.
/// Vues pures — aucune mutation, aucun I/O (tables fournies par l'appelant,
/// asset `levels` + asset `LvlWarp` du manifest à l'intégration).
pub struct WarpResolver<'a> {
    tables: &'a LevelTables,
    graph: &'a WarpGraph,
    lvl_warp: Vec<LvlWarpRecord>,
}

impl<'a> WarpResolver<'a> {
    /// `lvl_warp_tsv` = texte `LvlWarp.txt` (asset runtime ou include_str!).
    pub fn new(
        tables: &'a LevelTables,
        graph: &'a WarpGraph,
        lvl_warp_tsv: &str,
    ) -> Result<WarpResolver<'a>, String> {
        Ok(WarpResolver {
            tables,
            graph,
            lvl_warp: parse_lvl_warp(lvl_warp_tsv)?,
        })
    }

    fn lvl_warp_record(&self, warp_type: i32) -> Option<&LvlWarpRecord> {
        if warp_type < 0 {
            return None;
        }
        self.lvl_warp.iter().find(|w| w.id == warp_type as u32)
    }

    /// Voisins du niveau (8c-1) — graphe W04 enrichi, tout le jeu. Ordre du
    /// graphe (slot, to), déterministe.
    pub fn neighbors(&self, level_id: u32) -> Vec<Neighbor> {
        let Some(rec) = self.tables.get(level_id) else {
            return Vec::new();
        };
        self.graph
            .edges_of(level_id)
            .iter()
            .map(|e| {
                let (name, level_name, act) = match self.tables.get(e.to) {
                    Some(n) => (
                        n.name.clone(),
                        if n.level_name.is_empty() { n.name.clone() } else { n.level_name.clone() },
                        (n.act + 1).min(u8::MAX as u32) as u8,
                    ),
                    None => (String::new(), String::new(), rec.act.min(254) as u8 + 1),
                };
                Neighbor {
                    id: e.to,
                    name,
                    level_name,
                    act,
                    slot: e.slot,
                    warp_type: e.warp_type,
                    warp_name: self
                        .lvl_warp_record(e.warp_type)
                        .map(|w| w.name.clone())
                        .unwrap_or_default(),
                }
            })
            .collect()
    }

    /// Points d'entrée candidats du niveau (8c-3) — contrat DRLG : un
    /// candidat par (voisin, type, position) dédoublonné, position dans les
    /// bornes de grille du niveau, géométrie LvlWarp relative jointe.
    pub fn entry_points(&self, level_id: u32) -> Vec<EntryPoint> {
        let Some(rec) = self.tables.get(level_id) else {
            return Vec::new();
        };
        let (w, h) = grid_size(rec.size_x, rec.size_y);
        let mut out: Vec<EntryPoint> = Vec::new();
        for slot in 0..8usize {
            let vis = rec.vis[slot];
            if vis <= 0 {
                continue;
            }
            let wt = rec.warp[slot];
            let lw = self.lvl_warp_record(wt);
            let dir = lw.map(|r| r.direction.clone()).unwrap_or_default();
            let (pos, source) = match lw.as_ref() {
                Some(_) => (border_point(w, h, &dir), EntrySource::LvlWarpBorder),
                None => (
                    (GRID_CORNER_INSET, GRID_CORNER_INSET),
                    EntrySource::GridCorner,
                ),
            };
            let candidate = (vis as u32, wt, (pos.0.to_bits(), pos.1.to_bits()));
            if out.iter().any(|e| {
                (e.to, e.warp_type, (e.pos.0.to_bits(), e.pos.1.to_bits())) == candidate
            }) {
                continue;
            }
            out.push(EntryPoint {
                to: vis as u32,
                slot: slot as u8,
                warp_type: wt,
                warp_name: lw.map(|r| r.name.clone()).unwrap_or_default(),
                direction: dir,
                pos,
                select_center: lw.map(|r| {
                    (r.select_x as f32 + r.select_dx as f32 / 2.0,
                     r.select_y as f32 + r.select_dy as f32 / 2.0)
                }),
                exit_walk: lw.map(|r| (r.exit_walk_x, r.exit_walk_y)),
                source,
            });
        }
        out
    }

    /// Warps du niveau dérivés de la TABLE (8c-1) — un [`WarpDef`] par
    /// voisin dédoublonné, prêt à nourrir un niveau sans bloc manifest.
    /// Le premier slot déclaré fait foi (même règle que le graphe W04).
    pub fn warp_defs(&self, level_id: u32) -> Vec<WarpDef> {
        let mut out: Vec<WarpDef> = Vec::new();
        for e in self.graph.edges_of(level_id) {
            if out.iter().any(|d| d.to == format!("level:{}", e.to)) {
                continue;
            }
            if let Some(d) = WarpDef::from_table(self.tables, &self.lvl_warp, level_id, e.to, e.warp_type) {
                out.push(d);
            }
        }
        out
    }

    /// Nombre de PAIRES de warps non orientées uniques résolues depuis la
    /// table (métrique 8c : 94 sur `patch_levels.txt` v1.12).
    pub fn pair_count(&self) -> usize {
        self.graph
            .node_ids()
            .map(|from| self.graph.edges_of(from).iter().filter(|e| from < e.to).count())
            .sum()
    }
}

/// Taille de grille en cellules d2rules (0 si la table ne porte pas de taille).
fn grid_size(size_x: u32, size_y: u32) -> (f32, f32) {
    (size_x as f32 * CELLS_PER_TILE, size_y as f32 * CELLS_PER_TILE)
}

/// Point candidat sur le bord de grille correspondant à `Direction`
/// (`b` sud, `t` nord, `l` ouest, `r` est ; inconnu ⇒ sud), [`BORDER_INSET`]
/// cellules dedans.
fn border_point(w: f32, h: f32, dir: &str) -> (f32, f32) {
    inset_point(w, h, dir, BORDER_INSET)
}

/// Point candidat à `d` cellules du bord dans l'axe de `dir`.
fn inset_point(w: f32, h: f32, dir: &str, d: f32) -> (f32, f32) {
    match dir {
        "t" => (w / 2.0, d),
        "l" => (d, h / 2.0),
        "r" => (w - d, h / 2.0),
        // "b" (78 types sur 88) et inconnu : bord sud.
        _ => (w / 2.0, h - d),
    }
}

impl WarpDef {
    /// Adaptation TABLE (8c-1) du `WarpDef` manifest : lien `from`
    /// --`warp_type`--> `to_id`. `to` = clé `"level:{to_id}"` (résolue par
    /// `GameCore::resolve_level_def` quand les tables sont branchées — y
    /// compris pour les niveaux sans bloc manifest, dès que leurs assets
    /// existent). Géométrie : bord de grille `Direction` du type LvlWarp
    /// (coin nord-ouest en repli), arrivée [`ENTRY_INSET`] cellules dedans
    /// (hors zone). `None` si `from`/`to_id` sont absents de la table ou sans
    /// taille.
    pub fn from_table(
        tables: &LevelTables,
        lvl_warps: &[LvlWarpRecord],
        from: u32,
        to_id: u32,
        warp_type: i32,
    ) -> Option<WarpDef> {
        let rec = tables.get(from)?;
        let (w, h) = grid_size(rec.size_x, rec.size_y);
        if w <= 0.0 || h <= 0.0 || tables.get(to_id).is_none() {
            return None;
        }
        let dir = lvl_warps
            .iter()
            .find(|r| warp_type >= 0 && r.id == warp_type as u32)
            .map(|r| r.direction.clone())
            .unwrap_or_default();
        Some(WarpDef {
            to: format!("level:{to_id}"),
            at: border_point(w, h, &dir),
            r: WARP_RADIUS,
            entry: inset_point(w, h, &dir, ENTRY_INSET),
        })
    }
}

// ---------------------------------------------------------------------------
// Tests — tables réelles embarquées (8c-1 : ≥ 10 niveaux ; 8c-3 : contrat)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::levels::LevelTables;

    const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");
    const LVL_WARP: &str = include_str!("../../../../assets/LvlWarp.txt");

    /// Gabarit possédé (tables + graphe) — le résolveur n'emprunte que des
    /// vues, la durée de vie reste locale au test.
    struct Fixture {
        tables: LevelTables,
        graph: WarpGraph,
    }

    impl Fixture {
        fn new() -> Fixture {
            let tables = LevelTables::parse(PATCH_LEVELS).unwrap();
            let graph = WarpGraph::build(&tables);
            Fixture { tables, graph }
        }

        fn resolver(&self) -> WarpResolver<'_> {
            WarpResolver::new(&self.tables, &self.graph, LVL_WARP).unwrap()
        }
    }

    /// Comparaison champ-à-champ d'un WarpDef (le type manifest ne dérive
    /// pas `PartialEq` — le contrat reste inchangé côté game.rs).
    fn assert_warp_eq(d: &WarpDef, to: &str, at: (f32, f32), r: f32, entry: (f32, f32)) {
        assert_eq!(d.to, to);
        assert_eq!(d.at, at);
        assert_eq!(d.r, r);
        assert_eq!(d.entry, entry);
    }

    /// Ensemble trié des voisins d'un niveau (aide d'assertion).
    fn targets(r: &WarpResolver, id: u32) -> Vec<u32> {
        let mut v: Vec<u32> = r.neighbors(id).iter().map(|n| n.id).collect();
        v.sort_unstable();
        v
    }

    /// 8c-1 : voisins cohérents avec patch_levels pour 12 niveaux réels
    /// (actes I-II, maze/wilderness/preset), positions dans les bornes.
    #[test]
    fn neighbors_coherent_with_patch_levels_on_twelve_real_levels() {
        let f = Fixture::new();
        let r = f.resolver();

        // Acte I — liens tels que déclarés (cf. tests W04 de levels.rs) :
        assert_eq!(targets(&r, 8), vec![2], "Den of Evil → Blood Moor (Vis0=2)");
        assert_eq!(targets(&r, 13), vec![9], "Cave 2 Treasure → Cave 2");
        assert_eq!(targets(&r, 4), vec![10], "Black Marsh → Hole 1");
        assert_eq!(targets(&r, 5), vec![10], "Dark Wood → Hole 1");
        assert_eq!(targets(&r, 7), vec![12], "Tamoe Highland → Pit 1");
        for id in [2, 3, 6, 9] {
            assert!(!targets(&r, id).is_empty(), "{id} a des voisins Vis");
        }
        assert!(targets(&r, 2).contains(&8), "Blood Moor → Den of Evil (slots 3-6)");
        assert!(targets(&r, 9).contains(&3) && targets(&r, 9).contains(&13), "Cave 2 → Cold Plains + trésor");
        assert_eq!(targets(&r, 6), vec![11, 20], "Blood Moor west → Pit 1 + Cemetery");

        // Le town (1) : AUCUN slot Vis — sa transition reste manifest.
        assert!(targets(&r, 1).is_empty(), "town sans arête Vis/Warp (manifest porte la sienne)");

        // Acte II — Lut Gholein (40) : Sewer 1 A (47) ×2 slots + Sewer 2 (50).
        assert_eq!(targets(&r, 40), vec![47, 50], "Lut Gholein → égouts (Vis2/3=47, Vis4=50)");
        assert!(targets(&r, 47).contains(&40) && targets(&r, 47).contains(&48), "Sewer 1 A → Lut Gholein + Sewer 1 B");
        assert_eq!(targets(&r, 50), vec![40, 51], "Sewer 2 → Lut Gholein + Sewers 3");
        // Canyon of the Magi (46) : les 7 tombes (66-72), 7 slots déclarés.
        assert_eq!(targets(&r, 46), vec![66, 67, 68, 69, 70, 71, 72], "Canyon → 7 tombes");

        // Enrichissement : noms/actes/types LvlWarp résolus depuis les tables.
        // NB le graphe W04 garde la PREMIÈRE déclaration d'une paire (ordre
        // du fichier) : la paire {2,8} est déclarée d'abord par la Blood
        // Moor (slot 3, type 0 « Cave Cliff L »), avant Den of Evil (slot 0,
        // type 4) — l'arête 8→2 porte donc (slot 3, type 0).
        let den = r.neighbors(8);
        assert_eq!(den.len(), 1);
        assert_eq!(den[0].level_name, "Blood Moor");
        assert_eq!(den[0].act, 1);
        assert_eq!(den[0].slot, 3);
        assert_eq!((den[0].warp_type, den[0].warp_name.as_str()),
            (0, "Act 1 Wilderness to Cave Cliff L"));
        let lg = r.neighbors(40);
        let sewer = lg.iter().find(|n| n.id == 47).unwrap();
        assert_eq!(sewer.act, 2);
        assert!(sewer.warp_type >= 0 && !sewer.warp_name.is_empty(), "type LvlWarp résolu");
        assert_eq!(r.neighbors(999), Vec::new(), "niveau inconnu : pas de voisins");
    }

    /// 8c-1 : métrique de campagne — paires de warps résolues depuis la
    /// table (dédoublonnées non orientées).
    #[test]
    fn pair_count_golden_on_real_tables() {
        let f = Fixture::new();
        let r = f.resolver();
        assert_eq!(r.pair_count(), 94, "paires Vis/Warp uniques de patch_levels v1.12");
    }

    /// 8c-1 : WarpDef depuis la table — marche pour les niveaux SANS bloc
    /// manifest (Den of Evil), clé "level:{id}", géométrie LvlWarp/bord.
    #[test]
    fn warp_def_from_table_works_without_manifest_block() {
        let t = LevelTables::parse(PATCH_LEVELS).unwrap();
        let lw = parse_lvl_warp(LVL_WARP).unwrap();
        // Den of Evil (200×200 tuiles = 1000×1000 cellules) → Blood Moor,
        // type 4 « Act 1 Cave Up » dir "b" : bord sud, at 2 cellules dedans,
        // entry 15 cellules dedans (hors zone r=4).
        let d = WarpDef::from_table(&t, &lw, 8, 2, 4).expect("warp table Den of Evil");
        assert_warp_eq(&d, "level:2", (500.0, 998.0), WARP_RADIUS, (500.0, 985.0));
        assert!((d.entry.1 - d.at.1).abs() > d.r + 4.0, "arrivée hors de la zone warp");
        // Sens inverse : Blood Moor → Den of Evil (type 0 « Cliff L », dir "b").
        let d2 = WarpDef::from_table(&t, &lw, 2, 8, 0).expect("warp table Blood Moor");
        assert_warp_eq(&d2, "level:8", (200.0, 398.0), WARP_RADIUS, (200.0, 385.0));
        // Le resolver produit les warps complets d'un niveau sans manifest.
        // (L'arête 8→2 du graphe porte le type 0 de la première déclaration
        // — même direction "b" que le type 4 : géométrie identique.)
        let f = Fixture::new();
        let r = f.resolver();
        let defs = r.warp_defs(8);
        assert_eq!(defs.len(), 1);
        assert_warp_eq(&defs[0], d.to.as_str(), d.at, d.r, d.entry);
        assert!(!r.warp_defs(40).is_empty(), "Lut Gholein : warps de table");
        assert!(r.warp_defs(1).is_empty(), "town : rien côté table (manifest only)");
        // Garde-fous : records absents / sans taille.
        assert!(WarpDef::from_table(&t, &lw, 999, 2, 4).is_none());
        assert!(WarpDef::from_table(&t, &lw, 8, 9999, 4).is_none());
        assert!(WarpDef::from_table(&t, &lw, 0, 1, 0).is_none(), "Null sans taille");
    }

    /// 8c-3 : contrat entry_points — candidats dans les bornes, géométrie
    /// LvlWarp relative exacte, repli coin de grille pour Vis sans Warp.
    #[test]
    fn entry_points_contract_on_real_and_synthetic_levels() {
        let f = Fixture::new();
        let r = f.resolver();

        // Den of Evil : 1 candidat (→ 2), bord sud (dir "b"), géométrie
        // LvlWarp type 4 : Select (-30,-120) DX 120 DY 150 → centre (30,-45),
        // ExitWalk (3,5).
        let eps = r.entry_points(8);
        assert_eq!(eps.len(), 1);
        let e = &eps[0];
        assert_eq!((e.to, e.slot, e.source), (2, 0, EntrySource::LvlWarpBorder));
        assert_eq!(e.direction, "b");
        assert_eq!(e.pos, (500.0, 998.0), "bord sud de la grille 1000×1000");
        assert_eq!(e.select_center, Some((30.0, -45.0)));
        assert_eq!(e.exit_walk, Some((3, 5)));

        // Lut Gholein : 3 candidats — DEUX warps physiques distincts vers
        // Sewer 1 A (types 19 « Town to Sewer Trap » et 20 « Town to Sewer
        // Dock », slots 2-3) + Harem (type 24, slot 4). Dédoublon uniquement
        // sur (voisin, type, position) identiques.
        let eps40 = r.entry_points(40);
        assert_eq!(eps40.len(), 3, "{:?}", eps40);
        assert_eq!(eps40.iter().filter(|e| e.to == 47).count(), 2, "trap + dock");
        assert!(eps40.iter().any(|e| e.to == 50));
        let trap = eps40.iter().find(|e| e.warp_type == 19).unwrap();
        assert_eq!(trap.warp_name, "Act 2 Town to Sewer Trap");
        assert_eq!(trap.select_center, Some((-10.0, -10.0)), "SelectDX/DY = 0");
        // Bornes de grille pour TOUS les niveaux visés (8c-1 : cohérence).
        let t = LevelTables::parse(PATCH_LEVELS).unwrap();
        for id in [2, 3, 4, 5, 6, 7, 8, 9, 40, 46, 47, 50] {
            let rec = t.get(id).unwrap();
            let (w, h) = grid_size(rec.size_x, rec.size_y);
            for e in r.entry_points(id) {
                assert!(e.pos.0 >= 0.0 && e.pos.0 <= w && e.pos.1 >= 0.0 && e.pos.1 <= h,
                    "{id} : {:?} hors grille {w}×{h}", e.pos);
            }
        }

        // Repli coin de grille : Vis sans type LvlWarp exploitable.
        let tsv = "Name\tId\tSizeX\tSizeY\tVis0\tWarp0\nA\t1\t10\t8\t2\t-1\nB\t2\t0\t0\t0\t-1\n";
        let t2 = LevelTables::parse(tsv).unwrap();
        let g2 = WarpGraph::build(&t2);
        let r2 = WarpResolver::new(&t2, &g2, LVL_WARP).unwrap();
        let eps = r2.entry_points(1);
        assert_eq!(eps.len(), 1);
        assert_eq!((eps[0].to, eps[0].source), (2, EntrySource::GridCorner));
        assert_eq!(eps[0].pos, (GRID_CORNER_INSET, GRID_CORNER_INSET), "coin nord-ouest");
        assert_eq!(eps[0].select_center, None, "pas de géométrie LvlWarp");
        // Dédoublon : 4 slots identiques → un seul candidat.
        let tsv4 = "Name\tId\tSizeX\tSizeY\tVis0\tVis1\tVis2\tVis3\tWarp0\tWarp1\tWarp2\tWarp3\n\
                    A\t1\t10\t8\t2\t2\t2\t2\t4\t4\t4\t4\nB\t2\t0\t0\t0\t0\t0\t0\t-1\t-1\t-1\t-1\n";
        let t4 = LevelTables::parse(tsv4).unwrap();
        let g4 = WarpGraph::build(&t4);
        let r4 = WarpResolver::new(&t4, &g4, LVL_WARP).unwrap();
        assert_eq!(r4.entry_points(1).len(), 1, "slots identiques dédoublonnés");
    }
}
