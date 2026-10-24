//! Sauvegarde / chargement JSON (la page l'appellera pour localStorage).
//!
//! Le format embarque la grille (RLE trivial : '1' marchable, '0' bloqué) et le
//! manifest — `World::load(json)` n'a pas d'autres paramètres, conformément au
//! contrat. Versionnée (`version: 3`) ; tout écart ⇒ `Err(())`. Rétro-compat :
//! une sauvegarde v1 (sans `hero_code`, `missiles` ni `skills`) ou v2 (sans
//! `level`) se charge — défauts du manifest / systèmes vides / skills de départ
//! de la classe / niveau "town".

use super::world::*;
use crate::difficulty::Difficulty;
use crate::rng::Rng;
use d2missiles::MissileSystem;
use crate::{AnimState, Entity, HeroSkill, HeroStats, ItemDef, ItemOnGround};
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
    /// Clé du niveau courant dans le manifest (`"town"`, `"bloodmoor"`…) —
    /// absent des sauvegardes v1/v2 antérieures au multi-niveaux persistant
    /// (rétro-compat : défaut "town", cf. `DEFAULT_LEVEL_KEY`).
    #[serde(default)]
    level: Option<String>,
    /// Héros.
    hero: HeroSave,
    /// Monstres (même ordre que `monsters()`).
    monsters: Vec<MonSave>,
    /// Objets au sol.
    items: Vec<ItemOnGround>,
    /// Projectiles en vol (JSON du MissileSystem ; absent des sauvegardes
    /// v1 antérieures aux missiles — rétro-compat : système vide).
    #[serde(default)]
    missiles: Option<serde_json::Value>,
    /// Compétences du héros (absent des sauvegardes v1 antérieures aux skills
    /// — rétro-compat : skills de départ de la classe, cf. `skills.rs`).
    #[serde(default)]
    skills: Option<Vec<HeroSkill>>,
    /// Index du skill actif (absent v1 — défaut par classe).
    #[serde(default)]
    active_skill: Option<usize>,
    /// Waypoints activés (slots du panneau, W19) — absent des sauvegardes
    /// antérieures aux waypoints : rétro-compat, actifs vides. Champ
    /// OPTIONNEL additive : pas de bump de version (les saves existantes
    /// restent valides dans les deux sens — un champ inconnu est ignoré).
    #[serde(default)]
    waypoints: Option<Vec<u8>>,
    /// Difficulté (W08/K) — absent des sauvegardes antérieures à
    /// impl-difficultes (rétro-compat : Normal). Champ OPTIONNEL : un autre
    /// chantier fait évoluer save.rs en parallèle — l'intégrateur fusionne.
    #[serde(default)]
    difficulty: Option<Difficulty>,
    /// NPC de ville (W16, `npc.rs`) — absent des sauvegardes antérieures aux
    /// villes vivantes (rétro-compat : roster vide, l'attach d2game
    /// re-spawn). Champ OPTIONNEL `#[serde(default)]` : les saves existantes
    /// restent valides dans les deux sens. Les NPC sont PERSISTANTS
    /// (position + compteur de dialogues + patrouille résolue).
    #[serde(default)]
    npcs: Option<Vec<crate::npc::NpcState>>,
    /// Journal de quêtes (W13, JSON de `QuestLog::to_json`) — absent des
    /// sauvegardes antérieures (rétro-compat : pas de journal branché).
    /// Champ OPTIONNEL ; un journal PRÉSENT mais invalide ⇒ `Err` (contrat
    /// quests.rs : tout écart = pas d'état incohérent).
    #[serde(default)]
    quests: Option<serde_json::Value>,
    /// Mercenaire (H, `mercenary.rs`) — absent des sauvegardes antérieures au
    /// chantier mercenaires (rétro-compat : aucun mercenaire). Champ
    /// OPTIONNEL ADDITIF `#[serde(default)]` : save v3 inchangée, les stats
    /// effectives sont recalculées au chargement depuis `hireling.txt`.
    #[serde(default)]
    mercenary: Option<crate::mercenary::MercSave>,
    /// Corps au sol (J1, `corpse.rs`) : position + inventaire du héros
    /// mort. ABSENT des sauvegardes antérieures au corps (rétro-compat :
    /// aucun corps) — champ OPTIONNEL additif, pas de bump de version.
    #[serde(default)]
    corpse: Option<crate::corpse::Corpse>,
    /// Journal de quêtes Acte I SPEC (quests_actes.rs, JSON de
    /// `ActQuests::to_json`) — absent des sauvegardes antérieures (rétro-compat
    /// : pas de journal spec branché). Champ OPTIONNEL ADDITIF
    /// `#[serde(default)]` : save v3 inchangée ; présent mais invalide ⇒ Err
    /// (contrat quests_actes.rs : tout écart = pas d'état incohérent).
    #[serde(default)]
    quest_act1: Option<serde_json::Value>,
    /// Journal de quêtes Actes II-III SPEC (quests_actes2.rs, JSON de
    /// `Act2Quests::to_json`) — absent des sauvegardes antérieures
    /// (rétro-compat : pas de journal branché). Champ OPTIONNEL ADDITIF
    /// `#[serde(default)]` : save v3 inchangée ; présent mais invalide ⇒
    /// Err (contrat quests_actes2.rs : tout écart = pas d'état incohérent).
    #[serde(default)]
    quest_actes2: Option<serde_json::Value>,
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
    /// Rang (champion/unique/boss…) — absent des sauvegardes antérieures à
    /// impl-monstres-boss (rétro-compat : Normal).
    #[serde(default)]
    kind: Option<MonKind>,
    /// TC de drop effective (surcharge champion/unique/boss) — absent v2.1.
    #[serde(default)]
    tc: Option<Option<String>>,
    /// Résists effectifs de la difficulté de spawn (W08/K, ordre
    /// ResDm/Ma/Fi/Li/Co/Po) — absent des saves antérieures (rétro-compat :
    /// tout à 0 = Normal).
    #[serde(default)]
    res: Option<[i32; 6]>,
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
        level: Some(w.level_key.clone()),
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
        skills: Some(w.hero.skills.clone()),
        active_skill: Some(w.hero.active_skill),
        difficulty: Some(w.difficulty),
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
                kind: Some(d.kind),
                tc: Some(d.tc.clone()),
                res: Some(d.res),
            })
            .collect(),
        items: w.items.clone(),
        corpse: w.corpse.clone(), // J1 : champ additif (rétro-compat v3 sans corps)
        quest_act1: w.quests_act1.as_ref().map(|log| {
            serde_json::from_str(&log.to_json()).expect("d2rules: sérialisation quêtes acte I")
        }),
        quest_actes2: w.quests_actes2.as_ref().map(|log| {
            serde_json::from_str(&log.to_json()).expect("d2rules: sérialisation quêtes actes II-III")
        }),
        missiles: Some(serde_json::to_value(&w.missiles).expect("d2rules: sérialisation missiles")),
        waypoints: Some(w.wp.active.clone()),
        npcs: Some(w.npcs.clone()),
        quests: w.quests.as_ref().map(|log| {
            serde_json::from_str(&log.to_json()).expect("d2rules: sérialisation journal quêtes")
        }),
        mercenary: w.merc.as_ref().map(crate::mercenary::MercSave::of),
    };
    serde_json::to_string(&save).expect("d2rules: sérialisation save")
}

/// Reconstruit le monde (contracte `World::load`).
pub(crate) fn load_json(json: &str) -> Result<World, ()> {
    let s: Save = serde_json::from_str(json).map_err(|_| ())?;
    // versions acceptées : plage [SAVE_VERSION_OLDEST..=SAVE_VERSION] —
    // 1 (sans skills/hero_code/level), 2 (sans level), 3 (courant).
    if s.version < SAVE_VERSION_OLDEST || s.version > SAVE_VERSION {
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
    // clé de niveau : champ v3, défaut "town" pour les sauvegardes v1/v2
    // (rétro-compat multi-niveaux — le Camp des Rousses est le niveau
    // historique de tous les saves antérieurs). Chaîne vide = défaut aussi.
    let level_key = s
        .level
        .clone()
        .map(|k| k.trim().to_ascii_lowercase())
        .filter(|k| !k.is_empty())
        .unwrap_or_else(|| super::world::DEFAULT_LEVEL_KEY.to_string());
    // skills : champ v2 sinon skills de départ de la classe (sauvegardes v1) ;
    // index actif hors bornes ⇒ défaut par classe.
    let skills = match s.skills {
        Some(v) => v,
        None => crate::skills::starting_skills(&hero_code),
    };
    let active_skill = match s.active_skill {
        Some(i) if i < skills.len() => i,
        _ => crate::skills::default_active(&hero_code),
    };
    let world = World {
        w,
        h,
        grid,
        manifest: s.manifest.clone(),
        hero_code: hero_code.clone(),
        level_key,
        hero_tpl: super::data::parse_hero_code(&s.manifest, &hero_code),
        mon_tpls: super::data::parse_monsters(&s.manifest),
        // Tables de loot statiques (embarquées) — pas d'état à recharger.
        loot: super::loot::build(),
        hero_ent: s.hero.ent,
        hero: {
            let mut stats = s.hero.stats;
            // EXP-H2 (stamina) : les saves antérieures au champ chargent
            // `stamina = max_stamina = 0` (serde default) — reconcile au
            // gabarit du manifest et aligne l'accumulateur (transitoire).
            if stats.max_stamina == 0 {
                stats.max_stamina = super::data::parse_hero_code(&s.manifest, &hero_code).stamina;
                stats.stamina = stats.max_stamina;
            }
            let stamina_milli = (stats.stamina as i64) * 1000;
            HeroData {
                stats,
                path: s.hero.path,
                swing_ms: s.hero.swing_ms,
                dead_ms: s.hero.dead_ms,
                anim_ms: s.hero.anim_ms,
                run: false, // transient (touche Maj) — jamais persisté
                spawn: s.hero.spawn,
                inv,
                skills,
                active_skill,
                stamina_milli, // EXP-H2 : transitoire, recalé à la valeur chargée
            }
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
                kind: m.kind.unwrap_or_default(),
                tc: m.tc.clone().unwrap_or(None),
                res: m.res.unwrap_or([0; 6]),
            })
            .collect(),
        items: s.items,
        missiles: match s.missiles {
            Some(v) => serde_json::from_value(v).map_err(|_| ())?,
            None => MissileSystem::with_seed(s.rng ^ 0x4D49_5353_1A4E_0001),
        },
        // Waypoints W19 : champ optionnel (saves antérieures = actifs vides) ;
        // `here` n'est pas persisté — d2game le rattache au niveau chargé.
        wp: crate::waypoints::WaypointState {
            here: None,
            active: s.waypoints.clone().unwrap_or_default(),
        },
        // J1 : corps au sol (saves antérieures = aucun) ; `corpse_pending`
        // est transitoire — jamais persisté.
        corpse: s.corpse,
        corpse_pending: false,
        rng: Rng::new(s.rng),
        now_ms: s.now_ms,
        difficulty: s.difficulty.unwrap_or_default(),
        // NPC de ville (W16) : champ optionnel (saves antérieures = roster
        // vide — l'attach d2game re-spawn). Validation stricte : index de
        // définition dans la table + entité dans les bornes (boucle sanity
        // plus bas) — tout écart ⇒ Err, jamais d'état incohérent.
        npcs: {
            let list = s.npcs.unwrap_or_default();
            if list.iter().any(|n| n.def >= crate::npc::NPCS.len()) {
                return Err(());
            }
            list
        },
        // Journal de quêtes : JSON versionné strict (`QuestLog::from_json`) —
        // présent mais invalide ⇒ Err (contrat quests.rs).
        quests: match s.quests {
            Some(v) => Some(crate::quests::QuestLog::from_json(&v.to_string()).map_err(|_| ())?),
            None => None,
        },
        pending_events: Vec::new(),
        // Mercenaire H : champ optionnel (saves antérieures = aucun) ; stats
        // effectives recalculées depuis hireling.txt au niveau sauvegardé.
        // Record introuvable (table d'une autre version) ⇒ mercenaire IGNORÉ
        // (jamais un save invalide pour un compagnon).
        merc: match s.mercenary {
            Some(ms) => match ms.rebuild(s.difficulty.unwrap_or_default()) {
                Ok(m) => Some(m),
                Err(()) => None,
            },
            None => None,
        },
        merc_target: None, // transient — jamais persisté
        deferred: Vec::new(),
        // IA P1 (ai_behaviors) : état DÉRIVÉ des gabarits (archétypes) — les
        // cadavres et overlays (panique/incantation) sont transitoires et ne
        // survivent pas au rechargement, comme `corpse_pending`.
        ai: crate::ai_behaviors::AiState::default(),
        // Journal de quêtes Acte I spec : JSON versionné strict
        // (`ActQuests::from_json`) — présent mais invalide ⇒ Err.
        quests_act1: match s.quest_act1 {
            Some(v) => {
                Some(crate::quests_actes::ActQuests::from_json(&v.to_string()).map_err(|_| ())?)
            }
            None => None,
        },
        // Journal de quêtes Actes II-III spec : JSON versionné strict
        // (`Act2Quests::from_json`) — présent mais invalide ⇒ Err.
        quests_actes2: match s.quest_actes2 {
            Some(v) => {
                Some(crate::quests_actes2::Act2Quests::from_json(&v.to_string()).map_err(|_| ())?)
            }
            None => None,
        },
    };
    let mut world = world;
    // sanité minimale : gabarits connus, entités dans les bornes
    for d in &world.mon_dat {
        if d.tpl >= world.mon_tpls.len() {
            return Err(());
        }
    }
    // IA P1 : reconstruit les archétypes depuis les codes gabarits chargés —
    // APRÈS la validation des gabarits (un tpl hors bornes ⇒ Err, jamais un
    // panic : boss ajoutés paresseusement au spawn, absent du manifest).
    world.ai = crate::ai_behaviors::AiState::rebuild(&world);
    for e in world
        .mon_ent
        .iter()
        .chain(world.npcs.iter().map(|n| &n.ent))
        .chain(std::iter::once(&world.hero_ent))
    {
        if e.x < 0.0 || e.y < 0.0 || e.x >= w as f32 || e.y >= h as f32 {
            return Err(());
        }
    }
    // J1 : le corps, s'il en existe un, doit être dans les bornes de la carte.
    if let Some(c) = &world.corpse {
        if c.x < 0.0 || c.y < 0.0 || c.x >= w as f32 || c.y >= h as f32 {
            return Err(()); // corps hors carte : état incohérent
        }
    }
    if world.missiles.missiles().iter().any(|m| {
        m.x < 0.0 || m.y < 0.0 || m.x >= w as f32 || m.y >= h as f32
    }) {
        return Err(()); // missile hors carte : état incohérent
    }
    if world.hero_ent.anim == AnimState::DD && world.hero.dead_ms == 0 {
        return Err(()); // anim DD héros sans compte de mort : état incohérent
    }
    // mercenaire H : entité et missiles dans les bornes (symétrique héros)
    if let Some(m) = &world.merc {
        if m.ent.x < 0.0 || m.ent.y < 0.0 || m.ent.x >= w as f32 || m.ent.y >= h as f32 {
            return Err(());
        }
        if m.missiles.missiles().iter().any(|mm| {
            mm.x < 0.0 || mm.y < 0.0 || mm.x >= w as f32 || mm.y >= h as f32
        }) {
            return Err(()); // missile merc hors carte : état incohérent
        }
    }
    Ok(world)
}
