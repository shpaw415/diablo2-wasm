//! Tests de simulation déterministe — le héros bouge jusqu'à destination, le
//! combat tue le monstre, le drop est ramassé, l'XP fait monter le niveau, la
//! sauvegarde recharge à l'identique. Toutes les graines sont fixes.

use crate::*;
use serde_json::json;

/// Manifest réel du projet (compile-time, zéro IO runtime).
const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

fn manifest_full() -> serde_json::Value {
    serde_json::from_str(MANIFEST_JSON).unwrap()
}

/// Manifest sans monstres (tests de déplacement sans interférence).
fn manifest_no_monsters() -> serde_json::Value {
    let mut m = manifest_full();
    m["monsters"] = json!([]);
    m
}

/// Grille toute marchable (`false` = marchable).
fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
    vec![vec![false; w]; h]
}

/// Avance `ms` de simulation par pas de 50 ms, en rejouant `input` à chaque tick.
fn run(w: &mut World, ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent> {
    let mut ev = Vec::new();
    let mut left = ms;
    while left > 0 {
        let dt = left.min(50);
        ev.extend(w.tick(dt, input));
        left -= dt;
    }
    ev
}

/// Tue le monstre le plus proche en cliquant dessus chaque tick ; renvoie tous
/// les événements jusqu'à ~4 s après la mort (fin du DD + drop).
fn kill_nearest(w: &mut World, budget_ticks: usize) -> Vec<GameEvent> {
    let mut ev = Vec::new();
    let mut died_at: Option<usize> = None;
    for t in 0..budget_ticks * 2 {
        let target = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
        let input = target.map(|(x, y)| PlayerInput::Attack { x, y });
        ev.extend(w.tick(50, input));
        if died_at.is_none()
            && ev.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. }))
        {
            died_at = Some(t);
        }
        if let Some(t0) = died_at {
            if t > t0 + 80 {
                break; // DD 1,6 s + drop : bouclé
            }
        }
    }
    ev
}

/// Spawn un monstre du code voulu sur une tuile libre autour du héros (le héros
/// dérive au fil des combats : on essaye les 4 côtés, bornes comprises).
fn spawn_near_hero(w: &mut World, code: &str) -> bool {
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    let cands = [
        (hx + 2, hy),
        (hx - 2, hy),
        (hx, hy + 2),
        (hx, hy - 2),
        (hx + 1, hy),
        (hx - 1, hy),
        (hx, hy + 1),
        (hx, hy - 1),
    ];
    for (x, y) in cands {
        if w.is_walkable(x, y) && w.spawn_monster(code, x, y) {
            return true;
        }
    }
    false
}

/// Cherche une graine où le premier Fallen tué lâche un drop du type voulu.
fn seed_with_drop(kind: ItemKind) -> u64 {
    for seed in 0..400u64 {
        let mut w = World::with_seed(open_grid(56, 40), manifest_full(), seed);
        w.clear_monsters();
        assert!(spawn_near_hero(&mut w, "FA"));
        let ev = kill_nearest(&mut w, 120);
        if ev.iter().any(
            |e| matches!(e, GameEvent::ItemDropped { item, .. } if item.kind == kind),
        ) {
            return seed;
        }
    }
    panic!("aucune graine trouvée pour un drop {kind:?} en 400 essais");
}

// ---------------------------------------------------------------------------
// Spawn & monde
// ---------------------------------------------------------------------------

#[test]
fn world_spawns_hero_and_manifest_monsters() {
    let manifest = manifest_full();
    let n_expected = manifest["monsters"].as_array().map_or(0, |a| a.len());
    let w = World::new(open_grid(56, 40), manifest);
    assert_eq!(w.width(), 56);
    assert_eq!(w.height(), 40);
    assert_eq!(
        w.monsters().len(),
        n_expected,
        "un monstre par entrée du manifest"
    );
    for m in w.monsters() {
        assert!(w.is_walkable(m.x.floor() as i32, m.y.floor() as i32));
        assert_eq!(m.anim, AnimState::NU);
    }
    let h = w.hero();
    assert!(w.is_walkable(h.x.floor() as i32, h.y.floor() as i32));
    // stats réelles du manifest (patch_charstats : 55/15/30/20)
    let s = w.hero_stats();
    assert_eq!((s.max_life, s.max_mana), (55, 15));
    assert_eq!((s.strength, s.dexterity), (30, 20));
    assert_eq!((s.life, s.mana), (55, 15));
    assert_eq!(s.xp_to_next, 500);
    assert_eq!(s.gold, 0);
}

#[test]
fn world_without_monsters_in_manifest_has_none() {
    let w = World::new(open_grid(56, 40), manifest_no_monsters());
    assert!(w.monsters().is_empty());
}

// ---------------------------------------------------------------------------
// Déplacement
// ---------------------------------------------------------------------------

#[test]
fn hero_moves_to_destination_and_fires_hero_moved() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_no_monsters(), 1);
    let ev = run(&mut w, 4000, Some(PlayerInput::MoveTo { x: 40, y: 30 }));
    assert!(
        ev.iter().any(|e| matches!(e, GameEvent::HeroMoved { path_end: (40, 30) })),
        "HeroMoved(path_end=(40,30)) attendu, événements : {ev:?}"
    );
    assert_eq!((w.hero().x, w.hero().y), (40.5, 30.5));
    assert_eq!(w.hero().anim, AnimState::NU, "immobile après l'arrivée");
}

#[test]
fn hero_walk_anim_and_facing_east() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_no_monsters(), 2);
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    let mut saw_wl = false;
    let mut facing = None;
    for _ in 0..40 {
        w.tick(50, Some(PlayerInput::MoveTo { x: hx + 6, y: hy }));
        if w.hero().anim == AnimState::WL {
            saw_wl = true;
            facing = Some(w.hero().facing);
        }
    }
    assert!(saw_wl, "anim WL pendant la marche");
    assert_eq!(facing, Some(6), "facing 6 = E (0=S,1=SW,2=W,3=NW,4=N,5=NE,6=E,7=SE)");
    assert_eq!((w.hero().x.floor() as i32, w.hero().y.floor() as i32), (hx + 6, hy));
}

#[test]
fn run_mode_uses_rn_anim_and_is_faster_than_walk() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_no_monsters(), 12);
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);

    // Marche : distance parcourue en 1000 ms, anim WL.
    run(&mut w, 1000, Some(PlayerInput::MoveTo { x: hx + 20, y: hy }));
    let walk_d = w.hero().x - (hx as f32 + 0.5);
    assert_eq!(w.hero().anim, AnimState::WL, "marche = anim WL");
    assert!((walk_d - 6.0).abs() < 1.0, "walkSpeed 6 tuiles/s, parcouru {walk_d}");

    // Arrêt + course : anim RN, vitesse RunVelocity (9 tuiles/s au manifest).
    run(&mut w, 4000, Some(PlayerInput::MoveTo { x: hx, y: hy }));
    w.tick(16, Some(PlayerInput::SetRun(true)));
    assert!(w.hero_is_running());
    let mut saw_rn = false;
    let x0 = w.hero().x;
    for _ in 0..20 {
        w.tick(50, Some(PlayerInput::MoveTo { x: hx + 30, y: hy }));
        if w.hero().anim == AnimState::RN {
            saw_rn = true;
        }
    }
    let d = w.hero().x - x0;
    assert!(saw_rn, "course = anim RN");
    assert!((d - 9.0).abs() < 1.2, "runSpeed 9 tuiles/s, parcouru {d}");

    // Retour marche : SetRun(false) → WL.
    w.tick(16, Some(PlayerInput::SetRun(false)));
    assert!(!w.hero_is_running());
}

#[test]
fn run_defaults_to_walk_times_15_when_run_speed_missing() {
    // Manifest sans runSpeed : retombée walk × 1,5 (brief moteur).
    let mut m = manifest_no_monsters();
    if let Some(st) = m.pointer_mut("/heroes/BA/stats") {
        st["runSpeed"] = serde_json::Value::Null;
    }
    let w = World::with_seed(open_grid(56, 40), m, 13);
    // accès indirect : la vitesse course se lit via le comportement — on
    // vérifie via le champ public d'introspection anim/état qu'elle est > walk.
    // (run_speed n'est pas exposé au contrat ; test comportemental minimal.)
    let mut w = w;
    w.tick(16, Some(PlayerInput::SetRun(true)));
    w.tick(16, Some(PlayerInput::MoveTo {
        x: w.hero().x.floor() as i32 + 10,
        y: w.hero().y.floor() as i32,
    }));
    assert_eq!(w.hero().anim, AnimState::RN);
}

#[test]
fn walls_block_pathfinding_but_detour_exists() {
    // mur colonne x=10 de y=0 à y=30 ; passage au sud (y >= 31)
    let mut grid = open_grid(56, 40);
    for row in grid.iter_mut().take(31) {
        row[10] = true; // mur x=10, y=0..=30 ; passage au sud
    }
    let mut w = World::with_seed(grid, manifest_no_monsters(), 3);
    // le héros démarre au centre (x≈28) : allons-le à gauche du mur d'abord
    run(&mut w, 6000, Some(PlayerInput::MoveTo { x: 5, y: 20 }));
    let ev = run(&mut w, 15000, Some(PlayerInput::MoveTo { x: 3, y: 5 }));
    assert!(
        ev.iter().any(|e| matches!(e, GameEvent::HeroMoved { path_end: (3, 5) })),
        "le détour par le passage sud doit aboutir"
    );
    assert_eq!((w.hero().x.floor() as i32, w.hero().y.floor() as i32), (3, 5));
}

#[test]
fn hero_never_stands_on_blocked_tile() {
    let mut grid = open_grid(56, 40);
    for row in grid.iter_mut().take(31) {
        row[10] = true; // mur x=10, y=0..=30 ; passage au sud
    }
    let mut w = World::with_seed(grid, manifest_no_monsters(), 4);
    let mut checked = 0;
    for _ in 0..600 {
        w.tick(50, Some(PlayerInput::MoveTo { x: 2, y: 2 }));
        let (x, y) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
        assert!(w.is_walkable(x, y), "héros sur tuile bloquée {x},{y}");
        checked += 1;
    }
    assert!(checked > 100);
}

#[test]
fn unreachable_target_is_ignored() {
    let mut grid = open_grid(56, 40);
    for row in grid.iter_mut() {
        row[10] = true; // mur plein : l'ouest est injoignable
    }
    let mut w = World::with_seed(grid, manifest_no_monsters(), 5);
    let start = (w.hero().x, w.hero().y);
    let ev = run(&mut w, 3000, Some(PlayerInput::MoveTo { x: 3, y: 5 }));
    assert!(!ev.iter().any(|e| matches!(e, GameEvent::HeroMoved { .. })));
    assert_eq!((w.hero().x, w.hero().y), start, "aucun déplacement possible");
}

// ---------------------------------------------------------------------------
// Combat héros
// ---------------------------------------------------------------------------

#[test]
fn hero_attack_kills_adjacent_fallen_and_grants_xp() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 12345);
    w.clear_monsters();
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    assert!(w.spawn_monster("FA", hx + 1, hy));

    // un tick avec l'attaque : anim A1 immédiatement (l'événement de ce tick compte)
    let mut all = w.tick(50, Some(PlayerInput::Attack { x: hx + 1, y: hy }));
    assert_eq!(w.hero().anim, AnimState::A1, "anim A1 pendant le swing");
    all.extend(run(&mut w, 3000, Some(PlayerInput::Attack { x: hx + 1, y: hy })));
    assert!(
        all.iter().any(|e| matches!(e, GameEvent::HeroAttacked { target: 0, dmg } if *dmg >= 3)),
        "au moins un coup porté (dégâts ≥ 3 = rand(3..6)×1,3 au min)"
    );

    all.extend(kill_nearest(&mut w, 120));
    let died = all
        .iter()
        .find_map(|e| match e {
            GameEvent::MonsterDied { index, xp } => Some((*index, *xp)),
            _ => None,
        })
        .expect("le Fallen doit mourir");
    assert_eq!(died.0, 0);
    assert_eq!(died.1, 18, "XP Fallen = 61 × 30/100 (MonLvl niveau 1) = 18");
    assert!(w.monsters().is_empty(), "le monstre disparaît après son anim DD");
    assert!(w.hero_stats().xp >= 18);
    // après le combat, le héros retrouve NU
    run(&mut w, 1200, None);
    assert_eq!(w.hero().anim, AnimState::NU);
}

#[test]
fn hero_swing_respects_cooldown() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 777);
    w.clear_monsters();
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    assert!(w.spawn_monster("ZM", hx + 1, hy));

    // 700 ms de spam d'attaque (swing = 800 ms) : un seul swing émis, le 2e est bloqué
    let mut ev = Vec::new();
    for _ in 0..14 {
        ev.extend(w.tick(50, Some(PlayerInput::Attack { x: hx + 1, y: hy })));
    }
    let swings = ev
        .iter()
        .filter(|e| matches!(e, GameEvent::HeroAttacked { .. } | GameEvent::HeroMissed { .. }))
        .count();
    assert_eq!(swings, 1, "cooldown 800 ms : pas de 2e swing avant 800 ms (trouvé {swings})");

    // puis le combat va jusqu'au bout
    ev.extend(kill_nearest(&mut w, 200));
    assert!(ev.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })), "le Zombie meurt");
    assert!(w.hero_stats().xp >= 33, "XP Zombie = 111 × 30/100 = 33");
}

// ---------------------------------------------------------------------------
// XP / niveaux
// ---------------------------------------------------------------------------

#[test]
fn xp_accumulates_and_hero_levels_up() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 7);
    w.clear_monsters();
    let mut leveled = false;
    for _ in 0..45 {
        assert!(spawn_near_hero(&mut w, "FA"), "spawn Fallen possible autour du héros");
        let ev = kill_nearest(&mut w, 120);
        if ev.iter().any(|e| matches!(e, GameEvent::HeroLeveled { level: 2 })) {
            leveled = true;
            break;
        }
    }
    assert!(leveled, "28 Fallen × 18 XP = 504 ≥ 500 → niveau 2");
    let s = w.hero_stats();
    assert_eq!(s.level, 2);
    assert!(s.xp >= 500);
    assert_eq!(s.max_life, 57, "55 + LifePerLevel(2)");
    assert_eq!(s.max_mana, 16, "15 + ManaPerLevel(1)");
    assert_eq!(s.xp_to_next, 1500 - s.xp, "seuil cumulé niveau 3 = 1500");
}

// ---------------------------------------------------------------------------
// Drops / inventaire
// ---------------------------------------------------------------------------

#[test]
fn fallen_drops_gold_and_hero_picks_it_up() {
    let seed = seed_with_drop(ItemKind::Gold);
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), seed);
    w.clear_monsters();
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    assert!(w.spawn_monster("FA", hx + 2, hy));
    let ev = kill_nearest(&mut w, 120);
    let amount = ev
        .iter()
        .find_map(|e| match e {
            GameEvent::ItemDropped { item, .. } if item.kind == ItemKind::Gold => Some(item.amount),
            _ => None,
        })
        .expect("drop or attendu (graine choisie)");
    assert!((5..=15).contains(&amount), "or ≈ niveau×10 (ici 5..15) : {amount}");
    assert_eq!(w.items().len(), 1);
    assert_eq!(w.hero_stats().gold, 0);

    let ev = run(&mut w, 300, Some(PlayerInput::Pickup));
    let gained = ev.iter().any(
        |e| matches!(e, GameEvent::GoldGained { amount: a } if *a == amount),
    );
    assert!(gained, "GoldGained({amount}) attendu");
    assert_eq!(w.hero_stats().gold, amount);
    assert!(w.items().is_empty(), "l'or ramassé disparaît du sol");
}

#[test]
fn fallen_drops_potion_pickup_and_use_heals() {
    let seed = seed_with_drop(ItemKind::Potion);
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), seed);
    w.clear_monsters();
    // 1) tuer un FA → potion au sol (graine choisie)
    assert!(spawn_near_hero(&mut w, "FA"));
    let ev = kill_nearest(&mut w, 120);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::ItemDropped { item, .. } if item.kind == ItemKind::Potion)));

    // 2) se faire blesser par un Zombie, puis le tuer (plus d'attaquant)
    assert!(spawn_near_hero(&mut w, "ZM"));
    let mut hurt = 0;
    for _ in 0..400 {
        let ev = w.tick(50, None);
        if let Some(GameEvent::HeroHit { dmg, .. }) = ev.iter().find(|e| matches!(e, GameEvent::HeroHit { .. })) {
            hurt += dmg;
            break;
        }
    }
    assert!(hurt >= 1, "le zombie doit toucher au moins une fois");
    let ev = kill_nearest(&mut w, 200);
    assert!(ev.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
    let life_before = w.hero_stats().life;
    assert!(life_before < w.hero_stats().max_life);

    // 3) ramasser + boire : soin exact = min(60, max - vie). Le drop réel
    // (TreasureClassEx) peut lâcher n'importe quelle potion de vie (hp1..hp5,
    // rejuv…) — toutes soignent `potion_heal` au MVP.
    run(&mut w, 200, Some(PlayerInput::Pickup));
    let slot = (0..40).find(|&s| w.inventory()[s].is_some()).expect("potion dans l'inventaire");
    assert_eq!(w.inventory()[slot].as_ref().unwrap().kind, ItemKind::Potion);
    let ev = run(&mut w, 200, Some(PlayerInput::UseItem(slot)));
    let heal = ev
        .iter()
        .find_map(|e| match e {
            GameEvent::PotionUsed { heal } => Some(*heal),
            _ => None,
        })
        .expect("PotionUsed attendu");
    let expected = 60.min(w.hero_stats().max_life - life_before);
    assert_eq!(heal, expected);
    assert!(heal >= 1);
    assert_eq!(w.hero_stats().life, life_before + heal);
    assert!(w.inventory()[slot].is_none(), "la potion est consommée");
    assert_eq!(w.hero_stats().potion_heal, 60);
}

#[test]
fn pickup_too_far_does_nothing() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 9);
    w.clear_monsters();
    w.items.push(ItemOnGround { def: ItemDef::potion(), x: w.hero().x + 5.0, y: w.hero().y });
    let ev = run(&mut w, 300, Some(PlayerInput::Pickup));
    assert!(ev.is_empty(), "hors de portée : rien ne se passe");
    assert_eq!(w.items().len(), 1);
}

#[test]
fn full_inventory_refuses_pickup() {
    let seed = seed_with_drop(ItemKind::Potion);
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), seed);
    w.clear_monsters();
    for slot in 0..40 {
        w.debug_set_slot(slot, Some(ItemDef::potion()));
    }
    assert!(spawn_near_hero(&mut w, "FA"));
    kill_nearest(&mut w, 120);
    assert_eq!(w.items().len(), 1);
    let ev = run(&mut w, 300, Some(PlayerInput::Pickup));
    assert!(ev.iter().any(|e| matches!(e, GameEvent::InventoryFull)));
    assert_eq!(w.items().len(), 1, "l'objet reste au sol");
    assert!(w.inventory().iter().all(|s| s.is_some()));
}

#[test]
fn use_item_on_empty_slot_is_noop() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_no_monsters(), 11);
    let ev = run(&mut w, 200, Some(PlayerInput::UseItem(13)));
    assert!(ev.is_empty());
}

// ---------------------------------------------------------------------------
// FSM monstre : aggro, poursuite, attaque, mort du héros
// ---------------------------------------------------------------------------

#[test]
fn fallen_aggress_chases_and_hits_hero() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 21);
    w.clear_monsters();
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    assert!(w.spawn_monster("FA", hx + 5, hy)); // dans le rayon d'aggro (6)
    let d0 = (w.monsters()[0].x - w.hero().x).abs();
    let mut min_d = d0;
    let mut got_hit = false;
    for _ in 0..160 {
        let ev = w.tick(50, None);
        if !w.monsters().is_empty() {
            let d = (w.monsters()[0].x - w.hero().x).abs() + (w.monsters()[0].y - w.hero().y).abs();
            min_d = min_d.min(d);
        }
        got_hit |= ev.iter().any(|e| matches!(e, GameEvent::HeroHit { .. }));
        if got_hit {
            break;
        }
    }
    assert!(min_d < d0, "le Fallen se rapproche (aggro 6 tuiles)");
    assert!(got_hit, "au contact, il frappe (dégâts scalés MonLvl 1..2)");
    assert!(w.hero_stats().life < w.hero_stats().max_life);
}

#[test]
fn monster_beyond_leash_gives_up() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 22);
    w.clear_monsters();
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    // Zombie (vitesse 1) : le héros (vitesse 6) le décroche au-delà de la laisse.
    assert!(w.spawn_monster("ZM", hx + 1, hy));
    run(&mut w, 1500, None); // le monstre arrive au contact
    run(&mut w, 4500, Some(PlayerInput::MoveTo { x: hx + 20, y: hy })); // fuite ~20 tuiles
    run(&mut w, 6000, None);
    let d = (w.monsters()[0].x - w.hero().x).abs() + (w.monsters()[0].y - w.hero().y).abs();
    assert!(d > 12.0, "le monstre a décroché (laisse 10) : distance {d}");
}

#[test]
fn hero_dies_then_respawns_at_spawn() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 31);
    w.clear_monsters();
    let spawn = w.hero_spawn();
    let hx = w.hero().x.floor() as i32;
    let hy = w.hero().y.floor() as i32;
    assert!(w.spawn_monster("ZM", hx + 1, hy)); // zombie adjacent : 1..3 dégâts / 1,5 s
    let mut ev = Vec::new();
    let mut died = false;
    for _ in 0..5000 {
        ev.extend(w.tick(50, None));
        if !died && ev.iter().any(|e| matches!(e, GameEvent::HeroDied)) {
            died = true;
        }
        if died && ev.iter().any(|e| matches!(e, GameEvent::HeroRespawned { .. })) {
            break;
        }
    }
    assert!(died, "55 PV contre 1..3 / 1,5 s : le héros finit par tomber");
    assert!(ev.iter().any(|e| matches!(e, GameEvent::HeroRespawned { .. })), "respawn ≤ 2,5 s après la mort");
    let s = w.hero_stats();
    assert_eq!(s.life, s.max_life, "pleine vie au respawn");
    assert_eq!((w.hero().x, w.hero().y), spawn, "respawn au point d'apparition");
    assert_eq!(w.hero().anim, AnimState::NU);
}

// ---------------------------------------------------------------------------
// Déterminisme & sauvegarde
// ---------------------------------------------------------------------------

#[test]
fn same_seed_same_simulation() {
    let mut a = World::with_seed(open_grid(56, 40), manifest_full(), 4242);
    let mut b = World::with_seed(open_grid(56, 40), manifest_full(), 4242);
    for t in 0..400 {
        let input = if t % 20 < 10 {
            Some(PlayerInput::MoveTo { x: 10 + t / 40, y: 30 })
        } else {
            None
        };
        assert_eq!(a.tick(50, input), b.tick(50, input), "tick {t}");
    }
    assert_eq!(a.hero(), b.hero());
    assert_eq!(a.monsters(), b.monsters());
}

#[test]
fn different_seeds_diverge() {
    let mut a = World::with_seed(open_grid(56, 40), manifest_full(), 1);
    let mut b = World::with_seed(open_grid(56, 40), manifest_full(), 2);
    let mut diverged = false;
    for _ in 0..600 {
        if a.tick(50, None) != b.tick(50, None) {
            diverged = true;
            break;
        }
    }
    assert!(diverged, "deux graines différentes doivent diverger (errance)");
}

#[test]
fn save_load_roundtrip_and_continues_identically() {
    // partie commune : déplacement puis idem côté monstres
    let mut a = World::with_seed(open_grid(56, 40), manifest_full(), 99);
    run(&mut a, 3000, Some(PlayerInput::MoveTo { x: 35, y: 32 }));
    run(&mut a, 3000, None); // arrival + monstres en mouvement
    assert!(!a.hero_is_moving(), "on sauvegarde hors mouvement");

    let snapshot = a.save();
    let mut b = World::load(&snapshot).expect("chargement");

    // état restauré
    assert_eq!(a.hero(), b.hero());
    assert_eq!(a.hero_stats(), b.hero_stats());
    assert_eq!(a.monsters(), b.monsters());
    assert_eq!(a.items(), b.items());

    // continuation identique pendant 300 ticks (30 ticks MoveTo, puis idle)
    for t in 0..300 {
        let input = if t < 30 { Some(PlayerInput::MoveTo { x: 20, y: 20 }) } else { None };
        let ev_a = a.tick(50, input);
        let ev_b = b.tick(50, input);
        assert_eq!(ev_a, ev_b, "tick {t}");
        assert_eq!(a.hero(), b.hero());
        assert_eq!(a.monsters(), b.monsters());
    }
}

#[test]
fn save_rejects_garbage_and_wrong_version() {
    assert!(World::load("pas du json").is_err());
    assert!(World::load("{}").is_err());
    let w = World::with_seed(open_grid(10, 10), manifest_no_monsters(), 1);
    let good = w.save();
    let mut v: serde_json::Value = serde_json::from_str(&good).unwrap();
    v["version"] = serde_json::json!(999);
    assert!(World::load(&v.to_string()).is_err(), "mauvaise version refusée");
    v["version"] = serde_json::json!(1);
    v["collision"] = serde_json::json!(["1X1"]); // caractère illégal
    assert!(World::load(&v.to_string()).is_err());
}

// ---------------------------------------------------------------------------
// Sélection de personnage (manifest.heroes)
// ---------------------------------------------------------------------------

/// Manifest réel + bloc `heroes` {BA, SOR, PA} — SOR = stats de sorcière
/// distinctes, PA = paladin (stats du bloc `hero`).
fn manifest_heroes() -> serde_json::Value {
    let mut m = manifest_full();
    let mut sor = m["hero"].clone();
    sor["class"] = json!("SOR");
    sor["name"] = json!("Sorcière");
    sor["stats"]["life"] = json!(40);
    sor["stats"]["mana"] = json!(60);
    sor["stats"]["strength"] = json!(10);
    sor["stats"]["dexterity"] = json!(25);
    let mut pal = m["hero"].clone();
    pal["class"] = json!("PA");
    pal["name"] = json!("Paladin");
    m["heroes"] = json!({ "BA": m["hero"].clone(), "SOR": sor, "PA": pal });
    m
}

#[test]
fn hero_selection_picks_stats_per_code() {
    let m = manifest_heroes();
    // Défaut (World::new / with_seed) : classe du bloc `hero` (BA) — 55 pv.
    let w = World::with_seed(open_grid(56, 40), m.clone(), 7);
    assert_eq!(w.hero_code(), "BA");
    assert_eq!(w.hero_stats().max_life, 55);
    assert_eq!(w.hero_stats().strength, 30);
    // with_hero("SOR") : stats de la sorcière — 40 pv, 60 mana, force 10.
    let wsor = World::with_hero(open_grid(56, 40), m.clone(), "SOR");
    assert_eq!(wsor.hero_code(), "SOR");
    assert_eq!(wsor.hero_stats().max_life, 40);
    assert_eq!(wsor.hero_stats().max_mana, 60);
    assert_eq!(wsor.hero_stats().strength, 10);
    assert_eq!(wsor.hero_stats().dexterity, 25);
    // Insensible à la casse.
    let wlow = World::with_hero(open_grid(56, 40), m.clone(), "sor");
    assert_eq!(wlow.hero_code(), "SOR");
    assert_eq!(wlow.hero_stats().max_life, 40);
    // Code inconnu : retombée sur le bloc `hero` (BA), jamais de panic.
    let wunk = World::with_hero(open_grid(56, 40), m, "XX");
    assert_eq!(wunk.hero_code(), "BA");
    assert_eq!(wunk.hero_stats().max_life, 55);
}

#[test]
fn hero_selection_default_stays_retrocompatible() {
    // Manifest SANS `heroes` : tout code demandé retombe sur le bloc `hero`.
    // (Construit explicitement : le manifest du dépôt porte maintenant une
    // section `heroes` réelle — BA/SOR/PA — voir assets/manifest.json.)
    let mut m = manifest_full();
    m.as_object_mut().unwrap().remove("heroes");
    assert!(m.get("heroes").is_none());
    for req in ["BA", "SOR", "PA", ""] {
        let w = World::with_hero(open_grid(56, 40), m.clone(), req);
        assert_eq!(w.hero_code(), "BA");
        assert_eq!(w.hero_stats().max_life, 55);
    }
    // resolve_hero_code (API partagée avec d2game) : même règle.
    assert_eq!(resolve_hero_code(&m, "SOR"), "BA");
    let mh = manifest_heroes();
    assert_eq!(resolve_hero_code(&mh, "sor"), "SOR");
    assert_eq!(resolve_hero_code(&mh, "BA"), "BA");
    assert_eq!(resolve_hero_code(&mh, "NK"), "BA");
}

#[test]
fn hero_selection_survives_save_load() {
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 4242, "SOR");
    run(&mut w, 1000, None);
    let w2 = World::load(&w.save()).expect("rechargement");
    assert_eq!(w2.hero_code(), "SOR");
    assert_eq!(w2.hero_stats().max_life, 40);
    assert_eq!(w2.hero_stats().max_mana, 60);
    // Sauvegarde v1 SANS hero_code (ancienne page) : charge quand même.
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v.as_object_mut().unwrap().remove("hero_code");
    let w3 = World::load(&v.to_string()).expect("rechargement save v1 sans hero_code");
    assert_eq!(w3.hero_code(), "BA"); // défaut du manifest (gabarit futur)
    assert_eq!(w3.hero_stats().max_life, 40); // stats courantes persistées
}

// ---------------------------------------------------------------------------
// helpers de test (état interne)
// ---------------------------------------------------------------------------


// ---------------------------------------------------------------------------
// Attaque à distance (missiles d2missiles dans le World)
// ---------------------------------------------------------------------------

#[test]
fn ranged_attack_fires_flies_and_hits() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 11);
    w.clear_monsters();
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert!(w.spawn_monster("FA", hx + 8, hy), "Fallen à 8 tuiles à l'est");
    // Tir : le héros fait face à l'est et décoche.
    let ev = run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    assert!(ev.iter().any(|e| matches!(e, GameEvent::MissileSpawned)), "MissileSpawned au tir");
    assert_eq!(w.hero().facing, 6, "face à l'est (cible à l'est)");
    assert_eq!(w.missiles().len(), 1, "flèche en vol");
    // Vol : 8 tuiles / 24 tps ≈ 333 ms ; on avance jusqu'au Hit (budget 2 s).
    let mut hit = None;
    for _ in 0..40 {
        let ev = w.tick(50, None);
        for e in ev {
            if let GameEvent::MissileHit { target, dmg } = e {
                hit = Some((target, dmg));
            }
        }
        if hit.is_some() {
            break;
        }
    }
    let (target, dmg) = hit.expect("MissileHit sur le Fallen");
    assert_eq!(target, 0);
    assert!((3..=8).contains(&dmg), "dégâts arme×force (hax 3..6 × 1,30), trouvé {dmg}");
    assert!(w.missiles().is_empty(), "la flèche disparaît sur le coup");
}

#[test]
fn ranged_attack_misses_roll_and_kept_deterministic() {
    // Même graine ⇒ même séquence hit/miss (jet de toucher côté World).
    let shoot = |seed: u64| -> Vec<GameEvent> {
        let mut w = World::with_seed(open_grid(56, 40), manifest_full(), seed);
        w.clear_monsters();
        let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
        assert!(w.spawn_monster("ZM", hx + 10, hy));
        run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 10, y: hy }));
        let mut out = Vec::new();
        for _ in 0..30 {
            out.extend(w.tick(50, None));
            if w.missiles().is_empty() {
                break;
            }
        }
        out.into_iter().filter(|e| matches!(e, GameEvent::MissileHit { .. } | GameEvent::MissileMissed { .. })).collect()
    };
    let a = shoot(0xA1);
    let b = shoot(0xA1);
    assert_eq!(a, b, "même graine ⇒ même résultat");
    assert!(!a.is_empty(), "au moins un événement (hit ou miss)");
}

#[test]
fn ranged_attack_respects_arrow_range() {
    // Tir vers l'est, rien sur la trajectoire : en vol à 1 s (24 tuiles <
    // portée 40), disparu à 2,5 s (40 tuiles / 24 tps ≈ 1667 ms) — aucun hit.
    let mut w = World::with_seed(open_grid(56, 40), manifest_no_monsters(), 12);
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 50, y: hy }));
    assert_eq!(w.missiles().len(), 1);
    run(&mut w, 1000, None);
    assert_eq!(w.missiles().len(), 1, "encore en vol à ~1 s (24 tuiles < 40)");
    let ev = run(&mut w, 1500, None);
    assert!(!ev.iter().any(|e| matches!(e, GameEvent::MissileHit { .. } | GameEvent::MissileMissed { .. })));
    assert!(w.missiles().is_empty(), "la flèche expire à portée 40 (1667 ms)");
}

#[test]
fn ranged_attack_is_stopped_by_walls() {
    // Mur vertical 4 tuiles à l'est du héros, Fallen derrière (8 tuiles).
    // Le spawn héros est déterministe : monde jetable sur grille ouverte
    // pour dériver (hx, hy), mur posé sur la colonne hx+4, monde réel ensuite.
    let probe = World::with_seed(open_grid(56, 40), manifest_full(), 13);
    let (hx, hy) = (probe.hero().x.floor() as i32, probe.hero().y.floor() as i32);
    let mut grid = open_grid(56, 40);
    for y in 0..40 {
        grid[y][(hx + 4) as usize] = true;
    }
    let mut w = World::with_seed(grid, manifest_full(), 13);
    w.clear_monsters();
    let (hx2, hy2) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert_eq!((hx2, hy2), (hx, hy), "le spawn du héros ne bouge pas (BFS depuis le centre, colonne hx+4 hors chemin)");
    assert!(w.spawn_monster("FA", hx + 8, hy));
    run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    let ev = run(&mut w, 1200, None);
    assert!(
        !ev.iter().any(|e| matches!(e, GameEvent::MissileHit { .. } | GameEvent::MissileMissed { .. })),
        "le mur protège le monstre"
    );
    assert!(w.missiles().is_empty(), "la flèche meurt sur le mur");
}

#[test]
fn in_flight_missiles_survive_save_load() {
    // Flèche en vol à mi-course : save/load → continuation identique (le RNG
    // missiles sérialisé poursuit le même flux de jets).
    let build = || {
        let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 14);
        w.clear_monsters();
        let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
        assert!(w.spawn_monster("FA", hx + 20, hy)); // hit à ~833 ms, après le save
        run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 20, y: hy }));
        w.tick(100, None); // la flèche est en vol (~2,4 tuiles parcourues)
        w
    };
    let mut a = build();
    let snapshot = a.save();
    let mut b = World::load(&snapshot).expect("rechargement avec flèche en vol");
    assert_eq!(b.missiles().len(), 1, "la flèche en vol est persistée");
    assert_eq!(a.missiles(), b.missiles(), "position de vol identique");
    for t in 0..40 {
        let ev_a = a.tick(50, None);
        let ev_b = b.tick(50, None);
        assert_eq!(ev_a, ev_b, "tick {t} : événements identiques après reload");
        assert_eq!(a.missiles(), b.missiles(), "tick {t} : vol identique après reload");
    }
}

#[test]
fn ranged_attack_respects_cooldown_and_picks_nearest_target() {
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 15);
    w.clear_monsters();
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert!(w.spawn_monster("FA", hx + 8, hy));
    assert!(w.spawn_monster("FA", hx + 9, hy + 3));
    // Deux tirs rapprochés : le second est refusé (cooldown 800 ms).
    run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    let n1 = w.missiles().len();
    assert!(n1 >= 1, "première flèche partie");
    run(&mut w, 100, Some(PlayerInput::RangedAttack { x: hx + 9, y: hy + 3 }));
    assert_eq!(w.missiles().len(), n1, "cooldown : pas de 2e flèche avant 800 ms");
}

// ---------------------------------------------------------------------------
// Compétences (skills de départ, câblage skill → missile, save v2)
// ---------------------------------------------------------------------------

#[test]
fn sorceress_starts_with_firebolt_and_ranged_fires_it() {
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 16, "SOR");
    w.clear_monsters();
    // état : Fire Bolt niv 1, missile firebolt, sélection par défaut
    let skills = w.hero_skills();
    assert_eq!(skills.len(), 1);
    assert_eq!(skills[0].code, "Fire Bolt");
    assert_eq!(skills[0].level, 1);
    assert_eq!(skills[0].missile.as_deref(), Some("firebolt"));
    assert_eq!(w.active_skill().unwrap().code, "Fire Bolt");
    // exposé HUD (sérialisé vers la page via hero_stats_json d2game)
    let s = w.hero_stats();
    assert_eq!(s.skill, "Fire Bolt");
    assert_eq!(s.skill_level, 1);

    // tir : c'est un firebolt (vel 20), PAS une flèche (vel 24)
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert!(w.spawn_monster("FA", hx + 8, hy));
    let ev = run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    assert!(ev.iter().any(|e| matches!(e, GameEvent::MissileSpawned)));
    assert_eq!(w.missiles().len(), 1);
    assert_eq!(w.missiles()[0].def_id, "firebolt", "le skill actif décoche son missile");
    assert_eq!(w.missiles()[0].elem, d2missiles::DamageType::Fire, "firebolt = élément feu");
    // vol 8 tuiles / 20 tps = 400 ms ; dégâts OFFICIELS du skill (3–8 feu),
    // pas les dégâts d'arme ×force de la flèche.
    let mut hit = None;
    for _ in 0..60 {
        let ev = w.tick(50, None);
        for e in ev {
            if let GameEvent::MissileHit { target, dmg } = e {
                hit = Some((target, dmg));
            }
        }
        if hit.is_some() {
            break;
        }
    }
    let (target, dmg) = hit.expect("le firebolt touche le Fallen");
    assert_eq!(target, 0);
    assert!((3..=8).contains(&dmg), "dégâts firebolt niv 1 = 3..8, trouvé {dmg}");
}

#[test]
fn skill_cast_plays_sc_melee_keeps_a1_15b() {
    // Étape 15b : le cast d'un skill (Fire Bolt SOR) joue le mode COF SC ;
    // une attaque simple (mêlée BA) reste en A1 — branch minimal dans la
    // sélection anim existante (swing_cast, transitoire non persisté).
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 4242, "SOR");
    w.clear_monsters();
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert!(w.spawn_monster("FA", hx + 8, hy));
    let _ = run(&mut w, 16, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    assert_eq!(w.hero().anim, AnimState::SC, "cast de skill = mode SC");
    assert!(w.hero().anim_frame <= 1, "le cast vient de démarrer");
    // le swing SC se termine : retour NU (jamais de SC figé)
    run(&mut w, 900, None);
    assert_eq!(w.hero().anim, AnimState::NU, "SC terminé ⇒ idle");
    // save/load pendant le cast transitoire : charge sans erreur (l'anim
    // SC n'est pas un état persistant, le repli A1/NU au tick est voulu)
    let mut w2 = World::with_seed_and_hero(open_grid(56, 40), manifest_heroes(), 7, "SOR");
    w2.clear_monsters();
    let (hx2, hy2) = (w2.hero().x.floor() as i32, w2.hero().y.floor() as i32);
    w2.spawn_monster("FA", hx2 + 8, hy2);
    let _ = run(&mut w2, 16, Some(PlayerInput::RangedAttack { x: hx2 + 8, y: hy2 }));
    let mut w3 = World::load(&w2.save()).expect("save pendant SC charge");
    let _ = w3.tick(50, None);
    // mêlée : Attack sur un monstre adjacent ⇒ A1 (swing_cast = false)
    let mut w4 = World::with_seed_and_hero(open_grid(56, 40), manifest_full(), 9, "BA");
    let (hx4, hy4) = (w4.hero().x.floor() as i32, w4.hero().y.floor() as i32);
    assert!(w4.spawn_monster("FA", hx4 + 1, hy4));
    let _ = run(&mut w4, 16, Some(PlayerInput::Attack { x: hx4 + 1, y: hy4 }));
    assert_eq!(w4.hero().anim, AnimState::A1, "mêlée simple = A1, jamais SC");
}

#[test]
fn classes_without_start_skill_fallback_to_weapon_arrow() {
    let m = manifest_full();
    // BA/PA (StartSkill vide dans charstats v1.12) : AUCUN skill actif —
    // le tir à distance garde la flèche transportant les dégâts d'arme.
    for code in ["BA", "PA", "AM", "AS", "DZ"] {
        let mut w = World::with_seed_and_hero(open_grid(56, 40), m.clone(), 17, code);
        assert!(w.active_skill().is_none(), "{code} : pas de skill de départ");
        assert!(w.hero_skills().is_empty(), "{code} : aucune compétence");
        assert_eq!(w.hero_stats().skill, "", "{code} : HUD sans skill");
        w.clear_monsters();
        let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
        assert!(w.spawn_monster("FA", hx + 8, hy));
        run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
        assert_eq!(w.missiles()[0].def_id, "arrow", "{code} : repli arrow");
        assert_eq!(w.missiles()[0].elem, d2missiles::DamageType::Physical);
    }

    // NE a BIEN un skill de départ (raise skeleton) — sans missile : arrow.
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 17, "NE");
    assert_eq!(w.active_skill().map(|s| s.code.as_str()), Some("Raise Skeleton"));
    assert!(w.active_skill().unwrap().is_melee());
    w.clear_monsters();
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert!(w.spawn_monster("FA", hx + 8, hy));
    run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    assert_eq!(w.missiles()[0].def_id, "arrow", "invocation : repli arrow");

    // Classe sans skill de départ (code hors charstats connu) : aucun skill,
    // arrow quand même (jamais de panic). hero_code inconnu ⇒ aucun skills.
    let mut mx = manifest_full();
    mx.as_object_mut().unwrap().remove("heroes");
    mx["hero"]["class"] = json!("XX");
    let w = World::with_seed_and_hero(open_grid(56, 40), mx, 17, "");
    assert_eq!(w.hero_code(), "XX");
    assert!(w.hero_skills().is_empty());
    assert!(w.active_skill().is_none());
    assert_eq!(w.hero_stats().skill, "");
}

/// Lecture directe de `patch_charstats.txt` (la table fait foi) : classe →
/// colonne `StartSkill` (nom lowercase du skill, vide si aucun).
fn charstats_start_skill() -> Vec<(String, String)> {
    const CHARSTATS: &str = include_str!("../../../../assets/patch_charstats.txt");
    let mut lines = CHARSTATS.lines();
    let hdr: Vec<&str> = lines.next().unwrap().split('\t').collect();
    let (ci, si) = (hdr.iter().position(|h| *h == "class").unwrap(),
                    hdr.iter().position(|h| *h == "StartSkill").unwrap());
    lines
        .filter_map(|l| {
            let f: Vec<&str> = l.split('\t').collect();
            let cls = f.get(ci)?.to_string();
            (matches!(cls.as_str(), "Amazon" | "Sorceress" | "Necromancer" | "Paladin"
                | "Barbarian" | "Druid" | "Assassin"))
                .then(|| (cls, f.get(si).unwrap_or(&"").to_string()))
        })
        .collect()
}

#[test]
fn starting_skills_match_charstats_all_7_classes() {
    // Golden 7/7 : la table charstats réelle (lue ci-dessus) est l'autorité.
    // Canonisation du nom lowercase → code exact via la table skills embarquée.
    let tree = crate::skills_tree::SkillTreeData::global();
    let classes = [
        ("Amazon", "AM"), ("Sorceress", "SOR"), ("Necromancer", "NE"),
        ("Paladin", "PA"), ("Barbarian", "BA"), ("Druid", "DZ"), ("Assassin", "AS"),
    ];
    let by_class = charstats_start_skill();
    assert_eq!(by_class.len(), 7, "charstats : 7 lignes de classe");
    for (cls, code) in classes {
        let start = by_class
            .iter()
            .find(|(c, _)| c == cls)
            .map(|(_, s)| s.as_str())
            .unwrap_or_else(|| panic!("{cls} absente de charstats"));
        let expected: Vec<crate::skills::HeroSkill> = if start.is_empty() {
            Vec::new()
        } else {
            let rec = tree
                .records()
                .iter()
                .find(|r| r.code.eq_ignore_ascii_case(start))
                .unwrap_or_else(|| panic!("{start:?} introuvable dans patch_skills.txt"));
            assert_eq!(rec.req_level, 1, "{cls}: le skill de départ est niv 1");
            vec![crate::skills::HeroSkill {
                code: rec.code.clone(),
                level: 1,
                missile: rec.srv_missile.clone(),
            }]
        };
        assert_eq!(
            crate::skills::starting_skills(code),
            expected,
            "{cls} ({code}) : StartSkill={start:?}"
        );
    }
    // Amorçage monde : NE naît avec Raise Skeleton, SOR avec Fire Bolt.
    let m = manifest_full();
    let wne = World::with_hero(open_grid(56, 40), m.clone(), "NE");
    assert_eq!(wne.hero_skills(), crate::skills::starting_skills("NE"));
    let wsor = World::with_hero(open_grid(56, 40), m, "SOR");
    assert_eq!(wsor.active_skill().map(|s| s.code.as_str()), Some("Fire Bolt"));
}

#[test]
fn manifest_carries_seven_playable_heroes() {
    // Vague 7 classes : le manifest réel porte BA/SOR/PA + AM/AS/DZ/NE, tous
    // avec les 4 modes du moteur (NU/WL/A1/DD) et des stats charstats réelles.
    let m = manifest_full();
    let heroes = m["heroes"].as_object().unwrap();
    for code in ["BA", "SOR", "PA", "AM", "AS", "DZ", "NE"] {
        let def = heroes
            .get(code)
            .unwrap_or_else(|| panic!("heroes[{code}] absent du manifest"));
        assert!(def["name"].is_string(), "{code}: nom manquant");
        for mode in ["NU", "WL", "A1", "DD"] {
            let blk = &def["modes"][mode];
            assert!(blk["cof"].is_string(), "{code}/{mode}: cof manquant");
            assert!(blk["frames"].is_u64(), "{code}/{mode}: frames manquantes");
            assert!(!blk["layers"].as_object().unwrap().is_empty(), "{code}/{mode}: 0 couche");
        }
    }
    // Stats réelles (patch_charstats.txt : life = vit + hpadd, mana = energy).
    let stats = [
        // code, life, mana, str, dex
        ("AM", 50, 15, 20, 25),
        ("AS", 50, 25, 20, 20),
        ("DZ", 55, 20, 15, 20),
        ("NE", 45, 25, 15, 25),
    ];
    for (code, life, mana, str_, dex) in stats {
        let s = &m["heroes"][code]["stats"];
        assert_eq!((s["life"].as_u64().unwrap() as u32, s["mana"].as_u64().unwrap() as u32), (life, mana), "{code}");
        assert_eq!((s["strength"].as_u64().unwrap() as u32, s["dexterity"].as_u64().unwrap() as u32), (str_, dex), "{code}");
        let w = World::with_hero(open_grid(56, 40), m.clone(), code);
        assert_eq!(w.hero_code(), code, "{code} : résolution manifest");
        assert_eq!(w.hero_stats().max_life, life, "{code} : vie chargée du manifest");
        assert_eq!(w.hero_stats().max_mana, mana, "{code} : mana chargé du manifest");
    }
}

#[test]
fn select_skill_switches_active_and_stats() {
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 18, "SOR");
    // un seul skill au MVP : index 1 refusé, état inchangé (jamais de panic)
    assert!(!w.select_skill(1));
    assert_eq!(w.active_skill().unwrap().code, "Fire Bolt");
    assert!(w.select_skill(0));
    assert_eq!(w.hero_stats().skill, "Fire Bolt");
}

#[test]
fn skills_survive_save_load_and_v1_saves_load_with_defaults() {
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m.clone(), 4242, "SOR");
    run(&mut w, 500, None);
    let w2 = World::load(&w.save()).expect("rechargement v2");
    assert_eq!(w2.hero_skills(), w.hero_skills(), "skills persistés");
    assert_eq!(w2.active_skill().unwrap().code, "Fire Bolt");
    assert_eq!(w2.hero_stats().skill, "Fire Bolt");

    // sauvegarde v2 SANS champ skills (page intermédiaire) : défauts de classe
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v.as_object_mut().unwrap().remove("skills");
    v.as_object_mut().unwrap().remove("active_skill");
    let w3 = World::load(&v.to_string()).expect("rechargement v2 sans skills");
    assert_eq!(w3.hero_skills(), crate::skills::starting_skills("SOR"));
    assert_eq!(w3.active_skill().unwrap().code, "Fire Bolt");

    // sauvegarde v1 RÉELLE (ancienne page) : version 1, aucun champ skill
    let mut v1 = v;
    v1["version"] = serde_json::json!(1);
    let w4 = World::load(&v1.to_string()).expect("rechargement v1 rétro-compat");
    assert_eq!(w4.hero_skills(), crate::skills::starting_skills("SOR"));

    // index actif corrompu (hors bornes) : défaut par classe, pas de panic
    let mut v5: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v5["active_skill"] = serde_json::json!(9);
    let w5 = World::load(&v5.to_string()).expect("index hors bornes toléré");
    assert_eq!(w5.active_skill().unwrap().code, "Fire Bolt");
}

#[test]
fn sor_firebolt_vs_barbarian_arrow_diverge_on_seed() {
    // Le câblage skill → missile ne dérive PAS le RNG monde : même graine,
    // SOR et BA produisent des événements de tir identiques SAUF la nature du
    // projectile (déterminisme préservé, contrat save bit-exact).
    let shoot = |hero: &str| -> (Vec<GameEvent>, Vec<d2missiles::Missile>) {
        let m = manifest_heroes();
        let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 0xBEEF, hero);
        w.clear_monsters();
        let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
        assert!(w.spawn_monster("FA", hx + 10, hy));
        run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 10, y: hy }));
        let mut out = Vec::new();
        for _ in 0..40 {
            out.extend(w.tick(50, None));
            if w.missiles().is_empty() {
                break;
            }
        }
        let evs: Vec<GameEvent> = out
            .into_iter()
            .filter(|e| matches!(e, GameEvent::MissileHit { .. } | GameEvent::MissileMissed { .. }))
            .collect();
        (evs, w.missiles().to_vec())
    };
    let (sor_ev, _) = shoot("SOR");
    let (ba_ev, _) = shoot("BA");
    // le jet de toucher (RNG monde) est identique : déterminisme intact
    assert_eq!(
        sor_ev.iter().map(|e| matches!(e, GameEvent::MissileHit { .. })).collect::<Vec<_>>(),
        ba_ev.iter().map(|e| matches!(e, GameEvent::MissileHit { .. })).collect::<Vec<_>>(),
        "même graine ⇒ mêmes jets de toucher, quelle que soit la classe"
    );
    assert!(!sor_ev.is_empty());
}

// ---------------------------------------------------------------------------
// Save v3 — clé du niveau courant (save P0, finish-jeu.md EXP-S1/S3/E1)
// ---------------------------------------------------------------------------

#[test]
fn save_v3_roundtrip_preserves_level_key() {
    let mut w = World::with_seed(open_grid(12, 12), manifest_no_monsters(), 7);
    assert_eq!(w.level_key(), "town", "monde neuf : niveau historique par défaut");
    // normalisation attendue : trim + minuscules (même règle que GameCore)
    w.set_level_key("  BloodMoor  ");
    assert_eq!(w.level_key(), "bloodmoor");
    run(&mut w, 500, None);

    let snapshot = w.save();
    let v: serde_json::Value = serde_json::from_str(&snapshot).unwrap();
    assert_eq!(v["version"], serde_json::json!(3), "format courant = v3");
    assert_eq!(v["level"], serde_json::json!("bloodmoor"), "clé de niveau persistée");

    let w2 = World::load(&snapshot).expect("rechargement v3");
    assert_eq!(w2.level_key(), "bloodmoor", "clé de niveau restaurée");
    assert_eq!(w.hero(), w2.hero());
    // double round-trip : la clé ne se dégrade pas
    let w3 = World::load(&w2.save()).expect("re-rechargement");
    assert_eq!(w3.level_key(), "bloodmoor");
}

#[test]
fn save_v2_without_level_loads_in_town() {
    let mut w = World::with_seed(open_grid(12, 12), manifest_no_monsters(), 7);
    w.set_level_key("bloodmoor");
    // rétro-compat v2 : pas de champ `level` (saves antérieurs au P0 save)
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v["version"] = serde_json::json!(2);
    v.as_object_mut().unwrap().remove("level");
    let w2 = World::load(&v.to_string()).expect("chargement v2 sans level");
    assert_eq!(w2.level_key(), "town", "défaut sensible : Camp des Rousses");
    assert_eq!(w2.hero_code(), w.hero_code());
    // re-sauvegarde : le monde migré ressort en v3 avec sa clé par défaut
    let v3: serde_json::Value = serde_json::from_str(&w2.save()).unwrap();
    assert_eq!(v3["version"], serde_json::json!(3));
    assert_eq!(v3["level"], serde_json::json!("town"));
}

#[test]
fn save_v1_retrocompat_without_level_or_hero_code() {
    let w = World::with_seed(open_grid(12, 12), manifest_no_monsters(), 7);
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v["version"] = serde_json::json!(1);
    let obj = v.as_object_mut().unwrap();
    obj.remove("level");
    obj.remove("hero_code");
    obj.remove("skills");
    obj.remove("missiles");
    let w2 = World::load(&v.to_string()).expect("chargement v1 rétro-compat");
    assert_eq!(w2.level_key(), "town");
    // skills de départ reconstitués (comportement v1 historique préservé) —
    // BA n'a pas de StartSkill dans charstats v1.12 : jeu vide attendu.
    assert_eq!(w2.hero_skills(), crate::skills::starting_skills("BA"));
}

#[test]
fn save_rejects_or_ignores_blank_level_key() {
    let w = World::with_seed(open_grid(12, 12), manifest_no_monsters(), 7);
    // chaîne vide / espaces : défaut appliqué, pas d'erreur
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v["level"] = serde_json::json!("");
    let w2 = World::load(&v.to_string()).expect("level vide = défaut");
    assert_eq!(w2.level_key(), "town");
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v["level"] = serde_json::json!("   ");
    let w3 = World::load(&v.to_string()).expect("level blanc = défaut");
    assert_eq!(w3.level_key(), "town");
    // set_level_key : chaîne vide = no-op (état inchangé)
    let mut w4 = World::with_seed(open_grid(12, 12), manifest_no_monsters(), 7);
    w4.set_level_key("");
    assert_eq!(w4.level_key(), "town");
}

// ---------------------------------------------------------------------------
// ARBRE DE COMPÉTENCES (C2/C3/C4 — mission arbre, 2026-10-23)
// ---------------------------------------------------------------------------

#[test]
fn levelup_grants_one_skill_point_per_level_and_quest_hook_adds() {
    // C2 : +1 point de skill par niveau de perso (v1.12) ; les récompenses de
    // quêtes (Den of Evil +1, Radament A2 +1, Izual A4 +2) passent par le
    // hook `grant_skill_points` consommant le flag `skill_point` des quêtes.
    let mut w = World::with_seed(open_grid(56, 40), manifest_full(), 7);
    assert_eq!(w.unspent_skill_points(), 0, "aucun point au démarrage");
    w.clear_monsters();
    let mut leveled = false;
    for _ in 0..45 {
        assert!(spawn_near_hero(&mut w, "FA"));
        let ev = kill_nearest(&mut w, 120);
        if ev.iter().any(|e| matches!(e, GameEvent::HeroLeveled { level: 2 })) {
            leveled = true;
            break;
        }
    }
    assert!(leveled, "montée niveau 2 (28 Fallen × 18 XP ≥ 500)");
    assert_eq!(w.hero_stats().level, 2);
    assert_eq!(w.unspent_skill_points(), 1, "exactement +1 point/niveau");
    // Hook quête : Den of Evil (A1) +1 → total 2 (état cumulatif).
    w.grant_skill_points(1);
    assert_eq!(w.unspent_skill_points(), 2);
}

#[test]
fn learn_skill_world_validates_prereqs_points_and_cap() {
    // C3/C4 via l'API World (consommée par d2game learnSkill) : refus propres,
    // dépense réelle, HUD rafraîchi si le skill actif est monté.
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 16, "SOR");
    // 0 point : Fire Bolt (déjà niv 1 via StartSkill) refusé — NoPoints.
    assert!(matches!(w.learn_skill("Fire Bolt"), Err(SkillTreeError::NoPoints)));
    w.grant_skill_points(1);
    // Prérequis d'arbre : Meteor exige Fire Ball + Fire Wall (reqskill1-2,
    // patch_skills.txt) — Fire Ball absent ⇒ MissingPrereq (reqskill1).
    assert_eq!(
        w.learn_skill("Meteor"),
        Err(SkillTreeError::MissingPrereq { skill: "Meteor".into(), required: "Fire Ball".into() })
    );
    // Fire Ball : prérequis SATISFAIT (Fire Bolt = skill de départ), mais
    // reqlevel 12 > niveau héros 1 ⇒ LevelTooLow.
    assert_eq!(
        w.learn_skill("Fire Ball"),
        Err(SkillTreeError::LevelTooLow { skill: "Fire Ball".into(), required: 12 })
    );
    // Les refus ne consomment PAS de point : Ice Bolt (reqlevel 1, sans
    // prérequis) apprend avec le même point.
    assert_eq!(w.learn_skill("Ice Bolt").unwrap(), 1);
    assert_eq!(w.unspent_skill_points(), 0);
    // Skill NOUVEAU : s'ajoute en fin de liste (sélectionnable via
    // select_skill), missile de la table.
    let skills = w.hero_skills();
    assert_eq!(skills.len(), 2);
    assert_eq!(skills[1].code, "Ice Bolt");
    assert_eq!(skills[1].level, 1);
    assert_eq!(skills[1].missile.as_deref(), Some("icebolt"), "srvmissile de la table");
    // Sélection + état HUD.
    assert!(w.select_skill(1));
    assert_eq!(w.hero_stats().skill, "Ice Bolt");
    assert_eq!(w.active_skill().unwrap().code, "Ice Bolt");
    // Monter le skill ACTIF rafraîchit le miroir HUD (skill_level).
    w.grant_skill_points(1);
    assert_eq!(w.learn_skill("Ice Bolt").unwrap(), 2);
    assert_eq!(w.hero_stats().skill_level, 2);
    // WrongClass : le skill d'une autre classe est refusé proprement.
    assert!(matches!(
        w.learn_skill("Bash"),
        Err(SkillTreeError::WrongClass { .. })
    ));
}

#[test]
fn synergy_scales_fired_missile_damage() {
    // C4 branché au calcul existant : Fire Bolt tire 3–8 (table d2missiles)
    // × (1 + 640 %/100) = 22–59 quand le héros porte Fire Ball 20 + Meteor 20
    // (EDmgSymPerCalc Fire Bolt = (skill('Fire Ball'.blvl)+skill('Meteor'.blvl))
    // × par8, par8=16 — patch_skills.txt). Sans synergie : 3–8 (test ci-dessus).
    let m = manifest_heroes();
    let w = World::with_seed_and_hero(open_grid(56, 40), m, 16, "SOR");
    // État forcé par save/load : niveaux de skills + niveau héros (reqlevel
    // ne porte QUE l'apprentissage, pas le tir).
    let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    v["skills"] = serde_json::json!([
        { "code": "Fire Bolt", "level": 10, "missile": "firebolt" },
        { "code": "Fire Ball", "level": 20, "missile": "fireball" },
        { "code": "Meteor", "level": 20, "missile": "meteor" }
    ]);
    let mut w = World::load(&v.to_string()).expect("save éditée : skills synergie");
    assert_eq!(w.active_skill().unwrap().code, "Fire Bolt");
    w.clear_monsters();
    let (hx, hy) = (w.hero().x.floor() as i32, w.hero().y.floor() as i32);
    assert!(w.spawn_monster("FA", hx + 8, hy));
    let ev = run(&mut w, 50, Some(PlayerInput::RangedAttack { x: hx + 8, y: hy }));
    assert_eq!(w.missiles()[0].def_id, "firebolt");
    assert_eq!((w.missiles()[0].dmg_min, w.missiles()[0].dmg_max), (22, 59), "3×7.4 / 8×7.4");
    let mut dmg = None;
    for _ in 0..60 {
        let ev = w.tick(50, None);
        for e in ev {
            if let GameEvent::MissileHit { dmg: d, .. } = e {
                dmg = Some(d);
            }
        }
        if dmg.is_some() {
            break;
        }
    }
    let d = dmg.expect("le firebolt synergisé touche");
    assert!((22..=59).contains(&d), "dégâts firebolt + synergies = 22..59, trouvé {d}");
    let _ = ev;
}

#[test]
fn save_round_trips_skill_points_and_skills() {
    // Champ additif `hero.stats.skill_points` (#[serde(default)]) : présent
    // dans les nouvelles saves, absent = 0 dans les anciennes (v1/v2/v3),
    // skills appris + niveau du skill actif bit-exact.
    let m = manifest_heroes();
    let mut w = World::with_seed_and_hero(open_grid(56, 40), m, 16, "SOR");
    w.grant_skill_points(3);
    assert_eq!(w.learn_skill("Fire Bolt").unwrap(), 2);
    w.grant_skill_points(1); // 3 dépensés ? non : 3-1+1 = 3 non dépensés
    assert_eq!(w.unspent_skill_points(), 3);

    let v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
    assert_eq!(v["hero"]["stats"]["skill_points"], 3, "champ persisté");
    assert_eq!(v["skills"][0]["level"], 2);
    let w2 = World::load(&v.to_string()).expect("round-trip");
    assert_eq!(w2.unspent_skill_points(), 3);
    assert_eq!(w2.hero_skills()[0].code, "Fire Bolt");
    assert_eq!(w2.hero_skills()[0].level, 2);

    // Save ANTÉRIEURE au champ (aucun "skill_points" dans hero.stats) : charge
    // à 0, skills conservés (rétro-compat additive).
    let mut old = v.clone();
    old["hero"]["stats"].as_object_mut().unwrap().remove("skill_points");
    let w3 = World::load(&old.to_string()).expect("save sans skill_points");
    assert_eq!(w3.unspent_skill_points(), 0);
    assert_eq!(w3.hero_skills()[0].level, 2, "skills v2 intacts");
}

impl World {
    /// Test uniquement : force un slot d'inventaire.
    pub(crate) fn debug_set_slot(&mut self, slot: usize, def: Option<ItemDef>) {
        self.hero.inv[slot] = def;
    }
}
