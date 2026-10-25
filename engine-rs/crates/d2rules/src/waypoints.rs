//! Waypoints (W05) + persistance (W19) — table réelle v1.12, activation au
//! contact, voyage direct. Blocs `impl World` ADDITIFS (zéro conflit attendu
//! à l'intégration).
//!
//! # Données réelles (preuves)
//!
//! * **Liste des waypoints par niveau** : colonne `Waypoint` de
//!   `assets/patch_levels.txt` (v1.12, extraite de `engine/patch_d2.mpq` →
//!   `data\global\excel\levels.txt`) — 40 records flaggés, 39 slots uniques
//!   (le slot 17 est porté deux fois : Canyon of the Magi lvl 46 Acte II ET
//!   Pandemonium Run 2 lvl 134 Acte V, duplicata retail connu — cf.
//!   `docs/parity/finish-monde.md` §2). `Waypoint = 255` = pas de waypoint.
//!   Les libellés sont les `LevelName` de la même table (EN canonique ; les
//!   libellés FR vivent dans les TBL, absentes du port — W18).
//! * **Position du waypoint du Camp des Rousses (lvl 1)** : section OBJECTS
//!   de `assets/map/TownN1.ds1` (v18) — l'objet waypoint y est le seul objet
//!   de type 2 non décoratif du camp, à (144, 51) sous-tuiles DS1 (l'échelle
//!   sous-tuile DS1 = 1/5 de tuile = l'unité de grille d2rules ;
//!   `tools/ds1_objects.py`, réf. layout `OpenDiablo2/d2ds1/ds1.go`).
//!   Anchor = centre de la cellule (144.5, 51.5).
//! * **Position du waypoint de la Blood Moor (lvl 2)** : le DRLG seedé pose
//!   le preset réel `waypoint.ds1` (7×7, extrait de d2exp.mpq) au chunk
//!   (3,3) tuiles = (27,27) (`drlg.rs` « le waypoint ! », identique au proto
//!   `tools/bloodmoor_build.py:250` et à la note `warp` du manifest) —
//!   centre du chunk = tuile (30,30) = cellule (152.5, 152.5).
//!
//! # MVP assumé (dit clairement, cf. brief W05)
//!
//! La table retail NE mappe PAS la Blood Moor à un slot du panneau
//! (`Waypoint = 255` pour lvl 2) — pourtant les données du niveau portent
//! bien la pierre (preset `waypoint.ds1` posé par le DRLG, ci-dessus). Pour
//! rendre l'arc jouable du port (Camp des Rousses ↔ Blood Moor) complet,
//! la Blood Moor reçoit le slot local **39** (hors plage retail 0..38,
//! zéro collision) avec la position réelle du chunk : **extension MVP
//! documentée**, marquée `mvp: true` dans [`WaypointDef`] et exposée au
//! JSON de l'UI. Les 38 autres slots réels n'ont pas de position (niveaux
//! pas encore jouables dans le port) : ils activent/découvrent rien et
//! l'UI les affiche « non découvert ».
//!
//! # État & save (W19)
//!
//! L'état (`WaypointState`) vit dans `World` : le waypoint du niveau
//! courant (attaché par d2game au chargement du niveau) + les slots
//! activés. La sauvegarde ajoute le champ OPTIONNEL `"waypoints"` (slots
//! `u8`) — `#[serde(default)]` : les saves v1/v2 antérieures se chargent
//! (actifs vides), et une save avec champ se charge dans un build antérieur
//! (champ inconnu ignoré par serde). PAS de bump de version (un autre
//! chantier fait évoluer save.rs en v3 en parallèle — champs optionnels
//! seulement, l'intégrateur fusionne).

use super::world::World;
use super::GameEvent;
use serde::{Deserialize, Serialize};

/// Un waypoint du jeu : entrée du panneau retail + position réelle si connue.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WaypointDef {
    /// Slot du panneau retail (colonne `Waypoint` de `patch_levels.txt`,
    /// 0..=38) ; 39 = extension MVP Blood Moor (voir doc du module).
    pub slot: u8,
    /// Id levels.txt du niveau porteur (colonne `Id`).
    pub level_id: u32,
    /// `LevelName` canonique EN de `patch_levels.txt` (TBL FR absentes — W18).
    pub name: &'static str,
    /// Acte (colonne `Act` + 1 : 1..=5).
    pub act: u8,
    /// Position d'activation en cellules de grille (centre), si connue.
    /// `None` = niveau non jouable dans le port MVP (pas d'activation).
    pub pos: Option<(f32, f32)>,
    /// Rayon d'activation au contact (cellules de grille).
    pub radius: f32,
    /// `true` = extension MVP hors table retail (assumée, documentée).
    pub mvp: bool,
}

/// Table complète v1.12 (`patch_levels.txt`, colonne `Waypoint` ≠ 255, triée
/// par slot) + l'extension MVP Blood Moor (slot 39). 40 entrées / 39 slots
/// retail (17 double) + 1 slot MVP.
pub const WAYPOINTS: &[WaypointDef] = &[
    WaypointDef { slot: 0, level_id: 1, name: "Rogue Encampment", act: 1, pos: Some((144.5, 51.5)), radius: 10.0, mvp: false },
    WaypointDef { slot: 1, level_id: 3, name: "Cold Plains", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 2, level_id: 4, name: "Stony Field", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 3, level_id: 5, name: "Dark Wood", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 4, level_id: 6, name: "Black Marsh", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 5, level_id: 27, name: "Outer Cloister", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 6, level_id: 29, name: "Jail Level 1", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 7, level_id: 32, name: "Inner Cloister", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 8, level_id: 35, name: "Catacombs Level 2", act: 1, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 9, level_id: 40, name: "Lut Gholein", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 10, level_id: 48, name: "Sewers Level 2", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 11, level_id: 42, name: "Dry Hills", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 12, level_id: 57, name: "Halls of the Dead Level 2", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 13, level_id: 43, name: "Far Oasis", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 14, level_id: 44, name: "Lost City", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 15, level_id: 52, name: "Palace Cellar Level 1", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 16, level_id: 74, name: "Arcane Sanctuary", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 17, level_id: 46, name: "Canyon of the Magi", act: 2, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 18, level_id: 75, name: "Kurast Docktown", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 19, level_id: 76, name: "Spider Forest", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 20, level_id: 77, name: "Great Marsh", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 21, level_id: 78, name: "Flayer Jungle", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 22, level_id: 79, name: "Lower Kurast", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 23, level_id: 80, name: "Kurast Bazaar", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 24, level_id: 81, name: "Upper Kurast", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 25, level_id: 83, name: "Travincal", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 26, level_id: 101, name: "Durance of Hate Level 2", act: 3, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 27, level_id: 103, name: "The Pandemonium Fortress", act: 4, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 28, level_id: 106, name: "City of the Damned", act: 4, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 29, level_id: 107, name: "River of Flame", act: 4, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 30, level_id: 109, name: "Harrogath", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 31, level_id: 111, name: "Rigid Highlands", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 32, level_id: 112, name: "Arreat Plateau", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 33, level_id: 113, name: "Crystalized Cavern Level 1", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 34, level_id: 115, name: "Crystalized Cavern Level 2", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 35, level_id: 123, name: "Halls of Death's Calling", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 36, level_id: 117, name: "Tundra Wastelands", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 37, level_id: 118, name: "Glacial Caves Level 1", act: 5, pos: None, radius: 10.0, mvp: false },
    WaypointDef { slot: 38, level_id: 129, name: "The Worldstone Keep Level 2", act: 5, pos: None, radius: 10.0, mvp: false },
    // Duplicata retail du slot 17 : Pandemonium Run 2 (Acte V) — même slot
    // panneau que Canyon of the Magi, tel quel dans patch_levels.txt.
    WaypointDef { slot: 17, level_id: 134, name: "Pandemonium Run 2", act: 5, pos: None, radius: 10.0, mvp: false },
    // --- Extension MVP (documentée en tête de module) : la Blood Moor porte
    // la pierre (preset waypoint.ds1 posé en (27,27) par le DRLG) mais la
    // colonne retail la met à 255. Slot local 39, position réelle du chunk.
    WaypointDef { slot: 39, level_id: 2, name: "Blood Moor", act: 1, pos: Some((152.5, 152.5)), radius: 12.0, mvp: true },
];

/// Le waypoint d'un niveau (par id levels.txt) — plusieurs slots réels
/// partagent leur `level_id`? Non : un seul par niveau (au gap du double
/// slot 17, deux NIVEAUX distincts). Premier match = suffisant.
pub fn for_level(level_id: u32) -> Option<&'static WaypointDef> {
    WAYPOINTS.iter().find(|w| w.level_id == level_id && w.pos.is_some())
}

/// Tous les waypoints d'un niveau (slots retail et MVP confondus).
pub fn all_for_level(level_id: u32) -> Vec<&'static WaypointDef> {
    WAYPOINTS.iter().filter(|w| w.level_id == level_id).collect()
}

/// Waypoint par slot de panneau (premier match — le double 17 retail donne
/// Canyon of the Magi ; Pandemonium Run 2 reste listé dans [`WAYPOINTS`]).
pub fn by_slot(slot: u8) -> Option<&'static WaypointDef> {
    WAYPOINTS.iter().find(|w| w.slot == slot)
}

/// État waypoints du `World` (W19 : sauvegardé ; voir `save.rs`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct WaypointState {
    /// Waypoint du niveau courant : `(slot, x, y, rayon)` en cellules —
    /// attaché par d2game au chargement du niveau, jamais sauvegardé
    /// (dérivé du niveau chargé).
    pub here: Option<(u8, f32, f32, f32)>,
    /// Slots activés (découverts), triés — champ save `"waypoints"`.
    pub active: Vec<u8>,
}

impl World {
    // ------------------------------------------------------------------
    // Waypoints W05 — attach, contact, activation, voyage (additif)
    // ------------------------------------------------------------------

    /// Attache le waypoint du niveau chargé (appelé par d2game au chargement
    /// d'un niveau dont la table porte une position réelle). Sans effet si
    /// le niveau n'a pas de waypoint positionné.
    pub fn set_level_waypoint(&mut self, slot: u8, x: f32, y: f32, radius: f32) {
        self.wp.here = Some((slot, x, y, radius));
    }

    /// Slot attaché au niveau courant, s'il existe (debug/E2E).
    pub fn level_waypoint(&self) -> Option<(u8, f32, f32)> {
        self.wp.here.map(|(s, x, y, _)| (s, x, y))
    }

    /// Slots activés (découverts), triés — miroir du champ save.
    pub fn active_waypoints(&self) -> &[u8] {
        &self.wp.active
    }

    /// Slot activé ?
    pub fn waypoint_activated(&self, slot: u8) -> bool {
        self.wp.active.contains(&slot)
    }

    /// Active un slot (découverte directe — tests / DRLG futur). Renvoie
    /// `true` si le slot est NOUVEAU (idempotent).
    pub fn activate_waypoint(&mut self, slot: u8) -> bool {
        if self.wp.active.contains(&slot) {
            return false;
        }
        self.wp.active.push(slot);
        self.wp.active.sort_unstable();
        true
    }

    /// Le voyage par panneau est permis vers un slot ACTIVÉ (règle D2 :
    /// on ne voyage que vers un waypoint déjà découvert).
    pub fn waypoint_travel_allowed(&self, slot: u8) -> bool {
        self.waypoint_activated(slot)
    }


    /// Hook de tick (une ligne dans `World::tick`) : activation au contact
    /// du waypoint du niveau courant. Idempotent : l'événement ne part que
    /// pour une activation NOUVELLE.
    pub(crate) fn update_waypoints(&mut self, ev: &mut Vec<GameEvent>) {
        use super::AnimState;
        let Some((slot, x, y, r)) = self.wp.here else { return };
        // 3b-1 (coop) : N'IMPORTE QUEL héros vivant active le waypoint au
        // contact (le plus proche — quêtes partagées ; solo : identique, le
        // slot 0 est le seul héros). Anim DD/mort du héros visé ⇒ pas
        // d'activation (cohérent avec le reste des inputs du tick).
        let s = self.nearest_living_hero_slot(x, y);
        let Some((hx, hy)) = self.hero_slot_pos(s) else { return };
        if self.hero_slot_ent(s).map(|e| e.anim == AnimState::DD).unwrap_or(true) {
            return;
        }
        let d = (hx - x).hypot(hy - y);
        if d <= r && self.activate_waypoint(slot) {
            ev.push(GameEvent::WaypointActivated { slot });
        }
    }

    /// Intention joueur additive (non branchée au MVP — l'UI passe par le
    /// voyage d2game) : garde-pointe symétrique pour un futur clic-map.
    /// `false` = slot non activé.
    pub fn waypoint_click(&mut self, slot: u8) -> bool {
        self.waypoint_travel_allowed(slot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn world20() -> World {
        // Grille 20×20 toute marchable, manifest minimal (0 monstre).
        let grid = vec![vec![false; 20]; 20];
        World::with_seed(grid, json!({}), 42)
    }

    // --- table réelle -----------------------------------------------------

    #[test]
    fn table_matches_real_levels_txt() {
        // 40 entrées retail (39 slots uniques — le double 17) + 1 MVP = 41.
        assert_eq!(WAYPOINTS.len(), 41);
        assert_eq!(WAYPOINTS.iter().filter(|w| !w.mvp).count(), 40);
        let mut retail_slots: Vec<u8> = WAYPOINTS.iter().filter(|w| !w.mvp).map(|w| w.slot).collect();
        retail_slots.sort_unstable();
        retail_slots.dedup();
        assert_eq!(retail_slots.len(), 39);
        assert_eq!(WAYPOINTS.iter().filter(|w| w.slot == 17).count(), 2);
        // Spots pris directement dans patch_levels.txt ce jour.
        let by_slot = |s: u8| WAYPOINTS.iter().find(|w| w.slot == s && !w.mvp).unwrap();
        assert_eq!(by_slot(0).level_id, 1);
        assert_eq!(by_slot(0).name, "Rogue Encampment");
        assert_eq!(by_slot(0).pos, Some((144.5, 51.5)), "objet waypoint TownN1.ds1 (144,51)");
        assert_eq!((by_slot(8).level_id, by_slot(8).name), (35, "Catacombs Level 2"));
        assert_eq!((by_slot(9).level_id, by_slot(9).name), (40, "Lut Gholein"));
        assert_eq!((by_slot(27).level_id, by_slot(27).name), (103, "The Pandemonium Fortress"));
        assert_eq!((by_slot(30).level_id, by_slot(30).name), (109, "Harrogath"));
        assert_eq!((by_slot(38).level_id, by_slot(38).name), (129, "The Worldstone Keep Level 2"));
        // Slots retail 0..=38 tous portés (le 17 deux fois).
        for s in 0..=38u8 {
            assert!(WAYPOINTS.iter().any(|w| w.slot == s && !w.mvp), "slot {s} manquant");
        }
        // Extension MVP Blood Moor : hors plage retail, position réelle.
        let bm = WAYPOINTS.iter().find(|w| w.slot == 39).unwrap();
        assert!(bm.mvp && bm.level_id == 2 && bm.pos.is_some());
        assert_eq!(bm.pos, Some((152.5, 152.5)));
        // La Blood Moor est bien 255 côté retail (pas de double entrée).
        assert!(!WAYPOINTS.iter().any(|w| w.level_id == 2 && !w.mvp));
    }

    #[test]
    fn for_level_resolves_town_and_bloodmoor() {
        assert_eq!(for_level(1).map(|w| w.slot), Some(0));
        assert_eq!(for_level(2).map(|w| (w.slot, w.mvp)), Some((39, true)));
        assert!(for_level(999).is_none(), "niveau sans waypoint positionné");
        assert_eq!(all_for_level(46).len(), 1);
    }

    // --- activation au contact ---------------------------------------------

    #[test]
    fn activation_on_contact_emits_once() {
        let mut w = world20();
        w.set_level_waypoint(0, 5.5, 5.5, 3.0);
        w.players[0].ent.x = 8.0; // à 2.5 cellules du waypoint (rayon 3)
        w.players[0].ent.y = 5.5;
        let ev = w.tick(100, None);
        assert!(ev.contains(&GameEvent::WaypointActivated { slot: 0 }), "event attendu : {ev:?}");
        assert!(w.waypoint_activated(0));
        // Deuxième tick au même endroit : PAS de doublon.
        let ev2 = w.tick(100, None);
        assert!(!ev2.contains(&GameEvent::WaypointActivated { slot: 0 }));
        assert_eq!(w.active_waypoints(), &[0]);
    }

    #[test]
    fn no_activation_outside_radius_or_without_attached_wp() {
        let mut w = world20();
        w.set_level_waypoint(0, 5.5, 5.5, 3.0);
        w.players[0].ent.x = 10.0; // loin (> rayon 3)
        w.players[0].ent.y = 10.0;
        assert!(!w.tick(100, None).iter().any(|e| matches!(e, GameEvent::WaypointActivated { .. })));
        assert!(w.active_waypoints().is_empty());
        // Sans waypoint attaché : rien, même sur place.
        let mut w2 = world20();
        assert!(!w2.tick(100, None).iter().any(|e| matches!(e, GameEvent::WaypointActivated { .. })));
    }

    #[test]
    fn activation_covers_both_playable_levels() {
        // Town (slot 0) et Blood Moor (slot 39) s'activent indépendamment.
        let mut w = world20();
        w.set_level_waypoint(39, 10.5, 10.5, 4.0);
        w.players[0].ent.x = 11.0;
        w.players[0].ent.y = 11.0;
        w.tick(100, None);
        assert!(w.waypoint_activated(39));
        w.set_level_waypoint(0, 2.5, 2.5, 4.0); // changement de niveau : ré-attach
        w.players[0].ent.x = 3.0;
        w.players[0].ent.y = 3.0;
        w.tick(100, None);
        assert_eq!(w.active_waypoints(), &[0, 39]);
    }

    // --- voyage -------------------------------------------------------------

    #[test]
    fn travel_requires_activation() {
        let mut w = world20();
        assert!(!w.waypoint_travel_allowed(0));
        assert!(w.activate_waypoint(0));
        assert!(w.waypoint_travel_allowed(0));
        assert!(!w.waypoint_travel_allowed(39), "slot distinct non activé");
        assert!(w.waypoint_click(0) && !w.waypoint_click(17));
    }

    // --- persistance (W19) ---------------------------------------------------

    #[test]
    fn save_load_round_trip_preserves_active_slots() {
        let mut w = world20();
        w.set_level_waypoint(0, 5.5, 5.5, 3.0);
        w.players[0].ent.x = 5.5;
        w.players[0].ent.y = 5.5;
        w.tick(50, None);
        w.activate_waypoint(39);
        let json = w.save();
        let w2 = World::load(&json).expect("reload");
        assert_eq!(w2.active_waypoints(), &[0, 39]);
        // Le champ save porte bien les slots.
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["waypoints"], json!([0, 39]));
        // here n'est PAS persisté (dérivé du niveau chargé).
        assert!(w2.level_waypoint().is_none());
    }

    #[test]
    fn retrocompat_save_without_waypoints_field() {
        // Une save antérieure (v1/v2 sans "waypoints") se charge : actifs vides.
        let mut w = world20();
        w.activate_waypoint(0);
        let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
        v.as_object_mut().unwrap().remove("waypoints");
        let w2 = World::load(&v.to_string()).expect("save sans champ waypoints");
        assert!(w2.active_waypoints().is_empty());
        // Et une save AVEC le champ se recharge dans les deux versions.
        let w3 = World::load(&w.save()).unwrap();
        assert_eq!(w3.active_waypoints(), &[0]);
    }
}
