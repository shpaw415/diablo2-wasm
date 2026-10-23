//! NPC de ville (W16) côté moteur : attach du roster au chargement du Camp
//! des Rousses, état JSON pour l'UI (marqueurs + clic), interaction `talkNpc`.
//!
//! Tout est ADDITIF, même pattern que `waypoint_travel.rs` : un `impl
//! GameCore` séparé + deux appels d'attach dans `game.rs` (`load_level` /
//! `load_world`). Les données viennent de la table réelle
//! `d2rules::npc::NPCS` (patch_monstats.txt + section PATH de TownN1.ds1,
//! preuves dans `d2rules/src/npc.rs`).
//!
//! # Contrat UI (demo/npc-ui.js)
//! * `npcs()` — JSON `[{index, id, code, name, questCode, role, x, y,
//!   vendor, assumed, talked, walkable}]` (coordonnées en cellules ; la
//!   projection écran est linéaire : `[(wx-wy)*16, (wx+wy)*8]` ancrée sur le
//!   héros — `heroScreen()`).
//! * `talkNpc(index)` — interaction si le héros est à portée (2.5 cellules) :
//!   JSON `NpcInteraction` `{name, role, dialogue, actions, healed,
//!   quest_rewards}` ou `Err` (`"trop loin — …"`). Le monde émet
//!   `GameEvent::NpcTalked` (flux de tick) et avance le journal de quêtes
//!   branché (q3/q4 — contrat `impl-quetes.md`).

use crate::game::{GameCore, LevelDef};
use crate::world_impl::World;
use serde_json::{json, Value};

/// Id levels.txt du Camp des Rousses (colonne `Id` de `patch_levels.txt`,
/// ligne `Rogue Encampment` — même preuve que le waypoint slot 0 de
/// `waypoints.rs`). Le roster Acte I ne spawn que là.
const TOWN_LEVELS_ID: u32 = 1;

/// Attache le roster de ville au monde fraîchement chargé — sans effet si le
/// niveau n'est pas le camp (`spawn_town_npcs` est idempotent : re-spawn
/// après un retour au camp = no-op, l'état vient de la save).
pub(crate) fn attach_town_npcs(world: &mut World, def: &LevelDef) {
    if def.levels_id == Some(TOWN_LEVELS_ID) {
        world.spawn_town_npcs();
    }
}

impl GameCore {
    /// État du roster NPC pour l'UI (JSON ; `[]` sans monde ou hors camp).
    pub fn npcs_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "[]".into() };
        let list: Vec<Value> = w
            .town_npcs()
            .iter()
            .enumerate()
            .map(|(index, npc)| {
                let def = crate::world_impl::npc::NPCS[npc.def];
                json!({
                    "index": index,
                    "id": def.id,
                    "code": def.code,
                    "name": def.name,
                    "questCode": def.quest_code,
                    "role": def.role.as_str(),
                    "x": npc.ent.x,
                    "y": npc.ent.y,
                    "vendor": def.vendor.map(|v| v.name()),
                    "assumed": def.pos_assumed,
                    "talked": npc.talked,
                    "walking": npc.ent.anim == crate::world::AnimState::WL,
                })
            })
            .collect();
        Value::Array(list).to_string()
    }

    /// Clic sur le NPC `index` : interaction (dialogue + effet de rôle +
    /// avancement quête). JSON [`d2rules::NpcInteraction`] ou erreur
    /// descriptive (`Err`).
    pub fn talk_npc(&mut self, index: usize) -> Result<String, String> {
        let talk = self
            .world
            .as_mut()
            .ok_or_else(|| "moteur non initialisé".to_string())?
            .talk_to_npc(index)
            .map_err(|e| e.as_str().to_string())?;
        serde_json::to_string(&talk).map_err(|e| format!("sérialisation NPC : {e}"))
    }

    /// Journal de quêtes branché ? (E2E/debug — le branchement complet du
    /// journal reste à l'intégration ; ici seul `set_quest_log` du monde
    /// compte. Renvoie les états (`(id, état)` JSON) si branché.)
    #[allow(dead_code)] // garde-pointe intégration : consommé au câblage des quêtes
    pub(crate) fn quest_states_json(&self) -> Option<String> {
        let log = self.world.as_ref()?.quest_log()?;
        let states: Vec<Value> = log
            .states()
            .into_iter()
            .map(|(id, s)| {
                let st = match s {
                    d2rules::QuestState::NotStarted => "not_started",
                    d2rules::QuestState::InProgress => "in_progress",
                    d2rules::QuestState::Done => "done",
                    d2rules::QuestState::Failed => "failed",
                };
                json!({ "id": id, "state": st })
            })
            .collect();
        Some(Value::Array(states).to_string())
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

    /// Boot complet town (mêmes assets que les tests `waypoint_travel.rs`).
    fn boot_town() -> GameCore {
        let Some(root) = ws_root() else {
            panic!("racine workspace introuvable (assets/ absents)");
        };
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        let mut g = GameCore::new(MANIFEST, "BA").unwrap();
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
    fn town_boot_spawns_real_roster_and_json_exposes_it() {
        let g = boot_town();
        // L'attach au chargement du camp (levelsId 1) a spawné le roster.
        assert_eq!(g.world.as_ref().unwrap().town_npcs().len(), 6, "roster Acte I complet");
        let npcs: Vec<Value> = serde_json::from_str(&g.npcs_json()).unwrap();
        assert_eq!(npcs.len(), 6);
        let akara = npcs.iter().find(|n| n["id"] == "akara").unwrap();
        assert_eq!(akara["code"], "PS");
        assert_eq!(akara["role"], "healer");
        assert_eq!(akara["vendor"], "Akara");
        assert_eq!(akara["assumed"], false, "position PATH réelle");
        let cain = npcs.iter().find(|n| n["id"] == "cain1").unwrap();
        assert_eq!(cain["questCode"], "cain");
        assert_eq!(cain["assumed"], true, "position authoring MVP documentée");
        // Stands marchables sur la VRAIE carte (snap DS1).
        for n in &npcs {
            let (x, y) = (n["x"].as_f64().unwrap() as i32, n["y"].as_f64().unwrap() as i32);
            assert!(
                g.world.as_ref().unwrap().is_walkable(x, y),
                "NPC {} sur tuile bloquée de la vraie carte",
                n["id"]
            );
        }
        // Non-attaquables : le camp garde monsters() vide.
        assert!(g.world.as_ref().unwrap().monsters().is_empty());
    }

    #[test]
    fn bloodmoor_boot_spawns_no_npcs() {
        // Le roster n'existe QUE dans le camp (levelsId 1).
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        let Some(root) = ws_root() else { panic!("assets absents") };
        let mut g = GameCore::with_level(MANIFEST, "BA", "bloodmoor").unwrap();
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
        g.finalize().expect("finalize bloodmoor");
        assert!(g.world.as_ref().unwrap().town_npcs().is_empty());
        assert_eq!(g.npcs_json(), "[]");
    }

    #[test]
    fn talk_npc_range_and_interaction_json() {
        let mut g = boot_town();
        // Héros au spawn (centre du camp) : loin de tout NPC → Err descriptif.
        let err = g.talk_npc(0).unwrap_err();
        assert!(err.contains("trop loin"), "{err}");
        // Le héros re-entre au camp juste à côté d'Akara (API publique —
        // `switch_level` pose le spawn snapé, donc à portée de dialogue).
        let (ax, ay) = {
            let e = &g.world.as_ref().unwrap().town_npcs()[0].ent;
            (e.x, e.y)
        };
        g.switch_level("town", ax + 1.5, ay).expect("re-entrée du camp près d'Akara");
        let out = g.talk_npc(0).expect("interaction Akara");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["name"], "Akara");
        assert_eq!(v["role"], "healer");
        assert_eq!(v["healed"], false, "héros pleine vie : rien à soigner");
        assert!(v["actions"].as_array().unwrap().len() >= 2);
        // Le compteur de dialogues a suivi (NPC persistant).
        assert_eq!(g.world.as_ref().unwrap().town_npcs()[0].talked, 1);
        // Index hors bornes : erreur propre, jamais de panique.
        assert!(g.talk_npc(999).unwrap_err().contains("trop loin"));
    }

    #[test]
    fn save_load_keeps_roster_through_engine() {
        let mut g = boot_town();
        let (ax, ay) = {
            let e = &g.world.as_ref().unwrap().town_npcs()[0].ent;
            (e.x, e.y)
        };
        g.switch_level("town", ax + 1.5, ay).expect("re-entrée près d'Akara");
        g.talk_npc(0).expect("interaction Akara");
        let json = g.save_world();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["npcs"].as_array().map(|a| a.len()), Some(6));
        g.load_world(&json).expect("reload");
        assert_eq!(g.world.as_ref().unwrap().town_npcs()[0].talked, 1, "compteur persistant");
        // Re-spawn (retour au camp) : idempotent, pas de doublon.
        assert_eq!(g.world.as_mut().unwrap().spawn_town_npcs(), 6);
    }
}
