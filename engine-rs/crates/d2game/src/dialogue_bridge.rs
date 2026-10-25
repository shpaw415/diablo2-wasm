//! 22c PNJ & DIALOGUES — pont d2game/wasm de `d2rules::dialogue` (moteur du
//! coéquipier, étape 22a) vers la démo (étape 22c, `demo/dialogue-ui.js`).
//!
//! Tout est ADDITIF, même pattern que `npc_bridge.rs` : un `impl GameCore`
//! séparé + trois endpoints wasm append-only dans `wasm.rs` + une ligne
//! `pub mod` dans `lib.rs`. AUCUNE écriture hors de `d2w-pnj`.
//!
//! # Contrat UI (demo/dialogue-ui.js, `window.D2Dialogue`)
//! * `openDialogue(index)` — JSON [`d2rules::dialogue::DialogueSession`]
//!   (serde du moteur, champ par champ : `npc_index/npc/npc_name/quest/
//!   phase/node{...}`) AUGMENTÉ de `visible` : indices BRUTS des options
//!   visibles ([`World::dialogue_visible_options`] — conditions `quest_done`
//!   évaluées en vivant, ex. voyage Warriv ⇔ a1q6 Done). `Err` descriptif si
//!   hors de portée / héros mort / index invalide (mêmes messages que
//!   `talkNpc`). L'ouverture GÈLE le monde ([`World::tick`] ne simule plus
//!   rien tant que la session est ouverte — cf. `dialogue.rs`).
//! * `chooseDialogue(option_ix)` — INDEX BRUT dans `node.options` (l'UI
//!   affiche la sous-liste `visible`, elle choisit l'indice d'origine).
//!   JSON : la session À JOUR si le nœud a avancé (ragots « dites-m'en
//!   plus »), sinon l'[`d2rules::dialogue::DialogueAction`] produite
//!   (`{"kind":"open_shop","vendor":"Akara"}`, `{"kind":"close"}`…),
//!   `null` si le choix est refusé (condition non remplie / hors bornes).
//! * `closeDialogue()` — referme la session (« partir » / Échap UI),
//!   JSON `true/false`. Le tick du monde reprend.

use crate::game::GameCore;
use d2rules::dialogue::DialogueSession;
use serde_json::{json, Value};

impl GameCore {
    /// Session JSON pour l'UI : DialogueSession serde + `visible` (indices
    /// bruts des options visibles — évaluation vive des conditions).
    fn session_ui_json(&self) -> Option<Value> {
        let sess = self.world.as_ref()?.dialogue_session()?.clone();
        let mut v = serde_json::to_value(&sess).ok()?;
        v["visible"] = json!(self.world.as_ref().unwrap().dialogue_visible_options());
        Some(v)
    }

    /// Ouvre le dialogue du NPC `index` (clic NPC → npc-ui.js →
    /// dialogue-ui.js) — JSON session + `visible`, ou `Err` lisible.
    pub fn open_dialogue_json(&mut self, npc_index: usize) -> Result<String, String> {
        let w = self.world.as_mut().ok_or_else(|| "moteur non initialisé".to_string())?;
        w.open_dialogue(npc_index)
            .ok_or_else(|| "trop loin — approchez-vous du NPC".to_string())?;
        let v = self.session_ui_json().ok_or_else(|| "session introuvable".to_string())?;
        serde_json::to_string(&v).map_err(|e| format!("sérialisation session : {e}"))
    }

    /// Choix de l'option `option_ix` (index brut) — JSON session à jour
    /// (nœud avancé), sinon action produite, `null` si refusé.
    pub fn choose_dialogue_json(&mut self, option_ix: usize) -> Result<String, String> {
        {
            let w = self.world.as_mut().ok_or_else(|| "moteur non initialisé".to_string())?;
            if w.dialogue_session().is_none() {
                return Err("aucun dialogue ouvert".to_string());
            }
        }
        let Some(action) = self.world.as_mut().unwrap().choose_dialogue(option_ix) else {
            return Ok("null".into()); // condition quête non remplie / index invalide
        };
        // Say (ragots « dites-m'en plus ») : le nœud a (peut-être) avancé —
        // l'UI re-rend la session à jour. Toute AUTRE action (OpenShop/
        // Travel/Close/Hire/Repair) est renvoyée TELLE QUELLE : l'UI ouvre
        // le panneau correspondant et referme la session via closeDialogue
        // (le moteur ne referme lui-même que sur `Close` — cf. dialogue.rs).
        if matches!(action, d2rules::dialogue::DialogueAction::Say) {
            if let Some(v) = self.session_ui_json() {
                return serde_json::to_string(&v)
                    .map_err(|e| format!("sérialisation session : {e}"));
            }
        }
        serde_json::to_string(&action).map_err(|e| format!("sérialisation action : {e}"))
    }

    /// Referme la session (« partir », Échap UI) — `true` si une session
    /// a été fermée. Le monde reprend son tick.
    pub fn close_dialogue_json(&mut self) -> bool {
        self.world.as_mut().map(|w| w.close_dialogue()).unwrap_or(false)
    }

    /// Session ouverte (lecture, JSON) — `null` sans dialogue. (Getter de
    /// secours pour l'UI si un `choose` ne renvoyait que l'action.)
    pub fn dialogue_session_json(&self) -> String {
        match self.session_ui_json() {
            Some(v) => v.to_string(),
            None => "null".into(),
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

    /// Boot complet town (même recette que `npc_bridge.rs::boot_town`).
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

    /// Héros re-placé à côté d'Akara (portée de dialogue, comme le retail).
    fn near_akara(g: &mut GameCore) {
        let (ax, ay) = {
            let e = &g.world.as_ref().unwrap().town_npcs()[0].ent;
            (e.x, e.y)
        };
        g.switch_level("town", ax + 1.5, ay).expect("re-entrée du camp près d'Akara");
    }

    // -----------------------------------------------------------------
    // CYCLE OPEN / CHOOSE / CLOSE via le pont JSON (contrat dialogue-ui.js)
    // -----------------------------------------------------------------

    #[test]
    fn cycle_open_choose_close_via_pont_json() {
        let mut g = boot_town();
        // Hors de portée : Err descriptif (même message que talkNpc).
        let err = g.open_dialogue_json(0).unwrap_err();
        assert!(err.contains("trop loin"), "{err}");
        near_akara(&mut g);
        // Pas de session avant ouverture.
        assert_eq!(g.dialogue_session_json(), "null");
        // OPEN : session JSON complète (champs serde du moteur + visible).
        let out = g.open_dialogue_json(0).expect("ouverture Akara");
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["npc_name"], "Akara");
        assert_eq!(v["npc"], "akara");
        assert_eq!(v["node"]["id"], "root");
        assert!(v["node"]["lignes"].as_array().unwrap().len() >= 1, "ligne fr réelle");
        assert!(!v["node"]["lignes"][0].as_str().unwrap().is_empty());
        let labels: Vec<&str> = v["node"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .map(|o| o["label"].as_str().unwrap())
            .collect();
        assert!(labels.contains(&"acheter/vendre"), "menu trade (NPCMenuTrade) : {labels:?}");
        assert!(labels.contains(&"partir"), "menu leave (NPCMenuLeave) : {labels:?}");
        assert!(labels.contains(&"divers"), "menu gossip (NPCGossipMenu) : {labels:?}");
        // `visible` = indices bruts couvrant le nœud (conditions always).
        let visible: Vec<usize> = v["visible"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_u64().unwrap() as usize)
            .collect();
        assert_eq!(visible.len(), 3, "toutes les options `always` sont visibles : {visible:?}");
        // Le monde est GELÉ : le tick ne bouge ni le héros ni l'horloge
        // (contrat World::tick — dialogue.rs) et l'UI reçoit les events.
        let (hx, hy) = {
            let h = g.world.as_ref().unwrap().hero();
            (h.x, h.y)
        };
        let ev = g.tick(16, Some(crate::world::PlayerInput::MoveTo { x: 1, y: 1 }));
        let (hx2, hy2) = {
            let h = g.world.as_ref().unwrap().hero();
            (h.x, h.y)
        };
        assert_eq!((hx, hy), (hx2, hy2), "monde gelé : le héros ne bouge pas");
        assert!(
            ev.iter().any(|e| matches!(
                e,
                crate::game::CoreEvent::World(d2rules::GameEvent::NpcTalked { ref npc })
                    if npc == "akara"
            )),
            "NpcTalked d'ouverture flushé au tick suivant : {ev:?}"
        );
        // CHOOSE « divers » → session à jour (nœud gossip, ligne fr réelle).
        let divers_ix = v["node"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .position(|o| o["label"] == "divers")
            .unwrap();
        let out = g.choose_dialogue_json(divers_ix).expect("choix divers");
        let v2: Value = serde_json::from_str(&out).unwrap();
        assert!(v2["node"]["id"] == "gossip1", "nœud ragot rendu : {}", v2["node"]["id"]);
        assert!(v2["node"]["lignes"][0].as_str().unwrap().contains("Akara"));
        // CHOOSE « acheter/vendre » → action OpenShop (session refermée).
        let trade_ix = v2["node"]["options"]
            .as_array()
            .unwrap()
            .iter()
            .position(|o| o["label"] == "acheter/vendre")
            .unwrap();
        let out = g.choose_dialogue_json(trade_ix).expect("choix trade");
        let v3: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v3["kind"], "open_shop", "action OpenShop pour l'UI (panneau Marchands)");
        assert_eq!(v3["vendor"], "Akara");
        // Le moteur garde la session ouverte (seul Close referme) — l'UI
        // referme via closeDialogue après avoir ouvert le panneau Marchands.
        assert!(g.dialogue_session_json() != "null", "session encore ouverte côté moteur");
        assert_eq!(g.close_dialogue_json(), true, "l'UI referme (partir boutique)");
        assert!(g.close_dialogue_json() == false, "plus rien à fermer côté moteur");
        // Ré-ouverture pour « partir » : CLOSE via l'endpoint.
        near_akara(&mut g);
        g.open_dialogue_json(0).expect("réouverture");
        let out = g.choose_dialogue_json(
            serde_json::from_str::<Value>(&g.dialogue_session_json()).unwrap()["node"]["options"]
                .as_array()
                .unwrap()
                .iter()
                .position(|o| o["label"] == "partir")
                .unwrap(),
        ).expect("choix partir");
        assert_eq!(serde_json::from_str::<Value>(&out).unwrap()["kind"], "close");
        assert_eq!(g.dialogue_session_json(), "null", "« partir » refermé");
        // Le monde a repris : le héros marche à nouveau (input move réel
        // vers une tuile marchable PROUVÉE près de lui — (1,1) serait hors
        // du camp, A* le refuserait et le héros resterait sur place).
        let (mut tx, mut ty) = (0i32, 0i32);
        'found: for r in 2..=10 {
            for (dx, dy) in [(-r, 0), (r, 0), (0, -r), (0, r), (-r, -r), (r, r)] {
                let c = (hx + dx as f32, hy + dy as f32);
                if g.world.as_ref().unwrap().is_walkable(c.0 as i32, c.1 as i32) {
                    tx = c.0 as i32;
                    ty = c.1 as i32;
                    break 'found;
                }
            }
        }
        assert!((tx, ty) != (0, 0) || g.world.as_ref().unwrap().is_walkable(hx as i32 + 2, hy as i32),
            "aucune tuile marchable proche trouvée (grille inattendue)");
        g.tick(16, Some(crate::world::PlayerInput::MoveTo { x: tx, y: ty }));
        for _ in 0..30 {
            g.tick(16, None); // ~0,5 s de marche (6 cellules/s)
        }
        let (hx3, hy3) = {
            let h = g.world.as_ref().unwrap().hero();
            (h.x, h.y)
        };
        assert_ne!((hx3, hy3), (hx, hy), "monde repris après close (le héros avance)");
    }

    #[test]
    fn choose_refuse_et_close_endpoint_sans_session() {
        let mut g = boot_town();
        // Aucun dialogue ouvert : choose/close propres (pas de panique).
        assert!(g.choose_dialogue_json(0).unwrap_err().contains("aucun dialogue"));
        assert_eq!(g.close_dialogue_json(), false);
        near_akara(&mut g);
        g.open_dialogue_json(0).expect("ouverture");
        // Index hors bornes : `null` (choix refusé), session INCHANGÉE.
        assert_eq!(g.choose_dialogue_json(99).unwrap(), "null");
        assert!(g.dialogue_session_json() != "null", "session préservée après refus");
        // CLOSE endpoint (Échap UI) : true, puis le monde reprend.
        assert_eq!(g.close_dialogue_json(), true);
        assert_eq!(g.dialogue_session_json(), "null");
        // Hors monde (aucun monde) ne peut être testé ici : boot_town garantit
        // un monde ; le contrat `moteur non initialisé` est défensif.
    }
}
