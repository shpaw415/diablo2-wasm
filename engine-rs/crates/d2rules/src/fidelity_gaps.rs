//! Étape 21a — DIVERS FIDÉLITÉ (PLAN-FIDELITE phase 21) : fermeture des 3
//! gaps consignés par la 13a (`tests/progression_golden.rs` GAPS 6/7/8 +
//! `docs/parity/PLAN-FIDELITE.md` §21). Chaque gap a ici son test de
//! non-régression, verrouillé contre les DONNÉES réelles v1.12 — le module
//! ne porte AUCUNE logique de jeu : documentation d'écarts + gardes.
//!
//! (Provenance : le fichier a été POSÉ EN STUB le 2026-10-25 par la passe
//! 21b parallèle — la déclaration `lib.rs` de 21a avait furtivement cassé la
//! compilation avant l'écriture du corps ; ce corps complet remplace le
//! stub, contrats inchangés, aucun symbole 21b touché.)
//!
//! # GAPS 7 — « Fangskin (H) » : trou de la TABLE retail (écart source, PAS
//! un écart de port)
//!
//! `assets/superuniques.txt:14` (Fangskin, classe `clawviper3`) référence en
//! colonne `TC(H)` le nom **« Fangskin (H) »**, ABSENT de
//! `assets/TreasureClassEx.txt` v1.12 (aucune rangée « Fangskin » du tout —
//! vérifié entrée par entrée par le golden
//! `golden_all_spawned_tcs_exist_with_N_H_variants`, qui attend exactement
//! ce trou). Blizzard a oublié la rangée : trou de la SOURCE, on n'invente
//! PAS la donnée.
//!
//! Le port le gère par le garde `has_tc` ([`crate::loot::Loot::has_tc`])
//! dans `World::roll_drop` :
//!
//! 1. le chemin VIVANT part de la TC de base du superunique (col `TC` =
//!    « Act 2 Super B ») ; en Enfer [`crate::Difficulty::tc_variant`] compose
//!    « Act 2 (H) Super B » — elle EXISTE (TreasureClassEx.txt:771) ⇒ drop
//!    réel, le trou n'est jamais atteint ;
//! 2. si un appelant fournit directement le nom troué (« Fangskin (H) »,
//!    déjà suffixé ⇒ inchangé) : `has_tc` échoue ⇒ repli MVP (60 % rien /
//!    25 % or / 15 % hp1) — **jamais de panic, jamais d'invention** ;
//! 3. une TC de base inconnue suit le même chemin (variante absente ⇒ base,
//!    base absente ⇒ repli MVP).
//!
//! Tests : [`tests::fangskin_hole_guarded_no_panic_fallback`] (cas 2, bout en
//! bout) et [`tests::fangskin_live_path_drops_real_chain`] (cas 1, contrôle
//! : la même mécanique dropne de VRAIS objets quand la variante existe).
//!
//! # GAPS 6 — Andariel : API stats d2rules
//!
//! Hier : stats absentes de d2rules (spawn acte I data-driven d2game,
//! manifest `bosses[]`). Fermé par [`crate::quests_actes2::andariel_stats`]
//! (convention [`crate::quests_actes2::boss_stats`] : `patch_monstats.txt`
//! + ligne `MonLvl.txt` (N) runtime, zéro constante dupliquée) et branché
//! superuniques (rang [`crate::MonKind::SuperUnique`] + TC dédiée
//! « Andariel » → même chaîne de drop à variante retail que les autres boss
//! nommés). Golden : [`tests::andariel_stats_golden_real_tables`]. Le scope
//! de `boss_stats` reste actes II-III (`boss_stats("andariel") == None`).
//!
//! # GAPS 8 — commentaires MonLvl 25/26 (quests_actes2/finale)
//!
//! Constat 13a : les commentaires citaient « HP 98/XP 521/DM 18 » au lieu de
//! **100/470 (ligne 25) et DM 17 (ligne 26)** — le code lit la table, seuls
//! les commentaires dataient. État dans CET arbre : les commentaires
//! portent déjà les vraies valeurs (re-vérifié ligne à ligne contre
//! `MonLvl.txt` — quests_actes2.rs « ligne 22 = 84, ligne 26 = 106,
//! AC 132/156, TH 221/265 » ; quests_finale.rs Hephasto « MonLvl 25 N
//! (150/254/100/16/470) »). Reste à les VERROUILLER pour qu'aucun
//! commentaire ne puisse re-dater :
//! [`tests::monlvl_25_26_comment_claims`] fait égaliser les valeurs
//! COMMENTÉES avec la sortie des APIs réelles (`boss_stats`,
//! `finale_stats`) — si un commentaire re-mente, son test devient rouge.

#[cfg(test)]
mod tests {
    use crate::quests_actes2::{andariel_stats, boss_stats};
    use crate::{Difficulty, GameEvent, MonKind, PlayerInput, World};
    use serde_json::json;

    // -- Helpers (mêmes gabarits que progression_golden, données réelles) --

    /// Grille toute marchable (`false` = marchable, contrat save).
    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// zombie1 RÉEL (`patch_monstats.txt:7`, Code `ZM`) — valeurs BRUTES de
    /// table : le scaling MonLvl (ligne 1 : HP 7 %, DM 2 %) les rend
    /// tuables par le héros BA niveau 1 (convention spawn d2game).
    const ZOMBIE1_MANIFEST: &str = r#"{
        "monsters": [{ "code": "ZM", "name": "Zombie", "stats": {
            "level": 1, "hpMin": 101, "hp": 101, "dmgMin": 51, "dmgMax": 151,
            "exp": 111, "treasureClass": "Act 1 H2H A" } }]
    }"#;

    /// Tue le monstre 0 à coups d'ATTACK et collecte les codes droppés.
    fn kill_and_collect(seed: u64, tc: Option<&str>, hell: bool) -> Vec<String> {
        let manifest: serde_json::Value =
            serde_json::from_str(ZOMBIE1_MANIFEST).expect("manifest");
        let mut w = World::with_seed(open_grid(24, 16), manifest, seed);
        w.clear_monsters();
        if hell {
            w.set_difficulty(Difficulty::Hell);
        }
        let hero = *w.hero();
        let (hx, hy) = (hero.x.floor() as i32, hero.y.floor() as i32);
        let mut rule = crate::SpawnRule::default();
        rule.treasure_class = tc.map(|s| s.to_string());
        let mut spawned = false;
        for (x, y) in [
            (hx + 2, hy),
            (hx - 2, hy),
            (hx, hy + 2),
            (hx, hy - 2),
            (hx + 1, hy),
            (hx - 1, hy - 1),
        ] {
            if w.is_walkable(x, y) && w.spawn_monster_rule("ZM", x, y, rule.clone()) {
                spawned = true;
                break;
            }
        }
        assert!(spawned, "zombie spawné (seed {seed})");
        let mut drops = Vec::new();
        for _ in 0..400 {
            let target =
                w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
            let input = target.map(|(x, y)| PlayerInput::Attack { x, y });
            for ev in w.tick(50, input) {
                if let GameEvent::ItemDropped { item, .. } = ev {
                    drops.push(item.code.clone());
                }
            }
            if w.monsters().is_empty() {
                break;
            }
        }
        drops
    }

    // -- GAPS 7 : garde has_tc sur le trou de table « Fangskin (H) » -------

    /// Le trou de la table v1.12 est géré BOUT EN BOUT : en Enfer, un drop
    /// dont la TC est le nom troué « Fangskin (H) » (col `TC(H)` de
    /// superuniques.txt:14, ABSENT de TreasureClassEx) ne panique PAS et
    /// tombe en repli MVP — jamais un item inventé. Sur 40 seeds on voit
    /// forcément des drops (25 % or + 15 % potion = 40 % par kill) : tous
    /// appartiennent au repli {gld, hp1}.
    #[test]
    fn fangskin_hole_guarded_no_panic_fallback() {
        // La donnée : le nom troué est inconnu du loot embarqué (la base
        // « Fangskin » nue aussi — la table ne connaît AUCUNE rangée
        // Fangskin ; la base vivante du superunique est « Act 2 Super B »).
        let w = World::with_seed(open_grid(24, 16), json!({}), 7);
        let loot = w.loot.as_ref().expect("loot embarqué");
        assert!(!loot.has_tc("Fangskin (H)"), "trou v1.12 : « Fangskin (H) » absente");
        assert!(!loot.has_tc("Fangskin"), "aucune rangée Fangskin dans la table");
        assert!(
            loot.has_tc("Act 2 (H) Super B"),
            "la variante composée de la TC de base vivante existe (TCE:771)"
        );
        // …et le moteur : drops de repli SEULS, aucun panic (le test tourne).
        let mut total = 0;
        for seed in 0..40u64 {
            for code in kill_and_collect(seed, Some("Fangskin (H)"), true) {
                total += 1;
                assert!(
                    code == "gld" || code == "hp1",
                    "seed {seed} : « {code} » hors repli — le trou fuiterait"
                );
            }
        }
        assert!(total >= 4, "{total} drops sur 40 seeds : le repli n'a jamais parlé ?");
    }

    /// Contrôle (cas 1) : la TC de base VIVANTE du même superunique —
    /// « Act 2 Super B » composée en « Act 2 (H) Super B » (existe,
    /// TreasureClassEx.txt:771) — dropne de VRAIS objets en Enfer : le
    /// trou « Fangskin (H) » n'est PAS atteint par le chemin vivant, le
    /// repli MVP n'y masque rien.
    #[test]
    fn fangskin_live_path_drops_real_chain() {
        let mut real = 0;
        for seed in 40..80u64 {
            let drops = kill_and_collect(seed, Some("Act 2 Super B"), true);
            if drops.iter().any(|c| c != "gld" && c != "hp1") {
                real += 1;
            }
        }
        assert!(
            real >= 5,
            "{real}/40 seeds avec un vrai drop : la variante « Act 2 (H) Super B » n'est pas utilisée"
        );
    }

    // -- GAPS 6 : andariel_stats, convention boss_stats + golden tables ----

    /// Golden DONNÉES d'Andariel (GAPS 6) : `andariel_stats` ==
    /// `patch_monstats.txt:158` × `MonLvl.txt` ligne 12 (N) — valeurs
    /// re-vérifiées par awk sur les tables réelles avant le test. Scope :
    /// `boss_stats("andariel")` reste `None` (actes II-III), et la règle
    /// est consommable par `spawn_monster_rule` (branchement superuniques :
    /// HP effectifs 2562 × 40 % = 1024).
    #[test]
    fn andariel_stats_golden_real_tables() {
        // Scope : hors BOSSES_ACTS23 — le golden 13a reste vrai.
        assert!(boss_stats("andariel").is_none(), "boss_stats : scope actes II-III conservé");
        let a = andariel_stats().expect("andariel présent des tables v1.12");
        assert_eq!(a.rec.id, "andariel");
        assert_eq!(a.rec.code, "AN", "code sprite manifest (patch_monstats col Code)");
        assert_eq!(a.rec.name, "Andariel");
        assert_eq!(a.rec.lair, 37, "Catacombes 4 (patch_levels.txt:39)");
        assert_eq!(a.rec.quest_slot, 5, "a1q6 = 6e slot du journal acte I");
        // patch_monstats.txt:158 (Level 12 N, HP 2562, AC 84, Exp 916).
        assert_eq!(a.level, 12);
        assert_eq!(a.hp, 2562);
        assert_eq!(a.ac, 84);
        assert_eq!(a.exp, 916);
        // Résists Normal réels (vides = 0) : [0,0,-50,50,50,80].
        assert_eq!(a.res, [0, 0, -50, 50, 50, 80], "résists réels Andariel N");
        // MonLvl.txt ligne 12 (N) : AC 72, TH 111, HP 40, DM 8, XP 140.
        assert_eq!(a.rule.kind, MonKind::SuperUnique, "branchement superuniques");
        assert_eq!(a.rule.hp_pct, 40);
        assert_eq!(a.rule.dmg_pct, 8);
        assert_eq!(a.rule.th_pct, 111);
        assert_eq!(a.rule.xp_pct, 140);
        assert_eq!(a.rule.ac, 72);
        assert_eq!(a.rule.treasure_class.as_deref(), Some("Andariel"), "TC dédiée boss");
        // Consommation réelle : la règle spawnne un boss aux stats composées
        // (2562 × 40 % = 1024 PV effectifs, AC 72 de la ligne MonLvl).
        let manifest = json!({
            "monsters": [{ "code": "AN", "name": "Andariel", "stats": {
                "level": 12, "hpMin": 2562, "hp": 2562, "dmgMin": 76,
                "dmgMax": 238, "exp": 916, "treasureClass": "Andariel" } }]
        });
        let mut w = World::with_seed(open_grid(24, 16), manifest, 11);
        w.clear_monsters(); // l'auto-spawn manifest (facteurs défaut) sort
        assert!(
            w.spawn_monster_rule("AN", 4, 4, a.rule),
            "règle andariel_stats consommable par spawn_monster_rule"
        );
        let s: serde_json::Value = serde_json::from_str(&w.save()).expect("save JSON");
        let m = &s["monsters"][0];
        assert_eq!(m["hp"].as_u64(), Some(1024), "2562 × 40 % (MonLvl 12 N)");
        assert_eq!(m["max_hp"].as_u64(), Some(1024));
        assert_eq!(m["ac"].as_u64(), Some(72));
    }

    // -- GAPS 8 : les commentaires MonLvl 25/26 ne re-dateront pas ---------

    /// Les valeurs portées par les commentaires corrigés (quests_actes2.rs
    /// en-tête « ligne 22 = 84, ligne 26 = 106, AC 132/156, TH 221/265 » ;
    /// quests_finale.rs Hephasto « MonLvl 25 N (150/254/100/16/470) ») sont
    /// EXACTEMENT celles que sortent les APIs qui lisent la table — le
    /// commentaire ne peut re-mentir sans rendre ce test rouge :
    /// MonLvl 25 N = (150, 254, 100, 16, 470) et 26 N = (156, 265, 106,
    /// 17, 503) [« DM 18 » du constat 13a corrigé en 17].
    #[test]
    fn monlvl_25_26_comment_claims() {
        // quests_actes2 : Duriel (ligne 22) + Mephisto (ligne 26) via boss_stats.
        let d = boss_stats("duriel").expect("duriel des tables");
        assert_eq!((d.rule.hp_pct, d.rule.dmg_pct), (84, 14), "ligne 22 commentée");
        assert_eq!((d.rule.ac, d.rule.th_pct), (132, 221));
        let m = boss_stats("mephisto").expect("mephisto des tables");
        assert_eq!((m.rule.hp_pct, m.rule.dmg_pct), (106, 17), "ligne 26 : DM 17 (pas 18)");
        assert_eq!((m.rule.ac, m.rule.th_pct), (156, 265));
        // quests_finale : Hephasto (ligne 25) via finale_stats — 150/254/100/16/470.
        let h = crate::quests_finale::finale_stats("hephasto").expect("hephasto des tables");
        assert_eq!(h.level, 25);
        assert_eq!((h.rule.hp_pct, h.rule.dmg_pct), (100, 16), "ligne 25 : HP 100 (pas 98)");
        assert_eq!((h.rule.ac, h.rule.th_pct), (150, 254));
        assert_eq!(h.rule.xp_pct, 470, "ligne 25 : XP 470 (pas 521)");
    }
}
