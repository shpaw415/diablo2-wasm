//! The game loop core — native, no WASM types, fully testable.
//!
//! Flow: JS fetches the manifest, then every asset it lists, feeds them
//! with [`GameCore::provide`], and calls [`GameCore::finalize`], which
//! composes the map (index framebuffer + collision grid), builds the
//! sprite sets, and constructs the contract `World`. Then it's
//! [`GameCore::tick`] (events out) and [`GameCore::render`] (RGBA
//! viewport, camera centered on the hero) once per frame.
//!
//! Draw order per frame: floor map → shadows → items/monsters/hero
//! sorted by screen y. Corpses (`anim == Dd`) are skipped (no DD frames
//! in the extracted assets yet).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::compose::{self, ComposedMap};
use crate::ds1;
use crate::dt1::{self, Dt1Tile};
use crate::pal::MapPalette;
use crate::sprite::{sprite_palette, SpriteFrame, SpriteSet};
use crate::world::{AnimState, Entity, GameEvent, PlayerInput, World};

pub const DEFAULT_VIEWPORT_W: u32 = 800;
pub const DEFAULT_VIEWPORT_H: u32 = 600;

/// Anti-rebond après une transition : le héros arrive à quelques tuiles de la
/// zone warp de retour — ce délai empêche un va-et-vient instantané.
const WARP_COOLDOWN_MS: u32 = 1500;
/// Distance minimale (sous-tuiles) héros↔monstre au spawn d'un niveau.
const MONSTER_SPAWN_CLEAR_OF_HERO: f32 = 15.0;
/// Distance minimale (sous-tuiles) entre deux monstres spawnés.
const MONSTER_SPAWN_SPACING: f32 = 12.0;
/// Distance minimale (sous-tuiles) monstre↔zone warp.
const MONSTER_SPAWN_CLEAR_OF_WARP: f32 = 10.0;

/// Définition d'un niveau lue du manifest (`levels.{key}`, alias `level` pour
/// le town historique).
#[derive(Debug, Clone)]
pub struct LevelDef {
    /// Clé manifest normalisée ("town" | "bloodmoor"…).
    pub key: String,
    /// Nom affichable (`nameDisplay` > `name` > clé).
    pub name: String,
    pub ds1: String,
    pub dt1: Vec<String>,
    pub palette: String,
    /// Codes monstres à spawn (`monsters` du bloc ; [] pour le town — zone
    /// sûre —, palette FA/ZM/SI par défaut pour un niveau extérieur).
    pub monsters: Vec<String>,
    /// Zone de transition vers un autre niveau (aucune si absente).
    pub warp: Option<WarpDef>,
}

/// Zone warp : entrer dans le cercle `at`±`r` (sous-tuiles) téléporte vers `to`
/// au point `entry` (snap marchable). Données manifest `warp` du niveau.
#[derive(Debug, Clone)]
pub struct WarpDef {
    pub to: String,
    pub at: (f32, f32),
    pub r: f32,
    pub entry: (f32, f32),
}

/// Info de changement de niveau (HUD : bannière + nom de zone).
#[derive(Debug, Clone, serde::Serialize)]
pub struct LevelSwitchInfo {
    pub key: String,
    pub name: String,
}

/// Événements du tick : ceux du monde (contrat d2rules) + les événements
/// propres au moteur multi-niveaux (transitions, échecs de transition).
#[derive(Debug, Clone)]
pub enum CoreEvent {
    World(GameEvent),
    LevelSwitch(LevelSwitchInfo),
    LevelSwitchFailed(String),
}

impl CoreEvent {
    /// Sérialisation JSON : les événements monde gardent EXACTEMENT la forme
    /// du contrat ({"HeroMoved":{…}}…) ; les extras du moteur s'ajoutent.
    pub fn to_json(&self) -> Value {
        match self {
            CoreEvent::World(e) => serde_json::to_value(e).unwrap_or(Value::Null),
            CoreEvent::LevelSwitch(i) => serde_json::json!({ "LevelSwitch": i }),
            CoreEvent::LevelSwitchFailed(m) => serde_json::json!({ "LevelSwitchFailed": m }),
        }
    }
}

pub struct GameCore {
    manifest: Value,
    assets: BTreeMap<String, Vec<u8>>,
    /// Niveau demandé au boot (clé manifest, "town" par défaut) — résolu en
    /// [`LevelDef`] au `finalize`.
    level_name: String,
    /// Niveau chargé (carte + warp + monstres) — `None` avant `finalize`.
    level: Option<LevelDef>,
    map: Option<ComposedMap>,
    map_rgba: [u8; 256 * 4], // palette-index → RGBA, precomputed for blits
    sprite_pal: d2dcc::render::Palette,
    sprites: BTreeMap<String, SpriteSet>,
    /// Modes d'animation listés au manifest mais aux fichiers absents/illisibles
    /// — sautés avec note (fallback gracieux), visibles via `sprite_notes_json`.
    sprite_notes: Vec<String>,
    /// Notes de transitions multi-niveaux (transferts héros dégradés…).
    switch_notes: Vec<String>,
    /// Code monstre par index de `monsters()` — parallèle au monde, maintenu
    /// aux spawns (d2game choisit les codes par niveau) et aux retraits
    /// (MonsterDied retire l'index, cf. contrat d2rules).
    mon_codes: Vec<String>,
    /// Délai anti-rebond restant après une transition (ms).
    warp_cd_ms: u32,
    world: Option<World>,
    /// Héros demandé ("BA"|"SOR"|"PA"…, normalisé uppercase) — résolu contre
    /// `manifest.heroes` (retombée `hero` à l'ancienne via d2rules).
    hero_request: String,
    vw: u32,
    vh: u32,
    cam_x: f32,
    cam_y: f32,
    fb: Vec<u8>,
    finalized: bool,
}

impl GameCore {
    /// `manifest_json` — le `assets/manifest.json` brut ; `hero` — le code de
    /// classe choisi (`"BA"`, `"SOR"`, `"PA"`…, insensible à la casse, "BA" par
    /// défaut). Le sprite et les stats du héros viennent TOUS DEUX de
    /// `manifest.heroes[code]` si présent, sinon du bloc `hero` (rétro-compat).
    /// Niveau initial : "town" (voir [`GameCore::with_level`]).
    pub fn new(manifest_json: &str, hero: &str) -> Result<GameCore, String> {
        Self::with_level(manifest_json, hero, "town")
    }

    /// Comme [`GameCore::new`] avec le niveau initial `level` (clé du bloc
    /// `levels` du manifest : `"town"`, `"bloodmoor"`…, insensible à la casse,
    /// "town" par défaut). Le bloc `level` historique reste l'alias de "town".
    pub fn with_level(manifest_json: &str, hero: &str, level: &str) -> Result<GameCore, String> {
        let manifest: Value =
            serde_json::from_str(manifest_json).map_err(|e| format!("manifest JSON: {e}"))?;
        let hero_request = {
            let t = hero.trim().to_ascii_uppercase();
            if t.is_empty() { "BA".to_string() } else { t }
        };
        let level_name = {
            let t = level.trim().to_ascii_lowercase();
            if t.is_empty() { "town".to_string() } else { t }
        };
        Ok(GameCore {
            manifest,
            assets: BTreeMap::new(),
            level_name,
            level: None,
            map: None,
            map_rgba: [0; 256 * 4],
            sprite_pal: sprite_palette(&[]),
            sprites: BTreeMap::new(),
            sprite_notes: Vec::new(),
            switch_notes: Vec::new(),
            mon_codes: Vec::new(),
            warp_cd_ms: 0,
            world: None,
            hero_request,
            vw: DEFAULT_VIEWPORT_W,
            vh: DEFAULT_VIEWPORT_H,
            cam_x: 0.0,
            cam_y: 0.0,
            fb: Vec::new(),
            finalized: false,
        })
    }

    /// Register one asset blob under its manifest path (case-insensitive;
    /// the lowercase basename also indexes it for `.tg1` → DT1 mapping).
    pub fn provide(&mut self, path: &str, bytes: Vec<u8>) {
        let lower = path.to_ascii_lowercase();
        let base = lower.rsplit('/').next().unwrap_or(&lower).to_string();
        self.assets.insert(lower, bytes.clone());
        self.assets.entry(base).or_insert(bytes);
    }

    fn asset(&self, path: &str) -> Result<&Vec<u8>, String> {
        let lower = path.to_ascii_lowercase();
        self.assets
            .get(&lower)
            .or_else(|| self.assets.get(lower.rsplit('/').next().unwrap_or("")))
            .ok_or_else(|| format!("missing asset: {path} (provide it before finalize)"))
    }

    /// Compose the map, build sprites, construct the world.
    pub fn finalize(&mut self) -> Result<(), String> {
        // --- sprites ------------------------------------------------------
        // Sets are keyed "<entity>" (marche/défaut, toujours présent) et
        // "<entity>:MODE" (NU/WL/RN/A1/DD) quand le manifest les fournit
        // (`heroes[code].modes` ou blocs nommés idle/attack/death). Un mode
        // listé mais aux fichiers absents est SAUTÉ avec note (fallback
        // gracieux) — jamais d'échec de finalize sur un mode optionnel.
        // Indépendant du niveau : construit une seule fois au boot.
        let hero_code = d2rules::resolve_hero_code(&self.manifest, &self.hero_request);
        let mut sprites = BTreeMap::new();
        let mut notes: Vec<String> = Vec::new();
        if let Some(hero_def) = hero_def_for(&self.manifest, &hero_code) {
            Self::build_entity_sets("hero", &hero_def, &mut sprites, &mut notes, self)?;
        } else {
            // Rétro-compat totale : bloc `hero` historique seul.
            let walk = &self.manifest["hero"]["walk"];
            let set = self.build_sprite_set(walk)?;
            sprites.insert("hero".into(), set);
        }
        if let Some(list) = self.manifest.get("monsters").and_then(|m| m.as_array()) {
            for m in list {
                let code = m.get("code").and_then(|c| c.as_str()).unwrap_or("??").to_string();
                Self::build_entity_sets(&code, m, &mut sprites, &mut notes, self)?;
            }
        }
        self.sprites = sprites;
        self.sprite_notes = notes;

        // --- niveau initial : carte + monde + monstres ----------------------
        let def = self.resolve_level_def(&self.level_name)?;
        self.load_level(&def, None)?; // None = auto-spawn du monde (centre, historique)

        let (hw, hh) = (self.vw as usize * 4, self.vh as usize);
        self.fb = vec![0; hw * hh];
        self.finalized = true;
        Ok(())
    }

    /// Résout la définition d'un niveau : `levels.{key}` ; retombée sur le bloc
    /// `level` historique pour "town" (avec ou sans bloc `levels`).
    fn resolve_level_def(&self, key: &str) -> Result<LevelDef, String> {
        let norm = key.trim().to_ascii_lowercase();
        let key = if norm.is_empty() { "town".to_string() } else { norm };
        let legacy = self.manifest.get("level").filter(|_| key == "town");
        if let Some(block) = self
            .manifest
            .pointer(&format!("/levels/{key}"))
            .or(legacy)
        {
            return LevelDef::from_manifest(&key, block);
        }
        if let Some(levels) = self.manifest.get("levels").and_then(|l| l.as_object()) {
            Err(format!(
                "manifest: niveau inconnu \"{key}\" (disponibles : {})",
                levels.keys().cloned().collect::<Vec<_>>().join(", ")
            ))
        } else {
            Err(format!("manifest: niveau \"{key}\" indisponible (pas de bloc levels)"))
        }
    }

    /// (Re)charge un niveau : palette, DS1+DT1 (mapping .tg1 par niveau),
    /// composition de la carte, monde (transfert du héros si un monde existe
    /// déjà), monstres du niveau, caméra. `spawn` = point d'entrée en
    /// sous-tuiles (snap marchable) ; `None` = auto-spawn du monde.
    fn load_level(&mut self, def: &LevelDef, spawn: Option<(f32, f32)>) -> Result<(), String> {
        // --- palette -----------------------------------------------------
        let raw_pal = self.asset(&def.palette)?.clone();
        self.map_rgba = build_map_rgba_lut(&raw_pal, 0.7);
        self.sprite_pal = sprite_palette(&raw_pal);

        // --- tilesets, résolution .tg1 → DT1 du niveau --------------------
        let ds1_bytes = self.asset(&def.ds1)?.clone();
        let ds1 = ds1::decode_ds1(&ds1_bytes).map_err(|e| format!("{}: {e:?}", def.ds1))?;
        let mut tilesets: Vec<Dt1Tile> = Vec::new();
        for tg1 in &ds1.files {
            let container = resolve_container(tg1, &def.dt1);
            let bytes = self.asset(&container)?.clone();
            let mut tiles = dt1::decode_dt1(&bytes).map_err(|e| format!("{container}: {e:?}"))?;
            tilesets.append(&mut tiles);
        }

        // --- map ----------------------------------------------------------
        let map = compose::compose(&ds1, &tilesets);
        self.cam_x = 0.0;
        self.cam_y = 0.0;

        // --- monde ----------------------------------------------------------
        // Polarity: compose yields `true` = WALKABLE, the d2rules contract
        // takes `true` = BLOCKED — invert per-cell (see world.rs doc).
        let blocked: Vec<Vec<bool>> = map
            .walk_rows()
            .into_iter()
            .map(|row| row.into_iter().map(|walkable| !walkable).collect())
            .collect();
        // Snap du point d'entrée sur une tuile marchable (le warp/handoff
        // donne des coordonnées approximatives au bord des zones).
        let spawn = match spawn {
            Some((x, y)) => {
                let sx = (x.max(0.0) as usize).min(map.grid_w.saturating_sub(1));
                let sy = (y.max(0.0) as usize).min(map.grid_h.saturating_sub(1));
                let (gx, gy) = compose::find_nearby_walkable(&map.walk, map.grid_w, map.grid_h, sx, sy, 10)
                    .ok_or_else(|| format!("niveau {}: spawn ({x},{y}) sans tuile marchable à proximité", def.key))?;
                Some((gx as f32 + 0.5, gy as f32 + 0.5))
            }
            None => None,
        };
        let world = self.build_level_world(blocked, spawn, def)?;
        self.level = Some(def.clone());
        self.map = Some(map);
        self.world = Some(world);
        self.spawn_level_monsters();
        self.follow_camera();
        Ok(())
    }

    /// Monde du niveau : transfert du héros existant (save JSON patché :
    /// grille neuve, position au point d'entrée, monstres/objets vidés —
    /// stats/inventaire/xp/rng conservés), sinon monde neuf.
    fn build_level_world(
        &mut self,
        blocked: Vec<Vec<bool>>,
        spawn: Option<(f32, f32)>,
        def: &LevelDef,
    ) -> Result<World, String> {
        let mut world = match self.world.take() {
            Some(old) => {
                let json = old.save();
                match patch_save_for_level(&json, &blocked, spawn) {
                    Ok(w) => w,
                    Err(e) => {
                        let msg = format!("niveau {}: transfert du héros impossible ({e}) — monde neuf", def.key);
                        self.switch_notes.push(msg);
                        World::with_hero(blocked, self.manifest.clone(), &self.hero_request)
                    }
                }
            }
            None => World::with_hero(blocked, self.manifest.clone(), &self.hero_request),
        };
        // Monde neuf (boot sans transfert) avec point d'entrée demandé :
        // le respawn y est posé (l'auto-spawn centre reste la position initiale).
        if let Some((x, y)) = spawn {
            world.set_hero_spawn(x.floor() as i32, y.floor() as i32);
        }
        Ok(world)
    }

    /// Change de niveau : recharge DS1/DT1/collision (bytes fournis via
    /// `provide`, y compris à chaud), transfère le héros (stats/inventaire/xp),
    /// spawn les monstres du niveau et pose le cooldown anti-rebond. Avant
    /// `finalize`, mémorise simplement le niveau initial demandé.
    pub fn switch_level(&mut self, name: &str, spawn_x: f32, spawn_y: f32) -> Result<LevelSwitchInfo, String> {
        let def = self.resolve_level_def(name)?;
        if !self.finalized {
            self.level_name = def.key.clone();
            return Ok(LevelSwitchInfo { key: def.key, name: def.name });
        }
        self.load_level(&def, Some((spawn_x, spawn_y)))?;
        self.warp_cd_ms = WARP_COOLDOWN_MS;
        Ok(LevelSwitchInfo { key: def.key, name: def.name })
    }

    /// Spawn les monstres du niveau chargé : town = zone sûre (aucun monstre),
    /// niveaux extérieurs = la liste `monsters` du bloc (FA/ZM/SI par défaut),
    /// sur tuiles marchables aléatoires (RNG déterministe par niveau), à
    /// distance du héros, des autres monstres et de la zone warp.
    fn spawn_level_monsters(&mut self) {
        let mut codes: Vec<String> = Vec::new();
        let mut spots: Vec<(f32, f32)> = Vec::new();
        let list = self
            .level
            .as_ref()
            .map(|l| l.monsters.clone())
            .unwrap_or_default();
        if !list.is_empty() {
            let world = self.world.as_mut().expect("world avant spawn");
            world.clear_monsters();
            let (w, h) = (world.width().max(1) as u32, world.height().max(1) as u32);
            let hero = *world.hero();
            let seed = fnv1a(self.level.as_ref().map(|l| l.key.as_str()).unwrap_or("?"));
            let mut rng = LevelRng::new(seed);
            let warp = self.level.as_ref().and_then(|l| l.warp.clone());
            for code in &list {
                let mut placed = false;
                // Relâchement progressif des contraintes (cartes denses).
                for relax in 0u32..4 {
                    let clear_hero = MONSTER_SPAWN_CLEAR_OF_HERO * (1.0 - 0.25 * relax as f32);
                    let spacing = MONSTER_SPAWN_SPACING * (1.0 - 0.25 * relax as f32);
                    let clear_warp = MONSTER_SPAWN_CLEAR_OF_WARP * (1.0 - 0.25 * relax as f32);
                    for _ in 0..200 {
                        let (tx, ty) = (rng.below(w) as i32, rng.below(h) as i32);
                        if !world.is_walkable(tx, ty) {
                            continue;
                        }
                        let (cx, cy) = (tx as f32 + 0.5, ty as f32 + 0.5);
                        if (cx - hero.x).hypot(cy - hero.y) < clear_hero {
                            continue;
                        }
                        if let Some(wp) = &warp {
                            if (cx - wp.at.0).hypot(cy - wp.at.1) < clear_warp {
                                continue;
                            }
                        }
                        if spots.iter().any(|(sx, sy)| (cx - sx).hypot(cy - sy) < spacing) {
                            continue;
                        }
                        if world.spawn_monster(code, tx, ty) {
                            codes.push(code.clone());
                            spots.push((cx, cy));
                            placed = true;
                            break;
                        }
                    }
                    if placed {
                        break;
                    }
                }
            }
        } else if let Some(world) = self.world.as_mut() {
            // Zone sûre (town) : aucun monstre, même si le monde neuf en a
            // auto-spawné (comportement par défaut de d2rules).
            world.clear_monsters();
        }
        self.mon_codes = codes;
    }

    fn build_sprite_set(&self, walk: &Value) -> Result<SpriteSet, String> {
        let cof_path = walk
            .get("cof")
            .and_then(|c| c.as_str())
            .ok_or("manifest: walk.cof missing")?;
        let cof = Cof::parse(self.asset(cof_path)?).map_err(|e| format!("{cof_path}: {e:?}"))?;
        let mut set = SpriteSet::new(cof);
        if let Some(layers) = walk.get("layers").and_then(|l| l.as_object()) {
            for (token, path) in layers {
                let path = path.as_str().ok_or("manifest: layer path not a string")?;
                let dcc = Dcc::decode(self.asset(path)?).map_err(|e| format!("{path}: {e:?}"))?;
                set.add_layer(token, dcc);
            }
        }
        Ok(set)
    }

    /// Construit tous les sets d'une entité (héros ou monstre) : `modes`
    /// {NU,WL,RN,A1,DD} si présent, sinon les blocs nommés
    /// walk/idle/attack/death, sinon walk seul. Le set « base » (sans suffixe)
    /// reste le repli universel = la marche. Les modes optionnels aux fichiers
    /// absents produisent une note, pas une erreur.
    fn build_entity_sets(
        base: &str,
        def: &Value,
        sprites: &mut BTreeMap<String, SpriteSet>,
        notes: &mut Vec<String>,
        me: &GameCore,
    ) -> Result<(), String> {
        // 1. Set de base : `modes.WL` > `walk` (jamais d'erreur si absent —
        //    l'entité ne sera simplement pas dessinée, comportement historique).
        let base_set: Option<SpriteSet> = if let Some(wl) = def
            .pointer("/modes/WL")
            .filter(|v| v.get("cof").is_some())
        {
            me.build_sprite_set(wl).ok()
        } else if let Some(walk) = def.get("walk").filter(|v| !v.is_null()) {
            match me.build_sprite_set(walk) {
                Ok(s) => Some(s),
                Err(e) => {
                    return Err(format!("{base}: {e}"));
                }
            }
        } else {
            None
        };
        if let Some(s) = base_set {
            sprites.insert(base.to_string(), s);
        }

        // 2. Modes additionnels. `modes` = {NU|WL|RN|A1|DD: {cof, layers}} ;
        //    à défaut blocs nommés idle→NU, attack→A1, death→DD.
        let mut mode_of: Vec<(&str, &Value)> = Vec::new();
        if let Some(modes) = def.get("modes").and_then(|m| m.as_object()) {
            for tag in ["NU", "WL", "RN", "A1", "DD"] {
                if let Some(v) = modes.get(tag).filter(|v| v.get("cof").is_some()) {
                    mode_of.push((tag, v));
                }
            }
        } else {
            for (field, tag) in [("idle", "NU"), ("attack", "A1"), ("death", "DD")] {
                if let Some(v) = def.get(field).filter(|v| v.get("cof").is_some()) {
                    mode_of.push((tag, v));
                }
            }
        }
        for (tag, spec) in mode_of {
            let key = format!("{base}:{tag}");
            if sprites.contains_key(&key) {
                continue;
            }
            match me.build_sprite_set(spec) {
                Ok(set) => {
                    sprites.insert(key, set);
                }
                Err(e) => {
                    notes.push(format!("{key}: ignoré ({e})"));
                }
            }
        }
        Ok(())
    }

    pub fn set_viewport(&mut self, w: u32, h: u32) {
        self.vw = w.max(16);
        self.vh = h.max(16);
        if self.finalized {
            self.fb = vec![0; self.vw as usize * self.vh as usize * 4];
        }
    }

    /// Nearest alive monster within `radius` sub-tiles of a world point
    /// (for the JS click router: attack vs move). Index into monsters().
    pub fn monster_near(&self, wx: f32, wy: f32, radius: f32) -> Option<usize> {
        let world = self.world.as_ref()?;
        let mut best: Option<(f32, usize)> = None;
        for (i, m) in world.monsters().iter().enumerate() {
            if !world.monster_alive(i) {
                continue;
            }
            let d = ((m.x - wx).powi(2) + (m.y - wy).powi(2)).sqrt();
            if d <= radius && best.map(|(bd, _)| d < bd).unwrap_or(true) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }

    /// Convert a canvas click (viewport px) to world sub-tile coords.
    pub fn screen_to_world(&self, px: f32, py: f32) -> (f32, f32) {
        let map = self.map.as_ref().expect("finalize first");
        map.screen_to_sub(px + self.cam_x, py + self.cam_y)
    }

    pub fn hero(&self) -> &Entity {
        self.world.as_ref().expect("finalize first").hero()
    }

    /// Hero position in viewport px (feet anchor), for the JS cursor UI.
    pub fn hero_screen(&self) -> (f32, f32) {
        let map = self.map.as_ref().expect("finalize first");
        let h = self.hero();
        let (sx, sy) = map.sub_to_screen(h.x, h.y);
        (sx - self.cam_x, sy - self.cam_y)
    }

    pub fn map_pixel_size(&self) -> (u32, u32) {
        let map = self.map.as_ref().expect("finalize first");
        (map.width as u32, map.height as u32)
    }

    /// The collision grid flattened as 0/1 bytes (for d2rules / debug UIs).
    pub fn collision(&self) -> Vec<u8> {
        let map = self.map.as_ref().expect("finalize first");
        map.walk.iter().map(|&w| w as u8).collect()
    }

    pub fn grid_size(&self) -> (u32, u32) {
        let map = self.map.as_ref().expect("finalize first");
        (map.grid_w as u32, map.grid_h as u32)
    }

    pub fn compose_stats(&self) -> ComposeStatsCopy {
        let map = self.map.as_ref().expect("finalize first");
        let s = map.stats;
        ComposeStatsCopy {
            drawn: s.drawn,
            hidden: s.hidden,
            empty: s.empty,
            out_of_bounds: s.out_of_bounds,
        }
    }

    pub fn is_finalized(&self) -> bool {
        self.finalized
    }

    /// Hero HUD stats as JSON (`HeroStats`), for the demo status/HUD.
    pub fn hero_stats_json(&self) -> String {
        match self.world.as_ref() {
            Some(w) => serde_json::to_string(w.hero_stats()).unwrap_or_else(|_| "{}".into()),
            None => "{}".into(),
        }
    }

    /// Classe du héros résolue, JSON `{"code":"BA","name":"Barbare"}` — pour le
    /// HUD (nom affiché) et l'E2E. Fonctionne avant comme après `finalize`.
    pub fn hero_class_json(&self) -> String {
        let code = d2rules::resolve_hero_code(&self.manifest, &self.hero_request);
        let name = self
            .manifest
            .pointer(&format!("/heroes/{code}/name"))
            .or_else(|| self.manifest.pointer("/hero/name"))
            .and_then(|n| n.as_str())
            .unwrap_or(&code);
        serde_json::json!({ "code": code, "name": name }).to_string()
    }

    /// État d'animation du héros, JSON `{"anim":"RN","frame":3,"facing":6,
    /// "run":true,"moving":true}` — HUD course + captures E2E.
    pub fn hero_state_json(&self) -> String {
        match self.world.as_ref() {
            Some(w) => {
                let h = w.hero();
                let anim = match h.anim {
                    AnimState::NU => "NU",
                    AnimState::WL => "WL",
                    AnimState::RN => "RN",
                    AnimState::A1 => "A1",
                    AnimState::DD => "DD",
                };
                serde_json::json!({
                    "anim": anim,
                    "frame": h.anim_frame,
                    "facing": h.facing,
                    "run": w.hero_is_running(),
                    "moving": w.hero_is_moving(),
                })
                .to_string()
            }
            None => "{}".into(),
        }
    }

    /// Modes d'animation sautés (fichiers absents au manifest) — visibilité
    /// du fallback gracieux côté page. JSON `["hero:DD: ignoré (…)", …]`.
    pub fn sprite_notes_json(&self) -> String {
        serde_json::to_string(&self.sprite_notes).unwrap_or_else(|_| "[]".into())
    }

    /// Niveau chargé — JSON `{"key":"bloodmoor","name":"Blood Moor (Acte I)"}`
    /// pour le HUD (nom de zone, bannière de changement). Résolu même avant
    /// `finalize` (niveau initial demandé) ; `{}` si la clé est inconnue.
    pub fn level_info_json(&self) -> String {
        let info = match self.level.as_ref() {
            Some(l) => Some(LevelSwitchInfo { key: l.key.clone(), name: l.name.clone() }),
            None => self
                .resolve_level_def(&self.level_name)
                .ok()
                .map(|d| LevelSwitchInfo { key: d.key, name: d.name }),
        };
        match info {
            Some(i) => serde_json::to_string(&i).unwrap_or_else(|_| "{}".into()),
            None => "{}".into(),
        }
    }

    /// Notes de transitions multi-niveaux (transferts héros dégradés…).
    /// JSON `["niveau bloodmoor : …", …]`.
    pub fn switch_notes_json(&self) -> String {
        serde_json::to_string(&self.switch_notes).unwrap_or_else(|_| "[]".into())
    }

    /// Codes monstres du niveau courant, parallèles à `monsters()` —
    /// debug/E2E. JSON `["FA","ZM",…]`.
    pub fn monster_codes_json(&self) -> String {
        serde_json::to_string(&self.mon_codes).unwrap_or_else(|_| "[]".into())
    }

    /// Inventory (40 slots, `null` = empty) as JSON.
    pub fn inventory_json(&self) -> String {
        match self.world.as_ref() {
            Some(w) => serde_json::to_string(w.inventory()).unwrap_or_else(|_| "[]".into()),
            None => "[]".into(),
        }
    }

    /// Serialize the world (d2rules save format, JSON v1) for the demo's
    /// Save button. Empty string when not finalized.
    pub fn save_world(&self) -> String {
        self.world.as_ref().map(|w| w.save()).unwrap_or_default()
    }

    /// Replace the world with one deserialized from `json` (d2rules save).
    /// The map/sprites stay as-is (same level, same assets).
    pub fn load_world(&mut self, json: &str) -> Result<(), String> {
        let world = World::load(json).map_err(|()| "sauvegarde invalide ou version inconnue".to_string())?;
        // Sanity: the save's grid must match this map's (same level).
        if world.width() as usize != self.grid_size().0 as usize
            || world.height() as usize != self.grid_size().1 as usize
        {
            return Err("sauvegarde d'une autre carte".into());
        }
        self.world = Some(world);
        self.follow_camera();
        Ok(())
    }

    /// Advance the world, follow the hero with the camera, return events.
    /// Détecte en plus l'entrée du héros dans la zone warp du niveau :
    /// recharge le niveau cible immédiatement (transfert du héros) et émet
    /// `LevelSwitch` (HUD/fondu côté page) — ou `LevelSwitchFailed`.
    pub fn tick(&mut self, dt_ms: u32, input: Option<PlayerInput>) -> Vec<CoreEvent> {
        let mut events: Vec<CoreEvent> = self
            .world
            .as_mut()
            .expect("finalize first")
            .tick(dt_ms, input)
            .into_iter()
            .map(CoreEvent::World)
            .collect();
        self.follow_camera();

        // d2rules retire les cadavres à la fin de l'anim DD (MonsterDied porte
        // l'index au moment du retrait) : miroir exact sur mon_codes.
        for e in &events {
            if let CoreEvent::World(GameEvent::MonsterDied { index, .. }) = e {
                let i = *index;
                if i < self.mon_codes.len() {
                    self.mon_codes.remove(i);
                }
            }
        }

        // --- zone warp ------------------------------------------------------
        if self.warp_cd_ms > 0 {
            self.warp_cd_ms = self.warp_cd_ms.saturating_sub(dt_ms);
        } else if let Some(warp) = self.level.as_ref().and_then(|l| l.warp.clone()) {
            let h = *self.world.as_ref().expect("finalize first").hero();
            if (h.x - warp.at.0).hypot(h.y - warp.at.1) <= warp.r {
                // Arrivée = point d'entrée du niveau DESTINATION (son propre
                // `warp.entry`), repli : l'entrée du niveau courant.
                let dest = self.resolve_level_def(&warp.to);
                let (spawn, key) = match dest {
                    Ok(d) => (d.warp.as_ref().map(|w| w.entry).unwrap_or(warp.entry), d.key),
                    Err(_) => (warp.entry, warp.to.clone()),
                };
                match self.switch_level(&key, spawn.0, spawn.1) {
                    Ok(info) => events.push(CoreEvent::LevelSwitch(info)),
                    Err(e) => {
                        // Réessai plus tard : la partie reste jouable tel quel.
                        self.warp_cd_ms = 3000;
                        events.push(CoreEvent::LevelSwitchFailed(e));
                    }
                }
            }
        }
        events
    }

    fn follow_camera(&mut self) {
        // Absolute hero screen position (hero_screen() is viewport-relative).
        let map = self.map.as_ref().unwrap();
        let h = *self.world.as_ref().unwrap().hero();
        let (sx, sy) = map.sub_to_screen(h.x, h.y);
        self.cam_x = (sx - self.vw as f32 / 2.0).clamp(0.0, (map.width - self.vw as usize) as f32);
        self.cam_y = (sy - self.vh as f32 / 2.0).clamp(0.0, (map.height - self.vh as usize) as f32);
    }

    /// Render one frame into the internal RGBA framebuffer; returns a view.
    pub fn render(&mut self) -> &[u8] {
        let map = self.map.as_ref().expect("finalize first");
        let (vw, vh) = (self.vw as usize, self.vh as usize);
        let lut = self.map_rgba;
        let mut fb = std::mem::take(&mut self.fb);

        // 1. floor: viewport blit of the index framebuffer through the LUT.
        for y in 0..vh {
            let dst = &mut fb[y * vw * 4..(y + 1) * vw * 4];
            let my = y + self.cam_y as usize;
            if my >= map.height {
                dst.fill(0);
                continue;
            }
            let x0 = self.cam_x as usize;
            let xs = x0..(x0 + vw).min(map.width);
            let row = &map.idx[my * map.width..my * map.width + map.width];
            let mut di = 0usize;
            for mx in xs.clone() {
                let p = row[mx] as usize * 4;
                dst[di..di + 4].copy_from_slice(&lut[p..p + 4]);
                di += 4;
            }
            // right margin (camera clamped): black
            dst[xs.len() * 4..vw * 4].fill(0);
        }

        // 2. entities: shadows first, then sprites sorted by screen y.
        let world = self.world.as_ref().unwrap();
        let mut drawlist: Vec<(f32, DrawKind)> = Vec::new();
        for (i, m) in world.monsters().iter().enumerate() {
            if m.anim != AnimState::DD {
                drawlist.push((m.y, DrawKind::Monster(i)));
            }
        }
        for (i, it) in world.items().iter().enumerate() {
            drawlist.push((it.y, DrawKind::Item(i)));
        }
        drawlist.push((world.hero().y, DrawKind::Hero));

        for (_, kind) in &drawlist {
            let (x, y) = self.entity_screen(*kind);
            shadow(&mut fb, vw, vh, x, y - 2.0, 16.0, 6.0, 70);
        }

        drawlist.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        for (_, kind) in &drawlist {
            match *kind {
                DrawKind::Hero => {
                    let h = *world.hero();
                    self.blit_entity(&mut fb, vw, vh, "hero", &h);
                }
                DrawKind::Monster(i) => {
                    let m = world.monsters()[i];
                    let key = self.monster_sprite_key(i);
                    self.blit_entity(&mut fb, vw, vh, &key, &m);
                }
                DrawKind::Item(i) => {
                    let (sx, sy) = self.entity_screen(DrawKind::Item(i));
                    item_marker(&mut fb, vw, vh, sx, sy);
                }
            }
        }

        self.fb = fb;
        &self.fb
    }

    fn monster_sprite_key(&self, index: usize) -> String {
        // Les codes suivent les spawns par niveau de d2game (`mon_codes`,
        // parallèle à monsters()) ; repli : ordre du manifest (historique).
        self.mon_codes.get(index).cloned().unwrap_or_else(|| {
            self.manifest
                .get("monsters")
                .and_then(|m| m.as_array())
                .and_then(|arr| arr.get(index))
                .and_then(|m| m.get("code"))
                .and_then(|c| c.as_str())
                .unwrap_or("")
                .to_string()
        })
    }

    fn entity_screen(&self, kind: DrawKind) -> (f32, f32) {
        let world = self.world.as_ref().unwrap();
        let map = self.map.as_ref().unwrap();
        let e = match kind {
            DrawKind::Hero => world.hero(),
            DrawKind::Monster(i) => &world.monsters()[i],
            DrawKind::Item(i) => {
                let it = &world.items()[i];
                let (sx, sy) = map.sub_to_screen(it.x, it.y);
                return (sx - self.cam_x, sy - self.cam_y);
            }
        };
        let (sx, sy) = map.sub_to_screen(e.x, e.y);
        (sx - self.cam_x, sy - self.cam_y)
    }

    fn blit_entity(&self, fb: &mut [u8], vw: usize, vh: usize, key: &str, e: &Entity) {
        // Set par mode "<key>:<ANIM>" en priorité, repli "<key>" (la marche).
        let tag = match e.anim {
            AnimState::NU => "NU",
            AnimState::WL => "WL",
            AnimState::RN => "RN",
            AnimState::A1 => "A1",
            AnimState::DD => "DD",
        };
        let mode_key = format!("{key}:{tag}");
        let (set, fallback_idle) = match (self.sprites.get(&mode_key), self.sprites.get(key)) {
            (Some(s), _) => (s, false),
            (None, Some(s)) => (s, true),
            // Aucun set (ex. DD sans assets DD) : entité non dessinée.
            (None, None) => return,
        };
        // Mort sans set DD dédié : pas de « cadavre qui marche » — invisible.
        if fallback_idle && e.anim == AnimState::DD {
            return;
        }
        // Sélection de frame : set dédié ⇒ frame animée du mode ; repli marche
        // ⇒ NU figé frame 0 (comportement historique), autres états animés.
        let frame = if fallback_idle && e.anim == AnimState::NU {
            0
        } else {
            (e.anim_frame as usize).min(set.frames().saturating_sub(1))
        };
        let Some(sf) = set.compose(e.facing, frame, &self.sprite_pal) else { return };
        let map = self.map.as_ref().unwrap();
        let (wx, wy) = map.sub_to_screen(e.x, e.y);
        blit_sprite(fb, vw, vh, &sf, wx - self.cam_x, wy - self.cam_y);
    }
}

#[derive(Debug, Clone, Copy)]
enum DrawKind {
    Hero,
    Monster(usize),
    Item(usize),
}

/// Le bloc de définition du héros résolu : `heroes[code]` si présent, sinon
/// le bloc `hero` historique — même règle de résolution que d2rules.
fn hero_def_for(manifest: &Value, code: &str) -> Option<Value> {
    manifest
        .pointer(&format!("/heroes/{code}"))
        .cloned()
        .or_else(|| manifest.get("hero").cloned())
}

/// Palette monstres par défaut d'un niveau extérieur sans liste `monsters`
/// (Blood Moor retail : fallen1, zombie1, quillrat1 — MAP-SPEC-BLOODMOOR §6.5).
pub const DEFAULT_OUTDOOR_MONSTERS: [&str; 6] = ["FA", "FA", "FA", "ZM", "ZM", "SI"];

impl LevelDef {
    fn from_manifest(key: &str, block: &Value) -> Result<LevelDef, String> {
        let get_str = |p: &str| {
            block.pointer(p).and_then(|v| v.as_str()).map(|s| s.to_string()).ok_or_else(|| {
                format!("manifest: levels.{key}{p} missing")
            })
        };
        let ds1 = get_str("/ds1")?;
        let palette = get_str("/palette")?;
        let dt1: Vec<String> = block
            .get("dt1")
            .and_then(|d| d.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let name = block
            .pointer("/nameDisplay")
            .or_else(|| block.pointer("/name"))
            .and_then(|n| n.as_str())
            .map(String::from)
            .unwrap_or_else(|| key.to_string());
        let monsters: Vec<String> = match block.get("monsters").and_then(|m| m.as_array()) {
            Some(a) => a.iter().filter_map(|v| v.as_str().map(|s| s.to_ascii_uppercase())).collect(),
            None => {
                if key == "town" {
                    Vec::new() // zone sûre
                } else {
                    DEFAULT_OUTDOOR_MONSTERS.iter().map(|s| s.to_string()).collect()
                }
            }
        };
        let warp = block.get("warp").and_then(|w| {
            let to = w.get("to")?.as_str()?.to_string();
            let at = json_point2(w.get("at")?)?;
            let entry = match w.get("entry") {
                Some(v) => json_point2(v)?,
                None => at,
            };
            let r = w.get("r").and_then(|v| v.as_f64()).unwrap_or(3.0) as f32;
            Some(WarpDef { to, at, r, entry })
        });
        Ok(LevelDef { key: key.to_string(), name, ds1, dt1, palette, monsters, warp })
    }
}

/// Champ JSON `[x, y]` → tuple.
fn json_point2(v: &Value) -> Option<(f32, f32)> {
    let a = v.as_array()?;
    if a.len() < 2 {
        return None;
    }
    Some((a[0].as_f64()? as f32, a[1].as_f64()? as f32))
}

/// Adapte un save d2rules (JSON v1) à un changement de niveau : grille
/// remplacée par celle du niveau cible, héros reposé au point d'entrée
/// (pleine vie, mort annulée), monstres et objets au sol vidés (le niveau
/// spawn les siens). Stats/inventaire/xp/rng conservés tels quels — c'est le
/// mécanisme de transfert du héros entre niveaux (aucun setter d2rules requis,
/// le format save v1 est le contrat public).
fn patch_save_for_level(
    save_json: &str,
    blocked: &[Vec<bool>],
    spawn: Option<(f32, f32)>,
) -> Result<World, String> {
    let mut v: Value = serde_json::from_str(save_json).map_err(|e| format!("save: {e}"))?;
    if v.get("version").and_then(|x| x.as_u64()) != Some(1) {
        return Err(format!("save: version inattendue {:?}", v.get("version")));
    }
    // Grille : '1' marchable / '0' bloqué (convention save d2rules).
    let rows: Vec<String> = blocked
        .iter()
        .map(|row| row.iter().map(|&b| if b { '0' } else { '1' }).collect())
        .collect();
    v["collision"] = Value::Array(rows.into_iter().map(Value::String).collect());
    v["monsters"] = Value::Array(Vec::new());
    v["items"] = Value::Array(Vec::new());
    if let Some((x, y)) = spawn {
        v["hero"]["ent"]["x"] = serde_json::json!(x);
        v["hero"]["ent"]["y"] = serde_json::json!(y);
        v["hero"]["ent"]["facing"] = serde_json::json!(0);
        v["hero"]["ent"]["anim"] = serde_json::json!("NU");
        v["hero"]["ent"]["anim_frame"] = serde_json::json!(0);
        v["hero"]["path"] = serde_json::json!([]);
        v["hero"]["spawn"] = serde_json::json!([x, y]);
    }
    // Mort en cours au moment du warp : le changement de niveau relève le
    // héros (pleine vie) — il renaît à l'entrée du niveau cible.
    v["hero"]["dead_ms"] = serde_json::json!(0);
    if v.pointer("/hero/stats/life").and_then(|l| l.as_u64()) == Some(0) {
        let max = v.pointer("/hero/stats/max_life").cloned().unwrap_or(serde_json::json!(55));
        v["hero"]["stats"]["life"] = max;
    }
    World::load(&v.to_string()).map_err(|()| "save patché rejeté par d2rules".to_string())
}

/// LCG déterministe (spawn des monstres par niveau) — indépendant du rng
/// d2rules : le contenu d'un niveau ne dépend pas de l'historique de combat.
struct LevelRng(u64);

impl LevelRng {
    fn new(seed: u64) -> Self {
        LevelRng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }
    fn below(&mut self, n: u32) -> u32 {
        ((self.next() >> 33) as u32) % n.max(1)
    }
}

/// FNV-1a 64 — graine de spawn stable par clé de niveau.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ComposeStatsCopy {
    pub drawn: usize,
    pub hidden: usize,
    pub empty: usize,
    pub out_of_bounds: usize,
}

use d2dcc::{Cof, Dcc};

/// Precompute index → RGBA for the map palette (gamma-normalized, alpha 0
/// where the index was never painted).
fn build_map_rgba_lut(raw_pal: &[u8], gamma: f32) -> [u8; 256 * 4] {
    let pal = MapPalette::load(raw_pal, gamma);
    let mut lut = [0u8; 256 * 4];
    for i in 0..=255u8 {
        let [r, g, b, a] = pal.rgba(i);
        let o = i as usize * 4;
        lut[o] = r;
        lut[o + 1] = g;
        lut[o + 2] = b;
        lut[o + 3] = a;
    }
    lut
}

/// The validated `.tg1` → DT1-container mapping (`tools/map_proto2.py`) —
/// table historique du Town. Pour les autres niveaux, voir
/// [`resolve_container`] qui consulte d'abord la liste DT1 du niveau.
pub fn tg1_to_container(tg1: &str) -> String {
    let norm = tg1.replace('/', "\\");
    let parts: Vec<&str> = norm.split('\\').filter(|p| !p.is_empty()).collect();
    let base = parts
        .last()
        .map(|p| p.strip_suffix(".tg1").unwrap_or(p).to_ascii_lowercase())
        .unwrap_or_default();
    let folder = parts
        .iter()
        .rev()
        .nth(1)
        .map(|p| p.to_ascii_lowercase())
        .unwrap_or_default();
    if base == "objects" {
        return if folder == "town" { "town_objects.dt1" } else { "out_objects.dt1" }.into();
    }
    match base.as_str() {
        "floor" => "town_floor.dt1",
        "fence" => "town_fence.dt1",
        "stonewall" => "out_stonewall.dt1",
        "river" => "out_river.dt1",
        "treegroups" => "out_treegroups.dt1",
        "warp" => "warp.dt1",
        // Unknown tg1 names: no container mapping — the finalize() asset
        // lookup surfaces the miss with a clear error.
        _ => "\0missing",
    }
    .to_string()
}

/// Résolution `.tg1`/`.dt1` embarqué dans le DS1 → conteneur DT1 fourni,
/// PAR NIVEAU : 1) stem exact dans la liste `dt1` du niveau (Blood Moor :
/// `floor.dt1`) ; 2) stem en suffixe du nom du conteneur (`cliff1` →
/// `bloodmoor_cliff1.dt1`, `floor` → `town_floor.dt1`, `treegroups` →
/// `out_treegroups.dt1`…) ; 3) sinon la table d'alias historique du Town
/// [`tg1_to_container`] (règle `objects` selon le dossier).
pub fn resolve_container(tg1: &str, level_dt1: &[String]) -> String {
    let norm = tg1.replace('/', "\\");
    let stem = norm
        .split('\\')
        .filter(|p| !p.is_empty())
        .last()
        .and_then(|base| base.split('.').next())
        .unwrap_or("")
        .to_ascii_lowercase();
    if !stem.is_empty() {
        let basenames: Vec<String> = level_dt1
            .iter()
            .map(|d| d.rsplit('/').next().unwrap_or(d).to_ascii_lowercase())
            .collect();
        // 1. stem exact ("floor" → "floor.dt1")
        if let Some(b) = basenames.iter().find(|b| b.strip_suffix(".dt1") == Some(&stem)) {
            return b.clone();
        }
        // 2. suffixe ("cliff1" → "bloodmoor_cliff1.dt1")
        if let Some(b) = basenames
            .iter()
            .find(|b| b.strip_suffix(".dt1").is_some_and(|s| s.ends_with(&stem)))
        {
            return b.clone();
        }
    }
    tg1_to_container(tg1)
}

/// Blit a composed sprite: bottom-center anchored on the world position.
fn blit_sprite(fb: &mut [u8], vw: usize, vh: usize, sf: &SpriteFrame, sx: f32, sy: f32) {
    let dx0 = (sx - sf.width as f32 / 2.0).round() as i32;
    let dy0 = (sy - sf.height as f32).round() as i32;
    for y in 0..sf.height {
        let fy = dy0 + y;
        if fy < 0 || fy as usize >= vh {
            continue;
        }
        for x in 0..sf.width {
            let fx = dx0 + x;
            if fx < 0 || fx as usize >= vw {
                continue;
            }
            let si = ((y * sf.width + x) * 4) as usize;
            let a = sf.rgba[si + 3];
            if a == 0 {
                continue;
            }
            let di = (fy as usize * vw + fx as usize) * 4;
            fb[di..di + 4].copy_from_slice(&sf.rgba[si..si + 4]);
        }
    }
}

/// Soft dark ellipse under an entity (cheap D2-style ground shadow).
fn shadow(fb: &mut [u8], vw: usize, vh: usize, cx: f32, cy: f32, rx: f32, ry: f32, alpha: u8) {
    let x0 = (cx - rx).floor() as i32;
    let x1 = (cx + rx).ceil() as i32;
    let y0 = (cy - ry).floor() as i32;
    let y1 = (cy + ry).ceil() as i32;
    for y in y0..=y1 {
        if y < 0 || y as usize >= vh {
            continue;
        }
        for x in x0..=x1 {
            if x < 0 || x as usize >= vw {
                continue;
            }
            let nx = (x as f32 - cx) / rx;
            let ny = (y as f32 - cy) / ry;
            if nx * nx + ny * ny > 1.0 {
                continue;
            }
            let di = (y as usize * vw + x as usize) * 4;
            let a = alpha as u32;
            for c in 0..3 {
                let v = &mut fb[di + c];
                *v = ((*v as u32 * (255 - a)) / 255) as u8;
            }
        }
    }
}

/// Small glowing marker for items on the ground (no item sprites yet).
fn item_marker(fb: &mut [u8], vw: usize, vh: usize, sx: f32, sy: f32) {
    let (w, h) = (7.0f32, 3.5f32);
    for dy in -h as i32..=h as i32 {
        for dx in -(w as i32)..=w as i32 {
            let x = sx as i32 + dx;
            let y = sy as i32 + dy;
            if x < 0 || y < 0 || x as usize >= vw || y as usize >= vh {
                continue;
            }
            let nx = dx as f32 / w;
            let ny = dy as f32 / h;
            if nx * nx + ny * ny <= 1.0 {
                let di = (y as usize * vw + x as usize) * 4;
                // warm gold sparkle, saturating add per channel
                fb[di] = fb[di].saturating_add(120);
                fb[di + 1] = fb[di + 1].saturating_add(220);
                fb[di + 2] = fb[di + 2].saturating_add(120);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");

    fn ws_root() -> Option<String> {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
        let assets = p.join("assets/manifest.json");
        assets.is_file().then(|| p.to_string_lossy().into_owned())
    }

    #[test]
    fn tg1_mapping_matches_prototype() {
        // Real TownN1.ds1 file list (release layout, leading drive slash).
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\town\\floor.tg1"), "town_floor.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\town\\fence.tg1"), "town_fence.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\town\\objects.tg1"), "town_objects.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\treegroups.tg1"), "out_treegroups.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\stonewall.tg1"), "out_stonewall.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\river.tg1"), "out_river.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\outdoors\\objects.tg1"), "out_objects.dt1");
        assert_eq!(tg1_to_container("\\d2\\data\\global\\tiles\\act1\\barracks\\warp.tg1"), "warp.dt1");
    }

    #[test]
    fn bad_manifest_rejected() {
        assert!(GameCore::new("not json", "BA").is_err());
    }

    #[test]
    fn empty_hero_code_defaults_to_ba() {
        let g = GameCore::new(MANIFEST, "").unwrap();
        let hc: Value = serde_json::from_str(&g.hero_class_json()).unwrap();
        assert_eq!(hc["code"], "BA");
        assert_eq!(hc["name"], "Barbare");
    }

    #[test]
    fn hero_class_resolves_from_heroes_block() {
        // manifest + `heroes` {BA, SOR} — la même règle que d2rules côté stats.
        let mut m: Value = serde_json::from_str(MANIFEST).unwrap();
        let mut sor = m["hero"].clone();
        sor["name"] = Value::String("Sorcière".into());
        m["heroes"] = serde_json::json!({ "BA": m["hero"], "SOR": sor });
        let g = GameCore::new(&m.to_string(), "sor").unwrap();
        let hc: Value = serde_json::from_str(&g.hero_class_json()).unwrap();
        assert_eq!(hc["code"], "SOR");
        assert_eq!(hc["name"], "Sorcière");
        // Code absent de `heroes` : retombée sur le bloc `hero` (BA).
        let g2 = GameCore::new(&m.to_string(), "NK").unwrap();
        let hc2: Value = serde_json::from_str(&g2.hero_class_json()).unwrap();
        assert_eq!(hc2["code"], "BA");
        assert_eq!(hc2["name"], "Barbare");
    }

    #[test]
    fn provide_then_missing_finalize_fails_cleanly() {
        let mut g = GameCore::new(MANIFEST, "BA").unwrap();
        g.provide("assets/map/pal_act1.dat", vec![0; 767]);
        assert!(g.finalize().is_err()); // ds1 not provided yet
    }

    // --- modes d'animation -------------------------------------------------

    /// Fournit à `g` tous les assets d'un bloc d'animation {cof, layers}.
    fn provide_anim_block(g: &mut GameCore, root: &str, block: &Value) {
        if let Some(p) = block.get("cof").and_then(|c| c.as_str()) {
            g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
        }
        if let Some(layers) = block.get("layers").and_then(|l| l.as_object()) {
            for (_, p) in layers {
                let p = p.as_str().unwrap();
                g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
            }
        }
    }

    #[test]
    fn hero_modes_built_from_named_blocks() {
        // Manifest réel : SOR a walk/idle/attack/death (fichiers présents).
        let Some(root) = ws_root() else { return };
        let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
        let m: Value = serde_json::from_str(&manifest).unwrap();
        let mut g = GameCore::new(&manifest, "SOR").unwrap();
        g.provide(
            m.pointer("/level/palette").unwrap().as_str().unwrap(),
            std::fs::read(format!("{root}/{}", m.pointer("/level/palette").unwrap().as_str().unwrap())).unwrap(),
        );
        g.provide(
            m.pointer("/level/ds1").unwrap().as_str().unwrap(),
            std::fs::read(format!("{root}/{}", m.pointer("/level/ds1").unwrap().as_str().unwrap())).unwrap(),
        );
        for p in m.pointer("/level/dt1").unwrap().as_array().unwrap() {
            let p = p.as_str().unwrap();
            g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
        }
        let sor = &m["heroes"]["SOR"];
        for field in ["walk", "idle", "attack", "death"] {
            provide_anim_block(&mut g, &root, &sor[field]);
        }
        // `modes` {NU|WL|RN|A1|DD} : fournir aussi ces assets (manifest complet).
        if let Some(modes) = sor.get("modes").and_then(|x| x.as_object()) {
            for mv in modes.values() {
                provide_anim_block(&mut g, &root, mv);
            }
        }
        for mon in m.get("monsters").unwrap().as_array().unwrap() {
            if mon.get("walk").and_then(|w| w.get("cof")).is_some() {
                provide_anim_block(&mut g, &root, &mon["walk"]);
            }
            if let Some(modes) = mon.get("modes").and_then(|x| x.as_object()) {
                for mv in modes.values() {
                    provide_anim_block(&mut g, &root, mv);
                }
            }
        }
        g.finalize().unwrap();
        // walk → base ; idle/attack/death → NU/A1/DD dédiés.
        for key in ["hero", "hero:NU", "hero:A1", "hero:DD"] {
            assert!(g.sprites.contains_key(key), "set `{key}` attendu (sets: {:?})", g.sprites.keys().collect::<Vec<_>>());
        }
        assert!(g.sprite_notes_json() == "[]", "{}", g.sprite_notes_json());
        let _ = g.tick(16, None);
        let frame = g.render();
        assert_eq!(frame.len(), (DEFAULT_VIEWPORT_W * DEFAULT_VIEWPORT_H * 4) as usize);
    }

    #[test]
    fn missing_mode_files_gracefully_skipped_with_note() {
        // `modes.RN` pointe vers des fichiers inexistants : finalize OK,
        // note émise, jeu jouable (la marche sert de repli).
        let Some(root) = ws_root() else { return };
        let mut m: Value =
            serde_json::from_str(&std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap())
                .unwrap();
        m["heroes"]["BA"]["modes"] = serde_json::json!({
            "WL": m["heroes"]["BA"]["walk"],
            "RN": {
                "cof": "assets/dcc/BARN_INEXISTANT.COF",
                "layers": { "TR": "assets/dcc/BARN_INEXISTANT.dcc" }
            }
        });
        let manifest = m.to_string();
        let mut g = GameCore::new(&manifest, "BA").unwrap();
        g.provide(
            m.pointer("/level/palette").unwrap().as_str().unwrap(),
            std::fs::read(format!("{root}/{}", m.pointer("/level/palette").unwrap().as_str().unwrap())).unwrap(),
        );
        g.provide(
            m.pointer("/level/ds1").unwrap().as_str().unwrap(),
            std::fs::read(format!("{root}/{}", m.pointer("/level/ds1").unwrap().as_str().unwrap())).unwrap(),
        );
        for p in m.pointer("/level/dt1").unwrap().as_array().unwrap() {
            let p = p.as_str().unwrap();
            g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
        }
        provide_anim_block(&mut g, &root, &m["heroes"]["BA"]["walk"]);
        for mon in m.get("monsters").unwrap().as_array().unwrap() {
            provide_anim_block(&mut g, &root, &mon["walk"]);
        }
        g.finalize().unwrap();
        let notes = g.sprite_notes_json();
        assert!(notes.contains("hero:RN"), "note RN attendue, eu : {notes}");
        assert!(g.sprites.contains_key("hero:WL"), "WL du bloc modes devient la base");
        // Le jeu tourne quand même (marche en repli sur RN).
        let _ = g.tick(16, None);
        let frame = g.render();
        let lit = frame.chunks_exact(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count();
        assert!(lit > 10_000);
    }

    #[test]
    fn hero_dead_without_dd_set_has_no_corpse_walk() {
        // BA n'a que walk : aucun set hero:DD ⇒ pas de set de mort construit,
        // et le blit DD sans set dédié ne dessine pas (pas de cadavre marchant).
        let Some(root) = ws_root() else { return };
        let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
        let m: Value = serde_json::from_str(&manifest).unwrap();
        let mut g = GameCore::new(&manifest, "BA").unwrap();
        g.provide(
            m.pointer("/level/palette").unwrap().as_str().unwrap(),
            std::fs::read(format!("{root}/{}", m.pointer("/level/palette").unwrap().as_str().unwrap())).unwrap(),
        );
        g.provide(
            m.pointer("/level/ds1").unwrap().as_str().unwrap(),
            std::fs::read(format!("{root}/{}", m.pointer("/level/ds1").unwrap().as_str().unwrap())).unwrap(),
        );
        for p in m.pointer("/level/dt1").unwrap().as_array().unwrap() {
            let p = p.as_str().unwrap();
            g.provide(p, std::fs::read(format!("{root}/{p}")).unwrap());
        }
        provide_anim_block(&mut g, &root, &m["heroes"]["BA"]["walk"]);
        for mon in m.get("monsters").unwrap().as_array().unwrap() {
            provide_anim_block(&mut g, &root, &mon["walk"]);
        }
        g.finalize().unwrap();
        assert!(!g.sprites.contains_key("hero:DD"), "BA n'a pas de DD au manifest");
        // État héros exposé pour le HUD/E2E.
        let st: Value = serde_json::from_str(&g.hero_state_json()).unwrap();
        assert_eq!(st["anim"], "NU");
        assert_eq!(st["run"], false);
        // Mort simulée : l'entité DD sans set dédié n'est PAS blittée (aucun
        // panic, framebuffer rendu) — le contrat « pas de cadavre qui marche ».
        let mut dead = *g.world.as_ref().unwrap().hero();
        dead.anim = AnimState::DD;
        dead.anim_frame = 5;
        let fb_len = g.vw as usize * g.vh as usize * 4;
        let mut fb = vec![0u8; fb_len];
        g.blit_entity(&mut fb, g.vw as usize, g.vh as usize, "hero", &dead);
        assert!(fb.iter().all(|&b| b == 0), "aucun pixel héros en DD sans set");
    }

    #[test]
    fn full_pipeline_smoke() {
        let Some(root) = ws_root() else { return }; // skip outside workspace
        let manifest = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
        let mut g = GameCore::new(&manifest, "BA").unwrap();
        let m: Value = serde_json::from_str(&manifest).unwrap();
        let mut paths = vec![
            m.pointer("/level/ds1").unwrap().as_str().unwrap().to_string(),
            m.pointer("/level/palette").unwrap().as_str().unwrap().to_string(),
        ];
        for p in m.pointer("/level/dt1").unwrap().as_array().unwrap() {
            paths.push(p.as_str().unwrap().to_string());
        }
        paths.push(m.pointer("/hero/walk/cof").unwrap().as_str().unwrap().to_string());
        for (_, p) in m.pointer("/hero/walk/layers").unwrap().as_object().unwrap() {
            paths.push(p.as_str().unwrap().to_string());
        }
        for mon in m.get("monsters").unwrap().as_array().unwrap() {
            if mon.pointer("/walk/cof").is_some() {
                paths.push(mon.pointer("/walk/cof").unwrap().as_str().unwrap().to_string());
                for (_, p) in mon.pointer("/walk/layers").unwrap().as_object().unwrap() {
                    paths.push(p.as_str().unwrap().to_string());
                }
            }
            // monstres MVP : `modes` remplace les blocs nommés
            if let Some(modes) = mon.get("modes").and_then(|x| x.as_object()) {
                for mv in modes.values() {
                    paths.push(mv["cof"].as_str().unwrap().to_string());
                    for (_, p) in mv["layers"].as_object().unwrap() {
                        paths.push(p.as_str().unwrap().to_string());
                    }
                }
            }
        }
        for p in &paths {
            let bytes = std::fs::read(format!("{root}/{p}")).unwrap_or_else(|e| panic!("{p}: {e}"));
            g.provide(p, bytes);
        }
        g.finalize().unwrap();

        // Map sanity: plenty drawn, mostly-walkable town grid.
        let stats = g.compose_stats();
        assert!(stats.drawn > 500, "drawn too few: {stats:?}");
        let (gw, gh) = g.grid_size();
        assert_eq!((gw, gh), (57 * 5, 41 * 5));
        let grid = g.collision();
        let walkable = grid.iter().filter(|&&w| w == 1).count();
        let ratio = walkable as f32 / grid.len() as f32;
        assert!(ratio > 0.2 && ratio < 0.98, "walkable ratio suspicious: {ratio}");

        // Tick + render produce a sane framebuffer with non-black pixels.
        let _ = g.tick(16, None);
        let frame = g.render();
        assert_eq!(frame.len(), (DEFAULT_VIEWPORT_W * DEFAULT_VIEWPORT_H * 4) as usize);
        let lit = frame.chunks_exact(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count();
        assert!(lit > 10_000, "render too dark: {lit} lit pixels");
    }

    // --- multi-niveaux -------------------------------------------------------

    /// Tous les assets du manifest (les 2 niveaux + héros + monstres, tous
    /// modes) fournis à `g` — prérequis des tests de changement de niveau.
    fn provide_all_levels(g: &mut GameCore, m: &Value, root: &str) {
        let mut paths: Vec<String> = Vec::new();
        let mut add_anim = |block: &Value, out: &mut Vec<String>| {
            if let Some(p) = block.get("cof").and_then(|c| c.as_str()) {
                out.push(p.to_string());
            }
            if let Some(layers) = block.get("layers").and_then(|l| l.as_object()) {
                for p in layers.values() {
                    out.push(p.as_str().unwrap().to_string());
                }
            }
        };
        let mut add_level = |lv: &Value, out: &mut Vec<String>| {
            out.push(lv["ds1"].as_str().unwrap().to_string());
            out.push(lv["palette"].as_str().unwrap().to_string());
            for p in lv["dt1"].as_array().unwrap() {
                out.push(p.as_str().unwrap().to_string());
            }
        };
        if let Some(levels) = m.get("levels").and_then(|l| l.as_object()) {
            for (_, lv) in levels {
                add_level(lv, &mut paths);
            }
        } else {
            add_level(&m["level"], &mut paths);
        }
        let hero = m.pointer("/heroes/BA").unwrap_or(&m["hero"]);
        add_anim(&hero["walk"], &mut paths);
        if let Some(modes) = hero.get("modes").and_then(|x| x.as_object()) {
            for mv in modes.values() {
                add_anim(mv, &mut paths);
            }
        }
        for mon in m.get("monsters").and_then(|x| x.as_array()).unwrap() {
            add_anim(&mon["walk"], &mut paths);
            if let Some(modes) = mon.get("modes").and_then(|x| x.as_object()) {
                for mv in modes.values() {
                    add_anim(mv, &mut paths);
                }
            }
        }
        for p in &paths {
            let bytes = std::fs::read(format!("{root}/{p}")).unwrap_or_else(|e| panic!("{p}: {e}"));
            g.provide(p, bytes);
        }
    }

    fn full_game(manifest_text: &str, level: &str, root: &str) -> GameCore {
        let m: Value = serde_json::from_str(manifest_text).unwrap();
        let mut g = GameCore::with_level(manifest_text, "BA", level).unwrap();
        provide_all_levels(&mut g, &m, root);
        g.finalize().unwrap();
        g
    }

    #[test]
    fn resolve_container_matches_both_levels() {
        // Town : table historique (floor.tg1 → town_floor.dt1).
        let town_dt1: Vec<String> = ["town_floor.dt1", "town_fence.dt1", "warp.dt1"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(resolve_container("\\d2\\tiles\\town\\floor.tg1", &town_dt1), "town_floor.dt1");
        assert_eq!(resolve_container("\\d2\\tiles\\town\\fence.tg1", &town_dt1), "town_fence.dt1");
        // Blood Moor : la DS1 embarque des noms .dt1 (stems courts) — la liste
        // du niveau tranche (préfixes bloodmoor_/out_).
        let bm_dt1: Vec<String> = [
            "out_treegroups.dt1",
            "out_stonewall.dt1",
            "floor.dt1",
            "out_stones.dt1",
            "out_river.dt1",
            "bloodmoor_cliff2.dt1",
            "bloodmoor_corner.dt1",
            "bloodmoor_cliff1.dt1",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(resolve_container("\\outdoors\\treegroups.dt1", &bm_dt1), "out_treegroups.dt1");
        assert_eq!(resolve_container("\\town\\floor.dt1", &bm_dt1), "floor.dt1");
        assert_eq!(resolve_container("\\outdoors\\stones.dt1", &bm_dt1), "out_stones.dt1");
        assert_eq!(resolve_container("\\outdoors\\cliff1.dt1", &bm_dt1), "bloodmoor_cliff1.dt1");
        assert_eq!(resolve_container("\\outdoors\\corner.dt1", &bm_dt1), "bloodmoor_corner.dt1");
        // Aucune collision stones/stonewall.
        assert_eq!(resolve_container("\\outdoors\\stonewall.dt1", &bm_dt1), "out_stonewall.dt1");
    }

    #[test]
    fn level_def_resolves_town_legacy_and_unknown() {
        let m: Value = serde_json::from_str(MANIFEST).unwrap();
        let g = GameCore::new(MANIFEST, "BA").unwrap();
        // town : via levels.town
        let def = g.resolve_level_def("town").unwrap();
        assert_eq!(def.ds1, m.pointer("/levels/town/ds1").unwrap().as_str().unwrap());
        assert_eq!(def.name, "Camp des Rousses");
        assert!(def.monsters.is_empty(), "town = zone sûre");
        // bloodmoor : nom affiché + palette FA/ZM/SI + warp retour
        let bm = g.resolve_level_def("BLOODMOOR").unwrap(); // insensible à la casse
        assert_eq!(bm.name, "Blood Moor (Acte I)");
        assert_eq!(bm.monsters.len(), 6);
        assert!(bm.monsters.iter().all(|c| ["FA", "ZM", "SI"].contains(&c.as_str())));
        let warp = bm.warp.as_ref().unwrap();
        assert_eq!(warp.to, "town");
        // niveau inconnu : erreur claire (pas de panique)
        assert!(g.resolve_level_def("chaos_sanctuary").is_err());
        // rétro-compat : sans bloc `levels`, `level` sert de town
        let mut legacy = m.clone();
        legacy.as_object_mut().unwrap().remove("levels");
        let g2 = GameCore::new(&legacy.to_string(), "BA").unwrap();
        let def2 = g2.resolve_level_def("town").unwrap();
        assert_eq!(def2.ds1, m.pointer("/level/ds1").unwrap().as_str().unwrap());
        assert!(g2.resolve_level_def("bloodmoor").is_err());
    }

    #[test]
    fn town_boot_is_safe_zone() {
        let Some(root) = ws_root() else { return };
        let g = full_game(MANIFEST, "town", &root);
        assert_eq!(g.grid_size(), (57 * 5, 41 * 5));
        assert!(g.world.as_ref().unwrap().monsters().is_empty(), "town sans monstres");
        assert!(g.mon_codes.is_empty());
    }

    #[test]
    fn switch_to_bloodmoor_reloads_and_spawns() {
        let Some(root) = ws_root() else { return };
        let mut g = full_game(MANIFEST, "town", &root);
        // switch : nouvelle carte, héros au point d'entrée, monstres spawnés
        let info = g.switch_level("bloodmoor", 137.0, 162.0).unwrap();
        assert_eq!(info.key, "bloodmoor");
        assert_eq!(g.grid_size(), (72 * 5, 72 * 5));
        assert_eq!(g.level_info_json().contains("Blood Moor (Acte I)"), true);
        let world = g.world.as_ref().unwrap();
        let h = world.hero();
        assert!((h.x - 137.5).abs() < 3.0 && (h.y - 162.5).abs() < 3.0, "héros à l'entrée: {:?}", (h.x, h.y));
        let n = world.monsters().len();
        assert!((5..=8).contains(&n), "5-8 monstres, eu {n}");
        assert_eq!(g.mon_codes.len(), n);
        assert!(g.mon_codes.iter().all(|c| ["FA", "ZM", "SI"].contains(&c.as_str())));
        for m in world.monsters() {
            assert!(world.is_walkable(m.x as i32, m.y as i32), "monstre hors marchable");
            assert!((m.x - h.x).hypot(m.y - h.y) >= 10.0, "monstre collé au héros");
            // hors de la zone warp de retour
            assert!((m.x - 137.0).hypot(m.y - 147.0) >= 9.0, "monstre sur le warp");
        }
        // le rendu suit (frame pleine, non noire)
        let _ = g.tick(16, None);
        let frame = g.render();
        let lit = frame.chunks_exact(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30).count();
        assert!(lit > 10_000, "bloodmoor rendu trop sombre: {lit}");
        // et retour au camp : de nouveau sans monstre
        g.switch_level("town", 247.0, 107.0).unwrap();
        assert_eq!(g.grid_size(), (57 * 5, 41 * 5));
        assert!(g.world.as_ref().unwrap().monsters().is_empty());
    }

    #[test]
    fn level_switch_carries_hero_stats_and_inventory() {
        let Some(root) = ws_root() else { return };
        let mut g = full_game(MANIFEST, "town", &root);
        // état héros « vécu » : via un save patché (le save/load est public)
        let save: Value = serde_json::from_str(&g.save_world()).unwrap();
        let mut patched = save.clone();
        patched["hero"]["stats"]["life"] = serde_json::json!(17);
        patched["hero"]["stats"]["xp"] = serde_json::json!(650);
        patched["hero"]["stats"]["level"] = serde_json::json!(2);
        patched["hero"]["stats"]["gold"] = serde_json::json!(123);
        patched["hero"]["inv"][5] = serde_json::json!({
            "code": "hp1", "name": "Lesser Healing Potion", "kind": "Potion", "amount": 0
        });
        g.load_world(&patched.to_string()).unwrap();

        g.switch_level("bloodmoor", 137.0, 162.0).unwrap();
        let st = g.world.as_ref().unwrap().hero_stats().clone();
        assert_eq!(st.life, 17, "vie conservée");
        assert_eq!(st.xp, 650, "xp conservé");
        assert_eq!(st.level, 2, "niveau conservé");
        assert_eq!(st.gold, 123, "or conservé");
        let inv = g.inventory_json();
        assert!(inv.contains("hp1"), "inventaire conservé : {inv}");
        // retour au camp : toujours là
        g.switch_level("town", 247.0, 107.0).unwrap();
        assert_eq!(g.world.as_ref().unwrap().hero_stats().xp, 650);
    }

    #[test]
    fn warp_zones_round_trip_town_bloodmoor_town() {
        let Some(root) = ws_root() else { return };
        let mut g = full_game(MANIFEST, "town", &root);
        let walk_to = |g: &mut GameCore, x: i32, y: i32| -> bool {
            let mut switched = false;
            let _ = g.tick(16, Some(PlayerInput::MoveTo { x, y }));
            for _ in 0..6000 {
                let evs = g.tick(16, None);
                if evs.iter().any(|e| matches!(e, CoreEvent::LevelSwitch(_))) {
                    switched = true;
                    break;
                }
            }
            switched
        };
        // Town → Blood Moor : le héros marche dans la zone de sortie Est.
        assert!(walk_to(&mut g, 282, 107), "le warp Town→BloodMoor n'a pas déclenché");
        assert_eq!(g.grid_size(), (72 * 5, 72 * 5));
        // Blood Moor → Town : retour par les chemins du centre.
        assert!(walk_to(&mut g, 137, 147), "le warp BloodMoor→Town n'a pas déclenché");
        assert_eq!(g.grid_size(), (57 * 5, 41 * 5));
        assert!(g.world.as_ref().unwrap().monsters().is_empty());
    }
}
