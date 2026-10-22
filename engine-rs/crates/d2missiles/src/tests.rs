//! Tests `d2missiles` — trajectoire, portée, collision, dégâts, murs,
//! déterminisme, sérialisation. Graines fixes, zéro IO.

use crate::*;

/// Grille vide : aucune entité touchée.
fn no_hit(_: f32, _: f32) -> Option<usize> {
    None
}

#[test]
fn real_table_carries_patch_missiles_values() {
    let sys = MissileSystem::new();
    assert_eq!(sys.defs().len(), 10, "10 définitions MVP embarquées");

    // arrow : Vel=24, Range=40, HitClass=10, CelFile=Arrow (missiles.txt ligne Id=0).
    let a = sys.def("arrow").expect("arrow dans la table");
    assert_eq!((a.vel, a.range, a.hit_class), (24.0, 40.0, 10));
    assert_eq!(a.cel_file, "Arrow");
    assert_eq!(a.dmg_src, DmgSrc::Weapon);
    assert_eq!(a.elem, DamageType::Physical);

    // bolt : Vel=24, Range=50, CelFile=XBowBolt (Id=31).
    let b = sys.def("bolt").expect("bolt");
    assert_eq!((b.vel, b.range), (24.0, 50.0));
    assert_eq!(b.cel_file, "XBowBolt");

    // firebolt : Vel=20, Range=50, feu, 3–8 (Id=58 ; EType du skill Fire Bolt).
    let f = sys.def("firebolt").expect("firebolt");
    assert_eq!((f.vel, f.range), (20.0, 50.0));
    assert_eq!(f.elem, DamageType::Fire);
    assert_eq!((f.dmg_min, f.dmg_max, f.dmg_src), (3, 8, DmgSrc::Skill));

    // icebolt : Vel=12, froid, 3–7.
    let i = sys.def("icebolt").unwrap();
    assert_eq!((i.vel, i.elem), (12.0, DamageType::Cold));
    assert_eq!((i.dmg_min, i.dmg_max), (3, 7));

    // chargedbolt : Vel=12, Range=98 (le plus long), foudre 2–6, HitClass=64.
    let c = sys.def("chargedbolt").unwrap();
    assert_eq!((c.vel, c.range, c.hit_class), (12.0, 98.0, 64));
    assert_eq!(c.elem, DamageType::Lightning);

    // magicarrow : EType=mag dans missiles.txt, +1 dégât (MinDam=1 skills.txt).
    let m = sys.def("magicarrow").unwrap();
    assert_eq!(m.elem, DamageType::Magic);
    assert_eq!((m.dmg_min, m.dmg_max), (1, 1));

    // Code inconnu ⇒ None, jamais de panic.
    assert!(sys.def("nainportequoi").is_none());
}

#[test]
fn spawn_aims_and_flies_straight() {
    let mut sys = MissileSystem::with_seed(1);
    let id = sys.spawn("arrow", (10.0, 10.0), (30.0, 10.0)).expect("arrow existe");
    // 1 s de vol (5 ticks de 200 ms, dt tronqué à 200 max) à 24 tuiles/s :
    // avance de 24 tuiles vers l'est.
    for _ in 0..5 {
        sys.tick(200, &mut no_hit);
    }
    let m = sys.get(id).expect("en vol (portée 40 > 24)");
    assert!((m.x - 34.0).abs() < 1e-4, "x = 10 + 24×1 s, trouvé {}", m.x);
    assert!((m.y - 10.0).abs() < 1e-4);
    assert!((m.travelled - 24.0).abs() < 1e-3);
    // Direction normalisée × vel : vx = 24, vy = 0.
    assert!((m.vx - 24.0).abs() < 1e-4 && m.vy.abs() < 1e-4);

    // Diagonale : direction unitaire × vel, distance exacte.
    let mut s2 = MissileSystem::with_seed(1);
    let id2 = s2.spawn("arrow", (0.0, 0.0), (3.0, 4.0)).unwrap(); // distance 5
    let m2 = s2.get(id2).unwrap();
    assert!((m2.vx - 24.0 * 3.0 / 5.0).abs() < 1e-4);
    assert!((m2.vy - 24.0 * 4.0 / 5.0).abs() < 1e-4);
}

#[test]
fn missile_expires_at_real_range() {
    // arrow : range 40, vel 24 → expiration au tick où travelled atteint 40
    // (1667 ms de vol → 34 ticks de 50 ms = 1700 ms).
    let mut sys = MissileSystem::with_seed(2);
    let id = sys.spawn("arrow", (0.0, 0.0), (1.0, 0.0)).unwrap();
    let mut arrow_ms = 0;
    let mut expired = false;
    for _ in 0..60 {
        arrow_ms += 50;
        if sys.tick(50, &mut no_hit).iter().any(|e| matches!(e, MissileEvent::Expired { id: i } if *i == id)) {
            expired = true;
            break;
        }
    }
    assert!(expired, "Expired émis à portée 40");
    assert_eq!(arrow_ms, 1700, "40 tuiles / 24 tps = 1667 ms → 34 ticks de 50 ms");
    assert!(sys.is_empty(), "le missile expire et disparaît");
    // throwaxe : portée 20 — expire bien plus tôt (833 ms → 850 ms).
    let mut s2 = MissileSystem::with_seed(2);
    let axe = s2.spawn("throwaxe", (0.0, 0.0), (1.0, 0.0)).unwrap();
    let mut axe_ms = 0;
    for _ in 0..60 {
        axe_ms += 50;
        if s2.tick(50, &mut no_hit).iter().any(|e| matches!(e, MissileEvent::Expired { id: i } if *i == axe)) {
            break;
        }
    }
    assert_eq!(axe_ms, 850, "20 tuiles / 24 tps = 833 ms → 17 ticks de 50 ms");
}

#[test]
fn hit_reports_target_and_weapon_damage() {
    let mut sys = MissileSystem::with_seed(3);
    // Cible à (15, 10) : la closure renvoie Some(0) quand le missile passe
    // dans le rayon 0,7 tuile (rayon de collision côté d2rules).
    let id = sys.spawn_weapon("arrow", (10.0, 10.0), (30.0, 10.0), 4, 7).expect("spawn");
    let hit = sys
        .tick(1500, &mut |x, y| {
            if ((x - 15.0).powi(2) + (y - 10.0).powi(2)).sqrt() <= 0.7 {
                Some(0)
            } else {
                None
            }
        })
        .into_iter()
        .find_map(|e| match e {
            MissileEvent::Hit { id: i, target, dmg, .. } if i == id => Some((target, dmg)),
            _ => None,
        })
        .expect("Hit sur la trajectoire");
    assert_eq!(hit.0, 0, "index d'entité restitué tel quel");
    assert!((4..=7).contains(&hit.1), "dégâts dans la fourchette d'arme résolue (4..=7), trouvé {}", hit.1);
    assert!(sys.is_empty(), "CollideKill=1 : le missile disparaît sur le coup");
}

#[test]
fn skill_bolts_roll_their_own_damage() {
    // 30 tirs firebolt (graines différentes) : dégâts 3–8, les deux bornes sortent.
    let mut hits = Vec::new();
    for i in 0..30_u64 {
        let mut s = MissileSystem::with_seed(100 + i);
        s.spawn("firebolt", (0.0, 0.0), (10.0, 0.0)).unwrap();
        hits.push(
            s.tick(3000, &mut |_x, _y| Some(7))
                .iter()
                .find_map(|e| match e {
                    MissileEvent::Hit { dmg, .. } => Some(*dmg),
                    _ => None,
                })
                .expect("hit garanti (portée 50 > 10)"),
        );
    }
    assert!(hits.iter().all(|d| (3..=8).contains(d)), "firebolt 3–8 officiel, trouvé {hits:?}");
    assert!(hits.contains(&3) && hits.contains(&8), "les deux bornes sortent");
}

#[test]
fn walls_block_and_kill_the_missile() {
    let mut sys = MissileSystem::with_seed(5);
    let id = sys.spawn("arrow", (0.0, 0.0), (20.0, 0.0)).unwrap();
    // Mur sur la bande x ∈ [5, 6) : le missile meurt dessus, jamais de Hit.
    let mut blocked = false;
    for _ in 0..100 {
        if sys
            .tick_walls(50, &mut |_x, _y| None, &mut |x, _| (5.0..6.0).contains(&x))
            .iter()
            .any(|e| matches!(e, MissileEvent::Blocked { id: i } if *i == id))
        {
            blocked = true;
            break;
        }
    }
    assert!(blocked, "Blocked émis au mur (5 tuiles ≈ 208 ms de vol)");
    assert!(sys.is_empty());
}

#[test]
fn substepping_never_tunnels_a_target() {
    // Cible ponctuelle à mi-trajectoire : une flèche à 24 tuiles/s traverse
    // 1,2 tuile par tick de 50 ms — les sous-pas de ≤ 0,5 tuile doivent la
    // voir (rayon de collision 0,7 autour du point (5, 0), comme d2rules).
    let mut sys = MissileSystem::with_seed(6);
    sys.spawn("arrow", (0.0, 0.0), (10.0, 0.0)).unwrap();
    let mut hit = false;
    for _ in 0..30 {
        if sys
            .tick(50, &mut |x, y| {
                if ((x - 5.0).powi(2) + y.powi(2)).sqrt() <= 0.7 {
                    Some(0)
                } else {
                    None
                }
            })
            .iter()
            .any(|e| matches!(e, MissileEvent::Hit { target: 0, .. }))
        {
            hit = true;
            break;
        }
    }
    assert!(hit, "cible détectée malgré 1,2 tuile/tick");
}

#[test]
fn same_seed_same_events_tick_after_tick() {
    let run = |seed: u64| -> Vec<MissileEvent> {
        let mut sys = MissileSystem::with_seed(seed);
        let mut ev = Vec::new();
        for i in 0..6 {
            if i == 0 {
                sys.spawn("chargedbolt", (0.0, 0.0), (9.0, 3.0)).unwrap();
            }
            ev.extend(sys.tick(100, &mut |x, y| {
                if ((x - 6.0).powi(2) + (y - 2.0).powi(2)).sqrt() <= 0.8 {
                    Some(2)
                } else {
                    None
                }
            }));
        }
        ev
    };
    assert_eq!(run(0xD2), run(0xD2), "même graine ⇒ événements identiques");
    // Graines différentes : quasi certain de diverger au premier jet (2..=6).
    assert_ne!(run(0xD2), run(0xD3));
}

#[test]
fn in_flight_missiles_survive_serialization() {
    let mut sys = MissileSystem::with_seed(9);
    let a = sys.spawn("arrow", (0.0, 0.0), (30.0, 0.0)).unwrap();
    let b = sys.spawn("firebolt", (0.0, 0.0), (0.0, 30.0)).unwrap();
    sys.tick(500, &mut no_hit);
    assert_eq!(sys.len(), 2);

    let json = serde_json::to_string(&sys).expect("sérialisation");
    let mut back: MissileSystem = serde_json::from_str(&json).expect("désérialisation");
    assert_eq!(back.len(), 2, "missiles en vol préservés");
    assert_eq!(back.get(a).map(|m| (m.x, m.y)), sys.get(a).map(|m| (m.x, m.y)));

    // Continuation identique : mêmes événements, mêmes positions.
    let ev_a = sys.tick(400, &mut no_hit);
    let ev_b = back.tick(400, &mut no_hit);
    assert_eq!(ev_a, ev_b, "le RNG sérialisé poursuit le même flux");
    assert_eq!(sys.missiles(), back.missiles());
    assert_eq!(sys.get(b).map(|m| (m.y, m.travelled)), back.get(b).map(|m| (m.y, m.travelled)));
}

#[test]
fn custom_table_from_json_overrides() {
    let v = serde_json::json!({
        "missiles": [ { "id": "Boulder", "vel": 10.0, "range": 15.0, "elem": "fire",
                        "dmgMin": 5, "dmgMax": 9, "dmgSrc": "skill", "celFile": "Boulder", "animLen": 3 } ]
    });
    let mut sys = MissileSystem::from_json(v).expect("table custom");
    assert!(sys.def("boulder").is_some(), "insensible à la casse");
    assert!(sys.def("arrow").is_none(), "pas de table réelle en override");
    sys.spawn("boulder", (0.0, 0.0), (10.0, 0.0)).unwrap();
    let dmg = sys
        .tick(2000, &mut |_x, _y| Some(1))
        .iter()
        .find_map(|e| match e {
            MissileEvent::Hit { dmg, .. } => Some(*dmg),
            _ => None,
        })
        .expect("hit");
    assert!((5..=9).contains(&dmg));

    // JSON invalide : Err, pas de panic.
    assert!(MissileSystem::from_json(serde_json::json!({})).is_err());
    assert!(MissileSystem::from_json(serde_json::json!({"missiles": [{"vel": 1.0}]})).is_err());
}
