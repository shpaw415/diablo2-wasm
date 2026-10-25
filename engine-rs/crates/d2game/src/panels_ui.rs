//! Panneaux UI fidèles 18b (PLAN-FIDELITE phase 18) — wire MINIMAL des
//! endpoints JSON que consomme `demo/panels-ui.js` (même pattern additif que
//! `npc_bridge.rs`/`consumables_ui.rs` : un `impl GameCore` séparé, zéro
//! changement du flux historique).
//!
//! * [`GameCore::hero_panel_json`] — panneau PERSONNAGE complet, fr :
//!   str/dex/vit/ene (gabarit manifest + HeroStats), dégâts DÉRIVÉS de
//!   l'arme de départ (même formule que le combat `world.rs` :
//!   `base × (1 + str/100)`), défense, toucher, résists 4 + pénalité de
//!   difficulté (`patch_difficultylevels.txt` v1.12 : 0/−40/−100) + max 75,
//!   MF/GF (0 : aucun item MF/GF au MVP — écart documenté), or + plafond
//!   porté (`economy::carry_cap` 10 000 × niveau), XP/niveau + endurance,
//!   points à dépenser.
//! * [`GameCore::stash_json`] — panneau STASH 6×4 (inventory.txt
//!   « Bank Page 1 ») : or déposé + plafond de difficulté
//!   (`economy::stash_gold_cap` 300k/500k/800k), items déposés avec leur
//!   taille réelle (`EquipItem.w/h` = invwidth/invheight des tables) et
//!   leur position (première cellule de la grille partagée).
//! * [`GameCore::stash_deposit_gold`] / [`GameCore::stash_withdraw_gold`] —
//!   dépôt/retrait d'or via le branchement `d2rules` 11c EXISTANT
//!   (`World::stash_deposit_gold`/`stash_withdraw_gold`, economy.rs fait
//!   foi) ; erreurs lisibles en fr, aucun effet de bord partiel.
//!
//! Lecture seule : `equipment.rs`/`statlist.rs`/`economy.rs`/`inventory.rs`
//! (d2rules) ne sont PAS modifiés — ce fichier ne fait que composer leurs
//! sorties publiques en JSON d'UI.

use serde_json::{json, Value};

use crate::game::GameCore;
use d2rules::economy::EconError;
use d2rules::Difficulty;

/// 18b : Pénalité de résistance par difficulté (`patch_difficultylevels.txt`
/// v1.12, colonne `ResistPenalty` : Normal 0, Nightmare −40, Hell −100).
fn resist_penalty(d: Difficulty) -> i32 {
    match d {
        Difficulty::Normal => 0,
        Difficulty::Nightmare => -40,
        Difficulty::Hell => -100,
    }
}

impl GameCore {
    /// Panneau PERSONNAGE complet (18b) — JSON pour `demo/panels-ui.js` :
    /// `{"class","className","level","xp","xpToNext","life","maxLife","mana",
    /// "maxMana","strength","dexterity","vitality","energy","defense",
    /// "toHit","dmgMin","dmgMax","weapon","resFire","resLight","resCold",
    /// "resPoison","resMax","mf","gf","gold","goldCap","stamina",
    /// "maxStamina","statPoints","skillPoints"}` ; `{}` avant `finalize`.
    pub fn hero_panel_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "{}".into() };
        let s = w.hero_stats();
        let Ok(hc) = serde_json::from_str::<Value>(&self.hero_class_json()) else {
            return "{}".into();
        };
        let code = hc["code"].as_str().unwrap_or("BA").to_string();
        // Gabarit manifest (bloc classes `heroes.<code>.stats`, repli `hero.stats`) :
        // vitalité/énergie ne baissent jamais au MVP (aucun dépense de points
        // câblée côté moteur) — valeurs de base de la classe.
        let ms = |key: &str| -> Value {
            self.manifest_value()
                .pointer(&format!("/heroes/{code}/stats/{key}"))
                .or_else(|| self.manifest_value().pointer(&format!("/hero/stats/{key}")))
                .cloned()
                .unwrap_or(Value::Null)
        };
        // Dégâts dérivés de l'ÉQUIPEMENT porté : arme de départ du gabarit
        // (le branchement Equipment→combat reste à faire, cf. full_playthrough
        // §stub) — MÊME formule que le combat réel (`world.rs` :
        // `base × (1 + str/100)` plancher) et MÊMES défauts que `data.rs`
        // (`u32_at` : null/absent ⇒ 3..6, comme le gabarit du combat).
        let at = |key: &str, d: u64| -> u64 {
            self.manifest_value()
                .pointer(&format!("/heroes/{code}/stats/{key}"))
                .or_else(|| self.manifest_value().pointer(&format!("/hero/stats/{key}")))
                .and_then(|x| x.as_u64())
                .unwrap_or(d)
        };
        let (wmin, wmax) = (at("startWeapon/dmgMin", 3) as f32, at("startWeapon/dmgMax", 6) as f32);
        let wname = ms("startWeapon/name").as_str().unwrap_or("").to_string();
        let str_f = 1.0 + s.strength as f32 / 100.0;
        let dmg_min = (wmin * str_f).floor() as u64;
        let dmg_max = (wmax * str_f).floor() as u64;
        // Résists : base 0 + pénalité de difficulté (réel v1.12) ; max +75 %.
        let pen = resist_penalty(w.difficulty());
        json!({
            "class": code,
            "className": hc["name"],
            "level": s.level,
            "xp": s.xp,
            "xpToNext": s.xp_to_next,
            "life": s.life,
            "maxLife": s.max_life,
            "mana": s.mana,
            "maxMana": s.max_mana,
            "strength": s.strength,
            "dexterity": s.dexterity,
            "vitality": ms("vitality"),
            "energy": ms("energy"),
            "defense": s.defense,
            "toHit": s.to_hit,
            "dmgMin": dmg_min,
            "dmgMax": dmg_max,
            "weapon": wname,
            "resFire": pen,
            "resLight": pen,
            "resCold": pen,
            "resPoison": pen,
            "resMax": 75,
            "mf": 0,
            "gf": 0,
            "gold": s.gold,
            "goldCap": d2rules::economy::carry_cap(s.level),
            "stamina": s.stamina,
            "maxStamina": s.max_stamina,
            "statPoints": s.stat_points,
            "skillPoints": s.skill_points,
        })
        .to_string()
    }

    /// Panneau STASH 6×4 (18b) — JSON pour `demo/panels-ui.js` :
    /// `{"gold","goldCap","carryCap","difficulty","w":6,"h":4,"items":[
    /// {"index","code","name","w","h","x","y"},…]}` ; `{}` avant `finalize`.
    /// Positions : première cellule de l'item dans la grille partagée
    /// (`Stash.grid`, balayage rangée par rangée — inventory.rs).
    pub fn stash_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "{}".into() };
        let st = w.stash();
        let diff = w.difficulty();
        let mut items = Vec::new();
        for (idx, slot) in st.items.iter().enumerate() {
            let Some(it) = slot else { continue };
            // Première cellule occupée par cet index = coin haut-gauche.
            let pos = st
                .grid
                .occupied
                .iter()
                .position(|c| *c == Some(idx))
                .map(|p| (p % st.grid.w, p / st.grid.w))
                .unwrap_or((0, 0));
            items.push(json!({
                "index": idx,
                "code": it.code,
                "name": it.name,
                "w": it.inv_w,
                "h": it.inv_h,
                "x": pos.0,
                "y": pos.1,
            }));
        }
        json!({
            "gold": st.gold,
            "goldCap": d2rules::economy::stash_gold_cap(diff),
            "carryCap": d2rules::economy::carry_cap(w.hero_stats().level),
            "difficulty": diff.as_str(),
            "w": st.grid.w,
            "h": st.grid.h,
            "items": items,
        })
        .to_string()
    }

    /// Dépôt d'or au stash (18b) — branchement economy.rs réel
    /// ([`d2rules::World::stash_deposit_gold`] : bourse débitée, plafond de
    /// difficulté). Renvoie le [`GameCore::stash_json`] à jour ; `Err` fr
    /// lisible, aucun effet de bord partiel.
    pub fn stash_deposit_gold(&mut self, amount: u32) -> Result<String, String> {
        let w = self.world.as_mut().ok_or("finalize d'abord")?;
        w.stash_deposit_gold(amount).map_err(|e| econ_fr(&e))?;
        Ok(self.stash_json())
    }

    /// Retrait d'or du stash (18b) — branchement economy.rs réel
    /// ([`d2rules::World::stash_withdraw_gold`] : plafond porté
    /// 10 000 × niveau). Renvoie le [`GameCore::stash_json`] à jour.
    pub fn stash_withdraw_gold(&mut self, amount: u32) -> Result<String, String> {
        let w = self.world.as_mut().ok_or("finalize d'abord")?;
        w.stash_withdraw_gold(amount).map_err(|e| econ_fr(&e))?;
        Ok(self.stash_json())
    }
}

/// Erreurs economy.rs → messages fr du panneau (EconError n'a pas de Display).
fn econ_fr(e: &EconError) -> String {
    match *e {
        EconError::NotEnoughGold { need, have } => {
            format!("Or insuffisant : {need} requis, {have} disponible.")
        }
        EconError::CarryGoldFull { cap } => {
            format!("Bourse pleine : plafond {cap} or (10 000 × niveau).")
        }
        EconError::StashGoldFull { cap } => {
            format!("Stash plein : plafond {cap} or à cette difficulté.")
        }
        EconError::StashFull => "Stash plein (grille 6×4).".into(),
        EconError::ItemTooBig { w, h } => {
            format!("Objet {w}×{h} : ne tiendra jamais dans la page 6×4.")
        }
        EconError::BadSlot(i) => format!("Emplacement de stash invalide ({i})."),
        EconError::NothingToRepair => "Rien à réparer.".into(),
        EconError::EtherealNotRepairable => "Objet éthéré : jamais réparable.".into(),
    }
}
