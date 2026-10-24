//! Records typés des tables réelles v1.12 — socle data-driven W01/A3 (audits
//! d2w-re).
//!
//! Chaque structure mappe les colonnes consommées d'une table `.txt` réelle ;
//! les golden tests (bas de fichier) verrouillent **record-à-record** les
//! comptes ET des champs de contrôle contre les fichiers réels embarqués par
//! `include_str!` — toute dérive de la table casse le build de tests, pas le
//! runtime (les records restent lenient : cellule absente ⇒ `""`/`0`).
//!
//! Sources embarquées (lecture seule) :
//! * `d2w-re/tables/monstres/` — monstats2, monai, monpreset, MonPlace ;
//! * `d2w-re/tables/monde/` — objgroup, shrines, AutoMap, LvlSub ;
//! * `d2w-re/tables/items-ui/` — rareprefix, raresuffix, qualityitems,
//!   lowqualityitems, inventory ;
//! * `d2w-integ/assets/` — superuniques, monumod, LvlWarp, LvlMaze,
//!   patch_itemratio.
//!
//! Découvertes format notées au fil des tests (CRLF, quotes Excel `""`,
//! colonnes `*` commentaires, en-tête dupliqué d'AutoMap.txt…).

use crate::tsv::{Tsv, TsvError};

/// `d2w-re/tables/monstres/monstats2.txt` (610 records) — gabarits
/// d'animation/corps des monstres. `BaseW` est une chaîne (`1hs`) : les
/// colonnes d'armes portent des listes quotées (`"lit,med,hvy"`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonStats2Rec {
    pub id: String,
    pub height: u32,
    pub overlay_height: u32,
    pub pix_height: u32,
    pub size_x: u32,
    pub size_y: u32,
    pub melee_rng: u32,
    pub base_w: String,
    pub hit_class: u32,
}

/// `monai.txt` (148 records) — IA par nom ; colonnes `*aip1..*aip8` vides
/// (paramètres non instanciés) et `*eol` = 0 (colonne commentaire D2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonAiRec {
    pub ai: String,
    pub eol: u32,
}

/// `monpreset.txt` (229 records) — presets de placement `(acte, place)`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonPresetRec {
    pub act: u32,
    pub place: String,
}

/// `MonPlace.txt` (37 records) — codes de placement (`place_nothing`…).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonPlaceRec {
    pub code: String,
}

/// `objgroup.txt` (133 records) — groupes d'objets DRLG : 8 slots
/// `ID{i}`/`DENSITY{i}`/`PROB{i}` + colonnes `SHRINES`/`WELLS`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjGroupRec {
    pub group_name: String,
    pub offset: u32,
    pub id: [u32; 8],
    pub density: [u32; 8],
    pub prob: [u32; 8],
    pub shrines: u32,
    pub wells: u32,
}

/// `shrines.txt` (23 records) — sanctuaires ; `Effect` quoté Excel avec `""`
/// (ex. `doubles current mana (") (OUT)`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ShrineRec {
    pub shrine_type: String,
    pub name: String,
    pub effect: String,
    pub code: u32,
    pub rarity: u32,
    pub level_min: u32,
}

/// `AutoMap.txt` (3287 records) — styles d'automap par tuile. L'en-tête réel
/// porte DEUX colonnes `Type2` : le parseur prend la première occurrence.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutoMapRec {
    pub level_name: String,
    pub tile_name: String,
    pub style: i32,
    pub start_sequence: i32,
    pub end_sequence: i32,
}

/// `LvlSub.txt` (35 records) — sous-types DS1 (bords, thèmes, preshashes).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LvlSubRec {
    pub name: String,
    pub kind: u32,
    pub file: String,
    pub dt1_mask: u32,
    pub beta: u32,
}

/// `items-ui/rareprefix.txt` (46) & `raresuffix.txt` (155) — affixes rares
/// (mêmes colonnes `name/version/itype1..7/etype1..4/add/multiply/divide`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RareAffixRec {
    pub name: String,
    pub version: u32,
    pub itype1: String,
    pub add: u32,
    pub multiply: u32,
    pub divide: u32,
}

/// `items-ui/qualityitems.txt` (8 records) — mods des objets de qualité.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QualityItemRec {
    pub nummods: u32,
    pub mod1code: String,
    pub mod1param: u32,
    pub mod1min: u32,
    pub mod1max: u32,
    pub effect1: String,
    pub level: u32,
    pub add: u32,
}

/// `items-ui/lowqualityitems.txt` (4 records) — noms basse qualité.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LowQualityItemRec {
    pub name: String,
}

/// `items-ui/inventory.txt` (33 records) — gabarits d'inventaire par classe
/// (coordonnées écran possiblement négatives ⇒ `i32`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InventoryRec {
    pub class: String,
    pub inv_left: i32,
    pub inv_top: i32,
    pub grid_x: u32,
    pub grid_y: u32,
    pub grid_left: i32,
    pub grid_top: i32,
}

/// `assets/superuniques.txt` (67 records) — boss nommés ; packs `MinGrp`/
/// `MaxGrp`, TC par difficulté (`TC`/`TC(N)`/`TC(H)`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SuperUniqueRec {
    pub superunique: String,
    pub name: String,
    pub class: String,
    pub min_grp: u32,
    pub max_grp: u32,
    pub tc: String,
}

/// `assets/monumod.txt` (43 records) — mods unique/champion ; `constants`
/// porte les constantes moteur indexées par `*constant desc`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MonUModRec {
    pub uniquemod: String,
    pub id: u32,
    pub enabled: u32,
    pub constants: u32,
}

/// `assets/LvlWarp.txt` (89 records) — tuiles de transition ; `Direction`
/// est une lettre (`b`…), `LitVersion` un drapeau.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LvlWarpRec {
    pub name: String,
    pub id: u32,
    pub lit_version: u32,
    pub direction: String,
}

/// `assets/LvlMaze.txt` (82 records) — tailles labyrinthes (colonnes
/// `Rooms`/`Rooms(N)`/`Rooms(H)` — record = variante Normal).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LvlMazeRec {
    pub name: String,
    pub level: u32,
    pub rooms: u32,
    pub size_x: u32,
    pub size_y: u32,
    pub merge: u32,
}

/// `assets/patch_itemratio.txt` (6 records) — ratios de drop
/// (unique/set/rare/magic/hiqueality/normal ; `Class Specific` contient une
/// espace : accès par nom exact).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemRatioRec {
    pub function: String,
    pub version: u32,
    pub uber: u32,
    pub class_specific: u32,
    pub unique: u32,
    pub unique_divisor: u32,
    pub unique_min: u32,
    pub rare: u32,
    pub rare_divisor: u32,
    pub rare_min: u32,
}

macro_rules! map_rows {
    ($tsv:expr, |$rec:ident| $body:expr) => {{
        let tsv: &Tsv = $tsv;
        let mut out = Vec::with_capacity(tsv.len());
        for i in 0..tsv.len() {
            let $rec = tsv.rec(i).expect("index valide");
            out.push($body);
        }
        Ok(out)
    }};
}

impl MonStats2Rec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<MonStats2Rec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| MonStats2Rec {
            id: r.s("Id").to_string(),
            height: r.u("Height"),
            overlay_height: r.u("OverlayHeight"),
            pix_height: r.u("pixHeight"),
            size_x: r.u("SizeX"),
            size_y: r.u("SizeY"),
            melee_rng: r.u("MeleeRng"),
            base_w: r.s("BaseW").to_string(),
            hit_class: r.u("HitClass"),
        })
    }
}

impl MonAiRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<MonAiRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| MonAiRec { ai: r.s("AI").to_string(), eol: r.u("*eol") })
    }
}

impl MonPresetRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<MonPresetRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| MonPresetRec { act: r.u("Act"), place: r.s("Place").to_string() })
    }
}

impl MonPlaceRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<MonPlaceRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| MonPlaceRec { code: r.s("code").to_string() })
    }
}

impl ObjGroupRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<ObjGroupRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| {
            let slot = |p: &str| -> [u32; 8] {
                core::array::from_fn(|i| r.u(&format!("{p}{i}")))
            };
            ObjGroupRec {
                group_name: r.s("GroupName").to_string(),
                offset: r.u("Offset"),
                id: slot("ID"),
                density: slot("DENSITY"),
                prob: slot("PROB"),
                shrines: r.u("SHRINES"),
                wells: r.u("WELLS"),
            }
        })
    }
}

impl ShrineRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<ShrineRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| ShrineRec {
            shrine_type: r.s("Shrine Type").to_string(),
            name: r.s("Shrine name").to_string(),
            effect: r.s("Effect").to_string(),
            code: r.u("Code"),
            rarity: r.u("rarity"),
            level_min: r.u("LevelMin"),
        })
    }
}

impl AutoMapRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<AutoMapRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| AutoMapRec {
            level_name: r.s("LevelName").to_string(),
            tile_name: r.s("TileName").to_string(),
            style: r.i("Style"),
            start_sequence: r.i("StartSequence"),
            end_sequence: r.i("EndSequence"),
        })
    }
}

impl LvlSubRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<LvlSubRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| LvlSubRec {
            name: r.s("Name").to_string(),
            kind: r.u("Type"),
            file: r.s("File").to_string(),
            dt1_mask: r.u("Dt1Mask"),
            beta: r.u("Beta"),
        })
    }
}

impl RareAffixRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<RareAffixRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| RareAffixRec {
            name: r.s("name").to_string(),
            version: r.u("version"),
            itype1: r.s("itype1").to_string(),
            add: r.u("add"),
            multiply: r.u("multiply"),
            divide: r.u("divide"),
        })
    }
}

impl QualityItemRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<QualityItemRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| QualityItemRec {
            nummods: r.u("nummods"),
            mod1code: r.s("mod1code").to_string(),
            mod1param: r.u("mod1param"),
            mod1min: r.u("mod1min"),
            mod1max: r.u("mod1max"),
            effect1: r.s("effect1").to_string(),
            level: r.u("level"),
            add: r.u("add"),
        })
    }
}

impl LowQualityItemRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<LowQualityItemRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| LowQualityItemRec { name: r.s("Name").to_string() })
    }
}

impl InventoryRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<InventoryRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| InventoryRec {
            class: r.s("class").to_string(),
            inv_left: r.i("invLeft"),
            inv_top: r.i("invTop"),
            grid_x: r.u("gridX"),
            grid_y: r.u("gridY"),
            grid_left: r.i("gridLeft"),
            grid_top: r.i("gridTop"),
        })
    }
}

impl SuperUniqueRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<SuperUniqueRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| SuperUniqueRec {
            superunique: r.s("Superunique").to_string(),
            name: r.s("Name").to_string(),
            class: r.s("Class").to_string(),
            min_grp: r.u("MinGrp"),
            max_grp: r.u("MaxGrp"),
            tc: r.s("TC").to_string(),
        })
    }
}

impl MonUModRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<MonUModRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| MonUModRec {
            uniquemod: r.s("uniquemod").to_string(),
            id: r.u("id"),
            enabled: r.u("enabled"),
            constants: r.u("constants"),
        })
    }
}

impl LvlWarpRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<LvlWarpRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| LvlWarpRec {
            name: r.s("Name").to_string(),
            id: r.u("Id"),
            lit_version: r.u("LitVersion"),
            direction: r.s("Direction").to_string(),
        })
    }
}

impl LvlMazeRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<LvlMazeRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| LvlMazeRec {
            name: r.s("Name").to_string(),
            level: r.u("Level"),
            rooms: r.u("Rooms"),
            size_x: r.u("SizeX"),
            size_y: r.u("SizeY"),
            merge: r.u("Merge"),
        })
    }
}

impl ItemRatioRec {
    pub fn parse_all(tsv_text: &str) -> Result<Vec<ItemRatioRec>, TsvError> {
        let t = Tsv::parse(tsv_text)?;
        map_rows!(&t, |r| ItemRatioRec {
            function: r.s("Function").to_string(),
            version: r.u("Version"),
            uber: r.u("Uber"),
            class_specific: r.u("Class Specific"),
            unique: r.u("Unique"),
            unique_divisor: r.u("UniqueDivisor"),
            unique_min: r.u("UniqueMin"),
            rare: r.u("Rare"),
            rare_divisor: r.u("RareDivisor"),
            rare_min: r.u("RareMin"),
        })
    }
}

// ---------------------------------------------------------------------------
// Golden tests — comptes + champs de contrôle, record-à-record, contre les
// tables réelles. Sources citées fichier par fichier.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod golden {
    use super::*;

    // Tables d2w-re (livrables RE, lecture seule) — 5 remontées depuis
    // crates/d2rules/src : src → d2rules → crates → engine-rs → d2w-integ →
    // workspace.
    const MONSTATS2: &str = include_str!("../../../../../d2w-re/tables/monstres/monstats2.txt");
    const MONAI: &str = include_str!("../../../../../d2w-re/tables/monstres/monai.txt");
    const MONPRESET: &str = include_str!("../../../../../d2w-re/tables/monstres/monpreset.txt");
    const MONPLACE: &str = include_str!("../../../../../d2w-re/tables/monstres/MonPlace.txt");
    const OBJGROUP: &str = include_str!("../../../../../d2w-re/tables/monde/objgroup.txt");
    const SHRINES: &str = include_str!("../../../../../d2w-re/tables/monde/shrines.txt");
    const AUTOMAP: &str = include_str!("../../../../../d2w-re/tables/monde/AutoMap.txt");
    const LVLSUB: &str = include_str!("../../../../../d2w-re/tables/monde/LvlSub.txt");
    const RAREPREFIX: &str = include_str!("../../../../../d2w-re/tables/items-ui/rareprefix.txt");
    const RARESUFFIX: &str = include_str!("../../../../../d2w-re/tables/items-ui/raresuffix.txt");
    const QUALITYITEMS: &str = include_str!("../../../../../d2w-re/tables/items-ui/qualityitems.txt");
    const LOWQUALITYITEMS: &str =
        include_str!("../../../../../d2w-re/tables/items-ui/lowqualityitems.txt");
    const INVENTORY: &str = include_str!("../../../../../d2w-re/tables/items-ui/inventory.txt");
    // Tables embarquées du projet (d2w-integ/assets).
    const SUPERUNIQUES: &str = include_str!("../../../../assets/superuniques.txt");
    const MONUMOD: &str = include_str!("../../../../assets/monumod.txt");
    const LVLWARP: &str = include_str!("../../../../assets/LvlWarp.txt");
    const LVLMAZE: &str = include_str!("../../../../assets/LvlMaze.txt");
    const ITEM_RATIO: &str = include_str!("../../../../assets/patch_itemratio.txt");

    #[test]
    fn golden_monstats2() {
        // d2w-re/tables/monstres/monstats2.txt : 611 lignes CRLF = en-tête +
        // 610 records. Quotes Excel sur les colonnes composites
        // (`"lit,med,hvy"` ligne 2). Contrôles lus à la main (lignes 2, 6+…).
        let rs = MonStats2Rec::parse_all(MONSTATS2).unwrap();
        assert_eq!(rs.len(), 610, "monstats2.txt : 610 records");
        let first = &rs[0]; // ligne 2 du fichier
        assert_eq!(first.id, "skeleton1");
        assert_eq!((first.height, first.overlay_height, first.pix_height), (3, 2, 80));
        assert_eq!((first.size_x, first.size_y, first.melee_rng), (2, 2, 0));
        assert_eq!((first.base_w.as_str(), first.hit_class), ("1hs", 3));
        let zombie = rs.iter().find(|r| r.id == "zombie1").expect("zombie1 présent");
        assert_eq!(zombie.pix_height, 72, "zombie1 : pixHeight 72");
    }

    #[test]
    fn golden_monai() {
        // monai.txt : 149 lignes CRLF = en-tête + 148 records ; colonnes
        // `*aip1..*aip8` (commentaires D2, vides) et `*eol` = 0.
        let rs = MonAiRec::parse_all(MONAI).unwrap();
        assert_eq!(rs.len(), 148, "monai.txt : 148 records");
        assert_eq!(rs[0].ai, "None");
        assert_eq!(rs[1].ai, "Idle");
        assert_eq!(rs.last().unwrap().ai, "UberDiablo");
        assert_eq!(rs[0].eol, 0, "*eol = 0 sur les records réels");
    }

    #[test]
    fn golden_monpreset() {
        // monpreset.txt : 230 lignes CRLF = en-tête + 229 records (Act, Place).
        let rs = MonPresetRec::parse_all(MONPRESET).unwrap();
        assert_eq!(rs.len(), 229, "monpreset.txt : 229 records");
        assert_eq!((rs[0].act, rs[0].place.as_str()), (1, "gheed"));
        assert_eq!((rs[1].act, rs[1].place.as_str()), (1, "cain1"));
        assert_eq!((rs.last().unwrap().act, rs.last().unwrap().place.as_str()), (5, "place_nothing"));
    }

    #[test]
    fn golden_monplace() {
        // MonPlace.txt : 38 lignes CRLF = en-tête `code` + 37 records.
        let rs = MonPlaceRec::parse_all(MONPLACE).unwrap();
        assert_eq!(rs.len(), 37, "MonPlace.txt : 37 records");
        assert_eq!(rs[0].code, "place_nothing");
        assert_eq!(rs[1].code, "place_npc_pack");
        assert_eq!(rs.last().unwrap().code, "place_group100");
    }

    #[test]
    fn golden_objgroup() {
        // objgroup.txt : 134 lignes CRLF = en-tête + 133 records ; slots
        // ID0..7/DENSITY0..7/PROB0..7 + SHRINES/WELLS.
        let rs = ObjGroupRec::parse_all(OBJGROUP).unwrap();
        assert_eq!(rs.len(), 133, "objgroup.txt : 133 records");
        assert_eq!(rs[0].group_name, "Not Used");
        assert_eq!(rs[0].offset, 0);
        let rogues = &rs[1]; // « Rogues for act1( including staked rogues) »
        assert_eq!(rogues.offset, 1);
        assert_eq!((rogues.id[0], rogues.density[0], rogues.prob[0]), (54, 30, 25));
        assert_eq!((rogues.id[1], rogues.density[1], rogues.prob[1]), (55, 30, 25));
        assert_eq!(rogues.id[4], 0, "slots vides = 0");
        assert_eq!(rs.last().unwrap().group_name, "dummy");
        assert_eq!(rs.last().unwrap().offset, 132);
    }

    #[test]
    fn golden_shrines() {
        // shrines.txt : 24 lignes CRLF = en-tête + 23 records ; effets quotés
        // Excel avec `""` (ligne 5 : `doubles current mana (") (OUT)`).
        let rs = ShrineRec::parse_all(SHRINES).unwrap();
        assert_eq!(rs.len(), 23, "shrines.txt : 23 records");
        assert_eq!(rs[0].shrine_type, "None");
        assert_eq!(rs[0].code, 0);
        assert_eq!(rs[1].shrine_type, "Recharge");
        let mana = rs.iter().find(|r| r.name == "Mana Boost").expect("Mana Boost");
        assert_eq!(mana.effect, "doubles current mana (\") (OUT)", "déquotage Excel");
        assert_eq!((mana.code, mana.rarity, mana.level_min), (3, 2, 1));
        let last = rs.last().unwrap(); // Magic / Poison Shrine
        assert_eq!((last.shrine_type.as_str(), last.name.as_str()), ("Magic", "Poison Shrine"));
        assert_eq!(last.code, 22);
    }

    #[test]
    fn golden_automap() {
        // AutoMap.txt : 3288 lignes CRLF = en-tête + 3287 records.
        // DÉCOUVERTE FORMAT : l'en-tête porte DEUX colonnes `Type2`
        // (positions 8 et 10, 1-based) — le parseur garde la première.
        let t = Tsv::parse(AUTOMAP).unwrap();
        assert_eq!(t.len(), 3287, "AutoMap.txt : 3287 records");
        assert_eq!(t.column("Type2"), Some(7), "première colonne Type2 (0-based)");
        assert_eq!(t.columns().len(), 13, "13 colonnes nommées malgré le doublon");
        let rs = AutoMapRec::parse_all(AUTOMAP).unwrap();
        assert_eq!(rs.len(), 3287);
        assert_eq!((rs[0].level_name.as_str(), rs[0].tile_name.as_str()), ("1 Town", "fl"));
        assert_eq!((rs[0].style, rs[0].start_sequence, rs[0].end_sequence), (0, 1, 46));
        let last = rs.last().unwrap();
        assert_eq!((last.level_name.as_str(), last.tile_name.as_str()), ("5 Lava", "wl"));
        assert_eq!(last.style, 40);
    }

    #[test]
    fn golden_lvlsub() {
        // LvlSub.txt : 36 lignes CRLF = en-tête + 35 records.
        let rs = LvlSubRec::parse_all(LVLSUB).unwrap();
        assert_eq!(rs.len(), 35, "LvlSub.txt : 35 records");
        assert_eq!((rs[0].name.as_str(), rs[0].kind), ("Border - Cliffs", 0));
        assert_eq!(rs[0].file, "Act1/Outdoors/BorderCliffs.ds1");
        let last = rs.last().unwrap();
        assert_eq!((last.name.as_str(), last.kind), ("Barricade", 12));
        assert_eq!(last.file, "Expansion/Siege/4x3barricade_rules.ds1");
    }

    #[test]
    fn golden_rareprefix_raresuffix() {
        // rareprefix.txt : 47 lignes = en-tête + 46 ; raresuffix.txt :
        // 156 lignes = en-tête + 155. Mêmes colonnes d'affixes rares.
        let prefixes = RareAffixRec::parse_all(RAREPREFIX).unwrap();
        assert_eq!(prefixes.len(), 46, "rareprefix.txt : 46 records");
        assert_eq!((prefixes[0].name.as_str(), prefixes[0].version), ("Beast", 0));
        assert_eq!(prefixes[0].itype1, "armo");
        assert_eq!(prefixes.last().unwrap().name, "Corruption");
        let suffixes = RareAffixRec::parse_all(RARESUFFIX).unwrap();
        assert_eq!(suffixes.len(), 155, "raresuffix.txt : 155 records");
        assert_eq!((suffixes[0].name.as_str(), suffixes[0].version), ("bite", 0));
        assert_eq!(suffixes[0].itype1, "swor");
        assert_eq!(suffixes.last().unwrap().name, "flange");
    }

    #[test]
    fn golden_qualityitems() {
        // qualityitems.txt : 9 lignes = en-tête + 8 records.
        let rs = QualityItemRec::parse_all(QUALITYITEMS).unwrap();
        assert_eq!(rs.len(), 8, "qualityitems.txt : 8 records");
        assert_eq!(rs[0].nummods, 1);
        assert_eq!((rs[0].mod1code.as_str(), rs[0].mod1param, rs[0].mod1min, rs[0].mod1max),
            ("att", 0, 1, 3), "ligne 2 : improved to hit");
        assert_eq!(rs[0].effect1, "improved to hit");
        let last = rs.last().unwrap();
        assert_eq!((last.nummods, last.mod1code.as_str(), last.mod1min, last.mod1max),
            (2, "ac%", 5, 15), "dernier : 2 mods (ac% + dur%)");
    }

    #[test]
    fn golden_lowqualityitems() {
        // lowqualityitems.txt : 5 lignes = en-tête `Name` + 4 records.
        let rs = LowQualityItemRec::parse_all(LOWQUALITYITEMS).unwrap();
        assert_eq!(rs.len(), 4, "lowqualityitems.txt : 4 records");
        let names: Vec<&str> = rs.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["Crude", "Cracked", "Damaged", "Low Quality"]);
    }

    #[test]
    fn golden_inventory() {
        // inventory.txt : 34 lignes = en-tête + 33 records (classes + pages
        // de trade/bank + expansion).
        let rs = InventoryRec::parse_all(INVENTORY).unwrap();
        assert_eq!(rs.len(), 33, "inventory.txt : 33 records");
        assert_eq!(rs[0].class, "Amazon");
        assert_eq!((rs[0].inv_left, rs[0].inv_top, rs[0].grid_x), (320, 0, 10));
        assert!(rs.iter().any(|r| r.class == "Barbarian"));
        assert!(rs.iter().any(|r| r.class == "Druid2"), "variante expansion présente");
    }

    #[test]
    fn golden_superuniques() {
        // assets/superuniques.txt : 68 lignes CRLF = en-tête + 67 records ;
        // colonnes Mod1-3, MinGrp/MaxGrp, TC/TC(N)/TC(H), *eol.
        let rs = SuperUniqueRec::parse_all(SUPERUNIQUES).unwrap();
        assert_eq!(rs.len(), 67, "superuniques.txt : 67 records");
        let bishi = &rs[0];
        assert_eq!(bishi.superunique, "Bishibosh");
        assert_eq!(bishi.class, "fallenshaman1");
        assert_eq!((bishi.min_grp, bishi.max_grp), (2, 2));
        assert_eq!(bishi.tc, "Act 1 Super A");
        assert_eq!(rs.last().unwrap().name, "Baal Subject 5");
        assert!(rs.iter().any(|r| r.superunique == "Rakanishu"), "Rakanishu présent");
    }

    #[test]
    fn golden_monumod() {
        // assets/monumod.txt : 44 lignes CRLF = en-tête + 43 records ; la
        // colonne `constants` porte les constantes moteur (desc dans
        // `*constant desc`). Le nom de colonne `cpick (N)` contient une
        // espace — accès par nom exact.
        let rs = MonUModRec::parse_all(MONUMOD).unwrap();
        assert_eq!(rs.len(), 43, "monumod.txt : 43 records");
        assert_eq!((rs[0].uniquemod.as_str(), rs[0].id, rs[0].enabled), ("none", 0, 0));
        assert_eq!(rs[0].constants, 20, "chance champion (desc: champion chance)");
        assert_eq!((rs[1].uniquemod.as_str(), rs[1].id, rs[1].enabled), ("rndname", 1, 1));
        assert_eq!(rs[1].constants, 100, "minion +hp%");
        assert_eq!((rs.last().unwrap().uniquemod.as_str(), rs.last().unwrap().id),
            ("lightningdeath", 42));
    }

    #[test]
    fn golden_lvlwarp() {
        // assets/LvlWarp.txt : 90 lignes CRLF = en-tête + 89 records ;
        // coordonnées négatives (SelectX -90…) ⇒ i32 côté consommateurs.
        let rs = LvlWarpRec::parse_all(LVLWARP).unwrap();
        assert_eq!(rs.len(), 89, "LvlWarp.txt : 89 records");
        assert_eq!(rs[0].name, "Act 1 Wilderness to Cave Cliff L");
        assert_eq!((rs[0].id, rs[0].lit_version, rs[0].direction.as_str()), (0, 1, "b"));
        assert_eq!((rs.last().unwrap().name.as_str(), rs.last().unwrap().id),
            ("Act 5 Baal Temple Down", 82));
    }

    #[test]
    fn golden_lvlmaze() {
        // assets/LvlMaze.txt : 83 lignes CRLF = en-tête + 82 records.
        let rs = LvlMazeRec::parse_all(LVLMAZE).unwrap();
        assert_eq!(rs.len(), 82, "LvlMaze.txt : 82 records");
        assert_eq!(rs[0].name, "Act 1 - Cave 1");
        assert_eq!((rs[0].level, rs[0].rooms, rs[0].size_x, rs[0].size_y, rs[0].merge),
            (8, 1, 24, 24, 500));
        assert_eq!((rs.last().unwrap().name.as_str(), rs.last().unwrap().level),
            ("Act 5 - Pandemonium Run 3", 135));
    }

    #[test]
    fn golden_itemratio() {
        // assets/patch_itemratio.txt : 7 lignes CRLF = en-tête + 6 records ;
        // la colonne `Class Specific` contient une espace.
        let rs = ItemRatioRec::parse_all(ITEM_RATIO).unwrap();
        assert_eq!(rs.len(), 6, "patch_itemratio.txt : 6 records");
        let base = &rs[0]; // « Ratio - (Monster Level / Ratio Divisor) »
        assert_eq!(base.function, "Ratio - (Monster Level / Ratio Divisor)");
        assert_eq!((base.uber, base.class_specific), (0, 0));
        assert_eq!((base.unique, base.unique_divisor, base.unique_min), (400, 2, 6400));
        assert_eq!((base.rare, base.rare_divisor, base.rare_min), (160, 3, 3200));
        assert_eq!(rs[1].function, "Uber");
        assert_eq!(rs[1].uber, 1);
        assert_eq!(rs.last().unwrap().function, "Class Specific Uber");
    }
}
