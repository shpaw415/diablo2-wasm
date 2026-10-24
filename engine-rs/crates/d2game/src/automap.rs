//! AutoMap (étape 9d — monde vivant) : résolution `AutoMap.txt` v1.12 et
//! export JSON de la mini-carte.
//!
//! # Ce que fait le jeu réel
//! Chaque tuile posée d'un niveau porte, dans son en-tête DT1, un triplet
//! `(Type, Style, Sequence)`. `AutoMap.txt` (3 287 records) dit, PAR LvlType
//! (`LevelName` = « acte + nom LvlTypes », ex. `1 Town`), quelles frames du
//! sprite `MaxiMap.dc6` dessiner pour une tuile : `Style` + plage
//! `StartSequence..EndSequence` → `Cel1..Cel4` (le jeu tire aléatoirement
//! une des valeurs ≥ 0 — Go `d2records/automap_record.go:43-45`, lecture :
//! `d2records/automap_loader.go:10-31`).
//!
//! # Ce que fait ce module (port)
//! * [`AutoMapTable::parse`] — la table réelle `assets/AutoMap.txt`
//!   (embarquée `include_str!`, même pattern que `PATCH_LEVELS`), groupée
//!   par `(acte, nom LvlTypes court)` ; `1 Town` → 83 records,
//!   `1 Wilderness` → 142.
//! * [`dt1_meta_aligned`] — méta `(Type@+20, Style@+24, Sequence@+28)` des
//!   tuiles DT1 réelles (base `276 + 96k`), ALIGNÉE sur la table du port
//!   (`dt1::decode_dt1` lit `252 + 96k` : tuile 0 fantôme vide, tuile
//!   `k ≥ 1` = tuile réelle `k-1` — la table réelle commence à `276`).
//!   L'alignement est construit par les mêmes gardes que `decode_dt1`
//!   (version `(7,6)`, compte @268) : toute blob que `decode_dt1` accepte
//!   donne exactement `n` méta pour `n` tuiles.
//! * [`LevelAutomap::build`] — au chargement d'un niveau (appelé depuis
//!   `game.rs::load_level`, qui possède déjà DS1 + table de tuiles + grille
//!   de marche), classe chaque cellule visible (mêmes gardes que le
//!   composeur : `p1 != 0`, bit 0x80, index dans la table) : sol/mur/spécial
//!   (L2/L5), marchable (centre 5×5 de la grille de collision), puis résout
//!   le `Cel` AutoMap du record couvrant `(Style, Sequence)` — PREMIER
//!   match ordre fichier et première valeur `Cel1..4 ≥ 0` (déterministe ;
//!   le jeu réel tire au hasard, voir plus haut).
//! * `GameCore::automap_json` — sérialisation légère pour la démo
//!   (`demo/automap.js`) : par cellule visible `[x, y, flags, cel]`, plus
//!   les dimensions ; zéro calcul lourd côté JS.
//!
//! # Fallback documenté (missions 9d puis 13b)
//! Les `Cel` indexent les frames de `MaxiMap.dc6`, sprite ABSENT du port —
//! le JSON n'expose donc que les INDICES (`cel`), pas de pixels. Mode :
//! * `"automap"` — groupe AutoMap résolu pour le LvlType du niveau ET
//!   mapping PROPRE aux tuiles DT1 composées : chaque cellule porte son
//!   `cel` réel (≥ 0) ou `-1` si aucun record ne couvre sa tuile ;
//! * `"fallback"` — pas de tables levels branchées, pas de bloc manifest
//!   `levelsId`, LvlType sans groupe AutoMap, ou COUVERTURE INSUFFISANTE
//!   (cf. [`AUTOMAP_MIN_COVERAGE`]) : la démo dessine le POLYGONE
//!   MARCHABLE (diamants des cellules `walk`, ignorés : murs) — repli
//!   marchable assumé, documenté ici et dans REPORT.md.
//!
//! # Couverture mesurée par niveau (mission 13b — niveaux finaux A4-A5)
//! « Propre » = ≥ [`AUTOMAP_MIN_COVERAGE`] (60 %) des cellules visibles
//! portent un `cel` résolu (mesuré sur la carte COMPOSÉE, pas la banque).
//! Sonde sur les banques DT1 réelles du port (base 276, Style@+24,
//! Sequence@+28, jokers AutoMap inclus — cf. tests `a45_bank_coverage_*`) :
//!
//! | niveau (clé manifest) | LvlType → groupe AutoMap | couverture | mode |
//! |---|---|---|---|
//! | `pandemonium` (103) | 26 « Act 4 - Town » → `4 Town` | 4 % — le groupe v1.12 ne couvre que le style 12 ; les tuiles mesa/fort du port portent les styles 0-18 | `fallback` |
//! | `harrogath` (109) | 29 « Act 5 - Town » → `5 Town` | groupe ABSENT d'AutoMap.txt v1.12 (35 groupes, aucun « 5 Town ») | `fallback` |
//! | `bloodyfoothills` (110) | 30 « Act 5 - Siege » → `5 Siege` | 42 % — les sols ground/rockclifffloor (styles 0-1) n'ont pas de records | `fallback` |
//! | `crystalline` (113) | 33 « Act 5 - Ice Caves » → `5 Ice` (ALIAS, cf. [`group_alias`]) | 95 % — les Cel1-4 mappent proprement | `automap` |
//! | `hallsvaught` (124) | 32 « Act 5 - Temple » → `5 Temple` | 34 % — interior.dt1 porte les styles 0-24, la table n'en couvre que 7 | `fallback` |
//! (Références LECTURE SEULE : `d2w-re/tables/monde/AutoMap.txt` 3 287
//! records, DT1 `assets/map` — même chaîne d'extraction que le port.)
//!
//! # Format JSON livré (`automap()`)
//! ```json
//! {"level":"town","name":"Camp des Rousses","mode":"automap","act":1,
//!  "lvlType":"Town","coverage":0.73,"cellW":57,"cellH":41,"sub":5,
//!  "cells":[[0,0,1,0],[3,1,3,-1],…]}
//! ```
//! `cells` : `[x, y, flags, cel]` par cellule VISIBLE ; `flags` bit0=walk,
//! bit1=wall, bit2=special (cellule rare L2/L5 — pierres, warp) ; `cel` =
//! première valeur active `Cel1..4` du record AutoMap (frame `MaxiMap`),
//! `-1` sans match (→ diamant générique / polygone marchable côté démo).
//! `act` = acte résolu (préfixe du nom LvlTypes, repli colonne `Act`) ;
//! `coverage` = fraction des cellules visibles portant un `cel` ≥ 0
//! (arrondie au centième — le juge du mode, cf. [`AUTOMAP_MIN_COVERAGE`]).

use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::{json, Value};

use crate::compose::{ComposedMap, SUB};
use crate::ds1::{cell_fields, cell_tile_index, Ds1};
use crate::levels::{parse_lvl_types, LevelTables};

/// AutoMap.txt v1.12 (`d2data.mpq/data/global/excel/AutoMap.txt`, 3 287
/// records) — extrait une fois des MPQ (même chaîne d'extraction que les
/// autres tables du port, voir `d2w-re/tables/monde/AutoMap.txt`).
const AUTOMAP_TSV: &str = include_str!("../../../../assets/AutoMap.txt");

/// LvlTypes.txt v1.12 — pour nommer le tileset du niveau (colonne
/// `LevelType` de levels.txt → nom → groupe AutoMap `"{act} {court}"`).
const LVL_TYPES: &str = include_str!("../../../../assets/LvlTypes.txt");

// ---------------------------------------------------------------------------
// Table AutoMap.txt
// ---------------------------------------------------------------------------

/// Un record AutoMap.txt — Go `d2records/automap_record.go` (mêmes champs
/// utiles ; les colonnes `Type1..4` sont des commentaires, ignorées ici
/// comme dans le loader Go).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoMapRecord {
    /// Groupe « acte + LvlType » (ex. `1 Town`) — découpé en (acte, nom).
    pub level_name: String,
    /// Nom d'orientation de tuile (`fl`, `wr`… — documentation, le matching
    /// réel est `(Style, plage Sequence)`, cf. le commentaire Go).
    pub tile_name: String,
    pub style: i32,
    pub start_sequence: i32,
    pub end_sequence: i32,
    /// Frames `MaxiMap.dc6` — `-1` = pas de valeur (le jeu en tire une des
    /// actives ; ici on prend la PREMIère active, déterministe).
    pub cels: [i32; 4],
}

/// La table groupée par `(acte, nom court du LvlType)`.
#[derive(Debug, Clone, Default)]
pub struct AutoMapTable {
    groups: HashMap<(u32, String), Vec<AutoMapRecord>>,
    pub total: usize,
}

/// Entier lenient d'une cellule TSV : vide/illisible ⇒ `dflt` (les Cel
/// inactifs sont `-1` ou vides selon les lignes de la table réelle).
fn i32_or(s: &str, dflt: i32) -> i32 {
    s.trim().parse::<i32>().unwrap_or(dflt)
}

/// `1 Town` → `(1, "Town")` ; `3 Kurast 2` (nom avec espace) garde tout le
/// reste de la ligne comme nom. Malformé ⇒ `None`.
fn split_level_name(s: &str) -> Option<(u32, String)> {
    let (act, rest) = s.trim().split_once(' ')?;
    let act = act.parse::<u32>().ok()?;
    Some((act, rest.trim().to_string()))
}

impl AutoMapTable {
    /// Parse `AutoMap.txt` (en-tête réel : `… Type1 Cel1 Type2 Cel2 Type2
    /// Cel3 Type4 Cel4` — le doublon `Type2` est dans la table d'origine ;
    /// les `Cel1..4` restent uniques et adressables par nom).
    pub fn parse(tsv: &str) -> AutoMapTable {
        let mut out = AutoMapTable::default();
        crate::levels::each_row(tsv, |row| {
            let Some((act, short)) = split_level_name(row.s("LevelName")) else { return };
            let rec = AutoMapRecord {
                level_name: row.s("LevelName").to_string(),
                tile_name: row.s("TileName").to_string(),
                style: i32_or(row.s("Style"), -1),
                start_sequence: i32_or(row.s("StartSequence"), -1),
                end_sequence: i32_or(row.s("EndSequence"), -1),
                cels: [
                    i32_or(row.s("Cel1"), -1),
                    i32_or(row.s("Cel2"), -1),
                    i32_or(row.s("Cel3"), -1),
                    i32_or(row.s("Cel4"), -1),
                ],
            };
            out.groups.entry((act, short)).or_default().push(rec);
            out.total += 1;
        });
        out
    }

    /// Les records d'un LvlType (ordre fichier — le PREMIER match gagne).
    pub fn group(&self, act: u32, short: &str) -> Option<&[AutoMapRecord]> {
        self.groups.get(&(act, short.to_string())).map(|v| v.as_slice())
    }
}

/// La table v1.12 embarquée, parsée une fois (`OnceLock`, pattern `spawn.rs`).
pub fn table() -> &'static AutoMapTable {
    static TABLE: OnceLock<AutoMapTable> = OnceLock::new();
    TABLE.get_or_init(|| AutoMapTable::parse(AUTOMAP_TSV))
}

/// Le record couvrant `(style, seq)` : `Style` égal ET plage couvrante —
/// `Start < 0` = joker (seul `Style` compte, cf. commentaire Go
/// `automap_record.go:19-23`) ; `End < 0` avec `Start ≥ 0` = « seq ≥ start »
/// (rangées `wbr 0 5 -1` de la table réelle). Premier match ordre fichier.
pub fn match_record<'a>(
    records: &'a [AutoMapRecord],
    style: i32,
    seq: i32,
) -> Option<&'a AutoMapRecord> {
    records.iter().find(|r| {
        r.style == style
            && (r.start_sequence < 0
                || (seq >= r.start_sequence && (r.end_sequence < 0 || seq <= r.end_sequence)))
    })
}

/// Première frame active (`≥ 0`) d'un record — `-1` si aucune. Le jeu réel
/// choisit AU HASARD parmi les actives ; le port prend la première
/// (déterminisme exigé par les tests 9d, documenté dans REPORT.md).
pub fn first_active_cel(r: &AutoMapRecord) -> i32 {
    r.cels.iter().copied().find(|&c| c >= 0).unwrap_or(-1)
}

// ---------------------------------------------------------------------------
// Méta DT1 (Type / Style / Sequence), alignée sur la table du port
// ---------------------------------------------------------------------------

/// Méta d'AutoMap d'une tuile DT1 réelle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TileMeta {
    /// Champ `Type` (orientation canonique DT1 — redondant avec `Direction`
    /// @+0 dans cette release ; `0` = sol `fl`, `1..3` = clôtures `fe`…).
    pub orientation: i32,
    /// Champ `Style` — la clé `Style` d'AutoMap.txt.
    pub style: i32,
    /// Champ `Sequence` — la plage `StartSequence..EndSequence`.
    pub sequence: i32,
    /// `false` pour la tuile fantôme 0 de la table du port (voir
    /// [`dt1_meta_aligned`]) ; `true` pour une tuile réelle.
    pub valid: bool,
}

impl Default for TileMeta {
    fn default() -> Self {
        TileMeta { orientation: 0, style: -1, sequence: -1, valid: false }
    }
}

/// Méta des tuiles d'un conteneur DT1, ALIGNÉE sur [`crate::dt1::decode_dt1`].
///
/// Trappe documentée (mesurée sur `town_floor.dt1`, 144 tuiles) : la table
/// réelle des tuiles est à `276 + 96k` (le `bodyPosition` @272) avec
/// `Type@+20`, `Style@+24`, `Sequence@+28` ; `decode_dt1` lit les triplets
/// `(dataoff, datalen, nsub)` à `252 + 96k` — soit la tuile réelle `k-1`,
/// avec une tuile 0 fantôme vide. [`dt1_meta_aligned`] rend donc `n` entrées
/// : `[fantôme, réel 0, réel 1, … réel n-2]` — `meta[i]` décrit EXACTEMENT
/// la tuile que le renderer a peinte pour l'index `i` (décalage compris).
pub fn dt1_meta_aligned(d: &[u8]) -> Result<Vec<TileMeta>, crate::dt1::Dt1Error> {
    // Mêmes gardes que decode_dt1 : version (7,6) puis compte @268.
    let v1 = i32_at(d, 0)?;
    let v2 = i32_at(d, 4)?;
    if v1 != 7 || v2 != 6 {
        return Err(crate::dt1::Dt1Error::BadVersion(v1, v2));
    }
    let n = i32_at(d, 268)?.max(0) as usize;
    let mut out = Vec::with_capacity(n);
    out.push(TileMeta::default()); // tuile 0 fantôme (base 252 du port)
    for k in 0..n.saturating_sub(1) {
        let r = 276 + 96 * k; // base RÉELLE (bodyPosition @272)
        out.push(TileMeta {
            orientation: i32_at(d, r + 20)?,
            style: i32_at(d, r + 24)?,
            sequence: i32_at(d, r + 28)?,
            valid: true,
        });
    }
    Ok(out)
}

fn i32_at(d: &[u8], at: usize) -> Result<i32, crate::dt1::Dt1Error> {
    let b = d.get(at..at + 4).ok_or(crate::dt1::Dt1Error::TooShort)?;
    Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

// ---------------------------------------------------------------------------
// Cache par niveau + construction
// ---------------------------------------------------------------------------

/// Une cellule visible du niveau, prête pour la mini-carte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomapCell {
    pub x: u32,
    pub y: u32,
    /// Marchable (centre 5×5 de la grille de collision du composeur).
    pub walk: bool,
    /// Une tuile mur (non-floor) couvre la cellule (n'importe quelle couche,
    /// même priorité que la passe 2 du composeur).
    pub wall: bool,
    /// Cellule rare L2/L5 (pierres, warp).
    pub special: bool,
    /// Frame `MaxiMap` résolue (première `Cel1..4` active du record), `-1`
    /// sans match / en fallback.
    pub cel: i32,
}

/// Le cache AutoMap du niveau courant — construit au `load_level`, sérialisé
/// par `GameCore::automap_json`.
#[derive(Debug, Clone)]
pub struct LevelAutomap {
    pub level_key: String,
    pub act: u32,
    /// Nom court du LvlTypes (`"Town"`, `"Ice"`…) — `""` si non résolu
    /// (après alias [`group_alias`], même espace que les groupes AutoMap).
    pub lvl_type: String,
    /// `"automap"` (groupe résolu ET couverture ≥ [`AUTOMAP_MIN_COVERAGE`])
    /// ou `"fallback"` (voir en-tête de module).
    pub mode: &'static str,
    /// Fraction des cellules visibles portant un `cel` ≥ 0 (le juge du
    /// mode, mission 13b — exposée dans le JSON `automap_json`).
    pub coverage: f32,
    pub cell_w: u32,
    pub cell_h: u32,
    pub cells: Vec<AutomapCell>,
}

/// Préfixe LvlTypes `Act N - Town` → `Town` (format réel des noms de la
/// table embarquée). Autre format ⇒ nom tel quel.

/// `Act 1 - Town` → `(1, "Town")` — l'acte du groupe AutoMap vient du
/// PRÉFIXE du nom LvlTypes : la colonne `Act` de levels.txt vaut 0 pour les
/// niveaux génériques d'acte (mesuré : ids 1/2 de patch_levels.txt), alors
/// que le nom porte l'acte réel. Autre format ⇒ `(None, nom tel quel)`.
fn split_act_prefix(name: &str) -> (Option<u32>, String) {
    if let Some((prefix, rest)) = name.split_once(" - ") {
        if let Some(n) = prefix.strip_prefix("Act ").and_then(|n| n.trim().parse::<u32>().ok()) {
            return (Some(n), rest.trim().to_string());
        }
    }
    (None, name.trim().to_string())
}

/// Seuil « mapping propre » (mission 13b) : fraction minimale de cellules
/// visibles portant un `cel` résolu pour activer le mode `"automap"`. Sous
/// le seuil — OU sans groupe AutoMap — le niveau reste en `"fallback"`
/// (polygone marchable). 0,60 sépare proprement les données réelles v1.12 :
/// crystalline 95 % / town 73 % / bloodmoor ~76 % d'un côté, hallsvaught
/// 34 % / bloodyfoothills 42 % / pandemonium 4 % de l'autre (sondes dans
/// les tests `a45_*` ci-dessous et en-tête de module).
pub const AUTOMAP_MIN_COVERAGE: f32 = 0.60;

/// Alias noms LvlTypes → groupes AutoMap.txt v1.12 (mission 13b) : la
/// table AutoMap regroupe les glaces sous « 5 Ice » quand LvlTypes.txt
/// nomme le type 33 « Act 5 - Ice Caves » — seule divergence de nommage
/// des 32 LvlTypes (l'autre écart, « 5 Town » ABSENT pour Harrogath, est
/// une vraie absence de records v1.12, pas un alias). Raccourci ⇒ nom de
/// groupe réel ; défaut = nom tel quel.
fn group_alias(short: &str) -> &str {
    match short {
        "Ice Caves" => "Ice",
        other => other,
    }
}

/// `(acte, nom court)` du LvlType du niveau depuis les tables levels.txt —
/// `None` sans tables / sans record / sans LvlType correspondant.
fn resolve_group(tables: Option<&LevelTables>, levels_id: Option<u32>) -> Option<(u32, String)> {
    let rec = tables?.get(levels_id?)?;
    let types = parse_lvl_types(LVL_TYPES).ok()?;
    let lt = types.iter().find(|t| t.id == rec.level_type)?;
    let (act_from_name, short) = split_act_prefix(&lt.name);
    Some((act_from_name.unwrap_or(rec.act), group_alias(&short).to_string()))
}

/// Index de tuile résolu d'une cellule DS1 — MÊMES gardes ET même repli que
/// le composeur (`draw_layer` / `for_cell_tiles`) : `p1 != 0`, non caché
/// (bit 0x80), index dans la table, repli `p1` si la tuile résolue est vide.
fn visible_tile_index(ds1: &Ds1, tilesets: &[crate::dt1::Dt1Tile], li: usize, cx: usize, cy: usize) -> Option<usize> {
    let v = *ds1.layers.get(li)?.get(cy * ds1.w + cx)?;
    let (p1, p2, _p3, orient) = cell_fields(v);
    if p1 == 0 || orient & 0x80 != 0 {
        return None;
    }
    let index = cell_tile_index(p1, p2);
    if index >= tilesets.len() || p1 as usize >= tilesets.len() {
        return None;
    }
    // Repli du composeur : tuile vide → tuile famille p1 (même index pour
    // la méta, donc style/seq de la tuile réellement peinte).
    Some(if tilesets[index].blocks.is_empty() { p1 as usize } else { index })
}

impl LevelAutomap {
    /// Construit le cache AutoMap d'un niveau — appelé par
    /// `game.rs::load_level` juste après la composition (la grille de marche
    /// `map` et la table de tuiles `tilesets` y sont encore en main).
    ///
    /// `meta` : concaténé par conteneur dans le MÊME ordre que `tilesets`
    /// (les `build_*_tiles` de `game.rs` le construisent à la volée).
    pub fn build(
        level_key: &str,
        tables: Option<&LevelTables>,
        levels_id: Option<u32>,
        ds1: &Ds1,
        tilesets: &[crate::dt1::Dt1Tile],
        meta: &[TileMeta],
        map: &ComposedMap,
    ) -> LevelAutomap {
        // Identité (acte, LvlType) résolue INDÉPENDAMMENT du groupe : un
        // LvlType sans groupe AutoMap (« 5 Town » pour Harrogath, mission
        // 13b) garde son acte et son nom dans le JSON — seul le mode passe
        // en « fallback ».
        let (act, lvl_type, group) = match resolve_group(tables, levels_id) {
            Some((act, short)) => {
                let g = table().group(act, &short);
                (act, short, g)
            }
            None => (0, String::new(), None),
        };

        let mut cells = Vec::new();
        for cy in 0..ds1.h {
            for cx in 0..ds1.w {
                // sol prioritaire (L4 puis rues puis rares), mur sur toute couche
                let mut floor = None;
                for li in [4usize, 0, 2, 5] {
                    if floor.is_none() {
                        if let Some(idx) = visible_tile_index(ds1, tilesets, li, cx, cy) {
                            if tilesets[idx].is_floor {
                                floor = Some((idx, li == 2 || li == 5));
                            }
                        }
                    }
                }
                let mut wall = None;
                for li in [4usize, 0, 1, 2, 5] {
                    if wall.is_none() {
                        if let Some(idx) = visible_tile_index(ds1, tilesets, li, cx, cy) {
                            if !tilesets[idx].is_floor {
                                wall = Some(idx);
                            }
                        }
                    }
                }
                // Mur prioritaire pour la méta (il masque la cellule) ;
                // ni sol ni mur visible ⇒ cellule invisible, ignorée.
                let (kind_idx, dominant) = match (wall, floor) {
                    (Some(w), _) => (w, true),
                    (None, Some((f, _))) => (f, false),
                    (None, None) => continue,
                };
                let special = floor.map_or(false, |(_, s)| s);
                let (style, sequence) = meta
                    .get(kind_idx)
                    .filter(|m| m.valid)
                    .map(|m| (m.style, m.sequence))
                    .unwrap_or((-1, -1));
                let cel = group
                    .and_then(|g| match_record(g, style, sequence))
                    .map(first_active_cel)
                    .unwrap_or(-1);
                let (sx, sy) = (cx * SUB + 2, cy * SUB + 2);
                cells.push(AutomapCell {
                    x: cx as u32,
                    y: cy as u32,
                    walk: map.walkable(sx, sy),
                    wall: dominant,
                    special,
                    cel,
                });
            }
        }
        // Mode 13b : le groupe seul ne suffit plus — la couverture MESURÉE
        // sur la carte composée décide (mission : « si les Cel1-4 mappent
        // proprement aux tuiles DT1 composées, rendu précis ; sinon
        // fallback »). Les `cel` restent portés dans les deux modes.
        let coverage = if cells.is_empty() {
            0.0
        } else {
            cells.iter().filter(|c| c.cel >= 0).count() as f32 / cells.len() as f32
        };
        let mode = if group.is_some() && coverage >= AUTOMAP_MIN_COVERAGE {
            "automap"
        } else {
            "fallback"
        };
        LevelAutomap {
            level_key: level_key.to_string(),
            act,
            lvl_type,
            mode,
            coverage,
            cell_w: ds1.w as u32,
            cell_h: ds1.h as u32,
            cells,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint GameCore
// ---------------------------------------------------------------------------

impl crate::game::GameCore {
    /// La mini-carte du niveau courant en JSON (voir en-tête de module pour
    /// le format et le fallback). `"{}"` avant `finalize`.
    pub fn automap_json(&self) -> String {
        let Some(a) = self.automap.as_ref() else { return "{}".into() };
        let name = self
            .level
            .as_ref()
            .map(|l| self.level_display_name(l))
            .unwrap_or_default();
        let cells: Vec<Value> = a
            .cells
            .iter()
            .map(|c| {
                let flags = (c.walk as i64) | ((c.wall as i64) << 1) | ((c.special as i64) << 2);
                json!([c.x, c.y, flags, c.cel])
            })
            .collect();
        json!({
            "level": a.level_key,
            "name": name,
            "mode": a.mode,
            "act": a.act,
            "lvlType": a.lvl_type,
            "coverage": (a.coverage * 100.0).round() / 100.0,
            "cellW": a.cell_w,
            "cellH": a.cell_h,
            "sub": SUB as u32,
            "cells": cells,
        })
        .to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests — parseur + matching + méta DT1 (les tests GameCore sont dans
// game.rs : town + bloodmoor + fallback, avec les assets réels)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TSV: &str = include_str!("../../../../assets/AutoMap.txt");

    #[test]
    fn parses_full_real_table() {
        let t = table();
        // 3 287 lignes de données brutes ; la ligne expander « Expansion »
        // (niveau 2605, sans acte) est ignorée comme dans levels.rs.
        assert_eq!(t.total, 3286, "3 287 records v1.12 - 1 expander");
        let town = t.group(1, "Town").expect("groupe 1 Town");
        assert_eq!(town.len(), 83);
        let wild = t.group(1, "Wilderness").expect("groupe 1 Wilderness");
        assert_eq!(wild.len(), 142);
        assert!(t.group(1, "Inexistant").is_none());
    }

    #[test]
    fn matches_floor_path_and_river_rows() {
        // lignes réelles « 1 Town fl » :
        //   style 0, seq 1..46 → Floor Path (Cel 0..3)
        //   style 2, seq 4..7  → River T A (Cel 4)
        let town = table().group(1, "Town").unwrap();
        let path = match_record(town, 0, 5).expect("sol style 0 seq 5");
        assert_eq!(path.tile_name, "fl");
        assert_eq!(first_active_cel(path), 0);
        assert_eq!(path.cels, [0, 1, 2, 3]);
        let river = match_record(town, 2, 6).expect("rivière style 2 seq 6");
        assert_eq!(first_active_cel(river), 4, "River T A → cel 4");
        // hors de toute plage : aucun match (la cellule tombera en diamant
        // générique — documenté).
        assert!(match_record(town, 9, 999).is_none());
    }

    #[test]
    fn joker_semantics_start_and_end_negative() {
        // Sémantique des jokers (Go automap_record.go:19-23) : Start < 0 →
        // seul Style compte ; End < 0 avec Start ≥ 0 → « seq >= Start »
        // (rangées réelles `wbr 0 5 -1`).
        let mk = |style: i32, start: i32, end: i32| AutoMapRecord {
            level_name: "1 Town".into(),
            tile_name: "x".into(),
            style,
            start_sequence: start,
            end_sequence: end,
            cels: [7, -1, -1, -1],
        };
        // Ordre « fichier » : la plage pleine (start 5, end -1 = +infini)
        // AVANT le joker (start<0 = seul Style compte) — même sémantique que
        // le groupe réel `1 Town` (`wbr 0 5 -1` avant `wtr 0 -1 -1`).
        let recs = [mk(0, 5, -1), mk(0, -1, -1)];
        assert_eq!(match_record(&recs, 0, 3).unwrap().start_sequence, -1,
            "joker start<0 : seul Style compte (couvre les seq hors plage)");
        assert_eq!(match_record(&recs, 0, 1000).unwrap().start_sequence, 5,
            "end<0 = pas de borne de fin : seq>=start suffit");
        assert!(match_record(&recs, 1, 3).is_none(), "style différent : aucun match");
    }

    #[test]
    fn first_file_match_wins_in_town() {
        // Ordre fichier du groupe réel « 1 Town » : `fl 0 1 46` (plage
        // pleine) AVANT `wr 0 0 18` AVANT `wbr 0 5 -1` AVANT les jokers
        // `wtr/wbl 0 -1 -1`. PREMIER match dans l'ordre du fichier gagne.
        let town = table().group(1, "Town").unwrap();
        assert_eq!(match_record(town, 0, 5).unwrap().tile_name, "fl", "plage pleine d'abord");
        assert_eq!(match_record(town, 0, 6).unwrap().tile_name, "fl",
            "seq 6 : la même plage fl (1..46) couvre — premier match, pas de re-tir tardif");
        assert_eq!(match_record(town, 0, 0).unwrap().tile_name, "wr", "wr couvre seq 0 (fl commence à 1)");
        assert_eq!(match_record(town, 0, 1000).unwrap().tile_name, "wbr",
            "wbr 0 5 -1 : end<0 = seq>=start (infini), avant le joker wtr");
        assert_eq!(match_record(town, 2, 6).unwrap().tile_name, "fl", "River T A est une ligne fl");
        assert!(match_record(town, 9, 999).is_none(), "style 0..8 seulement en acte 1 ville");
    }

    #[test]
    fn empty_cel_cells_are_inactive() {
        // Lignes rivière : Cel2..4 vides ou « -1 » ⇒ inactifs.
        let town = table().group(1, "Town").unwrap();
        let river = match_record(town, 2, 6).unwrap();
        assert_eq!(river.cels, [4, -1, -1, -1]);
    }

    #[test]
    fn dt1_meta_aligned_matches_port_table() {
        // DT1 synthétique : version (7,6), 3 tuiles, base réelle 276.
        let mut d = vec![0u8; 276 + 96 * 3];
        d[0..4].copy_from_slice(&7i32.to_le_bytes());
        d[4..8].copy_from_slice(&6i32.to_le_bytes());
        d[268..272].copy_from_slice(&3i32.to_le_bytes());
        for (k, (typ, sty, seq)) in
            [(0usize, (1i32, 10i32, 11i32)), (1, (2, 20, 21)), (2, (3, 30, 31))]
        {
            let r = 276 + 96 * k;
            d[r + 20..r + 24].copy_from_slice(&typ.to_le_bytes());
            d[r + 24..r + 28].copy_from_slice(&sty.to_le_bytes());
            d[r + 28..r + 32].copy_from_slice(&seq.to_le_bytes());
        }
        let m = dt1_meta_aligned(&d).unwrap();
        assert_eq!(m.len(), 3, "n méta pour n tuiles (alignement port)");
        assert!(!m[0].valid, "tuile 0 fantôme");
        assert_eq!(m[1], TileMeta { orientation: 1, style: 10, sequence: 11, valid: true });
        assert_eq!(m[2], TileMeta { orientation: 2, style: 20, sequence: 21, valid: true });
        // Gardes identiques à decode_dt1 : mauvaise version rejetée.
        d[0..4].copy_from_slice(&8i32.to_le_bytes());
        assert_eq!(dt1_meta_aligned(&d), Err(crate::dt1::Dt1Error::BadVersion(8, 6)));
    }

    #[test]
    fn real_dt1_headers_match_automap_style_space() {
        // Sur le DT1 réel du sol de ville : Style ∈ {0, 5}, Sequence 0..55 —
        // le groupe « 1 Town » couvre bien ces styles (preuve d'ancrage
        // table↔DT1, cf. REPORT.md §9d).
        let bytes = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../assets/map/town_floor.dt1"
        ))
        .unwrap();
        let metas = dt1_meta_aligned(&bytes).unwrap();
        let town = table().group(1, "Town").unwrap();
        let matched = metas
            .iter()
            .filter(|m| m.valid)
            .filter(|m| match_record(town, m.style, m.sequence).is_some())
            .count();
        assert!(matched > 100, "sol de ville : {matched}/143 tuiles couvertes par AutoMap");
    }

    #[test]
    fn act_prefix_splits_act_and_short_name() {
        assert_eq!(split_act_prefix("Act 1 - Town"), (Some(1), "Town".into()));
        assert_eq!(split_act_prefix("Act 1 - Wilderness"), (Some(1), "Wilderness".into()));
        assert_eq!(split_act_prefix("Act 3 - Jungle"), (Some(3), "Jungle".into()));
        assert_eq!(split_act_prefix("Town"), (None, "Town".into()));
    }

    // --- 13b : alias LvlTypes → groupes AutoMap + sonde des niveaux finaux --

    #[test]
    fn group_alias_covers_the_only_naming_divergence() {
        // Sonde v1.12 (cf. REPORT.md §13b) : sur les 32 LvlTypes, un SEUL
        // écart de nommage (« Ice Caves » vs groupe « 5 Ice ») ; « 5 Town »
        // est lui une vraie ABSENCE de records (Harrogath → fallback).
        assert_eq!(group_alias("Ice Caves"), "Ice");
        assert_eq!(group_alias("Town"), "Town");
        assert_eq!(group_alias("Siege"), "Siege");
        let t = table();
        assert!(t.group(5, "Ice").is_some(), "groupe « 5 Ice » : 64 records");
        assert!(t.group(5, "Ice Caves").is_none(), "pas de groupe « Ice Caves » brut");
        assert!(t.group(5, "Town").is_none(), "AutoMap.txt v1.12 n'a PAS de « 5 Town »");
        assert!(t.group(4, "Town").is_some(), "« 4 Town » existe (mais ne couvre que le style 12)");
    }

    /// Sonde banque réelle : couverture AutoMap des tuiles DT1 d'un niveau
    /// final A4-A5 (mêmes gardes que [`dt1_meta_aligned`], mêmes jokers que
    /// [`match_record`]). `None` = pas de groupe AutoMap pour ce LvlType
    /// (Harrogath — « 5 Town » absent de la table v1.12). Retourne
    /// (tuiles, matchées).
    fn a45_bank_coverage(level_id: u32) -> Option<(usize, usize)> {
        let bank = crate::drlg_a45::bank_for_level(level_id)?;
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3).unwrap().to_path_buf();
        let (act, short) = split_act_prefix(bank.name);
        let group = table().group(act?, group_alias(&short))?;
        let (mut tot, mut matched) = (0usize, 0usize);
        for f in bank.files {
            let bytes = std::fs::read(root.join("assets/map").join(f.local))
                .unwrap_or_else(|e| panic!("{}: {e}", f.local));
            for m in dt1_meta_aligned(&bytes).unwrap().iter().filter(|m| m.valid) {
                tot += 1;
                matched += match_record(group, m.style, m.sequence).is_some() as usize;
            }
        }
        Some((tot, matched))
    }

    #[test]
    fn a45_bank_coverages_match_documented_modes() {
        // Mission 13b : les modes par niveau dérivent des couvertures réelles
        // (sondes python identiques sur les mêmes DT1 — cf. en-tête module).
        // Seuil de DÉCISION côté cellules = 0,60 ([`AUTOMAP_MIN_COVERAGE`]) ;
        // ici on valide la SOURCE (banques) : crystalline seul au-dessus.
        let cov = |id: u32| {
            let (tot, m) = a45_bank_coverage(id).expect("groupe AutoMap présent");
            assert!(tot > 0, "banque {id} non vide");
            m as f32 / tot as f32
        };
        let pandemonium = cov(crate::drlg_a45::PANDEMONIUM);
        assert!(pandemonium < 0.10, "« 4 Town » ne couvre que le style 12 : {pandemonium:.2}");
        assert!(a45_bank_coverage(crate::drlg_a45::HARROGATH).is_none(),
            "pas de groupe « 5 Town » : Harrogath fallback garanti");
        let foothills = cov(crate::drlg_a45::BLOODY_FOOTHS);
        assert!(foothills < 0.60, "sols styles 0-1 sans records : {foothills:.2}");
        let crystalline = cov(crate::drlg_a45::CRYSTALLINE);
        assert!(crystalline >= 0.90, "glaces propres : {crystalline:.2} ≥ 0,90");
        let vaught = cov(crate::drlg_a45::HALLS_VAUGHT);
        assert!(vaught < 0.60, "interior styles 0-24 mal couverts : {vaught:.2}");
    }
}
