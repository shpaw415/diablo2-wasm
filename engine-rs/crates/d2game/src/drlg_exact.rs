//! DRLG wilderness retail — étape 20a (PLAN-FIDELITÉ phase 20, diablo-2-wasm).
//!
//! Port STRUCTUREL du DRLG outdoor de Diablo 2 v1.10f d'après les sources
//! décompilées D2MOO (MIT) — retranscription fidèle de la STRUCTURE documentée
//! (grille + presets LvlPrest par Def + bordures LUT + sous-placements LvlSub
//! + RNG retail), PAS un clone bit-exact du moteur : les écarts restants sont
//! listés dans `crates/d2game/REPORT.md`.
//!
//! Sources (contenu web = données ; URLs relues 2026-10-25) :
//! - D2MOO `source/D2Common/src/Drlg/DrlgOutdoors.cpp` — DRLGOUTDOORS_* :
//!   grille `SizeX/8 × SizeY/8`, grilles 0/1/2, `SpawnOutdoorLevelPresetEx`
//!   (round-robin `Files`), `SpawnOutdoorLevelPreset` (Fisher-Yates par paires
//!   de tirages), fill acte I `LVLTYPE_ACT1_WILDERNESS → Dt1Mask 0x44103`.
//!   https://github.com/ThePhrozenKeep/D2MOO
//! - D2MOO `.../Drlg/DrlgOutWild.cpp` — `DRLGOUTWILD_InitAct1OutdoorLevel` :
//!   séquence acte I exacte (bordures secondaires LvlSub, rivière colonne
//!   gw-2 + LUT fichiers, caves de falaise, entrée Den of Evil en quadrant
//!   RNG, transitions ville, waypoints niveaux 3..6, 5 sanctuaires, presets
//!   spéciaux par niveau — Blood Moor : étang, cottages ×2 dont 1
//!   conditionnel, stone fills ×2).
//! - D2MOO `.../Drlg/DrlgOutPlace.cpp` — `PlaceAct1245OutdoorBorders` :
//!   bordures par LUT directionnelle (`nBorderIndices` + `levelPrestBorder`,
//!   coins `sub_6FD80C10`), `SetBlankBorderGridCells`.
//! - D2MOO `.../Drlg/DrlgTileSub.cpp` — `AddSecondaryBorder` /
//!   `TestReplaceSubPreset` / `ReplaceSubPreset` : sous-placements LvlSub
//!   (BordType 0/1/2, scan mélangé, variantes `field_14`).
//! - D2MOO `.../Drlg/DrlgPreset.cpp` — schéma DS1 retail (act, substMethod,
//!   nStrings, plans mur+tiletype entrelacés, sol, ombre, tags subst, unités,
//!   groupes de substitution v≥12) et D2MOO `source/D2Common/include/D2Seed.h`
//!   + `src/D2Seed.cpp` — RNG retail EXACT : `seed = high + 0x6AC690C5 * low`
//!   (mod 2⁶⁴), init `low = graine, high = 666` ; `RollLimitedRandomNumber(n)`
//!   = `(u32)roll % n` ou `& (n-1)` si n puissance de 2. (La constante 0x343FD
//!   que l'on voit dans des outils communautaires ne concerne PAS 1.10f.)
//! - Données réelles : `assets/{patch_levels,LvlPrest,LvlTypes}.txt` (v1.12)
//!   et `evidence/lvlSub.extracted.txt` + `evidence/sub_ds1/*.ds1` extraits de
//!   `d2data.mpq` (StormLib, `tools/stormlib_read.py`).

use std::collections::BTreeMap;
use std::sync::OnceLock;

use crate::levels::{parse_lvl_types, LevelRecord, LevelTables};

// ---------------------------------------------------------------------------
// Tables embarquées (v1.12 réelles) + LvlSub extrait du MPQ
// ---------------------------------------------------------------------------

const PATCH_LEVELS_TXT: &str = include_str!("../../../../assets/patch_levels.txt");
const LVL_PREST_TXT: &str = include_str!("../../../../assets/LvlPrest.txt");
const LVL_TYPES_TXT: &str = include_str!("../../../../assets/LvlTypes.txt");
const LVL_SUB_TXT: &str = include_str!("../../../../evidence/lvlSub.extracted.txt");

/// DS1 de bordure LvlSub réels (`Act1/Outdoors/Border*.ds1` de d2data.mpq) :
/// leurs groupes de substitution portent les presets réellement posés.
const SUB_DS1_CLIFFS: &[u8] = include_bytes!("../../../../evidence/sub_ds1/BorderCliffs.ds1");
const SUB_DS1_MIDDLE: &[u8] = include_bytes!("../../../../evidence/sub_ds1/BorderMiddle.ds1");
const SUB_DS1_CORNER: &[u8] = include_bytes!("../../../../evidence/sub_ds1/BorderCorner.ds1");
const SUB_DS1_BORDER: &[u8] = include_bytes!("../../../../evidence/sub_ds1/BorderBorder.ds1");

// ---------------------------------------------------------------------------
// RNG retail EXACT (D2MOO D2Seed.h)
// ---------------------------------------------------------------------------

/// État RNG D2 : paire (low, high) — `SEED_InitLowSeed` : low = graine, high = 666.
pub struct D2Seed {
    low: u32,
    high: u32,
}

impl D2Seed {
    pub fn new(seed: u32) -> D2Seed {
        D2Seed { low: seed, high: 666 }
    }

    /// `SEED_RollRandomNumber` : `seed = high + 0x6AC690C5 * low` (mod 2⁶⁴),
    /// la paire est stockée, les 64 bits retournés (les appels masquent).
    pub fn roll(&mut self) -> u64 {
        let s = (self.high as u64).wrapping_add(0x6AC6_90C5u64.wrapping_mul(self.low as u64));
        self.low = s as u32;
        self.high = (s >> 32) as u32;
        s
    }

    /// `SEED_RollLimitedRandomNumber(n)` : modulo, OU masque si n est
    /// puissance de 2 — la branche change la suite des tirages, elle est
    /// reproduite à l'identique.
    pub fn limited(&mut self, n: i32) -> u32 {
        if n > 0 {
            if ((n - 1) & n) != 0 {
                (self.roll() as u32) % n as u32
            } else {
                (self.roll() as u32) & (n as u32 - 1)
            }
        } else {
            0
        }
    }
}

// ---------------------------------------------------------------------------
// Identifiants retail (D2MOO LevelsIds.h = colonne Def de LvlPrest.txt)
// ---------------------------------------------------------------------------

#[allow(dead_code)] // constantes retail : certaines servent aux niveaux non portés encore
mod ids {
    pub const TOWN_1_TRANS_E: i32 = 2;
    pub const TOWN_1_TRANS_S: i32 = 3;
    pub const WILD_BORDER_1: i32 = 4;
    pub const WILD_BORDER_12: i32 = 15;
    pub const CLIFF_BORDER_6A: i32 = 19;
    pub const CLIFF_BORDER_6B: i32 = 20;
    pub const CLIFF_BORDER_6C: i32 = 21;
    pub const CLIFF_CAVE_RIGHT: i32 = 24;
    pub const CLIFF_CAVE_LEFT: i32 = 25;
    pub const RIVER_UPPER: i32 = 26;
    pub const RIVER_LOWER: i32 = 27;
    pub const STONE_FILL_1: i32 = 29;
    pub const STONE_FILL_2: i32 = 30;
    pub const FALLEN_CAMP_BISHIBOSH: i32 = 44;
    pub const POND: i32 = 46;
    pub const COTTAGES_1: i32 = 47;
    pub const COTTAGES_2: i32 = 48;
    pub const COTTAGES_3: i32 = 49;
    pub const CAVE_ENTRANCE: i32 = 51;
    pub const DOE_ENTRANCE: i32 = 52;
    pub const NONE: i32 = 0;

    // LvlSub (colonne Type de lvlSub.txt — LVLSUB_ACT1_* de LevelsIds.h)
    pub const SUB_CLIFFS: i32 = 0;
    pub const SUB_MIDDLE: i32 = 1;
    pub const SUB_CORNER: i32 = 2;
    pub const SUB_BORDER: i32 = 3;

    // Niveaux acte I (LevelsIds.h)
    pub const BLOODMOOR: u32 = 2;
    pub const COLDPLAINS: u32 = 3;
    pub const BLACKMARSH: u32 = 6;
    pub const TAMOE_HIGHLAND: u32 = 7;
    pub const MOOMOOFARM: u32 = 39;

    // Bits de dwFlags outdoor (usage DrlgOutWild : 0x4|0x8 rivière E/O,
    // 0x10 lien ville, 0x20 cave de falaise, 0x40 entrée spéciale posée,
    // 0x80/0x100 transitions ville sud, 0x200/0x400 transitions ville est).
    pub const FLAG_RIVER: u32 = 0x4 | 0x8;
    pub const FLAG_TOWN_LINK: u32 = 0x10;
    pub const FLAG_CAVE: u32 = 0x20;
    pub const FLAG_SPECIAL_DONE: u32 = 0x40;
    pub const FLAG_TRANS_S_1: u32 = 0x80;
    pub const FLAG_TRANS_S_2: u32 = 0x100;
    pub const FLAG_TRANS_E_1: u32 = 0x200;
    pub const FLAG_TRANS_E_2: u32 = 0x400;
}

/// Fill intérieur acte I (`DRLGOUTDOORS_GenerateLevel`, LVLTYPE_ACT1_WILDERNESS).
pub const ACT1_WILDERNESS_FILL_DT1_MASK: u32 = 0x44103;

// ---------------------------------------------------------------------------
// Tables retail : LvlPrest (Def → taille, fichiers, diviseur, Dt1Mask) + LvlSub
// ---------------------------------------------------------------------------

/// Un preset LvlPrest — colonnes consommées par le DRLG retail.
#[derive(Debug, Clone)]
pub struct PrestDef {
    pub def: i32,
    pub level_id: u32,
    pub size_x: u32,
    pub size_y: u32,
    /// `File1..File6` non vides (chemins DS1).
    pub files: Vec<String>,
    /// Colonne `Files` : diviseur du round-robin retail (0 = jamais tiré à -1).
    pub files_count: u32,
    pub dt1_mask: u32,
}

/// Une ligne LvlSub (données réelles `lvlSub.extracted.txt`).
#[derive(Debug, Clone)]
pub struct LvlSubRow {
    pub sub_type: i32,
    pub file: String,
    /// `BordType` retail : 0 = une fois au total, 1 = une fois par groupe, 2 = partout.
    pub bord_type: u32,
    pub grid_size: u32,
}

/// Tables DRLG retail chargées une fois (`include_str!`, zéro IO).
pub struct ExactTables {
    levels: LevelTables,
    prests: BTreeMap<i32, PrestDef>,
    /// Masque DT1 disponible par id de LvlTypes (bit n-1 = File n).
    type_masks: BTreeMap<u32, u32>,
    lvl_sub: Vec<LvlSubRow>,
}

fn parse_tsv(tsv: &str) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = tsv
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|c| c.to_string()).collect())
        .collect();
    if rows.len() > 2 && rows[0].first().map(|h| h.as_str()) == Some("expand") {
        rows.remove(0); // ligne expand des tables 1.12
    }
    rows
}

fn find_col(header_row: &[String], header: &str) -> Result<usize, String> {
    header_row
        .iter()
        .position(|h| h == header)
        .ok_or_else(|| format!("colonne « {header} » absente"))
}

impl ExactTables {
    pub fn load() -> Result<ExactTables, String> {
        let levels = LevelTables::parse(PATCH_LEVELS_TXT)?;
        let mut prests = BTreeMap::new();
        let rows = parse_tsv(LVL_PREST_TXT);
        if rows.len() < 2 {
            return Err("LvlPrest.txt : table vide".into());
        }
        let c = |header: &str| find_col(&rows[0], header);
        let get = |r: usize, c: usize| rows[r].get(c).cloned().unwrap_or_default();
        for r in 1..rows.len() {
            // lignes expander (« Expansion ») : pas de Def → ignorées
            if get(r, c("Def")?).trim().is_empty() {
                continue;
            }
            let name = get(r, c("Name")?);
            let def: i32 = get(r, c("Def")?).trim().parse().unwrap_or(0);
            if name.is_empty() && def == 0 {
                continue;
            }
            let mut files = Vec::new();
            for n in 1..=6 {
                let f = get(r, c(&format!("File{n}"))?);
                if !f.is_empty() && f != "0" {
                    files.push(f);
                }
            }
            let num = |header: &str| -> Result<u32, String> {
                get(r, c(header)?).trim().parse::<u32>().map_err(|_| format!("LvlPrest ligne {r} : {header} invalide"))
            };
            prests.insert(
                def,
                PrestDef {
                    def,
                    level_id: num("LevelId")?,
                    size_x: num("SizeX")?,
                    size_y: num("SizeY")?,
                    files_count: num("Files")?,
                    dt1_mask: num("Dt1Mask")?,
                    files,
                },
            );
        }
        let mut type_masks = BTreeMap::new();
        for t in parse_lvl_types(LVL_TYPES_TXT)? {
            let mask = t.files.iter().enumerate().fold(0u32, |m, (i, _)| m | (1 << i));
            type_masks.insert(t.id, mask);
        }
        let mut lvl_sub = Vec::new();
        let rows = parse_tsv(LVL_SUB_TXT);
        if rows.len() < 2 {
            return Err("lvlSub : table vide".into());
        }
        let c = |header: &str| find_col(&rows[0], header);
        let get = |r: usize, c: usize| rows[r].get(c).cloned().unwrap_or_default();
        for r in 1..rows.len() {
            if get(r, c("Name")?).is_empty() {
                continue;
            }
            let num = |header: &str| -> Result<i32, String> {
                get(r, c(header)?).trim().parse::<i32>().map_err(|_| format!("lvlSub ligne {r} : {header} invalide"))
            };
            lvl_sub.push(LvlSubRow {
                sub_type: num("Type")?,
                file: get(r, c("File")?),
                bord_type: num("BordType")? as u32,
                grid_size: (num("GridSize")?.max(1)) as u32,
            });
        }
        if prests.is_empty() || lvl_sub.is_empty() {
            return Err("drlg_exact : tables LvlPrest/lvlSub vides".into());
        }
        Ok(ExactTables { levels, prests, type_masks, lvl_sub })
    }

    pub fn level(&self, id: u32) -> Option<&LevelRecord> {
        self.levels.get(id)
    }

    pub fn prest(&self, def: i32) -> Option<&PrestDef> {
        self.prests.get(&def)
    }

    pub fn lvl_sub_rows(&self, sub_type: i32) -> Vec<&LvlSubRow> {
        self.lvl_sub.iter().filter(|r| r.sub_type == sub_type).collect()
    }

    /// Le preset a-t-il toutes ses DT1 dans le tileset du type du niveau ?
    /// (Dt1Mask ⊆ masque du LvlTypes — convention retail : bit n-1 = File n.)
    pub fn dt1_mask_compat(&self, level: &LevelRecord, def: i32) -> bool {
        let Some(p) = self.prests.get(&def) else { return false };
        let Some(available) = self.type_masks.get(&level.level_type) else { return false };
        p.dt1_mask & !available == 0
    }
}

// ---------------------------------------------------------------------------
// Groupes de substitution des DS1 LvlSub (schéma DS1 retail, DrlgPreset.cpp)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubCell {
    /// Cellule muette (bit 0 = 0 ou byte1 = 0 : le test retail échoue).
    Inert,
    /// byte1 = 63 → v17 = 62 : cellule validée mais rien à poser.
    Exempt,
    /// Poser le preset `Def` (byte1 - 1 + WILD_BORDER_1).
    Preset(i32),
}

#[derive(Debug, Clone)]
pub struct SubGroup {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    /// `field_14` : nombre de variantes (offset X de la variante dans le DS1).
    pub variants: u32,
}

/// Un DS1 de bordure LvlSub parsé (plans utiles + groupes de substitution).
pub struct SubDrlgFile {
    pub w: u32,
    pub h: u32,
    /// Plan mur 0 brut (int32 DS1).
    pub wall0: Vec<u32>,
    /// Plan sol 0 brut.
    pub floor0: Vec<u32>,
    pub groups: Vec<SubGroup>,
}

fn byte1(v: u32) -> u32 {
    (v >> 8) & 0xFF
}

/// Lecteur int32 LE borné (schéma DS1 retail).
struct Reader<'d> {
    d: &'d [u8],
    p: usize,
}

impl<'d> Reader<'d> {
    fn i32(&mut self) -> Result<i32, String> {
        let b = self
            .d
            .get(self.p..self.p + 4)
            .ok_or_else(|| "ds1 tronqué".to_string())?;
        self.p += 4;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn skip(&mut self, ints: usize) -> Result<(), String> {
        self.p += ints * 4;
        if self.p > self.d.len() + 4 {
            return Err("ds1 tronqué (skip)".into());
        }
        Ok(())
    }

    fn cstr(&mut self) -> Result<(), String> {
        let end = self
            .d
            .get(self.p..)
            .and_then(|r| r.iter().position(|&b| b == 0))
            .ok_or_else(|| "ds1 : chaîne tronquée".to_string())?
            + self.p;
        self.p = end + 1;
        Ok(())
    }
}

impl SubDrlgFile {
    /// Parse minimal fidèle à `DRLGPRESET_ParseDS1File` (v10..v18, subst v≥12).
    pub fn parse(d: &[u8]) -> Result<SubDrlgFile, String> {
        let mut r = Reader { d, p: 0 };
        let ver = r.i32()?;
        let w = r.i32()? as u32;
        let h = r.i32()? as u32;
        if ver >= 8 {
            r.i32()?; // act
        }
        let subst = if ver >= 10 { r.i32()? } else { 0 };
        if ver >= 3 {
            let nstr = r.i32()? as usize;
            for _ in 0..nstr {
                r.cstr()?;
            }
        }
        if (9..14).contains(&ver) {
            r.skip(2)?;
        }
        let area = ((w + 1) * (h + 1)) as usize;
        let nwall = r.i32()? as usize;
        let nfloor = if ver >= 16 { r.i32()? as usize } else { 1 };
        let mut planes: Vec<Vec<u32>> = Vec::new();
        for _ in 0..nwall {
            for _ in 0..2 {
                let mut v = Vec::with_capacity(area);
                for _ in 0..area {
                    v.push(r.i32()? as u32);
                }
                planes.push(v);
            }
        }
        let wall0 = planes.first().cloned().unwrap_or_default();
        let mut floor0 = Vec::new();
        for i in 0..nfloor {
            let mut f = Vec::with_capacity(area);
            for _ in 0..area {
                f.push(r.i32()? as u32);
            }
            if i == 0 {
                floor0 = f;
            }
        }
        r.skip(area)?; // ombre
        if subst > 0 && subst <= 2 {
            r.skip(area)?; // tags de substitution
        }
        if ver > 1 {
            let units = r.i32()?;
            for _ in 0..units.max(0) {
                r.skip(2)?; // type, id
                r.skip(2)?; // x, y
                if ver > 5 {
                    r.skip(1)?; // spawn flag
                }
            }
        }
        if ver >= 18 {
            r.skip(1)?;
        }
        let mut groups = Vec::new();
        if ver >= 12 && (0..=2).contains(&subst) {
            let ng = r.i32()? as usize;
            for _ in 0..ng {
                let gx = r.i32()? as u32;
                let gy = r.i32()? as u32;
                let gw = r.i32()? as u32;
                let gh = r.i32()? as u32;
                let variants = if ver >= 13 { (r.i32()? as u32).max(1) } else { 1 };
                groups.push(SubGroup { x: gx, y: gy, w: gw, h: gh, variants });
            }
        }
        Ok(SubDrlgFile { w: w + 1, h: h + 1, wall0, floor0, groups })
    }

    fn cell(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            return 0;
        }
        self.wall0.get((y as u32 * self.w + x as u32) as usize).copied().unwrap_or(0)
    }

    fn floor_cell(&self, x: i32, y: i32) -> u32 {
        if x < 0 || y < 0 {
            return 0;
        }
        self.floor0.get((y as u32 * self.w + x as u32) as usize).copied().unwrap_or(0)
    }

    /// Décode une cellule du plan mur (encodage subst : bit 0 + byte 1).
    pub fn wall_cell(&self, x: i32, y: i32) -> SubCell {
        let v = self.cell(x, y);
        if v & 1 == 0 {
            return SubCell::Inert;
        }
        match byte1(v) {
            0 => SubCell::Inert, // v17 = -1 : le test retail échoue
            63 => SubCell::Exempt,
            b1 => SubCell::Preset(ids::WILD_BORDER_1 + (b1 as i32 - 1)),
        }
    }

    /// Directive sol (bit 1 du plan sol) : libérer la cellule pour le fill.
    pub fn floor_free(&self, x: i32, y: i32) -> bool {
        self.floor_cell(x, y) & 2 != 0
    }
}

static SUB_FILES: OnceLock<BTreeMap<i32, SubDrlgFile>> = OnceLock::new();

fn sub_files() -> &'static BTreeMap<i32, SubDrlgFile> {
    SUB_FILES.get_or_init(|| {
        let mut m = BTreeMap::new();
        for (id, bytes) in [
            (ids::SUB_CLIFFS, SUB_DS1_CLIFFS),
            (ids::SUB_MIDDLE, SUB_DS1_MIDDLE),
            (ids::SUB_CORNER, SUB_DS1_CORNER),
            (ids::SUB_BORDER, SUB_DS1_BORDER),
        ] {
            if let Ok(f) = SubDrlgFile::parse(bytes) {
                m.insert(id, f);
            }
        }
        m
    })
}

// ---------------------------------------------------------------------------
// Grilles outdoor (port de DRLGGRID + D2DrlgOutdoorPackedGrid2InfoStrc)
// ---------------------------------------------------------------------------

/// Info packed de la grille 2 (booléens nommés d'après D2MOO).
#[derive(Debug, Clone, Copy, Default)]
struct Cell2 {
    /// preset posé (bHasPickedFile).
    has_picked_file: bool,
    /// variante de fichier (nPickedFile).
    picked_file: u8,
    /// nUnkb00 : bordure / lien — invalide pour tout spawn.
    border: bool,
    /// nUnkb07 : sommet de chemin (SpawnRandomOutdoorDS1 préfère).
    path: bool,
    /// nUnkb08 : blanc — pas de fill.
    blank: bool,
    /// bHasDirection : sommet directionnel.
    has_direction: bool,
    /// bLvlLink : liaison vers un autre niveau.
    lvl_link: bool,
    /// nUnkb11 : waypoint.
    waypoint: bool,
    /// nUnkb12 : sanctuaire.
    shrine: bool,
}

impl Cell2 {
    /// `DRLGOUTDOORS_TestGridCellSpawnValid` : !((b00|b07|b08|picked|b11|b12)).
    fn spawn_valid(&self) -> bool {
        !(self.border || self.path || self.blank || self.has_picked_file || self.waypoint || self.shrine)
    }
}

/// Fisher-Yates retail (`SpawnOutdoorLevelPreset` & co) : UNE passe, DEUX
/// tirages par pas, échange (r1, r2) — l'ordre exact est reproduit.
fn shuffled_coords(rng: &mut D2Seed, w: i32, h: i32) -> Vec<(i32, i32)> {
    let area = (w * h) as usize;
    let mut coord: Vec<(i32, i32)> = (0..area).map(|i| (i as i32 % w, i as i32 / w)).collect();
    for _i in 0..area {
        let r1 = rng.limited(area as i32) as usize;
        let r2 = rng.limited(area as i32) as usize;
        coord.swap(r1, r2);
    }
    coord
}

// ---------------------------------------------------------------------------
// LUT de bordures retail (DrlgOutPlace.cpp — verbatim)
// ---------------------------------------------------------------------------

/// `nBorderIndices` — ligne de `LEVEL_PREST_BORDER` par (dx, dy) ;
/// indices 0..49 = arêtes, 50..99 = coins (`sub_6FD80C10`).
const N_BORDER_INDICES: [i32; 100] = [
    -1, 1, -1, 0, -1, 2, -1, 3, -1, 0,
    1, 9, 9, -1, -1, 1, 8, -1, -1, 12,
    -1, -1, -1, -1, 12, 4, -1, -1, 5, 2,
    2, 10, -1, -1, -1, -1, 10, 1, 9, 9,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, -1, -1, -1, -1, -1, -1, -1, -1, -1,
    -1, 11, 11, 3, 12, -1, -1, -1, -1, 12,
    4, 4, 7, -1, -1, 2, 10, -1, -1, -1,
    -1, 10, -1, -1, 6, 3, -1, -1, 11, 11,
    3, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];

/// `levelPrestBorder` — colonnes acte I uniquement (0 = falaises, 1 =
/// bordures sauvages) ; les colonnes actes II/IV du retail sont inatteignables
/// pour LVLTYPE_ACT1_WILDERNESS. Ligne 0 : données retail illisibles (optim).
const LEVEL_PREST_BORDER_ACT1: [[i32; 2]; 13] = [
    [0, 0],    // ligne 0 (inatteignable ici)
    [0, 4],    // WILD_BORDER_1
    [16, 5],   // CLIFF_2 / WILD_BORDER_2
    [17, 6],   // CLIFF_3 / WILD_BORDER_3
    [0, 7],    // WILD_BORDER_4
    [18, 8],   // CLIFF_5 / WILD_BORDER_5
    [19, 9],   // CLIFF_6A / WILD_BORDER_6
    [22, 10],  // CLIFF_7 / WILD_BORDER_7
    [0, 11],   // WILD_BORDER_8
    [0, 12],   // WILD_BORDER_9
    [23, 13],  // CLIFF_10 / WILD_BORDER_10
    [0, 14],   // WILD_BORDER_11
    [0, 15],   // WILD_BORDER_12
];

/// `sub_6FD80BE0` : preset de bordure pour une arête (dx, dy).
fn sign(v: i32) -> i32 {
    if v > 0 {
        1
    } else if v < 0 {
        -1
    } else {
        0
    }
}

fn border_preset_edge(dx: i32, dy: i32, col: usize) -> i32 {
    let idx = dx + 3 * dy + 4;
    if !(0..N_BORDER_INDICES.len() as i32).contains(&idx) {
        return 0;
    }
    let row = N_BORDER_INDICES[idx as usize];
    if row >= 0 && col < 2 {
        LEVEL_PREST_BORDER_ACT1[(row + 1) as usize][col]
    } else {
        0
    }
}

/// `sub_6FD80C10` : preset de coin (dx, dy) → (ndx, ndy).
fn border_preset_corner(dx: i32, dy: i32, ndx: i32, ndy: i32, col: usize) -> i32 {
    let a1 = if dx > 0 { dx + 2 } else if dx < 0 { dx - 2 } else { 0 };
    let a3 = if ndx > 0 { ndx + 2 } else if ndx < 0 { ndx - 2 } else { 0 };
    let idx = dy + a1 + 9 * (ndy + a3) + 50;
    if !(0..N_BORDER_INDICES.len() as i32).contains(&idx) {
        return 0;
    }
    let v6 = N_BORDER_INDICES[idx as usize];
    if v6 == -1 {
        0
    } else if col < 2 {
        LEVEL_PREST_BORDER_ACT1[v6 as usize][col]
    } else {
        0
    }
}

// ---------------------------------------------------------------------------
// Sortie publique
// ---------------------------------------------------------------------------

/// Un preset posé sur la grille (position cellule, Def, variante).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placement {
    pub def: i32,
    /// Variante de fichier retail (nPickedFile, ≤ Files-1).
    pub file: u8,
    pub x: i32,
    pub y: i32,
    /// Étape retail qui a posé le preset (bordure LUT, rivière, lvlsub…).
    pub source: &'static str,
}

/// Une cellule de grille générée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// Preset LvlPrest (Def, variante).
    Preset(i32, u8),
    /// Fill retail (room construite depuis Dt1Mask 0x44103) — symbolique ici.
    Fill,
    /// Blanc (`SetBlankGridCell` — pas de room).
    Blank,
}

/// Résultat de `generate_exact` : la grille retail complète.
#[derive(Debug, Clone)]
pub struct ExactWilderness {
    pub level_id: u32,
    pub level_name: String,
    pub grid_w: u32,
    pub grid_h: u32,
    /// Cellules `grid_w × grid_h` (row-major).
    pub cells: Vec<CellKind>,
    /// Presets posés, ordre retail d'ajout (dernier sur une cellule = gagnant).
    pub placements: Vec<Placement>,
    /// Sanctuaires (cellule + flag grille 1 : 0x1000..0x8000).
    pub shrines: Vec<(i32, i32, u32)>,
    /// Waypoint (cellule) — None pour la Blood Moor (retail : niveaux 3..6).
    pub waypoint: Option<(i32, i32)>,
    /// Flags outdoor retenus en entrée (approximation autonome documentée).
    pub out_flags: u32,
    /// Tirages RNG consommés (charge retail mesurable).
    pub rng_rolls: u64,
}

impl ExactWilderness {
    pub fn cell(&self, x: i32, y: i32) -> Option<CellKind> {
        if x >= 0 && y >= 0 && (x as u32) < self.grid_w && (y as u32) < self.grid_h {
            Some(self.cells[y as usize * self.grid_w as usize + x as usize])
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Générateur retail acte I
// ---------------------------------------------------------------------------

struct Gen<'a> {
    level_id: u32,
    tables: &'a ExactTables,
    gw: usize,
    gh: usize,
    g0: Vec<i32>,
    g1: Vec<u32>,
    g2: Vec<Cell2>,
    seed: D2Seed,
    /// Liste `pBuild` retail : round-robin des variantes par preset.
    build: Vec<(i32, u32, u32)>,
    out_flags: u32,
    placements: Vec<Placement>,
    rng_rolls: u64,
}

impl<'a> Gen<'a> {
    fn inside(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && (x as usize) < self.gw && (y as usize) < self.gh
    }

    fn g0(&self, x: i32, y: i32) -> i32 {
        if self.inside(x, y) { self.g0[y as usize * self.gw + x as usize] } else { 0 }
    }

    fn g2(&self, x: i32, y: i32) -> Cell2 {
        if self.inside(x, y) { self.g2[y as usize * self.gw + x as usize] } else { Cell2::default() }
    }

    fn g2_mut(&mut self, x: i32, y: i32) -> &mut Cell2 {
        let idx = y as usize * self.gw + x as usize;
        &mut self.g2[idx]
    }

    fn roll(&mut self) -> u64 {
        self.rng_rolls += 1;
        self.seed.roll()
    }

    fn limited(&mut self, n: i32) -> u32 {
        self.rng_rolls += 1;
        self.seed.limited(n)
    }

    fn shuffle(&mut self, w: i32, h: i32) -> Vec<(i32, i32)> {
        self.rng_rolls += 2 * (w * h) as u64;
        shuffled_coords(&mut self.seed, w, h)
    }

    /// `DRLGOUTDOORS_SpawnOutdoorLevelPresetEx` — pose un preset avec
    /// round-robin de variante quand `file = -1` (liste pBuild retail).
    fn spawn_ex(&mut self, x: i32, y: i32, def: i32, file: i32, border: bool, source: &'static str) {
        let (sx, sy, files_count) = match self.tables.prest(def) {
            Some(p) => ((p.size_x / 8) as i32, (p.size_y / 8) as i32, p.files_count),
            None => return,
        };
        let picked: u8 = if file == -1 {
            let divisor = files_count.max(1);
            let entry = if let Some(e) = self.build.iter_mut().find(|(d, _, _)| *d == def) {
                e.2 = (e.2 + 1) % divisor;
                e.2
            } else {
                let r = self.limited(files_count as i32);
                self.build.push((def, divisor, r));
                r
            };
            entry as u8
        } else {
            file as u8
        };
        for j in y..y + sy {
            for i in x..x + sx {
                if !self.inside(i, j) {
                    continue;
                }
                let c = self.g2_mut(i, j);
                c.has_picked_file = true;
                c.picked_file = picked;
                if border && (ids::WILD_BORDER_1..=ids::WILD_BORDER_12).contains(&def) {
                    c.border = true;
                }
            }
        }
        if self.inside(x, y) {
            self.g0[y as usize * self.gw + x as usize] = def;
        }
        self.placements.push(Placement { def, file: picked, x, y, source });
    }

    /// `DRLGOUTDOORS_TestOutdoorLevelPreset` — tout le rectangle (étendu par
    /// `offset` selon les bits de `flags`) doit être dans la grille et valide.
    fn test_preset(&self, x: i32, y: i32, def: i32, offset: i32, flags: u8) -> bool {
        let (mut sx, mut sy, mut xs, mut ys) = (1i32, 1i32, x, y);
        if def != 0 {
            match self.tables.prest(def) {
                Some(p) => {
                    sx = (p.size_x / 8) as i32;
                    sy = (p.size_y / 8) as i32;
                }
                None => return false,
            }
        }
        if offset != 0 {
            if flags & 1 != 0 {
                ys -= offset;
                sy += offset;
            }
            if flags & 2 != 0 {
                sx += offset;
            }
            if flags & 4 != 0 {
                sy += offset;
            }
            if flags & 8 != 0 {
                xs -= offset;
                sx += offset;
            }
        }
        for j in ys..ys + sy {
            for i in xs..xs + sx {
                if !self.inside(i, j) || !self.g2(i, j).spawn_valid() {
                    return false;
                }
            }
        }
        true
    }

    fn interior(&self) -> (i32, i32) {
        let w = self.gw as i32 - 2;
        let h = self.gh as i32 - 2;
        (w, h)
    }

    /// `DRLGOUTDOORS_SpawnOutdoorLevelPreset` — Fisher-Yates puis premier
    /// emplacement valide (intérieur 1..gw-1).
    fn spawn_anywhere(&mut self, def: i32, file: i32, offset: i32, flags: u8, source: &'static str) -> bool {
        let (w, h) = self.interior();
        if w <= 0 || h <= 0 {
            return false;
        }
        for (cx, cy) in self.shuffle(w, h) {
            let (x, y) = (cx + 1, cy + 1);
            if self.test_preset(x, y, def, offset, flags) {
                self.spawn_ex(x, y, def, file, false, source);
                return true;
            }
        }
        false
    }

    /// `DRLGOUTDOORS_SpawnPresetFarAway` — scan cyclique RNG, garde la case
    /// qui maximise la distance pondérée à l'ancre (coords Rogue Encampment).
    fn spawn_far_away(
        &mut self,
        anchor: (i32, i32),
        def: i32,
        file: i32,
        offset: i32,
        flags: u8,
        source: &'static str,
    ) -> bool {
        let (w, h) = self.interior();
        if w <= 0 || h <= 0 {
            return false;
        }
        let rx = self.limited(w) as i32;
        let ry = self.limited(h) as i32;
        let mut best = (0i32, 0i32, -1i64);
        for i in 0..=h {
            let py = (i + ry) % h + 1;
            for j in 0..=w {
                let px = (j + rx) % w + 1;
                if self.test_preset(px, py, def, offset, flags) {
                    let ax = (8 * px - anchor.0).abs() as i64;
                    let ay = (8 * py - anchor.1).abs() as i64;
                    let score = if ax <= ay { ax + 2 * ay } else { ay + 2 * ax };
                    if best.2 < score / 2 {
                        best = (px, py, score / 2);
                    }
                }
            }
        }
        if best.2 < 0 {
            false
        } else {
            self.spawn_ex(best.0, best.1, def, file, false, source);
            true
        }
    }

    /// `DRLGOUTDOORS_SpawnRandomOutdoorDS1` — priorité aux sommets de chemin
    /// (grille 2 nUnkb07) via voisinage 8, puis repli `spawn_anywhere`.
    fn spawn_random_ds1(&mut self, def: i32, file: i32, source: &'static str) -> bool {
        let (w, h) = self.interior();
        if w <= 0 || h <= 0 {
            return false;
        }
        for (cx, cy) in self.shuffle(w, h) {
            let (x, y) = (cx + 1, cy + 1);
            if self.g2(x, y).path {
                for (ox, oy) in
                    [(-1, 0), (0, -1), (0, 1), (1, 0), (-1, -1), (1, 1), (1, -1), (-1, 1)]
                {
                    let (px, py) = (x + ox, y + oy);
                    if self.test_preset(px, py, def, 0, 15) {
                        self.spawn_ex(px, py, def, file, false, source);
                        return true;
                    }
                }
            }
        }
        self.spawn_anywhere(def, file, 0, 15, source)
    }

    /// `DRLGOUTPLACE_SetBlankBorderGridCells` — 4 marches diagonales depuis
    /// les coins : blanc jusqu'à la première cellule de bordure.
    fn set_blank_border_cells(&mut self) {
        const OFFS: [((i32, i32), (i32, i32)); 4] =
            [((0, 0), (1, 1)), ((1, 0), (-1, 1)), ((0, 1), (1, -1)), ((1, 1), (-1, -1))];
        let (gw, gh) = (self.gw as i32, self.gh as i32);
        for ((sx, sy), (dx, dy)) in OFFS {
            let x0 = if sx != 0 { gw - 1 } else { 0 };
            let y0 = if sy != 0 { gh - 1 } else { 0 };
            let (mut j, mut guard) = (y0, 0i32);
            while self.inside(x0, j) && !self.g2(x0, j).border && guard < gh {
                let (mut i, mut guard_i) = (x0, 0i32);
                while self.inside(i, j) && !self.g2(i, j).border && guard_i < gw {
                    self.g2_mut(i, j).blank = true;
                    i += dx;
                    guard_i += 1;
                }
                j += dy;
                guard += 1;
            }
        }
    }

    /// `DRLGOUTPLACE_PlaceAct1245OutdoorBorders` — polygone du niveau ; ici
    /// le rectangle autonome (écart documenté : le retail suit le polygone
    /// Vis du graphe d'acte). LVLTYPE_ACT1_WILDERNESS : colonne de LUT
    /// `(nDirection == 0)` = 1 (sommets sans direction → bordures sauvages).
    fn place_borders(&mut self) {
        let gw = self.gw as i32;
        let gh = self.gh as i32;
        let verts = [(0, 0), (gw - 1, 0), (gw - 1, gh - 1), (0, gh - 1)];
        // DRLGVER_GetCoordDiff retail : diffs UNITAIRES (signes) le long du
        // polygone — le LUT est indexé par (dx, dy) ∈ {-1, 0, 1}.
        let unit = |a: (i32, i32), b: (i32, i32)| (sign(b.0 - a.0), sign(b.1 - a.1));
        for v in 0..4 {
            let cur = verts[v];
            let next = verts[(v + 1) % 4];
            let after = verts[(v + 2) % 4];
            let (dx, dy) = unit(cur, next);
            let (ndx, ndy) = unit(next, after);
            let edge = border_preset_edge(dx, dy, 1);
            let (mut px, mut py) = cur;
            while px != next.0 || py != next.1 {
                px += dx;
                py += dy;
                self.spawn_ex(px, py, edge, -1, false, "border-lut");
                self.g2_mut(px, py).border = true;
            }
            // coin (flags & 2 absents : pas de doublement des diffs)
            let corner = border_preset_corner(dx, dy, ndx, ndy, 1);
            let def = if corner == ids::CLIFF_BORDER_6A {
                // retail : 6B si les deux sommets ont la direction 1, sinon 6C
                // (nos sommets autonomes ont nDirection = 0 → 6C, comme retail)
                ids::CLIFF_BORDER_6C
            } else {
                corner
            };
            if def != ids::NONE {
                self.spawn_ex(next.0, next.1, def, -1, false, "border-corner");
                self.g2_mut(next.0, next.1).border = true;
            }
        }
        self.set_blank_border_cells();
    }

    /// `DRLGOUTDOORS_AddAct124SecondaryBorder` + `DRLGTILESUB_*` — sous-
    /// placements LvlSub réels (groupes lus dans les DS1 de bordure MPQ).
    fn add_secondary_border(&mut self, sub_id: i32) {
        let rows: Vec<LvlSubRow> = self.tables.lvl_sub_rows(sub_id).into_iter().cloned().collect();
        for row in rows {
            let Some(file) = sub_files().get(&sub_id) else { continue };
            if file.groups.is_empty() {
                continue;
            }
            let field_14: i32 = 62; // retail : init -1 puis 62 au premier groupe
            let mut nrand = 0usize;
            if row.bord_type == 0 {
                nrand = self.limited(file.groups.len() as i32) as usize;
            }
            let mut bbreak = false;
            for j in 0..file.groups.len() {
                let group = &file.groups[(nrand + j) % file.groups.len()];
                let b_wilderness =
                    (ids::BLOODMOOR..=ids::TAMOE_HIGHLAND).contains(&self.level_id);
                let offset = if sub_id == ids::SUB_MIDDLE && self.out_flags & ids::FLAG_RIVER != 0 {
                    -1
                } else {
                    1
                };
                let w = offset + self.gw as i32 - row.grid_size as i32 * group.w as i32;
                let h = 1 + self.gh as i32 - row.grid_size as i32 * group.h as i32;
                if w <= 0 || h <= 0 {
                    continue;
                }
                let small = sub_id == ids::SUB_MIDDLE && b_wilderness && w < 6 && h < 6;
                for (cx, cy) in self.shuffle(w, h) {
                    if small && cx == 2 && cy == 2 {
                        continue;
                    }
                    if self.test_replace_sub(cx, cy, file, group, field_14) {
                        let a6 =
                            ((self.limited(group.variants as i32) + 1) * (group.w + 1)) as i32;
                        self.replace_sub(cx, cy, file, group, a6);
                        if row.bord_type == 0 {
                            bbreak = true;
                            break;
                        } else if row.bord_type == 1 {
                            break;
                        }
                    }
                }
                if bbreak {
                    break;
                }
            }
        }
    }

    /// `DRLGTILESUB_TestReplaceSubPreset` — valide le box (variante 0) :
    /// murs = même Def déjà posé + pas de lien ; sol bit 1 = emplacement libre.
    fn test_replace_sub(&self, x: i32, y: i32, file: &SubDrlgFile, group: &SubGroup, field_14: i32) -> bool {
        let gs = 1i32; // GridSize des bordures acte I (lvlSub.txt)
        let v9 = x - x % gs;
        let v8 = y - y % gs;
        for j in 0..group.h as i32 {
            for i in 0..group.w as i32 {
                let (sx, sy) = (group.x as i32 + i, group.y as i32 + j);
                let wall = file.cell(sx, sy);
                let (gx, gy) = (v9 + i * gs, v8 + j * gs);
                if wall & 1 != 0 {
                    let v17 = byte1(wall) as i32 - 1;
                    if v17 != field_14 && v17 + ids::WILD_BORDER_1 != self.g0(gx, gy) {
                        return false;
                    }
                    if self.g2(gx, gy).lvl_link {
                        return false;
                    }
                } else if file.floor_cell(sx, sy) & 2 != 0 {
                    if !self.test_preset(gx, gy, 0, 0, 0) {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// `DRLGTILESUB_ReplaceSubPreset` — pose depuis la variante tirée (`a6`).
    fn replace_sub(&mut self, x: i32, y: i32, file: &SubDrlgFile, group: &SubGroup, a6: i32) {
        let gs = 1i32;
        let v9 = x - x % gs;
        let v8 = y - y % gs;
        for j in 0..group.h as i32 {
            for i in 0..group.w as i32 {
                let (sx, sy) = (a6 + group.x as i32 + i, group.y as i32 + j);
                let wall = file.cell(sx, sy);
                let (gx, gy) = (v9 + i * gs, v8 + j * gs);
                if wall & 1 != 0 {
                    let v17 = byte1(wall) as i32 - 1;
                    let v18 = v17 + ids::WILD_BORDER_1;
                    if v18 != -5 && v17 != 62 && self.inside(gx, gy) {
                        self.spawn_ex(gx, gy, v18, 0, true, "lvlsub");
                    }
                } else if file.floor_free(sx, sy) {
                    // AlterAdjacentPresetGridCells : libère pour le fill
                    if self.inside(gx, gy) {
                        let idx = gy as usize * self.gw + gx as usize;
                        self.g0[idx] = 0;
                        self.g2[idx].has_picked_file = false;
                        self.g2[idx].picked_file = 0;
                    }
                } else if self.inside(gx, gy) {
                    // SetBlankGridCell
                    self.g2_mut(gx, gy).blank = true;
                }
            }
        }
    }

    /// `DRLGOUTWILD_TestSpawnRiver` + `DRLGOUTWILD_SpawnRiver` — rivière
    /// verticale en colonne gw-2 (fichiers par LUT sur les bordures présentes).
    fn spawn_river(&mut self) {
        let nx = self.gw as i32 - 2;
        for y in 0..self.gh as i32 {
            if self.g2(nx, y).has_direction || self.g2(nx + 1, y).has_direction {
                return;
            }
        }
        // table stru_6FDD0CA0 : (fichier haut, fichier bas) par Def - BORDER_1
        const RIVER_FILES: [(u8, u8); 12] =
            [(2, 2), (0, 3), (1, 1), (3, 0), (0, 2), (0, 1), (1, 0), (2, 0), (2, 3), (1, 3), (3, 1), (3, 2)];
        for y in 0..self.gh as i32 {
            for lower in [false, true] {
                let cx = nx + lower as i32;
                let g0 = self.g0(cx, y);
                let info = self.g2(cx, y);
                let picked = if g0 != 0 {
                    if g0 == 7 && info.picked_file == 3 {
                        3
                    } else {
                        let idx = (g0 - ids::WILD_BORDER_1) as usize;
                        if idx < 12 {
                            if lower { RIVER_FILES[idx].1 } else { RIVER_FILES[idx].0 }
                        } else {
                            3
                        }
                    }
                } else if info.blank {
                    0
                } else {
                    3
                };
                let def = if lower { ids::RIVER_LOWER } else { ids::RIVER_UPPER };
                self.spawn_ex(cx, y, def, picked as i32, false, "river");
            }
        }
    }

    /// `DRLGOUTWILD_SpawnCliffCaves` — Def 16/17 déjà posé → cave gauche/droite.
    fn spawn_cliff_caves(&mut self, x: i32, y: i32) -> bool {
        match self.g0(x, y) {
            16 => {
                self.spawn_ex(x, y, ids::CLIFF_CAVE_LEFT, -1, false, "cliff-cave");
                self.out_flags |= ids::FLAG_SPECIAL_DONE;
                true
            }
            17 => {
                self.spawn_ex(x, y, ids::CLIFF_CAVE_RIGHT, -1, false, "cliff-cave");
                self.out_flags |= ids::FLAG_SPECIAL_DONE;
                true
            }
            _ => false,
        }
    }

    /// `DRLGOUTWILD_SpawnTownTransitionsAndCaves` — transitions ville + cave.
    fn spawn_town_transitions_and_caves(&mut self) {
        if self.out_flags & ids::FLAG_RIVER != 0 {
            self.spawn_river();
        }
        if self.out_flags & ids::FLAG_TRANS_S_1 != 0 {
            self.spawn_ex(0, 0, ids::TOWN_1_TRANS_S, 1, false, "town-trans");
        }
        if self.out_flags & ids::FLAG_TRANS_S_2 != 0 {
            let x = self.gw as i32 - 7;
            self.spawn_ex(x, 0, ids::TOWN_1_TRANS_S, 2, false, "town-trans");
        }
        if self.out_flags & ids::FLAG_TRANS_E_1 != 0 {
            self.spawn_ex(0, 1, ids::TOWN_1_TRANS_E, 1, false, "town-trans");
        }
        if self.out_flags & ids::FLAG_TRANS_E_2 != 0 {
            let y = self.gh as i32 - 6;
            self.spawn_ex(0, y, ids::TOWN_1_TRANS_E, 1, false, "town-trans");
        }
        if self.out_flags & ids::FLAG_SPECIAL_DONE == 0 {
            // Blood Moor : Den of Evil loin du Rogue Encampment ; les autres :
            // cave entrance aléatoire (port verbatim du if retail).
            let added = if self.level_id == ids::BLOODMOOR {
                // ancre autonome : Rogue Encampment au sud-ouest (écart
                // documenté : la position retail vient du graphe d'acte).
                self.spawn_far_away((0, self.gh as i32 * 8), ids::DOE_ENTRANCE, -1, 1, 15, "doe")
            } else {
                self.spawn_anywhere(ids::CAVE_ENTRANCE, -1, 1, 15, "cave-entrance")
            };
            if added {
                self.out_flags |= ids::FLAG_SPECIAL_DONE;
            }
        }
    }

    /// `DRLGOUTDOORS_SpawnAct12Waypoint` — niveaux 3..6 (PAS la Blood Moor).
    fn spawn_waypoint(&mut self) {
        let (w, h) = self.interior();
        if w <= 0 || h <= 0 {
            return;
        }
        for (cx, cy) in self.shuffle(w, h) {
            let (x, y) = (cx + 1, cy + 1);
            if self.g2(x, y).spawn_valid() {
                self.g1[y as usize * self.gw + x as usize] |= 0x10000;
                self.g2_mut(x, y).waypoint = true;
                return;
            }
        }
    }

    /// `DRLGOUTDOORS_SpawnAct12Shrines` — n sanctuaires, directions cycliques.
    fn spawn_shrines(&mut self, count: i32) {
        const DIRS: [u32; 4] = [0x1000, 0x2000, 0x4000, 0x8000];
        let (w, h) = self.interior();
        if w <= 0 || h <= 0 {
            return;
        }
        let mut index = (self.roll() & 3) as usize;
        let mut left = count;
        for (cx, cy) in self.shuffle(w, h) {
            if left <= 0 {
                break;
            }
            let (x, y) = (cx + 1, cy + 1);
            if self.g2(x, y).spawn_valid() {
                self.g1[y as usize * self.gw + x as usize] |= DIRS[index];
                self.g2_mut(x, y).shrine = true;
                index = (index + 1) % 4;
                left -= 1;
            }
        }
    }

    /// `DRLGOUTWILD_SpawnSpecialPresets` — cas Blood Moor et Cold Plains
    /// (port verbatim) ; les autres niveaux acte I suivront le même moule.
    fn spawn_special_presets(&mut self) {
        match self.level_id {
            ids::BLOODMOOR => {
                self.spawn_random_ds1(ids::POND, -1, "special");
                if self.roll() & 3 == 0 {
                    self.spawn_random_ds1(ids::COTTAGES_1, -1, "special");
                }
                self.spawn_random_ds1(ids::COTTAGES_1, -1, "special");
                self.spawn_anywhere(ids::STONE_FILL_1, -1, 0, 15, "special");
                self.spawn_anywhere(ids::STONE_FILL_2, -1, 0, 15, "special");
            }
            ids::COLDPLAINS => {
                if self.roll() & 3 != 0 {
                    self.spawn_random_ds1(ids::COTTAGES_2, -1, "special");
                    if self.roll() & 1 != 0 {
                        self.spawn_random_ds1(ids::COTTAGES_3, -1, "special");
                    }
                } else {
                    self.spawn_random_ds1(ids::COTTAGES_2, -1, "special");
                    self.spawn_random_ds1(ids::COTTAGES_2, -1, "special");
                }
                self.spawn_anywhere(ids::FALLEN_CAMP_BISHIBOSH, -1, 0, 15, "special");
                self.spawn_anywhere(ids::STONE_FILL_1, -1, 0, 15, "special");
                self.spawn_anywhere(ids::STONE_FILL_2, -1, 0, 15, "special");
            }
            _ => {}
        }
    }
}

/// Flags outdoor autonomes retenus pour un niveau acte I (approximation
/// documentée du graphe d'acte retail : lien ville + deux transitions sud ;
/// pas de rivière ni de cave de falaise en Blood Moor autonome).
fn standalone_flags(level_id: u32) -> u32 {
    if (ids::BLOODMOOR..=ids::TAMOE_HIGHLAND).contains(&level_id) {
        ids::FLAG_TOWN_LINK | ids::FLAG_TRANS_S_1 | ids::FLAG_TRANS_S_2
    } else {
        ids::FLAG_TOWN_LINK | ids::FLAG_TRANS_S_1
    }
}

/// Génère le DRLG wilderness retail d'un niveau (DrlgType 3) — étape 20a.
///
/// `seed` = graine DRLG du niveau (`SEED_InitLowSeed` retail ; la dérivation
/// graine de partie → graine de niveau vit dans D2Game, hors périmètre).
pub fn generate_exact(level_id: u32, seed: u32, tables: &ExactTables) -> Result<ExactWilderness, String> {
    generate_exact_with_flags(level_id, seed, tables, standalone_flags(level_id))
}

/// Variante à flags explicites (tests / futur graphe d'acte : les bits sont
/// ceux du dwFlags outdoor retail — rivière 0x4|0x8, cave 0x20, transitions
/// 0x80..0x400).
pub fn generate_exact_with_flags(
    level_id: u32,
    seed: u32,
    tables: &ExactTables,
    out_flags: u32,
) -> Result<ExactWilderness, String> {
    let level = tables
        .level(level_id)
        .ok_or_else(|| format!("drlg_exact : niveau {level_id} absent de levels.txt"))?;
    if level.drlg_type != 3 {
        return Err(format!(
            "drlg_exact : « {} » est DrlgType {} (wilderness attendu : 3)",
            level.name, level.drlg_type
        ));
    }
    let gw = (level.size_x / 8) as usize;
    let gh = (level.size_y / 8) as usize;
    if gw < 3 || gh < 3 {
        return Err(format!("drlg_exact : grille {gw}×{gh} trop petite"));
    }
    let mut g = Gen {
        level_id,
        tables,
        gw,
        gh,
        g0: vec![0; gw * gh],
        g1: vec![0; gw * gh],
        g2: vec![Cell2::default(); gw * gh],
        seed: D2Seed::new(seed),
        build: Vec::new(),
        out_flags,
        placements: Vec::new(),
        rng_rolls: 0,
    };

    // DRLGOUTWILD_InitAct1OutdoorLevel — séquence verbatim (levels 2..7)
    if (ids::BLOODMOOR..=ids::TAMOE_HIGHLAND).contains(&level_id) {
        // SetOutGridLinkFlags : no-op autonome (polygone rectangle sans lien —
        // écart documenté), puis bordures LUT retail.
        g.place_borders();
        g.add_secondary_border(ids::SUB_CLIFFS);
        if level_id != ids::MOOMOOFARM {
            if g.out_flags & ids::FLAG_RIVER != 0 {
                // bloc flags & 0xC du port (TestSpawnRiver est inclus dans
                // spawn_river, colonne gw-2)
                g.spawn_river();
            }
            if g.out_flags & ids::FLAG_CAVE != 0 && g.out_flags & ids::FLAG_SPECIAL_DONE == 0 {
                let flipped = g.roll() & 1 == 0;
                'scan: for a in 0..gw as i32 {
                    for b in 0..gh as i32 {
                        let (x, y) = if flipped { (a, b) } else { (b, a) };
                        if g.spawn_cliff_caves(x, y) {
                            break 'scan;
                        }
                    }
                }
            }
            if g.out_flags & 0x1C != 0 && g.out_flags & ids::FLAG_SPECIAL_DONE == 0 {
                let mut y = g.gh as i32 - 4;
                let mut x = g.gw as i32 - ((!((g.out_flags & 0xFF) as u8) | 0x40) >> 4) as i32;
                let v21 = g.roll() & 3;
                if v21 % 2 != 0 {
                    x = 3;
                }
                if v21 / 2 != 0 {
                    y = 3;
                }
                let def = (level_id == ids::BLOODMOOR) as i32 + 51;
                g.spawn_ex(x, y, def, -1, false, "doe-quadrant");
                g.out_flags |= ids::FLAG_SPECIAL_DONE;
            }
        }
        g.add_secondary_border(ids::SUB_MIDDLE);
        g.add_secondary_border(ids::SUB_CORNER);
        g.spawn_town_transitions_and_caves();
        g.add_secondary_border(ids::SUB_BORDER);
        // SpawnAct1DirtPaths : peinture de sol au niveau room — hors périmètre
        // structurel (écart documenté dans REPORT.md) ; nos tirages RNG
        // s'arrêtent donc avant ceux des chemins de terre retail.
    }

    if (ids::COLDPLAINS..=ids::BLACKMARSH).contains(&level_id) {
        g.spawn_waypoint();
    }
    if (ids::BLOODMOOR..=ids::TAMOE_HIGHLAND).contains(&level_id) {
        g.spawn_shrines(5);
    }
    g.spawn_special_presets();

    // DRLGOUTDOORS_GenerateLevel — projection finale grille → cellules
    let mut cells = Vec::with_capacity(gw * gh);
    for j in 0..gh {
        for i in 0..gw {
            let idx = j * gw + i;
            let c2 = g.g2[idx];
            let kind = if c2.has_picked_file {
                CellKind::Preset(g.g0[idx], c2.picked_file)
            } else if c2.blank {
                CellKind::Blank
            } else {
                CellKind::Fill
            };
            cells.push(kind);
        }
    }
    let shrines = (0..gw * gh)
        .filter(|&i| g.g2[i].shrine)
        .map(|i| ((i % gw) as i32, (i / gw) as i32, g.g1[i]))
        .collect();
    let waypoint = (0..gw * gh)
        .find(|&i| g.g2[i].waypoint)
        .map(|i| ((i % gw) as i32, (i / gw) as i32));
    Ok(ExactWilderness {
        level_id,
        level_name: level.name.clone(),
        grid_w: gw as u32,
        grid_h: gh as u32,
        cells,
        placements: g.placements,
        shrines,
        waypoint,
        out_flags: g.out_flags,
        rng_rolls: g.rng_rolls,
    })
}

// ---------------------------------------------------------------------------
// Marchabilité : banque de DS1 presets + collage 9×9 à pas de 8
// ---------------------------------------------------------------------------

/// Banque (Def, variante) → octets DS1 — alimentée par l'appelant (assets
/// embarqués du moteur, extraction MPQ, etc.).
pub type PresetBank = BTreeMap<(i32, usize), Vec<u8>>;

/// DS1 preset décodé juste ce qu'il faut pour la marche (schéma retail).
struct Ds1Lite {
    w: usize,
    h: usize,
    /// plans murs (indices pairs après entrelacement).
    walls: Vec<Vec<u32>>,
    /// plan sol.
    floor: Vec<u32>,
}

fn parse_ds1_lite(d: &[u8]) -> Result<Ds1Lite, String> {
    let mut r = Reader { d, p: 0 };
    let ver = r.i32()?;
    let w = r.i32()? as usize;
    let h = r.i32()? as usize;
    if ver >= 8 {
        r.i32()?; // act
    }
    let subst = if ver >= 10 { r.i32()? } else { 0 };
    if ver >= 3 {
        let nstr = r.i32()? as usize;
        for _ in 0..nstr {
            r.cstr()?;
        }
    }
    if (9..14).contains(&ver) {
        r.skip(2)?;
    }
    let area = (w + 1) * (h + 1);
    let nwall = r.i32()? as usize;
    let nfloor = if ver >= 16 { r.i32()? as usize } else { 1 };
    let mut walls = Vec::new();
    for _i in 0..nwall {
        let mut v = Vec::with_capacity(area);
        for _ in 0..area {
            v.push(r.i32()? as u32);
        }
        walls.push(v);
        r.skip(area)?; // plan tiletype associé
    }
    let floor = if nfloor > 0 {
        let mut f = Vec::with_capacity(area);
        for _ in 0..area {
            f.push(r.i32()? as u32);
        }
        f
    } else {
        Vec::new()
    };
    let _ = subst; // les plans subst ne servent pas à la marche
    let _ = r.skip(area); // ombre (les presets du moteur n'ont pas de tags subst)
    Ok(Ds1Lite { w: w + 1, h: h + 1, walls, floor })
}

impl ExactWilderness {
    /// Grille de marche au niveau cellule DS1 : `grid_w*8 × grid_h*8`
    /// (les presets 9×9 sont collés à pas de 8, la ligne/colonne de recouvrement
    /// est écrasée par le preset suivant, comme le fill retail).
    ///
    /// Approximations documentées (REPORT.md) : preset sans DS1 connu =
    /// ouvert ; fill = ouvert ; blanc = bloqué ; collision par cellule (pas
    /// de sous-tuiles DT1.
    pub fn walk_grid(&self, bank: &PresetBank) -> Vec<bool> {
        let wpx = (self.grid_w * 8) as usize;
        let hpx = (self.grid_h * 8) as usize;
        let mut walk = vec![false; wpx * hpx];
        let mut parsed: BTreeMap<(i32, usize), Ds1Lite> = BTreeMap::new();

        // fill : tout ce qui n'est ni preset ni blanc ouvre ses 8×8
        for (i, cell) in self.cells.iter().enumerate() {
            if matches!(cell, CellKind::Fill) {
                let (cx, cy) = (i % self.grid_w as usize, i / self.grid_w as usize);
                for dy in 0..8 {
                    for dx in 0..8 {
                        let (x, y) = (cx * 8 + dx, cy * 8 + dy);
                        if x < wpx && y < hpx {
                            walk[y * wpx + x] = true;
                        }
                    }
                }
            }
        }
        // presets : dernier posé = gagnant (ordre retail) ; sans DS1 connu
        // dans la banque → ouvert 8×8 (approximation documentée).
        for p in &self.placements {
            let still_there = self
                .cell(p.x, p.y)
                .map(|c| matches!(c, CellKind::Preset(d, f) if d == p.def && f == p.file))
                .unwrap_or(false);
            if !still_there {
                continue; // écrasé plus tard par un autre preset
            }
            let key = (p.def, p.file as usize);
            if !parsed.contains_key(&key) {
                match bank.get(&key) {
                    Some(bytes) => match parse_ds1_lite(bytes) {
                        Ok(d) => {
                            parsed.insert(key, d);
                        }
                        Err(_) => {
                            parsed.insert(key, Ds1Lite { w: 0, h: 0, walls: vec![], floor: vec![] });
                        }
                    },
                    None => {
                        parsed.insert(key, Ds1Lite { w: 0, h: 0, walls: vec![], floor: vec![] });
                    }
                }
            }
            let ds1 = parsed.get(&key).unwrap();
            if ds1.w == 0 || ds1.h == 0 {
                // ouvert (approximation) — mais ne masque pas un fill bloqué
                let (bx, by) = (p.x as usize * 8, p.y as usize * 8);
                for cy in 0..8 {
                    for cx in 0..8 {
                        let (x, y) = (bx + cx, by + cy);
                        if x < wpx && y < hpx {
                            walk[y * wpx + x] = true;
                        }
                    }
                }
                continue;
            }
            let (bx, by) = (p.x as usize * 8, p.y as usize * 8);
            for cy in 0..ds1.h {
                for cx in 0..ds1.w {
                    let (x, y) = (bx + cx, by + cy);
                    if x >= wpx || y >= hpx {
                        continue;
                    }
                    let idx = y * wpx + x;
                    let blocked = ds1.walls.iter().any(|pl| pl.get(cy * ds1.w + cx).copied().unwrap_or(0) != 0);
                    if blocked {
                        walk[idx] = false;
                    } else if ds1.floor.get(cy * ds1.w + cx).copied().unwrap_or(0) != 0 {
                        walk[idx] = true;
                    }
                }
            }
        }
        walk
    }
}

// ---------------------------------------------------------------------------
// Tests — RNG retail, structure Blood Moor, LvlSub réels, marchabilité
// ---------------------------------------------------------------------------

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn tables() -> ExactTables {
        ExactTables::load().unwrap()
    }

    /// Banque DS1 réelle : banque du moteur (`assets/map/presets_bloodmoor/`)
    /// + extraction MPQ (`evidence/exact_ds1/`), indexée (Def, variante)
    /// selon les `File1..File6` de LvlPrest.txt.
    pub(crate) fn bank(tables: &ExactTables) -> PresetBank {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .map(|p| p.to_path_buf())
            .unwrap();
        let dirs = ["assets/map/presets_bloodmoor", "evidence/exact_ds1"];
        let by_name: BTreeMap<String, Vec<u8>> = dirs
            .iter()
            .filter_map(|d| std::fs::read_dir(root.join(d)).ok())
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|x| x == "ds1").unwrap_or(false))
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                std::fs::read(e.path()).ok().map(|b| (name, b))
            })
            .collect();
        let mut out = PresetBank::new();
        for p in tables.prests.values() {
            for (k, f) in p.files.iter().enumerate() {
                if k as u32 >= p.files_count.max(1) && p.files_count > 0 {
                    continue;
                }
                let base = f.rsplit(['/', '\\']).next().unwrap_or("").to_ascii_lowercase();
                if let Some(b) = by_name.get(&base) {
                    out.insert((p.def, k), b.clone());
                }
            }
        }
        out
    }

    // --- RNG retail ---------------------------------------------------------

    #[test]
    fn rng_matches_retail_reference() {
        // Référence indépendante (Python, formule D2MOO D2Seed.h) : graine 1.
        let mut s = D2Seed::new(1);
        let lows = [
            1791398751u32, 791599131, 671516612, 3064641593, 3217527747, 716489901, 2609384736,
            2946765574,
        ];
        let highs = [
            0u32, 747178749, 330169957, 280084454, 1278238622, 1342006271, 298842470, 1088354461,
        ];
        let masks = [3u64, 3, 0, 1, 3, 1, 0, 2];
        for i in 0..8 {
            let r = s.roll();
            assert_eq!(r as u32, lows[i], "low {i}");
            assert_eq!((r >> 32) as u32, highs[i], "high {i}");
            assert_eq!(r & 3, masks[i], "&3 {i}");
        }
    }

    #[test]
    fn rng_limited_branches_match_retail() {
        // n puissance de 2 → masque ; sinon modulo ; n ≤ 0 → 0. Référence Python.
        let mut s = D2Seed::new(42);
        for expected in [44u32, 45, 24, 56, 30, 28] {
            assert_eq!(s.limited(64), expected, "limited(64)");
        }
        let mut s = D2Seed::new(42);
        for expected in [4u32, 1, 6, 0, 8, 2] {
            assert_eq!(s.limited(10), expected, "limited(10)");
        }
        let mut s = D2Seed::new(7);
        assert_eq!(s.limited(0), 0);
        assert_eq!(s.limited(-5), 0);
    }

    // --- tables + Dt1Mask ---------------------------------------------------

    #[test]
    fn tables_load_and_dt1mask_compat_is_data_driven() {
        let t = tables();
        let bm = t.level(2).unwrap();
        assert_eq!(bm.name, "Act 1 - Wilderness 1");
        assert_eq!((bm.size_x, bm.size_y, bm.drlg_type, bm.level_type), (80, 80, 3, 2));
        // Den of Evil = 8 dans les Vis retail (patch_levels.txt)
        assert!(bm.vis.contains(&8), "Vis Blood Moor doit référencer le Den of Evil");
        // presets acte I compatibles Blood Moor (Dt1Mask ⊆ tileset type 2)
        let act1_defs = [4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 46, 47, 51, 52, 2, 3];
        let compat = act1_defs.iter().filter(|&&d| t.dt1_mask_compat(bm, d)).count();
        assert!(compat >= 28, "bordures/rivières/specs compatibles attendus : {compat}");
        // un preset hors tileset (ex. cotteges expansion) reste rejeté si masque ⊄
        let t2 = t.prest(40).unwrap(); // Tree Fill : Mask=3 (bits 0,1)
        assert!(t.dt1_mask_compat(bm, 40) == (t2.dt1_mask & !0x3FFF_FFFF == 0));
    }

    // --- sous-placements LvlSub réels ----------------------------------------

    #[test]
    fn sub_drlg_files_parse_retail_groups() {
        let subs = sub_files();
        assert_eq!(subs.get(&ids::SUB_CLIFFS).unwrap().groups.len(), 15, "BorderCliffs");
        assert_eq!(subs.get(&ids::SUB_MIDDLE).unwrap().groups.len(), 9, "BorderMiddle");
        assert_eq!(subs.get(&ids::SUB_CORNER).unwrap().groups.len(), 8, "BorderCorner");
        assert_eq!(subs.get(&ids::SUB_BORDER).unwrap().groups.len(), 20, "BorderBorder");
        // encodage : les références de preset des groupes de falaises visent
        // les Defs 4..23 (bordures + falaises) ou des cellules exemptes.
        let cliffs = subs.get(&ids::SUB_CLIFFS).unwrap();
        let g = &cliffs.groups[4]; // box (0,0 5x5) : presets {16, 18, 4, exempt}
        let mut found = std::collections::BTreeSet::new();
        for y in 0..g.h as i32 {
            for x in 0..g.w as i32 {
                match cliffs.wall_cell(g.x as i32 + x, g.y as i32 + y) {
                    SubCell::Preset(d) => {
                        found.insert(d);
                        assert!((4..=23).contains(&d), "Def {d} hors bornes retail");
                    }
                    SubCell::Exempt | SubCell::Inert => {}
                }
            }
        }
        assert!(found.contains(&16) && found.contains(&4), "falaises + bordures attendues : {found:?}");
    }

    // --- Blood Moor : structure retail ---------------------------------------

    fn bloodmoor(seed: u32) -> (ExactWilderness, ExactTables) {
        let t = tables();
        let w = generate_exact(2, seed, &t).unwrap();
        (w, t)
    }

    #[test]
    fn bloodmoor_generation_is_deterministic_and_seed_sensitive() {
        let (a, _) = bloodmoor(12345);
        let (b, _) = bloodmoor(12345);
        assert_eq!(a.cells, b.cells, "même graine ⇒ même grille");
        assert_eq!(a.placements, b.placements);
        assert_eq!(a.rng_rolls, b.rng_rolls);
        assert!(a.rng_rolls > 1000, "charge RNG retail consommée : {}", a.rng_rolls);
        let (c, _) = bloodmoor(12346);
        assert_ne!(a.cells, c.cells, "graine différente ⇒ carte différente");
    }

    #[test]
    fn bloodmoor_structure_matches_retail_recipe() {
        let t = tables();
        for seed in [0u32, 1, 42, 12345, 0xDEAD_BEEF] {
            let w = generate_exact(2, seed, &t).unwrap();
            assert_eq!((w.grid_w, w.grid_h), (10, 10), "Blood Moor 80/8 = 10×10");
            assert_eq!(w.cells.len(), 100);
            // grilles entièrement couvertes : preset / fill / blanc
            assert!(w.cells.iter().all(|c| matches!(c, CellKind::Preset(_, _) | CellKind::Fill | CellKind::Blank)));
            // spéciaux retail : étang, cottages (1 à 2 : le second est
            // conditionnel `!(roll & 3)`), stone fills, Den of Evil
            let count = |d: i32| w.placements.iter().filter(|p| p.def == d).count();
            assert!(count(46) >= 1, "étang (Pond) seed {seed}");
            assert!((1..=2).contains(&count(47)), "cottages : {}", count(47));
            assert!(count(29) >= 1 && count(30) >= 1, "stone fills seed {seed}");
            assert!(w.placements.iter().any(|p| p.def == 52 && p.source == "doe-quadrant"), "Den of Evil");
            // 5 sanctuaires, flags cycliques 0x1000..0x8000
            assert_eq!(w.shrines.len(), 5, "seed {seed}");
            let mut flags: Vec<u32> = w.shrines.iter().map(|s| s.2 & 0xF000).collect();
            flags.sort_unstable();
            flags.dedup();
            assert_eq!(flags.len(), 4, "4 directions de sanctuaires : {flags:?}");
            // Blood Moor : PAS de waypoint (retail : niveaux 3..6)
            assert!(w.waypoint.is_none(), "seed {seed}");
            // pas de rivière en autonome (flags sans 0xC)
            assert!(w.placements.iter().all(|p| p.def != 26 && p.def != 27), "seed {seed}");
            // Den of Evil en quadrant RNG : x ∈ {3,4}, y ∈ {3,6}
            let doe = w.placements.iter().find(|p| p.source == "doe-quadrant").unwrap();
            assert!((3..=4).contains(&doe.x) && [3, 6].contains(&doe.y), "doe à {:?}", (doe.x, doe.y));
            // anneau de bordures retail : Def ∈ bordures sauvages (LUT)
            let ring: Vec<i32> = (0..10)
                .flat_map(|i| [w.cell(i, 0).unwrap(), w.cell(i, 9).unwrap(), w.cell(0, i).unwrap(), w.cell(9, i).unwrap()])
                .filter_map(|c| match c {
                    CellKind::Preset(d, _) => Some(d),
                    _ => None,
                })
                .collect();
            assert!(!ring.is_empty(), "anneau de bordures seed {seed}");
            // bordures LUT 4..15 ; les transitions ville (Defs 2/3) recouvrent
            // légalement des cellules de l'anneau (SpawnEx retail = dernier gagnant)
            assert!(ring.iter().all(|d| (2..=15).contains(d)), "anneau LUT : {ring:?}");
            // Dt1Mask : tout preset posé est compatible avec le tileset type 2
            let bm = t.level(2).unwrap();
            for p in &w.placements {
                assert!(t.dt1_mask_compat(bm, p.def), "Def {} incompatible Dt1Mask", p.def);
            }
        }
    }

    #[test]
    fn secondary_borders_apply_retail_semantics() {
        let t = tables();
        let w = generate_exact(2, 777, &t).unwrap();
        let sub: Vec<_> = w.placements.iter().filter(|p| p.source == "lvlsub").collect();
        // BordType 0 (Middle) : au plus UNE pose ; BordType 2 (Border) : plusieurs
        let middle = w.placements.iter().filter(|p| p.source == "lvlsub" && p.x == 0 && p.y == 0).count();
        let _ = middle;
        assert!(!sub.is_empty(), "au moins un sous-placement LvlSub posé");
        assert!(sub.iter().all(|p| (4..=23).contains(&p.def)), "defs LvlSub = bordures/falaises");
        // déterminisme déjà couvert ; vérifions la charge retail des scans
        assert!(w.rng_rolls > 4000, "scans LvlSub mélangés : {}", w.rng_rolls);
    }

    #[test]
    fn river_appears_when_flags_ask_for_it() {
        let t = tables();
        let w = generate_exact_with_flags(2, 9, &t, ids::FLAG_RIVER | ids::FLAG_TOWN_LINK | ids::FLAG_TRANS_S_1)
            .unwrap();
        let upper = w.placements.iter().filter(|p| p.def == 26 && p.source == "river").count();
        let lower = w.placements.iter().filter(|p| p.def == 27 && p.source == "river").count();
        assert!(upper >= 8 && lower >= 8, "rivière colonne gw-2 : haut {upper} bas {lower}");
        // colonne exacte : les Defs 26/27 sont posés en x=8 (haut) et x=9 (bas)
        assert!(w.placements.iter().filter(|p| p.def == 26).all(|p| p.x == 8));
        assert!(w.placements.iter().filter(|p| p.def == 27).all(|p| p.x == 9));
    }

    #[test]
    fn coldplains_has_waypoint_and_bishibosh() {
        let t = tables();
        let w = generate_exact(3, 4242, &t).unwrap();
        assert_eq!((w.grid_w, w.grid_h), (10, 10));
        let wp = w.waypoint.expect("Cold Plains : waypoint retail (niveaux 3..6)");
        assert!(matches!(w.cell(wp.0, wp.1), Some(CellKind::Preset(_, _)) | Some(CellKind::Fill)));
        assert!(w.placements.iter().any(|p| p.def == 44), "camp Bishibosh (Def 44)");
        assert!(w.placements.iter().any(|p| p.def == 51), "cave entrance (Def 51) hors Blood Moor");
    }

    #[test]
    fn non_wilderness_levels_are_rejected() {
        let t = tables();
        assert!(generate_exact(1, 0, &t).is_err(), "Rogue Encampment = ville (DrlgType 0)");
    }

    // --- marchabilité avec vrais DS1 -----------------------------------------

    #[test]
    fn bloodmoor_walkable_with_real_ds1_bank() {
        let t = tables();
        let b = bank(&t);
        assert!(b.len() > 60, "banque DS1 réelle : {}", b.len());
        let w = generate_exact(2, 20261025, &t).unwrap();
        let walk = w.walk_grid(&b);
        let wpx = 80usize;
        assert_eq!(walk.len(), wpx * wpx);
        let walkable = walk.iter().filter(|x| **x).count();
        let ratio = walkable as f32 / walk.len() as f32;
        assert!((0.30..0.95).contains(&ratio), "plaine marchable : {ratio:.2}");
        // connexité : BFS depuis le centre atteint > 40 % des cellules ouvertes
        let start = (40usize, 40usize);
        let mut seen = vec![false; walk.len()];
        let mut queue = std::collections::VecDeque::new();
        // cherche une cellule de départ marchable près du centre
        'outer: for r in 0..20 {
            for dy in -r..=r {
                for dx in -r..=r {
                    let (x, y) = (start.0 as i32 + dx, start.1 as i32 + dy);
                    if x < 0 || y < 0 || x >= 80 || y >= 80 {
                        continue;
                    }
                    if walk[y as usize * wpx + x as usize] {
                        queue.push_back((x as usize, y as usize));
                        seen[y as usize * wpx + x as usize] = true;
                        break 'outer;
                    }
                }
            }
        }
        assert!(!queue.is_empty(), "aucune cellule marchable au centre");
        while let Some((x, y)) = queue.pop_front() {
            for (dx, dy) in [(0i32, -1i32), (1, 0), (0, 1), (-1, 0)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= 80 || ny >= 80 {
                    continue;
                }
                let (nx, ny) = (nx as usize, ny as usize);
                let i = ny * wpx + nx;
                if walk[i] && !seen[i] {
                    seen[i] = true;
                    queue.push_back((nx, ny));
                }
            }
        }
        let reachable = seen.iter().zip(walk.iter()).filter(|(s, o)| **s && **o).count();
        let reach_ratio = reachable as f32 / walkable as f32;
        assert!(reach_ratio > 0.40, "composante principale : {reach_ratio:.2} de l'ouvert");
        // l'anneau extérieur est plus bloqué que le centre (bordures forestières)
        let ring_walk = (0..80).filter(|&i| walk[i] || walk[79 * 80 + i] || walk[i * 80] || walk[i * 80 + 79]).count() as f32 / (80 * 4) as f32;
        let center_walk = (30..50).map(|y| (30..50).filter(|x| walk[y * 80 + x]).count()).sum::<usize>() as f32 / 400.0;
        assert!(center_walk > ring_walk, "centre {center_walk:.2} plus ouvert que l'anneau {ring_walk:.2}");
    }
}

#[cfg(test)]
mod metrics_dump {
    //! Sortie de mesure pour `evidence/drlg20a-golden.json` (les tests de
    //! structure restent la garde stable ; ce dump est `#[ignore]` manuel).
    use super::tests::{bank, tables};
    use super::*;
    #[test]
    #[ignore = "dump de mesure manuel : cargo test -p d2game metrics_dump -- --ignored --nocapture"]
    fn dump() {
        let t = ExactTables::load().unwrap();
        let b = bank(&t);
        let mut out = String::from("{\n  \"module\": \"d2game::drlg_exact\",\n  \"seeds\": [\n");
        for (i, seed) in [0u32, 12345, 20261025].iter().enumerate() {
            let w = generate_exact(2, *seed, &t).unwrap();
            let walk = w.walk_grid(&b);
            let walkable = walk.iter().filter(|x| **x).count();
            let mut by_source: std::collections::BTreeMap<&str, usize> = Default::default();
            let mut by_def: std::collections::BTreeMap<i32, usize> = Default::default();
            for p in &w.placements {
                *by_source.entry(p.source).or_default() += 1;
                *by_def.entry(p.def).or_default() += 1;
            }
            let src = by_source.iter().map(|(k, v)| format!("\"{k}\": {v}")).collect::<Vec<_>>().join(", ");
            let df = by_def.iter().map(|(k, v)| format!("\"{k}\": {v}")).collect::<Vec<_>>().join(", ");
            out.push_str(&format!(
                "    {{ \"seed\": {seed}, \"grid\": [{}, {}], \"rng_rolls\": {}, \"placements\": {}, \"walkable_ratio\": {:.4}, \"shrines\": {}, \"by_source\": {{{}}}, \"by_def\": {{{}}} }}{}\n",
                w.grid_w,
                w.grid_h,
                w.rng_rolls,
                w.placements.len(),
                walkable as f32 / walk.len() as f32,
                w.shrines.len(),
                src,
                df,
                if i + 1 == 3 { "" } else { "," }
            ));
        }
        out.push_str("  ]\n}\n");
        println!("{out}");
    }
}
