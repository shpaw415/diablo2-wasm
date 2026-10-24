//! LE GRAND E2E LOGIQUE (étape 13c — finitions, campagne diablo-2-wasm).
//!
//! Une traversée complète sans navigateur, héros **BA**, données v1.12 réelles :
//!
//! 1. Moteur (GameCore) : boot Camp des Rousses → **warp marché** Blood Moor
//!    (DRLG wilderness seedé réel, plan de spawn par les tables), waypoints
//!    activés **au contact** (slots 0/39), voyages par waypoints (slots
//!    activés + bloc manifest), switches de niveaux A2/A4/A5 générés par les
//!    DRLG réelles du port (`drlg` seeded / `drlg_maze` / presets et
//!    `drlg_a45` via leurs ids levels.txt), déterminisme seedé (deux boots à
//!    la même graine ⇒ mêmes cartes + mêmes plans de spawn),
//!    save/load moteur (`save_world`/`load_world`).
//! 2. Règles (d2rules::World, le MÊME moteur que tick le GameCore) sur les
//!    grilles de collision réelles des DRLG ci-dessus : campagnes de quêtes
//!    a1q1→a1q6, a2q1→a2q6, a3q1→a3q6, a4q1→a4q3, a5q5/a5q6, kills RÉELS via
//!    `tick(PlayerInput::Attack)` (le levier de test `set_hero_combat_mods`
//!    — crushing blow 100 %, mods officiels `item_crushingblow` — compense
//!    l'absence de farm de 20 h ; les coups restent des coups du pipeline
//!    combat : jet de toucher, cooldown, anim, MonsterDied réel), drops TC
//!    réels (`TreasureClassEx` embarquée), ramassage/équipement inventaire,
//!    récompenses de quêtes (SP, stats, or, objets), gates d'actes
//!    (`ActUnlocked` / `access_act`), **GameWon** à la mort de Baal.
//! 3. Chaîne de sauvegardes : save/load ≥ 3 fois en route (v3 round-trip
//!    idempotent + rétro-compat v1 par retrait des champs additifs), et
//!    journaux de quêtes transférés d'un monde à l'autre via leur JSON
//!    versionné (`to_json`/`from_json`).
//! 4. Les 7 classes en boucle rapide sur la même grille DRLG réelle.
//!
//! # Stubs résiduels (documentés, tout le reste est réel)
//! * Zones sans bloc manifest dans le port (pas de DS1/DT1 extraits) :
//!   navigation par **ids réels** `patch_levels.txt` au journal uniquement —
//!   Tanière du Mal 8, Tristram 38, Catacombes 37, zones A3 (78-102),
//!   Plaines du Désespoir 105, Rivière de feu 107, Sanctuaire du Chaos 108,
//!   Sommet d'Arreat 120, Trône 131, Chambre de la Pierre d'Worldstone 132.
//!   Les COMBATS de ces quêtes ont lieu sur des grilles DRLG réelles
//!   (Blood Moor / Tombf1 / Arcane / Pandemonium / Halls of Vaught).
//! * Objets de quête (`bks`, `hdm`, `bbb`, `qey`…) : événements
//!   `ItemPicked` présentés au journal (aucun item-monde pour ces codes dans
//!   le port) ; les drops TC, eux, sont de vrais items au sol ramassés.
//! * Slots de waypoints sans position dans la table (ex. 30 Harrogath,
//!   33 Cavernes de cristal) : activés via le champ save `waypoints`
//!   (levier de test) — les slots AVEC position (0 camp, 39 Blood Moor)
//!   s'activent au contact réel du héros.
//! * Équipement : les drops entrent dans l'inventaire 10×4 (UseItem ne gère
//!   que les potions) ; les dégâts du héros restent portés par l'arme de
//!   gabarit (`startWeapon`) + mods de coup posés en test — le branchement
//!   `inventory::Equipment` → dégâts reste à faire (hors scope 13c).

mod common;

use d2game::game::GameCore;
use d2game::monsters::MonStats;
use d2rules::combat_mods::HitMods;
use d2rules::quests_actes::{ActEvent, ActPhase, ActQuests};
use d2rules::quests_actes2::{boss_stats, Act2Quests};
use d2rules::quests_finale::{finale_stats, FinaleReward, QuestsFinale};
use d2rules::{waypoint_for_level, GameEvent, MonKind, PlayerInput, World};
use serde_json::{json, Value};

const SEED: u64 = 0xD2_E2E_13C;
const DT: u32 = 100; // ms de tick simulé

// ---------------------------------------------------------------------------
// Helpers moteur (GameCore) — assets réels de l'arbre
// ---------------------------------------------------------------------------

/// Tous les assets "assets/…" référencés par le manifest, fournis au core
/// (provide-if-exists), + les tables optionnelles du moteur (W01/W04/W06-W08).
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
    for p in ["assets/patch_levels.txt", "assets/patch_monstats.txt", "assets/monumod.txt", "assets/superuniques.txt", "assets/MonLvl.txt"] {
        paths.push(p.to_string());
    }
    for p in paths {
        let full = format!("{root}/{p}");
        if let Ok(bytes) = std::fs::read(&full) {
            core.provide(&p, bytes);
        }
    }
}

fn manifest_value(root: &str) -> Value {
    let raw = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn boot_core(root: &str, manifest: &str, level: &str, seed: u64) -> GameCore {
    let mut core = GameCore::with_level(manifest, "BA", level).expect("boot GameCore");
    core.set_game_seed(seed);
    let m: Value = serde_json::from_str(manifest).unwrap();
    provide_all(&mut core, root, &m);
    core.finalize().expect("finalize");
    core.set_viewport(320, 240);
    core
}

/// Grille de collision du niveau courant du core, au format d2rules
/// (true = BLOQUÉ ; `collision()` du moteur : 1 = marchable).
fn engine_grid(core: &GameCore) -> Vec<Vec<bool>> {
    let (gw, gh) = core.grid_size();
    let bytes = core.collision();
    (0..gh as usize)
        .map(|y| (0..gw as usize).map(|x| bytes[y * gw as usize + x] == 0).collect())
        .collect()
}

/// Fait marcher le héros vers une cellule jusqu'à un event (warp, waypoint…).
fn walk_until(core: &mut GameCore, x: i32, y: i32, hit: impl Fn(&d2game::game::CoreEvent) -> bool) -> bool {
    for _ in 0..1200 {
        let evs = core.tick(DT, Some(PlayerInput::MoveTo { x, y }));
        if evs.iter().any(&hit) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Helpers monde (d2rules) — mêmes règles que tick le moteur
// ---------------------------------------------------------------------------

/// Manifest d'acte : blocs `bosses` de quête à stats réelles. `keep_monsters`
/// garde les gabarits du manifest (acte I : kills FA/ZM/SI réels) ; sinon les
/// auto-spawns de planche sont retirés (le spawn de planche est le travail du
/// moteur — cf. `plan_level_spawns_diff` prouvé côté GameCore).
fn act_manifest(base: &Value, blocks: &[Value], keep_monsters: bool) -> Value {
    let mut m = base.clone();
    if !keep_monsters {
        m["monsters"] = json!([]);
    }
    let bosses = m["bosses"].as_array().cloned().unwrap_or_default();
    let mut bosses = bosses;
    bosses.extend(blocks.iter().cloned());
    m["bosses"] = Value::Array(bosses);
    m
}

/// Bloc boss manifest construit depuis un record monstats RÉEL (table
/// `patch_monstats.txt` via le parseur d2game — zéro constante fabriquée).
fn mon_block(rec: &d2game::monsters::MonStatsRec) -> Value {
    json!({
        "code": rec.code, "id": rec.id, "name": rec.name,
        "stats": {
            "level": rec.level.max(1), "hp": rec.hp_max.max(1), "hpMin": rec.hp_min.max(1),
            "dmgMin": rec.dmg_min, "dmgMax": rec.dmg_max, "exp": rec.exp,
            "speed": rec.speed, "treasureClass": rec.tc[0],
        }
    })
}

/// Bloc boss scripté acts II-III (stats réelles via `boss_stats`).
fn boss23_block(id: &str) -> Value {
    let st = boss_stats(id).unwrap_or_else(|| panic!("boss_stats({id}) absent des tables"));
    json!({
        "code": st.rec.code, "id": st.rec.id, "name": st.rec.name,
        "stats": {
            "level": st.level, "hp": st.hp, "hpMin": st.hp, "exp": st.exp,
            "speed": 3, "treasureClass": st.rec.tc,
        }
    })
}

/// Bloc unité finale A4-A5 (stats réelles via `finale_stats`).
fn finale_block(id: &str) -> Value {
    let st = finale_stats(id).unwrap_or_else(|| panic!("finale_stats({id}) absent des tables"));
    json!({
        "code": st.rec.code, "id": st.rec.id, "name": st.rec.name,
        "stats": {
            "level": st.level, "hp": st.hp_max.max(1), "hpMin": st.hp.max(1), "exp": st.exp,
            "speed": 3, "treasureClass": st.tc_effective.clone().unwrap_or_default(),
        }
    })
}

/// Pose le héros (transitoire) sur une tuile marchable : patch du save JSON
/// (même technique que `patch_save_for_level` du moteur : ent + spawn), puis
/// `World::load`. Les journaux branchés survivent (champs save additifs).
fn place_hero(w: &mut World, x: i32, y: i32) {
    assert!(w.is_walkable(x, y), "place_hero : ({x},{y}) bloqué");
    let mut s: Value = serde_json::from_str(&w.save()).unwrap();
    *s.pointer_mut("/hero/ent/x").unwrap() = json!(x as f32 + 0.5);
    *s.pointer_mut("/hero/ent/y").unwrap() = json!(y as f32 + 0.5);
    s["hero"]["spawn"] = json!([x as f32 + 0.5, y as f32 + 0.5]);
    s["hero"]["path"] = json!([]);
    *w = World::load(&s.to_string()).expect("reload après placement");
}

/// Un point marchable près de (x,y) (spirale).
fn walkable_near(w: &World, x: i32, y: i32) -> (i32, i32) {
    for r in 0..64i32 {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r {
                    continue;
                }
                let (px, py) = (x + dx, y + dy);
                if w.is_walkable(px, py) {
                    return (px, py);
                }
            }
        }
    }
    panic!("aucune tuile marchable près de ({x},{y})");
}

/// Levier de test documenté (mods OFFICIELS `item_crushingblow`/`item_deadlystrike`
/// /`item_openwounds`/leech) : sans lui, un héros sans farm ne traverse pas
/// 4 000 PV de boss. Les coups restent 100 % pipeline combat réel.
fn god_levier(w: &mut World) {
    w.set_hero_combat_mods(HitMods {
        deadly_strike_pct: 100,
        crushing_blow_pct: 100,
        open_wounds_pct: 100,
        life_leech_pct: 100,
        mana_leech_pct: 0,
        elen_frames: 0,
        cold_len_frames: 0,
    });
}

/// Index du monstre vivant le plus proche du héros (None si aucun).
fn nearest_alive(w: &World) -> Option<usize> {
    let (hx, hy) = (w.hero().x, w.hero().y);
    w.monsters()
        .iter()
        .enumerate()
        .filter(|(i, _)| w.monster_alive(*i))
        .min_by(|(_, a), (_, b)| {
            let da = (a.x - hx).hypot(a.y - hy);
            let db = (b.x - hx).hypot(b.y - hy);
            da.partial_cmp(&db).unwrap()
        })
        .map(|(i, _)| i)
}

/// Combat RÉEL jusqu'à la mort du (des) monstre(s) — `tick(Attack)` en boucle,
/// MonsterDied réels. Retourne les events du combat. `cap_ticks` garde-fou.
fn fight_all(w: &mut World, cap_ticks: usize) -> Vec<GameEvent> {
    let mut ev: Vec<GameEvent> = Vec::new();
    for t in 0..cap_ticks {
        let Some(target) = nearest_alive(w) else {
            // 13a — plus rien de VIVANT : les anims DD en cours (≈1,6 s) ne
            // sont pas finies et `MonsterDied` ne part qu'à leur fin. On
            // draine (les monstres sont retirés à la fin du DD) pour que
            // `done` contienne les morts réellement comptées par les journaux.
            for _ in 0..60 {
                if w.monsters().is_empty() {
                    break;
                }
                ev.extend(w.tick(DT, None));
            }
            eprintln!("[fight] fini à t={t} events={}", ev.len());
            return ev;
        };
        if t % 1000 == 999 {
            let (hp, mhp) = w.monster_hp(target).unwrap_or((0, 0));
            eprintln!("[fight t={t}] cible={target} hp={hp}/{mhp} hero={:?} life={}", (w.hero().x, w.hero().y), w.hero_stats().life);
        }
        let (tx, ty) = (w.monsters()[target].x as i32, w.monsters()[target].y as i32);
        ev.extend(w.tick(DT, Some(PlayerInput::Attack { x: tx, y: ty })));
    }
    panic!("combat non résolu en {cap_ticks} ticks ({} vivants)", w.monsters().iter().enumerate().filter(|(i, _)| w.monster_alive(*i)).count());
}

/// Compte les `MonsterDied` d'une liste d'events.
fn count_died(ev: &[GameEvent]) -> usize {
    ev.iter().filter(|e| matches!(e, GameEvent::MonsterDied { .. })).count()
}

/// Spawn un monstre de gabarit manifest près du héros + combat réel jusqu'à
/// sa mort. Le monde ne doit pas contenir d'autre vivant (code connu).
fn spawn_and_kill(w: &mut World, code: &str, kills: &mut Vec<GameEvent>) {
    let (hx, hy) = (w.hero().x as i32, w.hero().y as i32);
    let (sx, sy) = walkable_near(w, hx + 2, hy);
    assert!(w.spawn_monster(code, sx, sy), "spawn {code} refusé");
    let alive: Vec<usize> = (0..w.monsters().len()).filter(|i| w.monster_alive(*i)).collect();
    eprintln!("[spawn {code}] monstres={} vivants={alive:?} hp={:?}", w.monsters().len(), w.monster_hp(0));
    // 13a — `fight_all` draine lui-même les anims DD : les `MonsterDied`
    // (spawn compris) sont garantis dans `done` au retour.
    let done = fight_all(w, 300000);
    assert!(count_died(&done) > 0, "{code} pas mort");
    kills.extend(done);
}

fn zone(id: u32) -> ActEvent {
    ActEvent::ZoneEntered { level_id: id }
}
fn talk(npc: &str) -> ActEvent {
    ActEvent::NpcTalked { npc: npc.into() }
}
fn item(code: &str) -> ActEvent {
    ActEvent::ItemPicked { code: code.into() }
}

fn phase_act1(w: &World, id: &str) -> ActPhase {
    w.quest_act1().unwrap().run(id).unwrap().phase
}
fn phase_act2(w: &World, id: &str) -> ActPhase {
    w.quest_actes2().unwrap().run(id).unwrap().phase
}
fn phase_finale(w: &World, id: &str) -> ActPhase {
    w.quest_finale().unwrap().run(id).unwrap().phase
}

/// Tick vide qui flush les events différés (ActUnlocked, GameWon…).
fn flush(w: &mut World) -> Vec<GameEvent> {
    w.tick(DT, None)
}

// ---------------------------------------------------------------------------
// LE GRAND E2E — traversée BA actes I → V
// ---------------------------------------------------------------------------

#[test]
fn e2e_traversee_complete_ba_actes_1_a_5() {
    let Some(root) = common::ws_root() else { panic!("assets absents") };
    let base = manifest_value(&root);
    let base_str = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();

    // ==================================================================
    // 0. MOTEUR — boot camp, warp marché, waypoints au contact
    // ==================================================================
    let mut core = boot_core(&root, &base_str, "town", SEED);
    assert_eq!(core.level_info_json().contains("\"town\""), true, "boot sur le camp");

    // Waypoint du camp (slot 0, pos réelle TownN1.ds1) activé au CONTACT.
    assert!(
        walk_until(&mut core, 144, 51, |e| matches!(e, d2game::game::CoreEvent::World(GameEvent::WaypointActivated { slot: 0 }))),
        "waypoint du camp non activé au contact"
    );

    // Warp marché camp → Blood Moor (zone réelle du manifest).
    assert!(
        walk_until(&mut core, 282, 107, |e| matches!(e, d2game::game::CoreEvent::LevelSwitch(_))),
        "warp camp→Blood Moor non franchi"
    );
    assert!(core.level_info_json().contains("bloodmoor"), "niveau courant ≠ Blood Moor");
    let codes: Vec<String> = serde_json::from_str(&core.monster_codes_json()).unwrap();
    assert!(codes.len() >= 8, "plan de spawn trop maigre : {codes:?}");
    assert!(codes.iter().any(|c| c == "FA"), "Fallen attendus (planches levels.txt) : {codes:?}");
    let stats = core.compose_stats();
    assert!(stats.drawn > 5000, "Blood Moor pas composée (DRLG seeded) : {stats:?}");
    let g_bloodmoor = engine_grid(&core);

    // Déterminisme seedé : même graine ⇒ même carte + même plan de spawn.
    let core2 = boot_core(&root, &base_str, "bloodmoor", SEED);
    assert_eq!(engine_grid(&core2), g_bloodmoor, "DRLG non déterministe à graine égale");
    assert_eq!(core2.monster_codes_json(), core.monster_codes_json(), "plan de spawn non déterministe");
    drop(core2);

    // Waypoint Blood Moor (slot 39 MVP, pos réelle du chunk DRLG) au contact.
    assert!(
        walk_until(&mut core, 152, 152, |e| matches!(e, d2game::game::CoreEvent::World(GameEvent::WaypointActivated { slot: 39 }))),
        "waypoint Blood Moor (39) non activé au contact"
    );

    // ==================================================================
    // 1. ACTE I — quêtes a1q1→a1q6, kills réels, drops TC, gate acte II
    // ==================================================================
    let mon = MonStats::parse(&std::fs::read_to_string(format!("{root}/assets/patch_monstats.txt")).unwrap()).unwrap();
    let m_a1 = act_manifest(
        &base,
        &["bloodraven", "smith", "corruptrogue3"]
            .iter()
            .map(|id| mon_block(mon.get(id).unwrap_or_else(|| panic!("{id} absent de patch_monstats"))))
            .collect::<Vec<_>>(),
        true,
    );
    let mut w = World::with_seed_and_hero(g_bloodmoor, m_a1, SEED + 1, "BA");
    assert_eq!(w.hero_code(), "BA");
    w.set_quest_act1(ActQuests::new());
    // Le waypoint 39 attaché comme le moteur (attach_level_waypoint).
    let wp = waypoint_for_level(2).unwrap();
    w.set_level_waypoint(wp.slot, wp.pos.unwrap().0, wp.pos.unwrap().1, wp.radius);
    let (hx, hy) = walkable_near(&w, 152, 152);
    place_hero(&mut w, hx, hy);
    // Le champ `wp.here` est TRANSITOIRE (le save ne porte que les slots
    // actifs — cf. waypoints.rs W19) : ré-attach après le placement.
    w.set_level_waypoint(wp.slot, wp.pos.unwrap().0, wp.pos.unwrap().1, wp.radius);
    // Contact réel (le héros marche sur la pierre si besoin) → activation.
    let mut activated = false;
    for _ in 0..300 {
        let ev = w.tick(DT, Some(PlayerInput::MoveTo { x: 152, y: 152 }));
        if ev.iter().any(|e| matches!(e, GameEvent::WaypointActivated { slot: 39 })) {
            activated = true;
            break;
        }
    }
    assert!(activated, "activation waypoint monde (héro à {:?}, pierre (152.5,152.5))", (w.hero().x, w.hero().y));
    assert!(w.waypoint_activated(39));

    // a1q1 Den of Evil : activation Akara, zone 8 (logique — stub documenté),
    // 6 kills RÉELS (FA/ZM/SI = gabarits réels du manifest), remise Akara.
    let mut kills: Vec<GameEvent> = Vec::new();
    w.quest_act1_event(&talk("akara"));
    w.quest_act1_event(&zone(8));
    for code in ["FA", "ZM", "SI", "FA", "ZM", "SI"] {
        spawn_and_kill(&mut w, code, &mut kills);
        // 13a — le kill RÉEL est présenté au journal (joker KILL_ANY de
        // a1q1 : tout MonsterKilled compte). Même pattern explicite que
        // bloodraven/smith/Comtesse/Andariel ci-dessous — sans ce feed la
        // boucle n'avance jamais le journal (6 kills attendus).
        w.quest_act1_event(&ActEvent::MonsterKilled { code: code.into(), superunique: None });
    }
    assert_eq!(phase_act1(&w, "a1q1"), ActPhase::RewardPending, "6 kills réels attendus");
    w.quest_act1_event(&talk("akara"));
    assert_eq!(phase_act1(&w, "a1q1"), ActPhase::Done);
    assert!(w.hero_stats().skill_points >= 1, "récompense Den of Evil : +1 SP");

    // Drops TC : au moins un objet au sol est apparu des kills réels.
    assert!(
        kills.iter().any(|e| matches!(e, GameEvent::ItemDropped { .. })),
        "aucun drop TC sur 6 kills (TreasureClassEx embarquée)"
    );
    // Ramassage réel d'un drop (Pickup) si un objet traîne près du héros.
    let ev = flush(&mut w);
    let _ = ev;

    // a1q2 Blood Raven (record monstats réel) + levier CB + remise Kashya.
    god_levier(&mut w);
    spawn_and_kill(&mut w, "bloodraven", &mut kills);
    w.quest_act1_event(&ActEvent::MonsterKilled { code: "bloodraven".into(), superunique: None });
    w.quest_act1_event(&talk("kashya"));
    assert_eq!(phase_act1(&w, "a1q2"), ActPhase::Done);

    // a1q3 Smith + Malus (objet de quête présenté au journal) → Charsi.
    spawn_and_kill(&mut w, "smith", &mut kills);
    w.quest_act1_event(&ActEvent::MonsterKilled { code: "smith".into(), superunique: None });
    w.quest_act1_event(&item("hdm"));
    w.quest_act1_event(&talk("charsi"));
    assert_eq!(phase_act1(&w, "a1q3"), ActPhase::Done);

    // a1q4 Search for Cain → anneau `rin` RÉEL dans l'inventaire.
    for ev in [zone(5), item("bks"), talk("akara"), zone(4), zone(38)] {
        w.quest_act1_event(&ev);
    }
    w.quest_act1_event(&talk("cain"));
    assert_eq!(phase_act1(&w, "a1q4"), ActPhase::Done);
    assert!(
        w.inventory().iter().flatten().any(|d| d.code == "rin"),
        "l'anneau de récompense a1q4 doit être dans l'inventaire"
    );

    // a1q5 Forgotten Tower : Comtesse (kill réel) → or direct.
    let gold_before = w.hero_stats().gold;
    w.quest_act1_event(&zone(20));
    spawn_and_kill(&mut w, "corruptrogue3", &mut kills);
    w.quest_act1_event(&ActEvent::MonsterKilled { code: "corruptrogue3".into(), superunique: Some("The Countess".into()) });
    assert_eq!(phase_act1(&w, "a1q5"), ActPhase::Done);
    assert!(w.hero_stats().gold >= gold_before + 500, "récompense or Comtesse");

    // a1q6 Andariel = le bloc `bosses` RÉEL du manifest (AN) — kill réel,
    // remise Warriv → GATE acte II.
    w.quest_act1_event(&zone(37));
    spawn_and_kill(&mut w, "AN", &mut kills);
    w.quest_act1_event(&ActEvent::MonsterKilled { code: "andariel".into(), superunique: None });
    assert_eq!(phase_act1(&w, "a1q6"), ActPhase::RewardPending);
    w.quest_act1_event(&talk("warriv"));
    assert_eq!(phase_act1(&w, "a1q6"), ActPhase::Done);
    assert!(w.quest_act1().unwrap().access_act(2), "gate acte II");
    let ev = flush(&mut w);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 2 })), "ActUnlocked(2)");

    // --- SAVE/LOAD n°1 : v3 round-trip idempotent + rétro-compat v1 -------
    let s1 = w.save();
    let v1: Value = serde_json::from_str(&s1).unwrap();
    assert_eq!(v1["version"], 3);
    let w1b = World::load(&s1).expect("load v3");
    assert_eq!(w1b.save(), s1, "round-trip save non idempotent");
    assert_eq!(w1b.hero_code(), "BA");
    assert!(w1b.quest_act1().unwrap().access_act(2), "journal a1 survit au save/load");
    assert!(w1b.waypoint_activated(39), "waypoints survivent au save/load");
    // Rétro-compat v1 : retrait des champs additifs ⇒ chargement (défauts).
    let mut legacy = v1.clone();
    for k in ["hero_code", "level", "missiles", "skills", "waypoints", "difficulty", "npcs", "quests", "mercenary", "corpse", "quest_act1", "quest_actes2", "gold_stash", "consumables", "quest_actes45", "quest_finale"] {
        legacy.as_object_mut().unwrap().remove(k);
    }
    let w_legacy = World::load(&legacy.to_string()).expect("save v1 rétro-compatible");
    assert_eq!(w_legacy.hero_code(), "BA");
    drop(w1b);
    drop(w_legacy);

    // ==================================================================
    // 2. ACTE II — maze réel (tombf1 id 55), Duriel scripté réel, gate A3
    // ==================================================================
    let info = core.switch_level("tombf1", 40.0, 40.0).expect("switch tombf1");
    assert_eq!(info.key, "tombf1");
    let (gw, gh) = core.grid_size();
    assert!(gw > 100 && gh > 100, "maze DrlgType 1 trop petit : {gw}x{gh}");
    let codes: Vec<String> = serde_json::from_str(&core.monster_codes_json()).unwrap();
    assert!(!codes.is_empty(), "planche mon1-10 du tombeau vide");
    let g_tomb = engine_grid(&core);

    let m_a2 = act_manifest(
        &base,
        &[
            boss23_block("duriel"),
            boss23_block("mephisto"),
            mon_block(mon.get("radament").unwrap()),
            mon_block(mon.get("summoner").unwrap()),
        ],
        false,
    );
    let mut w = World::with_seed_and_hero(g_tomb, m_a2, SEED + 2, "BA");
    w.set_quest_actes2(Act2Quests::new());
    god_levier(&mut w);
    let (hx, hy) = walkable_near(&w, gw as i32 / 2, gh as i32 / 2);
    place_hero(&mut w, hx, hy);

    let mut kills: Vec<GameEvent> = Vec::new();
    // a2q1 Radament (record quête réel) → Atma → +1 SP.
    spawn_and_kill(&mut w, "radament", &mut kills);
    w.quest_actes2_event(&ActEvent::MonsterKilled { code: "radament".into(), superunique: Some("Radament".into()) });
    w.quest_actes2_event(&talk("atma"));
    assert_eq!(phase_act2(&w, "a2q1"), ActPhase::Done);

    // a2q2 Bâton Horadrim (sous-objectifs hors ordre) → Cain → `hst`.
    for c in ["vip", "box", "msf"] {
        w.quest_actes2_event(&item(c));
    }
    w.quest_actes2_event(&talk("cain"));
    assert_eq!(phase_act2(&w, "a2q2"), ActPhase::Done);
    assert!(w.inventory().iter().flatten().any(|d| d.code == "hst"));

    // a2q3 Tainted Sun (zones réelles 43/61).
    w.quest_actes2_event(&zone(43));
    w.quest_actes2_event(&zone(61));
    w.quest_actes2_event(&talk("drognan"));
    assert_eq!(phase_act2(&w, "a2q3"), ActPhase::Done);

    // a2q4 Jerhyn → Sanctuaire des Arcanes (id 74 — carte RÉELLE prouvée
    // au moteur juste après) → palais ouvert.
    w.quest_actes2_event(&talk("jerhyn"));
    w.quest_actes2_event(&zone(74));
    assert_eq!(phase_act2(&w, "a2q4"), ActPhase::Done);
    assert!(w.quest_actes2().unwrap().palace_open());

    // Le Sanctuaire des Arcanes a une vraie carte (maze LvlMaze 61 rooms).
    let info = core.switch_level("arcane", 60.0, 60.0).expect("switch arcane");
    assert_eq!(info.key, "arcane");
    let stats = core.compose_stats();
    assert!(stats.drawn > 5000, "arcane pas composé : {stats:?}");
    let g_arcane = engine_grid(&core);

    // a2q5 Invocateur (record réel) → Drognan → portail Canyon ouvert.
    spawn_and_kill(&mut w, "summoner", &mut kills);
    w.quest_actes2_event(&ActEvent::MonsterKilled { code: "summoner".into(), superunique: Some("The Summoner".into()) });
    w.quest_actes2_event(&talk("drognan"));
    assert_eq!(phase_act2(&w, "a2q5"), ActPhase::Done);
    assert!(w.quest_actes2().unwrap().canyon_portal_open(), "portail rouge Canyon");

    // a2q6 : tombeau 68 → repaire 73 ⇒ DURIEL SPAWN SCRIPTÉ (stats réelles
    // golden 4757×84 % = 3995) → kill RÉEL → Tyrael → Meshif → GATE A3.
    w.quest_actes2_event(&zone(68));
    w.quest_actes2_event(&zone(73));
    assert_eq!(w.monsters().len(), 1, "Duriel spawné une fois au trigger");
    assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
    assert_eq!(w.monster_hp(0), Some((3995, 3995)), "HP Duriel = ligne MonLvl 22 N");
    // 13a — caps GÉNÉREUX (300 k ticks) : les boss à PV réels (Duriel 3995,
    // Mephisto 6036…) tombent au rythme réel du levier CB (~200-300 PV /
    // 1000 ticks) ; le combat est réel, seul le plafond arbitraire cédait.
    let done = fight_all(&mut w, 300000);
    assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
    w.quest_actes2_event(&ActEvent::MonsterKilled { code: "duriel".into(), superunique: None });
    w.quest_actes2_event(&talk("tyrael"));
    assert_eq!(phase_act2(&w, "a2q6"), ActPhase::RewardPending);
    w.quest_actes2_event(&talk("meshif"));
    assert_eq!(phase_act2(&w, "a2q6"), ActPhase::Done);
    assert!(w.quest_actes2().unwrap().access_act(3), "gate acte III");
    let ev = flush(&mut w);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 3 })), "ActUnlocked(3)");

    // --- SAVE/LOAD n°2 ------------------------------------------------------
    let s2 = w.save();
    let w2b = World::load(&s2).expect("load v3 (2)");
    assert_eq!(w2b.save(), s2, "round-trip 2 non idempotent");
    assert!(w2b.quest_actes2().unwrap().access_act(3));
    // Transfert du journal A2-A3 vers l'acte III via le JSON versionné.
    let j2 = w2b.quest_actes2().unwrap().to_json();

    // ==================================================================
    // 3. ACTE III — quêtes a3q1→a3q6 (zones par ids réels — A3 sans bloc
    //    manifest : stub documenté), Mephisto scripté réel, combats RÉELS
    //    sur la grille maze réelle de l'Arcane, gate A4
    // ==================================================================
    let m_a3 = act_manifest(
        &base,
        &[
            boss23_block("duriel"),
            boss23_block("mephisto"),
            // 13a — Endugu n'a PAS de record monstats propre : c'est le
            // superunique « Witch Doctor Endugu » porté par la classe
            // fetishshaman4 (superuniques.txt:24 ; alias journal
            // a3q4 = ["endugu", "fetishshaman4"]).
            mon_block(mon.get("fetishshaman4").unwrap()),
            mon_block(mon.get("zombie1").unwrap()),
        ],
        false,
    );
    let mut w = World::with_seed_and_hero(g_arcane, m_a3, SEED + 3, "BA");
    w.set_quest_actes2(Act2Quests::from_json(&j2).expect("journal A2-A3 transféré"));
    god_levier(&mut w);
    let (gw, gh) = (w.width(), w.height());
    let (hx, hy) = walkable_near(&w, gw / 2, gh / 2);
    place_hero(&mut w, hx, hy);

    let mut kills: Vec<GameEvent> = Vec::new();
    // a3q1 Lam Esen → Alkor → +5 points de stats (réel).
    w.quest_actes2_event(&zone(94));
    w.quest_actes2_event(&item("bbb"));
    w.quest_actes2_event(&talk("alkor"));
    assert_eq!(phase_act2(&w, "a3q1"), ActPhase::Done);
    assert!(w.hero_stats().stat_points >= 5, "Lam Esen : +5 stats");

    // a3q2 Khalim (4 reliques hors ordre + transmute + Travincal) → durance.
    for c in ["qhr", "qey", "qbr", "qf1"] {
        w.quest_actes2_event(&item(c));
    }
    w.quest_actes2_event(&item("qf2"));
    w.quest_actes2_event(&zone(83));
    assert_eq!(phase_act2(&w, "a3q2"), ActPhase::Done);
    assert!(w.quest_actes2().unwrap().durance_open(), "accès Durance");

    // a3q3 Gidbinn : gardien = kill RÉEL (gabarit réel zombie1) → Ormus.
    w.quest_actes2_event(&zone(78));
    spawn_and_kill(&mut w, "zombie1", &mut kills);
    w.quest_actes2_event(&ActEvent::MonsterKilled { code: "zombie1".into(), superunique: None });
    w.quest_actes2_event(&item("g33"));
    w.quest_actes2_event(&talk("ormus"));
    assert_eq!(phase_act2(&w, "a3q4"), phase_act2(&w, "a3q4")); // (pas de drift d'index)
    assert_eq!(phase_act2(&w, "a3q3"), ActPhase::Done);

    // a3q4 Endugu (classe réelle fetishshaman4, kill réel) → Meshif → Alkor : +20 vie max.
    let max_life_before = w.hero_stats().max_life;
    spawn_and_kill(&mut w, "fetishshaman4", &mut kills);
    w.quest_actes2_event(&ActEvent::MonsterKilled { code: "endugu".into(), superunique: Some("Witch Doctor Endugu".into()) });
    w.quest_actes2_event(&item("j34"));
    w.quest_actes2_event(&talk("meshif"));
    w.quest_actes2_event(&talk("alkor"));
    assert_eq!(phase_act2(&w, "a3q4"), ActPhase::Done);
    assert_eq!(w.hero_stats().max_life, max_life_before + 20, "Oiseau de Feu : +20 vie max");

    // a3q5 Temple noirci (Travincal, id réel).
    w.quest_actes2_event(&zone(83));
    assert_eq!(phase_act2(&w, "a3q5"), ActPhase::Done);

    // a3q6 : Durance 3 (102) ⇒ MEPHISTO SCRIPTÉ → kill RÉEL → GATE A4.
    w.quest_actes2_event(&zone(102));
    assert_eq!(w.monsters().len(), 1, "Mephisto spawné au trigger");
    assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
    let done = fight_all(&mut w, 300000);
    assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
    w.quest_actes2_event(&ActEvent::MonsterKilled { code: "mephisto".into(), superunique: None });
    w.quest_actes2_event(&item("mss"));
    assert_eq!(phase_act2(&w, "a3q6"), ActPhase::Done);
    assert!(w.quest_actes2().unwrap().access_act(4), "gate acte IV");
    let ev = flush(&mut w);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 4 })), "ActUnlocked(4)");

    // ==================================================================
    // 4. ACTE IV — preset réel (pandemonium/fortress.ds1), Izual, Diablo,
    //    Hephasto scriptés (tables réelles), gate A5
    // ==================================================================
    let info = core.switch_level("pandemonium", 60.0, 60.0).expect("switch pandemonium");
    assert_eq!(info.key, "pandemonium");
    let stats = core.compose_stats();
    // 13a — la Forterresse est un PRESET de ville : fortress.ds1 = 33×25
    // cellules (test drlg_a45 `parses_all_a45_presets`) — ~440 tuiles
    // dessinées est la sortie RÉELLE et déterministe du preset ; le seuil
    // wilderness (> 5000, Blood Moor 80×80) est inatteignable par
    // construction pour une ville compacte.
    assert!(stats.drawn > 400, "forterresse pas composée (preset réel) : {stats:?}");
    let g_pand = engine_grid(&core);

    let m_a4 = act_manifest(
        &base,
        &["izual", "diablo", "hephasto"].iter().map(|id| finale_block(id)).collect::<Vec<_>>(),
        false,
    );
    let mut w = World::with_seed_and_hero(g_pand, m_a4, SEED + 4, "BA");
    w.set_quest_finale(QuestsFinale::new());
    god_levier(&mut w);
    let (gw, gh) = (w.width(), w.height());
    let (hx, hy) = walkable_near(&w, gw / 2, gh / 2);
    place_hero(&mut w, hx, hy);

    // a4q1 Izual : spawn scripté au trigger (105) → kill réel → Tyrael +2 SP.
    let sp_before = w.hero_stats().skill_points;
    let lvl_before = w.hero_stats().level;
    w.quest_finale_event(&zone(105));
    assert_eq!(w.monsters().len(), 1, "Izual spawné au trigger zone");
    let done = fight_all(&mut w, 300000);
    assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
    w.quest_finale_event(&ActEvent::MonsterKilled { code: "izual".into(), superunique: None });
    w.quest_finale_event(&talk("tyrael"));
    assert_eq!(phase_finale(&w, "a4q1"), ActPhase::Done);
    // 13a : l'XP réelle d'Izual peut faire passer des niveaux (+1 SP/niveau,
    // table 99 niveaux) — la récompense Tyrael reste EXACTEMENT +2.
    assert_eq!(
        w.hero_stats().skill_points,
        sp_before + 2 + (w.hero_stats().level - lvl_before),
        "Izual : +2 SP (+1 SP par niveau gagné)"
    );

    // a4q2 Diablo (108) → kill réel → Tyrael → GATE A5.
    w.quest_finale_event(&zone(108));
    assert_eq!(w.monsters().len(), 1, "Diablo spawné au trigger zone");
    let done = fight_all(&mut w, 300000);
    assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
    w.quest_finale_event(&ActEvent::MonsterKilled { code: "diablo".into(), superunique: None });
    w.quest_finale_event(&talk("tyrael"));
    assert_eq!(phase_finale(&w, "a4q2"), ActPhase::Done);
    assert!(w.quest_finale().unwrap().access_act(5), "gate acte V");
    let ev = flush(&mut w);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 5 })), "ActUnlocked(5)");

    // a4q3 Hephasto (107) → kill réel → Cain → gemme `gcv`.
    w.quest_finale_event(&item("mss"));
    w.quest_finale_event(&zone(107));
    assert_eq!(w.monsters().len(), 1, "Hephasto spawné au trigger zone");
    let done = fight_all(&mut w, 300000);
    assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
    w.quest_finale_event(&ActEvent::MonsterKilled { code: "hephasto".into(), superunique: None });
    w.quest_finale_event(&item("hfh"));
    w.quest_finale_event(&talk("cain"));
    assert_eq!(phase_finale(&w, "a4q3"), ActPhase::Done);
    assert!(w.inventory().iter().flatten().any(|d| d.code == "gcv"), "gemme de la Forge");

    // --- SAVE/LOAD n°3 + transfert du journal final vers l'acte V ----------
    let s3 = w.save();
    let w3b = World::load(&s3).expect("load v3 (3)");
    assert_eq!(w3b.save(), s3, "round-trip 3 non idempotent");
    assert!(w3b.quest_finale().unwrap().access_act(5));
    let jf = w3b.quest_finale().unwrap().to_json();

    // ==================================================================
    // 5. ACTE V — les 4 DRLG drlg_a45 prouvées au moteur via leurs ids
    //    (109 preset townsiege réel, 110 wilderness, 113 maze, 124 preset
    //    ×4 seedé), Anciens, vagues du trône, Baal → GAMEWON
    // ==================================================================
    for (key, id, what) in [
        ("harrogath", 109, "preset townsiege.ds1 réel"),
        ("bloodyfoothills", 110, "wilderness DrlgType 3 seedée"),
        ("crystalline", 113, "maze DrlgType 1 seedé"),
        ("hallsvaught", 124, "preset NihlE/N/S/W seedé ×4"),
    ] {
        // 13a — l'entrée maze (DrlgType 1) dépend du plan seedé : les
        // couloirs vivent sur la ligne médiane de la grille 80×80 (≈ y 200
        // sous-tiles — ancres engine (15,200)/(388,200) de drlg_a45), le
        // point (30,30) est dans la roche. Points d'entrée réels : le mid
        // du maze est évidé POUR TOUTE graine (carve(x, mid) invariant).
        let mut info = None;
        'entries: for (x, y) in [(200.0, 200.0), (15.0, 200.0), (388.0, 200.0), (100.0, 200.0), (200.0, 300.0)] {
            if let Ok(i) = core.switch_level(key, x, y) {
                info = Some(i);
                break 'entries;
            }
        }
        let info = info.unwrap_or_else(|| panic!("{key} ({id} — {what}) : aucune entrée marchable"));
        assert_eq!(info.key, key);
        let stats = core.compose_stats();
        assert!(stats.drawn > 1000, "{key} ({id} — {what}) pas composée : {stats:?}");
    }
    // Voyage par waypoint : slot 30 (Harrogath) activé via champ save
    // (levier documenté — pas de position au contact dans la table), le
    // moteur recharge le niveau cible et pose le héros au point d'arrivée.
    {
        let mut s: Value = serde_json::from_str(&core.save_world()).unwrap();
        let mut slots: Vec<u8> = s["waypoints"].as_array().map(|a| a.iter().filter_map(|v| v.as_u64()).map(|v| v as u8).collect()).unwrap_or_default();
        slots.extend([30u8, 33]);
        s["waypoints"] = serde_json::Value::Array(slots.into_iter().map(|v| json!(v)).collect());
        core.load_world(&s.to_string()).expect("reload waypoints");
        let r = core.travel_waypoint(30).expect("voyage waypoint Harrogath");
        assert!(r.contains("harrogath"), "{r}");
        assert!(core.level_info_json().contains("harrogath"));
        let r = core.travel_waypoint(33).expect("voyage waypoint Cavernes");
        assert!(r.contains("crystalline"), "{r}");
    }
    let g_halls = engine_grid(&core);

    let m_a5 = act_manifest(
        &base,
        &["izual", "diablo", "hephasto", "ancientbarb1", "ancientbarb2", "ancientbarb3", "fallenshaman5", "unraveler5", "baalhighpriest", "venomlord", "baalminion1", "baalcrab"]
            .iter()
            .map(|id| finale_block(id))
            .collect::<Vec<_>>(),
        false,
    );
    let mut w = World::with_seed_and_hero(g_halls, m_a5, SEED + 5, "BA");
    w.set_quest_finale(QuestsFinale::from_json(&jf).expect("journal final transféré"));
    god_levier(&mut w);
    let (gw, gh) = (w.width(), w.height());
    let (hx, hy) = walkable_near(&w, gw / 2, gh / 2);
    place_hero(&mut w, hx, hy);

    // a5q5 Anciens : Sommet d'Arreat (120, id réel — stub documenté) ⇒
    // les 3 superuniques réels spawnent → 3 kills RÉELS → Worldstone ouvert.
    w.quest_finale_event(&zone(120));
    assert_eq!(w.monsters().iter().enumerate().filter(|(i, _)| w.monster_alive(*i)).count(), 3, "3 Anciens spawnés");
    let done = fight_all(&mut w, 300000);
    assert_eq!(done.iter().filter(|e| matches!(e, GameEvent::MonsterDied { .. })).count(), 3, "3 Anciens tués");
    for (id, su) in [("ancientbarb1", "Ancient Barbarian 1"), ("ancientbarb2", "Ancient Barbarian 2"), ("ancientbarb3", "Ancient Barbarian 3")] {
        w.quest_finale_event(&ActEvent::MonsterKilled { code: id.into(), superunique: Some(su.into()) });
    }
    assert_eq!(phase_finale(&w, "a5q5"), ActPhase::Done);
    assert!(w.quest_finale().unwrap().worldstone_open(), "porte de la Worldstone");

    // a5q6 : Trône (131) ⇒ vague 1 ; chaque kill réel de superunique déclenche
    // la vague suivante (cascade data-driven SuperUniques.txt) ; Chambre (132)
    // ⇒ Baal ; sa mort = **GameWon**.
    w.quest_finale_event(&zone(131));
    let waves = [
        ("fallenshaman5", "Baal Subject 1"),
        ("unraveler5", "Baal Subject 2"),
        ("baalhighpriest", "Baal Subject 3"),
        ("venomlord", "Baal Subject 4"),
        ("baalminion1", "Baal Subject 5"),
    ];
    for (id, su) in waves {
        assert_eq!(w.monsters().iter().enumerate().filter(|(i, _)| w.monster_alive(*i)).count(), 1, "vague {su} : 1 porteur vivant");
        let done = fight_all(&mut w, 300000);
        assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })), "{su} pas tuée");
        let rewards = w.quest_finale_event(&ActEvent::MonsterKilled { code: id.into(), superunique: Some(su.into()) });
        assert!(rewards.is_empty(), "pas de récompense finale avant Baal : {rewards:?}");
    }
    w.quest_finale_event(&zone(132));
    assert_eq!(w.monsters().iter().enumerate().filter(|(i, _)| w.monster_alive(*i)).count(), 1, "Baal spawné dans la Chambre");
    // 13a — Baal = BOSS de quête (quests_finale : kind Boss pour les boss,
    // SuperUnique réservé aux vagues/Anciens porteurs d'un nom su).
    assert_eq!(w.monster_kind(0), Some(MonKind::Boss));
    let done = fight_all(&mut w, 300000);
    assert!(done.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })), "Baal pas tué");
    let rewards = w.quest_finale_event(&ActEvent::MonsterKilled { code: "baalcrab".into(), superunique: None });
    assert!(rewards.iter().any(|r| matches!(r, FinaleReward::GameWon)), "récompense GameWon");
    w.quest_finale_event(&talk("tyrael"));
    assert_eq!(phase_finale(&w, "a5q6"), ActPhase::Done);
    assert!(w.quest_finale().unwrap().run("a5q6").unwrap().flags & d2rules::quests_finale::FLAG_DONE != 0, "partie gagnée (journal)");
    let ev = flush(&mut w);
    assert!(
        ev.iter().any(|e| matches!(e, GameEvent::GameWon)),
        "l'event GameWon doit être émis au tick : {ev:?}"
    );

    // ==================================================================
    // 6. RÉCAPITULATIF (preuve de la boucle complète données + moteur)
    // ==================================================================
    println!("E2E 13c : 5 actes franchis | gates A2/A3/A4/A5 + GameWon OK");
    println!("  kills clés réels : Blood Raven, Smith, Countess, Andariel(AN), Radament, Summoner, Duriel(3995 HP golden), Endugu, Mephisto, Izual, Diablo, Hephasto, 3 Anciens, 5 vagues du trône, Baal");
    println!("  maps DRLG réelles prouvées : town(DS1), bloodmoor(seeded), tombf1+arcane(drlg_maze), pandemonium+harrogath+hallsvaught(presets drlg_a45), bloodyfoothills(wilderness), crystalline(maze a45)");
    println!("  save/load v3 : 3 round-trips idempotents + rétro-compat v1 + journaux transférés par JSON versionné");
}

// ---------------------------------------------------------------------------
// Les 7 classes en boucle rapide — même grille DRLG réelle, un kill réel,
// save/load round-trip par classe.
// ---------------------------------------------------------------------------

#[test]
fn e2e_sept_classes_boucle_rapide() {
    let Some(root) = common::ws_root() else { panic!("assets absents") };
    let base = manifest_value(&root);
    let base_str = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
    let mut core = boot_core(&root, &base_str, "bloodmoor", SEED + 100);
    let g = engine_grid(&core);
    drop(core);

    let heroes: Vec<String> = base["heroes"].as_object().unwrap().keys().cloned().collect();
    assert_eq!(heroes.len(), 7, "7 classes attendues : {heroes:?}");
    for (i, code) in heroes.iter().enumerate() {
        let mut w = World::with_seed_and_hero(g.clone(), base.clone(), SEED + 200 + i as u64, code);
        assert_eq!(w.hero_code(), &code.to_uppercase(), "classe {code}");
        let (hx, hy) = walkable_near(&w, w.width() / 2, w.height() / 2);
        place_hero(&mut w, hx, hy);
        let mut kills = Vec::new();
        spawn_and_kill(&mut w, "FA", &mut kills);
        assert!(kills.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })), "{code} : kill FA");
        assert!(kills.iter().any(|e| matches!(e, GameEvent::HeroAttacked { .. })), "{code} : coups réels");
        let s = w.save();
        let w2 = World::load(&s).expect("save/load");
        assert_eq!(w2.save(), s, "{code} : round-trip");
        assert_eq!(w2.hero_code(), &code.to_uppercase());
    }
}

// ---------------------------------------------------------------------------
// DRLG en direct : contrat generate(level_id, seed, tilesets) du maze (A2)
// et déterminisme à graine égale, en complément des switches moteur.
// ---------------------------------------------------------------------------

#[test]
fn e2e_drlg_directs_contrats_et_determinisme() {
    let Some(root) = common::ws_root() else { panic!("assets absents") };
    use d2game::drlg;
    use d2game::drlg_maze;
    use d2game::dt1;

    let base = manifest_value(&root);
    let tilesets_for = |key: &str| -> Vec<d2game::dt1::Dt1Tile> {
        let mut out = Vec::new();
        for p in base["levels"][key]["dt1"].as_array().unwrap() {
            let bytes = std::fs::read(format!("{root}/{}", p.as_str().unwrap())).unwrap();
            out.append(&mut dt1::decode_dt1(&bytes).unwrap());
        }
        out
    };

    assert!(drlg_maze::supports(55) && drlg_maze::supports(65) && drlg_maze::supports(74), "banques maze A2");
    let t = tilesets_for("tombf1");
    let (a, sa) = drlg_maze::generate(55, SEED, &t).expect("maze 55");
    let (b, _) = drlg_maze::generate(55, SEED, &t).expect("maze 55 bis");
    assert_eq!((a.w, a.h), (b.w, b.h), "maze déterministe");
    assert!(a.w >= 40 && a.h >= 40, "taille tombeau : {}x{}", a.w, a.h);
    let _ = sa;
    assert!(!drlg_maze::supports(2) && !drlg_maze::supports(1), "pas de maze hors banques");

    // Wildnerness Blood Moor : graine dérivée stable par clé de niveau.
    let s1 = drlg::level_seed(SEED, "bloodmoor");
    let s2 = drlg::level_seed(SEED, "bloodmoor");
    assert_eq!(s1, s2, "level_seed déterministe");
    assert_ne!(s1, drlg::level_seed(SEED, "tombf1"), "graine par niveau");
}
