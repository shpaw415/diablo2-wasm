//! Texte fr-FR in-engine (W18) — couche noms affichables adossée aux tables
//! TBL v1.12 réelles (`d2rules::tbl`, embarquées `include_bytes!`).
//!
//! Tout est ADDITIF, même pattern que `npc_bridge.rs` / `waypoint_travel.rs` :
//! un `impl GameCore` séparé + des appels aux points d'affichage de `game.rs`
//! (bannière/zone `LevelSwitchInfo` + `levelInfo()`, survol monstre
//! `monsterInfo()`). La page `demo/game.js` ne change pas : elle reçoit déjà
//! `name` par JSON.
//!
//! # Règles de résolution (repli documenté, testé)
//! * **Niveaux** — clé TBL = colonne `LevelName` de `patch_levels.txt`
//!   (levels.txt final v1.12, ex. id 1 → `Rogue Encampment` → « Le Camp des
//!   Rogues ») ; source prioritaire : tables de niveaux branchées
//!   (`GameCore::level_tables`, W01), repli embarqué [`LEVEL_NAME_KEYS`]
//!   (extrait v1.12 — la démo ne fournit pas `patch_levels.txt`) ; si la clé
//!   manque dans les TBL → **repli manifest** (`nameDisplay`, comportement
//!   historique).
//! * **Monstres** — clé TBL = `NameStr` de monstats = champ `name` des blocs
//!   `monsters`/`bosses` du manifest (ex. `Fallen` → « Déchu ») ; les noms de
//!   super-uniques/boss (`mon_names`, W10/W11) passent par la même table ;
//!   clé absente → **repli sur le nom résolu d'avant** (manifest, code).
//! * Priorité TBL : patchstring > expansionstring > string (retail).
//! * Pas de port d2font : le rendu reste le repli DOM `fillText`
//!   (décision EXP-T1/T2).

use std::sync::OnceLock;

use crate::game::{GameCore, LevelDef};
use d2rules::tbl::TblSet;

/// Clés LevelName embarquées (repli quand les tables de niveaux ne sont pas
/// fournies) — extraites de `assets/patch_levels.txt` v1.12 (colonne
/// `LevelName`, 136 lignes à Id+LevelName non vides). Sert d'index
/// levelsId → clé TBL pour TOUS les niveaux des 5 actes + extras.
pub(crate) static LEVEL_NAME_KEYS: &[(u32, &str)] = &[
    (1, "Rogue Encampment"),
    (2, "Blood Moor"),
    (3, "Cold Plains"),
    (4, "Stony Field"),
    (5, "Dark Wood"),
    (6, "Black Marsh"),
    (7, "Tamoe Highland"),
    (8, "Den of Evil"),
    (9, "Cave Level 1"),
    (10, "Underground Passage Level 1"),
    (11, "Hole Level 1"),
    (12, "Pit Level 1"),
    (13, "Cave Level 2"),
    (14, "Underground Passage Level 2"),
    (15, "Hole Level 2"),
    (16, "Pit Level 2"),
    (17, "Burial Grounds"),
    (18, "Crypt"),
    (19, "Mausoleum"),
    (20, "Forgotten Tower"),
    (21, "Tower Cellar Level 1"),
    (22, "Tower Cellar Level 2"),
    (23, "Tower Cellar Level 3"),
    (24, "Tower Cellar Level 4"),
    (25, "Tower Cellar Level 5"),
    (26, "Monastery Gate"),
    (27, "Outer Cloister"),
    (28, "Barracks"),
    (29, "Jail Level 1"),
    (30, "Jail Level 2"),
    (31, "Jail Level 3"),
    (32, "Inner Cloister"),
    (33, "Cathedral"),
    (34, "Catacombs Level 1"),
    (35, "Catacombs Level 2"),
    (36, "Catacombs Level 3"),
    (37, "Catacombs Level 4"),
    (38, "Tristram"),
    (39, "Moo Moo Farm"),
    (40, "Lut Gholein"),
    (41, "Rocky Waste"),
    (42, "Dry Hills"),
    (43, "Far Oasis"),
    (44, "Lost City"),
    (45, "Valley of Snakes"),
    (46, "Canyon of the Magi"),
    (47, "Sewers Level 1"),
    (48, "Sewers Level 2"),
    (49, "Sewers Level 3"),
    (50, "Harem Level 1"),
    (51, "Harem Level 2"),
    (52, "Palace Cellar Level 1"),
    (53, "Palace Cellar Level 2"),
    (54, "Palace Cellar Level 3"),
    (55, "Stony Tomb Level 1"),
    (56, "Halls of the Dead Level 1"),
    (57, "Halls of the Dead Level 2"),
    (58, "Claw Viper Temple Level 1"),
    (59, "Stony Tomb Level 2"),
    (60, "Halls of the Dead Level 3"),
    (61, "Claw Viper Temple Level 2"),
    (62, "Maggot Lair Level 1"),
    (63, "Maggot Lair Level 2"),
    (64, "Maggot Lair Level 3"),
    (65, "Ancient Tunnels"),
    (66, "Tal Rasha's Tomb"),
    (67, "Tal Rasha's Tomb"),
    (68, "Tal Rasha's Tomb"),
    (69, "Tal Rasha's Tomb"),
    (70, "Tal Rasha's Tomb"),
    (71, "Tal Rasha's Tomb"),
    (72, "Tal Rasha's Tomb"),
    (73, "Duriel's Lair"),
    (74, "Arcane Sanctuary"),
    (75, "Kurast Docktown"),
    (76, "Spider Forest"),
    (77, "Great Marsh"),
    (78, "Flayer Jungle"),
    (79, "Lower Kurast"),
    (80, "Kurast Bazaar"),
    (81, "Upper Kurast"),
    (82, "Kurast Causeway"),
    (83, "Travincal"),
    (84, "Spider Cave"),
    (85, "Spider Cavern"),
    (86, "Swampy Pit Level 1"),
    (87, "Swampy Pit Level 2"),
    (88, "Flayer Dungeon Level 1"),
    (89, "Flayer Dungeon Level 2"),
    (90, "Swampy Pit Level 3"),
    (91, "Flayer Dungeon Level 3"),
    (92, "Sewers Level 1"),
    (93, "Sewers Level 2"),
    (94, "Ruined Temple"),
    (95, "Disused Fane"),
    (96, "Forgotten Reliquary"),
    (97, "Forgotten Temple"),
    (98, "Ruined Fane"),
    (99, "Disused Reliquary"),
    (100, "Durance of Hate Level 1"),
    (101, "Durance of Hate Level 2"),
    (102, "Durance of Hate Level 3"),
    (103, "The Pandemonium Fortress"),
    (104, "Outer Steppes"),
    (105, "Plains of Despair"),
    (106, "City of the Damned"),
    (107, "River of Flame"),
    (108, "Chaos Sanctum"),
    (109, "Harrogath"),
    (110, "Bloody Foothills"),
    (111, "Rigid Highlands"),
    (112, "Arreat Plateau"),
    (113, "Crystalized Cavern Level 1"),
    (114, "Cellar of Pity"),
    (115, "Crystalized Cavern Level 2"),
    (116, "Echo Chamber"),
    (117, "Tundra Wastelands"),
    (118, "Glacial Caves Level 1"),
    (119, "Glacial Caves Level 2"),
    (120, "Rocky Summit"),
    (121, "Nihlathaks Temple"),
    (122, "Halls of Anguish"),
    (123, "Halls of Death's Calling"),
    (124, "Halls of Vaught"),
    (125, "Hell1"),
    (126, "Hell2"),
    (127, "Hell3"),
    (128, "The Worldstone Keep Level 1"),
    (129, "The Worldstone Keep Level 2"),
    (130, "The Worldstone Keep Level 3"),
    (131, "Throne of Destruction"),
    (132, "The Worldstone Chamber"),
    (133, "Pandemonium Run 1"),
    (134, "Pandemonium Run 2"),
    (135, "Pandemonium Run 3"),
    (136, "Tristram"),
];

/// Clé TBL d'un niveau par son id levels.txt (repli embarqué [`LEVEL_NAME_KEYS`]).
pub(crate) fn level_name_key(levels_id: u32) -> Option<&'static str> {
    LEVEL_NAME_KEYS
        .iter()
        .find(|(id, _)| *id == levels_id)
        .map(|(_, key)| *key)
}

/// Accès global aux TBL fr embarquées (parse ~670 Ko une seule fois).
pub(crate) fn global() -> &'static TblSet {
    static TBL: OnceLock<TblSet> = OnceLock::new();
    TBL.get_or_init(TblSet::bundled_fra)
}

/// Résout une clé TBL avec repli : `patchstring > expansionstring > string`,
/// `fallback` si la clé manque aux trois (comportement historique).
pub(crate) fn translate(key: &str, fallback: &str) -> String {
    global().get(key).unwrap_or_else(|| fallback.to_string())
}

impl GameCore {
    /// Nom affichable fr du niveau `def` : clé `LevelName` (tables branchées
    /// W01, sinon repli embarqué) → TBL → repli manifest `nameDisplay`.
    pub(crate) fn level_display_name(&self, def: &LevelDef) -> String {
        let fallback = def.name.as_str();
        let Some(id) = def.levels_id else {
            return fallback.to_string();
        };
        let key = self
            .level_tables()
            .and_then(|t| t.get(id))
            .map(|r| if r.level_name.is_empty() { r.name.as_str() } else { r.level_name.as_str() })
            .or_else(|| level_name_key(id));
        match key {
            Some(k) => translate(k, fallback),
            None => fallback.to_string(),
        }
    }

    /// Nom affichable fr du monstre `index` : traduit le nom résolu par la
    /// chaîne historique (`monster_display_name` : super-unique/boss, puis
    /// manifest `monsters`/`bosses`, puis code brut) via la TBL. Clé absente
    /// → le nom d'origine (repli documenté, testé).
    pub(crate) fn monster_display_name_tbl(&self, index: usize) -> String {
        let base = self.monster_display_name(index);
        translate(&base, &base)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameCore;
    use crate::levels;

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");
    const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");

    fn core(level: &str) -> GameCore {
        GameCore::with_level(MANIFEST, "BA", level).expect("GameCore de test")
    }

    /// Niveau de boot résolu avant finalize (le HUD affiche déjà ce nom).
    #[test]
    fn town_level_name_is_french_from_tbl() {
        let g = core("town");
        let def = g.resolve_level_def("town").expect("town résolu");
        assert_eq!(def.levels_id, Some(1));
        let name = g.level_display_name(&def);
        assert_eq!(name, "Le Camp des Rogues", "bannière de zone fr (W18)");
    }

    #[test]
    fn bloodmoor_level_name_is_french_from_tbl() {
        let g = core("bloodmoor");
        let def = g.resolve_level_def("bloodmoor").expect("bloodmoor résolu");
        assert_eq!(def.levels_id, Some(2));
        assert_eq!(g.level_display_name(&def), "La Lande sanglante");
    }

    /// Niveau sans id levels.txt (aucun du manifest actuel) ou clé absente
    /// des TBL → repli manifest nameDisplay (documenté).
    #[test]
    fn fallback_to_manifest_when_tbl_key_missing() {
        let g = core("town");
        let mut def = g.resolve_level_def("town").unwrap();
        def.levels_id = None; // pas de clé LevelName possible
        let manifest_name = def.name.clone();
        assert_eq!(g.level_display_name(&def), manifest_name);
        // Id connu des tables dont la clé LevelName existe bien en fr :
        // 133 = « Pandemonium Run 1 », présente dans patchstring (priorité
        // patchstring > expansionstring > string, preuve tbl-fra/evidence.md
        // §patchstring 1155) → traduit, PAS de repli manifest.
        def.levels_id = Some(133);
        assert_eq!(g.level_display_name(&def), "Repaire de la Matrone");
        // Id hors patch_levels (aucune clé possible) → repli manifest.
        def.levels_id = Some(9999);
        let manifest_name = def.name.clone();
        assert_eq!(g.level_display_name(&def), manifest_name);
    }

    /// Le repli embarqué couvre TOUTES les lignes Id/LevelName de la vraie
    /// patch_levels.txt (garde anti-dérive de l'extrait).
    #[test]
    fn embedded_level_keys_cover_patch_levels_txt() {
        let tables = levels::LevelTables::parse(PATCH_LEVELS).expect("patch_levels.txt parse");
        assert!(tables.len() >= 100);
        let mut checked = 0;
        for rec in tables.iter() {
            if rec.level_name.is_empty() {
                continue;
            }
            assert_eq!(
                level_name_key(rec.id),
                Some(rec.level_name.as_str()),
                "extrait LevelName désynchronisé à l'id {}",
                rec.id
            );
            checked += 1;
        }
        assert_eq!(checked, LEVEL_NAME_KEYS.len(), "toutes les clés couvertes");
        // Et ces clés résolvent réellement dans les TBL fr (couverture).
        let mut translated = 0;
        for (_, key) in LEVEL_NAME_KEYS.iter() {
            if global().get(key).is_some() {
                translated += 1;
            }
        }
        assert!(translated >= 100, "{translated}/{} clés LevelName dans les TBL fr", LEVEL_NAME_KEYS.len());
    }

    /// Noms de monstres : le survol (`monsterInfo`) sert le fr de la TBL,
    /// repli manifest. Test de la chaîne de traduction bas niveau (le monde
    /// complet est couvert par les tests GameCore existants).
    #[test]
    fn monster_names_are_french_from_tbl() {
        let g = core("bloodmoor");
        assert_eq!(translate("Fallen", "Fallen"), "Déchu");
        assert_eq!(translate("QuillRat", "QuillRat"), "Rat-épic");
        assert_eq!(translate("Andariel", "Andariel"), "Andarielle");
        // Clé absente → repli sur l'entrée d'origine (jamais de vide).
        assert_eq!(translate("ZKZKR", "ZKZKR"), "ZKZKR");
        // Sans monde (avant finalize), la fiche monstre JSON est null.
        assert_eq!(g.monster_display_name_tbl(0), "?");
    }
}
