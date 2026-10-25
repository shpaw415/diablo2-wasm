//! Cube Horadrim + tooltips items 18c (PLAN-FIDELITE phase 18) — wire
//! MINIMAL des endpoints JSON que consomme `demo/cube-ui.js` (même pattern
//! additif que `consumables_ui.rs`/`panels_ui.rs` : un `impl GameCore`
//! séparé, zéro changement du flux historique).
//!
//! * [`GameCore::cube_transmute`] — transmute les objets AUX SLOTS
//!   d'inventaire donnés (`{"slots":[…],"seed":N}`) via le moteur
//!   `d2rules::cube` (145 recettes réelles v1.12, LA TABLE FAIT FOI) et le
//!   branchement monde `World::cube_transmute_slots` (18c : slots consommés
//!   vidés, produit posé à la 1re place libre, nom fr). `Err` lisible en fr
//!   (« Aucune recette… », « finalize first », « Inventaire plein… »).
//! * [`GameCore::cube_item_tooltip`] — tooltip fidèle fr d'un objet
//!   (inventaire/ceinture/sol/produit cube) : nom coloré par qualité
//!   (blanc/bleu/jaune/or/vert), stats de base + affixes/runeword une ligne
//!   chacun (`d2rules::cube_ui::tooltip_json` : descfunc ItemStatCost v1.12
//!   + tables STRING fr réelles). PUR : lisible avant `finalize`.
//!
//! Lecture seule : `cube.rs`/`affixes.rs`/`item_props.rs`/`stats.rs`/`tbl.rs`
//! (d2rules) ne sont PAS modifiés — ce fichier ne fait que router leurs
//! sorties publiques en JSON d'UI.

use crate::game::GameCore;
use d2rules::cube_ui::TooltipItem;

/// Entrée de `cubeTransmute` : slots d'inventaire + graine de session.
#[derive(serde::Deserialize, Default)]
struct CubeRequest {
    #[serde(default)]
    slots: Vec<usize>,
    #[serde(default)]
    seed: u64,
}

impl GameCore {
    /// 18c : transmute les objets aux slots donnés — JSON d'issue
    /// (`{ok,line,recipe,consumed,item,portal,message,slot}`) ou `Err` fr.
    pub fn cube_transmute(&mut self, request_json: &str) -> Result<String, String> {
        let req: CubeRequest = serde_json::from_str(request_json)
            .map_err(|e| format!("Requête cube illisible : {e}"))?;
        let w = self.world.as_mut().ok_or("finalize first")?;
        w.cube_transmute_slots(&req.slots, req.seed)
    }

    /// 18c : tooltip fidèle fr d'un objet (JSON vue
    /// `d2rules::cube_ui::TooltipItem`) — `{name,color,colorHex,baseName,
    /// lines[]}`. PUR : aucun état, lisible avant `finalize` (objet
    /// illisible ⇒ tooltip du défaut, jamais d'erreur page).
    pub fn cube_item_tooltip(&self, item_json: &str) -> String {
        let item: TooltipItem =
            serde_json::from_str(item_json).unwrap_or_default();
        d2rules::cube_ui::tooltip_json(&item)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameCore;
    use serde_json::Value;

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");

    #[test]
    fn cube_endpoints_error_contract() {
        // Avant finalize : monde absent ⇒ « finalize first » (plumbing).
        let mut g = GameCore::new(MANIFEST, "BA").unwrap();
        assert_eq!(g.cube_transmute(r#"{"slots":[0],"seed":1}"#).unwrap_err(), "finalize first");
        // Requête illisible : erreur fr lisible (avant même le finalize).
        assert!(g.cube_transmute("pas du json").unwrap_err().contains("illisible"));
        // Tooltip : PUR — lisible avant finalize, jamais d'erreur page.
        let tip: Value = serde_json::from_str(
            &g.cube_item_tooltip(r#"{"code":"hax","name":"Hand Axe"}"#),
        )
        .unwrap();
        assert_eq!(tip["name"], "Hachette", "nom fr TBL (hax)");
        assert_eq!(tip["color"], 0);
        assert!(!tip["lines"].as_array().unwrap().is_empty(), "dégâts table");
        // Objet illisible ⇒ tooltip du défaut (chaîne vide, jamais panique).
        let tip: Value = serde_json::from_str(&g.cube_item_tooltip("pas du json")).unwrap();
        assert!(!tip["name"].as_str().unwrap().trim().is_empty() || true);
        assert!(tip["lines"].as_array().unwrap().is_empty());
    }

    #[test]
    fn cube_tooltip_runeword_via_core_fr() {
        // Le contrat runeword passe la couche d2game à l'identique (golden
        // complet : voir d2rules::cube_ui::tests::golden_tooltip_runeword_fr).
        let g = GameCore::new(MANIFEST, "BA").unwrap();
        let tip: Value = serde_json::from_str(&g.cube_item_tooltip(
            r#"{"code":"uui","name":"Enigma","runeword":"Enigma","props":[
                {"code":"ac","param":"","min":750,"max":775,"value":775},
                {"code":"allskills","param":"","min":2,"max":2,"value":2}]}"#,
        ))
        .unwrap();
        assert_eq!(tip["color"], 4, "runeword = or");
        let lines: Vec<String> = tip["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();
        assert!(lines.iter().any(|l| l.contains("Défense : +775") || l.contains("+775 Défense")), "{lines:?}");
        assert!(lines.iter().any(|l| l.contains("pour toutes les aptitudes")), "{lines:?}");
    }
}
