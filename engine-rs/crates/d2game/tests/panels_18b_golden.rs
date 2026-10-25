//! Golden tests — panneaux UI fidèles 18b (PLAN-FIDELITE phase 18).
//!
//! Les JSON consommés par `demo/panels-ui.js` sont verrouillés sur les
//! données v1.12 réelles :
//!
//! * `heroPanel` (panneau PERSONNAGE) : stats COMPLÈTES non vides pour les
//!   7 classes — str/dex/vit/ene du gabarit (`patch_charstats`), dégâts
//!   dérivés de l'arme de départ (formule combat `base × (1 + str/100)`,
//!   défauts `data.rs` 3..6 quand le manifest porte null), défense
//!   (dex/4 + AC bouclier), toucher (ToHitFactor + dex), résists 4 +
//!   pénalité de difficulté (`patch_difficultylevels.txt` : 0/−40/−100) +
//!   max 75, MF/GF, or + plafond porté (`economy::carry_cap`), XP/niveau,
//!   endurance, points à dépenser.
//! * `stashInfo` + `stashDeposit`/`stashWithdraw` (panneau STASH 6×4) :
//!   branchement economy.rs réel — plafonds 300k/500k/800k par difficulté,
//!   plafond porté 10 000 × niveau, erreurs fr sans effet de bord partiel,
//!   cohérence croisée or (panneau perso ↔ stash après dépôt/retrait).

mod common;

use d2game::game::GameCore;
use serde_json::Value;

/// Tous les assets "assets/…" du manifest + tables moteur optionnelles.
fn provide_all(core: &mut GameCore, root: &str, manifest: &Value) {
    let mut paths: Vec<String> = Vec::new();
    fn walk(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => {
                if s.starts_with("assets/") {
                    out.push(s.clone());
                }
            }
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            Value::Object(o) => o.values().for_each(|x| walk(x, out)),
            _ => {}
        }
    }
    walk(manifest, &mut paths);
    for p in [
        "assets/patch_levels.txt",
        "assets/patch_monstats.txt",
        "assets/monumod.txt",
        "assets/superuniques.txt",
        "assets/MonLvl.txt",
    ] {
        paths.push(p.to_string());
    }
    for p in paths {
        if let Ok(bytes) = std::fs::read(format!("{root}/{p}")) {
            core.provide(&p, bytes);
        }
    }
}

fn boot(root: &str, manifest: &str, hero: &str, diff: &str) -> GameCore {
    let mut core = GameCore::with_level(manifest, hero, "town").expect("boot GameCore");
    if diff != "normal" {
        core.set_difficulty(d2rules::Difficulty::parse(diff));
    }
    let m: Value = serde_json::from_str(manifest).unwrap();
    provide_all(&mut core, root, &m);
    core.finalize().expect("finalize");
    core
}

fn panel(gc: &GameCore) -> Value {
    serde_json::from_str(&gc.hero_panel_json()).expect("heroPanel JSON")
}

fn stash(gc: &GameCore) -> Value {
    serde_json::from_str(&gc.stash_json()).expect("stashInfo JSON")
}

/// Force l'or porté via le save moteur (levier de test, save v3 `hero.stats.gold`).
fn set_gold(gc: &mut GameCore, gold: u64) {
    let mut save: Value = serde_json::from_str(&gc.save_world()).expect("save JSON");
    save["hero"]["stats"]["gold"] = gold.into();
    gc.load_world(&save.to_string()).expect("load world");
}

// ---------------------------------------------------------------------------
// Panneau PERSONNAGE — golden BA niveau 1 (valeurs exactes patch_charstats)
// ---------------------------------------------------------------------------

#[test]
fn hero_panel_golden_ba_level1() {
    let Some(root) = common::ws_root() else { return };
    let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let gc = boot(&root, &manifest, "BA", "normal");
    let p = panel(&gc);

    // Toutes les clés du panneau présentes (JSON complet, jamais tronqué).
    for k in [
        "class", "className", "level", "xp", "xpToNext", "life", "maxLife", "mana", "maxMana",
        "strength", "dexterity", "vitality", "energy", "defense", "toHit", "dmgMin", "dmgMax",
        "weapon", "resFire", "resLight", "resCold", "resPoison", "resMax", "mf", "gf", "gold",
        "goldCap", "stamina", "maxStamina", "statPoints", "skillPoints",
    ] {
        assert!(p.get(k).is_some_and(|v| !v.is_null()), "champ {k} absent/null");
    }
    // Golden exact niveau 1 — Barbarie (patch_charstats + startWeapon hax 3-6,
    // bouclier buc 4-6) : defense = 20/4 + (4+6)/2 = 10 ; toHit = 20 + 20 = 40 ;
    // dégâts dérivés = floor(3×1,3)..floor(6×1,3) = 3..7.
    assert_eq!(p["class"], "BA");
    assert_eq!(p["className"], "Barbare");
    assert_eq!(p["level"], 1);
    assert_eq!(p["xpToNext"], 500, "seuil XP niv 2 (patch_experience)");
    assert_eq!(p["life"], 55);
    assert_eq!(p["maxLife"], 55);
    assert_eq!(p["mana"], 15);
    assert_eq!(p["maxMana"], 15);
    assert_eq!(p["strength"], 30);
    assert_eq!(p["dexterity"], 20);
    assert_eq!(p["vitality"], 25);
    assert_eq!(p["energy"], 10);
    assert_eq!(p["defense"], 10);
    assert_eq!(p["toHit"], 40);
    assert_eq!(p["dmgMin"], 3, "dégâts dérivés de l'arme (×1,3 str 30)");
    assert_eq!(p["dmgMax"], 7);
    assert_eq!(p["weapon"], "Hand Axe");
    // Résists Normal : 0 partout, max +75 (D2 réel).
    for k in ["resFire", "resLight", "resCold", "resPoison"] {
        assert_eq!(p[k], 0, "{k} Normal");
    }
    assert_eq!(p["resMax"], 75);
    assert_eq!(p["mf"], 0);
    assert_eq!(p["gf"], 0);
    assert_eq!(p["gold"], 0);
    assert_eq!(p["goldCap"], 10000, "plafond porté 10 000 × niveau");
    assert_eq!(p["stamina"], 92);
    assert_eq!(p["maxStamina"], 92);
}

/// Stats COMPLÈTES non vides pour les 7 classes — dérivées cohérentes avec
/// le gabarit réel de chacune (str/vit/ene/dex + arme de départ).
#[test]
fn hero_panel_complete_non_empty_all_7_classes() {
    let Some(root) = common::ws_root() else { return };
    let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    // (classe, str, dex, vit, ene, armMin, armMax, life, mana) — manifest réel.
    const G: [(&str, u64, u64, u64, u64, u64, u64, u64, u64); 7] = [
        ("BA", 30, 20, 25, 10, 3, 7, 55, 15),
        ("SOR", 10, 25, 10, 35, 3, 6, 40, 35), // sst null ⇒ défauts 3..6 (data.rs)
        ("PA", 25, 20, 25, 15, 2, 8, 55, 15),  // ssd 2-7 ×1,25
        ("AM", 20, 25, 20, 15, 1, 6, 50, 15),  // jav 1-5 ×1,2
        ("AS", 20, 20, 20, 25, 4, 8, 50, 25),  // ktr 4-7 ×1,2
        ("DZ", 15, 20, 25, 20, 1, 6, 55, 20),  // clb 1-6 ×1,15
        ("NE", 15, 25, 15, 25, 2, 4, 45, 25),  // wnd 2-4 ×1,15
    ];
    for (code, str_, dex, vit, ene, dmin, dmax, life, mana) in G {
        let gc = boot(&root, &manifest, code, "normal");
        let p = panel(&gc);
        assert_eq!(p["class"], code);
        assert_eq!(p["strength"], str_, "{code} str");
        assert_eq!(p["dexterity"], dex, "{code} dex");
        assert_eq!(p["vitality"], vit, "{code} vit");
        assert_eq!(p["energy"], ene, "{code} ene");
        assert_eq!(p["dmgMin"], dmin, "{code} dmgMin (arme × str)");
        assert_eq!(p["dmgMax"], dmax, "{code} dmgMax (arme × str)");
        assert!(p["dmgMax"].as_u64() >= p["dmgMin"].as_u64(), "{code} dmg max>=min");
        assert!(p["defense"].as_u64().is_some(), "{code} defense présente");
        assert!(p["toHit"].as_u64().is_some_and(|v| v > 0), "{code} toHit non vide");
        assert_eq!(p["life"], life, "{code} life");
        assert_eq!(p["mana"], mana, "{code} mana");
        assert!(p["maxStamina"].as_u64().is_some_and(|v| v > 0), "{code} endurance");
        assert!(p["className"].as_str().is_some_and(|s| !s.is_empty()), "{code} nom fr");
    }
}

/// Pénalité de résistance par difficulté (patch_difficultylevels.txt réel :
/// Normal 0, Nightmare −40) + plafond stash NM 500 000.
#[test]
fn hero_panel_resist_penalty_by_difficulty() {
    let Some(root) = common::ws_root() else { return };
    let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let mut gc = GameCore::with_level(&manifest, "BA", "town").unwrap();
    gc.set_difficulty(d2rules::Difficulty::parse("nm"));
    let m: Value = serde_json::from_str(&manifest).unwrap();
    provide_all(&mut gc, &root, &m);
    gc.finalize().unwrap();
    let p = panel(&gc);
    for k in ["resFire", "resLight", "resCold", "resPoison"] {
        assert_eq!(p[k], -40, "{k} Nightmare (ResPenalty réel)");
    }
    let s = stash(&gc);
    assert_eq!(s["goldCap"], 2500000, "plafond stash Nightmare (economy.rs, étape 19b : cap interop .d2s unique)");
    assert_eq!(s["difficulty"], "nightmare");
}

// ---------------------------------------------------------------------------
// Panneau STASH — branchement economy.rs réel (dépôt/retrait, plafonds, fr)
// ---------------------------------------------------------------------------

#[test]
fn stash_golden_deposit_withdraw_roundtrip() {
    let Some(root) = common::ws_root() else { return };
    let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let mut gc = boot(&root, &manifest, "BA", "normal");

    // État initial : stash vide 6×4, or 0, plafond Normal 300 000.
    let s = stash(&gc);
    assert_eq!(s["gold"], 0);
    assert_eq!(s["w"], 6, "Bank Page 6 colonnes (inventory.txt)");
    assert_eq!(s["h"], 4, "Bank Page 4 rangées");
    assert_eq!(s["goldCap"], 2500000, "plafond stash Normal (economy.rs, étape 19b : cap interop .d2s unique)");
    assert_eq!(s["carryCap"], 10000, "plafond porté niveau 1");
    assert_eq!(s["items"], Value::Array(vec![]));

    // Dépôt sans or : refus net, AUCUN effet de bord, message fr.
    let err = gc.stash_deposit_gold(100).unwrap_err();
    assert!(err.contains("insuffisant"), "erreur fr : {err}");
    assert_eq!(stash(&gc)["gold"], 0, "aucun dépôt partiel");

    // Or réel via save → dépôt 1 000 : bourse débitée, stash crédité.
    set_gold(&mut gc, 5000);
    let s = gc.stash_deposit_gold(1000).unwrap();
    let s: Value = serde_json::from_str(&s).unwrap();
    assert_eq!(s["gold"], 1000, "stash après dépôt");
    assert_eq!(panel(&gc)["gold"], 4000, "bourse débitée (cohérence cross-panneau)");

    // Retrait supérieur au stash : refus.
    let err = gc.stash_withdraw_gold(3000).unwrap_err();
    assert!(err.contains("insuffisant"), "erreur fr : {err}");

    // Dépôt au-delà du plafond : refus, état inchangé. Plafond interop 19b
    // = 2 500 000 (fait de format .d2s : README oracle + javazon goldbank) —
    // le remplissage boucle par boucle (10 000/tour) ne l'atteint plus : le
    // stash est pré-rempli près du cap par le levier save (même mécanique
    // que set_gold), puis UN dépôt déborde.
    {
        let mut save: Value = serde_json::from_str(&gc.save_world()).unwrap();
        save["gold_stash"]["gold"] = (2_500_000 - 5_000).into();
        gc.load_world(&save.to_string()).expect("load world");
    }
    set_gold(&mut gc, 10000);
    let full_err = gc.stash_deposit_gold(10000).unwrap_err();
    assert!(full_err.contains("2500000") && full_err.contains("Stash"), "erreur fr : {full_err}");
    let near_cap = stash(&gc)["gold"].as_u64().unwrap();
    assert_eq!(near_cap, 2495000, "stash intact après refus (2 500 000 − 5 000)");

    // Bourse AU PLEIN plafond : retrait refusé (10 000 × niveau 1), stash
    // et bourse intacts.
    set_gold(&mut gc, 10000);
    let err = gc.stash_withdraw_gold(1).unwrap_err();
    assert!(err.contains("10000") && err.contains("Bourse"), "erreur fr : {err}");
    assert_eq!(stash(&gc)["gold"], near_cap, "aucun retrait partiel");
    assert_eq!(panel(&gc)["gold"], 10000, "bourse au plafond intacte");

    // Retrait au-delà de la place portée : REFUS SANS EFFET DE BORD
    // (economy.rs::withdraw_gold — « rien ne bouge sinon », pas de retrait
    // partiel ; validé contre l'impl réelle, le doc du test initial était
    // faux). Puis retrait exact de la place restante : accepté.
    set_gold(&mut gc, 5000);
    let err = gc.stash_withdraw_gold(100_000).unwrap_err();
    assert!(err.contains("10000") && err.contains("Bourse"), "erreur fr : {err}");
    assert_eq!(stash(&gc)["gold"], near_cap, "stash intact après refus");
    let s: Value = serde_json::from_str(&gc.stash_withdraw_gold(5000).unwrap()).unwrap();
    assert_eq!(s["gold"], near_cap - 5000, "reliquat au stash");
    assert_eq!(panel(&gc)["gold"], 10000, "bourse re-pleine au plafond");
}

#[test]
fn stash_items_shape_contract() {
    let Some(root) = common::ws_root() else { return };
    let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let gc = boot(&root, &manifest, "BA", "normal");
    let s = stash(&gc);
    let items = s["items"].as_array().expect("items tableau");
    for it in items {
        for k in ["index", "code", "name", "w", "h", "x", "y"] {
            assert!(it.get(k).is_some_and(|v| !v.is_null()), "item sans {k}");
        }
        let (w, h) = (it["w"].as_u64().unwrap(), it["h"].as_u64().unwrap());
        assert!(w >= 1 && w <= 6 && h >= 1 && h <= 4, "taille {w}×{h} hors page 6×4");
        assert!(it["x"].as_u64().unwrap() + w <= 6, "dépasse la largeur");
        assert!(it["y"].as_u64().unwrap() + h <= 4, "dépasse la hauteur");
    }
}
