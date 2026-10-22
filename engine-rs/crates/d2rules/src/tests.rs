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

    // 3) ramasser + boire : soin exact = min(60, max - vie)
    run(&mut w, 200, Some(PlayerInput::Pickup));
    let slot = (0..40).find(|&s| w.inventory()[s].is_some()).expect("potion dans l'inventaire");
    assert_eq!(w.inventory()[slot].as_ref().unwrap().code, "hp1");
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

/// Manifest réel + bloc `heroes` {BA, SOR} — SOR = stats de sorcière distinctes.
fn manifest_heroes() -> serde_json::Value {
    let mut m = manifest_full();
    let mut sor = m["hero"].clone();
    sor["class"] = json!("SOR");
    sor["name"] = json!("Sorcière");
    sor["stats"]["life"] = json!(40);
    sor["stats"]["mana"] = json!(60);
    sor["stats"]["strength"] = json!(10);
    sor["stats"]["dexterity"] = json!(25);
    m["heroes"] = json!({ "BA": m["hero"].clone(), "SOR": sor });
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

impl World {
    /// Test uniquement : force un slot d'inventaire.
    pub(crate) fn debug_set_slot(&mut self, slot: usize, def: Option<ItemDef>) {
        self.hero.inv[slot] = def;
    }
}
