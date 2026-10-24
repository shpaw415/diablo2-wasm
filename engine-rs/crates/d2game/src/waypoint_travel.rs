//! Waypoints W05 côté moteur : attach du waypoint du niveau chargé, état
//! JSON pour l'UI panneau, voyage direct (switch_level + régénération seedée
//! du niveau cible, cf. `impl-drlg.md`).
//!
//! Tout est ADDITIF : un `impl GameCore` séparé + deux appels d'attach dans
//! `game.rs` (`load_level` / `load_world`). Les positions viennent de la
//! table réelle `d2rules::waypoints::WAYPOINTS` (patch_levels.txt + DS1,
//! preuves en tête de ce module).

use crate::game::{GameCore, LevelDef};
use crate::world_impl::World;
use serde_json::{json, Value};

/// Attache le waypoint du niveau (table réelle, position en cellules) au
/// monde fraîchement chargé — sans effet si le niveau n'en porte pas
/// (table retail à 255 et pas d'extension MVP). Fonction libre (pas de
/// `&self`) pour s'appeler pendant que `GameCore.world` est mutably
/// emprunté (load_level / load_world).
pub(crate) fn attach_level_waypoint(world: &mut World, def: &LevelDef) {
    let Some(id) = def.levels_id else { return };
    if let Some(wp) = crate::world_impl::waypoint_for_level(id) {
        if let Some((x, y)) = wp.pos {
            world.set_level_waypoint(wp.slot, x, y, wp.radius);
        }
    }
}

impl GameCore {
    /// État waypoints complet pour l'UI (JSON) :
    /// `{"current":{"key","name","levelId","slot","activated"},"list":[
    ///   {"slot","levelId","name","act","mvp","pos":[x,y]|null,
    ///    "activated","here","travel","levelKey"} …]}`
    /// `travel` = activé ET niveau disponible dans le port (bloc manifest).
    pub fn waypoints_json(&self) -> String {
        let current = self.level.as_ref().map(|l| {
            let slot = self
                .world
                .as_ref()
                .and_then(|w| w.level_waypoint())
                .map(|(s, _, _)| s);
            json!({
                "key": l.key,
                "name": l.name,
                "levelId": l.levels_id,
                "slot": slot,
                "activated": slot.and_then(|s| self.world.as_ref().map(|w| w.waypoint_activated(s))).unwrap_or(false),
            })
        });
        let list: Vec<Value> = crate::world_impl::WAYPOINTS
            .iter()
            .map(|wp| {
                let activated = self
                    .world
                    .as_ref()
                    .map(|w| w.waypoint_activated(wp.slot))
                    .unwrap_or(false);
                let here = self
                    .world
                    .as_ref()
                    .and_then(|w| w.level_waypoint())
                    .map(|(s, _, _)| s == wp.slot)
                    .unwrap_or(false);
                let level_key = self
                    .manifest_block_for_level_id(wp.level_id)
                    .map(|(k, _)| k);
                json!({
                    "slot": wp.slot,
                    "levelId": wp.level_id,
                    "name": wp.name,
                    "act": wp.act,
                    "mvp": wp.mvp,
                    "pos": wp.pos.map(|(x, y)| json!([x, y])),
                    "activated": activated,
                    "here": here,
                    "travel": activated && level_key.is_some(),
                    "levelKey": level_key,
                })
            })
            .collect();
        json!({ "current": current, "list": list }).to_string()
    }

    /// Voyage direct vers un waypoint ACTIVÉ (W05) : recharge le niveau
    /// cible (`switch_level` → régénération seedée cf. impl-drlg) et pose le
    /// héros SUR le waypoint d'arrivée (position réelle de la table ; repli
    /// : l'entrée de warp du bloc manifest). Renvoie le JSON de la
    /// transition `{"slot","key","name"}` ou une erreur descriptive.
    pub fn travel_waypoint(&mut self, slot: u8) -> Result<String, String> {
        let wp = crate::world_impl::waypoint_by_slot(slot)
            .ok_or_else(|| format!("waypoint : slot {slot} inconnu"))?;
        let allowed = self
            .world
            .as_ref()
            .map(|w| w.waypoint_travel_allowed(slot))
            .unwrap_or(false);
        if !allowed {
            return Err(format!(
                "waypoint : slot {slot} ({}) non activé — touchez d'abord la pierre",
                wp.name
            ));
        }
        let Some((key, block)) = self.manifest_block_for_level_id(wp.level_id) else {
            return Err(format!(
                "waypoint : {} (levels.txt {}) sans bloc manifest — niveau non jouable dans le port",
                wp.name, wp.level_id
            ));
        };
        // Atterrissage SUR le waypoint cible (règle D2) ; repli : entrée warp.
        let (x, y) = wp
            .pos
            .or_else(|| {
                block
                    .get("warp")
                    .and_then(|w| w.get("entry"))
                    .and_then(|e| e.as_array())
                    .and_then(|a| Some((a.first()?.as_f64()? as f32, a.get(1)?.as_f64()? as f32)))
            })
            .ok_or_else(|| format!("waypoint : {} sans position d'arrivée", wp.name))?;
        let info = self.switch_level(&key, x, y)?;
        Ok(json!({ "slot": slot, "key": info.key, "name": info.name }).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameCore;
    use serde_json::Value;
    use std::path::Path;

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");

    fn ws_root() -> Option<String> {
        let p = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        p.join("assets/manifest.json").is_file().then(|| p.to_string_lossy().into_owned())
    }

    /// Boot complet town (palette + DS1 + DT1 + anims BA/monstres), comme
    /// les tests historiques de `game.rs`.
    fn provide_anim(g: &mut GameCore, root: &str, block: &Value) {
        if let Some(p) = block.get("cof").and_then(|c| c.as_str()) {
            g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
        }
        if let Some(layers) = block.get("layers").and_then(|l| l.as_object()) {
            for (_, p) in layers {
                g.provide(p.as_str().unwrap(), std::fs::read(format!("{root}/{}", p.as_str().unwrap())).unwrap());
            }
        }
    }

    fn boot_town() -> GameCore {
        let Some(root) = ws_root() else {
            panic!("racine workspace introuvable (assets/ absents)");
        };
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        let mut g = GameCore::new(MANIFEST, "BA").unwrap();
        // Les DEUX niveaux (le voyage change de carte : assets cibles requis).
        for key in ["town", "bloodmoor"] {
            let lv = &m["levels"][key];
            g.provide(
                lv["palette"].as_str().unwrap(),
                std::fs::read(format!("{root}/{}", lv["palette"].as_str().unwrap())).unwrap(),
            );
            g.provide(
                lv["ds1"].as_str().unwrap(),
                std::fs::read(format!("{root}/{}", lv["ds1"].as_str().unwrap())).unwrap(),
            );
            for p in lv["dt1"].as_array().unwrap() {
                let p = p.as_str().unwrap();
                g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
            }
        }
        provide_anim(&mut g, &root, &m["heroes"]["BA"]["walk"]);
        for mon in m["monsters"].as_array().unwrap() {
            provide_anim(&mut g, &root, &mon["walk"]);
        }
        g.set_viewport(320, 240);
        g.finalize().expect("finalize town");
        g
    }

    #[test]
    fn town_waypoint_attached_and_json_exposes_it() {
        let g = boot_town();
        // Attach automatique au chargement du niveau (levelsId 1 → slot 0).
        let (slot, x, y) = g.world.as_ref().unwrap().level_waypoint().expect("town wp attaché");
        assert_eq!(slot, 0);
        assert_eq!((x, y), (144.5, 51.5)); // objet TownN1.ds1 (144,51)
        let v: Value = serde_json::from_str(&g.waypoints_json()).unwrap();
        assert_eq!(v["current"]["slot"], 0);
        assert_eq!(v["current"]["key"], "town");
        assert_eq!(v["current"]["activated"], false);
        let list = v["list"].as_array().unwrap();
        assert_eq!(list.len(), 41, "table complète (40 retail + 1 MVP)");
        let town = list.iter().find(|e| e["slot"] == 0).unwrap();
        assert_eq!(town["activated"], false);
        assert_eq!(town["travel"], false, "non activé = pas de voyage");
        assert_eq!(town["levelKey"], "town");
        let bm = list.iter().find(|e| e["slot"] == 39).unwrap();
        assert_eq!(bm["mvp"], true);
        assert_eq!(bm["pos"][0], 152.5);
    }

    #[test]
    fn activation_and_travel_town_to_bloodmoor_and_back() {
        let mut g = boot_town();
        // Voyage refusé tant que le slot n'est pas activé.
        let err = g.travel_waypoint(39).unwrap_err();
        assert!(err.contains("non activé"), "{err}");
        // Découverte directe du slot Blood Moor (MVP) puis voyage.
        g.world.as_mut().unwrap().activate_waypoint(39);
        let out = g.travel_waypoint(39).expect("voyage bloodmoor");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["key"], "bloodmoor");
        assert_eq!(v["slot"], 39);
        // Le niveau cible est chargé (seedé ou figé) et SON waypoint attaché.
        assert_eq!(
            serde_json::from_str::<Value>(&g.level_info_json()).unwrap()["key"],
            "bloodmoor"
        );
        let (slot, _, _) = g.world.as_ref().unwrap().level_waypoint().expect("bloodmoor wp attaché");
        assert_eq!(slot, 39);
        // Atterrissage SUR la pierre (régénération seedée : chunk (27,27)).
        // Retour vers le Camp des Rousses (slot 0 non activé → refus).
        let err = g.travel_waypoint(0).unwrap_err();
        assert!(err.contains("non activé"), "{err}");
        // Contact au sol bloodmoor : marche vers la pierre → activation 39.
        // (déjà activée ici) — on active le slot 0 directement puis retour.
        g.world.as_mut().unwrap().activate_waypoint(0);
        let out = g.travel_waypoint(0).expect("voyage town");
        assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["key"], "town");
        let (slot, _, _) = g.world.as_ref().unwrap().level_waypoint().unwrap();
        assert_eq!(slot, 0);
    }

    #[test]
    fn travel_unknown_slot_and_unplayable_level_fail_cleanly() {
        let mut g = boot_town();
        assert!(g.travel_waypoint(200).unwrap_err().contains("inconnu"));
        // Cold Plains (slot 1) : waypoint réel mais niveau sans bloc manifest.
        g.world.as_mut().unwrap().activate_waypoint(1);
        let err = g.travel_waypoint(1).unwrap_err();
        assert!(err.contains("sans bloc manifest"), "{err}");
    }

    #[test]
    fn save_load_keeps_activated_slots_through_engine() {
        let mut g = boot_town();
        g.world.as_mut().unwrap().activate_waypoint(0);
        g.world.as_mut().unwrap().activate_waypoint(39);
        let json = g.save_world();
        // (Le save porte le champ additif "waypoints".)
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["waypoints"], serde_json::json!([0, 39]));
        // Chargement : les slots restent, le waypoint du niveau est ré-attaché.
        g.load_world(&json).expect("reload");
        assert!(g.world.as_ref().unwrap().waypoint_activated(39));
        assert_eq!(
            g.world.as_ref().unwrap().level_waypoint().map(|(s, _, _)| s),
            Some(0),
            "attach refait après load_world"
        );
    }

    // --- 13b (finitions) : le panneau liste les 38 slots retail — vérif A4-A5

    #[test]
    fn waypoint_table_exposes_retail_slots_27_to_38() {
        // Slots 27-38 = actes IV-V : ids levels.txt v1.12, vérifiés contre la
        // colonne `Waypoint` de patch_levels.txt (sonde python 13b, CONCORDE).
        // C'est LA source du panneau démo (waypoints.js liste tout `list`).
        let want: &[(u8, u32)] = &[
            (27, 103), (28, 106), (29, 107),             // A4 : Pandémonium, Ville des damnés, Rivière de feu
            (30, 109), (31, 111), (32, 112), (33, 113),  // A5 : Harrogath, Rigid Highlands, Arreat, Passage Cristallin
            (34, 115), (35, 123), (36, 117), (37, 118), (38, 129), // Cavern 2, Death's Calling, Tundra, Glacial, Worldstone 2
        ];
        for (slot, level_id) in want {
            let wp = crate::world_impl::waypoint_by_slot(*slot)
                .unwrap_or_else(|| panic!("slot {slot} : absent de la table retail"));
            assert_eq!(wp.level_id, *level_id, "slot {slot} : levelId retail");
            assert!(!wp.name.is_empty(), "slot {slot} : nom affiché");
            assert_eq!(wp.act, if *slot <= 29 { 4 } else { 5 }, "slot {slot} : acte A4-A5");
        }
    }
}
