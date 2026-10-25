//! 3c-b COOP — pont d2game/wasm de `d2rules::dialogue` (port 22a du corps
//! maître, adapté multi-joueurs) vers la démo (demo/dialogue-ui.js).
//!
//! Tout est ADDITIF, même pattern que `npc_bridge.rs` : un `impl GameCore`
//! séparé + endpoints wasm append-only dans `wasm.rs` + une ligne `pub mod`
//! dans `lib.rs`. AUCUNE écriture hors de l'étape 3c-b.
//!
//! # Contrat UI (demo/dialogue-ui.js, `window.D2Dialogue`)
//! * `openDialogueSlot(slot, index)` — JSON [`d2rules::dialogue::
//!   DialogueSession`] (serde du moteur : `slot/npc_index/npc/npc_name/
//!   quest/phase/node{...}`) AUGMENTÉ de `visible` (indices BRUTS des
//!   options visibles — conditions `quest_done` évaluées en vivant) et de
//!   `player`/`playerName` (le teneur, pour l'UI). `Err` descriptif si
//!   VERROU (autre slot discute déjà — le message porte NPC + teneur),
//!   hors de portée, héros mort, slot/index inconnu. L'ouverture GÈLE le
//!   monde pour TOUS les slots ([`d2rules::World::tick_inputs`] ne simule
//!   plus rien — décision style retail, cf. dialogue.rs § coop).
//! * `chooseDialogueSlot(slot, option_ix)` — INDEX BRUT dans
//!   `node.options`, réservé au TENEUR (`null` sinon). JSON : la session à
//!   jour si le nœud a avancé (ragots « dites-m'en plus »), sinon
//!   l'action produite (`{"kind":"open_shop","vendor":"Akara"}`,
//!   `{"kind":"close"}`…) — l'UI n'ouvre le panneau vendeur QUE sur le
//!   client du teneur (point (3) mission).
//! * `closeDialogueSlot(slot)` — « partir »/Échap, `false` si le slot n'est
//!   pas le teneur. Le tick du monde reprend pour tous.
//! * `dialogueSession()` — session JSON courante (poll bandeau/refresh),
//!   `null` sans dialogue.
//! * `dialogueLock()` — `{"active":true,npc,npcName,slot,player,playerName}`
//!   quand une session est tenue par `slot` (les AUTRES clients affichent
//!   « <NPC> discute avec <joueur> » et bloquent le clic), `{"active":false}`
//!   sinon.
//!
//! # Verrou + gel (décisions 3c-b, cf. dialogue.rs § coop)
//! Le monde est partagé : UNE seule session à la fois, gel GLOBAL (tous
//! slots) pendant qu'elle est ouverte — le brief « ne pas ouvrir le même
//! NPC » est couvert a fortiori. Les chemins historiques sans slot
//! (`openDialogue` solo maître) ne sont PAS exposés ici : l'UI coop passe
//! toujours par les variantes `_slot` (chaque client connaît son slot).

use crate::game::GameCore;
use serde_json::{json, Value};

impl GameCore {
    /// Code de classe du héros `slot` (public World — `players[slot].code`).
    fn slot_hero_code(&self, slot: usize) -> Option<String> {
        self.world.as_ref()?.player(slot).map(|p| p.code().to_string())
    }

    /// Nom affichable de la classe `code` (manifest `heroes[code].name`).
    fn slot_hero_name(&self, code: &str) -> String {
        self.manifest
            .pointer(&format!("/heroes/{code}/name"))
            .and_then(|n| n.as_str())
            .unwrap_or(code)
            .to_string()
    }

    /// Session JSON pour l'UI : DialogueSession serde + `visible` (indices
    /// bruts des options visibles) + identité du teneur (`player`).
    fn session_ui_json(&self) -> Option<Value> {
        let w = self.world.as_ref()?;
        let sess = w.dialogue_session()?.clone();
        let mut v = serde_json::to_value(&sess).ok()?;
        v["visible"] = json!(w.dialogue_visible_options());
        let code = self.slot_hero_code(sess.slot).unwrap_or_default();
        v["player"] = json!(code);
        v["playerName"] = json!(self.slot_hero_name(&code));
        Some(v)
    }

    /// Ouvre le dialogue du NPC `index` pour le joueur `slot` (clic NPC →
    /// dialogue-ui.js `tryOpen`) — JSON session + `visible` + teneur, ou
    /// `Err` lisible (verrou « X discute avec le joueur N », portée…).
    pub fn open_dialogue_slot_json(&mut self, slot: usize, npc_index: usize) -> Result<String, String> {
        let w = self
            .world
            .as_mut()
            .ok_or_else(|| "moteur non initialisé".to_string())?;
        w.open_dialogue_slot(slot, npc_index)
            .map_err(|e| e.as_str())?;
        let v = self
            .session_ui_json()
            .ok_or_else(|| "session introuvable".to_string())?;
        serde_json::to_string(&v).map_err(|e| format!("sérialisation session : {e}"))
    }

    /// Choix de l'option `option_ix` (index brut) par le TENEUR `slot` —
    /// JSON session à jour (nœud avancé), sinon action produite, `null`
    /// si refusé (verrou autre slot / condition non remplie / index).
    pub fn choose_dialogue_slot_json(&mut self, slot: usize, option_ix: usize) -> Result<String, String> {
        {
            let w = self
                .world
                .as_mut()
                .ok_or_else(|| "moteur non initialisé".to_string())?;
            if w.dialogue_session().is_none() {
                return Err("aucun dialogue ouvert".to_string());
            }
        }
        let Some(action) = self
            .world
            .as_mut()
            .unwrap()
            .choose_dialogue_slot(slot, option_ix)
        else {
            return Ok("null".into()); // verrou / condition quête / index invalide
        };
        // Say (ragots « dites-m'en plus ») : le nœud a (peut-être) avancé —
        // l'UI re-rend la session à jour. Toute AUTRE action (OpenShop/
        // Travel/Close/Hire/Repair) est renvoyée TELLE QUELLE : l'UI du
        // TENEUR ouvre le panneau correspondant et referme via
        // closeDialogueSlot (le moteur ne referme lui-même que sur `Close`).
        if matches!(action, d2rules::dialogue::DialogueAction::Say) {
            if let Some(v) = self.session_ui_json() {
                return serde_json::to_string(&v)
                    .map_err(|e| format!("sérialisation session : {e}"));
            }
        }
        serde_json::to_string(&action).map_err(|e| format!("sérialisation action : {e}"))
    }

    /// Referme la session si `slot` en est le teneur (« partir », Échap
    /// UI) — `false` sinon (session inchangée). Le monde reprend.
    pub fn close_dialogue_slot_json(&mut self, slot: usize) -> bool {
        self.world.as_mut().map(|w| w.close_dialogue_slot(slot)).unwrap_or(false)
    }

    /// Session ouverte (lecture, JSON + `visible` + teneur) — `null` sans
    /// dialogue. (Getter de secours pour l'UI si un `choose` ne renvoyait
    /// que l'action ; pollé aussi pour le refresh bandeau.)
    pub fn dialogue_session_json(&self) -> String {
        match self.session_ui_json() {
            Some(v) => v.to_string(),
            None => "null".into(),
        }
    }

    /// VERROU (3c-b) : qui discute, avec qui — JSON
    /// `{"active":true,"npc","npcName","slot","player","playerName"}` ou
    /// `{"active":false}`. Les clients NON teneurs affichent le bandeau
    /// « <NPC> discute avec <joueur> » et bloquent l'ouverture (le moteur
    /// refuse de toute façon : Busy).
    pub fn dialogue_lock_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "{\"active\":false}".into() };
        match w.dialogue_session() {
            Some(s) => {
                let code = self.slot_hero_code(s.slot).unwrap_or_default();
                json!({
                    "active": true,
                    "npc": s.npc,
                    "npcName": s.npc_name,
                    "slot": s.slot,
                    "player": code,
                    "playerName": self.slot_hero_name(&code),
                })
                .to_string()
            }
            None => "{\"active\":false}".into(),
        }
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

    /// Boot coop 2 slots au camp (même pipeline que `coop_3b2_proof`) :
    /// niveau + anims de marche, `with_level_coop`, finalize.
    fn boot_coop_town() -> GameCore {
        let Some(root) = ws_root() else {
            panic!("racine workspace introuvable (assets/ absents)");
        };
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        let heroes = serde_json::json!(["BA", "SOR"]).to_string();
        let mut g = GameCore::with_level_coop(MANIFEST, &heroes, "town").unwrap();
        let lv = &m["levels"]["town"];
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
        for code in ["BA", "SOR"] {
            let resolved = d2rules::resolve_hero_code(&m, code);
            if let Some(walk) = m.pointer(&format!("/heroes/{resolved}/walk")) {
                if let Some(cof) = walk.get("cof").and_then(|c| c.as_str()) {
                    g.provide(cof, std::fs::read(format!("{root}/{cof}")).unwrap());
                }
                if let Some(layers) = walk.get("layers").and_then(|l| l.as_object()) {
                    for (_, p) in layers {
                        let p = p.as_str().unwrap();
                        g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
                    }
                }
            }
        }
        g.finalize().expect("finalize town coop");
        g
    }

    fn akara_index(g: &GameCore) -> usize {
        g.world
            .as_ref()
            .unwrap()
            .town_npcs()
            .iter()
            .position(|n| crate::world_impl::npc::NPCS[n.def].quest_code == "akara")
            .expect("Akara au roster")
    }

    #[test]
    fn coop_verrou_json_et_gel_tous_slots_via_le_pont() {
        let mut g = boot_coop_town();
        // Teneur = slot 0 (téléporté près d'Akara via l'API publique,
        // même chemin que `npc_bridge::talk_npc_range_and_interaction_json`).
        let a = akara_index(&g);
        let (ax, ay) = {
            let e = &g.world.as_ref().unwrap().town_npcs()[a].ent;
            (e.x, e.y)
        };
        g.switch_level("town", ax + 1.5, ay).expect("re-entrée près d'Akara");
        // Pas de dialogue : verrou inactif.
        assert_eq!(g.dialogue_lock_json(), "{\"active\":false}");
        // Slot 1 : hors de portée → Err lisible, monde toujours actif.
        let err = g.open_dialogue_slot_json(1, a).unwrap_err();
        assert!(err.contains("trop loin"), "{err}");
        // Slot 0 ouvre : session JSON porte slot/teneur + options visibles.
        let raw = g.open_dialogue_slot_json(0, a).expect("ouverture slot 0");
        let sess: Value = serde_json::from_str(&raw).unwrap();
        assert_eq!(sess["slot"], 0);
        assert_eq!(sess["npc"], "akara");
        assert_eq!(sess["player"], "BA");
        assert!(sess["visible"].as_array().unwrap().len() >= 2, "options visibles");
        // VERROU : le slot 1 est refusé — le message porte NPC + teneur.
        let err = g.open_dialogue_slot_json(1, a).unwrap_err();
        assert!(err.contains("discute avec le joueur 1"), "{err}");
        let lock: Value = serde_json::from_str(&g.dialogue_lock_json()).unwrap();
        assert_eq!(lock["active"], true);
        assert_eq!(lock["npc"], "akara");
        assert_eq!(lock["slot"], 0);
        assert_eq!(lock["playerName"], "Barbare");
        // Choix par le mauvais slot : `null`, session inchangée.
        assert_eq!(g.choose_dialogue_slot_json(1, 0).unwrap(), "null");
        assert!(g.dialogue_session_json() != "null");
        // GEL GLOBAL via le pont : entrées des DEUX slots en file + un
        // tickQueued → RIEN ne bouge, ni l'un ni l'autre.
        let pos = |gg: &GameCore, slot: usize| {
            let v: Value = serde_json::from_str(&gg.hero_state_slot_json(slot)).unwrap();
            (v["x"].as_f64().unwrap(), v["y"].as_f64().unwrap())
        };
        let before = (pos(&g, 0), pos(&g, 1));
        g.queue_input(0, d2rules::PlayerInput::MoveTo { x: 50, y: 50 });
        g.queue_input(1, d2rules::PlayerInput::MoveTo { x: 40, y: 40 });
        g.tick_queued(100);
        assert_eq!(pos(&g, 0), before.0, "slot 0 gelé");
        assert_eq!(pos(&g, 1), before.1, "slot 1 gelé (gel GLOBAL)");
        // Fermeture par le mauvais slot : refusée ; par le teneur : ok.
        assert!(!g.close_dialogue_slot_json(1));
        assert!(g.close_dialogue_slot_json(0));
        assert_eq!(g.dialogue_lock_json(), "{\"active\":false}");
        // Le monde reprend : les mêmes entrées déplacent les deux héros.
        g.queue_input(0, d2rules::PlayerInput::MoveTo { x: 50, y: 50 });
        g.queue_input(1, d2rules::PlayerInput::MoveTo { x: 40, y: 40 });
        g.tick_queued(100);
        g.tick_queued(100);
        assert_ne!(pos(&g, 0), before.0, "slot 0 redémarre");
        assert_ne!(pos(&g, 1), before.1, "slot 1 redémarre");
    }

    #[test]
    fn coop_boutique_reservee_au_teneur_action_open_shop() {
        let mut g = boot_coop_town();
        let a = akara_index(&g);
        let (ax, ay) = {
            let e = &g.world.as_ref().unwrap().town_npcs()[a].ent;
            (e.x, e.y)
        };
        g.switch_level("town", ax + 1.5, ay).expect("re-entrée près d'Akara");
        g.open_dialogue_slot_json(0, a).expect("session slot 0");
        // L'option « acheter/vendre » (Akara) : index brut dans node.options.
        let sess: Value = serde_json::from_str(&g.dialogue_session_json()).unwrap();
        let shop_ix = sess["node"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .position(|o| o["action"]["kind"] == "open_shop")
            .expect("option boutique d'Akara");
        // Le non-teneur ne peut même pas sélectionner (verrou UI+moteur).
        assert_eq!(g.choose_dialogue_slot_json(1, shop_ix).unwrap(), "null");
        // Le teneur choisit : l'action open_shop sort — SEUL SON CLIENT
        // ouvrira le panneau vendeur (dialogue-ui.js vérifie slot === mien).
        let out = g.choose_dialogue_slot_json(0, shop_ix).unwrap();
        let act: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(act["kind"], "open_shop");
        assert_eq!(act["vendor"], "Akara");
    }
}
