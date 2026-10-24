//! Consommables 11b côté moteur — wire MINIMAL de `d2rules::consumables`
//! vers l'UI (même pattern additif que `npc_bridge.rs`/`waypoint_travel.rs` :
//! un `impl GameCore` séparé, zéro changement du flux historique).
//!
//! * [`GameCore::use_consumable_slot`] / [`GameCore::use_belt_key`] —
//!   appliquent l'effet réel par code (potions/buffs/charges de tomes) et
//!   renvoient le [`d2rules::consumables::ConsumUse`] en JSON ; en cas de
//!   PORTE DE VILLE (`tsc`/`tbk`), basculent vers le camp via le
//!   `switch_level` EXISTANT ([`GameCore::town_portal_switch`] — aucune
//!   mécanique de portail nouvelle ; arrivée à l'entrée du camp
//!   `warp.entry`, même règle que la zone warp de `game.rs`), en portant
//!   `levelSwitch` dans le JSON de retour pour la bannière/musique page ;
//! * [`GameCore::belt_auto_store`] — « auto-ranger » les potions
//!   `autobelt=1` (la page l'appelle après un ramassage) ;
//! * [`GameCore::belt_json`] — état ceinture+buffs pour l'HUD (lecture).
//!
//! L'input historique `useItem` (soin MVP 60) reste strictement inchangé ;
//! la couverture d'effets/consommation vit dans `d2rules::consumables`
//! (11 tests golden) — ici on teste le PLUMBING moteur (switch camp,
//! erreurs JSON, ceinture vide), l'injection d'inventaire n'ayant pas
//! d'API publique cross-crate.

use serde_json::json;

use crate::game::GameCore;
use d2rules::consumables::ConsumUse;

impl GameCore {
    /// Utilise le consommable du slot d'inventaire `slot` (effets réels
    /// 11b) — JSON `{"use":{code,name,family,heal,mana,buffMs,townPortal,
    /// identified,chargesLeft},"levelSwitch":{key,name}?}` ou `Err` lisible.
    pub fn use_consumable_slot(&mut self, slot: usize) -> Result<String, String> {
        let use1 = {
            let w = self.world.as_mut().ok_or("finalize first")?;
            w.use_consumable(slot).map_err(|e| e.to_string())?
        };
        self.finish_consumable_use(use1)
    }

    /// Touche de ceinture `key` (1..=4) — même contrat JSON que
    /// [`GameCore::use_consumable_slot`].
    pub fn use_belt_key(&mut self, key: usize) -> Result<String, String> {
        let use1 = {
            let w = self.world.as_mut().ok_or("finalize first")?;
            w.use_belt_key(key).map_err(|e| e.to_string())?
        };
        self.finish_consumable_use(use1)
    }

    /// Auto-ranger les potions `autobelt=1` de l'inventaire vers la ceinture
    /// — nombre d'objets rangés (idempotent, la page l'appelle au ramassage).
    pub fn belt_auto_store(&mut self) -> usize {
        match self.world.as_mut() {
            Some(w) => w.belt_auto_store(),
            None => 0,
        }
    }

    /// État ceinture + buffs (lecture HUD) — JSON
    /// `{"belt":[[4 colonnes × 4 cellules]],"buffs":[{"state","remainingMs",
    /// "stats":[[id,val],…]},…]}` ; `{}` avant `finalize`.
    pub fn belt_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "{}".into() };
        let c = w.consumables();
        json!({
            "belt": c.belt.iter().map(|col| col.iter().map(|cell| match cell {
                Some(d) => json!({"code": d.code, "name": d.name, "kind": d.kind}),
                None => serde_json::Value::Null,
            }).collect::<Vec<_>>()).collect::<Vec<_>>(),
            "buffs": c.buffs.iter().map(|b| json!({
                "state": b.state,
                "remainingMs": b.remaining_ms,
                "stats": b.stats,
            })).collect::<Vec<_>>(),
        })
        .to_string()
    }

    /// Retour au camp via le `switch_level` EXISTANT : destination `town`,
    /// arrivée à `warp.entry` du camp (repli `(0,0)`, snappé marchable par
    /// le switch). Public : consommé par [`GameCore::finish_consumable_use`]
    /// (parchemins/tomes `town_portal`) et testable isolément.
    pub fn town_portal_switch(&mut self) -> Result<crate::game::LevelSwitchInfo, String> {
        let (sx, sy) = self
            .resolve_level_def("town")
            .ok()
            .and_then(|d| d.warp.map(|wp| wp.entry))
            .unwrap_or((0.0, 0.0));
        self.switch_level("town", sx, sy)
    }

    /// Suite commune : porte de ville ⇒ [`GameCore::town_portal_switch`],
    /// `levelSwitch` ajouté au JSON de retour.
    fn finish_consumable_use(&mut self, use1: ConsumUse) -> Result<String, String> {
        let mut out = json!({ "use": use1 });
        if use1.town_portal {
            let info = self.town_portal_switch()?;
            out["levelSwitch"] = json!({ "key": info.key, "name": info.name });
        }
        Ok(out.to_string())
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

    /// Boot complet town+bloodmoor (mêmes assets que les tests historiques
    /// de `waypoint_travel.rs` — le TP change de carte).
    fn boot(level: &str) -> GameCore {
        let Some(root) = ws_root() else {
            panic!("racine workspace introuvable (assets/ absents)");
        };
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        let mut g = GameCore::with_level(MANIFEST, "BA", level).unwrap();
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
        if let Some(p) = m["heroes"]["BA"]["walk"]["cof"].as_str() {
            g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
        }
        g.set_viewport(320, 240);
        g.finalize().expect("finalize");
        g
    }

    #[test]
    fn town_portal_switch_uses_existing_switch_to_town_entry() {
        let mut g = boot("bloodmoor");
        assert_eq!(
            serde_json::from_str::<Value>(&g.level_info_json()).unwrap()["key"],
            "bloodmoor",
            "boot en lande"
        );
        let info = g.town_portal_switch().expect("switch camp");
        assert_eq!(info.key, "town");
        let li: Value = serde_json::from_str(&g.level_info_json()).unwrap();
        assert_eq!(li["key"], "town", "le switch EXISTANT a bien basculé");
        assert_eq!(li["name"], "Le Camp des Rogues", "nom fr W18");
        // le héros est posé près de l'entrée du camp (warp.entry 247,107 —
        // snap marchable), hors de la zone warp de sortie (282,107 r=4) :
        // pas de re-warp immédiat.
        let h = *g.world.as_ref().unwrap().hero();
        assert!(
            (h.x - 282.0).hypot(h.y - 107.0) > 4.0,
            "hors zone warp ({}, {})",
            h.x,
            h.y
        );
    }

    #[test]
    fn consumable_endpoints_error_contract() {
        // Avant finalize : monde absent ⇒ "finalize first" (plumbing).
        let mut g = GameCore::new(MANIFEST, "BA").unwrap();
        assert_eq!(g.use_consumable_slot(0).unwrap_err(), "finalize first");
        assert_eq!(g.belt_auto_store(), 0);
        assert_eq!(g.belt_json(), "{}");
        // Après finalize : erreurs TYPÉES d2rules traversées en texte lisible.
        let mut g = boot("town");
        assert!(g.use_consumable_slot(0).unwrap_err().contains("slot vide"));
        assert!(g.use_belt_key(1).unwrap_err().contains("slot vide"), "ceinture vide");
        assert!(g.use_belt_key(5).unwrap_err().contains("invalide"), "touche 5");
        // Ceinture vide : auto-ranger est un no-op propre, l'état est lisible.
        assert_eq!(g.belt_auto_store(), 0);
        let belt: Value = serde_json::from_str(&g.belt_json()).unwrap();
        assert_eq!(belt["belt"].as_array().unwrap().len(), 4, "4 colonnes (touches 1-4)");
        assert!(belt["belt"][0].as_array().unwrap().iter().all(|c| c.is_null()));
        assert!(belt["buffs"].as_array().unwrap().is_empty());
    }
}
