//! `World` : simulation déterministe — héros, FSM monstres, combat, XP, drops,
//! inventaire 10×4, save/load. Zéro IO, zéro rendu.

use crate::data::{self, HeroTpl, MonTpl};
use crate::path::{astar, snap_near, Grid};
use crate::rng::Rng;
use crate::{
    Entity, AnimState, GameEvent, HeroStats, ItemDef, ItemKind, ItemOnGround, PlayerInput,
};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constantes gameplay (décisions MVP, cf. REPORT.md)
// ---------------------------------------------------------------------------

/// Durée d'une frame d'animation (ms). 16 frames ⇒ anim complète 1,6 s.
pub(crate) const FRAME_MS: u32 = 100;
/// Nombre de frames par animation (standard D2).
pub(crate) const FRAMES: u32 = 16;
/// Portée d'attaque au corps à corps (distance centres de tuiles).
pub(crate) const ATTACK_RANGE: f32 = 1.5;
/// Rayon de sélection d'une cible autour du clic.
pub(crate) const PICK_RADIUS: f32 = 2.0;
/// Rayon de ramassage d'un objet au sol.
pub(crate) const PICKUP_RANGE: f32 = 2.0;
/// Durée du swing héros = cooldown d'attaque (1hs, arrondi MVP).
pub(crate) const HERO_SWING_MS: u32 = 800;
/// Cooldown d'attaque des monstres.
pub(crate) const MONSTER_ATK_CD_MS: u32 = 1500;
/// Grâce à l'entrée en état ATTACK (le monstre ne frappe pas instantanément).
pub(crate) const MONSTER_ATK_GRACE_MS: u32 = 600;
/// Rayon d'aggro (tuiles, brief MVP).
pub(crate) const AGGRO_RADIUS: f32 = 6.0;
/// Rayon de laisse : au-delà, le monstre rentre (évite les trains sur toute la carte).
pub(crate) const LEASH_RADIUS: f32 = 10.0;
/// Errance : rayon autour du point d'origine.
pub(crate) const WANDER_RADIUS: i32 = 3;
/// Re-calcul de chemin monstre vers héros.
pub(crate) const REPATH_MS: u32 = 400;
/// Durée de l'anim DD avant disparition (16 frames × 100 ms).
pub(crate) const DYING_MS: u32 = FRAME_MS * FRAMES;
/// Délai mort → réapparition.
pub(crate) const RESPAWN_MS: u32 = 2000;
/// Soin d'une `hp1` (`patch_misc.txt`, brief MVP).
pub(crate) const POTION_HEAL: u32 = 60;
/// dt max accepté par tick (anti-tunneling ; au-delà, tronqué).
pub(crate) const MAX_DT_MS: u32 = 100;
/// Niveau max MVP (seuils XP livrés jusqu'à 10).
pub(crate) const MAX_LEVEL: u32 = 10;

/// Seuils XP CUMULÉS pour atteindre le niveau N+1 — colonne Barbarian,
/// `assets/patch_experience.txt` lignes 1..=10 (GAME-DATA.md §4).
pub(crate) const XP_TABLE: [u32; 10] =
    [500, 1500, 3750, 7875, 14175, 22680, 32886, 44396, 57715, 72144];

// Facteurs MonLvl.txt ligne niveau 1 Normal (GAME-DATA.md §2.1).
/// AC effectif monstre niveau 1.
pub(crate) const MONLVL_AC: u32 = 6;
/// TH effectif monstre niveau 1.
pub(crate) const MONLVL_TH: u32 = 8;
/// Numérateur du facteur HP (×7/100).
pub(crate) const MONLVL_HP_NUM: u32 = 7;
/// Dénominateur du facteur HP.
pub(crate) const MONLVL_HP_DEN: u32 = 100;
/// Numérateur du facteur dégâts (×2/100).
pub(crate) const MONLVL_DM_NUM: u32 = 2;
/// Dénominateur du facteur dégâts.
pub(crate) const MONLVL_DM_DEN: u32 = 100;
/// Numérateur du facteur XP (×30/100).
pub(crate) const MONLVL_XP_NUM: u32 = 30;
/// Dénominateur du facteur XP.
pub(crate) const MONLVL_XP_DEN: u32 = 100;

// Table de drop MVP (brief) — remplacée par TreasureClassEx réel en phase 3.
/// % rien.
pub(crate) const DROP_NOTHING_PCT: u32 = 60;
/// % or (ensuite : % potion).
pub(crate) const DROP_GOLD_PCT: u32 = 25;
/// Version du format de sauvegarde.
pub(crate) const SAVE_VERSION: u32 = 1;
/// Grille d'inventaire MVP : 10 colonnes × 4 rangées.
pub(crate) const INV_COLS: usize = 10;
pub(crate) const INV_ROWS: usize = 4;
pub(crate) const INV_SLOTS: usize = INV_COLS * INV_ROWS;

// ---------------------------------------------------------------------------
// États internes
// ---------------------------------------------------------------------------

/// FSM monstre (interne — d2game voit l'anim via `Entity.anim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MonState {
    /// Errance aléatoire autour du point d'origine.
    Idle,
    /// Poursuite du héros (A*, re-path périodique).
    Chase,
    /// Au contact : frappes périodiques.
    Attack,
    /// Anim DD en cours, puis disparition.
    Dying,
}

/// Données héros (l'`Entity` est à côté, cf. `World.hero_ent`).
pub(crate) struct HeroData {
    /// Stats courantes (life = PV restants).
    pub stats: HeroStats,
    /// Chemin de waypoints (centres de tuiles), vide = immobile.
    pub path: Vec<(f32, f32)>,
    /// Compteur du swing en cours (= cooldown).
    pub swing_ms: u32,
    /// > 0 : héros mort, compte jusqu'à réapparition.
    pub dead_ms: u32,
    /// Horloge d'animation (reset à chaque changement d'état).
    pub anim_ms: u32,
    /// Course active (touche Maj côté navigateur) : vitesse RN + anim RN.
    pub run: bool,
    /// Point d'apparition / respawn.
    pub spawn: (f32, f32),
    /// Inventaire 10×4, index rangée par rangée.
    pub inv: Vec<Option<ItemDef>>,
}

/// Données monstre (l'`Entity` est à côté, cf. `World.mon_ent`).
pub(crate) struct MonData {
    /// Index dans `World.mon_tpls`.
    pub tpl: usize,
    /// PV effectifs courants (scalés MonLvl).
    pub hp: u32,
    /// PV max effectifs.
    pub max_hp: u32,
    /// État FSM.
    pub state: MonState,
    /// Chemin courant.
    pub path: Vec<(f32, f32)>,
    /// Prochaine errance (ms restantes).
    pub wander_ms: u32,
    /// Prochain re-path de poursuite (ms restantes).
    pub repath_ms: u32,
    /// Cooldown d'attaque (ms restantes).
    pub atk_cd_ms: u32,
    /// Temps passé en DD.
    pub dying_ms: u32,
    /// Horloge d'animation.
    pub anim_ms: u32,
    /// Point d'origine (errance + laisse).
    pub home: (f32, f32),
    /// Dégâts effectifs min (scalés).
    pub dmg_min: u32,
    /// Dégâts effectifs max (scalés).
    pub dmg_max: u32,
    /// Toucher effectif.
    pub th: u32,
    /// AC effectif.
    pub ac: u32,
    /// XP effective à la mort.
    pub xp: u32,
    /// Vitesse marche tuiles/s.
    pub speed: f32,
}

/// Monde de jeu complet (pur : aucune IO, aucune dépendance de rendu).
pub struct World {
    pub(crate) grid: Vec<Vec<bool>>,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) manifest: serde_json::Value,
    /// Code héros effectif (ex. "BA") — persisté dans la sauvegarde.
    pub(crate) hero_code: String,
    pub(crate) hero_tpl: HeroTpl,
    pub(crate) mon_tpls: Vec<MonTpl>,
    pub(crate) hero_ent: Entity,
    pub(crate) hero: HeroData,
    /// Entités monstres — même ordre / même longueur que `mon_dat`.
    pub(crate) mon_ent: Vec<Entity>,
    /// Données monstres — parallèle à `mon_ent`.
    pub(crate) mon_dat: Vec<MonData>,
    pub(crate) items: Vec<ItemOnGround>,
    pub(crate) rng: Rng,
    pub(crate) now_ms: u64,
}

///Chance de toucher : `200×TH/(TH+AC)` bornée [5, 95] (GAME-DATA.md §3).
fn hit_chance(th: u32, ac: u32) -> u32 {
    let c = 200 * th / (th + ac.max(1));
    c.clamp(5, 95)
}

/// Distance euclidienne.
fn dist(dx: f32, dy: f32) -> f32 {
    (dx * dx + dy * dy).sqrt()
}

/// Orientation 8 dirs (y vers le bas) : 0=S, 1=SW, 2=W, 3=NW, 4=N, 5=NE, 6=E, 7=SE.
pub(crate) fn facing_from(dx: f32, dy: f32) -> u8 {
    if dx == 0.0 && dy == 0.0 {
        return 0;
    }
    let deg = dy.atan2(dx).to_degrees();
    let oct = (((deg - 90.0) / 45.0).round() as i32).rem_euclid(8);
    oct as u8
}

/// Change d'animation (reset horloge + frame si changement).
fn set_anim(ent: &mut Entity, s: AnimState, anim_ms: &mut u32) {
    if ent.anim != s {
        ent.anim = s;
        *anim_ms = 0;
        ent.anim_frame = 0;
    }
}

/// Avance l'horloge d'anim et met à jour la frame (boucle ou clamp sur DD/A1).
fn advance_anim(ent: &mut Entity, anim_ms: &mut u32, dt: u32, looping: bool) {
    *anim_ms = anim_ms.saturating_add(dt);
    let f = *anim_ms / FRAME_MS;
    ent.anim_frame = if looping {
        (f % FRAMES) as u8
    } else {
        f.min(FRAMES - 1) as u8
    };
}

/// Avance le long du chemin à `speed` tuiles/s. Renvoie true si le chemin est
/// épuisé (arrivé). Met à jour x/y/facing.
fn advance_along_path(ent: &mut Entity, path: &mut Vec<(f32, f32)>, speed: f32, dt_ms: u32) -> bool {
    let mut step = speed * dt_ms as f32 / 1000.0;
    while step > 0.0 {
        let Some(wp) = path.first().copied() else { break };
        let (dx, dy) = (wp.0 - ent.x, wp.1 - ent.y);
        let d = dist(dx, dy);
        if d <= step {
            ent.facing = facing_from(dx, dy);
            ent.x = wp.0;
            ent.y = wp.1;
            path.remove(0);
            step -= d;
        } else {
            ent.facing = facing_from(dx, dy);
            ent.x += dx / d * step;
            ent.y += dy / d * step;
            step = 0.0;
        }
    }
    path.is_empty()
}

impl World {
    /// Contrat : construit le monde depuis la grille de collision (d2game) et le
    /// manifest. Panique si la grille est vide/rattachée de façon incohérente.
    ///
    /// Auto-spawn : héros + un monstre par entrée `monsters` du manifest, sur des
    /// tuiles marchables espacées (BFS depuis le centre). `spawn_monster` /
    /// `clear_monsters` permettent un placement fin.
    pub fn new(level_collision: Vec<Vec<bool>>, manifest: serde_json::Value) -> World {
        Self::with_seed_and_hero(level_collision, manifest, 0xD2D2_D2D2_D2D2_D2D2, "BA")
    }

    /// Comme [`World::new`] mais avec le héros `hero` (code "BA"|"SOR"|"PA"…,
    /// insensible à la casse) : stats lues dans `manifest.heroes[code]` si
    /// présent, sinon retombée sur le bloc `hero` (rétro-compat).
    pub fn with_hero(level_collision: Vec<Vec<bool>>, manifest: serde_json::Value, hero: &str) -> World {
        Self::with_seed_and_hero(level_collision, manifest, 0xD2D2_D2D2_D2D2_D2D2, hero)
    }

    /// Comme [`World::new`] avec une graine RNG explicite (tests, replay).
    pub fn with_seed(level_collision: Vec<Vec<bool>>, manifest: serde_json::Value, seed: u64) -> World {
        Self::with_seed_and_hero(level_collision, manifest, seed, "BA")
    }

    /// Comme [`World::with_hero`] avec une graine RNG explicite.
    pub fn with_seed_and_hero(
        level_collision: Vec<Vec<bool>>,
        manifest: serde_json::Value,
        seed: u64,
        hero: &str,
    ) -> World {
        assert!(!level_collision.is_empty(), "d2rules: grille de collision vide");
        let w = level_collision[0].len() as i32;
        let h = level_collision.len() as i32;
        assert!(w > 0, "d2rules: grille de largeur nulle");
        for (i, row) in level_collision.iter().enumerate() {
            assert_eq!(row.len() as i32, w, "d2rules: ligne {i} de largeur {} != {w}", row.len());
        }
        let hero_code = data::resolve_hero(&manifest, hero);
        let world = World {
            w,
            h,
            grid: level_collision,
            hero_code: hero_code.clone(),
            manifest: manifest.clone(),
            hero_tpl: data::parse_hero_code(&manifest, &hero_code),
            mon_tpls: data::parse_monsters(&manifest),
            hero_ent: Entity {
                x: 0.0,
                y: 0.0,
                facing: 0,
                anim: AnimState::NU,
                anim_frame: 0,
            },
            hero: HeroData {
                stats: HeroStats {
                    level: 1,
                    xp: 0,
                    xp_to_next: XP_TABLE[0],
                    life: 0,
                    max_life: 0,
                    mana: 0,
                    max_mana: 0,
                    strength: 0,
                    dexterity: 0,
                    defense: 0,
                    to_hit: 0,
                    gold: 0,
                    walk_speed: 0.0,
                    potion_heal: POTION_HEAL,
                },
                path: Vec::new(),
                swing_ms: 0,
                dead_ms: 0,
                anim_ms: 0,
                run: false,
                spawn: (0.0, 0.0),
                inv: vec![None; INV_SLOTS],
            },
            mon_ent: Vec::new(),
            mon_dat: Vec::new(),
            items: Vec::new(),
            rng: Rng::new(seed),
            now_ms: 0,
        };
        let mut world = world;
        world.init_hero();
        world.auto_spawn();
        world
    }

    /// Remplit les stats héros depuis le gabarit (appelé à la construction).
    fn init_hero(&mut self) {
        let t = &self.hero_tpl;
        let defense = t.dexterity / 4 + (t.shield_ac_min + t.shield_ac_max) / 2;
        let spawns = self.pick_auto_spawns(0);
        let (sx, sy) = spawns[0];
        self.hero_ent.x = sx;
        self.hero_ent.y = sy;
        self.hero_ent.facing = 0;
        self.hero.stats = HeroStats {
            level: 1,
            xp: 0,
            xp_to_next: XP_TABLE[0],
            life: t.life,
            max_life: t.life,
            mana: t.mana,
            max_mana: t.mana,
            strength: t.strength,
            dexterity: t.dexterity,
            defense,
            to_hit: t.to_hit_factor + t.dexterity,
            gold: 0,
            walk_speed: t.walk_speed,
            potion_heal: POTION_HEAL,
        };
        self.hero.spawn = (sx, sy);
    }

    /// Cherche n+1 tuiles marchables espacées (BFS depuis le centre) ;
    /// [0] = héros, [1..] = monstres.
    fn pick_auto_spawns(&self, n_monsters: usize) -> Vec<(f32, f32)> {
        let want = n_monsters + 1;
        let start = {
            let g = Grid { w: self.w, h: self.h, blocked: &self.grid };
            snap_near(&g, (self.w / 2, self.h / 2), self.w.max(self.h)).unwrap_or((0, 0))
        };
        // BFS en croix depuis le centre → ordre croissant de distance
        let mut order = Vec::new();
        let mut seen = vec![false; (self.w * self.h) as usize];
        let mut queue = std::collections::VecDeque::new();
        let si = start.1 as usize * self.w as usize + start.0 as usize;
        seen[si] = true;
        queue.push_back(start);
        while let Some((x, y)) = queue.pop_front() {
            order.push((x, y));
            for (dx, dy) in crate::path::DIRS {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && nx < self.w && ny < self.h {
                    let ni = ny as usize * self.w as usize + nx as usize;
                    if !seen[ni] && !self.grid[ny as usize][nx as usize] {
                        seen[ni] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
        let mut chosen = vec![start];
        'outer: for spacing in [6, 4, 2, 1] {
            chosen = vec![start];
            for &(x, y) in order.iter().skip(1) {
                if chosen.iter().all(|(cx, cy)| (cx - x).abs().max((cy - y).abs()) >= spacing) {
                    chosen.push((x, y));
                    if chosen.len() >= want {
                        break 'outer;
                    }
                }
            }
        }
        chosen.truncate(want);
        chosen.iter().map(|&(x, y)| (x as f32 + 0.5, y as f32 + 0.5)).collect()
    }

    /// Spawn automatique : un monstre par entrée du manifest.
    fn auto_spawn(&mut self) {
        let n = self.mon_tpls.len();
        if n == 0 {
            return;
        }
        let spawns = self.pick_auto_spawns(n);
        for i in 0..n {
            if i + 1 >= spawns.len() {
                break; // carte trop petite pour espacer tous les monstres
            }
            let (x, y) = spawns[i + 1];
            self.spawn_monster_at(i, x, y);
        }
    }

    /// Spawn un monstre du gabarit `tpl_index` à une position continue donnée.
    fn spawn_monster_at(&mut self, tpl_index: usize, x: f32, y: f32) {
        let t = &self.mon_tpls[tpl_index];
        // HP effectifs : rand(minHP..maxHP) × MonLvl.HP/100, min 1
        let raw = self.rng.range_u32(t.hp_min, t.hp_max);
        let hp = ((raw * MONLVL_HP_NUM) / MONLVL_HP_DEN).max(1);
        let ent = Entity { x, y, facing: 0, anim: AnimState::NU, anim_frame: 0 };
        let dat = MonData {
            tpl: tpl_index,
            hp,
            max_hp: hp,
            state: MonState::Idle,
            path: Vec::new(),
            wander_ms: self.rng.range_u32(1000, 3000),
            repath_ms: 0,
            atk_cd_ms: 0,
            dying_ms: 0,
            anim_ms: 0,
            home: (x, y),
            // dégâts effectifs scalés à la frappe (stockés bruts, scalés ici en min/max)
            dmg_min: ((t.dmg_min * MONLVL_DM_NUM) / MONLVL_DM_DEN).max(1),
            dmg_max: ((t.dmg_max * MONLVL_DM_NUM) / MONLVL_DM_DEN).max(1),
            th: MONLVL_TH,
            ac: MONLVL_AC,
            xp: t.exp * MONLVL_XP_NUM / MONLVL_XP_DEN,
            speed: t.speed,
        };
        self.mon_ent.push(ent);
        self.mon_dat.push(dat);
    }

    // ------------------------------------------------------------------
    // API contrat
    // ------------------------------------------------------------------

    /// Un pas de simulation. `dt_ms` est tronqué à 100 ms. Renvoie les événements
    /// du tick dans l'ordre d'émission.
    pub fn tick(&mut self, dt_ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent> {
        let mut ev = Vec::new();
        let dt = dt_ms.min(MAX_DT_MS);
        self.now_ms += dt as u64;
        if let Some(input) = input {
            if self.hero.dead_ms == 0 {
                self.handle_input(input, &mut ev);
            }
        }
        self.update_hero(dt, &mut ev);
        self.update_monsters(dt, &mut ev);
        ev
    }

    /// Entité héros (lecture).
    pub fn hero(&self) -> &Entity {
        &self.hero_ent
    }

    /// Entités monstres (lecture, même ordre que les événements `index`).
    pub fn monsters(&self) -> &[Entity] {
        &self.mon_ent
    }

    /// Monstre `index` vivant et ciblable ? Extra additif pour d2game : faux
    /// si hors bornes ou en anim DD (mourant) — les monstres retirés le sont
    /// après l'anim DD, donc l'indice peut être périmé (caveat REPORT.md §4).
    pub fn monster_alive(&self, index: usize) -> bool {
        self.mon_ent.get(index).map(|m| m.anim != AnimState::DD).unwrap_or(false)
    }

    /// Objets au sol (lecture).
    pub fn items(&self) -> &[ItemOnGround] {
        &self.items
    }

    /// Stats héros (lecture).
    pub fn hero_stats(&self) -> &HeroStats {
        &self.hero.stats
    }

    /// Inventaire (lecture) : 40 slots 10×4.
    pub fn inventory(&self) -> &[Option<ItemDef>] {
        &self.hero.inv
    }

    /// Sauvegarde JSON (positions, hp, inventaire, xp, timers, grille, manifest).
    pub fn save(&self) -> String {
        crate::save::save_json(self)
    }

    /// Recharge un monde depuis une sauvegarde [`World::save`].
    /// `Err(())` sans détail : imposé par le contrat d2game (phase 3 : erreur typée).
    #[allow(clippy::result_unit_err)]
    pub fn load(json: &str) -> Result<World, ()> {
        crate::save::load_json(json)
    }

    // ------------------------------------------------------------------
    // API additive (placement fin, introspection — documentée REPORT.md)
    // ------------------------------------------------------------------

    /// Spawn un monstre du code manifest (`"FA"`, `"ZM"`, `"SI"`) sur une tuile.
    /// Renvoie false si code inconnu ou tuile bloquée/hors bornes.
    pub fn spawn_monster(&mut self, code: &str, x: i32, y: i32) -> bool {
        let Some(i) = self.mon_tpls.iter().position(|t| t.code == code) else {
            return false;
        };
        if !self.is_walkable(x, y) {
            return false;
        }
        self.spawn_monster_at(i, x as f32 + 0.5, y as f32 + 0.5);
        true
    }

    /// Retire tous les monstres (vivants ou mourants) — tests / placement fin.
    pub fn clear_monsters(&mut self) {
        self.mon_ent.clear();
        self.mon_dat.clear();
    }

    /// Déplace le point d'apparition du héros (tuile marchable requise).
    pub fn set_hero_spawn(&mut self, x: i32, y: i32) -> bool {
        if !self.is_walkable(x, y) {
            return false;
        }
        self.hero.spawn = (x as f32 + 0.5, y as f32 + 0.5);
        true
    }

    /// Tuile marchable ?
    pub fn is_walkable(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < self.w && y < self.h && !self.grid[y as usize][x as usize]
    }

    /// Largeur en tuiles.
    pub fn width(&self) -> i32 {
        self.w
    }

    /// Hauteur en tuiles.
    pub fn height(&self) -> i32 {
        self.h
    }

    /// Temps de jeu écoulé (ms).
    pub fn elapsed_ms(&self) -> u64 {
        self.now_ms
    }

    /// Point d'apparition du héros.
    pub fn hero_spawn(&self) -> (f32, f32) {
        self.hero.spawn
    }

    /// Code du héros effectif (ex. `"BA"` — résolu via `heroes[code]`/`hero`).
    pub fn hero_code(&self) -> &str {
        &self.hero_code
    }

    /// Le héros a-t-il un déplacement en cours ?
    pub fn hero_is_moving(&self) -> bool {
        !self.hero.path.is_empty()
    }

    /// Le héros court-il (touche Maj, anim RN, vitesse course) ?
    pub fn hero_is_running(&self) -> bool {
        self.hero.run
    }

    // ------------------------------------------------------------------
    // Entrées
    // ------------------------------------------------------------------

    fn handle_input(&mut self, input: PlayerInput, ev: &mut Vec<GameEvent>) {
        match input {
            PlayerInput::MoveTo { x, y } => {
                if let Some(path) = self.find_path_from_hero((x, y)) {
                    if !path.is_empty() {
                        self.hero.path = path;
                    }
                }
            }
            PlayerInput::Attack { x, y } => {
                let click = (x as f32 + 0.5, y as f32 + 0.5);
                let mut best: Option<usize> = None;
                let mut bd = PICK_RADIUS;
                for i in 0..self.mon_ent.len() {
                    if self.mon_dat[i].state == MonState::Dying {
                        continue;
                    }
                    let e = &self.mon_ent[i];
                    let d = dist(e.x - click.0, e.y - click.1);
                    if d <= bd {
                        bd = d;
                        best = Some(i);
                    }
                }
                if let Some(i) = best {
                    let d = dist(self.mon_ent[i].x - self.hero_ent.x, self.mon_ent[i].y - self.hero_ent.y);
                    if d <= ATTACK_RANGE {
                        if self.hero.swing_ms == 0 {
                            self.hero_attack(i, ev);
                        }
                    } else {
                        // hors de portée : le héros marche vers le monstre
                        let mt = (
                            self.mon_ent[i].x.floor() as i32,
                            self.mon_ent[i].y.floor() as i32,
                        );
                        if let Some(path) = self.find_path_from_hero(mt) {
                            if !path.is_empty() {
                                self.hero.path = path;
                            }
                        }
                    }
                }
            }
            PlayerInput::Pickup => {
                let (hx, hy) = (self.hero_ent.x, self.hero_ent.y);
                let mut best: Option<usize> = None;
                let mut bd = PICKUP_RANGE;
                for (i, it) in self.items.iter().enumerate() {
                    let d = dist(it.x - hx, it.y - hy);
                    if d <= bd {
                        bd = d;
                        best = Some(i);
                    }
                }
                if let Some(i) = best {
                    let item = self.items.remove(i);
                    if item.def.kind == ItemKind::Gold {
                        self.hero.stats.gold += item.def.amount;
                        ev.push(GameEvent::GoldGained { amount: item.def.amount });
                    } else if let Some(slot) = self.hero.inv.iter().position(|s| s.is_none()) {
                        self.hero.inv[slot] = Some(item.def);
                        ev.push(GameEvent::HeroPickedUp { slot });
                    } else {
                        ev.push(GameEvent::InventoryFull);
                        self.items.push(item); // reste au sol
                    }
                }
            }
            PlayerInput::SetRun(on) => {
                self.hero.run = on;
            }
            PlayerInput::UseItem(slot) => {
                if slot >= INV_SLOTS {
                    return;
                }
                let Some(def) = self.hero.inv[slot].clone() else { return };
                if def.kind == ItemKind::Potion {
                    let heal = POTION_HEAL.min(self.hero.stats.max_life - self.hero.stats.life);
                    self.hero.stats.life += heal;
                    self.hero.inv[slot] = None;
                    ev.push(GameEvent::PotionUsed { heal });
                }
            }
        }
    }

    /// A* depuis la tuile du héros vers `to` (snap tuile marchable ≤ 4 si bloquée).
    fn find_path_from_hero(&self, to: (i32, i32)) -> Option<Vec<(f32, f32)>> {
        let start = (self.hero_ent.x.floor() as i32, self.hero_ent.y.floor() as i32);
        self.path_between(start, to)
    }

    /// A* générique en centres de tuiles.
    fn path_between(&self, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(f32, f32)>> {
        let g = Grid { w: self.w, h: self.h, blocked: &self.grid };
        let goal = snap_near(&g, to, 4)?;
        let tiles = astar(&g, from, goal)?;
        Some(tiles.into_iter().map(|(x, y)| (x as f32 + 0.5, y as f32 + 0.5)).collect())
    }

    // ------------------------------------------------------------------
    // Mise à jour héros
    // ------------------------------------------------------------------

    fn update_hero(&mut self, dt: u32, ev: &mut Vec<GameEvent>) {
        if self.hero.dead_ms > 0 {
            self.hero.dead_ms += dt;
            set_anim(&mut self.hero_ent, AnimState::DD, &mut self.hero.anim_ms);
            advance_anim(&mut self.hero_ent, &mut self.hero.anim_ms, dt, false);
            if self.hero.dead_ms >= RESPAWN_MS {
                self.respawn_hero(ev);
            }
            return;
        }

        // swing = cooldown
        if self.hero.swing_ms > 0 {
            self.hero.swing_ms = self.hero.swing_ms.saturating_sub(dt);
        }

        // déplacement (course = RunVelocity du manifest, sinon marche)
        let mut arrived = false;
        if !self.hero.path.is_empty() {
            let speed = if self.hero.run {
                self.hero_tpl.run_speed.max(self.hero.stats.walk_speed)
            } else {
                self.hero.stats.walk_speed
            };
            arrived = advance_along_path(&mut self.hero_ent, &mut self.hero.path, speed, dt);
        }

        // sélection anim
        let st = if !self.hero.path.is_empty() {
            if self.hero.run {
                AnimState::RN
            } else {
                AnimState::WL
            }
        } else if self.hero.swing_ms > 0 {
            AnimState::A1
        } else {
            AnimState::NU
        };
        set_anim(&mut self.hero_ent, st, &mut self.hero.anim_ms);
        advance_anim(
            &mut self.hero_ent,
            &mut self.hero.anim_ms,
            dt,
            matches!(st, AnimState::NU | AnimState::WL | AnimState::RN),
        );

        if arrived {
            let end = (self.hero_ent.x.floor() as i32, self.hero_ent.y.floor() as i32);
            ev.push(GameEvent::HeroMoved { path_end: end });
        }
    }

    fn respawn_hero(&mut self, ev: &mut Vec<GameEvent>) {
        let (sx, sy) = self.hero.spawn;
        self.hero_ent.x = sx;
        self.hero_ent.y = sy;
        self.hero_ent.facing = 0;
        self.hero.stats.life = self.hero.stats.max_life;
        self.hero.stats.mana = self.hero.stats.max_mana;
        self.hero.dead_ms = 0;
        self.hero.swing_ms = 0;
        self.hero.path.clear();
        self.hero.anim_ms = 0;
        self.hero_ent.anim = AnimState::NU;
        self.hero_ent.anim_frame = 0;
        // tout le monde perd l'aggro
        for dat in self.mon_dat.iter_mut() {
            if dat.state != MonState::Dying {
                dat.state = MonState::Idle;
                dat.path.clear();
                dat.atk_cd_ms = MONSTER_ATK_GRACE_MS;
            }
        }
        ev.push(GameEvent::HeroRespawned { x: sx.floor() as i32, y: sy.floor() as i32 });
    }

    /// Frappe héros → monstre `i` (déjà à portée, cooldown prêt).
    fn hero_attack(&mut self, i: usize, ev: &mut Vec<GameEvent>) {
        let (dx, dy) = (self.mon_ent[i].x - self.hero_ent.x, self.mon_ent[i].y - self.hero_ent.y);
        self.hero_ent.facing = facing_from(dx, dy);
        self.hero.swing_ms = HERO_SWING_MS;
        self.hero.anim_ms = 0;
        let ac = self.mon_dat[i].ac;
        let chance = hit_chance(self.hero.stats.to_hit, ac);
        if (self.rng.below(100)) < chance {
            // dégâts héros = floor(rand(mindam..maxdam) × (1 + force/100))
            let base = self.rng.range_u32(self.hero_tpl.weapon_min, self.hero_tpl.weapon_max) as f32;
            let dmg = (base * (1.0 + self.hero.stats.strength as f32 / 100.0)).floor() as u32;
            self.mon_dat[i].hp = self.mon_dat[i].hp.saturating_sub(dmg);
            ev.push(GameEvent::HeroAttacked { target: i, dmg });
            if self.mon_dat[i].hp == 0 {
                let ent = &mut self.mon_ent[i];
                let dat = &mut self.mon_dat[i];
                dat.state = MonState::Dying;
                dat.dying_ms = 0;
                dat.anim_ms = 0;
                dat.path.clear();
                ent.anim = AnimState::DD;
                ent.anim_frame = 0;
            }
        } else {
            ev.push(GameEvent::HeroMissed { target: i });
        }
    }

    /// Applique l'XP gagnée + paliers de niveau (événement par palier).
    fn apply_xp(&mut self, mut xp: u32, ev: &mut Vec<GameEvent>) {
        self.hero.stats.xp += xp;
        xp = self.hero.stats.xp;
        loop {
            let lvl = self.hero.stats.level;
            if lvl >= MAX_LEVEL {
                self.hero.stats.xp_to_next = 0;
                break;
            }
            let need = XP_TABLE[(lvl - 1) as usize];
            if xp >= need {
                self.hero.stats.level = lvl + 1;
                self.hero.stats.max_life += self.hero_tpl.life_per_level;
                self.hero.stats.life += self.hero_tpl.life_per_level;
                self.hero.stats.max_mana += self.hero_tpl.mana_per_level;
                self.hero.stats.mana += self.hero_tpl.mana_per_level;
                ev.push(GameEvent::HeroLeveled { level: self.hero.stats.level });
            } else {
                self.hero.stats.xp_to_next = need - xp;
                break;
            }
        }
    }

    // ------------------------------------------------------------------
    // Mise à jour monstres
    // ------------------------------------------------------------------

    fn update_monsters(&mut self, dt: u32, ev: &mut Vec<GameEvent>) {
        let hero_pos = (self.hero_ent.x, self.hero_ent.y);
        let mut xp_gain: u32 = 0;

        let mut i = 0;
        while i < self.mon_ent.len() {
            // --- branche DD : anim puis disparition + drop + XP ---
            if self.mon_dat[i].state == MonState::Dying {
                self.mon_dat[i].dying_ms += dt;
                advance_anim(&mut self.mon_ent[i], &mut self.mon_dat[i].anim_ms, dt, false);
                if self.mon_dat[i].dying_ms >= DYING_MS {
                    let xp = self.mon_dat[i].xp;
                    let mlvl = self.mon_tpls[self.mon_dat[i].tpl].level;
                    let (x, y) = (self.mon_ent[i].x, self.mon_ent[i].y);
                    self.roll_drop(x, y, mlvl, ev);
                    xp_gain += xp;
                    ev.push(GameEvent::MonsterDied { index: i, xp });
                    self.mon_dat.remove(i);
                    self.mon_ent.remove(i);
                    continue; // pas de i += 1 : l'élément suivant prend l'index i
                }
                i += 1;
                continue;
            }

            let d = dist(self.mon_ent[i].x - hero_pos.0, self.mon_ent[i].y - hero_pos.1);
            let hero_dead = self.hero.dead_ms > 0;

            match self.mon_dat[i].state {
                MonState::Idle => {
                    if !hero_dead && d <= AGGRO_RADIUS {
                        self.mon_dat[i].state = MonState::Chase;
                        self.mon_dat[i].repath_ms = 0;
                        self.mon_dat[i].path.clear();
                    } else if self.mon_dat[i].path.is_empty() {
                        self.mon_dat[i].wander_ms = self.mon_dat[i].wander_ms.saturating_sub(dt);
                        if self.mon_dat[i].wander_ms == 0 {
                            let ht = (self.mon_ent[i].x.floor() as i32, self.mon_ent[i].y.floor() as i32);
                            let home = self.mon_dat[i].home;
                            let target = self.wander_target(&home);
                            if let Some(p) = self.path_between(ht, target) {
                                if !p.is_empty() {
                                    self.mon_dat[i].path = p;
                                }
                            }
                            self.mon_dat[i].wander_ms = self.rng.range_u32(1500, 4500);
                        }
                    }
                }
                MonState::Chase => {
                    if hero_dead || d > LEASH_RADIUS {
                        // héros mort ou laisse dépassée : retour au point d'origine
                        self.mon_to_idle(i);
                    } else if d <= ATTACK_RANGE {
                        self.mon_dat[i].state = MonState::Attack;
                        self.mon_dat[i].atk_cd_ms = MONSTER_ATK_GRACE_MS;
                        self.mon_dat[i].path.clear();
                    } else {
                        self.mon_dat[i].repath_ms = self.mon_dat[i].repath_ms.saturating_sub(dt);
                        if self.mon_dat[i].path.is_empty() || self.mon_dat[i].repath_ms == 0 {
                            let mt = (
                                self.mon_ent[i].x.floor() as i32,
                                self.mon_ent[i].y.floor() as i32,
                            );
                            let ht = (hero_pos.0.floor() as i32, hero_pos.1.floor() as i32);
                            if let Some(p) = self.path_between(mt, ht) {
                                self.mon_dat[i].path = p;
                            }
                            self.mon_dat[i].repath_ms = REPATH_MS;
                        }
                    }
                }
                MonState::Attack => {
                    if hero_dead {
                        self.mon_to_idle(i);
                    } else if d > ATTACK_RANGE + 0.2 {
                        self.mon_dat[i].state = MonState::Chase;
                        self.mon_dat[i].repath_ms = 0;
                        self.mon_dat[i].path.clear();
                    } else {
                        let (dx, dy) = (hero_pos.0 - self.mon_ent[i].x, hero_pos.1 - self.mon_ent[i].y);
                        self.mon_ent[i].facing = facing_from(dx, dy);
                        self.mon_dat[i].atk_cd_ms = self.mon_dat[i].atk_cd_ms.saturating_sub(dt);
                        if self.mon_dat[i].atk_cd_ms == 0 {
                            self.monster_strike(i, ev);
                            self.mon_dat[i].atk_cd_ms = MONSTER_ATK_CD_MS;
                            self.mon_dat[i].anim_ms = 0; // rejoue le swing A1
                        }
                    }
                }
                MonState::Dying => unreachable!(), // traité plus haut
            }

            // déplacement + anim (hors Attack, immobile)
            if self.mon_dat[i].state != MonState::Attack && self.mon_dat[i].state != MonState::Dying {
                if !self.mon_dat[i].path.is_empty() {
                    let speed = self.mon_dat[i].speed;
                    advance_along_path(&mut self.mon_ent[i], &mut self.mon_dat[i].path, speed, dt);
                }
                let st = if !self.mon_dat[i].path.is_empty() { AnimState::WL } else { AnimState::NU };
                set_anim(&mut self.mon_ent[i], st, &mut self.mon_dat[i].anim_ms);
                advance_anim(&mut self.mon_ent[i], &mut self.mon_dat[i].anim_ms, dt, true);
            } else if self.mon_dat[i].state == MonState::Attack {
                set_anim(&mut self.mon_ent[i], AnimState::A1, &mut self.mon_dat[i].anim_ms);
                advance_anim(&mut self.mon_ent[i], &mut self.mon_dat[i].anim_ms, dt, false);
            }

            i += 1;
        }

        if xp_gain > 0 {
            self.apply_xp(xp_gain, ev);
        }
    }

    /// Retour à l'état neutre (mort du héros ou laisse dépassée).
    fn mon_to_idle(&mut self, i: usize) {
        self.mon_dat[i].state = MonState::Idle;
        self.mon_dat[i].path.clear();
        self.mon_dat[i].wander_ms = self.rng.range_u32(1000, 3000);
    }

    /// Tuile d'errance : home ± WANDER_RADIUS, marchable de préférence.
    fn wander_target(&mut self, home: &(f32, f32)) -> (i32, i32) {
        let hx = home.0.floor() as i32;
        let hy = home.1.floor() as i32;
        for _ in 0..8 {
            let x = hx + self.rng.range_i32(-WANDER_RADIUS, WANDER_RADIUS);
            let y = hy + self.rng.range_i32(-WANDER_RADIUS, WANDER_RADIUS);
            if self.is_walkable(x, y) {
                return (x, y);
            }
        }
        (hx, hy)
    }

    /// Frappe monstre → héros (jet de toucher vs défense, dégâts scalés MonLvl).
    fn monster_strike(&mut self, i: usize, ev: &mut Vec<GameEvent>) {
        let (dmin, dmax, th) = {
            let d = &self.mon_dat[i];
            (d.dmg_min, d.dmg_max, d.th)
        };
        // défense du héros pour ce coup : dex/4 + rand(minac..maxac bouclier)
        let def = self.hero.stats.dexterity / 4
            + self.rng.range_u32(self.hero_tpl.shield_ac_min, self.hero_tpl.shield_ac_max);
        let chance = hit_chance(th, def);
        if (self.rng.below(100)) < chance {
            let dmg = self.rng.range_u32(dmin, dmax).max(1);
            self.hero.stats.life = self.hero.stats.life.saturating_sub(dmg);
            ev.push(GameEvent::HeroHit { index: i, dmg });
            if self.hero.stats.life == 0 {
                self.hero.dead_ms = 1; // démarre le compte à rebours de respawn
                self.hero.path.clear();
                self.hero.swing_ms = 0;
                self.hero.anim_ms = 0;
                ev.push(GameEvent::HeroDied);
            }
        }
        // raté : pas d'événement (flux volontairement minimal)
    }

    /// Drop MVP : 60 % rien / 25 % or (~niveau×10, [×5..×15]) / 15 % potion hp1.
    fn roll_drop(&mut self, x: f32, y: f32, monster_level: u32, ev: &mut Vec<GameEvent>) {
        let r = self.rng.below(100);
        if r < DROP_NOTHING_PCT {
            return;
        }
        let def = if r < DROP_NOTHING_PCT + DROP_GOLD_PCT {
            let lvl = monster_level.max(1);
            let amount = self.rng.range_u32(lvl * 5, lvl * 15);
            ItemDef::gold(amount)
        } else {
            ItemDef::potion()
        };
        ev.push(GameEvent::ItemDropped {
            item: def.clone(),
            x: x.floor() as i32,
            y: y.floor() as i32,
        });
        self.items.push(ItemOnGround { def, x, y });
    }
}
