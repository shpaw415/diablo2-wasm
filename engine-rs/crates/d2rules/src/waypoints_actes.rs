//! Waypoints des actes II-V (8c, campagne d2w-actes) — extension du socle
//! W05/W19 ([`crate::waypoints`] + `d2game::waypoint_travel`) aux **38 slots
//! retail ids 1-38** : mapping niveaux lu dans la colonne `Waypoint` de
//! `patch_levels.txt`, attach de niveaux réels (pas seulement les 2 posés
//! MVP), voyage cross-acte, persistance additive PAR DIFFICULTÉ.
//!
//! # Données réelles (preuves)
//!
//! * `assets/patch_levels.txt` (v1.12) : **40 records flaggés** colonne
//!   `Waypoint` ≠ 255 — slots 0 (Rogue Encampment) à 38 (Worldstone Keep
//!   Level 2), le slot 17 porté deux fois (Canyon of the Magi lvl 46 Acte II
//!   ET Pandemonium Run 2 lvl 134 Acte V, duplicata retail connu). Ex.
//!   slot 9 = **Lut Gholein** (lvl 40, Acte II). `255`/vide = pas de
//!   waypoint (la Blood Moor lvl 2 est à 255 : elle n'existe ici que via le
//!   slot MVP 39 du socle W05, hors périmètre retail de ce module).
//! * Le mapping parsé est vérifié record-à-record contre la table statique
//!   [`crate::waypoints::WAYPOINTS`] (celle que l'UI d2game liste déjà) —
//!   golden test plus bas : mêmes (slot, level_id, acte, nom).
//!
//! # Règles (retail)
//!
//! * **Activation** : au contact du waypoint du niveau (déjà codée W05 sur
//!   `World::wp.here` — ce module l'ouvre aux 38 slots via
//!   [`World::attach_level_waypoint_table`] : le DRLG d'un niveau généré
//!   fournit la position, le slot vient de la table).
//! * **Voyage** : vers tout slot ACTIVÉ, sans contrainte d'adjacence ni
//!   d'acte (A1 → A5 permis, règle D2 du panneau) — même difficulté.
//! * **Difficultés** : Normal/Nightmare/Hell portent des ensembles
//!   d'activation DISTINCTS (repartir de zéro par difficulté) —
//!   [`ActsWaypointState`], sérialisation additive `#[serde(default)]`
//!   (charge les saves sans champ, et une save avec champ est ignorée par
//!   les builds antérieurs ; même contrat que le champ `"waypoints"` W19).
//!   Le champ `"waypoints"` existant (ensemble unique partagé) reste la
//!   persistance du moteur ; l'état par difficulté est un contrat additive
//!   pour l'intégration 8c.

use crate::waypoints::{WaypointDef, WAYPOINTS};
use crate::Difficulty;
use serde::{Deserialize, Serialize};

/// Un slot retail résolu depuis la colonne `Waypoint` (8c-2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActWaypointSlot {
    /// Id du panneau (valeur brute de la colonne, 0..=38).
    pub slot: u8,
    /// Id levels.txt du niveau porteur (colonne `Id`).
    pub level_id: u32,
    /// `LevelName` de la table (trimmé, ex. `Lut Gholein`).
    pub name: String,
    /// Acte retail 1..=5 (colonne `Act` + 1).
    pub act: u8,
}

/// Parse les slots waypoints d'une table levels (`patch_levels.txt`) :
/// un record par ligne dont `Id` est entier > 0 et `Waypoint` vaut 0..=38
/// (255/vide = pas de waypoint). Ordre du fichier ; le slot 17 apparaît
/// deux fois (46 et 134) — fidèle à la table.
pub fn parse_waypoint_slots(tsv: &str) -> Vec<ActWaypointSlot> {
    let Ok(t) = crate::tsv::Tsv::parse(tsv) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for i in 0..t.len() {
        let Some(r) = t.rec(i) else { continue };
        let Ok(id) = r.s("Id").parse::<u32>() else { continue };
        if id == 0 {
            continue; // record Null
        }
        let Ok(wp) = r.s("Waypoint").parse::<i32>() else { continue };
        if !(0..=38).contains(&wp) {
            continue; // 255 / vide / hors plage
        }
        out.push(ActWaypointSlot {
            slot: wp as u8,
            level_id: id,
            name: r.s("LevelName").trim().to_string(),
            act: (r.u("Act") + 1).min(255) as u8,
        });
    }
    out
}

/// Slot retail d'un niveau (8c-2) — TOUT niveau flaggé, même sans position
/// connue (contrairement à [`crate::waypoints::for_level`] qui exige un
/// `pos`). Extensions MVP hors retail (slot 39 Blood Moor) exclues : la
/// colonne `Waypoint` du lvl 2 vaut 255. Le duplicata 17 (lvl 134) résout
/// vers 17 comme le premier match retail.
pub fn slot_for_level(level_id: u32) -> Option<u8> {
    retail().find(|w| w.level_id == level_id).map(|w| w.slot)
}

/// Niveaux porteurs d'un slot (1 ou 2 — le duplicata retail du 17).
pub fn levels_for_slot(slot: u8) -> Vec<u32> {
    retail().filter(|w| w.slot == slot).map(|w| w.level_id).collect()
}

fn retail() -> impl Iterator<Item = &'static WaypointDef> {
    WAYPOINTS.iter().filter(|w| !w.mvp)
}

/// État waypoints PAR DIFFICULTÉ (8c-2) : un ensemble de slots activés par
/// difficulté retail (`index()` 0=Normal, 1=Nightmare, 2=Hell). Additif :
/// `#[serde(default)]` — une save sans champ charge à vide, une save avec
/// champ charge tel quel.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActsWaypointState {
    /// Slots activés, par difficulté, triés.
    #[serde(default)]
    active: [Vec<u8>; 3],
}

impl ActsWaypointState {
    /// Active un slot pour la difficulté (découverte). `true` si NOUVEAU.
    pub fn activate(&mut self, d: Difficulty, slot: u8) -> bool {
        let set = &mut self.active[d.index()];
        if set.contains(&slot) {
            return false;
        }
        set.push(slot);
        set.sort_unstable();
        true
    }

    /// Slot activé pour la difficulté ?
    pub fn is_active(&self, d: Difficulty, slot: u8) -> bool {
        self.active[d.index()].contains(&slot)
    }

    /// Voyage (panneau) permis vers `slot` : activé, même difficulté.
    /// Aucune contrainte d'acte (règle D2).
    pub fn travel_allowed(&self, d: Difficulty, slot: u8) -> bool {
        self.is_active(d, slot)
    }

    /// Aller `from` → `to` : les DEUX slots activés (on part d'un waypoint
    /// debout) et distincts.
    pub fn can_travel_between(&self, d: Difficulty, from: u8, to: u8) -> bool {
        from != to && self.is_active(d, from) && self.is_active(d, to)
    }

    /// Slots activés d'une difficulté (triés).
    pub fn active_in(&self, d: Difficulty) -> &[u8] {
        &self.active[d.index()]
    }

    /// Sauvegarde JSON (champ additif, `#[serde(default)]` au chargement).
    pub fn save(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".into())
    }

    /// Chargement JSON — accepte une save sans le champ (rétro-compat).
    pub fn load(json: &str) -> Result<ActsWaypointState, String> {
        serde_json::from_str(json).map_err(|e| format!("waypoints actes : save illisible ({e})"))
    }
}

impl crate::World {
    /// Attache le waypoint du niveau depuis la TABLE RÉELLE (8c-2) : slot
    /// résolu pour tout niveau flaggé (`Waypoint` ≠ 255), y compris les 38
    /// sans position statique — le DRLG/preset du niveau généré fournit
    /// `(x, y)` (cellules de grille), l'activation au contact et le voyage
    /// W05 s'appliquent ensuite tels quels. `false` = niveau sans waypoint
    /// retail (ex. Blood Moor lvl 2 : attach MVP via le chemin existant).
    pub fn attach_level_waypoint_table(&mut self, level_id: u32, x: f32, y: f32) -> bool {
        let Some(def) = retail().find(|w| w.level_id == level_id) else {
            return false;
        };
        self.set_level_waypoint(def.slot, x, y, def.radius);
        true
    }

    /// Le voyage vers le waypoint de ce niveau est-il permis (slot activé) ?
    /// `false` pour un niveau sans waypoint retail ou non activé.
    pub fn travel_allowed_to_level(&self, level_id: u32) -> bool {
        slot_for_level(level_id)
            .map(|slot| self.waypoint_travel_allowed(slot))
            .unwrap_or(false)
    }
}

// ---------------------------------------------------------------------------
// Tests — table réelle, cohérence statique, World 38 slots, allers-retours
// par difficulté
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::waypoints::WAYPOINTS;
    use serde_json::json;

    const PATCH_LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");

    fn world20() -> crate::World {
        let grid = vec![vec![false; 20]; 20];
        crate::World::with_seed(grid, json!({}), 42)
    }

    // --- table réelle (8c-2 : mapping depuis la colonne Waypoint) ----------

    #[test]
    fn parse_real_table_40_flagged_records_slots_0_to_38() {
        let slots = parse_waypoint_slots(PATCH_LEVELS);
        assert_eq!(slots.len(), 40, "40 records flaggés (39 slots uniques, 17 double)");
        // Slots 0..=38 tous portés — le 17 deux fois (46 + 134), les autres une fois.
        for s in 0..=38u8 {
            let n = slots.iter().filter(|r| r.slot == s).count();
            let expected = if s == 17 { 2 } else { 1 };
            assert_eq!(n, expected, "slot {s} : {n} records");
        }
        assert_eq!(slots.iter().filter(|r| r.slot == 17).count(), 2);
        // Spots pris à la main dans la table (colonne Waypoint) :
        let by_slot = |s: u8| slots.iter().find(|r| r.slot == s).unwrap();
        assert_eq!((by_slot(0).level_id, by_slot(0).name.as_str(), by_slot(0).act), (1, "Rogue Encampment", 1));
        assert_eq!((by_slot(9).level_id, by_slot(9).name.as_str(), by_slot(9).act), (40, "Lut Gholein", 2));
        assert_eq!((by_slot(27).level_id, by_slot(27).act), (103, 4));
        assert_eq!((by_slot(30).level_id, by_slot(30).act), (109, 5));
        assert_eq!((by_slot(38).level_id, by_slot(38).name.as_str()), (129, "The Worldstone Keep Level 2"));
        // Le double 17 : Canyon of the Magi (A2) + Pandemonium Run 2 (A5).
        let mut lv17: Vec<u32> = slots.iter().filter(|r| r.slot == 17).map(|r| r.level_id).collect();
        lv17.sort_unstable();
        assert_eq!(lv17, vec![46, 134]);
        // Actes par plages retail : 0-8 A1, 9-17 A2, 18-26 A3, 27-29 A4,
        // 30-38 A5 — SAUF le duplicata 17 (Pandemonium Run 2 lvl 134, A5).
        for r in &slots {
            if r.slot == 17 && r.level_id == 134 {
                assert_eq!(r.act, 5, "duplicata 17 : Pandemonium Run 2 en A5");
                continue;
            }
            let expected = match r.slot { 0..=8 => 1, 9..=17 => 2, 18..=26 => 3, 27..=29 => 4, _ => 5 };
            assert_eq!(r.act, expected, "slot {}", r.slot);
        }
        // La Blood Moor (2) est bien à 255 côté retail : absente du parse.
        assert!(!slots.iter().any(|r| r.level_id == 2), "lvl 2 : Waypoint = 255");
        // Garde-fous : table vide / sans en-tête.
        assert!(parse_waypoint_slots("").is_empty());
    }

    #[test]
    fn parsed_mapping_matches_static_waypoints_record_by_record() {
        // L'UI d2game liste déjà les slots depuis WAYPOINTS (W05) — le parse
        // 8c doit coller record-à-record (slot, level_id, acte, nom).
        let slots = parse_waypoint_slots(PATCH_LEVELS);
        let static_retail: Vec<&WaypointDef> = WAYPOINTS.iter().filter(|w| !w.mvp).collect();
        assert_eq!(slots.len(), static_retail.len());
        for s in &slots {
            let m = static_retail
                .iter()
                .find(|w| w.slot == s.slot && w.level_id == s.level_id)
                .unwrap_or_else(|| panic!("slot {} lvl {} absent de WAYPOINTS", s.slot, s.level_id));
            assert_eq!(s.act, m.act, "slot {} : acte", s.slot);
            assert_eq!(s.name, m.name.trim(), "slot {} : nom", s.slot);
        }
        // Et réciproquement (pas de doublon statique en plus).
        for w in &static_retail {
            assert!(
                slots.iter().any(|s| s.slot == w.slot && s.level_id == w.level_id),
                "WAYPOINTS slot {} lvl {} absent du parse",
                w.slot,
                w.level_id
            );
        }
    }

    #[test]
    fn slot_for_level_covers_all_flagged_levels() {
        assert_eq!(slot_for_level(1), Some(0));
        assert_eq!(slot_for_level(3), Some(1));
        assert_eq!(slot_for_level(40), Some(9), "slot 9 = Lut Gholein");
        assert_eq!(slot_for_level(46), Some(17));
        assert_eq!(slot_for_level(134), Some(17), "duplicata retail");
        assert_eq!(slot_for_level(109), Some(30));
        assert_eq!(slot_for_level(129), Some(38));
        assert_eq!(slot_for_level(2), None, "Blood Moor : 255 retail (MVP 39 hors table)");
        assert_eq!(slot_for_level(999), None);
        assert_eq!(levels_for_slot(17), vec![46, 134]);
        assert_eq!(levels_for_slot(9), vec![40]);
        // Les 38 slots 1-38 + le 0 : 40 niveaux distincts via la table
        // (39 slots — le duplicata 17 porte un 2e niveau).
        let distinct: std::collections::BTreeSet<u32> =
            (0..=38u8).flat_map(levels_for_slot).collect();
        assert_eq!(distinct.len(), 40);
    }

    // --- World : attach des niveaux réels, activation, voyage ---------------

    #[test]
    fn world_attach_real_levels_beyond_the_two_mvp_ones() {
        let mut w = world20();
        // Lut Gholein (40, slot 9) : le DRLG fournit la position, la table
        // fournit le slot — attach OK sans position statique.
        assert!(w.attach_level_waypoint_table(40, 12.5, 8.5));
        assert_eq!(w.level_waypoint(), Some((9, 12.5, 8.5)));
        // Canyon of the Magi (46, slot 17) et Pandemonium Run 2 (134, 17).
        assert!(w.attach_level_waypoint_table(134, 3.5, 3.5));
        assert_eq!(w.level_waypoint().map(|(s, _, _)| s), Some(17));
        // Niveaux sans waypoint retail : refus (Blood Moor passe par le
        // chemin MVP existant, le town est couvert par les deux).
        assert!(!w.attach_level_waypoint_table(2, 1.0, 1.0));
        assert!(!w.attach_level_waypoint_table(999, 1.0, 1.0));
        assert!(!w.travel_allowed_to_level(999), "niveau sans waypoint");
    }

    #[test]
    fn activation_at_contact_then_cross_act_travel_on_real_slots() {
        let mut w = world20();
        // Slot 9 (Lut Gholein, A2) attaché, héros dessus → contact → event.
        assert!(w.attach_level_waypoint_table(40, 10.5, 10.5));
        w.players[0].ent.x = 11.0;
        w.players[0].ent.y = 10.5;
        let ev = w.tick(100, None);
        assert!(ev.contains(&crate::GameEvent::WaypointActivated { slot: 9 }), "{ev:?}");
        assert!(w.waypoint_activated(9));
        // Voyage refusé vers un slot non activé, permis vers un activé —
        // y compris CROSS-ACTE (A2 → A5 Harrogath) : règle du panneau D2.
        assert!(!w.travel_allowed_to_level(109), "Harrogath non activé");
        assert!(w.activate_waypoint(30));
        assert!(w.travel_allowed_to_level(109));
        assert!(w.travel_allowed_to_level(40));
        // Retour A1 : le town (slot 0) activé → permis.
        assert!(!w.travel_allowed_to_level(1));
        w.activate_waypoint(0);
        assert!(w.travel_allowed_to_level(1));
    }

    // --- persistance par difficulté (8c-2 : allers-retours) -----------------

    #[test]
    fn acts_state_round_trip_for_each_difficulty() {
        let diffs = [
            (Difficulty::Normal, "normal"),
            (Difficulty::Nightmare, "nightmare"),
            (Difficulty::Hell, "hell"),
        ];
        for (d, label) in diffs {
            let mut st = ActsWaypointState::default();
            // Activation du town (0, A1) et de Lut Gholein (9, A2) : voyage
            // cross-acte permis dans CETTE difficulté.
            assert!(st.activate(d, 0));
            assert!(!st.activate(d, 0), "idempotent");
            assert!(st.activate(d, 9));
            assert!(st.can_travel_between(d, 0, 9), "{label} : A1 → A2");
            assert!(st.can_travel_between(d, 9, 0), "{label} : A2 → A1");
            assert!(!st.can_travel_between(d, 9, 9), "même slot : non");
            assert!(!st.can_travel_between(d, 0, 30), "Harrogath non activé");
            // Aller-retour save → load : l'état survit, isolé des autres
            // difficultés (règle retail : repartir de zéro en NM/Hell).
            let st2 = ActsWaypointState::load(&st.save()).expect("{label} : reload");
            assert_eq!(st2, st);
            assert!(st2.travel_allowed(d, 9), "{label} : slot 9 persisté");
            assert_eq!(st2.active_in(d), &[0, 9]);
            for (other, ol) in diffs {
                if other != d {
                    assert!(!st2.is_active(other, 9), "{label} : aucune fuite vers {ol}");
                    assert!(!st2.travel_allowed(other, 9));
                    assert!(!st2.can_travel_between(other, 0, 9));
                }
            }
        }
    }

    #[test]
    fn acts_state_save_is_additive_serde_default() {
        let mut st = ActsWaypointState::default();
        st.activate(Difficulty::Hell, 38);
        let v: serde_json::Value = serde_json::from_str(&st.save()).unwrap();
        assert_eq!(v["active"], json!([[], [], [38]]), "un ensemble par difficulté");
        // Save SANS le champ (build antérieur ou champ retiré) : charge à vide.
        let st2 = ActsWaypointState::load("{}").expect("objet vide");
        assert_eq!(st2, ActsWaypointState::default());
        let st3 = ActsWaypointState::load("{\"active\":[[1],[],[]]}").expect("champ partiel");
        assert!(st3.is_active(Difficulty::Normal, 1));
        assert!(!st3.is_active(Difficulty::Hell, 1));
        assert!(ActsWaypointState::load("n'importe quoi").is_err());
    }
}
