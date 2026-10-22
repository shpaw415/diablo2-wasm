//! Sauvegarde / chargement JSON (la page l'appellera pour localStorage).
//!
//! Le format embarque la grille (RLE trivial : '1' marchable, '0' bloqué) et le
//! manifest — `World::load(json)` n'a pas d'autres paramètres, conformément au
//! contrat. Versionnée (`version: 1`) ; tout écart ⇒ `Err(())`.

use super::world::*;
use crate::rng::Rng;
use crate::{AnimState, Entity, HeroStats, ItemDef, ItemOnGround};
use serde::{Deserialize, Serialize};

/// Sauvegarde sérialisable du monde complet.
#[derive(Serialize, Deserialize)]
struct Save {
    /// Version du format.
    version: u32,
    /// État du PRNG (déterminisme exact après rechargement).
    rng: u64,
    /// Horloge de jeu.
    now_ms: u64,
    /// Grille : une chaîne par rangée, '1' marchable / '0' bloqué.
    collision: Vec<String>,
    /// Manifest tel que reçu (re-parsé au chargement).
    manifest: serde_json::Value,
    /// Code héros effectif ("BA"…) — absent des sauvegardes v1 antérieures à
    /// la sélection de personnage (rétro-compat : défaut du manifest).
    #[serde(default)]
    hero_code: Option<String>,
    /// Héros.
    hero: HeroSave,
    /// Monstres (même ordre que `monsters()`).
    monsters: Vec<MonSave>,
    /// Objets au sol.
    items: Vec<ItemOnGround>,
}

/// État complet du héros.
#[derive(Serialize, Deserialize)]
struct HeroSave {
    /// Entité (position, facing, anim).
    ent: Entity,
    /// Stats (life = PV courants).
    stats: HeroStats,
    /// Chemin restant (mouvement en vol préservé).
    path: Vec<(f32, f32)>,
    /// Swing / cooldown restant.
    swing_ms: u32,
    /// Compteur de mort (0 = vivant).
    dead_ms: u32,
    /// Horloge d'animation.
    anim_ms: u32,
    /// Point d'apparition.
    spawn: (f32, f32),
    /// Inventaire 40 slots.
    inv: Vec<Option<ItemDef>>,
}

/// État complet d'un monstre.
#[derive(Serialize, Deserialize)]
struct MonSave {
    /// Entité.
    ent: Entity,
    /// Gabarit.
    tpl: usize,
    /// PV courants.
    hp: u32,
    /// PV max.
    max_hp: u32,
    /// État FSM.
    state: MonState,
    /// Chemin restant.
    path: Vec<(f32, f32)>,
    /// Timers.
    wander_ms: u32,
    repath_ms: u32,
    atk_cd_ms: u32,
    dying_ms: u32,
    anim_ms: u32,
    /// Origine.
    home: (f32, f32),
    /// Combat scalé.
    dmg_min: u32,
    dmg_max: u32,
    th: u32,
    ac: u32,
    xp: u32,
    speed: f32,
}

fn grid_to_strings(grid: &[Vec<bool>]) -> Vec<String> {
    grid.iter()
        .map(|row| row.iter().map(|&b| if b { '0' } else { '1' }).collect())
        .collect()
}

fn strings_to_grid(rows: &[String]) -> Result<Vec<Vec<bool>>, ()> {
    let mut grid = Vec::with_capacity(rows.len());
    for row in rows {
        let mut r = Vec::with_capacity(row.len());
        for c in row.chars() {
            match c {
                '1' => r.push(false),
                '0' => r.push(true),
                _ => return Err(()),
            }
        }
        grid.push(r);
    }
    Ok(grid)
}

/// Sérialise le monde (contracte `World::save`).
pub(crate) fn save_json(w: &World) -> String {
    let save = Save {
        version: SAVE_VERSION,
        rng: w.rng.s,
        now_ms: w.now_ms,
        collision: grid_to_strings(&w.grid),
        manifest: w.manifest.clone(),
        hero_code: Some(w.hero_code.clone()),
        hero: HeroSave {
            ent: w.hero_ent,
            stats: w.hero.stats.clone(),
            path: w.hero.path.clone(),
            swing_ms: w.hero.swing_ms,
            dead_ms: w.hero.dead_ms,
            anim_ms: w.hero.anim_ms,
            spawn: w.hero.spawn,
            inv: w.hero.inv.clone(),
        },
        monsters: w
            .mon_ent
            .iter()
            .zip(w.mon_dat.iter())
            .map(|(e, d)| MonSave {
                ent: *e,
                tpl: d.tpl,
                hp: d.hp,
                max_hp: d.max_hp,
                state: d.state,
                path: d.path.clone(),
                wander_ms: d.wander_ms,
                repath_ms: d.repath_ms,
                atk_cd_ms: d.atk_cd_ms,
                dying_ms: d.dying_ms,
                anim_ms: d.anim_ms,
                home: d.home,
                dmg_min: d.dmg_min,
                dmg_max: d.dmg_max,
                th: d.th,
                ac: d.ac,
                xp: d.xp,
                speed: d.speed,
            })
            .collect(),
        items: w.items.clone(),
    };
    serde_json::to_string(&save).expect("d2rules: sérialisation save")
}

/// Reconstruit le monde (contracte `World::load`).
pub(crate) fn load_json(json: &str) -> Result<World, ()> {
    let s: Save = serde_json::from_str(json).map_err(|_| ())?;
    if s.version != SAVE_VERSION {
        return Err(());
    }
    let grid = strings_to_grid(&s.collision)?;
    if grid.is_empty() || grid[0].is_empty() {
        return Err(());
    }
    let w = grid[0].len() as i32;
    let h = grid.len() as i32;
    for row in &grid {
        if row.len() as i32 != w {
            return Err(());
        }
    }
    let inv = s.hero.inv;
    if inv.len() != INV_SLOTS {
        return Err(());
    }
    let hero_code = s
        .hero_code
        .clone()
        .unwrap_or_else(|| super::data::resolve_hero(&s.manifest, "BA"));
    let world = World {
        w,
        h,
        grid,
        manifest: s.manifest.clone(),
        hero_code: hero_code.clone(),
        hero_tpl: super::data::parse_hero_code(&s.manifest, &hero_code),
        mon_tpls: super::data::parse_monsters(&s.manifest),
        hero_ent: s.hero.ent,
        hero: HeroData {
            stats: s.hero.stats,
            path: s.hero.path,
            swing_ms: s.hero.swing_ms,
            dead_ms: s.hero.dead_ms,
            anim_ms: s.hero.anim_ms,
            spawn: s.hero.spawn,
            inv,
        },
        mon_ent: s.monsters.iter().map(|m| m.ent).collect(),
        mon_dat: s
            .monsters
            .iter()
            .map(|m| MonData {
                tpl: m.tpl,
                hp: m.hp,
                max_hp: m.max_hp,
                state: m.state,
                path: m.path.clone(),
                wander_ms: m.wander_ms,
                repath_ms: m.repath_ms,
                atk_cd_ms: m.atk_cd_ms,
                dying_ms: m.dying_ms,
                anim_ms: m.anim_ms,
                home: m.home,
                dmg_min: m.dmg_min,
                dmg_max: m.dmg_max,
                th: m.th,
                ac: m.ac,
                xp: m.xp,
                speed: m.speed,
            })
            .collect(),
        items: s.items,
        rng: Rng::new(s.rng),
        now_ms: s.now_ms,
    };
    // sanité minimale : gabarits connus, entités dans les bornes
    for d in &world.mon_dat {
        if d.tpl >= world.mon_tpls.len() {
            return Err(());
        }
    }
    for e in world.mon_ent.iter().chain(std::iter::once(&world.hero_ent)) {
        if e.x < 0.0 || e.y < 0.0 || e.x >= w as f32 || e.y >= h as f32 {
            return Err(());
        }
    }
    if world.hero_ent.anim == AnimState::DD && world.hero.dead_ms == 0 {
        return Err(()); // anim DD héros sans compte de mort : état incohérent
    }
    Ok(world)
}
