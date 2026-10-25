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
use crate::drlg;
use crate::drlg_a45;
use crate::drlg_maze;
use crate::ds1::{self, Ds1};
use crate::dt1::{self, Dt1Tile};
use crate::levels;
use crate::monsters::{
    plan_level_spawns_diff, LevelSpawnPlan, MonLvl, MonStats, MonUMods, SuperUniques,
};
use crate::pal::MapPalette;
use crate::sprite::{sprite_palette, SpriteFrame, SpriteSet};
use crate::world::{AnimState, Difficulty, Entity, GameEvent, MonKind, PlayerInput, SpawnRule, World};

pub const DEFAULT_VIEWPORT_W: u32 = 800;
pub const DEFAULT_VIEWPORT_H: u32 = 600;

/// Asset optionnel `AnimData.d2` (extraits MPQ v1.12, `docs/GAME-DATA.md`) :
/// quand il est fourni via [`GameCore::provide`], chaque sprite set reçoit
/// les frames/speed officiels de son token COF (audit od2-features-formats
/// #19) ; sinon le comportement historique (octets COF) est conservé.
pub const ANIMDATA_ASSET: &str = "assets/animdata/AnimData.d2";

/// Asset optionnel `patch_levels.txt` (levels.txt final v1.12, W01) : quand
/// il est fourni via [`GameCore::provide`], le moteur branche le graphe
/// Vis/Warp (W04) et le DRLG wilderness seedé (W02) ; sinon — ou si la
/// table est illisible — le manifest historique reste l'unique source
/// (fallback gracieux, zéro régression).
pub const LEVELS_ASSET: &str = "assets/patch_levels.txt";

/// Assets optionnels du contenu monstre data-driven (W06-W08, brief
/// `impl-monstres-boss`) : TOUS requis pour activer le spawn data-driven ;
/// absents ou illisibles ⇒ repli intégral sur le chemin historique (codes du
/// manifest, `DEFAULT_OUTDOOR_MONSTERS`), jamais fatal.
pub const MONSTATS_ASSET: &str = "assets/patch_monstats.txt";
pub const MONUMOD_ASSET: &str = "assets/monumod.txt";
pub const SUPERUNIQUES_ASSET: &str = "assets/superuniques.txt";
pub const MONLVL_ASSET: &str = "assets/MonLvl.txt";

/// Les 4 tables monstre v1.12 parsées (`monsters.rs`).
#[derive(Debug, Clone)]
pub struct MonsterTables {
    pub monstats: MonStats,
    pub monumod: MonUMods,
    pub superuniques: SuperUniques,
    pub monlvl: MonLvl,
}

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
    /// Id levels.txt (`levelsId` du bloc, W01) — relie le niveau au graphe
    /// Vis/Warp et au DRLG seedé.
    pub levels_id: Option<u32>,
    /// Nom affichable (`nameDisplay` > `name` > clé).
    pub name: String,
    pub ds1: String,
    pub dt1: Vec<String>,
    pub palette: String,
    /// Codes monstres à spawn (`monsters` du bloc ; [] pour le town — zone
    /// sûre —, planche mon1-10 réelle du record levels.txt pour un niveau
    /// extérieur sans liste, socle W01/A3).
    pub monsters: Vec<String>,
    /// Packs `MinGrp`/`MaxGrp` (patch_monstats.txt) PARALLÈLES à `monsters`
    /// quand la palette vient des planches levels.txt (socle W01/A3) — vide
    /// pour une liste `monsters` du manifest : comportement historique,
    /// 1 monstre par entrée.
    pub plate_packs: Vec<(u32, u32)>,
    /// Boss de quête du niveau (W11) — code `manifest.bosses` (ex. "AN"
    /// andariel), spawné SEUL hors des packs, loin du héros. `None` = aucun.
    pub boss: Option<String>,
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
/// propres au moteur multi-niveaux (transitions, échecs de transition) + les
/// événements de quête (mort d'un boss, W11).
#[derive(Debug, Clone)]
pub enum CoreEvent {
    World(GameEvent),
    LevelSwitch(LevelSwitchInfo),
    LevelSwitchFailed(String),
    /// Un boss de quête est mort (`manifest.bosses[].questEvent`, ex.
    /// `"andariel_dead"`) — une quête/fin l'écoutera (W11/W13).
    QuestEvent { id: String },
}

impl CoreEvent {
    /// Sérialisation JSON : les événements monde gardent EXACTEMENT la forme
    /// du contrat ({"HeroMoved":{…}}…) ; les extras du moteur s'ajoutent.
    pub fn to_json(&self) -> Value {
        match self {
            CoreEvent::World(e) => serde_json::to_value(e).unwrap_or(Value::Null),
            CoreEvent::LevelSwitch(i) => serde_json::json!({ "LevelSwitch": i }),
            CoreEvent::LevelSwitchFailed(m) => serde_json::json!({ "LevelSwitchFailed": m }),
            CoreEvent::QuestEvent { id } => serde_json::json!({ "QuestEvent": { "id": id } }),
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
    pub(crate) level: Option<LevelDef>,
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
    /// Rangs (W08) par index de `monsters()` — parallèle à `mon_codes`,
    /// alimenté par le plan data-driven (champion/unique/super-unique/boss).
    mon_kinds: Vec<MonKind>,
    /// Noms affichables par index (super-uniques nommés W10, boss W11) —
    /// vide pour un monstre sans nom.
    mon_names: Vec<String>,
    /// Tables monstre v1.12 (W06-W08) si les 4 assets sont fournis et
    /// lisibles — `None` sinon (repli historique intégral).
    mon_tables: Option<MonsterTables>,
    /// Événements de quête par code monstre (`manifest.bosses[].questEvent`,
    /// W11) — émis à la mort du monstre correspondant.
    quest_events: BTreeMap<String, String>,
    /// Délai anti-rebond restant après une transition (ms).
    warp_cd_ms: u32,
    pub(crate) world: Option<World>,
    /// Héros demandé ("BA"|"SOR"|"PA"…, normalisé uppercase) — résolu contre
    /// `manifest.heroes` (retombée `hero` à l'ancienne via d2rules).
    hero_request: String,
    /// AnimData.d2 parsé si l'asset [`ANIMDATA_ASSET`] a été fourni — `None`
    /// sinon (fallback historique, zéro changement de comportement).
    animdata: Option<d2animdata::AnimationData>,
    /// Tables de niveaux v1.12 (W01) si [`LEVELS_ASSET`] fourni — `None`
    /// sinon (repli manifest historique intégral).
    tables: Option<levels::LevelTables>,
    /// Difficulté courante (W08/K, impl-difficultes) : Normal par défaut ;
    /// `set_difficulty` (wasm `setDifficulty`, `?diff=` de la page) avant
    /// `finalize` — gouverne le plan de spawn (MonLvl N/H, densités,
    /// résists/immunités) et le monde (pénalité XP à la mort). Persistée
    /// dans la save du monde (transferts de niveau).
    difficulty: Difficulty,
    /// Graphe Vis/Warp (W04) dérivé des tables — `None` sans tables.
    warp_graph: Option<levels::WarpGraph>,
    /// Cache AutoMap du niveau courant (9d monde vivant, `automap.rs`) —
    /// reconstruit à chaque `load_level`, sérialisé par `automap_json`.
    pub(crate) automap: Option<crate::automap::LevelAutomap>,
    /// Graine de partie : combine la graine DRLG de chaque niveau (W02) —
    /// même graine ⇒ mêmes cartes, parties différentes ⇒ mondes différents.
    /// 0 par défaut (cartes stables d'un boot à l'autre, historique respecté).
    game_seed: u64,
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
            mon_kinds: Vec::new(),
            mon_names: Vec::new(),
            mon_tables: None,
            quest_events: BTreeMap::new(),
            warp_cd_ms: 0,
            world: None,
            hero_request,
            animdata: None,
            tables: None,
            difficulty: Difficulty::default(),
            warp_graph: None,
            automap: None,
            game_seed: 0,
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

    /// Fixe la graine de partie (W02) : chaque niveau DRLG dérive sa graine
    /// de `drlg::level_seed(game_seed, clé)` — même graine ⇒ mêmes cartes,
    /// graine différente ⇒ monde différent, tous déterministes. À appeler
    /// avant `finalize`/`switch_level` (la carte est générée au chargement).
    pub fn set_game_seed(&mut self, seed: u64) {
        self.game_seed = seed;
    }

    /// Graine de partie courante (0 par défaut — cartes stables par boot).
    pub fn game_seed(&self) -> u64 {
        self.game_seed
    }

    /// Fixe la difficulté (W08/K) : `Difficulty::parse` côté wasm — à
    /// appeler avant `finalize` pour que le niveau initial spawn ses
    /// monstres scalés (MonLvl N/H, densités MonUMin/Max N/H, résists/
    /// immunités `Res*(N)/(H)`). Si un monde existe déjà (après finalize),
    /// il bascule aussi (pénalité XP à la mort, résists appliqués au tick) ;
    /// les monstres déjà posés gardent leurs stats.
    pub fn set_difficulty(&mut self, d: Difficulty) {
        self.difficulty = d;
        if let Some(w) = self.world.as_mut() {
            w.set_difficulty(d);
        }
    }

    /// Difficulté courante (lecture).
    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    /// Difficulté en JSON pour le HUD/E2E wasm : `{"difficulty":"hell",
    /// "label":"Hell"}`.
    pub fn difficulty_json(&self) -> String {
        let d = self.difficulty;
        serde_json::json!({ "difficulty": d.as_str(), "label": difficulty_label(d) })
            .to_string()
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
        // AnimData.d2 (optionnel) : si fourni, les sets recevront les
        // frames/speed officiels ; un fichier illisible produit une note,
        // pas un échec (fallback octets COF).
        self.animdata = match self.asset(ANIMDATA_ASSET) {
            Ok(bytes) => match d2animdata::AnimationData::load(bytes) {
                Ok(ad) => {
                    notes.push(format!(
                        "animdata branché : {} records / {} tokens ({})",
                        ad.total_records(),
                        ad.entry_count(),
                        ANIMDATA_ASSET
                    ));
                    Some(ad)
                }
                Err(e) => {
                    notes.push(format!("animdata fourni mais illisible ({e}) — repli octets COF"));
                    None
                }
            },
            Err(_) => None, // asset absent : fallback historique, pas une erreur
        };
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
        // Bosses de quête (W11) : sets de sprites sous leur CODE (ex. "AN"),
        // jamais auto-spawnés — posés par le bloc `boss` d'un niveau.
        if let Some(list) = self.manifest.get("bosses").and_then(|b| b.as_array()) {
            for b in list {
                let code = b.get("code").and_then(|c| c.as_str()).unwrap_or("??").to_string();
                Self::build_entity_sets(&code, b, &mut sprites, &mut notes, self)?;
                if let Some(ev) = b.get("questEvent").and_then(|e| e.as_str()) {
                    self.quest_events.insert(code.to_ascii_uppercase(), ev.to_string());
                }
            }
        }
        self.sprites = sprites;
        self.sprite_notes = notes;

        // --- tables de niveaux (W01/W04, optionnelles) ----------------------
        // Asset fourni ⇒ graphe Vis/Warp branché + DRLG seedé actif pour les
        // niveaux DrlgType 3 supportés ; illisible ⇒ note + repli manifest.
        match self.asset(LEVELS_ASSET) {
            Ok(bytes) => match levels::LevelTables::parse(&String::from_utf8_lossy(bytes)) {
                Ok(t) => {
                    let g = levels::WarpGraph::build(&t);
                    self.switch_notes.push(format!(
                        "tables de niveaux branchées (W01/W04) : {} records, graphe Vis/Warp {} liens",
                        t.len(),
                        g.edge_count()
                    ));
                    self.tables = Some(t);
                    self.warp_graph = Some(g);
                }
                Err(e) => {
                    self.switch_notes
                        .push(format!("tables de niveaux illisibles ({e}) — repli manifest"));
                }
            },
            Err(_) => {} // asset absent : comportement historique, pas une note
        }

        // --- tables monstre (W06-W08, optionnelles) -------------------------
        // Les 4 assets fournis et lisibles ⇒ spawn data-driven actif
        // (planches MonDen/packs/champions/uniques/super-uniques/MonLvl) ;
        // sinon repli historique intégral (codes manifest, jamais fatal).
        self.mon_tables = self.load_monster_tables();

        // --- niveau initial : carte + monde + monstres ----------------------
        let def = self.resolve_level_def(&self.level_name)?;
        self.load_level(&def, None)?; // None = auto-spawn du monde (centre, historique)

        let (hw, hh) = (self.vw as usize * 4, self.vh as usize);
        self.fb = vec![0; hw * hh];
        self.finalized = true;
        Ok(())
    }

    /// Résout la définition d'un niveau : `levels.{key}` ; retombée sur le bloc
    /// `level` historique pour "town" (avec ou sans bloc `levels`). Avec les
    /// tables branchées (W01/W04), accepte aussi les clés par id de table —
    /// `"2"` ou `"level:2"` — résolues vers le bloc manifest portant ce
    /// `levelsId` (les transitions peuvent viser un id levels.txt).
    pub(crate) fn resolve_level_def(&self, key: &str) -> Result<LevelDef, String> {
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
        // Résolution par id de table (W04) : "7" / "level:7" → bloc manifest
        // dont `levelsId` vaut 7. Sans tables, les ids n'ont pas de sens.
        let id_key = key.strip_prefix("level:").unwrap_or(&key);
        if let Ok(id) = id_key.parse::<u32>() {
            if self.tables.is_some() {
                if let Some((block_key, block)) = self.manifest_block_for_level_id(id) {
                    return LevelDef::from_manifest(&block_key, block);
                }
                return Err(format!(
                    "tables : niveau {id} sans bloc manifest (assets ds1/dt1/palette introuvables)"
                ));
            }
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

    /// Le bloc `levels.*` du manifest dont `levelsId` vaut `id` (clé incluse).
    pub(crate) fn manifest_block_for_level_id(&self, id: u32) -> Option<(String, &Value)> {
        self.manifest
            .get("levels")?
            .as_object()?
            .iter()
            .find(|(_, block)| block.get("levelsId").and_then(|v| v.as_u64()) == Some(id as u64))
            .map(|(k, block)| (k.clone(), block))
    }

    /// Ce niveau doit-il passer par le DRLG seedé ? Oui si les tables sont
    /// branchées, que le record levels.txt existe (`levelsId`) et déclare
    /// un niveau généré supporté : fin de jeu A4-A5 (étape 10a —
    /// [`drlg_a45::supports`] : presets réels + wilderness/maze seedés),
    /// wilderness (DrlgType 3, phase 1 Blood Moor — banque de presets
    /// extraite) ou maze (DrlgType 1, étape 8a — [`drlg_maze::supports`] :
    /// record LvlMaze + banque LvlTypes).
    fn drlg_applies(&self, def: &LevelDef) -> bool {
        let Some(tables) = &self.tables else { return false };
        let Some(id) = def.levels_id else { return false };
        let Some(rec) = tables.get(id) else { return false };
        match rec.drlg_type {
            _ if drlg_a45::supports(id) => true,
            3 => drlg::supports(&def.key),
            1 => drlg_maze::supports(id),
            _ => false,
        }
    }

    /// Niveau maze (DrlgType 1) ou fin de jeu généré sans DS1 figé
    /// (étape 10a : Bloody Foothills, Crystalized Cavern) — ni DS1 figé
    /// dans les données release ni repli possible — utilisé par la branche
    /// DRLG de [`GameCore::load_level`] pour rendre l'échec DRLG fatal
    /// (jamais de carte noire silencieuse).
    fn maze_applies(&self, def: &LevelDef) -> bool {
        if !self.drlg_applies(def) {
            return false;
        }
        let Some(id) = def.levels_id else { return false };
        if drlg_a45::generated_only(id) {
            return true;
        }
        self.tables
            .as_ref()
            .and_then(|t| t.get(id))
            .is_some_and(|rec| rec.drlg_type == 1)
    }

    /// Chemin historique : DS1 figé du manifest + DT1 résolus par .tg1.
    /// Rend aussi la méta AutoMap (9d) alignée sur la table de tuiles —
    /// chaque conteneur contribue ses `(Type, Style, Sequence)` dans le
    /// MÊME ordre que ses tuiles décodées (voir `automap::dt1_meta_aligned`).
    /// FIX-CAMPES : decode le DS1 figé au schéma réel v18 + les 8 conteneurs
    /// en lecture appariée « tuile k = record k » (métadonnées dir/type/
    /// style/seq). `None` = le DS1 n'est pas au schéma réel (DRLG, autres
    /// niveaux) — chemin historique. Ordre des conteneurs = ordre des .tg1
    /// du DS1 (l'espace d'indices du pool est celui de la concaténation).
    fn build_frozen_real(
        &self,
        def: &LevelDef,
    ) -> Option<(crate::ds1_real::Ds1Real, Vec<crate::dt1::Dt1Tile>, Vec<crate::dt1::Dt1Meta>)> {
        let ds1_bytes = self.asset(&def.ds1).ok()?.clone();
        let real = crate::ds1_real::decode_ds1_real(&ds1_bytes).ok()?;
        let mut tiles: Vec<crate::dt1::Dt1Tile> = Vec::new();
        let mut metas: Vec<crate::dt1::Dt1Meta> = Vec::new();
        for tg1 in &real.files {
            let container = resolve_container(tg1, &def.dt1);
            let bytes = self.asset(&container).ok()?.clone();
            let mut paired = crate::dt1::decode_dt1_paired(&bytes).ok()?;
            tiles.extend(paired.iter().map(|(t, _)| t.clone()));
            metas.extend(paired.drain(..).map(|(_, m)| m));
        }
        Some((real, tiles, metas))
    }

    fn build_frozen_tiles(&self, def: &LevelDef) -> Result<(Ds1, Vec<Dt1Tile>, Vec<crate::automap::TileMeta>), String> {
        let ds1_bytes = self.asset(&def.ds1)?.clone();
        let ds1 = ds1::decode_ds1(&ds1_bytes).map_err(|e| format!("{}: {e:?}", def.ds1))?;
        let mut tilesets: Vec<Dt1Tile> = Vec::new();
        let mut meta: Vec<crate::automap::TileMeta> = Vec::new();
        for tg1 in &ds1.files {
            let container = resolve_container(tg1, &def.dt1);
            let bytes = self.asset(&container)?.clone();
            let mut tiles = dt1::decode_dt1(&bytes).map_err(|e| format!("{container}: {e:?}"))?;
            meta.append(&mut crate::automap::dt1_meta_aligned(&bytes).map_err(|e| format!("{container}: {e:?}"))?);
            tilesets.append(&mut tiles);
        }
        Ok((ds1, tilesets, meta))
    }

    /// Chemin DRLG (W02 + étape 8a) : dispatch par DrlgType du record —
    /// 1 = maze ([`build_maze_tiles`]), 3 = wilderness Blood Moor (tuiles
    /// réelles des 8 conteneurs unifiés, ordre = [`drlg::UNIFIED_FILES`]).
    /// L'ordre de concaténation produit ici est exactement celui que la
    /// boucle historique aurait suivi sur les fichiers du [`Ds1`] généré —
    /// les indices des couches générées tombent donc dans le bon espace.
    fn build_drlg_tiles(&self, def: &LevelDef) -> Result<(Ds1, Vec<Dt1Tile>, Vec<crate::automap::TileMeta>), String> {
        let id = def
            .levels_id
            .and_then(|id| self.tables.as_ref()?.get(id).map(|_| id))
            .ok_or_else(|| format!("drlg : niveau {} sans record levels.txt", def.key))?;
        // Fin de jeu A4-A5 (étape 10a) : presets réels + wilderness/maze
        // seedés — la branche passe AVANT le dispatch historique.
        if drlg_a45::supports(id) {
            return self.build_a45_tiles(def, id);
        }
        if self.tables.as_ref().is_some_and(|t| t.get(id).is_some_and(|r| r.drlg_type == 1)) {
            return self.build_maze_tiles(def, id);
        }
        let mut tilesets: Vec<Dt1Tile> = Vec::new();
        let mut meta: Vec<crate::automap::TileMeta> = Vec::new();
        let mut counts: Vec<usize> = Vec::with_capacity(drlg::UNIFIED_FILES.len());
        for tg1 in drlg::UNIFIED_TG1 {
            let container = resolve_container(tg1, &def.dt1);
            let bytes = self.asset(&container)?.clone();
            let tiles = dt1::decode_dt1(&bytes).map_err(|e| format!("{container}: {e:?}"))?;
            meta.append(&mut crate::automap::dt1_meta_aligned(&bytes).map_err(|e| format!("{container}: {e:?}"))?);
            counts.push(tiles.len());
            tilesets.extend(tiles);
        }
        let counts = drlg::UnifiedCounts::from_slices(&counts)?;
        let seed = drlg::level_seed(self.game_seed, &def.key);
        let (ds1, _stats) = drlg::generate_bloodmoor(seed, &counts)?;
        Ok((ds1, tilesets, meta))
    }

    /// DRLG maze (étape 8a, DrlgType 1) : les DT1 de la banque du niveau
    /// ([`drlg_maze::bank_for_level`], résolus par la liste `dt1` du bloc
    /// manifest), concaténés dans l'ordre [`drlg_maze::MazeBank::files`] —
    /// puis `drlg_maze::generate(level_id, graine de partie)` : grille
    /// LvlMaze seedée (rooms + couloirs), tuiles réelles LvlTypes, sortie
    /// [`Ds1`] synthétique consommé par le composeur (comme la Blood Moor).
    fn build_maze_tiles(&self, def: &LevelDef, id: u32) -> Result<(Ds1, Vec<Dt1Tile>, Vec<crate::automap::TileMeta>), String> {
        let bank = drlg_maze::bank_for_level(id)
            .ok_or_else(|| format!("drlg maze : niveau {id} sans banque supportée"))?;
        let mut tilesets: Vec<Dt1Tile> = Vec::new();
        let mut meta: Vec<crate::automap::TileMeta> = Vec::new();
        for file in bank.files {
            let container = resolve_container(file.tg1, &def.dt1);
            let bytes = self.asset(&container)?.clone();
            let tiles = dt1::decode_dt1(&bytes).map_err(|e| format!("{container}: {e:?}"))?;
            meta.append(&mut crate::automap::dt1_meta_aligned(&bytes).map_err(|e| format!("{container}: {e:?}"))?);
            tilesets.extend(tiles);
        }
        let seed = drlg::level_seed(self.game_seed, &def.key);
        let (ds1, _stats) = drlg_maze::generate(id, seed, &tilesets)?;
        Ok((ds1, tilesets, meta))
    }

    /// DRLG fin de jeu A4-A5 (étape 10a, [`crate::drlg_a45`]) : les DT1 de
    /// la banque du niveau ([`drlg_a45::bank_for_level`], résolus par la
    /// liste `dt1` du bloc manifest), concaténés dans l'ordre
    /// [`drlg_a45::A45Bank::files`] — puis
    /// `drlg_a45::generate(level_id, graine de partie)` : presets réels
    /// (Fortress/townsiege/Nihl) ou niveaux seedés (Bloody Foothills,
    /// Crystalized Cavern), sortie [`Ds1`] consommée par le composeur.
    fn build_a45_tiles(&self, def: &LevelDef, id: u32) -> Result<(Ds1, Vec<Dt1Tile>, Vec<crate::automap::TileMeta>), String> {
        let bank = drlg_a45::bank_for_level(id)
            .ok_or_else(|| format!("drlg a45 : niveau {id} sans banque supportée"))?;
        let mut tilesets: Vec<Dt1Tile> = Vec::new();
        let mut meta: Vec<crate::automap::TileMeta> = Vec::new();
        for file in bank.files {
            // 13b : le conteneur local est porté par la banque elle-même
            // (extraction 10a) — PAS de résolution .tg1 : les noms embarqués
            // du DS1 réel divergent des fichiers extraits (foregate →
            // forgate, inv_wall → invwall) et tombaient sur « \0missing »,
            // rendant les 5 niveaux A4-A5 inchargeables via GameCore.
            let bytes = self.asset(file.local)?.clone();
            let container = file.local;
            let tiles = dt1::decode_dt1(&bytes).map_err(|e| format!("{container}: {e:?}"))?;
            meta.append(&mut crate::automap::dt1_meta_aligned(&bytes).map_err(|e| format!("{container}: {e:?}"))?);
            tilesets.extend(tiles);
        }
        let seed = drlg::level_seed(self.game_seed, &def.key);
        let (ds1, _stats) = drlg_a45::generate(id, seed, &tilesets)?;
        Ok((ds1, tilesets, meta))
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

        // --- tilesets + layout, résolution .tg1 → DT1 du niveau -------------
        // Deux chemins : DRLG seedé quand les tables sont branchées et que
        // le niveau est un wilderness supporté (Blood Moor) ou un maze
        // (DrlgType 1, étape 8a — drlg_maze) — le layout est GÉNÉRÉ en
        // mémoire depuis les tables réelles, seedé par partie. Sinon le
        // DS1 figé du manifest (comportement historique). UN SEUL POINT DE
        // BRANCHEMENT (ce match) : l'échec DRLG retombe sur le DS1 figé
        // avec note — SAUF maze, qui n'a AUCUN DS1 figé dans les données
        // release (DrlgType 1 = généré) : l'échec y est fatal, jamais une
        // carte invalide silencieuse.
        let mut drlg_used = false;
        let (ds1, tilesets, meta) = match self.drlg_applies(def) {
            true => match self.build_drlg_tiles(def) {
                Ok(tiles) => {
                    drlg_used = true;
                    tiles
                }
                Err(e) => {
                    if self.maze_applies(def) {
                        return Err(format!("niveau {} : DRLG maze en échec ({e})", def.key));
                    }
                    self.switch_notes
                        .push(format!("niveau {} : DRLG en échec ({e}) — repli DS1 figé", def.key));
                    self.build_frozen_tiles(def)?
                }
            },
            false => self.build_frozen_tiles(def)?,
        };
        if drlg_used {
            self.switch_notes.push(format!(
                "drlg seedé actif (W02/8a) : {} {}×{} généré, graine partie {}",
                def.key,
                ds1.w,
                ds1.h,
                self.game_seed
            ));
        }

        // --- map ----------------------------------------------------------
        // Variation seedée des tuiles (Go getRandomTile) : graine par clé
        // de niveau — même niveau ⇒ même carte, indépendamment du combat.
        // FIX-CAMPES : le DS1 figé porte-t-il le schéma réel v18 (section
        // `(2,1)` — ville 1) ? Si oui, la composition/appariement passent
        // par la règle du jeu réel ; sinon (DRLG synthétique, autres DS1)
        // chemin historique inchangé.
        let real = if drlg_used {
            None
        } else {
            self.build_frozen_real(def)
        };
        let map = match &real {
            Some((r, ts, ms)) => compose::compose_real_town(r, ts, ms, fnv1a(&def.key)),
            None => compose::compose_seeded(&ds1, &tilesets, fnv1a(&def.key)),
        };
        // 9d (monde vivant) : cache AutoMap du niveau — cellules visibles
        // classées + Cel résolus depuis AutoMap.txt (automap.rs). La grille
        // de marche vient d'être composée ; DS1/tuiles/méta encore en main.
        self.automap = Some(crate::automap::LevelAutomap::build(
            &def.key,
            self.tables.as_ref(),
            def.levels_id,
            &ds1,
            &tilesets,
            &meta,
            &map,
        ));
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
        // FIX-CAMPES : sans point d'entrée explicite (boot du monde), le
        // héros démarre au camp (centroïde des tentes du layout réel) au
        // lieu du centre de la carte (historique = en pleine forêt).
        let spawn = match spawn {
            None => map.camp_spawn.map(|(x, y)| (x as f32 + 0.5, y as f32 + 0.5)),
            other => other,
        };
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
        let mut world = self.build_level_world(blocked, spawn, def)?;
        // W05 : attache le waypoint du niveau (table réelle waypoints.rs).
        crate::waypoint_travel::attach_level_waypoint(&mut world, def);
        // W16 : attache le roster NPC de ville (camp seulement, npc_bridge.rs).
        crate::npc_bridge::attach_town_npcs(&mut world, def);
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
                match patch_save_for_level(&json, &blocked, spawn, &def.key) {
                    Ok(w) => w,
                    Err(e) => {
                        let msg = format!("niveau {}: transfert du héros impossible ({e}) — monde neuf", def.key);
                        self.switch_notes.push(msg);
                        World::with_hero(blocked, self.manifest.clone(), &self.hero_request)
                    }
                }
            }
            None => {
                // FIX-CAMPES : monde neuf au boot — le héros démarre au point
                // d'entrée calculé (camp des tentes sur la ville 1) au lieu
                // du centre de la grille (pick_auto_spawns).
                let mut w = World::with_hero(blocked, self.manifest.clone(), &self.hero_request);
                if let Some((x, y)) = spawn {
                    w.place_hero_at(x, y);
                }
                w
            }
        };
        // Clé du niveau courant (save v3) : cohérente avec le bloc `levels`
        // du manifest, y compris après un transfert de héros par warp.
        world.set_level_key(&def.key);
        // Monde neuf (boot sans transfert) avec point d'entrée demandé :
        // le respawn y est posé (l'auto-spawn centre reste la position initiale).
        if let Some((x, y)) = spawn {
            world.set_hero_spawn(x.floor() as i32, y.floor() as i32);
        }
        // Difficulté courante (W08/K) : monde neuf OU transféré (la save
        // porte déjà le champ ; remettre ici garde monde↔core synchrones).
        world.set_difficulty(self.difficulty);
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
            let display = self.level_display_name(&def);
            return Ok(LevelSwitchInfo { key: def.key, name: display });
        }
        self.load_level(&def, Some((spawn_x, spawn_y)))?;
        // `level_name` suit le niveau réel (levelInfo()/save cohérents après
        // un switch — le HUD page reçoit déjà l'événement LevelSwitch).
        self.level_name = def.key.clone();
        self.warp_cd_ms = WARP_COOLDOWN_MS;
        // W18 : nom fr résolu depuis les TBL réelles (repli manifest).
        let display = self.level_display_name(&def);
        Ok(LevelSwitchInfo { key: def.key, name: display })
    }

    /// Charge et parse les 4 tables monstre (`monsters.rs`) — `None` si un
    /// asset manque ou est illisible (repli historique, note de visibilité).
    fn load_monster_tables(&mut self) -> Option<MonsterTables> {
        let read = |core: &Self, path: &str| -> Option<String> {
            String::from_utf8(core.asset(path).ok()?.clone()).ok()
        };
        let (Some(monstats), Some(monumod), Some(superuniques), Some(monlvl)) = (
            read(self, MONSTATS_ASSET),
            read(self, MONUMOD_ASSET),
            read(self, SUPERUNIQUES_ASSET),
            read(self, MONLVL_ASSET),
        ) else {
            return None; // assets absents : comportement historique, pas une note
        };
        match (
            MonStats::parse(&monstats),
            MonUMods::parse(&monumod),
            SuperUniques::parse(&superuniques),
            MonLvl::parse(&monlvl),
        ) {
            (Ok(monstats), Ok(monumod), Ok(superuniques), Ok(monlvl)) => {
                self.switch_notes.push(format!(
                    "tables monstre branchées (W06-W08) : {} monstats, {} monumod, {} super-uniques, {} MonLvl",
                    monstats.len(),
                    monumod.mods.len(),
                    superuniques.len(),
                    monlvl.len()
                ));
                Some(MonsterTables { monstats, monumod, superuniques, monlvl })
            }
            _ => {
                self.switch_notes
                    .push("tables monstre illisibles — spawn historique (codes manifest)".into());
                None
            }
        }
    }

    /// Un spot de spawn valide pour un monstre : tuile marchable, à distance
    /// du héros, de la zone warp et des monstres déjà posés (relâchement
    /// progressif des contraintes sur les cartes denses).
    fn find_spawn_spot(
        rng: &mut LevelRng,
        world: &World,
        hero: (f32, f32),
        warp: Option<&(f32, f32)>,
        clear_warp: f32,
        spots: &[(f32, f32)],
        clear_hero: f32,
        spacing: f32,
    ) -> Option<(i32, i32)> {
        let (w, h) = (world.width().max(1) as u32, world.height().max(1) as u32);
        for relax in 0u32..4 {
            let ch = clear_hero * (1.0 - 0.25 * relax as f32);
            let sp = spacing * (1.0 - 0.25 * relax as f32);
            let cw = clear_warp * (1.0 - 0.25 * relax as f32);
            for _ in 0..200 {
                let (tx, ty) = (rng.below(w) as i32, rng.below(h) as i32);
                if !world.is_walkable(tx, ty) {
                    continue;
                }
                let (cx, cy) = (tx as f32 + 0.5, ty as f32 + 0.5);
                if (cx - hero.0).hypot(cy - hero.1) < ch {
                    continue;
                }
                if let Some(wp) = warp {
                    if (cx - wp.0).hypot(cy - wp.1) < cw {
                        continue;
                    }
                }
                if spots.iter().any(|(sx, sy)| (cx - sx).hypot(cy - sy) < sp) {
                    continue;
                }
                return Some((tx, ty));
            }
        }
        None
    }

    /// Construit le plan data-driven du niveau chargé (`monsters.rs`) —
    /// `None` si les tables sont absentes, le record levels.txt manque, ou la
    /// planche/densité est vide (l'appelant garde son chemin historique).
    fn data_driven_plan(&self) -> Option<(LevelSpawnPlan, LevelRng)> {
        let tables = self.mon_tables.as_ref()?;
        let def = self.level.as_ref()?;
        let id = def.levels_id?;
        let rec = self.tables.as_ref()?.get(id)?;
        if rec.mon.is_empty() || rec.mon_den == 0 {
            return None;
        }
        // Codes sprites disponibles : les sets construits au finalize
        // (`monsters` du manifest + `bosses`) — une entrée de planche sans
        // sprite est ignorée ; aucune → repli historique.
        let resolve_code = |code: &str| -> Option<String> {
            let up = code.to_ascii_uppercase();
            if self.sprites.contains_key(&up) {
                Some(up)
            } else {
                None
            }
        };
        let mut rng = LevelRng::new(fnv1a(&format!("mon:{}", def.key)));
        let plan = plan_level_spawns_diff(
            rec,
            &tables.monstats,
            &tables.monumod,
            &tables.monlvl,
            &tables.superuniques,
            self.difficulty,
            &mut rng,
            resolve_code,
        );
        if plan.packs.is_empty() && plan.super_uniques.is_empty() {
            return None;
        }
        Some((plan, rng))
    }

    /// Spawn les monstres du niveau chargé :
    /// * **data-driven (W06-W08)** — tables branchées + planche `mon1-10` du
    ///   record levels.txt : packs `MinGrp`/`MaxGrp` à la densité `MonDen`,
    ///   champions (20 % monumod) / uniques garantis (`MonUMin`/`MonUMax`),
    ///   super-uniques nommés de la planche, facteurs `MonLvl` réels ;
    /// * **repli historique** — la liste `monsters` du bloc (FA/ZM/SI par
    ///   défaut), town = zone sûre ; même comportement qu'avant ce cycle.
    /// Le boss de quête du niveau (`boss` du bloc, W11) se pose en dernier,
    /// loin du héros.
    fn spawn_level_monsters(&mut self) {
        let mut codes: Vec<String> = Vec::new();
        let mut kinds: Vec<MonKind> = Vec::new();
        let mut names: Vec<String> = Vec::new();
        let mut spots: Vec<(f32, f32)> = Vec::new();
        let plan = self.data_driven_plan();
        let level_key = self.level.as_ref().map(|l| l.key.clone()).unwrap_or_default();
        let warp_pt = self.level.as_ref().and_then(|l| l.warp.as_ref().map(|w| w.at));

        if let Some((plan, mut rng)) = plan {
            let world = self.world.as_mut().expect("world avant spawn");
            world.clear_monsters();
            let hero = *world.hero();
            let (hx, hy) = (hero.x, hero.y);
            let mut place_pack_member = |world: &mut World,
                                         rng: &mut LevelRng,
                                         code: &str,
                                         rule: SpawnRule,
                                         spots: &mut Vec<(f32, f32)>,
                                         codes: &mut Vec<String>,
                                         kinds: &mut Vec<MonKind>,
                                         names: &mut Vec<String>,
                                         name: &str| {
                let Some((tx, ty)) = Self::find_spawn_spot(
                    rng, world, (hx, hy), warp_pt.as_ref(), MONSTER_SPAWN_CLEAR_OF_WARP, spots,
                    MONSTER_SPAWN_CLEAR_OF_HERO, MONSTER_SPAWN_SPACING,
                ) else {
                    return; // carte saturée : pack tronqué (comme le repli)
                };
                let rule_kind = rule.kind;
                if world.spawn_monster_rule(code, tx, ty, rule) {
                    let (cx, cy) = (tx as f32 + 0.5, ty as f32 + 0.5);
                    spots.push((cx, cy));
                    codes.push(code.to_string());
                    kinds.push(rule_kind);
                    names.push(name.to_string());
                }
            };
            let packs = plan.packs.clone();
            for pack in &packs {
                let minions = pack.count.saturating_sub(1);
                place_pack_member(
                    world, &mut rng, &pack.code, pack.rule.clone(), &mut spots, &mut codes,
                    &mut kinds, &mut names, "",
                );
                // Minions du pack : mêmes facteurs MonLvl, boost HP monumod.
                let mut mrule = pack.rule.clone();
                mrule.kind = MonKind::Normal;
                if pack.minion_hp_pct > 0 {
                    mrule.hp_pct = mrule.hp_pct.saturating_mul(100 + pack.minion_hp_pct) / 100;
                }
                for _ in 0..minions {
                    place_pack_member(
                        world, &mut rng, &pack.code, mrule.clone(), &mut spots, &mut codes,
                        &mut kinds, &mut names, "",
                    );
                }
            }
            let sus = plan.super_uniques.clone();
            for su in &sus {
                let minions = su.count.saturating_sub(1);
                place_pack_member(
                    world, &mut rng, &su.code, su.rule.clone(), &mut spots, &mut codes,
                    &mut kinds, &mut names, &su.name,
                );
                let mut mrule = su.rule.clone();
                mrule.kind = MonKind::Normal;
                mrule.hp_pct = mrule.hp_pct.saturating_mul(100 + su.minion_hp_pct) / 100;
                mrule.treasure_class = None; // minions : TC normale du gabarit
                for _ in 0..minions {
                    place_pack_member(
                        world, &mut rng, &su.code, mrule.clone(), &mut spots, &mut codes,
                        &mut kinds, &mut names, "",
                    );
                }
            }
            self.switch_notes.push(format!(
                "spawn data-driven (W06-W08) : {} packs + {} super-uniques, cible {} (MonDen), difficulté {}",
                packs.len(),
                sus.len(),
                packs.iter().map(|p| p.count).sum::<u32>()
                    + sus.iter().map(|s| s.count).sum::<u32>(),
                difficulty_label(self.difficulty)
            ));
        } else {
            let list = self
                .level
                .as_ref()
                .map(|l| l.monsters.clone())
                .unwrap_or_default();
            if !list.is_empty() {
                // Packs MinGrp/MaxGrp PARALLÈLES à la planche (socle W01/A3)
                // — (1, 1) pour une liste `monsters` du manifest :
                // comportement historique inchangé (1 monstre par entrée).
                let packs: Vec<(u32, u32)> = self
                    .level
                    .as_ref()
                    .map(|l| {
                        (0..l.monsters.len())
                            .map(|i| l.plate_packs.get(i).copied().unwrap_or((1, 1)))
                            .collect()
                    })
                    .unwrap_or_default();
                let world = self.world.as_mut().expect("world avant spawn");
                world.clear_monsters();
                let hero = *world.hero();
                let (hx, hy) = (hero.x, hero.y);
                let seed = fnv1a(&level_key);
                let mut rng = LevelRng::new(seed);
                for (idx, code) in list.iter().enumerate() {
                    let (lo, hi) = packs.get(idx).copied().unwrap_or((1, 1));
                    let roll = rng.below((hi.saturating_sub(lo) + 1).max(1));
                    for _ in 0..crate::spawn::pack_size(lo, hi, roll) {
                        if let Some((tx, ty)) = Self::find_spawn_spot(
                            &mut rng, world, (hx, hy), warp_pt.as_ref(),
                            MONSTER_SPAWN_CLEAR_OF_WARP, &spots, MONSTER_SPAWN_CLEAR_OF_HERO,
                            MONSTER_SPAWN_SPACING,
                        ) {
                            if world.spawn_monster(code, tx, ty) {
                                spots.push((tx as f32 + 0.5, ty as f32 + 0.5));
                                codes.push(code.clone());
                            }
                        }
                    }
                }
            } else if let Some(world) = self.world.as_mut() {
                // Zone sûre (town) : aucun monstre, même si le monde neuf en a
                // auto-spawné (comportement par défaut de d2rules).
                world.clear_monsters();
            }
            kinds = vec![MonKind::Normal; codes.len()];
            names = vec![String::new(); codes.len()];
        }

        // Boss de quête (W11) : `boss` du bloc niveau → spawn SEUL, loin du
        // héros, avec ses stats manifest réelles + TC dédiée + événement.
        if let Some(boss_code) = self.level.as_ref().and_then(|l| l.boss.clone()) {
            if let Some(world) = self.world.as_mut() {
                let hero = *world.hero();
                let (hx, hy) = (hero.x, hero.y);
                let mut rng = LevelRng::new(fnv1a(&format!("boss:{level_key}")));
                // Le boss s'éloigne davantage ( zones boss = arènes retail) :
                // on essaie des spots de plus en plus loin du héros.
                let mut placed = false;
                for min_dist in [40.0, 30.0, 20.0, MONSTER_SPAWN_CLEAR_OF_HERO] {
                    if let Some((tx, ty)) = Self::find_spawn_spot(
                        &mut rng, world, (hx, hy), warp_pt.as_ref(),
                        MONSTER_SPAWN_CLEAR_OF_WARP, &spots, min_dist, MONSTER_SPAWN_SPACING,
                    ) {
                        // Résists du boss POUR LA DIFFICULTÉ (W08/K) : record
                        // monstats du code sprite (`AN`→andariel), colonnes
                        // Res*(N)/(H) — défaut [0;6] si le code est inconnu
                        // de la table (boss manifest-only).
                        let boss_res = self
                            .mon_tables
                            .as_ref()
                            .and_then(|t| t.monstats.get_by_code(&boss_code))
                            .map(|r| r.res_for(self.difficulty))
                            .unwrap_or([0; 6]);
                        if world.spawn_monster_rule(&boss_code, tx, ty, SpawnRule {
                            kind: MonKind::Boss,
                            res: boss_res,
                            ..SpawnRule::default()
                        }) {
                            spots.push((tx as f32 + 0.5, ty as f32 + 0.5));
                            codes.push(boss_code.clone());
                            kinds.push(MonKind::Boss);
                            names.push(self.boss_display_name(&boss_code));
                            placed = true;
                            break;
                        }
                    }
                }
                if !placed {
                    self.switch_notes
                        .push(format!("boss {boss_code} sans spot valide — absent du niveau"));
                }
            }
        }
        self.mon_codes = codes;
        self.mon_kinds = kinds;
        self.mon_names = names;
    }

    /// Nom affichable d'un boss (`manifest.bosses[].name`), "" sinon.
    fn boss_display_name(&self, code: &str) -> String {
        self.manifest
            .get("bosses")
            .and_then(|b| b.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find(|b| b.get("code").and_then(|c| c.as_str()) == Some(code))
                    .and_then(|b| b.get("name"))
                    .and_then(|n| n.as_str())
            })
            .unwrap_or("")
            .to_string()
    }

    fn build_sprite_set(&self, walk: &Value) -> Result<SpriteSet, String> {
        let cof_path = walk
            .get("cof")
            .and_then(|c| c.as_str())
            .ok_or("manifest: walk.cof missing")?;
        let cof = Cof::parse(self.asset(cof_path)?).map_err(|e| format!("{cof_path}: {e:?}"))?;
        let mut set = SpriteSet::new(cof);
        // AnimData.d2 : le token de lookup est le stem du COF (ex.
        // `assets/dcc/BAWL1HS.COF` → `BAWL1HS`) ; record absent du fichier
        // (ex. BARNUHTH) → pas de branchement, octets COF conservés.
        if let Some(ad) = &self.animdata {
            let base = cof_path.rsplit('/').next().unwrap_or(cof_path);
            let stem = base.split('.').next().unwrap_or(base).to_ascii_uppercase();
            if let Some(rec) = ad.get_record(&stem) {
                set.apply_animdata(rec);
            }
        }
        if let Some(layers) = walk.get("layers").and_then(|l| l.as_object()) {
            for (token, path) in layers {
                let path = path.as_str().ok_or("manifest: layer path not a string")?;
                // 15a : couches DC6 (Mephisto — ses composantes sont en
                // FORMAT DC6 dans le retail, aucun DCC n'existe) sautées
                // gracieusement : SpriteSet/compose est DCC-only, le
                // branchement d2dc6 est hors scope. Sans ce filtre le set
                // entier est rejeté (BadSignature) et le monstre perd même
                // son COF.
                if path.ends_with(".dc6") {
                    continue;
                }
                let dcc = Dcc::decode(self.asset(path)?).map_err(|e| format!("{path}: {e:?}"))?;
                set.add_layer(token, dcc);
            }
        }
        // FIX-HEROS : la couche LG (jambes) n'est pas déclarée dans les blocs
        // manifest (régénérations sans /LG) — convention de fichier D2 :
        // <code><mode>LG.dcc à côté du COF (stem « BAWLHTH » → code BA, mode
        // WL). Attachée au set si le fichier existe ; no-op pour les
        // monstres (aucun *LG.dcc de monstre dans les MPQ v1.12).
        {
            let base = cof_path.rsplit('/').next().unwrap_or(cof_path);
            let stem: Vec<char> = base.split('.').next().unwrap_or(base).chars().collect();
            if stem.len() >= 4 {
                let lg_path = format!("assets/dcc/{}{}LG.dcc", &stem[..2].iter().collect::<String>(), &stem[2..4].iter().collect::<String>());
                if let Ok(bytes) = self.asset(&lg_path) {
                    if let Ok(dcc) = Dcc::decode(&bytes) {
                        set.add_layer("LG", dcc);
                    }
                }
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

        // 2. Modes additionnels. `modes` = {NU|WL|RN|A1|DD|SC|BL|GH|S1|S2|S3|
        //    S4|KK|TH|A2: {cof, layers}} (étape 15b : tags additifs) ;
        //    à défaut blocs nommés idle→NU, attack→A1, death→DD.
        let mut mode_of: Vec<(&str, &Value)> = Vec::new();
        if let Some(modes) = def.get("modes").and_then(|m| m.as_object()) {
            for tag in [
                "NU", "WL", "RN", "A1", "DD", "SC", "BL", "GH", "S1", "S2", "S3", "S4",
                "KK", "TH", "A2",
            ] {
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
            tall: s.tall,
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
    /// Manifest brut (lecture) pour les blocs `impl GameCore` additifs qui
    /// composent du JSON d'UI depuis le gabarit (18b `panels_ui.rs` :
    /// vitalité/énergie/arme de départ). Lecture seule, aucun coût.
    pub(crate) fn manifest_value(&self) -> &Value {
        &self.manifest
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
                    AnimState::SC => "SC",
                    AnimState::BL => "BL",
                    AnimState::GH => "GH",
                    AnimState::S1 => "S1",
                    AnimState::S2 => "S2",
                    AnimState::S3 => "S3",
                    AnimState::S4 => "S4",
                    AnimState::KK => "KK",
                    AnimState::TH => "TH",
                    AnimState::A2 => "A2",
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

    /// Set de sprites construit pour une clé `"<entity>"` / `"<entity>:MODE"`
    /// (15a : vérification golden que finalize a bien porté tous les blocs
    /// monstres/bosses du manifest). Lecture seule, aucun coût.
    pub fn sprite_set(&self, key: &str) -> Option<&SpriteSet> {
        self.sprites.get(key)
    }

    /// Clés de tous les sets construits (diagnostic golden/tests).
    pub fn sprite_set_keys(&self) -> Vec<String> {
        self.sprites.keys().cloned().collect()
    }

    /// Niveau chargé — JSON `{"key":"bloodmoor","name":"Blood Moor (Acte I)"}`
    /// pour le HUD (nom de zone, bannière de changement). Résolu même avant
    /// `finalize` (niveau initial demandé) ; `{}` si la clé est inconnue.
    /// Tables branchées (W04) : ajoute `"neighbors"` — les voisins Vis/Warp
    /// du niveau courant, `[{id, name}, …]` (append-only, clients existants
    /// compatibles).
    pub fn level_info_json(&self) -> String {
        let info = match self.level.as_ref() {
            // W18 : nom fr (TBL) — même résolution que la bannière LevelSwitch.
            Some(l) => Some((l.key.clone(), self.level_display_name(l), l.levels_id)),
            None => self
                .resolve_level_def(&self.level_name)
                .ok()
                .map(|d| {
                    let name = self.level_display_name(&d);
                    (d.key, name, d.levels_id)
                }),
        };
        match info {
            Some((key, name, levels_id)) => {
                let neighbors: Vec<Value> = match (&self.warp_graph, levels_id, &self.tables) {
                    (Some(g), Some(id), Some(t)) => g
                        .edges_of(id)
                        .iter()
                        .map(|e| {
                            let n = t.get(e.to);
                            serde_json::json!({
                                "id": e.to,
                                // W18 : nom fr du voisin (clé LevelName → TBL,
                                // repli nom technique puis "").
                                "name": n.map(|r| {
                                    let base = if r.level_name.is_empty() { r.name.as_str() } else { r.level_name.as_str() };
                                    crate::tbl_names::translate(base, base)
                                }).unwrap_or_default(),
                            })
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                let mut v = serde_json::json!({ "key": key, "name": name });
                if !neighbors.is_empty() {
                    v["neighbors"] = Value::Array(neighbors);
                }
                v.to_string()
            }
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

    // ------------------------------------------------------------------
    // EXP-H* (HUD parité, docs/parity/impl-hud-parite.md) — API additive
    // consommée par demo/hud.js. Append-only : aucun appelant existant modifié.
    // ------------------------------------------------------------------

    /// Compétences connues du héros (EXP-H4, boutons L/R) — JSON
    /// `[{"index":0,"code":"Fire Bolt","name":"Fire Bolt","level":1,
    /// "missile":"firebolt"},…]` ; `[]` avant `finalize` ou sans skill.
    pub fn hero_skills_json(&self) -> String {
        match self.world.as_ref() {
            Some(w) => serde_json::to_string(
                &w.hero_skills()
                    .iter()
                    .enumerate()
                    .map(|(index, s)| {
                        serde_json::json!({
                            "index": index,
                            "code": s.code,
                            "name": s.code, // libellé table = code au MVP (EXP-T1 à venir)
                            "level": s.level,
                            "missile": s.missile,
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .unwrap_or_else(|_| "[]".into()),
            None => "[]".into(),
        }
    }

    /// Sélectionne la compétence `index` (EXP-H4) : la main droite D2 — celle
    /// que l'input `RangedAttack` consomme via [`d2rules::World::select_skill`].
    /// `false` si hors bornes (état inchangé).
    pub fn select_skill(&mut self, index: usize) -> bool {
        match self.world.as_mut() {
            Some(w) => w.select_skill(index),
            None => false,
        }
    }

    // ------------------------------------------------------------------
    // ARBRE DE COMPÉTENCES (C2/C3/C4/C5 — mission arbre, 2026-10-23) :
    // API additive consommée par demo/skills-ui.js (window.D2Skills).
    // Append-only : aucun appelant existant modifié (game.js évolue en
    // parallèle côté intégration — branchement minimal commenté).
    // ------------------------------------------------------------------

    /// Arbre complet de la classe du héros — JSON
    /// `{"class":"bar","hero":"BA","points":2,"charLevel":5,"skills":[{…
    /// code,name,nameKey,level,maxLevel,reqLevel,prereqs,page,row,column,
    /// icon,missile,mana,canLearn,mvp},…]}` — 30 skills/classe (tables réelles
    /// `patch_skills.txt` + `patch_skilldesc.txt` embarquées), trié par
    /// onglet (`SkillPage`) puis géométrie d'arbre (`SkillRow`/`SkillColumn`).
    /// `level` = points du héros ; `canLearn` = validation complète
    /// (prérequis/reqlevel/plafond/points) ; `icon` = `IconCel` (feuille
    /// générique `skillicon.dc6` 24 cellules — cellules ≥ 24 = sheets par
    /// classe, hors portée MVP : l'UI affiche un placeholder).
    pub fn skill_tree_json(&self) -> String {
        let data = d2rules::skills_tree::SkillTreeData::global();
        let hero_code = d2rules::resolve_hero_code(&self.manifest, &self.hero_request);
        let class = d2rules::skills_hero::class_code(&hero_code);
        let (mut tree, points, char_level) = match self.world.as_ref() {
            Some(w) => {
                let mut tree = d2rules::skills_tree::SkillTree::with_data(&class, data);
                tree.set_char_level(w.hero_stats().level);
                for s in w.hero_skills() {
                    tree.set_level(&s.code, s.level);
                }
                tree.grant_points(w.unspent_skill_points());
                (tree, w.unspent_skill_points(), w.hero_stats().level)
            }
            None => {
                (d2rules::skills_tree::SkillTree::with_data(&class, data), 0, 1)
            }
        };
        let mut rows: Vec<Value> = data
            .class_indices(&class)
            .iter()
            .filter_map(|&i| {
                let rec = &data.records()[i];
                let desc = data.desc_of(&rec.code);
                Some(serde_json::json!({
                    "code": rec.code,
                    "name": rec.code, // libellé table = code (TBL fr = W18)
                    "nameKey": desc.map(|d| d.str_name.clone()).unwrap_or_default(),
                    "level": tree.level_of(&rec.code),
                    "maxLevel": rec.max_level,
                    "reqLevel": rec.req_level,
                    "prereqs": rec.req_skills,
                    "page": desc.map(|d| d.page).unwrap_or(0),
                    "row": desc.map(|d| d.row).unwrap_or(0),
                    "column": desc.map(|d| d.column).unwrap_or(0),
                    "icon": desc.map(|d| d.icon_cel).unwrap_or(0),
                    "missile": rec.srv_missile,
                    "mana": tree.mana_cost(&rec.code),
                    "canLearn": tree.can_learn(&rec.code).is_ok(),
                    "mvp": data.is_mvp(&rec.code),
                }))
            })
            .collect();
        rows.sort_by_key(|r| {
            (r["page"].as_u64().unwrap_or(0), r["row"].as_u64().unwrap_or(0),
             r["column"].as_u64().unwrap_or(0))
        });
        serde_json::to_string(&serde_json::json!({
            "class": class,
            "hero": hero_code,
            "points": points,
            "charLevel": char_level,
            "skills": rows,
        }))
        .unwrap_or_else(|_| "{}".into())
    }

    /// Apprend (ou monte d'un niveau) le skill `code` en dépensant 1 point de
    /// skill du héros — validation RÉELLE d2rules (classe, prérequis
    /// `reqskill1-3`, `reqlevel`, plafond `maxlvl`) via
    /// [`d2rules::World::learn_skill`]. JSON
    /// `{"ok":true,"level":2,"points":1}` ou `{"ok":false,"error":"…"}`.
    pub fn learn_skill(&mut self, code: &str) -> String {
        let Some(w) = self.world.as_mut() else {
            return serde_json::json!({"ok": false, "error": "monde non initialisé"}).to_string();
        };
        match w.learn_skill(code) {
            Ok(level) => serde_json::json!({
                "ok": true,
                "level": level,
                "points": w.unspent_skill_points(),
            })
            .to_string(),
            Err(e) => serde_json::json!({"ok": false, "error": e.to_string()}).to_string(),
        }
    }

    /// Tables de niveaux branchées (W01) si l'asset `patch_levels.txt` a été
    /// fourni — exposé à la couche noms fr (`tbl_names.rs`, W18) qui préfère
    /// la clé `LevelName` réelle au repli embarqué.
    pub(crate) fn level_tables(&self) -> Option<&levels::LevelTables> {
        self.tables.as_ref()
    }

    /// Nom affichable du monstre `index` : nom d2game (super-unique/boss W10),
    /// sinon `monsters`/`bosses` du manifest par code, sinon le code brut.
    /// (`pub(crate)` W18 : la couche fr `tbl_names.rs` traduit ce résultat
    /// via les TBL — `monster_display_name_tbl`.)
    pub(crate) fn monster_display_name(&self, index: usize) -> String {
        if let Some(n) = self.mon_names.get(index) {
            if !n.is_empty() {
                return n.clone();
            }
        }
        if let Some(code) = self.mon_codes.get(index) {
            for key in ["monsters", "bosses"] {
                if let Some(arr) = self.manifest.get(key).and_then(|v| v.as_array()) {
                    if let Some(m) = arr.iter().find(|m| {
                        m.get("code").and_then(|c| c.as_str()) == Some(code.as_str())
                    }) {
                        if let Some(n) = m.get("name").and_then(|n| n.as_str()) {
                            return n.to_string();
                        }
                    }
                }
            }
            return code.clone();
        }
        "?".into()
    }

    /// Projette un point monde (sous-tuiles) en px viewport — miroir de
    /// [`GameCore::hero_screen`] pour n'importe quel point (labels, survol).
    fn world_to_screen(&self, x: f32, y: f32) -> (f32, f32) {
        match self.map.as_ref() {
            Some(map) => {
                let (sx, sy) = map.sub_to_screen(x, y);
                (sx - self.cam_x, sy - self.cam_y)
            }
            None => (x, y),
        }
    }

    /// Fiche de survol du monstre `index` (EXP-H6) — JSON
    /// `{"code","name","kind","alive","hp","maxHp","x","y","sx","sy"}`
    /// (`x/y` monde sous-tuiles, `sx/sy` px viewport, pieds de l'entité) ;
    /// `"null"` si hors bornes. Pollé chaque frame par hud.js pendant le survol.
    pub fn monster_info_json(&self, index: usize) -> String {
        let Some(w) = self.world.as_ref() else { return "null".into() };
        let Some(e) = w.monsters().get(index) else { return "null".into() };
        let (sx, sy) = self.world_to_screen(e.x, e.y);
        let (hp, max_hp) = w.monster_hp(index).unwrap_or((0, 0));
        let info = serde_json::json!({
            "code": self.mon_codes.get(index).cloned().unwrap_or_default(),
            // W18 : nom fr servi depuis les TBL réelles (repli historique).
            "name": self.monster_display_name_tbl(index),
            "kind": w.monster_kind(index).map(|k| k.as_str()).unwrap_or("normal"),
            "alive": w.monster_alive(index),
            "hp": hp,
            "maxHp": max_hp,
            "x": e.x,
            "y": e.y,
            "sx": sx,
            "sy": sy,
        });
        info.to_string()
    }

    /// Fiche du mercenaire (H, d2rules::mercenary) — JSON
    /// `{kind,subtype,level,hp,maxHp,alive,x,y,sx,sy}` ou `null` (aucun
    /// mercenaire). Endpoint additif consommé par demo/merc-ui.js.
    pub fn merc_info_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "null".into() };
        let Some(m) = w.mercenary() else { return "null".into() };
        let (sx, sy) = self.world_to_screen(m.ent.x, m.ent.y);
        serde_json::json!({
            "kind": m.kind.as_str(),
            "subtype": m.subtype,
            "level": m.level,
            "hp": m.hp,
            "maxHp": m.max_hp,
            "alive": m.alive,
            "x": m.ent.x,
            "y": m.ent.y,
            "sx": sx,
            "sy": sy,
        })
        .to_string()
    }

    /// Objets au sol avec projection écran (EXP-H8, labels Alt) — JSON
    /// `[{"index","code","name","kind","amount","x","y","sx","sy"},…]`.
    /// `[]` avant `finalize`. Pollé chaque frame quand Alt est maintenu.
    pub fn ground_items_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "[]".into() };
        let items: Vec<Value> = w
            .items()
            .iter()
            .enumerate()
            .map(|(index, it)| {
                let (sx, sy) = self.world_to_screen(it.x, it.y);
                serde_json::json!({
                    "index": index,
                    "code": it.def.code,
                    "name": it.def.name,
                    "kind": it.def.kind,
                    "amount": it.def.amount,
                    "x": it.x,
                    "y": it.y,
                    "sx": sx,
                    "sy": sy,
                })
            })
            .collect();
        serde_json::to_string(&items).unwrap_or_else(|_| "[]".into())
    }

    /// Corps au sol (J1) avec projection écran — JSON
    /// `{"x","y","sx","sy","items"}` ou `null` (aucun corps). `items` =
    /// nombre d'objets encore portés par le corps. Pollé par la page pour
    /// le flare du corps + le hint, et pour le clic « récupérer ».
    pub fn corpse_info_json(&self) -> String {
        let Some(w) = self.world.as_ref() else { return "null".into() };
        let Some(c) = w.corpse() else { return "null".into() };
        let (sx, sy) = self.world_to_screen(c.x, c.y);
        let info = serde_json::json!({
            "x": c.x,
            "y": c.y,
            "sx": sx,
            "sy": sy,
            "items": c.inv.iter().filter(|s| s.is_some()).count(),
        });
        serde_json::to_string(&info).unwrap_or_else(|_| "null".into())
    }

/// Serialize the world (d2rules save format, JSON v3 : grille + manifest +
    /// héros + monstres + objets + missiles + skills + clé du niveau courant)
    /// for the demo's Save button. Empty string when not finalized.
    pub fn save_world(&self) -> String {
        self.world.as_ref().map(|w| w.save()).unwrap_or_default()
    }

    /// Replace the world with one deserialized from `json` (d2rules save v1..v3).
    /// Si la sauvegarde porte une clé de niveau différente du niveau courant
    /// (save v3, warps multi-niveaux Town<->Blood Moor), le niveau cible est
    /// rechargé d'abord (même graine ⇒ même carte) et le héros est posé à sa
    /// position sauvegardée — la reprise "Continuer" retombe dans la bonne
    /// zone. Map/sprites suivent la sauvegarde.
    pub fn load_world(&mut self, json: &str) -> Result<(), String> {
        // Clé de niveau du save (v3 ; absent v1/v2 ⇒ "town", même défaut que
        // d2rules::save) + position héros pour le snap d'entrée.
        let (save_level, hero_at): (String, Option<(f32, f32)>) =
            serde_json::from_str::<Value>(json)
                .map(|v| {
                    let lvl = v
                        .get("level")
                        .and_then(|x| x.as_str())
                        .map(|s| s.trim().to_ascii_lowercase())
                        .filter(|s| !s.is_empty())
                        .unwrap_or_else(|| "town".to_string());
                    let pos = || -> Option<(f32, f32)> {
                        Some((
                            v.pointer("/hero/ent/x")?.as_f64()? as f32,
                            v.pointer("/hero/ent/y")?.as_f64()? as f32,
                        ))
                    }();
                    (lvl, pos)
                })
                .map_err(|e| format!("sauvegarde illisible : {e}"))?;
        // Sauvegarde d'une autre zone : on y va d'abord (assets déjà fournis
        // au boot pour tous les `levels`) — échec ⇒ erreur carte ci-dessous.
        if save_level != self.level_name {
            let spawn = hero_at.unwrap_or((0.0, 0.0));
            // Err ignoré volontairement : le contrôle de grille ci-dessous
            // reste l'arbitre (niveau inconnu/assets manquants).
            let _ = self.switch_level(&save_level, spawn.0, spawn.1);
        }
        let world = World::load(json).map_err(|()| "sauvegarde invalide ou version inconnue".to_string())?;
        // Sanity: the save's grid must match this map's (same level).
        if world.width() as usize != self.grid_size().0 as usize
            || world.height() as usize != self.grid_size().1 as usize
        {
            return Err("sauvegarde d'une autre carte".into());
        }
        self.level_name = world.level_key().to_string();
        self.world = Some(world);
        // W05 : ré-attache le waypoint du niveau courant (le save ne le
        // porte pas — il est dérivé du niveau chargé, cf. waypoints.rs).
        if let Some(def) = self.level.clone() {
            if let Some(w) = self.world.as_mut() {
                crate::waypoint_travel::attach_level_waypoint(w, &def);
                // W16 : ré-attache le roster NPC (idempotent — le save le
                // porte ; un save antérieur re-spawn le roster du camp).
                crate::npc_bridge::attach_town_npcs(w, &def);
            }
        }
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
        // l'index au moment du retrait) : miroir exact sur mon_codes,
        // mon_kinds et mon_names + événement de quête si le code en porte un
        // (`manifest.bosses[].questEvent`, ex. andariel → "andariel_dead").
        let mut quest_events: Vec<CoreEvent> = Vec::new();
        for e in &events {
            if let CoreEvent::World(GameEvent::MonsterDied { index, .. }) = e {
                let i = *index;
                let code = self.mon_codes.get(i).cloned();
                if let Some(id) = code.as_ref().and_then(|c| self.quest_events.get(c)) {
                    let id = id.clone();
                    quest_events.push(CoreEvent::QuestEvent { id });
                }
                if i < self.mon_codes.len() {
                    self.mon_codes.remove(i);
                }
                if i < self.mon_kinds.len() {
                    self.mon_kinds.remove(i);
                }
                if i < self.mon_names.len() {
                    self.mon_names.remove(i);
                }
            }
        }
        events.extend(quest_events);

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
                    self.blit_entity(&mut fb, vw, vh, "hero", &h, None);
                }
                DrawKind::Monster(i) => {
                    let m = world.monsters()[i];
                    let key = self.monster_sprite_key(i);
                    let tint = monster_tint(self.mon_kinds.get(i).copied().unwrap_or_default());
                    self.blit_entity(&mut fb, vw, vh, &key, &m, tint);
                }
                DrawKind::Item(i) => {
                    let (sx, sy) = self.entity_screen(DrawKind::Item(i));
                    item_marker(&mut fb, vw, vh, sx, sy);
                }
            }
        }

        // 3. tall walls: the second index buffer blitted OVER the entities
        // (occlusion — a hero walking behind a tree/wall is masked by it;
        // simplified Go renderer.go:231-358 passes 2/3: entities behind,
        // then tall walls; entities in front keep only the y-sort).
        blit_index_layer(&mut fb, vw, vh, self.cam_x, self.cam_y, &map.overlay, map.width, &lut);

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

    fn blit_entity(
        &self,
        fb: &mut [u8],
        vw: usize,
        vh: usize,
        key: &str,
        e: &Entity,
        tint: Option<[f32; 3]>,
    ) {
        // Set par mode "<key>:<ANIM>" en priorité, repli "<key>" (la marche).
        // Étape 15b : tags additifs des modes COF extraits (manifest modes) ;
        // un tag sans set dédié retombe sur la marche (fallback gracieux).
        let tag = match e.anim {
            AnimState::NU => "NU",
            AnimState::WL => "WL",
            AnimState::RN => "RN",
            AnimState::A1 => "A1",
            AnimState::DD => "DD",
            AnimState::SC => "SC",
            AnimState::BL => "BL",
            AnimState::GH => "GH",
            AnimState::S1 => "S1",
            AnimState::S2 => "S2",
            AnimState::S3 => "S3",
            AnimState::S4 => "S4",
            AnimState::KK => "KK",
            AnimState::TH => "TH",
            AnimState::A2 => "A2",
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
        blit_sprite(fb, vw, vh, &sf, wx - self.cam_x, wy - self.cam_y, tint);
    }
}

/// Teinte RGB d'un monstre selon son rang data-driven (W08) — simplification
/// des transforms palette D2 : champion bleu, unique/super-unique doré, boss
/// de quête rouge ; `None` = couleur d'origine.
fn monster_tint(kind: MonKind) -> Option<[f32; 3]> {
    match kind {
        MonKind::Normal => None,
        MonKind::Champion => Some([0.45, 0.55, 1.0]),
        MonKind::Unique | MonKind::SuperUnique => Some([1.0, 0.85, 0.25]),
        MonKind::Boss => Some([1.0, 0.3, 0.25]),
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

/// Palette monstres par défaut d'un niveau extérieur — repli ULTIME quand
/// l'Id levels.txt est absent de la table (socle W01/A3 : la palette normale
/// vient des planches mon1-10 réelles via `spawn.rs` ; Blood Moor retail :
/// fallen1, zombie1, quillrat1 — MAP-SPEC-BLOODMOOR §6.5).
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
        let levels_id = block.get("levelsId").and_then(|v| v.as_u64()).map(|v| v as u32);
        // Socle W01/A3 : sans liste `monsters`, un niveau extérieur reçoit
        // la planche réelle mon1-10 du record levels.txt (Blood Moor Id=2 :
        // zombie1/fallen1/quillrat1 → ZM/FA/SI, MonDen 520) + les packs
        // MinGrp/MaxGrp de patch_monstats.txt (`spawn.rs`) ; repli
        // historique si l'Id est inconnu de la table.
        let outdoor = if key == "town" || block.get("monsters").is_some() {
            None
        } else {
            crate::spawn::outdoor_spawn(levels_id.unwrap_or(u32::MAX))
        };
        let monsters: Vec<String> = match block.get("monsters").and_then(|m| m.as_array()) {
            Some(a) => a.iter().filter_map(|v| v.as_str().map(|s| s.to_ascii_uppercase())).collect(),
            None => {
                if key == "town" {
                    Vec::new() // zone sûre
                } else {
                    outdoor.as_ref().map(|s| s.plate_codes()).unwrap_or_else(|| {
                        DEFAULT_OUTDOOR_MONSTERS.iter().map(|s| s.to_string()).collect()
                    })
                }
            }
        };
        let plate_packs: Vec<(u32, u32)> =
            outdoor.map(|s| s.plate_packs()).unwrap_or_default();
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
        let boss = block
            .get("boss")
            .and_then(|b| b.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_ascii_uppercase());
        Ok(LevelDef {
            key: key.to_string(),
            levels_id,
            name,
            ds1,
            dt1,
            palette,
            monsters,
            plate_packs,
            boss,
            warp,
        })
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

/// Adapte un save d2rules (JSON v1..v3) à un changement de niveau : grille
/// remplacée par celle du niveau cible, héros reposé au point d'entrée
/// (pleine vie, mort annulée), monstres et objets au sol vidés (le niveau
/// spawn les siens), clé de niveau réécrite vers la cible (save v3 — la
/// reprise "Continuer" retombe dans la bonne zone). Stats/inventaire/xp/rng
/// conservés tels quels — c'est le mécanisme de transfert du héros entre
/// niveaux (aucun setter d2rules requis, le format save est le contrat public).
fn patch_save_for_level(
    save_json: &str,
    blocked: &[Vec<bool>],
    spawn: Option<(f32, f32)>,
    level_key: &str,
) -> Result<World, String> {
    let mut v: Value = serde_json::from_str(save_json).map_err(|e| format!("save: {e}"))?;
    // v3 = format courant (clé de niveau) ; v2/v1 = rétro-compat (d2rules::save
    // charge les trois). Les champs patchés ci-dessous existent dans les trois
    // — seul le numéro de version fait barrière ici.
    let ver = v.get("version").and_then(|x| x.as_u64());
    if ver != Some(1) && ver != Some(2) && ver != Some(3) {
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
    // La sauvegarde suit le héros : le niveau patché devient LE niveau du save.
    v["level"] = serde_json::json!(level_key);
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
/// d2rules : le contenu d'un niveau ne dépend pas de l'historique de
/// combat. Le même générateur sert à la variation seedée des tuiles du
/// composeur ([`compose::Lcg`], graine distincte par usage).
use crate::compose::Lcg as LevelRng;

/// FNV-1a 64 — graine de spawn stable par clé de niveau.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Libellé HUD d'une difficulté (affichage, pas une donnée table).
fn difficulty_label(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Normal => "Normal",
        Difficulty::Nightmare => "Nightmare",
        Difficulty::Hell => "Hell",
    }
}

#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct ComposeStatsCopy {
    pub drawn: usize,
    pub hidden: usize,
    pub empty: usize,
    pub out_of_bounds: usize,
    /// Murs hauts routés vers le tampon overlay (occlusion).
    pub tall: usize,
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
        // 0. FIX-CAMPES : règle dossier AVANT le suffixe — le .tg1 porte le
        // dossier du tileset réel et deux conteneurs partagent le stem
        // "objects" (`town\objects.tg1` → town_objects.dt1 ;
        // `outdoors\objects.tg1` → out_objects.dt1). L'ancienne règle du
        // suffixe résolvait les deux sur town_objects : les tentures/props
        // du camp (out_objects) n'entraient jamais dans le pool.
        let segs: Vec<String> = norm
            .split('\\')
            .filter(|p| !p.is_empty())
            .map(|p| p.to_ascii_lowercase())
            .collect();
        let prefixes: &[&str] = if segs.iter().any(|s| s == "town") {
            &["town_"]
        } else if segs.iter().any(|s| s == "outdoors") {
            &["out_"]
        } else {
            &[]
        };
        for pfx in prefixes {
            if let Some(b) = basenames
                .iter()
                .find(|b| b.strip_suffix(".dt1").is_some_and(|s| s.strip_prefix(pfx) == Some(&stem)))
            {
                return b.clone();
            }
        }
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

/// Blit a palette-index map layer over the viewport through the LUT —
/// used for the tall-wall overlay (`ComposedMap::overlay`): index 0
/// (never painted) is transparent, so only wall pixels land on the frame.
#[allow(clippy::too_many_arguments)] // même forme que `shadow`/`blit_sprite`
fn blit_index_layer(
    fb: &mut [u8],
    vw: usize,
    vh: usize,
    cam_x: f32,
    cam_y: f32,
    src: &[u8],
    map_w: usize,
    lut: &[u8; 256 * 4],
) {
    let map_h = src.len() / map_w.max(1);
    for y in 0..vh {
        let my = y + cam_y as usize;
        if my >= map_h {
            continue; // above the map: transparent overlay
        }
        let x0 = cam_x as usize;
        let xs = x0..(x0 + vw).min(map_w);
        let row = &src[my * map_w..my * map_w + map_w];
        let dst = &mut fb[y * vw * 4..(y + 1) * vw * 4];
        let mut di = 0usize;
        for mx in xs.clone() {
            let v = row[mx];
            if v != 0 {
                let p = v as usize * 4;
                dst[di..di + 4].copy_from_slice(&lut[p..p + 4]);
            }
            di += 4;
        }
    }
}

/// Blit a composed sprite: bottom-center anchored on the world position.
fn blit_sprite(
    fb: &mut [u8],
    vw: usize,
    vh: usize,
    sf: &SpriteFrame,
    sx: f32,
    sy: f32,
    tint: Option<[f32; 3]>,
) {
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
            if let Some([r, g, b]) = tint {
                // Teinte type monumod (champion bleu / unique doré) :
                // multiplication RGB simple du sprite composé.
                let px = [
                    (sf.rgba[si] as f32 * r).round().clamp(0.0, 255.0) as u8,
                    (sf.rgba[si + 1] as f32 * g).round().clamp(0.0, 255.0) as u8,
                    (sf.rgba[si + 2] as f32 * b).round().clamp(0.0, 255.0) as u8,
                    a,
                ];
                fb[di..di + 4].copy_from_slice(&px);
            } else {
                fb[di..di + 4].copy_from_slice(&sf.rgba[si..si + 4]);
            }
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
    fn overlay_blit_covers_entities_only_where_walls_are() {
        // blit_index_layer : l'overlay n'écrase que ses pixels peints —
        // l'entité (pixel 200,200,200) reste visible hors du mur, masquée
        // sous le mur (pixel LUT de l'index 42).
        let (vw, vh) = (4usize, 2usize);
        let mut fb = vec![0u8; vw * vh * 4];
        let entity = |fb: &mut [u8], x: usize, y: usize| {
            let di = (y * vw + x) * 4;
            fb[di..di + 4].copy_from_slice(&[200, 200, 200, 255]);
        };
        entity(&mut fb, 1, 0);
        entity(&mut fb, 2, 0);
        // Carte 4×2 : mur (index 42) en (2,0), rien ailleurs.
        let src = vec![0, 0, 42, 0, 0, 0, 0, 0];
        let mut lut = [0u8; 256 * 4];
        lut[42 * 4..42 * 4 + 4].copy_from_slice(&[10, 20, 30, 255]);
        blit_index_layer(&mut fb, vw, vh, 0.0, 0.0, &src, 4, &lut);
        let px = |fb: &[u8], x: usize, y: usize| {
            let di = (y * vw + x) * 4;
            (fb[di], fb[di + 1], fb[di + 2])
        };
        assert_eq!(px(&fb, 1, 0), (200, 200, 200), "entité hors mur intacte");
        assert_eq!(px(&fb, 2, 0), (10, 20, 30), "entité sous le mur masquée");
        // Caméra décalée : la ligne sous la carte reste transparente.
        entity(&mut fb, 1, 1);
        blit_index_layer(&mut fb, vw, vh, 0.0, 2.0, &src, 4, &lut);
        assert_eq!(px(&fb, 1, 1), (200, 200, 200), "hors carte: rien de blitté");
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
        for mon in m.get("monsters").unwrap().as_array().unwrap().iter().chain(
            m.get("bosses").and_then(|b| b.as_array()).map(|v| v.iter()).unwrap_or_default(),
        ) {
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

    // --- AnimData.d2 (audit od2-features-formats #19) ----------------------

    /// Fournit le niveau + les anims du héros BA et des monstres, puis finalize.
    fn full_ba_game(root: &str) -> (GameCore, Value) {
        let m: Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap(),
        )
        .unwrap();
        let mut g = GameCore::new(&m.to_string(), "BA").unwrap();
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
        provide_anim_block(&mut g, root, &m["heroes"]["BA"]["walk"]);
        for mon in m.get("monsters").unwrap().as_array().unwrap() {
            provide_anim_block(&mut g, root, &mon["walk"]);
        }
        (g, m)
    }

    #[test]
    fn animdata_provided_branches_official_frames_and_speed() {
        let Some(root) = ws_root() else { return };
        let (mut g, _) = full_ba_game(&root);
        // Le vrai AnimData.d2 extrait des MPQ (data\global\animdata.d2).
        g.provide(
            ANIMDATA_ASSET,
            std::fs::read(format!("{root}/assets/animdata/AnimData.d2")).unwrap(),
        );
        g.finalize().unwrap();

        // Note de branchement visible côté JS (spriteNotes()).
        let notes = g.sprite_notes_json();
        assert!(notes.contains("animdata branché"), "{notes}");

        // Héros BAWL1HS : record animdata = 8 frames, speed 168 → 16,406 fps.
        let hero = g.sprites.get("hero").unwrap();
        let rec = hero.animdata.as_ref().expect("hero set must carry its animdata record");
        assert_eq!(rec.name, "BAWL1HS");
        assert_eq!(hero.frames(), 8);
        assert!((hero.fps().unwrap() - 25.0 * 168.0 / 256.0).abs() < 1e-9);

        // Monstres : les tokens FAWLHTH/ZMWLHTH/SIWLHTH tombent aussi —
        // et le COF FA ne disait même pas la bonne speed (octet 0 vs 256).
        for (code, fpd, speed) in [("FA", 10u32, 256u16), ("ZM", 12, 104), ("SI", 9, 256)] {
            let set = g.sprites.get(code).unwrap_or_else(|| panic!("set {code} attendu"));
            let rec = set.animdata.as_ref().unwrap_or_else(|| panic!("{code} sans record"));
            assert_eq!((rec.frames_per_direction, rec.speed), (fpd, speed), "{code}");
        }

        // Le jeu tourne toujours.
        let _ = g.tick(16, None);
        assert_eq!(g.render().len(), (DEFAULT_VIEWPORT_W * DEFAULT_VIEWPORT_H * 4) as usize);
    }

    #[test]
    fn animdata_corrupt_falls_back_to_cof_with_note() {
        let Some(root) = ws_root() else { return };
        let (mut g, _) = full_ba_game(&root);
        g.provide(ANIMDATA_ASSET, vec![0xFF; 512]); // illisible
        g.finalize().unwrap(); // pas un échec
        let notes = g.sprite_notes_json();
        assert!(notes.contains("illisible"), "{notes}");
        // Fallback : octets COF, aucun record branché.
        let hero = g.sprites.get("hero").unwrap();
        assert!(hero.animdata.is_none());
        assert_eq!(hero.frames(), hero.cof.frames_per_direction);
        assert!(hero.fps().is_none());
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
        g.blit_entity(&mut fb, g.vw as usize, g.vh as usize, "hero", &dead, None);
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

        // Occlusion : le town a des arbres hauts → tampon overlay peint.
        assert!(stats.tall > 300, "arbres du town attendus en overlay: {stats:?}");
        let map = g.map.as_ref().unwrap();
        assert!(map.overlay.iter().any(|&v| v != 0), "overlay vide");
        assert_eq!(map.overlay.len(), map.idx.len(), "overlay même géométrie");

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
    fn outdoor_fallback_spawn_is_data_driven() {
        // Socle W01/A3 : un bloc niveau SANS `monsters` reçoit la planche
        // mon1-10 réelle (patch_levels.txt Id=2 : zombie1/fallen1/quillrat1
        // → codes ZM/FA/SI) + les packs MinGrp/MaxGrp de patch_monstats.txt
        // (spawn.rs) — plus la constante 6 codes en dur.
        let block: Value = serde_json::json!({
            "levelsId": 2,
            "ds1": "assets/map/bloodmoor.ds1",
            "palette": "assets/map/pal_act1_pl2.dat"
        });
        let def = LevelDef::from_manifest("data-driven", &block).unwrap();
        assert_eq!(def.monsters, ["ZM", "FA", "SI"], "planches réelles Id=2");
        assert_eq!(def.plate_packs, vec![(1u32, 2u32), (2, 3), (1, 2)], "packs patch_monstats");
        // Id inconnu de la table : repli historique (6 codes constants),
        // sans packs (comportement 1-entrée = 1-monstre conservé).
        let block: Value = serde_json::json!({ "levelsId": 9999, "ds1": "x.ds1", "palette": "p.dat" });
        let def = LevelDef::from_manifest("unknown", &block).unwrap();
        assert_eq!(def.monsters.len(), 6);
        assert!(def.plate_packs.is_empty());
        // Le manifest réel (bloodmoor AVEC liste `monsters`) reste tel quel :
        // priorité manifest, pas de packs (comportement historique).
        let g = GameCore::new(MANIFEST, "BA").unwrap();
        let bm = g.resolve_level_def("bloodmoor").unwrap();
        assert_eq!(bm.monsters.len(), 6);
        assert!(bm.plate_packs.is_empty());
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
    fn save_v3_keeps_level_key_across_switch_and_load_world() {
        let Some(root) = ws_root() else { return };
        // --- town : save v3 portant la clé "town"
        let mut g = full_game(MANIFEST, "town", &root);
        let save_town = g.save_world();
        let v: Value = serde_json::from_str(&save_town).unwrap();
        assert_eq!(v["version"], serde_json::json!(3), "format courant v3");
        assert_eq!(v["level"], serde_json::json!("town"));

        // --- bloodmoor : le save suit le héros (warp ⇒ patch_save_for_level)
        g.switch_level("bloodmoor", 137.0, 162.0).unwrap();
        let save_bm = g.save_world();
        let v: Value = serde_json::from_str(&save_bm).unwrap();
        assert_eq!(v["level"], serde_json::json!("bloodmoor"), "clé alignée sur le switch");

        // --- retour au camp, puis « Continuer » sur le save bloodmoor :
        // load_world recharge la bonne zone tout seul (grille 72×72 tuiles).
        g.switch_level("town", 247.0, 107.0).unwrap();
        g.load_world(&save_bm).unwrap();
        assert_eq!(g.grid_size(), (72 * 5, 72 * 5), "carte Blood Moor restaurée");
        let w = g.world.as_ref().unwrap();
        assert_eq!(w.level_key(), "bloodmoor");
        assert!(g.level_info_json().contains("La Lande sanglante"));
        // héros rendu à sa position sauvegardée
        let hv: Value = serde_json::from_str(&save_bm).unwrap();
        let hx = hv.pointer("/hero/ent/x").unwrap().as_f64().unwrap() as f32;
        let hy = hv.pointer("/hero/ent/y").unwrap().as_f64().unwrap() as f32;
        let h = w.hero();
        assert!((h.x - hx).abs() < 2.0 && (h.y - hy).abs() < 2.0, "({hx},{hy}) vs ({},{})", h.x, h.y);

        // --- rétro-compat : save v2 sans `level` ⇒ défaut town (re-switch auto)
        let mut v2 = serde_json::from_str::<Value>(&save_town).unwrap();
        v2["version"] = serde_json::json!(2);
        v2.as_object_mut().unwrap().remove("level");
        g.load_world(&v2.to_string()).unwrap();
        assert_eq!(g.grid_size(), (57 * 5, 41 * 5), "retour Camp des Rousses");
        assert_eq!(g.world.as_ref().unwrap().level_key(), "town");
    }

    #[test]
    fn switch_to_bloodmoor_reloads_and_spawns() {
        let Some(root) = ws_root() else { return };
        let mut g = full_game(MANIFEST, "town", &root);
        // switch : nouvelle carte, héros au point d'entrée, monstres spawnés
        let info = g.switch_level("bloodmoor", 137.0, 162.0).unwrap();
        assert_eq!(info.key, "bloodmoor");
        assert_eq!(g.grid_size(), (72 * 5, 72 * 5));
        // W18 : la bannière vient des TBL fr (string.tbl 5054 « La Lande sanglante »).
        assert_eq!(g.level_info_json().contains("La Lande sanglante"), true);
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

    // --- W01/W02/W04 : tables de niveaux + DRLG seedé ------------------------

    /// Comme [`full_game`] avec graine de partie et l'asset tables de
    /// niveaux fourni (branche W01/W04 active).
    fn full_game_with_tables(manifest_text: &str, level: &str, root: &str, seed: u64) -> GameCore {
        let m: Value = serde_json::from_str(manifest_text).unwrap();
        let mut g = GameCore::with_level(manifest_text, "BA", level).unwrap();
        g.set_game_seed(seed);
        provide_all_levels(&mut g, &m, root);
        g.provide(
            LEVELS_ASSET,
            std::fs::read(format!("{root}/{LEVELS_ASSET}")).expect("patch_levels.txt"),
        );
        g.finalize().unwrap();
        g
    }

    #[test]
    fn tables_branch_generates_seeded_bloodmoor() {
        let Some(root) = ws_root() else { return };
        let notes = |g: &GameCore| g.switch_notes_json();
        // Boot sur Blood Moor avec tables : DRLG seedé (pas le DS1 figé).
        let mut a = full_game_with_tables(MANIFEST, "bloodmoor", &root, 42);
        assert!(notes(&a).contains("tables de niveaux branchées"), "{}", notes(&a));
        assert!(notes(&a).contains("drlg seedé actif"), "{}", notes(&a));
        assert_eq!(a.grid_size(), (72 * 5, 72 * 5));
        // Le monde tourne et rend une carte non vide.
        let _ = a.tick(16, None);
        let lit = a
            .render()
            .chunks_exact(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
            .count();
        assert!(lit > 10_000, "carte générée rendue trop sombre : {lit}");

        // Déterminisme : même graine ⇒ grille de collision identique.
        let b = full_game_with_tables(MANIFEST, "bloodmoor", &root, 42);
        assert_eq!(a.collision(), b.collision(), "même partie ⇒ même Blood Moor");
        // Graine différente ⇒ monde différent (mais rejouable à l'identique).
        let c = full_game_with_tables(MANIFEST, "bloodmoor", &root, 43);
        assert_ne!(a.collision(), c.collision(), "graine différente ⇒ monde différent");
        let c2 = full_game_with_tables(MANIFEST, "bloodmoor", &root, 43);
        assert_eq!(c.collision(), c2.collision(), "chaque graine est déterministe");
        // Sans tables (fallback historique) : ni notes tables ni DRLG.
        let frozen = full_game(MANIFEST, "bloodmoor", &root);
        assert!(!notes(&frozen).contains("drlg seedé"));
        assert!(!notes(&frozen).contains("tables de niveaux"));
    }

    #[test]
    fn warp_round_trip_on_generated_map() {
        let Some(root) = ws_root() else { return };
        let mut g = full_game_with_tables(MANIFEST, "town", &root, 7);
        let walk_to = |g: &mut GameCore, x: i32, y: i32| -> bool {
            let _ = g.tick(16, Some(PlayerInput::MoveTo { x, y }));
            for _ in 0..6000 {
                let evs = g.tick(16, None);
                if evs.iter().any(|e| matches!(e, CoreEvent::LevelSwitch(_))) {
                    return true;
                }
            }
            false
        };
        // Town → Blood Moor GÉNÉRÉE (levelsId 2, DrlgType 3).
        assert!(walk_to(&mut g, 282, 107), "warp town→bloodmoor (générée) a échoué");
        assert!(g.level_info_json().contains("La Lande sanglante"));
        let world = g.world.as_ref().unwrap();
        let h = *world.hero();
        assert!(world.is_walkable(h.x as i32, h.y as i32), "héros posé hors marchable");
        let n = world.monsters().len();
        assert!((5..=8).contains(&n), "5-8 monstres sur la carte générée, eu {n}");
        for m in world.monsters() {
            assert!(world.is_walkable(m.x as i32, m.y as i32));
        }
        // Et retour au camp : zone sûre.
        assert!(walk_to(&mut g, 137, 147), "warp bloodmoor(générée)→town a échoué");
        assert_eq!(g.grid_size(), (57 * 5, 41 * 5));
        assert!(g.world.as_ref().unwrap().monsters().is_empty());
    }

    #[test]
    fn resolve_level_def_by_table_id_with_tables() {
        let Some(root) = ws_root() else { return };
        let g = full_game_with_tables(MANIFEST, "town", &root, 0);
        // "2" / "level:2" → bloc manifest bloodmoor (levelsId 2).
        let by_id = g.resolve_level_def("2").unwrap();
        assert_eq!(by_id.key, "bloodmoor");
        assert_eq!(by_id.levels_id, Some(2));
        let by_prefixed = g.resolve_level_def("level:2").unwrap();
        assert_eq!(by_prefixed.key, "bloodmoor");
        // Le DRLG s'applique bien à cette définition (record DrlgType 3).
        assert!(g.drlg_applies(&by_id));
        // Id sans bloc manifest : erreur claire (assets introuvables).
        assert!(g.resolve_level_def("20").is_err());
        // Le switch par id génère la carte seedée.
        let mut g2 = full_game_with_tables(MANIFEST, "town", &root, 0);
        let info = g2.switch_level("2", 137.0, 147.0).unwrap();
        assert_eq!(info.key, "bloodmoor");
        assert!(g2.switch_notes_json().contains("drlg seedé actif"));
        // Voisins Vis/Warp exposés dans level_info (W04, append-only).
        let info: Value = serde_json::from_str(&g2.level_info_json()).unwrap();
        let neighbors = info["neighbors"].as_array().expect("neighbors exposés");
        // W04 : voisins Vis/Warp exposés (append-only) ; W18 : noms fr des TBL
        // (string.tbl 5048 « Le Repaire du Mal »).
        assert!(
            neighbors
                .iter()
                .any(|n| n["id"] == 8 && n["name"] == "Le Repaire du Mal")
        );
    }

    // --- étape 8a : DRLG maze (DrlgType 1) — branche unique de load_level ----

    #[test]
    fn maze_drlg_branch_generates_seeded_tomb_then_tunnels() {
        let Some(root) = ws_root() else { return };
        let notes = |g: &GameCore| g.switch_notes_json();
        // Boot Stony Tomb 1 (levelsId 55, LvlMaze 4 rooms 16×16) : le DRLG
        // maze s'active par la MÊME branche que la Blood Moor (load_level).
        let mut a = full_game_with_tables(MANIFEST, "tombf1", &root, 42);
        assert!(notes(&a).contains("tables de niveaux branchées"), "{}", notes(&a));
        assert!(notes(&a).contains("drlg seedé actif"), "{}", notes(&a));
        // Grille 3×3 rooms de 16×16 = 48×48 tuiles (×5 sous-tuiles).
        assert_eq!(a.grid_size(), (48 * 5, 48 * 5));
        // Le monde tourne et rend une carte non noire.
        let _ = a.tick(16, None);
        let lit = a
            .render()
            .chunks_exact(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 30)
            .count();
        assert!(lit > 10_000, "maze rendu trop sombre : {lit}");
        // Déterminisme seedé au niveau du jeu complet.
        let b = full_game_with_tables(MANIFEST, "tombf1", &root, 42);
        assert_eq!(a.collision(), b.collision(), "même partie ⇒ même tombeau");
        let c = full_game_with_tables(MANIFEST, "tombf1", &root, 43);
        assert_ne!(a.collision(), c.collision(), "graine différente ⇒ tombeau différent");
        // Switch vers les Ancient Tunnels (65) : même branche, autre banque
        // (LvlType 13) — 5×5 rooms de 12×12 = 60×60 tuiles.
        let info = a.switch_level("ancienttunnels", 24.0, 24.0).unwrap();
        assert_eq!(info.key, "ancienttunnels");
        assert_eq!(a.grid_size(), (60 * 5, 60 * 5));
        let world = a.world.as_ref().unwrap();
        let h = *world.hero();
        assert!(world.is_walkable(h.x as i32, h.y as i32), "héros posé hors marchable");
        assert!(notes(&a).contains("ancienttunnels 60×60 généré"), "{}", notes(&a));
    }

    // --- ARBRE DE COMPÉTENCES (C2/C3/C4/C5, mission arbre 2026-10-23) ------

    #[test]
    fn skill_tree_json_lists_class_skills_with_prereqs_icons_and_geometry() {
        let Some(root) = ws_root() else { return };
        let (g, _) = full_ba_game(&root);
        let t: Value = serde_json::from_str(&g.skill_tree_json()).unwrap();
        assert_eq!(t["class"], "bar");
        assert_eq!(t["hero"], "BA");
        assert_eq!(t["points"], 0, "aucun point au démarrage");
        let skills = t["skills"].as_array().unwrap();
        assert_eq!(skills.len(), 30, "30 skills de classe (patch_skills.txt)");
        // Tri onglet → rangée → colonne : Bash (page 1, row 1, col 2) premier.
        assert_eq!(skills[0]["code"], "Bash");
        assert_eq!(skills[0]["page"], 1);
        assert_eq!(skills[0]["row"], 1);
        assert_eq!(skills[0]["icon"], 0, "IconCel bash (skilldesc.txt)");
        assert_eq!(skills[0]["nameKey"], "skillname126");
        // Prérequis réels : Double Swing ET Stun exigent Bash (reqskill1).
        let ds = skills.iter().find(|s| s["code"] == "Double Swing").unwrap();
        assert_eq!(ds["prereqs"], serde_json::json!(["Bash"]));
        assert_eq!(ds["reqLevel"], 6);
        let stun = skills.iter().find(|s| s["code"] == "Stun").unwrap();
        assert_eq!(stun["prereqs"], serde_json::json!(["Bash"]));
        assert_eq!(stun["reqLevel"], 12);
        assert_eq!(stun["icon"], 26, "IconCel stun (skilldesc.txt)");
        // Niveau 0 partout avant apprentissage, plafond maxlvl 20 (col maxlvl).
        assert!(skills.iter().all(|s| s["level"] == 0));
        assert!(skills.iter().all(|s| s["maxLevel"] == 20));
        // canLearn sans point : false partout (NoPoints).
        assert!(skills.iter().all(|s| s["canLearn"] == false));
        // Avant finalize (monde absent) : arbre vide de niveaux, pas de panic.
        let g2 = GameCore::new(MANIFEST, "BA").unwrap();
        let t2: Value = serde_json::from_str(&g2.skill_tree_json()).unwrap();
        assert_eq!(t2["skills"].as_array().unwrap().len(), 30);
        assert_eq!(t2["points"], 0);
    }

    #[test]
    fn learn_skill_spends_points_and_enforces_prereqs() {
        let Some(root) = ws_root() else { return };
        let (mut g, _) = full_ba_game(&root);
        g.finalize().unwrap();
        // Sans point : refus explicite (NoPoints), état inchangé.
        let r: Value = serde_json::from_str(&g.learn_skill("Bash")).unwrap();
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("point"), "{}", r["error"]);
        // Prérequis bloqué même avec un point : Double Swing exige Bash.
        g.world.as_mut().unwrap().grant_skill_points(1);
        let r: Value = serde_json::from_str(&g.learn_skill("Double Swing")).unwrap();
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("Bash"), "{}", r["error"]);
        // Apprend Bash (niv 1) : 1 point dépensé, skill connu, sélectionnable.
        g.world.as_mut().unwrap().grant_skill_points(2);
        let r: Value = serde_json::from_str(&g.learn_skill("Bash")).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["level"], 1);
        assert_eq!(r["points"], 2);
        let skills: Value = serde_json::from_str(&g.hero_skills_json()).unwrap();
        assert!(skills.as_array().unwrap().iter().any(|s| s["code"] == "Bash"));
        // Monter Bash au plafond : 20 niveaux max (19 de plus refusés au 21e).
        for _ in 0..19 {
            g.world.as_mut().unwrap().grant_skill_points(1);
            let r: Value = serde_json::from_str(&g.learn_skill("Bash")).unwrap();
            assert_eq!(r["ok"], true);
        }
        let r: Value = serde_json::from_str(&g.learn_skill("Bash")).unwrap();
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("plafond"), "{}", r["error"]);
        // Le prérequis est maintenant SATISFAIT : Double Swing apprenable
        // (reqlevel 6 > niv 1 du héros → refus reqlevel, pas prérequis).
        g.world.as_mut().unwrap().grant_skill_points(1);
        let r: Value = serde_json::from_str(&g.learn_skill("Double Swing")).unwrap();
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("niveau"), "{}", r["error"]);
        // Arbre rafraîchi : Bash niveau 20, canLearn reflète l'état.
        let t: Value = serde_json::from_str(&g.skill_tree_json()).unwrap();
        let bash = t["skills"].as_array().unwrap().iter().find(|s| s["code"] == "Bash").unwrap();
        assert_eq!(bash["level"], 20);
        assert_eq!(bash["canLearn"], false);
        // 3 points restants : 2 de la boucle (grant 1 / spend 1 → solde
        // inchangé après Bash niv 2..20) + 1 du refus reqlevel ci-dessus —
        // un refus ne dépense JAMAIS de point.
        assert_eq!(t["points"], 3, "seuls les apprentissages réussis coûtent");
    }

    #[test]
    fn sor_tree_and_learn_flow_match_real_table() {
        let Some(root) = ws_root() else { return };
        // Héros SOR : Fire Bolt apprenable niv 1 (reqlevel 1, sans prérequis) ;
        // Fire Ball exige Fire Bolt + reqlevel 12 (patch_skills.txt).
        let m: Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap(),
        )
        .unwrap();
        let mut g = GameCore::new(&m.to_string(), "SOR").unwrap();
        // Assets du town (comme full_ba_game, héros SOR — anims inutiles ici).
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
        g.finalize().unwrap();
        // Fire Bolt démarre CONNU (StartSkill charstats) niveau 1.
        let t: Value = serde_json::from_str(&g.skill_tree_json()).unwrap();
        assert_eq!(t["class"], "sor");
        let skills = t["skills"].as_array().unwrap();
        let fb = skills.iter().find(|s| s["code"] == "Fire Bolt").unwrap();
        assert_eq!(fb["level"], 1, "skill de départ porté dans l'arbre");
        assert_eq!(fb["missile"], "firebolt");
        let ball = skills.iter().find(|s| s["code"] == "Fire Ball").unwrap();
        assert_eq!(ball["prereqs"], serde_json::json!(["Fire Bolt"]));
        assert_eq!(ball["level"], 0);
        // 1 point : monter Fire Bolt marche ; Fire Ball refusé (reqlevel 12).
        g.world.as_mut().unwrap().grant_skill_points(1);
        let r: Value = serde_json::from_str(&g.learn_skill("Fire Bolt")).unwrap();
        assert_eq!(r["ok"], true);
        assert_eq!(r["level"], 2);
        g.world.as_mut().unwrap().grant_skill_points(1);
        let r: Value = serde_json::from_str(&g.learn_skill("Fire Ball")).unwrap();
        assert_eq!(r["ok"], false);
        assert!(r["error"].as_str().unwrap().contains("12"), "{}", r["error"]);
    }

    // --- 9d (monde vivant) : AutoMap — automap_json (automap.rs) -------------

    #[test]
    fn automap_empty_before_finalize() {
        let g = GameCore::new(MANIFEST, "BA").unwrap();
        assert_eq!(g.automap_json(), "{}");
    }

    #[test]
    fn automap_town_matches_grid_and_is_deterministic() {
        let Some(root) = ws_root() else { return };
        let g = full_game_with_tables(MANIFEST, "town", &root, 7);
        let j1 = g.automap_json();
        let v: Value = serde_json::from_str(&j1).unwrap();
        assert_eq!(v["level"], "town");
        assert_eq!(v["mode"], "automap", "tables branchées + groupe « 1 Town »");
        assert_eq!(v["lvlType"], "Town");
        assert_eq!(v["cellW"], 57);
        assert_eq!(v["cellH"], 41);
        assert_eq!(v["sub"], 5);
        // Comptes cohérents avec la grille : les cellules « walk » du JSON
        // sont EXACTEMENT les centres 5×5 marchables de la grille de collision.
        let cells = v["cells"].as_array().unwrap();
        assert!(!cells.is_empty());
        let mut walk = 0usize;
        let mut cels = 0usize;
        let mut glyphs: std::collections::HashSet<i64> = std::collections::HashSet::new();
        for c in cells {
            assert_eq!(c.as_array().map(|a| a.len()), Some(5), "cellule [x, y, flags, cel, glyph]");
            let (x, y) = (c[0].as_u64().unwrap() as usize, c[1].as_u64().unwrap() as usize);
            assert!(x < 57 && y < 41, "cellule dans les bornes");
            let f = c[2].as_i64().unwrap();
            if f & 1 != 0 { walk += 1; }
            if c[3].as_i64().unwrap() >= 0 { cels += 1; }
            if c[4].as_i64().unwrap() >= 0 { glyphs.insert(c[4].as_i64().unwrap()); }
        }
        assert!(cels > 0, "groupe 1 Town : au moins un Cel résolu");
        // 21b : les Cels résolus portent leurs traits précis (glyphes), le
        // tableau partagé couvre exactement les index référencés.
        let glyph_table = v["glyphs"].as_array().unwrap();
        assert!(!glyph_table.is_empty(), "mode automap : traits réels exposés");
        assert!(glyphs.iter().all(|g| (*g as usize) < glyph_table.len()), "index de glyphe dans les bornes");
        assert_eq!(glyphs.len(), glyph_table.len(), "pas de glyphe mort dans le tableau");
        assert!(glyph_table.iter().all(|g| g.as_array().unwrap().iter()
            .all(|s| s.as_array().unwrap().len() == 4)), "segments [x1,y1,x2,y2]");
        let map = g.map.as_ref().unwrap();
        let expected_walk: usize = (0..map.grid_h)
            .filter(|gy| gy % 5 == 2)
            .map(|gy| (0..map.grid_w).filter(|gx| gx % 5 == 2 && map.walkable(*gx, gy)).count())
            .sum();
        assert_eq!(walk, expected_walk, "JSON walk == centres marchables de la grille");
        // Déterminisme : même instance, puis même graine dans une 2e instance.
        assert_eq!(j1, g.automap_json(), "deux appels identiques");
        let g2 = full_game_with_tables(MANIFEST, "town", &root, 7);
        assert_eq!(j1, g2.automap_json(), "même graine ⇒ même automap");
    }

    #[test]
    fn automap_bloodmoor_seeded_drlg_wilderness() {
        let Some(root) = ws_root() else { return };
        let mut g = full_game_with_tables(MANIFEST, "bloodmoor", &root, 42);
        let j1 = g.automap_json();
        let v: Value = serde_json::from_str(&j1).unwrap();
        assert_eq!(v["level"], "bloodmoor");
        assert_eq!(v["mode"], "automap");
        assert_eq!(v["lvlType"], "Wilderness", "LvlTypes.txt Id 2 « Act 1 - Wilderness »");
        assert_eq!(v["cellW"], 72);
        assert_eq!(v["cellH"], 72);
        let cells = v["cells"].as_array().unwrap();
        let walk = cells.iter().filter(|c| c[2].as_i64().unwrap() & 1 == 1).count();
        assert!(walk > 500, "lande : {walk} cellules marchables attendues");
        // DRLG seedé : même graine de partie ⇒ JSON AutoMap identique.
        let g2 = full_game_with_tables(MANIFEST, "bloodmoor", &root, 42);
        assert_eq!(j1, g2.automap_json(), "même graine ⇒ même automap (DRLG + résolution)");
        // Après un switch retour ville : le cache suit le niveau courant.
        g.switch_level("town", 247.0, 107.0).unwrap();
        let vt: Value = serde_json::from_str(&g.automap_json()).unwrap();
        assert_eq!(vt["level"], "town");
        assert_eq!(vt["cellW"], 57);
    }

    #[test]
    fn automap_fallback_without_tables() {
        // Sans patch_levels.txt : pas d'identité (acte/LvlType) ⇒ mode
        // « fallback » documenté (polygone marchable côté démo), cellules
        // présentes mais AUCUN Cel.
        let Some(root) = ws_root() else { return };
        let g = full_game(MANIFEST, "town", &root);
        let v: Value = serde_json::from_str(&g.automap_json()).unwrap();
        assert_eq!(v["level"], "town");
        assert_eq!(v["mode"], "fallback");
        assert_eq!(v["lvlType"], "");
        let cells = v["cells"].as_array().unwrap();
        assert!(!cells.is_empty(), "les cellules visibles restent exposées");
        assert!(cells.iter().all(|c| c[3].as_i64().unwrap() == -1), "fallback : aucun Cel");
        let walk = cells.iter().filter(|c| c[2].as_i64().unwrap() & 1 == 1).count();
        assert!(walk > 0, "le polygone marchable a de la matière");
    }

    // --- 13b (finitions) : AutoMap des 5 niveaux finaux A4-A5 ---------------
    // Mode par niveau, résolu par la COUVERTURE mesurée (automap.rs, seuil
    // AUTOMAP_MIN_COVERAGE) et validé contre les banques DT1 réelles (sondes
    // `a45_bank_coverages_match_documented_modes`). Le `name` vient des TBL
    // fr réelles (clés LevelName de patch_levels.txt — bannière de zone).

    /// automap_json() du niveau chargé + déterminisme (même graine ⇒ même JSON).
    fn automap_a45(level: &str, root: &str, seed: u64) -> (Value, GameCore) {
        let g = full_game_with_tables(MANIFEST, level, root, seed);
        let j1 = g.automap_json();
        let v: Value = serde_json::from_str(&j1).unwrap();
        let g2 = full_game_with_tables(MANIFEST, level, root, seed);
        assert_eq!(j1, g2.automap_json(), "{level} : même graine ⇒ même automap");
        (v, g)
    }

    #[test]
    fn automap_a45_modes_coverage_and_fr_names() {
        let Some(root) = ws_root() else { return };
        // (clé, act, lvlType post-alias, mode, nom fr TBL, borne couverture)
        let plan: &[(&str, u32, &str, &str, &str, fn(f32) -> bool)] = &[
            ("pandemonium", 4, "Town", "fallback", "La Fort. de Pandémonium",
                |c| c < 0.10),
            // Harrogath : acte 5, même nom court « Town » — mais AUCUN groupe
            // « 5 Town » dans AutoMap.txt v1.12 → fallback garanti.
            ("harrogath", 5, "Town", "fallback", "Harrogath",
                |c| c < 0.10),
            ("bloodyfoothills", 5, "Siege", "fallback", "Les Collines Sanglantes",
                |c| c < 0.60),
            // Seul niveau au mapping propre (95 %, alias « Ice Caves »→« Ice »).
            ("crystalline", 5, "Ice", "automap", "Le Passage Cristallin",
                |c| c >= 0.90),
            ("hallsvaught", 5, "Temple", "fallback", "Les Corridors du Martyre",
                |c| c < 0.60),
        ];
        for (level, act, lvl_type, mode, name, cov_ok) in plan {
            let (v, g) = automap_a45(level, &root, 7);
            assert_eq!(v["level"], *level, "{level} : clé");
            assert_eq!(v["act"], *act, "{level} : acte résolu du nom LvlTypes");
            assert_eq!(v["lvlType"], *lvl_type, "{level} : LvlType (post-alias)");
            assert_eq!(v["mode"], *mode, "{level} : mode attendu (couverture réelle)");
            assert_eq!(v["name"], *name, "{level} : bannière de zone fr via TBL");
            let coverage = v["coverage"].as_f64().unwrap_or(-1.0) as f32;
            assert!(cov_ok(coverage), "{level} : couverture {coverage:.2} hors bande documentée");
            let cells = v["cells"].as_array().expect("cells");
            assert!(!cells.is_empty(), "{level} : cellules visibles");
            // Cohérence grille : mêmes bornes que le composeur.
            let (w, h) = (v["cellW"].as_u64().unwrap(), v["cellH"].as_u64().unwrap());
            assert!(cells.iter().all(|c| c[0].as_u64().unwrap() < w && c[1].as_u64().unwrap() < h));
            if *mode == "automap" {
                let cels = cells.iter().filter(|c| c[3].as_i64().unwrap() >= 0).count();
                assert!(cels > 0, "{level} : des Cel réels en mode automap");
                assert!((cels as f32 / cells.len() as f32 * 100.0).round() / 100.0 >= 0.60,
                    "{level} : le mode suit la couverture (seuil 0,60)");
            } else {
                // fallback : au moins le polygone marchable a de la matière.
                assert!(cells.iter().any(|c| c[2].as_i64().unwrap() & 1 == 1),
                    "{level} : marchable présent pour le repli");
            }
            drop(g);
        }
    }

    #[test]
    fn automap_town_and_bloodmoor_keep_automap_mode_at_threshold() {
        // Non-régression 13b : le seuil de couverture ne rétrograde PAS les
        // deux niveaux historiques (couvertures de banque 70 % / 76 %).
        let Some(root) = ws_root() else { return };
        for level in ["town", "bloodmoor"] {
            let (v, _) = automap_a45(level, &root, 7);
            assert_eq!(v["mode"], "automap", "{level} : reste en automap");
            assert!(v["coverage"].as_f64().unwrap() >= 0.60, "{level} : couverture au-dessus du seuil");
        }
    }

    // --- 21b (divers fidélité) : traits précis — les Cels mappés aux --------
    // polygones DT1 réels. Golden sur les 8 niveaux finaux (5 × A4-A5 +
    // tombeaux/égouts/arcane A2, DT1 frais de l'étape 8) + non-régression
    // ville/lande. Bandes calibrées par la sonde `traits_probe` (--nocapture).

    /// Sonde 21b : stats de traits d'un niveau (mode, couverture, Cels,
    /// glyphes, segments, cellules Cels sans glyphe) — imprimée à chaque
    /// golden (étalonnage `--nocapture`), puis bandes d'assertions.
    fn traits_probe(level: &str, root: &str, seed: u64) -> Value {
        let g = full_game_with_tables(MANIFEST, level, root, seed);
        let j1 = g.automap_json();
        let v: Value = serde_json::from_str(&j1).unwrap();
        // Déterminisme (même graine ⇒ même automap) — chargé deux fois.
        let g2 = full_game_with_tables(MANIFEST, level, root, seed);
        assert_eq!(j1, g2.automap_json(), "{level} : même graine ⇒ même automap");
        let cells = v["cells"].as_array().unwrap();
        let cels: std::collections::HashSet<i64> = cells
            .iter().map(|c| c[3].as_i64().unwrap()).filter(|&c| c >= 0).collect();
        let glyphs: std::collections::HashSet<i64> = cells
            .iter().map(|c| c[4].as_i64().unwrap()).filter(|&g| g >= 0).collect();
        let segs: usize = v["glyphs"].as_array().map(|g| g.iter()
            .map(|s| s.as_array().unwrap().len()).sum()).unwrap_or(0);
        let unmapped = cells.iter()
            .filter(|c| c[3].as_i64().unwrap() >= 0 && c[4].as_i64().unwrap() < 0).count();
        // Cels sans AUCUN glyphe (empreinte vide partout) — documentés.
        let cels_with: std::collections::HashSet<i64> = cells.iter()
            .filter(|c| c[4].as_i64().unwrap() >= 0).map(|c| c[3].as_i64().unwrap()).collect();
        let cels_orphans = cels.len() - cels_with.len();
        println!(
            "{level:16} mode={:8} cov={:.2} cells={} cels={} distinct={} glyphs={} segs={} unmapped={} orphans={}",
            v["mode"].as_str().unwrap(), v["coverage"].as_f64().unwrap(),
            cells.len(), cells.iter().filter(|c| c[3].as_i64().unwrap() >= 0).count(),
            cels.len(), glyphs.len(), segs, unmapped, cels_orphans
        );
        v
    }

    /// Golden 21b : mode + couverture + cohérence des traits par niveau.
    /// `(clé, mode, bande couverture, bande unmapped (frac cellules Cels
    /// sans empreinte), cels orphans autorisés)` — étalonné sur sonde réelle
    /// (seed 7). Les 5 niveaux A4-A5 finaux + 3 tombeaux A2 (étape 8) +
    /// ville/lande en non-régression.
    #[test]
    fn automap_21b_traits_golden_per_level() {
        let Some(root) = ws_root() else { return };
        let plan: &[(&str, &str, fn(f32) -> bool, fn(f32) -> bool)] = &[
            // A2 — tombeaux/égouts/arcane (DT1 frais étape 8) : sols/murs
            // couverts (banques 67-74 %), traits réels attendus.
            ("tombf1", "automap", |c| (0.60..0.85).contains(&c), |u| u < 0.20),
            ("ancienttunnels", "automap", |c| (0.60..0.85).contains(&c), |u| u < 0.10),
            ("arcane", "automap", |c| (0.65..0.90).contains(&c), |u| u < 0.25),
            // A4-A5 — les modes 13b inchangés.
            ("pandemonium", "fallback", |c| c < 0.10, |u| u == 0.0),
            ("harrogath", "fallback", |c| c < 0.10, |u| u == 0.0),
            ("bloodyfoothills", "fallback", |c| c < 0.60, |u| u == 0.0),
            ("crystalline", "automap", |c| c >= 0.90, |u| u < 0.15),
            ("hallsvaught", "fallback", |c| c < 0.60, |u| u == 0.0),
            // Non-régression A1 : couverture pleine, zéro cellule non mappée.
            ("town", "automap", |c| c >= 0.90, |u| u == 0.0),
            ("bloodmoor", "automap", |c| c >= 0.90, |u| u < 0.01),
        ];
        for (level, mode, cov_ok, unmapped_ok) in plan {
            let v = traits_probe(level, &root, 7);
            assert_eq!(v["mode"], *mode, "{level} : mode golden");
            let coverage = v["coverage"].as_f64().unwrap_or(-1.0) as f32;
            assert!(cov_ok(coverage), "{level} : couverture {coverage:.2} hors bande");
            let cells = v["cells"].as_array().expect("cells");
            assert!(!cells.is_empty(), "{level} : cellules visibles");
            let glyphs = v["glyphs"].as_array().expect("glyphs");
            let mapped = cells.iter().filter(|c| c[3].as_i64().unwrap() >= 0).count();
            let unmapped = cells.iter().filter(|c| c[3].as_i64().unwrap() >= 0
                && c[4].as_i64().unwrap() < 0).count();
            if *mode == "automap" {
                // Chaque Cel résolu porte son trait exact — sauf empreinte
                // vide (tuile sans blocs peints), bande documentée ci-dessus.
                assert!(unmapped_ok(unmapped as f32 / mapped.max(1) as f32),
                    "{level} : {unmapped}/{mapped} Cels sans empreinte, hors bande");
                // Cohérence Cel↔polygones : le tableau partagé référence
                // exactement les index utilisés, chaque entrée est un trait
                // axis-aligned dans le canevas 160×80, et les glyphes
                // dédupliqués restent ≤ au nombre de Cels distincts.
                let used: std::collections::HashSet<i64> = cells.iter()
                    .map(|c| c[4].as_i64().unwrap()).filter(|&g| g >= 0).collect();
                assert_eq!(used.len(), glyphs.len(), "{level} : pas de glyphe mort");
                assert!(used.iter().all(|g| (*g as usize) < glyphs.len()),
                    "{level} : index de glyphe dans les bornes");
                let distinct_cels: std::collections::HashSet<i64> = cells.iter()
                    .map(|c| c[3].as_i64().unwrap()).filter(|&c| c >= 0).collect();
                // Cohérence « Cels vs tuiles composées » : chaque Cel distinct
                // du niveau mappe à AU MOINS un polygone réel (un même Cel —
                // ex. joker mur `wr 0 -1 -1` — peut couvrir plusieurs
                // empreintes de murs, donc glyphes ≥ 1 par Cel, sans excès
                // au-delà des cellules mappées).
                let cels_with: std::collections::HashSet<i64> = cells.iter()
                    .filter(|c| c[4].as_i64().unwrap() >= 0)
                    .map(|c| c[3].as_i64().unwrap()).collect();
                assert_eq!(cels_with.len(), distinct_cels.len(),
                    "{level} : chaque Cel résolu a au moins un trait réel");
                assert!(glyphs.len() <= mapped, "{level} : glyphes dédupliqués ≤ cellules mappées");
                let segs: usize = glyphs.iter().map(|g| g.as_array().unwrap().len()).sum();
                assert!(segs > 0, "{level} : des traits réels");
                for g in glyphs {
                    for s in g.as_array().unwrap() {
                        let s = s.as_array().unwrap();
                        assert_eq!(s.len(), 4, "{level} : segment [x1,y1,x2,y2]");
                        let (x1, y1, x2, y2) = (s[0].as_i64().unwrap(), s[1].as_i64().unwrap(),
                            s[2].as_i64().unwrap(), s[3].as_i64().unwrap());
                        assert!(x1 == x2 || y1 == y2, "{level} : trait axis-aligned");
                        assert!((0..=160).contains(&x1) && (0..=160).contains(&x2)
                            && (0..=80).contains(&y1) && (0..=80).contains(&y2),
                            "{level} : trait dans le canevas 160×80");
                    }
                }
                println!("{level:16} glyphs={} segs={} (traits réels OK)", glyphs.len(), segs);
            } else {
                // Fallback : AUCUN trait exposé (raison documentée par
                // niveau — en-tête automap.rs), le polygone marchable reste.
                assert!(glyphs.is_empty(), "{level} : fallback sans glyphes");
                assert!(cells.iter().all(|c| c[4].as_i64().unwrap() == -1),
                    "{level} : aucun index de glyphe en fallback");
                assert!(cells.iter().any(|c| c[2].as_i64().unwrap() & 1 == 1),
                    "{level} : marchable présent pour le repli");
            }
        }
    }
}
