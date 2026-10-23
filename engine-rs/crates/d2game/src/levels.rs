//! Lecteur runtime des tables de niveaux v1.12 — W01 + W04 du plan
//! `docs/parity/finish-monde.md`.
//!
//! **W01** : mini « RecordManager » — parseur TSV générique (tab-separated,
//! en-tête nommé, lignes expanders ignorées) + les 5 tables de niveaux
//! extraites dans `assets/` : `patch_levels.txt` (levels.txt final v1.12,
//! 140 colonnes), `LvlTypes.txt` (tilesets), `LvlPrest.txt` (presets DRLG),
//! `LvlMaze.txt` (grilles maze, extrait ce cycle), `LvlWarp.txt` (géométrie
//! des warps, extrait ce cycle). Référence Go : `d2records/*_loader.go`
//! (même lecture « header → index de colonnes », sans les 140 structs).
//!
//! **W04** : graphe de transitions `Vis0-7`/`Warp0-7` — chaque slot Vis_i
//! nomme l'id du niveau voisin et Warp_i le type LvlWarp du lien. Le graphe
//! est NON ORIENTÉ (réciproque enregistrée si la table ne la déclare pas),
//! dédoublonné par paire (from, to). Limite assumée (constatée, pas
//! inventée) : l'adjacence entre niveaux outdoor (DrlgType 3) N'EST PAS dans
//! levels.txt (les slots Vis des wilderness portent leurs caves/tours, pas
//! les wilderness voisins) — la chaîne town→Blood Moor→… reste portée par le
//! manifest (voir `docs/parity/impl-drlg.md` §W04).
//!
//! Zéro I/O : les textes arrivent en `&str` (assets fournis via
//! `GameCore::provide`, tests via `include_str!`).

use std::collections::{BTreeMap, BTreeSet, HashMap};

// ---------------------------------------------------------------------------
// Parseur TSV générique
// ---------------------------------------------------------------------------

/// Index de colonnes d'une table TSV (première ligne = en-tête).
pub(crate) struct Columns(HashMap<String, usize>);

impl Columns {
    fn parse(header: &str) -> Columns {
        Columns(
            header
                .split('\t')
                .enumerate()
                .map(|(i, name)| (name.trim().to_string(), i))
                .collect(),
        )
    }

    fn get(&self, col: &str) -> Option<usize> {
        self.0.get(col).copied()
    }
}

/// Une ligne de données + son index de colonnes.
pub(crate) struct Row<'a> {
    cells: Vec<&'a str>,
    cols: &'a Columns,
}

impl<'a> Row<'a> {
    /// Cellule brute ("" si colonne absente / ligne courte).
    pub(crate) fn s(&self, col: &str) -> &'a str {
        self.cols
            .get(col)
            .and_then(|i| self.cells.get(i))
            .copied()
            .unwrap_or("")
            .trim()
    }

    /// Entier lenient : "" / absent / illisible ⇒ 0.
    pub(crate) fn i(&self, col: &str) -> i32 {
        self.s(col).parse::<i32>().unwrap_or(0)
    }

    pub(crate) fn u(&self, col: &str) -> u32 {
        self.i(col).max(0) as u32
    }

    /// Id de record : None pour les lignes expanders / vides (Id non entier).
    pub(crate) fn id(&self) -> Option<u32> {
        self.s("Id").parse::<u32>().ok()
    }

    pub(crate) fn is_blank(&self) -> bool {
        self.cells.iter().all(|c| c.trim().is_empty())
    }
}

/// Itère les lignes de données d'un TSV (split LF, tolère CRLF) via callback —
/// la première ligne (en-tête) construit l'index de colonnes partagé.
pub(crate) fn each_row<F: FnMut(Row<'_>)>(tsv: &str, mut f: F) {
    let mut lines = tsv.lines();
    let cols = Columns::parse(lines.next().unwrap_or(""));
    for line in lines {
        let cells: Vec<&str> = line.split('\t').collect();
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        f(Row { cells, cols: &cols });
    }
}

// ---------------------------------------------------------------------------
// W01 — levels.txt (patch_levels.txt)
// ---------------------------------------------------------------------------

/// Un record de levels.txt — les colonnes consommées par le moteur
/// (identité, DRLG, graphe Vis/Warp, monstres, waypoint, quête).
#[derive(Debug, Clone, Default)]
pub struct LevelRecord {
    pub id: u32,
    /// Nom technique (`Act 1 - Wilderness 1`).
    pub name: String,
    /// Nom affichable EN (`Blood Moor`) — le fr-FR vit dans les TBL.
    pub level_name: String,
    pub act: u32,
    pub size_x: u32,
    pub size_y: u32,
    /// 0 aucun · 1 maze · 2 preset · 3 wilderness (généré).
    pub drlg_type: u8,
    /// Id dans LvlTypes.txt (tileset).
    pub level_type: u32,
    /// Voisins par slot (Vis0-7) : id de niveau, 0 = pas de voisin.
    pub vis: [i32; 8],
    /// Type LvlWarp par slot (Warp0-7) : -1 = pas de warp.
    pub warp: [i32; 8],
    /// Densité de monstres (MonDen, 0 = aucun).
    pub mon_den: u32,
    /// Bornes d'uniques du niveau (`MonUMin`/`MonUMax`, vide ⇒ 0) — nombre
    /// de packs à leader unique doré (W08, impl-monstres-boss).
    pub mon_u_min: u32,
    pub mon_u_max: u32,
    /// Planche `mon1..mon10` (codes monstats bruts, ordre de la table).
    pub mon: Vec<String>,
    /// 255 = pas de waypoint ; sinon l'id du waypoint (1..38).
    pub waypoint: i32,
    /// Quête pointée (`Quest`, -1/0 = aucune).
    pub quest: i32,
    /// Libellé du warp sortant (`LevelWarp`, ex. `To The Blood Moor`).
    pub level_warp: String,
    /// Fichier d'entrée (`EntryFile`, ex. `A1L2`).
    pub entry_file: String,
    /// Portal flag (`Portal` = 1 pour les villes).
    pub portal: i32,
}

/// La table levels.txt complète, indexée par id.
#[derive(Debug, Clone, Default)]
pub struct LevelTables {
    records: Vec<LevelRecord>,
    index: BTreeMap<u32, usize>,
}

impl LevelTables {
    /// Parse `patch_levels.txt` (levels.txt final v1.12). Les lignes sans Id
    /// entier (expanders `Expansion`) sont ignorées ; le record `Null` (id 0)
    /// est conservé tel quel (jamais adressé par le graphe).
    pub fn parse(tsv: &str) -> Result<LevelTables, String> {
        let mut t = LevelTables::default();
        each_row(tsv, |row| {
            if row.is_blank() {
                return;
            }
            let Some(id) = row.id() else {
                return; // expander / ligne décorative
            };
            if id > 0 && row.s("Name").is_empty() {
                return;
            }
            let mut vis = [0i32; 8];
            let mut warp = [0i32; 8];
            for slot in 0..8 {
                vis[slot] = row.i(&format!("Vis{slot}"));
                warp[slot] = row.i(&format!("Warp{slot}"));
            }
            let mon = (1..=10)
                .filter_map(|n| {
                    let c = row.s(&format!("mon{n}"));
                    (!c.is_empty()).then(|| c.to_string())
                })
                .collect();
            let rec = LevelRecord {
                id,
                name: row.s("Name").to_string(),
                level_name: row.s("LevelName").to_string(),
                act: row.u("Act"),
                size_x: row.u("SizeX"),
                size_y: row.u("SizeY"),
                drlg_type: row.u("DrlgType") as u8,
                level_type: row.u("LevelType"),
                vis,
                warp,
                mon_den: row.u("MonDen"),
                mon_u_min: row.u("MonUMin"),
                mon_u_max: row.u("MonUMax"),
                mon,
                waypoint: row.i("Waypoint"),
                quest: row.i("Quest"),
                level_warp: row.s("LevelWarp").to_string(),
                entry_file: row.s("EntryFile").to_string(),
                portal: row.i("Portal"),
            };
            t.index.insert(id, t.records.len());
            t.records.push(rec);
        });
        if t.records.len() < 2 {
            return Err(format!(
                "levels.txt : {} records seulement (en-tête attendu + lignes Id)",
                t.records.len()
            ));
        }
        Ok(t)
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn get(&self, id: u32) -> Option<&LevelRecord> {
        self.index.get(&id).map(|&i| &self.records[i])
    }

    pub fn iter(&self) -> impl Iterator<Item = &LevelRecord> {
        self.records.iter()
    }

    /// Les niveaux d'un acte (colonne Act), triés par id.
    pub fn act_levels(&self, act: u32) -> Vec<&LevelRecord> {
        let mut v: Vec<&LevelRecord> = self.records.iter().filter(|r| r.act == act).collect();
        v.sort_by_key(|r| r.id);
        v
    }
}

// ---------------------------------------------------------------------------
// W01 — LvlTypes.txt (tilesets par type de niveau)
// ---------------------------------------------------------------------------

/// Un tileset de LvlTypes.txt : les DT1 du type (`File 1..32`, "0" = vide).
#[derive(Debug, Clone, Default)]
pub struct LvlTypeRecord {
    pub id: u32,
    pub name: String,
    /// Chemins DT1 réels (colonnes `File N` non vides et != "0", ordre table).
    pub files: Vec<String>,
}

/// Parse `LvlTypes.txt`.
pub fn parse_lvl_types(tsv: &str) -> Result<Vec<LvlTypeRecord>, String> {
    let mut out = Vec::new();
    each_row(tsv, |row| {
        let Some(id) = row.id() else { return };
        if row.s("Name").is_empty() && id != 0 {
            return;
        }
        let mut files = Vec::new();
        for n in 1..=32 {
            let f = row.s(&format!("File {n}"));
            if !f.is_empty() && f != "0" {
                files.push(f.to_string());
            }
        }
        out.push(LvlTypeRecord { id, name: row.s("Name").to_string(), files });
    });
    if out.is_empty() {
        return Err("LvlTypes.txt : aucun record".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// W01 — LvlPrest.txt (presets DRLG)
// ---------------------------------------------------------------------------

/// Un preset de LvlPrest.txt (DrlgType 2) : taille + DS1 référencés.
#[derive(Debug, Clone, Default)]
pub struct LvlPrestRecord {
    pub name: String,
    pub def: u32,
    /// Niveau cible (0 = générique) — la Blood Moor (2) n'y apparaît PAS :
    /// le DRLG outdoor retail n'assemble pas les wilderness depuis LvlPrest.
    pub level_id: u32,
    pub size_x: u32,
    pub size_y: u32,
    pub outdoors: u32,
    /// `File1..File6` non vides (chemins DS1).
    pub files: Vec<String>,
    /// Masque DT1 (`Dt1Mask`, bitmask des 32 files du LvlType).
    pub dt1_mask: u32,
}

/// Parse `LvlPrest.txt`.
pub fn parse_lvl_prest(tsv: &str) -> Result<Vec<LvlPrestRecord>, String> {
    let mut out = Vec::new();
    each_row(tsv, |row| {
        let def = row.i("Def");
        if row.s("Name").is_empty() && def == 0 {
            return;
        }
        let files = (1..=6)
            .filter_map(|n| {
                let f = row.s(&format!("File{n}"));
                (!f.is_empty() && f != "0").then(|| f.to_string())
            })
            .collect();
        out.push(LvlPrestRecord {
            name: row.s("Name").to_string(),
            def: def.max(0) as u32,
            level_id: row.u("LevelId"),
            size_x: row.u("SizeX"),
            size_y: row.u("SizeY"),
            outdoors: row.u("Outdoors"),
            files,
            dt1_mask: row.u("Dt1Mask"),
        });
    });
    if out.is_empty() {
        return Err("LvlPrest.txt : aucun record".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// W01 — LvlMaze.txt (grilles maze, DrlgType 1) — extrait ce cycle
// ---------------------------------------------------------------------------

/// Un record de LvlMaze.txt : grille de rooms d'un niveau maze.
#[derive(Debug, Clone, Default)]
pub struct LvlMazeRecord {
    pub name: String,
    /// Id du niveau concerné (colonne `Level`).
    pub level: u32,
    /// Rooms en Normal (`Rooms`) — (N)/(H) disponibles dans la table.
    pub rooms: u32,
    pub size_x: u32,
    pub size_y: u32,
    /// Seuil de fusion (`Merge`, 500 = fusion des rooms adjacentes).
    pub merge: u32,
}

/// Parse `LvlMaze.txt`.
pub fn parse_lvl_maze(tsv: &str) -> Result<Vec<LvlMazeRecord>, String> {
    let mut out = Vec::new();
    each_row(tsv, |row| {
        if row.is_blank() {
            return;
        }
        let name = row.s("Name").to_string();
        if name.is_empty() {
            return;
        }
        out.push(LvlMazeRecord {
            name,
            level: row.u("Level"),
            rooms: row.u("Rooms"),
            size_x: row.u("SizeX"),
            size_y: row.u("SizeY"),
            merge: row.u("Merge"),
        });
    });
    if out.is_empty() {
        return Err("LvlMaze.txt : aucun record".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// W01 — LvlWarp.txt (géométrie des warps) — extrait ce cycle
// ---------------------------------------------------------------------------

/// Un type de warp de LvlWarp.txt : rectangle de sélection + walk d'entrée.
#[derive(Debug, Clone, Default)]
pub struct LvlWarpRecord {
    pub id: u32,
    pub name: String,
    pub select_x: i32,
    pub select_y: i32,
    pub select_dx: i32,
    pub select_dy: i32,
    pub exit_walk_x: i32,
    pub exit_walk_y: i32,
    /// Direction d'entrée (`b` = bas, `l`/`r`/`t`…).
    pub direction: String,
}

/// Parse `LvlWarp.txt`.
pub fn parse_lvl_warp(tsv: &str) -> Result<Vec<LvlWarpRecord>, String> {
    let mut out = Vec::new();
    each_row(tsv, |row| {
        if row.is_blank() {
            return;
        }
        let name = row.s("Name").to_string();
        let Some(id) = row.id() else { return };
        if name.is_empty() && id == 0 {
            return;
        }
        out.push(LvlWarpRecord {
            id,
            name,
            select_x: row.i("SelectX"),
            select_y: row.i("SelectY"),
            select_dx: row.i("SelectDX"),
            select_dy: row.i("SelectDY"),
            exit_walk_x: row.i("ExitWalkX"),
            exit_walk_y: row.i("ExitWalkY"),
            direction: row.s("Direction").to_string(),
        });
    });
    if out.is_empty() {
        return Err("LvlWarp.txt : aucun record".into());
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// W04 — graphe Vis/Warp
// ---------------------------------------------------------------------------

/// Une arête du graphe de transitions : `from` --slot/warp_type--> `to`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct WarpEdge {
    /// Niveau voisin.
    pub to: u32,
    /// Slot d'origine (0-7) de la déclaration Vis/Warp.
    pub slot: u8,
    /// Type LvlWarp du lien (-1 si la table n'en déclare pas).
    pub warp_type: i32,
}

/// Graphe NON ORIENTÉ des transitions Vis/Warp de levels.txt. Une arête par
/// paire (from, to) : la première occurrence de slot fait foi (les wilderness
/// déclarent la même cave sur leurs 4 slots de bord — types LvlWarp 0-3) ;
/// la réciproque est enregistrée si la table ne la déclare pas déjà.
#[derive(Debug, Clone, Default)]
pub struct WarpGraph {
    nodes: BTreeMap<u32, Vec<WarpEdge>>,
}

impl WarpGraph {
    /// Construit le graphe depuis levels.txt (W04).
    pub fn build(tables: &LevelTables) -> WarpGraph {
        let mut g = WarpGraph::default();
        for rec in tables.iter() {
            for slot in 0..8usize {
                let vis = rec.vis[slot];
                if vis <= 0 {
                    continue;
                }
                let wt = rec.warp[slot];
                g.link(rec.id, vis as u32, slot as u8, wt);
                g.link(vis as u32, rec.id, slot as u8, wt); // no-op si déjà déclaré
            }
        }
        g
    }

    /// Enregistre from→to (et seulement s'il n'existe pas déjà d'arête
    /// from→to, quelle que soit sa variante de warp).
    fn link(&mut self, from: u32, to: u32, slot: u8, warp_type: i32) {
        if from == to {
            return;
        }
        let edges = self.nodes.entry(from).or_default();
        if edges.iter().any(|e| e.to == to) {
            return;
        }
        edges.push(WarpEdge { to, slot, warp_type });
        edges.sort_by_key(|e| (e.slot, e.to));
    }

    pub fn edges_of(&self, id: u32) -> &[WarpEdge] {
        self.nodes.get(&id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Degré (nombre de voisins uniques).
    pub fn degree(&self, id: u32) -> usize {
        self.edges_of(id).len()
    }

    /// Nombre total d'arêtes orientées enregistrées.
    pub fn edge_count(&self) -> usize {
        self.nodes.values().map(|v| v.len()).sum()
    }

    pub fn node_ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.nodes.keys().copied()
    }

    /// Tous les niveaux atteignables depuis `from` (BFS, graphe non orienté).
    pub fn reachable(&self, from: u32) -> BTreeSet<u32> {
        let mut seen: BTreeSet<u32> = BTreeSet::new();
        let mut queue = vec![from];
        while let Some(id) = queue.pop() {
            for e in self.edges_of(id) {
                if seen.insert(e.to) {
                    queue.push(e.to);
                }
            }
        }
        seen
    }
}

// ---------------------------------------------------------------------------
// Tests — les vraies tables extraites (W01) et le graphe Acte I (W04)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");
    const LVL_TYPES: &str = include_str!("../../../../assets/LvlTypes.txt");
    const LVL_PREST: &str = include_str!("../../../../assets/LvlPrest.txt");
    const LVL_MAZE: &str = include_str!("../../../../assets/LvlMaze.txt");
    const LVL_WARP: &str = include_str!("../../../../assets/LvlWarp.txt");

    #[test]
    fn parses_real_patch_levels() {
        let t = LevelTables::parse(PATCH_LEVELS).unwrap();
        // 137 records avec Id (Null + 136 niveaux) ; la ligne expander
        // `Expansion` (sans Id) est ignorée — compté ce jour sur la table.
        assert_eq!(t.len(), 137, "{} records parsés", t.len());
        // Town
        let town = t.get(1).unwrap();
        assert_eq!(town.name, "Act 1 - Town");
        assert_eq!(town.level_name, "Rogue Encampment");
        assert_eq!((town.size_x, town.size_y), (56, 40));
        assert_eq!(town.drlg_type, 2);
        assert_eq!(town.portal, 1);
        // Blood Moor
        let bm = t.get(2).unwrap();
        assert_eq!(bm.level_name, "Blood Moor");
        assert_eq!(bm.drlg_type, 3, "wilderness généré");
        assert_eq!(bm.level_type, 2);
        assert_eq!((bm.size_x, bm.size_y), (80, 80));
        assert_eq!(bm.mon_den, 520);
        assert_eq!(bm.mon, vec!["zombie1", "fallen1", "quillrat1"]);
        assert_eq!(bm.waypoint, 255, "Blood Moor : pas de waypoint propre");
        assert_eq!(bm.entry_file, "A1L2");
        // Den of Evil (maze)
        let den = t.get(8).unwrap();
        assert_eq!(den.drlg_type, 1);
        assert_eq!(den.vis[0], 2, "rattaché à la Blood Moor");
        assert_eq!(den.warp[0], 4, "type LvlWarp « Cave Up »");
        // Waypoints flaggés
        assert_eq!(t.get(3).unwrap().waypoint, 1);
        assert_eq!(t.get(9).unwrap().quest, 0);
        // Le record expander est bien ignoré
        assert!(t.get(0).unwrap().name == "Null");
    }

    #[test]
    fn act_levels_of_act0_are_sorted_by_id() {
        let t = LevelTables::parse(PATCH_LEVELS).unwrap();
        let act1 = t.act_levels(0);
        assert!(act1.len() >= 39, "acte I : {} niveaux", act1.len());
        assert_eq!(act1[0].id, 0, "Null (act 0) d'abord");
        assert_eq!(act1[1].id, 1);
        // tri strictement croissant
        for w in act1.windows(2) {
            assert!(w[0].id < w[1].id);
        }
    }

    #[test]
    fn parse_rejects_garbage() {
        assert!(LevelTables::parse("").is_err());
        assert!(LevelTables::parse("Name\tId\nfoo\t").is_err(), "pas d'Id entier");
        assert!(LevelTables::parse("Name\tId\nfoo\t1").is_err(), "1 record = pas une table");
    }

    // --- W01 : les 4 tables annexes ----------------------------------------

    #[test]
    fn parses_lvl_types() {
        let types = parse_lvl_types(LVL_TYPES).unwrap();
        // 37 lignes − expander `Expansion` = 36 types réels.
        assert!(types.len() >= 36, "{} types", types.len());
        let t2 = types.iter().find(|t| t.id == 2).expect("type 2 (Blood Moor)");
        assert!(t2.files.len() >= 20, "type 2 : {} DT1", t2.files.len());
        assert!(t2.files.iter().any(|f| f.to_ascii_lowercase().contains("floor")));
        assert!(t2.files.iter().any(|f| f.to_ascii_lowercase().contains("treegroups")));
    }

    #[test]
    fn parses_lvl_prest_and_proves_bloodmoor_absent() {
        let prest = parse_lvl_prest(LVL_PREST).unwrap();
        assert!(prest.len() >= 1090, "{} presets", prest.len());
        // Constat structurant (finish-monde §2/W02) : AUCUN preset LevelId=2 —
        // la Blood Moor n'est PAS assemblée depuis LvlPrest par le retail.
        assert!(
            !prest.iter().any(|p| p.level_id == 2),
            "LvlPrest ne doit pas référencer la Blood Moor"
        );
        // 1 090 presets avec un Def ; 2 d'entre eux sont des marqueurs
        // SANS AUCUN DS1 (File1..6 tous "0"), 6 autres n'ont qu'un File
        // secondaire — 1 088 portent au moins un fichier réel.
        let filled = prest.iter().filter(|p| p.def > 0).count();
        assert!(filled > 1000);
        let with_files = prest.iter().filter(|p| p.def > 0 && !p.files.is_empty()).count();
        assert_eq!(with_files, filled - 2, "2 marqueurs sans aucun DS1 attendus");
    }

    #[test]
    fn parses_lvl_maze() {
        let maze = parse_lvl_maze(LVL_MAZE).unwrap();
        assert!(maze.len() >= 80, "{} records", maze.len());
        let den = maze.iter().find(|m| m.level == 8).expect("Cave 1");
        assert_eq!(den.rooms, 1);
        assert_eq!((den.size_x, den.size_y), (24, 24));
        assert_eq!(den.merge, 500);
        let cave2 = maze.iter().find(|m| m.level == 9).expect("Cave 2");
        assert_eq!(cave2.rooms, 4);
    }

    #[test]
    fn parses_lvl_warp() {
        let warps = parse_lvl_warp(LVL_WARP).unwrap();
        // 89 lignes − expander = 88 warps réels.
        assert!(warps.len() >= 88, "{} warps", warps.len());
        let up = warps.iter().find(|w| w.id == 4).expect("Cave Up");
        assert!(up.name.contains("Cave Up"));
        assert_eq!(up.direction, "b");
        // Les types de warp référencés par les levels existent tous.
        let ids: BTreeSet<u32> = warps.iter().map(|w| w.id).collect();
        let t = LevelTables::parse(PATCH_LEVELS).unwrap();
        for rec in t.iter() {
            for wt in rec.warp {
                if wt > 0 {
                    assert!(ids.contains(&(wt as u32)), "warp type {wt} absent de LvlWarp");
                }
            }
        }
    }

    // --- W04 : graphe Vis/Warp ----------------------------------------------

    /// Les 9 niveaux « chaîne » de l'acte I (ids 1-9) + leurs liens réels.
    #[test]
    fn warp_graph_coherent_on_act1_nine_levels() {
        let t = LevelTables::parse(PATCH_LEVELS).unwrap();
        let g = WarpGraph::build(&t);

        // 1. Toute cible Vis existe dans la table et porte une arête valide.
        for rec in t.iter() {
            for e in g.edges_of(rec.id) {
                assert!(t.get(e.to).is_some(), "{} → {} inconnu", rec.id, e.to);
                assert_ne!(e.to, rec.id, "auto-boucle interdite");
                assert!(e.slot < 8);
            }
        }

        // 2. Les liens maze↔wilderness de l'acte I, tels que la table les
        //    déclare (et symétrisés par le graphe) :
        assert!(g.edges_of(8).iter().any(|e| e.to == 2), "Den of Evil → Blood Moor");
        assert!(g.edges_of(2).iter().any(|e| e.to == 8), "Blood Moor → Den of Evil (slots 3-6)");
        assert!(g.edges_of(9).iter().any(|e| e.to == 3), "Cave 2 → Cold Plains");
        assert!(g.edges_of(3).iter().any(|e| e.to == 9), "Cold Plains → Cave 2");
        assert!(g.edges_of(9).iter().any(|e| e.to == 13), "Cave 2 → Cave 2 Treasure");
        assert!(g.edges_of(13).iter().any(|e| e.to == 9), "Cave 2 Treasure → Cave 2");

        // 3. Le town (1) n'a AUCUN lien Vis dans la table : sa seule
        //    transition (vers Blood Moor) est portée par le manifest (LvlWarp
        //    géométrie absente des colonnes Vis/Warp pour ce preset).
        assert_eq!(g.degree(1), 0, "town sans arête Vis/Warp dans levels.txt");

        // 4. Graphe simple : pas de doublon (from,to).
        for id in g.node_ids() {
            let mut tos: Vec<u32> = g.edges_of(id).iter().map(|e| e.to).collect();
            tos.sort();
            tos.dedup();
            assert_eq!(tos.len(), g.edges_of(id).len(), "doublon d'arête depuis {id}");
        }

        // 5. Composantes connexes réelles de la table (données pures) :
        //    chaque cave rattache SON wilderness, sans lien entre wilderness
        //    (l'adjacence outdoor vit dans le DRLG outdoor retail, pas dans
        //    levels.txt — constaté, pas un bug du parseur, cf. impl-drlg.md).
        let reach_bm = g.reachable(2);
        assert!(reach_bm.contains(&8), "Blood Moor ↔ Den of Evil connexes");
        assert!(!reach_bm.contains(&3), "pas de lien Vis Blood Moor → Cold Plains");
        let reach_cp = g.reachable(3);
        for id in [3, 9, 13] {
            assert!(reach_cp.contains(&id), "{id} connexe à Cold Plains");
        }
        assert!(!reach_cp.contains(&2), "composantes cave distinctes");
        // Les 4 wilderness restants (4-7) ne pointent que vers leurs
        // caves/tours (ids 10-12, 20) — jamais entre wilderness : l'adjacence
        // outdoor vit dans le DRLG outdoor retail, pas dans levels.txt
        // (constaté, pas un bug du parseur — cf. impl-drlg.md).
        let targets = |id: u32| -> Vec<u32> { g.edges_of(id).iter().map(|e| e.to).collect() };
        assert_eq!(targets(4), vec![10]);
        assert_eq!(targets(5), vec![10]);
        assert_eq!(targets(6), vec![20, 11]);
        assert_eq!(targets(7), vec![12]);
    }

    #[test]
    fn warp_graph_registers_missing_reciprocal() {
        // Table synthétique à sens unique : A déclare B, B ne déclare rien.
        let tsv = "Name\tId\tVis0\tWarp0\nA\t1\t2\t4\nB\t2\t0\t-1\n";
        let t = LevelTables::parse(tsv).unwrap();
        let g = WarpGraph::build(&t);
        assert!(g.edges_of(1).iter().any(|e| e.to == 2 && e.warp_type == 4));
        assert!(g.edges_of(2).iter().any(|e| e.to == 1), "réciproque enregistrée");
        assert_eq!(g.edge_count(), 2);
    }
}
