//! `World` : simulation déterministe — héros, FSM monstres, combat, XP, drops,
//! inventaire 10×4, save/load. Zéro IO, zéro rendu.

use crate::data::{self, HeroTpl, MonTpl};
use crate::difficulty::{self, Difficulty};
use crate::loot;
use crate::npc::NpcState;
use crate::path::{astar, snap_near, Grid};
use crate::quests::QuestLog;
use crate::rng::Rng;
use crate::skills::{default_active, starting_skills, HeroSkill};
use crate::skills_tree::SkillTreeError;
use crate::{combat_mods, skills_hero, xp, Entity, AnimState, GameEvent, HeroStats, ItemDef, ItemKind, ItemOnGround, PlayerInput};
use d2missiles::MissileSystem;
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
/// Rayon de collision missile↔monstre (tuiles, rayon de corps MVP).
pub(crate) const MISSILE_HIT_RADIUS: f32 = 0.7;
/// Missile de REPLI du tir à distance (`arrow` : Vel=24, Range=40,
/// patch_missiles.txt ligne Id=0). Utilisé quand le skill actif n'a pas de
/// missile (mêlée BA/PA) ou quand le héros n'a aucune compétence — le skill
/// actif prime sinon (SOR : `firebolt`, cf. `skills.rs`).
pub(crate) const HERO_MISSILE: &str = "arrow";
// ---------------------------------------------------------------------------
// XP héros — TABLE RÉELLE (13a-b, finition) : plus aucune constante locale,
// la courbe 99 niveaux × 7 classes vient de `xp.rs` (`patch_experience.txt`
// embarqué, colonne de la classe du héros). L'ancien plafond MVP
// `MAX_LEVEL = 10` et `XP_TABLE: [u32; 10]` (colonne Barbarian copiée en
// dur) sont supprimés : le plafond et les seuils SUIVENT LA DONNÉE.
// ---------------------------------------------------------------------------

/// Courbe XP de la classe `code` (table réelle embarquée ; repli barbare —
/// les 7 courbes v1.12 sont identiques, golden `progression_golden.rs`).
fn xp_curve(code: &str) -> &'static xp::ClassXp {
    let t = xp::XpTable::global();
    t.class(code).or_else(|| t.class("bar")).expect("table XP embarquée")
}

/// Seuil XP CUMULÉS pour atteindre `level` (ligne `level - 1` de la table ;
/// niveau 1 = 0). u32 sûr : dernière ligne 3 837 739 017 < u32::MAX.
pub(crate) fn xp_threshold(code: &str, level: u32) -> u32 {
    u32::try_from(xp_curve(code).threshold(level)).unwrap_or(u32::MAX)
}

/// Niveau max de la classe (`MaxLvl` de la table = 99 en v1.12).
pub(crate) fn max_level_of(code: &str) -> u32 {
    xp_curve(code).max_level
}

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

// Repli drop ( TreasureClassEx absente / TC inconnue — cf. loot.rs).
/// % rien.
pub(crate) const DROP_NOTHING_PCT: u32 = 60;
/// % or (ensuite : % potion).
pub(crate) const DROP_GOLD_PCT: u32 = 25;
/// Version du format de sauvegarde (4 = `players[]` multi-joueurs 3b-1 ;
/// les champs v3 de tête restent écrits en miroir du slot 0 — `load` accepte
/// v1..=4, une save v3 solo se charge = 1 joueur, zéro perte).
pub(crate) const SAVE_VERSION: u32 = 4;
/// Plus ancienne version acceptée au chargement (1 = sans skills — rétro-compat
/// : les skills de départ de la classe sont reconstitués, cf. `skills.rs` ;
/// 2 = sans clé de niveau — rétro-compat : niveau "town" par défaut).
pub(crate) const SAVE_VERSION_OLDEST: u32 = 1;
/// Clé de niveau par défaut (bloc `levels.town` du manifest, alias du bloc
/// `level` historique) — appliquée aux mondes neufs et aux sauvegardes
/// v1/v2 qui ne portent pas de clé de niveau.
pub(crate) const DEFAULT_LEVEL_KEY: &str = "town";
/// Grille d'inventaire MVP : 10 colonnes × 4 rangées.
pub(crate) const INV_COLS: usize = 10;
pub(crate) const INV_ROWS: usize = 4;
pub(crate) const INV_SLOTS: usize = INV_COLS * INV_ROWS;
/// 3b (coop multi-héros) : plafond de héros par monde — slot 0 (historique) +
/// 7 coéquipiers au plus (calé sur MAX_PLAYERS = 8 du salon, serveur/rooms.ts).
pub const MAX_HEROES: usize = 8;
/// 3b-1 (déterminisme) : constante de mixage (fraction dorée) pour dériver le
/// flux RNG INDÉPENDANT de chaque joueur : `état_monde ^ GOLDEN_MIX×(slot+1)`.
/// Deux joueurs ⇒ deux flux disjoints ; un même slot ⇒ le même flux partout
/// (lockstep : tous les clients dérivent depuis le même état monde).
pub(crate) const GOLDEN_MIX: u64 = 0x9E37_79B9_7F4A_7C15;

// ---------------------------------------------------------------------------
// États internes
// ---------------------------------------------------------------------------

/// FSM monstre (interne — d2game voit l'anim via `Entity.anim`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

/// Rang d'un monstre spawné (W08/W10/W11, impl-monstres-boss) : gouverne la
/// teinte de rendu d2game et le boost appliqué au spawn. Les multiplicateurs
/// (HP/dégâts/toucher) viennent des constantes réelles de `monumod.txt`,
/// portées par [`SpawnRule`] au moment du spawn — ici on ne garde que le
/// rang (sauvegardé, exposé par [`World::monster_kind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash, Serialize, Deserialize)]
pub enum MonKind {
    /// Monstre normal (planche mon1-10).
    #[default]
    Normal,
    /// Champion (leader bleu, mods monumod champion).
    Champion,
    /// Unique doré (mods monumod upick).
    Unique,
    /// Super-unique nommé (SuperUniques.txt : Bishibosh, Rakanishu…) —
    /// teinte dorée comme un unique, nom exposé par d2game.
    SuperUnique,
    /// Boss de quête (Andariel…) : record monstats dédié, TC dédiée.
    Boss,
}

impl MonKind {
    /// Libellé court (JSON debug/E2E d2game).
    pub fn as_str(&self) -> &'static str {
        match self {
            MonKind::Normal => "normal",
            MonKind::Champion => "champion",
            MonKind::Unique => "unique",
            MonKind::SuperUnique => "super_unique",
            MonKind::Boss => "boss",
        }
    }
}

/// Facteurs de spawn explicites — l'appelant (d2game) les calcule depuis les
/// tables réelles (`MonLvl.txt` ligne niveau du monstre, boosts `monumod.txt`
/// champion/unique, XP communauté ×3/×5) : `d2rules` reste pur, sans IO ni
/// table embarquée de plus. Les pourcentages sont relatifs aux valeurs brutes
/// monstats (`hp_pct = MonLvl.HP` = 7 pour le niveau 1 Normal — les constantes
/// historiques de ce fichier). `treasure_class` surcharge la TC de drop du
/// gabarit (TC champion/unique/boss réelles : "Act 1 Champ A", "Andariel"…).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnRule {
    /// Rang du monstre spawné.
    pub kind: MonKind,
    /// % HP appliqué aux PV bruts (MonLvl.HP × boosts éventuels).
    pub hp_pct: u32,
    /// % dégâts (MonLvl.DM × boosts).
    pub dmg_pct: u32,
    /// % toucher (MonLvl.TH × boosts).
    pub th_pct: u32,
    /// % XP (MonLvl.XP × boosts).
    pub xp_pct: u32,
    /// AC effectif (MonLvl.AC de la ligne).
    pub ac: u32,
    /// TC de drop effectives (aucune = celle du gabarit manifest).
    pub treasure_class: Option<String>,
    /// Résists du monstre POUR LA DIFFICULTÉ du spawn (W08/K) — ordre
    /// `[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]` (colonnes
    /// `patch_monstats.txt` 130-147 ± `(N)/(H)` ; vides = 0 ; ≥100 =
    /// immunité, négatif = amplifié — cf. `difficulty::apply_resist`).
    /// Défaut `[0; 6]` : zéro changement pour les spawns existants.
    pub res: [i32; 6],
}

impl Default for SpawnRule {
    /// Règle historique : MonLvl.txt ligne 1 Normal (AC 6, TH 8, ×7/100,
    /// ×2/100, ×30/100) — zéro changement de comportement pour le spawn
    /// existant ([`World::spawn_monster`]).
    fn default() -> SpawnRule {
        SpawnRule {
            kind: MonKind::Normal,
            hp_pct: MONLVL_HP_NUM,
            dmg_pct: MONLVL_DM_NUM,
            th_pct: MONLVL_TH,
            xp_pct: MONLVL_XP_NUM,
            ac: MONLVL_AC,
            treasure_class: None,
            res: [0; 6],
        }
    }
}

/// Données héros (stats, chemin, swing, inventaire, skills…). 3b-1 : `pub`
/// (portée par [`Player`] et les re-exports lib.rs) — passer par les
/// méthodes de [`Player`]/[`World`].
pub struct HeroData {
    /// Stats courantes (life = PV restants).
    pub stats: HeroStats,
    /// Chemin de waypoints (centres de tuiles), vide = immobile.
    pub path: Vec<(f32, f32)>,
    /// Compteur du swing en cours (= cooldown).
    pub swing_ms: u32,
    /// Étape 15b : le swing courant est un CAST de skill (`AnimState::SC`)
    /// et non une attaque A1 — transitoire, jamais sauvegardé (même traitement
    /// que `stamina_milli`) ; posé par `hero_ranged_attack` (skill porteur de
    /// missile) et `hero_attack` (faux).
    pub swing_cast: bool,
    /// > 0 : héros mort, compte jusqu'à réapparition.
    pub dead_ms: u32,
    /// Horloge d'animation (reset à chaque changement d'état).
    pub anim_ms: u32,
    /// Course active (touche Maj côté navigateur) : vitesse RN + anim RN.
    pub run: bool,
    /// 12c (additif, transitoire non persisté — comme `stamina_milli`) :
    /// mods de coup du héros (deadly strike / CB / plaies / leech…).
    pub mods: combat_mods::HitMods,
    /// Point d'apparition / respawn.
    pub spawn: (f32, f32),
    /// Inventaire 10×4, index rangée par rangée.
    pub inv: Vec<Option<ItemDef>>,
    /// Compétences connues (skills de départ par classe, cf. `skills.rs`).
    pub skills: Vec<HeroSkill>,
    /// Index du skill actif dans `skills` (hors bornes = aucun).
    pub active_skill: usize,
    /// EXP-H2 : accumulateur d'endurance sous-unitaire (1000 milli = 1 pt,
    /// `stamina.rs`) — TRANSITOIRE, jamais sauvegardé (la valeur affichée
    /// `stats.stamina` l'est ; recalé au chargement, cf. `save::load_json`).
    pub stamina_milli: i64,
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
    /// Rang ([`MonKind`]) — teinte d2game + events quête (sauvegardé).
    pub kind: MonKind,
    /// TC de drop effective (surcharge du gabarit : TC champion/unique/boss
    /// réelles de monstats TreasureClass2/3 ou TC dédiée boss) — `None` =
    /// TC du gabarit (comportement historique).
    pub tc: Option<String>,
    /// Résists effectifs de la difficulté de spawn (`SpawnRule::res`, W08/K)
    /// — indexés comme les colonnes ResDm/Ma/Fi/Li/Co/Po ; ≥100 = immunité.
    pub res: [i32; 6],
    /// 12c (additif, transitoire non persisté) : DoT plaies/poison/froid.
    pub wounds: Option<combat_mods::Dot>,
    /// 12c (idem) : poison en cours (ticks de 25 frames).
    pub poison: Option<combat_mods::Dot>,
    /// 12c (idem) : froid — ralenti restant en ms (vitesse ×1/2).
    pub chill_ms: u32,
    /// 3b (coop) : slot du dernier héros ayant infligé des dégâts à ce
    /// monstre — crédite l'XP au tueur. TRANSITOIRE (jamais sauvegardé —
    /// au chargement : 0 = héros du slot 0 ; tous les clients repartent du
    /// même état ⇒ déterminisme conservé).
    pub(crate) last_hit_by: u8,
    /// 16a (additif, transitoire non persisté — comme les DoT ci-dessus) :
    /// malédiction de nécromancien active ([`crate::auras::ActiveCurse`] —
    /// UNE par monstre, durée `auralen`, jamais persistée).
    pub curse: Option<crate::auras::ActiveCurse>,
}

/// 3b-1 (coop multi-héros) : UN JOUEUR = UN HÉROS. Le slot 0 est le héros
/// historique (rétro-compat totale : solo = `players.len() == 1`), les slots
/// 1.. sont les coéquipiers. Toutes les données d'un héros vivent ici :
/// entité, code de classe, gabarit, [`HeroData`] complet (stats, inventaire
/// 10×4, skills, chemin, swing…). Sauvegardé additivement (save v4 champ
/// `players`, cf. save.rs — les saves v3 solo se chargent = 1 player).
pub struct Player {
    /// Slot du joueur (= index dans [`World::players`], 0..=[`MAX_HEROES`]).
    pub(crate) slot: usize,
    /// Entité (position, facing, anim).
    pub(crate) ent: Entity,
    /// Code héros effectif (résolu via `heroes[CODE]`/`hero`, ex. "SOR").
    pub(crate) code: String,
    /// Gabarit de la classe (stats de départ, vitesse, arme).
    pub(crate) tpl: HeroTpl,
    /// Données de héros (même structure que l'ancien slot 0).
    pub(crate) data: HeroData,
    /// 3b-1 (déterminisme) : flux RNG INDÉPENDANT du joueur — graine dérivée
    /// de la graine du monde (`seed ^ GOLDEN_MIX × (slot+1)`), état persisté
    /// dans la save v4. Les jets PROPRES au joueur (toucher/dégâts de ses
    /// attaques, slot ≥ 1) consomment CE flux : les actions d'un joueur ne
    /// décalent jamais les jets d'un autre (risque désync n°1 du lockstep).
    /// Le slot 0 garde le flux MONDE historique (bit-exact solo).
    pub(crate) rng: Rng,
}

impl Player {
    /// Slot du joueur (= index dans `World::players`).
    pub fn slot(&self) -> usize {
        self.slot
    }
    /// Entité (lecture) : position, orientation, animation.
    pub fn ent(&self) -> &Entity {
        &self.ent
    }
    /// Code héros effectif ("BA", "SOR"…).
    pub fn code(&self) -> &str {
        &self.code
    }
    /// Stats lisibles (HUD) : vie/mana courants, xp, or, endurance…
    pub fn stats(&self) -> &HeroStats {
        &self.data.stats
    }
    /// Inventaire 10×4 du joueur.
    pub fn inventory(&self) -> &[Option<ItemDef>] {
        &self.data.inv
    }
    /// Compétences connues du joueur.
    pub fn skills(&self) -> &[HeroSkill] {
        &self.data.skills
    }
    /// Index du skill actif.
    pub fn active_skill(&self) -> usize {
        self.data.active_skill
    }
    /// Le joueur est-il mort (compte à rebours de respawn en cours) ?
    pub fn is_dead(&self) -> bool {
        self.data.dead_ms > 0
    }
    /// Déplacement en cours ?
    pub fn is_moving(&self) -> bool {
        !self.data.path.is_empty()
    }
    /// Course active ?
    pub fn is_running(&self) -> bool {
        self.data.run
    }
    /// Point d'apparition / respawn.
    pub fn spawn(&self) -> (f32, f32) {
        self.data.spawn
    }

    /// Construit un joueur neuf de classe `code` (résolue) — spawn `(sx, sy)`,
    /// stats de départ du gabarit, skills de départ, flux RNG dérivé de
    /// `world_seed_state` (état courant du flux monde — deterministe en
    /// lockstep : tous les clients dérivent le même flux).
    pub(crate) fn new_player(
        manifest: &serde_json::Value,
        code: &str,
        sx: f32,
        sy: f32,
        world_rng_state: u64,
        slot: usize,
    ) -> Player {
        let resolved = data::resolve_hero(manifest, code);
        let tpl = data::parse_hero_code(manifest, &resolved);
        let skills = starting_skills(&resolved);
        let mut stats = Self::fresh_stats(&tpl, &resolved);
        stats.skill =
            skills.get(default_active(&resolved)).map(|s| s.code.clone()).unwrap_or_default();
        stats.skill_level =
            skills.get(default_active(&resolved)).map(|s| s.level).unwrap_or(0);
        let stamina_milli = (tpl.stamina as i64) * 1000;
        let active = default_active(&resolved);
        Player {
            slot,
            ent: Entity { x: sx, y: sy, facing: 0, anim: AnimState::NU, anim_frame: 0 },
            code: resolved,
            tpl,
            data: HeroData {
                stats,
                path: Vec::new(),
                swing_ms: 0,
                swing_cast: false,
                dead_ms: 0,
                anim_ms: 0,
                run: false,
                mods: combat_mods::HitMods::default(),
                spawn: (sx, sy),
                inv: vec![None; INV_SLOTS],
                active_skill: active,
                skills,
                stamina_milli,
            },
            rng: Rng::new(world_rng_state ^ (GOLDEN_MIX.wrapping_mul(slot as u64 + 1))),
        }
    }

    /// Stats de départ d'un héros neuf depuis son gabarit (même arithmétique
    /// que l'ancien `init_hero` — facteur commun slot 0 / coéquipiers).
    pub(crate) fn fresh_stats(t: &HeroTpl, code: &str) -> HeroStats {
        let defense = t.dexterity / 4 + (t.shield_ac_min + t.shield_ac_max) / 2;
        HeroStats {
            level: 1,
            xp: 0,
            xp_to_next: xp_threshold(code, 2),
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
            skill: String::new(),
            skill_level: 0,
            stamina: t.stamina,
            max_stamina: t.stamina,
            skill_points: 0,
            stat_points: 0,
            // P26-1 : vitalité/énergie de départ de la classe — champs save
            // additifs (`#[serde(default)]` HeroStats).
            vitality: t.vitality,
            energy: t.energy,
            mana_frac_fourths: 0,
            res_all_pct: 0,
        }
    }
}

/// Monde de jeu complet (pur : aucune IO, aucune dépendance de rendu).
pub struct World {
    pub(crate) grid: Vec<Vec<bool>>,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) manifest: serde_json::Value,
    /// Clé du niveau courant dans le manifest (`"town"`, `"bloodmoor"`…) —
    /// persistée dans la sauvegarde v3 pour reprendre dans la bonne zone
    /// (warps multi-niveaux Town<->Blood Moor). La simulation n'y touche pas :
    /// c'est d2game qui la pose au chargement / switch de niveau.
    pub(crate) level_key: String,
    pub(crate) hero_tpl: HeroTpl,
    pub(crate) mon_tpls: Vec<MonTpl>,
    /// 3b-1 (coop multi-héros) : LES JOUEURS — 1 slot = 1 héros. Le slot 0
    /// est le héros historique ; solo = exactement 1 player (rétro-compat
    /// totale : les mondes se construisent à 1 player et les API historiques
    /// (`hero()`, `hero_stats()`…) sont la vue du joueur `local_slot`).
    pub(crate) players: Vec<Player>,
    /// 3b-1 : slot « local » (vue HUD/API legacy). Solo = 0. NON persisté,
    /// NON hashé, JAMAIS lu par la simulation (une vue client ne peut pas
    /// créer de désync lockstep).
    pub(crate) local_slot: usize,
    /// 3c-b (dialogue.rs) : session de dialogue PNJ active — le slot teneur
    /// vit dans `DialogueSession.slot`. TRANSITOIRE (jamais persistée ni
    /// hashée) ; ouverte, le monde est GELÉ pour TOUS les slots (décision
    /// style retail, cf. `tick_inputs` + dialogue.rs § coop).
    pub(crate) dialogue: Option<crate::dialogue::DialogueSession>,
    /// 3b-1 : file d'entrées PAR SLOT (FIFO, drainée par [`World::tick_queued`]
    /// — ordre slot croissant). Alignée sur `players` (push/pop). TRANSITOIRE
    /// (jamais sauvegardé) : n'y pousser que les entrées RELAYÉES (lockstep).
    pub(crate) input_queues: Vec<std::collections::VecDeque<PlayerInput>>,
    /// Entités monstres — même ordre / même longueur que `mon_dat`.
    pub(crate) mon_ent: Vec<Entity>,
    /// Données monstres — parallèle à `mon_ent`.
    pub(crate) mon_dat: Vec<MonData>,
    pub(crate) items: Vec<ItemOnGround>,
    /// Tables de loot TreasureClassEx v1.12 embarquées (`loot.rs`) ;
    /// `None` si la table est illisible ⇒ repli MVP dans `roll_drop`.
    pub(crate) loot: Option<loot::Loot>,
    /// Projectiles en vol (crate pure d2missiles, table réelle embarquée).
    pub(crate) missiles: MissileSystem,
    /// Waypoints W05/W19 (waypoint du niveau + slots activés, sauvegardé) —
    /// état et logique dans `waypoints.rs` (blocs additifs).
    pub(crate) wp: crate::waypoints::WaypointState,
    /// Corps au sol (J1, `corpse.rs`) : `Some` après une mort non encore
    /// récupérée — porte position + inventaire. Additif, sauvegardé.
    pub(crate) corpse: Option<crate::corpse::Corpse>,
    /// Intention de récupération du corps en cours (clic sur un corps hors
    /// de portée : le héros marche, grab auto à l'arrivée). TRANSITOIRE,
    /// jamais sauvegardé (après un load il suffit de recliquer).
    pub(crate) corpse_pending: bool,
    /// Stash (11c, `economy.rs`) : or déposé + grille 6×4 d'items. Save
    /// additive `gold_stash` (champ `#[serde(default)]`, v3 inchangée).
    pub(crate) gold_stash: crate::economy::Stash,
    pub(crate) rng: Rng,
    pub(crate) now_ms: u64,
    /// Difficulté courante (W08/K — Normal par défaut ; `set_difficulty`
    /// avant les spawns pour des monstres scalés NM/Enfer). Persistée en
    /// save (champ optionnel, rétro-compat v1/v2).
    pub(crate) difficulty: Difficulty,
    /// NPC de ville (W16, `npc.rs`) — roster persistant, non-attaquable.
    /// Vide par défaut ; `spawn_town_npcs()` (idempotent) le pose.
    pub(crate) npcs: Vec<NpcState>,
    /// Journal de quêtes (W13) branché optionnellement (`set_quest_log`) —
    /// les clics NPC y avancent q3/q4. `None` = aucun câblage forcé.
    pub(crate) quests: Option<QuestLog>,
    /// Événements produits HORS tick (`talk_to_npc`…) — flushés au début du
    /// tick suivant (le contrat d2rules émet toujours via `tick`).
    pub(crate) pending_events: Vec<GameEvent>,
    /// Mercenaire (H, `mercenary.rs`) — `None` jusqu'à `hire_merc` ; save
    /// additive `mercenary` (champ `#[serde(default)]`, save v3 inchangée).
    pub(crate) merc: Option<crate::mercenary::MercData>,
    /// Dernière cible du héros (TRANSIENT, jamais sauvegardé) — le
    /// mercenaire l'attaque en priorité (règle du brief H). `None` quand le
    /// monstre visé meurt/disparaît (les indices monstres se décalent à la
    /// suppression — même caveat REPORT.md §4).
    pub(crate) merc_target: Option<usize>,
    /// Événements différés (embauche/résurrection — API directes hors tick)
    /// : livrés en tête du PROCHAIN `tick`.
    pub(crate) deferred: Vec<GameEvent>,
    /// IA P1 monstres (ai_behaviors.rs, étape 4 RE d2w-re) — registre des
    /// cadavres + overlays Fallen (panique) / FallenShaman (résurrection),
    /// aligné aux indices `mon_ent`/`mon_dat`. Dérivé au chargement
    /// ([`crate::ai_behaviors::AiState::rebuild`]) ; jamais persisté
    /// (transitoire comme `corpse_pending`).
    pub(crate) ai: crate::ai_behaviors::AiState,
    /// Journal de quêtes Acte I SPEC (quests_actes.rs) — `None` par défaut ;
    /// save additive `quest_act1` (champ `#[serde(default)]`, v3 inchangée).
    /// Coexiste avec [`World::quests`] (journal W13) — l'intégrateur branche.
    pub(crate) quests_act1: Option<crate::quests_actes::ActQuests>,
    /// Journal de quêtes Actes II-III SPEC (quests_actes2.rs) — `None` par
    /// défaut ; save additive `quest_actes2` (champ `#[serde(default)]`,
    /// v3 inchangée). Même contrat que `quests_act1`.
    pub(crate) quests_actes2: Option<crate::quests_actes2::Act2Quests>,
    /// Consommables 11b (consumables.rs) — ceinture 4×4 (touches 1-4) +
    /// buffs temporisés des potions. Additif, sauvegardé (`consumables`,
    /// champ `#[serde(default)]`, v3 inchangée).
    pub(crate) consum: crate::consumables::ConsumState,
    /// Journal de quêtes Actes IV-V SPEC (quests_actes45.rs) — `None` par
    /// défaut ; save additive `quest_actes45` (champ `#[serde(default)]`,
    /// v3 inchangée). Même contrat que `quests_act1`/`quests_actes2`.
    pub(crate) quests_actes45: Option<crate::quests_actes45::Act45Quests>,
    /// Journal des BOSS FINAUX (quests_finale.rs, étape 10b — Diablo,
    /// vagues du trône, Baal) — `None` par défaut ; save additive
    /// `quest_finale` (champ `#[serde(default)]`, v3 inchangée). Même
    /// contrat que `quests_actes45` (coéquipier 10c, qui consomme les kills
    /// diablo/baalcrab sans les spawner) ; la mort de Baal émet
    /// [`GameEvent::GameWon`].
    pub(crate) quests_finale: Option<crate::quests_finale::QuestsFinale>,
    /// Auras & malédictions 16a (`auras.rs`) — aura de paladin ACTIVE
    /// (transitoire, jamais persistée) + accumulateur de pulse. Additif :
    /// save v3 inchangée (vides au chargement).
    pub(crate) auras: crate::auras::AuraState,
    /// Étape 16c (assassin_druid.rs) — pièges/combos Assassine + formes
    /// Druide. Additif, TRANSITOIRE (jamais persisté, comme `ai`) : les
    /// événements sortent par [`World::drain_ad_events`].
    pub(crate) ad: crate::assassin_druid::AdState,
    /// Invocations 16b (`minions.rs`, PLAN-FIDÉLITÉ) — summons necro/druid,
    /// registre de cadavres invocables, missiles des mages. Additif,
    /// TRANSITOIRE : JAMAIS persisté (retail — les invocations ne survivent
    /// ni au save/load ni à un changement de niveau).
    pub(crate) minions: crate::minions::Minions,
    /// 3b-1 (coop) : propriétaire de chaque missile héros en vol —
    /// `(id missile, slot)` ; crédite toucher/dégâts/XP au bon joueur.
    /// Remappé de façon déterministe par [`World::remove_player`].
    /// TRANSITOIRE (jamais sauvegardé — les missiles non plus) ; absent =
    /// slot 0 (missiles merc/invocations/monstres, propriétaires historiques).
    pub(crate) missile_owner: Vec<(u32, u8)>,
}


/// 3b-1 : jet RNG d'ATTAQUE du joueur `slot` — slot ≥ 1 : le flux RNG
/// INDÉPENDANT du joueur ([`Player.rng`], graine dérivée par slot) ; slot 0 :
/// le flux MONDE historique (bit-exact solo). Les jets propres à un joueur ne
/// dépendent donc que de SES actions (risque désync n°1 du lockstep contenu).
pub(crate) fn hero_roll_below(w: &mut World, slot: usize, n: u32) -> u32 {
    if slot == 0 {
        w.rng.below(n)
    } else {
        w.players[slot].rng.below(n)
    }
}

/// 3b-1 : jet RNG de DÉGÂTS du joueur `slot` (même routage que
/// [`hero_roll_below`]).
pub(crate) fn hero_roll_range(w: &mut World, slot: usize, a: u32, b: u32) -> u32 {
    if slot == 0 {
        w.rng.range_u32(a, b)
    } else {
        w.players[slot].rng.range_u32(a, b)
    }
}

///Chance de toucher : `200×TH/(TH+AC)` bornée [5, 95] (GAME-DATA.md §3).
pub(crate) fn hit_chance(th: u32, ac: u32) -> u32 {
    let c = 200 * th / (th + ac.max(1));
    c.clamp(5, 95)
}

/// Distance euclidienne.
pub(crate) fn dist(dx: f32, dy: f32) -> f32 {
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
pub(crate) fn set_anim(ent: &mut Entity, s: AnimState, anim_ms: &mut u32) {
    if ent.anim != s {
        ent.anim = s;
        *anim_ms = 0;
        ent.anim_frame = 0;
    }
}

/// Avance l'horloge d'anim et met à jour la frame (boucle ou clamp sur DD/A1).
pub(crate) fn advance_anim(ent: &mut Entity, anim_ms: &mut u32, dt: u32, looping: bool) {
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
pub(crate) fn advance_along_path(
    ent: &mut Entity,
    path: &mut Vec<(f32, f32)>,
    speed: f32,
    dt_ms: u32,
) -> bool {
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

/// Emprunt séparé (entité, données) du joueur `slot` — 3b-1 : `players[slot]`.
/// Deux emprunts DISTINCTS des champs `ent`/`data` du même élément (le borrow
/// checker sépare les champs) — utilisé par `update_hero_slot`/
/// `hero_attack_slot`/etc.
macro_rules! hero_parts {
    ($self:ident, $slot:expr, $ent:ident, $data:ident) => {
        let p = $self.players.get_mut($slot).expect("slot joueur valide");
        let ($ent, $data): (&mut Entity, &mut HeroData) = (&mut p.ent, &mut p.data);
    };
}


// ---------------------------------------------------------------------------
// DÉPENSE DES POINTS DE CARACTÉRISTIQUES (P26-1 — A2/L7) : types purs +
// tables `patch_charstats.txt` (colonnes « in fourths » absentes du manifest).
// L'endpoint [`World::spend_stat`] câble ces formules sur l'état joueur.
// ---------------------------------------------------------------------------

/// Plafond de DÉPENSE d'un attribut de base — encodage 10 bits du .d2s
/// (str/dex/ene/vit, stat ids 0-3, `csv_bits_max`) : au-delà, l'export
/// .d2s sature. Inatteignable en jeu (5×98 + base ≈ 540 au niveau 99) :
/// garde-fou d'export/interop.
pub const ATTR_CAP: u32 = 1023;

/// Une caractéristique dépensable ([`World::spend_stat`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeroStat {
    /// Force (`str`) — dégâts `×(1+str/100)` au coup, requis d'items.
    Str,
    /// Dextérité (`dex`) — défense `dex/4` + toucher.
    Dex,
    /// Vitalité (`vit`) — vie max + endurance max.
    Vit,
    /// Énergie (`ene`) — mana max.
    Ene,
}

impl HeroStat {
    /// Code canonique (`"str"|"dex"|"vit"|"ene"`) — clé JSON/UI.
    pub fn code(self) -> &'static str {
        match self {
            HeroStat::Str => "str",
            HeroStat::Dex => "dex",
            HeroStat::Vit => "vit",
            HeroStat::Ene => "ene",
        }
    }
}

/// Code canonique ou nom long EN (`"strength"|"dexterity"|"vitality"|
/// "energy"`) → [`HeroStat`].
fn parse_hero_stat(s: &str) -> Option<HeroStat> {
    match s.trim().to_ascii_lowercase().as_str() {
        "str" | "strength" => Some(HeroStat::Str),
        "dex" | "dexterity" => Some(HeroStat::Dex),
        "vit" | "vitality" => Some(HeroStat::Vit),
        "ene" | "energy" => Some(HeroStat::Ene),
        _ => None,
    }
}

/// Résultat d'une dépense acceptée ([`World::spend_stat`]) — les gains
/// dérivés affichables (flash UI, tests golden).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StatSpent {
    /// Caractéristique dépensée.
    pub stat: HeroStat,
    /// Points dépensés (coût 1/point).
    pub points: u32,
    /// Nouvelle valeur de l'attribut.
    pub value: u32,
    /// Points de caractéristique restants.
    pub remaining: u32,
    /// Vie max gagnée (`vit` seulement).
    pub life_gain: u32,
    /// Mana max gagné (`ene` seulement).
    pub mana_gain: u32,
    /// Endurance max gagnée (`vit` seulement).
    pub stamina_gain: u32,
    /// Défense gagnée (`dex` seulement).
    pub defense_gain: u32,
    /// Toucher gagné (`dex` seulement).
    pub to_hit_gain: u32,
}

/// Erreurs de dépense ([`World::spend_stat`]) — état INCHANGÉ sur `Err`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatSpendError {
    /// Slot joueur hors table.
    SlotInvalide,
    /// Stat inconnue (`str`/`dex`/`vit`/`ene` attendus).
    StatInconnue(String),
    /// Dépense de 0 point (refusée — aucun effet, même contrat retail).
    AucunPoint,
    /// Pas assez de points non dépensés.
    PasAssezDePoints {
        /// Points réellement disponibles.
        disponibles: u32,
    },
    /// La dépense dépasserait [`ATTR_CAP`] (1023, encodage .d2s).
    PlafondAttribut {
        /// Plafond refusé.
        plafond: u32,
    },
}

impl std::fmt::Display for StatSpendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StatSpendError::SlotInvalide => write!(f, "slot joueur invalide"),
            StatSpendError::StatInconnue(s) => {
                write!(f, "caractéristique inconnue : {s} (str/dex/vit/ene)")
            }
            StatSpendError::AucunPoint => write!(f, "aucun point à dépenser"),
            StatSpendError::PasAssezDePoints { disponibles } => {
                write!(f, "pas assez de points ({disponibles} disponibles)")
            }
            StatSpendError::PlafondAttribut { plafond } => {
                write!(f, "plafond d'attribut atteint ({plafond})")
            }
        }
    }
}

impl std::error::Error for StatSpendError {}

/// Lignes réelles `patch_charstats.txt` v1.12 (même source que
/// `save_d2s::CHARSTATS`) — colonnes de CROISSANCE absentes du manifest.
const CHARSTATS_GROWTH: &str = include_str!("../../../../assets/patch_charstats.txt");

/// `StaminaPerVitality` EN QUARTS de la ligne de classe (colonne 23 —
/// « in fourths » : BA 4 ⇒ +1 endurance/point de vitalité, AS 5 ⇒ +1,25).
/// Défaut 4 (valeur des 6 autres classes) si la ligne manque.
fn charstats_stamina_per_vit_fourths(hero_code: &str) -> u32 {
    let want = match crate::skills_hero::class_code(hero_code).as_str() {
        "ama" => "Amazon",
        "sor" => "Sorceress",
        "nec" => "Necromancer",
        "pal" => "Paladin",
        "bar" => "Barbarian",
        "dru" => "Druid",
        "ass" => "Assassin",
        _ => "Barbarian",
    };
    for line in CHARSTATS_GROWTH.lines() {
        let mut cols = line.split('\t');
        if !cols.next().unwrap_or("").trim().eq_ignore_ascii_case(want) {
            continue;
        }
        if let Some(v) = cols.nth(21).and_then(|c| c.trim().parse::<u32>().ok()) {
            return v.max(1);
        }
    }
    4
}

/// 3b-1 : hachage d'un joueur (entité + données + flux RNG) — facteur commun
/// de [`World::state_hash`] et [`World::state_hash_slot`].
fn hash_player<H: std::hash::Hasher>(h: &mut H, p: &Player) {    use std::hash::Hash;
    p.slot.hash(h);
    hash_entity(h, &p.ent);
    p.code.hash(h);
    let d = &p.data;
    d.stats.level.hash(h);
    d.stats.xp.hash(h);
    d.stats.xp_to_next.hash(h);
    d.stats.life.hash(h);
    d.stats.max_life.hash(h);
    d.stats.mana.hash(h);
    d.stats.max_mana.hash(h);
    d.stats.strength.hash(h);
    d.stats.dexterity.hash(h);
    d.stats.defense.hash(h);
    d.stats.to_hit.hash(h);
    d.stats.gold.hash(h);
    d.stats.walk_speed.to_bits().hash(h);
    d.stats.stamina.hash(h);
    d.stats.max_stamina.hash(h);
    d.stats.skill_points.hash(h);
    d.stats.stat_points.hash(h);
    d.stats.vitality.hash(h);
    d.stats.energy.hash(h);
    d.stats.mana_frac_fourths.hash(h);
    d.stats.res_all_pct.hash(h);
    d.stats.skill.hash(h);
    d.stats.skill_level.hash(h);    d.swing_ms.hash(h);
    d.swing_cast.hash(h);
    d.dead_ms.hash(h);
    d.run.hash(h);
    for wp in &d.path {
        wp.0.to_bits().hash(h);
        wp.1.to_bits().hash(h);
    }
    d.spawn.0.to_bits().hash(h);
    d.spawn.1.to_bits().hash(h);
    d.active_skill.hash(h);
    for s in &d.skills {
        s.code.hash(h);
        s.level.hash(h);
    }
    for it in &d.inv {
        match it {
            Some(def) => {
                1u8.hash(h);
                def.code.hash(h);
                def.amount.hash(h);
            }
            None => 0u8.hash(h),
        }
    }
    p.rng.s.hash(h);
}

/// 3b-1 : hachage d'une entité (position bits, facing, anim, frame).
fn hash_entity<H: std::hash::Hasher>(h: &mut H, e: &Entity) {
    use std::hash::Hash;
    e.x.to_bits().hash(h);
    e.y.to_bits().hash(h);
    e.facing.hash(h);
    e.anim.hash(h);
    e.anim_frame.hash(h);
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
            level_key: DEFAULT_LEVEL_KEY.to_string(),
            manifest: manifest.clone(),
            hero_tpl: data::parse_hero_code(&manifest, &hero_code),
            mon_tpls: data::parse_monsters(&manifest),
            loot: loot::build(),
            mon_ent: Vec::new(),
            mon_dat: Vec::new(),
            items: Vec::new(),
            missiles: MissileSystem::with_seed(seed ^ 0x4D49_5353_1A4E_0001),
            wp: crate::waypoints::WaypointState::default(),
            corpse: None,      // J1 : aucun corps à la construction
            corpse_pending: false, // J1 : transitoire
            gold_stash: crate::economy::Stash::new(), // 11c : stash vide 6×4
            rng: Rng::new(seed),
            now_ms: 0,
            difficulty: Difficulty::default(),
            npcs: Vec::new(),
            quests: None,
            pending_events: Vec::new(),
            merc: None,
            merc_target: None,
            deferred: Vec::new(),
            ai: crate::ai_behaviors::AiState::default(),
            quests_act1: None,
            quests_actes2: None,
            quests_actes45: None,
            quests_finale: None,
            ad: crate::assassin_druid::AdState::default(),
            consum: crate::consumables::ConsumState::default(),
            // 16a : aucune aura active à la construction (transitoire).
            auras: crate::auras::AuraState::default(),
            // 16b : invocations vides, missiles seedés (déterminisme).
            minions: crate::minions::Minions::with_seed(seed),
            // 3b-1 : UN joueur (le héros historique, slot 0) — les mondes
            // multi-joueurs passent par [`World::add_player`].
            players: Vec::new(),
            local_slot: 0,
            // 3c-b : aucune session de dialogue à la construction (transitoire).
            dialogue: None,
            input_queues: Vec::new(),
            missile_owner: Vec::new(),
        };
        let mut world = world;
        // 3b-1 : le slot 0 est créé via le chemin unique des joueurs.
        let p = Player::new_player(&world.manifest, &hero_code, 0.0, 0.0, seed, 0);
        world.players.push(p);
        world.input_queues.push(std::collections::VecDeque::new());
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
        self.players[0].ent.x = sx;
        self.players[0].ent.y = sy;
        self.players[0].ent.facing = 0;
        let (skill, skill_level) = self.players[0]
            .data
            .skills
            .get(self.players[0].data.active_skill)
            .map(|s| (s.code.clone(), s.level))
            .unwrap_or_else(|| (String::new(), 0));
        self.players[0].data.stats = HeroStats {
            level: 1,
            xp: 0,
            xp_to_next: xp_threshold(&self.players[0].code, 2),
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
            skill,
            skill_level,
            // EXP-H2 : endurance pleine au gabarit (manifest `stats.stamina`).
            stamina: t.stamina,
            max_stamina: t.stamina,
            // ARBRE COMPÉTENCES (C2) : aucun point au démarrage (1/niveau +
            // quêtes) — champ save additif (`#[serde(default)]` HeroStats).
            skill_points: 0,
            // Étape 8d : caractéristiques de quêtes (Lam Esen A3 +5) —
            // champ save additif (`#[serde(default)]` HeroStats).
            stat_points: 0,
            // P26-1 : vitalité/énergie de départ de la classe (dépense de
            // points) — champs save additifs (`#[serde(default)]`).
            vitality: t.vitality,
            energy: t.energy,
            mana_frac_fourths: 0,
            // Étape 19b : bonus résistances de quêtes (Prison de glace +10 %
            // par difficulté) — champ save additif (`#[serde(default)]`).
            res_all_pct: 0,
        };
        self.players[0].data.stamina_milli = (t.stamina as i64) * 1000;
        self.players[0].data.spawn = (sx, sy);
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

    /// Gabarit d'un bloc monstre/boss du manifest (mêmes champs et défauts que
    /// `data::parse_monsters`) — utilisé pour les boss (`manifest.bosses`, W11)
    /// qui ne vivent PAS dans `monsters` (pas d'auto-spawn).
    fn mon_tpl_from_block(m: &serde_json::Value) -> MonTpl {
        let at = |p: &str, d: u32| -> u32 {
            m.pointer(p).and_then(|x| x.as_u64()).map(|x| x as u32).unwrap_or(d)
        };
        let atf = |p: &str, d: f32| -> f32 {
            m.pointer(p).and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(d)
        };
        let ats = |p: &str, d: &str| -> String {
            m.pointer(p).and_then(|x| x.as_str()).unwrap_or(d).to_string()
        };
        MonTpl {
            code: ats("/code", "???"),
            name: ats("/name", "???"),
            level: at("/stats/level", 1).max(1),
            hp_min: at("/stats/hpMin", 21),
            hp_max: at("/stats/hp", 61).max(at("/stats/hpMin", 21)),
            dmg_min: at("/stats/dmgMin", 51),
            dmg_max: at("/stats/dmgMax", 101).max(at("/stats/dmgMin", 51)),
            exp: at("/stats/exp", 61),
            speed: atf("/stats/speed", 3.0),
            treasure_class: ats("/stats/treasureClass", ""),
        }
    }

    /// Résout l'index de gabarit pour `code` : table `mon_tpls` d'abord, sinon
    /// bloc `manifest.bosses` correspondant (ajouté paresseusement à la table
    /// — les boss n'altèrent ni l'auto-spawn ni les spawns manifest existants).
    fn resolve_tpl_index(&mut self, code: &str) -> Option<usize> {
        if let Some(i) = self.mon_tpls.iter().position(|t| t.code == code) {
            return Some(i);
        }
        let bosses = self.manifest.get("bosses")?.as_array()?;
        // 13a : un bloc boss est repéré par son `code` sprite (« AN », « CR »)
        // OU par son `id` monstats (« andariel », « bloodraven ») — les deux
        // champs sont portés par le manifest (le sprite code n'est pas
        // unique : Blood Raven et Corrupt Rogue 3 partagent « CR »). Le
        // spawn par id monstats (journals/events) devient possible sans
        // toucher aux spawns sprite existants.
        let block = bosses
            .iter()
            .find(|b| {
                ["code", "id"].iter().any(|k| {
                    b.get(k).and_then(|c| c.as_str()).map(|c| c.eq_ignore_ascii_case(code)).unwrap_or(false)
                })
            })?;
        let tpl = Self::mon_tpl_from_block(block);
        self.mon_tpls.push(tpl);
        Some(self.mon_tpls.len() - 1)
    }

    /// Spawn un monstre du gabarit `tpl_index` à une position continue donnée.
    fn spawn_monster_at(&mut self, tpl_index: usize, x: f32, y: f32) {
        self.spawn_monster_at_rule(tpl_index, x, y, SpawnRule::default());
    }

    /// Comme [`Self::spawn_monster_at`] avec les facteurs explicites
    /// [`SpawnRule`] (MonLvl réel + boosts champion/unique + TC dédiée).
    /// `pub(crate)` : aussi appelé par ai_behaviors (résurrection — relance
    /// du même gabarit sur la tuile du cadavre).
    pub(crate) fn spawn_monster_at_rule(&mut self, tpl_index: usize, x: f32, y: f32, rule: SpawnRule) {
        let t = &self.mon_tpls[tpl_index];
        // HP effectifs : rand(minHP..maxHP) × hp%/100, min 1
        let raw = self.rng.range_u32(t.hp_min, t.hp_max);
        let hp = ((raw.saturating_mul(rule.hp_pct)) / 100).max(1);
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
            dmg_min: ((t.dmg_min.saturating_mul(rule.dmg_pct)) / 100).max(1),
            dmg_max: ((t.dmg_max.saturating_mul(rule.dmg_pct)) / 100).max(1),
            th: (rule.th_pct.max(1)),
            ac: rule.ac,
            xp: (t.exp.saturating_mul(rule.xp_pct)) / 100,
            speed: t.speed,
            kind: rule.kind,
            tc: rule.treasure_class,
            res: rule.res,
            // 12c : aucun DoT/refroidissement à l'apparition.
            wounds: None,
            poison: None,
            chill_ms: 0,
            // 3b : aucun toucheur connu au spawn (XP créditée au slot 0).
            last_hit_by: 0,
            // 16a : aucune malédiction à l'apparition (transitoire).
            curse: None,
        };
        self.mon_ent.push(ent);
        self.mon_dat.push(dat);
        // IA P1 (ai_behaviors) : aligne l'extension AI (archétype dérivé du
        // code gabarit — FA/FS = Fallen/FallenShaman, cf. family_of).
        crate::ai_behaviors::on_spawned(self);
    }

    // ------------------------------------------------------------------
    // API contrat
    // ------------------------------------------------------------------

    /// Un pas de simulation. `dt_ms` est tronqué à 100 ms. Renvoie les événements
    /// du tick dans l'ordre d'émission (les événements différés d'API directes —
    /// embauche/résurrection du mercenaire — arrivent en tête).
    /// 3b-1 : équivalent à [`Self::tick_inputs`] avec l'entrée (s'il y en a
    /// une) routée au joueur LOCAL ([`World::local_slot`] — solo : slot 0,
    /// chemin historique strictement identique ; coop : d2game passe par les
    /// files `queue_player_input`/`tick_queued` ou `tick_inputs`).
    pub fn tick(&mut self, dt_ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent> {
        match input {
            Some(i) => {
                let ls = self.local_slot;
                self.tick_inputs(dt_ms, &[(ls, i)])
            }
            None => self.tick_inputs(dt_ms, &[]),
        }
    }

    /// 3b (coop multi-héros) : un pas de simulation avec PLUSIEURS entrées,
    /// chacune routée au héros de son slot (`(slot, entrée)` ; slot 0 = héros
    /// historique, 1.. = coéquipiers). Les entrées sont traitées dans l'ordre
    /// du tableau (déterminisme lockstep : chaque client passe le MÊME
    /// tableau) ; une entrée hors bornes ou pour un héros mort est ignorée.
    pub fn tick_inputs(&mut self, dt_ms: u32, inputs: &[(usize, PlayerInput)]) -> Vec<GameEvent> {
        // Événements produits hors tick (clic NPC W16…) d'abord, ordre réel.
        let mut ev = std::mem::take(&mut self.pending_events);
        let mut deferred_ev = std::mem::take(&mut self.deferred);
        ev.extend(deferred_ev);
        // 3c-b (dialogue.rs) : session de dialogue active ⇒ monde GELÉ pour
        // TOUS les slots (décision style retail 22a, § coop : le monde est
        // partagé, une conversation à la fois — un gel par slot casserait le
        // determinisme lockstep). Ni héros, ni monstres, ni NPC, ni missiles,
        // ni horloge (`now_ms`) ; seuls les événements en attente (le
        // `NpcTalked` de l'ouverture, quêtes) sont flushés. Couvre `tick`,
        // `tick_slot` et `tick_queued` du pont d2game (tous passent ici).
        // `close_dialogue_slot` rend le tick.
        if self.dialogue.is_some() {
            return ev;
        }
        let dt = dt_ms.min(MAX_DT_MS);
        self.now_ms += dt as u64;
        for (slot, input) in inputs {
            if *slot >= self.hero_count() {
                continue; // slot inconnu : ignoré (jamais de panique réseau)
            }
            // un héros mort ne produit aucune entrée (flux historique slot 0)
            if self.hero_slot_dead(*slot).unwrap_or(true) {
                continue;
            }
            self.handle_input_slot(*slot, *input, &mut ev);
        }
        self.update_heroes(dt, &mut ev);
        self.update_monsters(dt, &mut ev);
        self.update_missiles(dt, &mut ev);
        crate::auras::tick(self, dt, &mut ev); // 16a : pulses d'aura (dégâts/soin/régén)
        self.update_waypoints(&mut ev); // W05 : activation au contact (waypoints.rs)
        self.update_npcs(dt); // W16 : patrouilles NPC de ville (npc.rs)
        crate::consumables::tick_buffs(self, dt); // 11b : buffs potions temporisés
        crate::mercenary::tick_mercenary(self, dt, &mut ev); // H : FSM mercenaire
        crate::mercenary::tick_merc_missiles(self, dt, &mut ev); // H : missiles du merc
        crate::assassin_druid::tick(self, dt); // 16c : pièges/combos/formes (events via drain_ad_events)
        crate::minions::tick_minions(self, dt, &mut ev); // 16b : invocations (IA compagnon + missiles mages)
        ev
    }

    /// Entité héros (lecture) — API historique : vue du joueur LOCAL
    /// ([`World::local_slot`], solo = slot 0, strictement identique).
    pub fn hero(&self) -> &Entity {
        &self.players[self.local_slot].ent
    }

    // ------------------------------------------------------------------
    // 3b-1 (coop multi-héros) — accesseurs par slot. `World.players` est le
    // stockage unique (1 slot = 1 [`Player`]) : plus aucune branche spéciale
    // slot 0. Les API historiques restent la vue du joueur local_slot.
    // ------------------------------------------------------------------

    /// Nombre de joueurs du monde — 1 hors coop, ≤ [`MAX_HEROES`].
    pub fn hero_count(&self) -> usize {
        self.players.len()
    }

    /// Le joueur `slot` est-il mort (compte à rebours de respawn en cours) ?
    /// `None` hors bornes.
    pub fn hero_slot_dead(&self, slot: usize) -> Option<bool> {
        self.players.get(slot).map(|p| p.data.dead_ms > 0)
    }

    /// Entité du joueur `slot` (lecture). `None` hors bornes.
    pub fn hero_slot_ent(&self, slot: usize) -> Option<&Entity> {
        self.players.get(slot).map(|p| &p.ent)
    }

    /// Stats du joueur `slot` (lecture). `None` hors bornes.
    pub fn hero_stats_slot(&self, slot: usize) -> Option<&HeroStats> {
        self.players.get(slot).map(|p| &p.data.stats)
    }

    /// Inventaire 10×4 du joueur `slot` (lecture). `None` hors bornes.
    pub fn inventory_slot(&self, slot: usize) -> Option<&[Option<ItemDef>]> {
        self.players.get(slot).map(|p| &p.data.inv[..])
    }

    /// Compétences du joueur `slot` (lecture). `None` hors bornes.
    pub fn hero_skills_slot(&self, slot: usize) -> Option<&[HeroSkill]> {
        self.players.get(slot).map(|p| &p.data.skills[..])
    }

    /// Index du skill actif du joueur `slot`. `None` hors bornes.
    pub fn active_skill_slot(&self, slot: usize) -> Option<usize> {
        self.players.get(slot).map(|p| p.data.active_skill)
    }

    /// Sélectionne le skill `index` du joueur `slot` — false si hors bornes.
    pub fn select_skill_slot(&mut self, slot: usize, index: usize) -> bool {
        let Some(data) = self.players.get_mut(slot).map(|p| &mut p.data) else {
            return false;
        };
        if index >= data.skills.len() {
            return false;
        }
        data.active_skill = index;
        let s = &data.skills[index];
        data.stats.skill = s.code.clone();
        data.stats.skill_level = s.level;
        true
    }

    /// Code héros effectif du slot `slot` ("BA", "SOR"…). `None` hors bornes.
    pub fn hero_code_slot(&self, slot: usize) -> Option<&str> {
        self.players.get(slot).map(|p| p.code.as_str())
    }

    /// Le joueur `slot` a-t-il un déplacement en cours ? `None` hors bornes.
    pub fn hero_slot_moving(&self, slot: usize) -> Option<bool> {
        self.players.get(slot).map(|p| !p.data.path.is_empty())
    }

    /// Le joueur `slot` court-il ? `None` hors bornes.
    pub fn hero_slot_running(&self, slot: usize) -> Option<bool> {
        self.players.get(slot).map(|p| p.data.run)
    }

    /// Position (f32) du joueur `slot`. `None` hors bornes.
    pub(crate) fn hero_slot_pos(&self, slot: usize) -> Option<(f32, f32)> {
        self.hero_slot_ent(slot).map(|e| (e.x, e.y))
    }

    /// Stats mutables du joueur `slot`. `None` hors bornes.
    pub(crate) fn hero_stats_slot_mut(&mut self, slot: usize) -> Option<&mut HeroStats> {
        self.players.get_mut(slot).map(|p| &mut p.data.stats)
    }

    /// Données mutables du joueur `slot` (entité + données séparées via
    /// macro `hero_parts!` — voir update_hero_slot). Cette variante simple
    /// ne donne que [`HeroData`].
    pub(crate) fn hero_data_slot_mut(&mut self, slot: usize) -> Option<&mut HeroData> {
        self.players.get_mut(slot).map(|p| &mut p.data)
    }

    /// Vue immuable du joueur `slot` (3b-1 — [`Player`]).
    pub fn player(&self, slot: usize) -> Option<&Player> {
        self.players.get(slot)
    }

    /// Vue multi-joueurs complète (ordre des slots).
    pub fn players(&self) -> &[Player] {
        &self.players
    }

    /// Slot « local » (vue HUD/API legacy). Solo = 0. Jamais lu par la
    /// simulation (une vue client ne peut pas créer de désync lockstep).
    pub fn local_slot(&self) -> usize {
        self.local_slot
    }

    /// Change le slot local (vue) — false si hors bornes.
    pub fn set_local_slot(&mut self, slot: usize) -> bool {
        if slot >= self.players.len() {
            return false;
        }
        self.local_slot = slot;
        true
    }

    /// Position du joueur VIVANT le plus proche de `(x, y)` (slots dans
    /// l'ordre, stricte `<` : à égalité le slot le plus bas gagne —
    /// déterminisme). Tous morts ⇒ position du slot 0 (les monstres
    /// repassent Idle de toute façon, cf. `all_heroes_dead`).
    pub(crate) fn nearest_living_hero_pos(&self, x: f32, y: f32) -> (f32, f32) {
        let mut best: Option<(f32, f32)> = None;
        let mut bd = f32::INFINITY;
        for slot in 0..self.hero_count() {
            if self.hero_slot_dead(slot).unwrap_or(true) {
                continue;
            }
            let (hx, hy) = self.hero_slot_pos(slot).unwrap_or((x, y));
            let d = dist(hx - x, hy - y);
            if d < bd {
                bd = d;
                best = Some((hx, hy));
            }
        }
        best.unwrap_or((self.players[0].ent.x, self.players[0].ent.y))
    }

    /// Slot du joueur VIVANT le plus proche de `(x, y)` (même règle que
    /// [`Self::nearest_living_hero_pos`] ; tous morts ⇒ slot 0).
    pub(crate) fn nearest_living_hero_slot(&self, x: f32, y: f32) -> usize {
        let mut best = 0usize;
        let mut bd = f32::INFINITY;
        for slot in 0..self.hero_count() {
            if self.hero_slot_dead(slot).unwrap_or(true) {
                continue;
            }
            let Some((hx, hy)) = self.hero_slot_pos(slot) else { continue };
            let d = dist(hx - x, hy - y);
            if d < bd {
                bd = d;
                best = slot;
            }
        }
        best
    }

    /// Tous les joueurs sont-ils morts ? (les monstres repassent Idle —
    /// flux historique mono-joueur : `dead_ms > 0`).
    pub(crate) fn all_heroes_dead(&self) -> bool {
        (0..self.hero_count()).all(|s| self.hero_slot_dead(s).unwrap_or(true))
    }

    /// 3b-1 : AJOUTE un joueur de classe `hero_code` au `slot` demandé
    /// (1..hero_count()). Le héros apparaît sur la première tuile marchable
    /// libre près du joueur précédent (pas de superposition), son spawn y est
    /// posé. false si slot invalide/déjà pris, monde plein ([`MAX_HEROES`])
    /// ou classe inconnue du manifest.
    pub fn add_player(&mut self, slot: usize, hero_code: &str) -> bool {
        if slot == 0 || slot > self.players.len() || self.players.len() >= MAX_HEROES {
            return false;
        }
        // résolution préalable (sans muter l'état si la classe est inconnue)
        let resolved = data::resolve_hero(&self.manifest, hero_code);
        if data::parse_hero_code_checked(&self.manifest, &resolved).is_none() {
            return false;
        }
        // ancre : le joueur précédent (slot-1) — tuile libre la plus proche.
        // Exclusion VIDE : à l'insertion, TOUS les joueurs actuels restent
        // dans le monde (ceux ≥ slot décalent) — la tuile doit être libre de
        // tout le monde (pas de superposition).
        let (ax, ay) = {
            let p = &self.players[slot - 1];
            (p.ent.x, p.ent.y)
        };
        let spot = self.free_tile_near_excluding(usize::MAX, ax.floor() as i32, ay.floor() as i32);
        let (sx, sy) = match spot {
            Some(s) => (s.0 as f32 + 0.5, s.1 as f32 + 0.5),
            None => (ax, ay),
        };
        // flux RNG dérivé de l'état COURANT du flux monde (déterministe en
        // lockstep : tous les clients exécutent le même appel au même état)
        let rng_state = self.rng.s;
        let p = Player::new_player(&self.manifest, &resolved, sx, sy, rng_state, slot);
        self.players.insert(slot, p);
        self.input_queues.insert(slot, std::collections::VecDeque::new());
        self.renumber_slots();
        true
    }

    /// 3b-1 : retire le joueur `slot` (≥ 1 — le slot 0 est le socle
    /// historique, non retirable). Les slots > `slot` DÉCALENT (-1) ;
    /// `missile_owner` et le crédit `last_hit_by` sont remappés de façon
    /// déterministe. false si slot invalide.
    pub fn remove_player(&mut self, slot: usize) -> bool {
        if slot == 0 || slot >= self.players.len() {
            return false;
        }
        self.players.remove(slot);
        self.input_queues.remove(slot);
        self.renumber_slots(); // invariant : players[i].slot == i
        // remappage déterministe des propriétés de missiles
        for (mid, owner) in self.missile_owner.iter_mut() {
            match *owner {
                s if s as usize == slot => *mid = u32::MAX, // marque « à retirer »
                s if (s as usize) > slot => *owner = s - 1,
                _ => {}
            }
        }
        self.missile_owner.retain(|(mid, _)| *mid != u32::MAX);
        // crédit `last_hit_by` des monstres (transitoire)
        for dat in self.mon_dat.iter_mut() {
            let s = dat.last_hit_by as usize;
            dat.last_hit_by = if s == slot {
                0 // propriétaire parti : crédit historique slot 0
            } else if s > slot {
                (s - 1) as u8
            } else {
                dat.last_hit_by
            };
        }
        if self.local_slot >= self.players.len() {
            self.local_slot = self.players.len() - 1;
        }
        // 3c-b (dialogue.rs) : la session de dialogue suit la compaction —
        // tenue par le partant ⇒ refermée (le monde gelé reprend, personne
        // n'est enfermé dans un verrou orphelin) ; tenue par un slot > slot
        // ⇒ son numéro DECALE (-1), comme `missile_owner` (remap
        // déterministe : le teneur reste le MÊME joueur après compaction).
        if let Some(s) = &mut self.dialogue {
            if s.slot == slot {
                self.dialogue = None;
            } else if s.slot > slot {
                s.slot -= 1;
            }
        }
        true
    }

    /// 3b-1 : file d'entrées du joueur `slot` — y pusher UNIQUEMENT les
    /// entrées RELAYÉES par le serveur lockstep (tous les clients poussent
    /// les mêmes). false si slot hors bornes.
    pub fn queue_player_input(&mut self, slot: usize, input: PlayerInput) -> bool {
        match self.input_queues.get_mut(slot) {
            Some(q) => {
                q.push_back(input);
                true
            }
            None => false,
        }
    }

    /// 3b-1 : un pas de simulation qui DRAINE toutes les files d'entrées
    /// (ordre slot croissant, FIFO dans chaque slot — déterministe) puis
    /// simule le tick. Alternative déterministe de [`Self::tick_inputs`] :
    /// choisir L'UN des deux styles par partie.
    pub fn tick_queued(&mut self, dt_ms: u32) -> Vec<GameEvent> {
        let batch: Vec<(usize, PlayerInput)> = self
            .input_queues
            .iter_mut()
            .enumerate()
            .flat_map(|(slot, q)| std::mem::take(q).into_iter().map(move |i| (slot, i)))
            .collect();
        self.tick_inputs(dt_ms, &batch)
    }

    /// 3b-1 : renumérote `slot` après insertion/retrait (invariant :
    /// `players[i].slot == i`).
    fn renumber_slots(&mut self) {
        for (i, p) in self.players.iter_mut().enumerate() {
            p.slot = i;
        }
    }

    /// Ajoute un coéquipier de classe `code` près de `(x, y)` (3b, coop) —
    /// la première tuile marchable LIBRE la plus proche est choisie (BFS en
    /// anneau depuis la tuile visée, tuiles des héros existants exclues),
    /// son point d'apparition y est posé. Renvoie le slot créé (≥ 1), ou
    /// `None` si le monde est plein ([`MAX_HEROES`]) ou la classe inconnue
    /// du manifest. Wrapper positionnel de [`Self::add_player`].
    pub fn add_cohero_near(&mut self, code: &str, x: f32, y: f32) -> Option<usize> {
        if self.hero_count() >= MAX_HEROES {
            return None;
        }
        // résolution préalable (None si classe inconnue, sans muter l'état)
        let resolved = data::resolve_hero(&self.manifest, code);
        if data::parse_hero_code_checked(&self.manifest, &resolved).is_none() {
            return None;
        }
        let slot = self.hero_count();
        // tuile la plus proche marchable ET sans héros dessus (anti-superposition)
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        let spot = self.free_tile_near_excluding(slot, tx, ty);
        let (sx, sy) = match spot {
            Some(s) => (s.0 as f32 + 0.5, s.1 as f32 + 0.5),
            None => (x, y),
        };
        let rng_state = self.rng.s;
        let p = Player::new_player(&self.manifest, &resolved, sx, sy, rng_state, slot);
        self.players.push(p);
        self.input_queues.push(std::collections::VecDeque::new());
        Some(slot)
    }

    /// Stats de départ d'un héros neuf depuis son gabarit (facteur commun
    /// slot 0 / coéquipiers — même arithmétique qu'`init_hero`).
    fn fresh_hero_stats(t: &HeroTpl, code: &str) -> HeroStats {
        let defense = t.dexterity / 4 + (t.shield_ac_min + t.shield_ac_max) / 2;
        HeroStats {
            level: 1,
            xp: 0,
            xp_to_next: xp_threshold(code, 2),
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
            skill: String::new(),
            skill_level: 0,
            stamina: t.stamina,
            max_stamina: t.stamina,
            skill_points: 0,
            stat_points: 0,
            // P26-1 : vitalité/énergie de départ de la classe — champs save
            // additifs (`#[serde(default)]` HeroStats).
            vitality: t.vitality,
            energy: t.energy,
            mana_frac_fourths: 0,
            res_all_pct: 0,
        }
    }

    /// Place l'entité du héros `slot` à une position sous-tuile (3b : arrivée
    /// de niveau coop). Position non vérifiée marchable (même contrat que
    /// [`Self::place_hero_at`]) ; facing/anim réinitialisés.
    pub fn place_hero_slot_at(&mut self, slot: usize, x: f32, y: f32) -> bool {
        if slot == 0 {
            self.place_hero_at(x, y);
            return true;
        }
        match self.players.get_mut(slot) {
            Some(c) => {
                c.ent.x = x;
                c.ent.y = y;
                c.ent.facing = 0;
                c.ent.anim = AnimState::NU;
                c.ent.anim_frame = 0;
                true
            }
            None => false,
        }
    }

    /// 3b : redéploie le coéquipier `slot` sur la tuile marchable libre la
    /// plus proche de `(x, y)` (tuiles des AUTRES héros exclues — pas de
    /// superposition) et y pose son point d'apparition (arrivée de niveau,
    /// warp coop). False si hors bornes/slot inconnu.
    pub fn place_hero_slot_near(&mut self, slot: usize, x: f32, y: f32) -> bool {
        if slot >= self.hero_count() {
            return false;
        }
        let (tx, ty) = (x.floor() as i32, y.floor() as i32);
        let spot = match self.free_tile_near_excluding(slot, tx, ty) {
            Some(s) => s,
            None => return false,
        };
        let (px, py) = (spot.0 as f32 + 0.5, spot.1 as f32 + 0.5);
        if let Some(c) = self.players.get_mut(slot) {
            c.ent.x = px;
            c.ent.y = py;
            c.ent.facing = 0;
            c.ent.anim = AnimState::NU;
            c.ent.anim_frame = 0;
            c.data.path.clear();
            c.data.spawn = (px, py);
            true
        } else {
            false
        }
    }

    /// 3b : BFS en anneau depuis `(tx, ty)` — première tuile marchable sans
    /// héros (le slot `exclude` est ignoré dans l'occupation : c'est LUI qu'on
    /// replace). Déterministe (ordre [`crate::path::DIRS`], files FIFO).
    pub(crate) fn free_tile_near_excluding(
        &self,
        exclude: usize,
        tx: i32,
        ty: i32,
    ) -> Option<(i32, i32)> {
        let mut occupied = std::collections::HashSet::new();
        for s in 0..self.hero_count() {
            if s == exclude {
                continue;
            }
            if let Some((hx, hy)) = self.hero_slot_pos(s) {
                occupied.insert((hx.floor() as i32, hy.floor() as i32));
            }
        }
        let mut seen = vec![false; (self.w * self.h) as usize];
        let mut queue = std::collections::VecDeque::new();
        let mut push = |queue: &mut std::collections::VecDeque<(i32, i32)>,
                        seen: &mut Vec<bool>,
                        p: (i32, i32)| {
            if p.0 >= 0 && p.1 >= 0 && p.0 < self.w && p.1 < self.h {
                let i = p.1 as usize * self.w as usize + p.0 as usize;
                if !seen[i] {
                    seen[i] = true;
                    queue.push_back(p);
                }
            }
        };
        push(&mut queue, &mut seen, (tx, ty));
        while let Some(p) = queue.pop_front() {
            if !self.grid[p.1 as usize][p.0 as usize] && !occupied.contains(&p) {
                return Some(p);
            }
            for (dx, dy) in crate::path::DIRS {
                push(&mut queue, &mut seen, (p.0 + dx, p.1 + dy));
            }
        }
        None
    }

    /// Point d'apparition du coéquipier `slot` — lecture (save/JSON).
    pub(crate) fn hero_slot_spawn(&self, slot: usize) -> Option<(f32, f32)> {
        if slot == 0 {
            Some(self.players[0].data.spawn)
        } else {
            self.players.get(slot).map(|p| p.data.spawn)
        }
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

    /// Missiles en vol (lecture — positions pour le rendu d2game).
    pub fn missiles(&self) -> &[d2missiles::Missile] {
        self.missiles.missiles()
    }

    /// Objets au sol (lecture).
    pub fn items(&self) -> &[ItemOnGround] {
        &self.items
    }

    /// Stats héros (lecture).
    pub fn hero_stats(&self) -> &HeroStats {
        &self.players[self.local_slot].data.stats
    }

    /// Compétences connues du héros (lecture ; vide = classe sans skill de départ).
    pub fn hero_skills(&self) -> &[HeroSkill] {
        &self.players[self.local_slot].data.skills
    }

    /// Compétence actuellement sélectionnée (lecture ; `None` si aucune).
    pub fn active_skill(&self) -> Option<&HeroSkill> {
        let p = &self.players[self.local_slot].data;
        p.skills.get(p.active_skill)
    }

    /// Sélectionne la compétence `index` — false si hors bornes (état inchangé).
    pub fn select_skill(&mut self, index: usize) -> bool {
        let ls = self.local_slot;
        self.select_skill_slot(ls, index)
    }

    // ------------------------------------------------------------------
    // ARBRE DE COMPÉTENCES (C2/C3/C4, 2026-10-23) — API additive pure,
    // validation via `skills_hero`/`skills_tree` (tables patch_skills.txt).
    // ------------------------------------------------------------------

    /// Points de skill non dépensés (C2 : +1/niveau via `apply_xp`,
    /// + récompenses de quêtes via [`World::grant_skill_points`]).
    /// 12c (additif) : pose les mods de coup du héros (deadly strike,
    /// crushing blow, open wounds, leech, durées poison/froid). TRANSITOIRE
    /// (jamais persisté — le branchement d'équipement viendra plus tard) :
    /// tests golden + futurs items. Sans appel, `HitMods::default()` — le
    /// pipeline garde l'arithmétique historique à l'identique.
    pub fn set_hero_combat_mods(&mut self, mods: combat_mods::HitMods) {
        self.players[self.local_slot].data.mods = mods;
    }

    pub fn unspent_skill_points(&self) -> u32 {
        self.players[self.local_slot].data.stats.skill_points
    }

    /// Accorde `n` points de skill non dépensés — hook des RÉCOMPENSES DE
    /// QUÊTES v1.12 : Den of Evil A1 +1 (`QuestReward::Unlock{flag:
    /// "skill_point"}`, quests.rs a1q1), Radament's Lair A2 +1, Izual A4 +2.
    /// L'intégration quêtes (W13) appelle ceci en consommant le flag.
    pub fn grant_skill_points(&mut self, n: u32) {
        let ls = self.local_slot;
        let p = &mut self.players[ls].data.stats;
        p.skill_points = p.skill_points.saturating_add(n);
    }

    /// Injecte des points de caractéristique HORS tick (récompenses de
    /// quêtes — le flux par niveau passe par [`Self::apply_xp_slot`],
    /// +`StatPerLevel`/niveau). Même convention que
    /// [`Self::grant_skill_points`].
    pub fn grant_stat_points(&mut self, n: u32) {
        let ls = self.local_slot;
        let p = &mut self.players[ls].data.stats;
        p.stat_points = p.stat_points.saturating_add(n);
    }

    /// Dépense `points` points de caractéristique (coût 1/point, v1.12) sur
    /// `stat` (`"str"|"dex"|"vit"|"ene"`, noms longs EN acceptés) du joueur
    /// `player_slot` — P26-1 (A2/L7). Formules RÉELLES
    /// `patch_charstats.txt` (colonnes « in fourths », manifest ÷4) :
    ///
    /// * `vit` → vie max +`LifePerVitality`×points (BA +4, SO/NE/DZ +2,
    ///   PA/AM/AS +3) ET endurance max +`StaminaPerVitality`×points (+1,
    ///   AS +1,25 — reste en quarts porté par `stamina_milli`, transitoire) ;
    ///   vie/endurance courantes suivent la montée du max (même contrat que
    ///   le level-up) ;
    /// * `ene` → mana max +`ManaPerMagic`×points (BA +1, SO/NE/DZ +2,
    ///   PA/AM +1,5 — cumul EXACT via [`HeroStats::mana_frac_fourths`]) ;
    ///   mana courant suit ;
    /// * `dex` → défense +`dex/4` (plancher par point, réel v1.12) et
    ///   toucher +`ToHitFactor`+dex (formule documentée du moteur) ;
    /// * `str` → rien de dérivé côté stats (dégâts `×(1+str/100)` évalués
    ///   au coup, requis d'items déjà gérés par `equipment.rs`).
    ///
    /// Plafonds : 0 point refusé, pas assez de points refusé, attribut
    /// plafonné à [`ATTR_CAP`] (1023 — encodage 10 bits du .d2s, jamais
    /// atteignable en jeu : garde-fou d'export). `Err` = raison exacte
    /// ([`StatSpendError`], affichable UI) ; état INCHANGÉ sur refus.
    pub fn spend_stat(
        &mut self,
        player_slot: usize,
        stat: &str,
        points: u32,
    ) -> Result<StatSpent, StatSpendError> {
        let which = parse_hero_stat(stat).ok_or_else(|| StatSpendError::StatInconnue(stat.trim().to_string()))?;
        if points == 0 {
            return Err(StatSpendError::AucunPoint);
        }
        let p = self.players.get(player_slot).ok_or(StatSpendError::SlotInvalide)?;
        let s = &p.data.stats;
        if points > s.stat_points {
            return Err(StatSpendError::PasAssezDePoints { disponibles: s.stat_points });
        }
        let (old_attr, cap_new) = match which {
            HeroStat::Str => (s.strength, s.strength.saturating_add(points)),
            HeroStat::Dex => (s.dexterity, s.dexterity.saturating_add(points)),
            HeroStat::Vit => (s.vitality, s.vitality.saturating_add(points)),
            HeroStat::Ene => (s.energy, s.energy.saturating_add(points)),
        };
        if cap_new > ATTR_CAP {
            return Err(StatSpendError::PlafondAttribut { plafond: ATTR_CAP });
        }
        let code = p.code.clone();
        let t = p.tpl.clone();
        let p = &mut self.players[player_slot];
        let s = &mut p.data.stats;
        let mut out = StatSpent {
            stat: which,
            points,
            value: cap_new,
            remaining: s.stat_points - points,
            life_gain: 0,
            mana_gain: 0,
            stamina_gain: 0,
            defense_gain: 0,
            to_hit_gain: 0,
        };
        s.stat_points = out.remaining;
        match which {
            HeroStat::Str => s.strength = cap_new,
            HeroStat::Dex => {
                // Défense = dex/4 + AC bouclier : seule la part dex/4 bouge
                // (plancher par point — 20→21 ne gagne rien, 20→24 gagne 1).
                let shield = s.defense.saturating_sub(old_attr / 4);
                let factor = s.to_hit.saturating_sub(old_attr);
                s.dexterity = cap_new;
                out.defense_gain = cap_new / 4 - old_attr / 4;
                s.defense = cap_new / 4 + shield;
                out.to_hit_gain = points;
                s.to_hit = cap_new + factor;
            }
            HeroStat::Vit => {
                s.vitality = cap_new;
                // Vie : LifePerVitality en quarts (16/12/8 — multiple de 4
                // pour les 7 classes v1.12 : gain toujours exact).
                let life = points * t.life_per_vitality_fourths;
                out.life_gain = life / 4;
                s.max_life = s.max_life.saturating_add(out.life_gain);
                s.life = s.life.saturating_add(out.life_gain);
                // Endurance : StaminaPerVitality en quarts (charstats col 23,
                // absent du manifest) — reste sous-unitaire porté par
                // `stamina_milli` (transitoire, recalé au chargement).
                let stam = points * charstats_stamina_per_vit_fourths(&code);
                out.stamina_gain = stam / 4;
                s.max_stamina = s.max_stamina.saturating_add(out.stamina_gain);
                let gain_milli = (out.stamina_gain * 1000 + (stam % 4) * 250) as i64;
                p.data.stamina_milli = (p.data.stamina_milli + gain_milli)
                    .min((s.max_stamina as i64) * 1000);
                s.stamina = (p.data.stamina_milli / 1000) as u32;
            }
            HeroStat::Ene => {
                s.energy = cap_new;
                // Mana : ManaPerMagic en quarts + reste porté entre dépenses
                // (cumul exact retail : PA/AM 1,5/point ⇒ +1,+3,+4,+6…).
                let total = s.mana_frac_fourths + points * t.mana_per_magic_fourths;
                out.mana_gain = total / 4;
                s.mana_frac_fourths = total % 4;
                s.max_mana = s.max_mana.saturating_add(out.mana_gain);
                s.mana = s.mana.saturating_add(out.mana_gain);
            }
        }
        Ok(out)
    }

    /// Injecte de l'XP au héros HORS tick (import d2s futurs, outils,
    /// garde-fous de tests) — MÊME chemin [`Self::apply_xp`] que les kills
    /// (paliers 1→`MaxLvl` de la table réelle, vie/mana/skill points par
    /// niveau) ; les [`GameEvent::HeroLeveled`] arrivent au prochain `tick`
    /// (contrat : tout événement passe par le tick).
    pub fn grant_xp(&mut self, xp: u32) {
        let mut ev = Vec::new();
        let ls = self.local_slot;
        self.apply_xp_slot(ls, xp, &mut ev);
        self.pending_events.extend(ev);
    }

    /// Apprend (ou monte d'un niveau) le skill `code` en dépensant 1 point —
    /// validation RÉELLE : classe du héros, prérequis `reqskill1-3`,
    /// `reqlevel`, plafond `maxlvl` (20). `Err` = raison exacte
    /// ([`SkillTreeError`], affichable UI arbre) ; état inchangé sur refus.
    /// Le skill appris devient sélectionnable via [`World::select_skill`]
    /// (index = position dans [`World::hero_skills`]).
    pub fn learn_skill(&mut self, code: &str) -> Result<u32, SkillTreeError> {
        let ls = self.local_slot;
        let (points, class, level_now) = {
            let p = &self.players[ls];
            (p.data.stats.skill_points, skills_hero::class_code(&p.code), p.data.stats.level)
        };
        let learned = {
            let p = &mut self.players[ls];
            skills_hero::learn(&mut p.data.skills, &class, level_now, code, points)
        }?;
        {
            let p = &mut self.players[ls];
            p.data.stats.skill_points = points.saturating_sub(1);
            // Skill ACTIF monté : rafraîchit le miroir HUD (stats.skill/_level),
            // même contrat que `select_skill`.
            if let Some(s) = p.data.skills.get(p.data.active_skill) {
                if s.code.eq_ignore_ascii_case(code) {
                    p.data.stats.skill = s.code.clone();
                    p.data.stats.skill_level = s.level;
                }
            }
        }
        Ok(learned)
    }

    /// Inventaire (lecture) : 40 slots 10×4.
    pub fn inventory(&self) -> &[Option<ItemDef>] {
        &self.players[self.local_slot].data.inv
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
        self.spawn_monster_rule(code, x, y, SpawnRule::default())
    }

    /// Comme [`World::spawn_monster`] avec les facteurs [`SpawnRule`] réels
    /// (MonLvl.txt ligne du monstre, boosts monumod champion/unique, TC
    /// champion/unique/boss). Code inconnu du manifest (`monsters`+`bosses`)
    /// ou tuile bloquée ⇒ false, état inchangé.
    pub fn spawn_monster_rule(&mut self, code: &str, x: i32, y: i32, rule: SpawnRule) -> bool {
        let Some(i) = self.resolve_tpl_index(code) else {
            return false;
        };
        if !self.is_walkable(x, y) {
            return false;
        }
        self.spawn_monster_at_rule(i, x as f32 + 0.5, y as f32 + 0.5, rule);
        true
    }

    /// Résolution pour les BOSS SCRIPTÉS : le bloc `manifest.bosses` (stats
    /// réelles — Duriel 3995 PV etc.) PRIME sur une entrée homonyme de la
    /// table data-driven (un tpl générique du même code la masquerait).
    /// Repli : table data-driven.
    pub(crate) fn resolve_tpl_boss(&mut self, code: &str) -> Option<usize> {
        if let Some(bosses) = self.manifest.get("bosses").and_then(|b| b.as_array()) {
            let block = bosses
                .iter()
                .find(|b| {
                    ["code", "id"].iter().any(|k| {
                        b.get(k)
                            .and_then(|c| c.as_str())
                            .map(|c| c.eq_ignore_ascii_case(code))
                            .unwrap_or(false)
                    })
                });
            if let Some(block) = block {
                let tpl = Self::mon_tpl_from_block(block);
                self.mon_tpls.push(tpl);
                return Some(self.mon_tpls.len() - 1);
            }
        }
        self.mon_tpls.iter().position(|t| t.code == code)
    }

    /// Les TABLES font foi : force les stats du tpl boss depuis
    /// `BossStats` (le manifest régressé peut perdre le sous-objet stats —
    /// cf. étape 15a, blocs régénérés sprites sans `/stats`).
    pub(crate) fn override_tpl_boss_stats(&mut self, i: usize, level: u32, hp: u32, exp: u32) {
        let Some(t) = self.mon_tpls.get_mut(i) else { return };
        t.level = level.max(1);
        t.hp_min = hp.max(1);
        t.hp_max = t.hp_min;
        t.exp = exp;
    }

    /// Comme [`Self::spawn_monster_rule`] mais avec la résolution boss
    /// (manifest prioritaire) — utilisé par les spawns scriptés de quêtes.
    pub(crate) fn spawn_monster_rule_boss(&mut self, code: &str, x: i32, y: i32, rule: SpawnRule) -> bool {
        let Some(i) = self.resolve_tpl_boss(code) else {
            return false;
        };
        if !self.is_walkable(x, y) {
            return false;
        }
        self.spawn_monster_at_rule(i, x as f32 + 0.5, y as f32 + 0.5, rule);
        true
    }

    /// Rang du monstre `index` ([`MonKind`]) — teinte/labels d2game, events
    /// quête. `None` hors bornes.
    pub fn monster_kind(&self, index: usize) -> Option<MonKind> {
        self.mon_dat.get(index).map(|d| d.kind)
    }

    /// PV courants/max du monstre `index` — EXP-H6 (survol : barre de vie),
    /// additif pour d2game. `None` hors bornes.
    pub fn monster_hp(&self, index: usize) -> Option<(u32, u32)> {
        self.mon_dat.get(index).map(|d| (d.hp, d.max_hp))
    }

    /// Retire tous les monstres (vivants ou mourants) — tests / placement fin.
    pub fn clear_monsters(&mut self) {
        self.mon_ent.clear();
        self.mon_dat.clear();
        self.ai = crate::ai_behaviors::AiState::default(); // IA P1 : ext + cadavres
    }

    /// Déplace le point d'apparition du héros (tuile marchable requise).
    pub fn set_hero_spawn(&mut self, x: i32, y: i32) -> bool {
        if !self.is_walkable(x, y) {
            return false;
        }
        self.players[self.local_slot].data.spawn = (x as f32 + 0.5, y as f32 + 0.5);
        true
    }

    /// FIX-CAMPES : place l'entité héros à une position sous-tuile — boot
    /// d'un monde neuf au camp (la position initiale sinon vient du centre
    /// via `pick_auto_spawns`, en pleine forêt sur la ville 1).
    pub fn place_hero_at(&mut self, x: f32, y: f32) {
        let ent = &mut self.players[self.local_slot].ent;
        ent.x = x;
        ent.y = y;
        ent.facing = 0;
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

    // ------------------------------------------------------------------
    // 3b-1 (déterminisme lockstep) — hash d'état GLOBAL + hash PAR SLOT.
    // Hachage manuel (DefaultHasher à clés fixes ⇒ déterministe dans le
    // même binaire), f32 via to_bits(). Couvre tout état qui influence la
    // simulation future ; les vues client (`local_slot`) et les transitoires
    // non déterminants (files d'entrées) sont exclus.
    // ------------------------------------------------------------------

    /// Hash d'état GLOBAL — anti-désync lockstep : tous les clients doivent
    /// observer la même valeur après chaque tick. Couvre joueurs, monstres,
    /// items, missiles, mercenaire, RNG monde, horloge.
    pub fn state_hash(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::hash::DefaultHasher::new();
        0x3B31u64.hash(&mut h); // étiquette de schéma (durabilité)
        self.now_ms.hash(&mut h);
        self.rng.s.hash(&mut h);
        self.level_key.hash(&mut h);
        for p in &self.players {
            hash_player(&mut h, p);
        }
        for (e, d) in self.mon_ent.iter().zip(self.mon_dat.iter()) {
            hash_entity(&mut h, e);
            d.tpl.hash(&mut h);
            d.hp.hash(&mut h);
            d.max_hp.hash(&mut h);
            d.state.hash(&mut h);
            for wp in &d.path {
                wp.0.to_bits().hash(&mut h);
                wp.1.to_bits().hash(&mut h);
            }
            d.wander_ms.hash(&mut h);
            d.repath_ms.hash(&mut h);
            d.atk_cd_ms.hash(&mut h);
            d.dying_ms.hash(&mut h);
            d.home.0.to_bits().hash(&mut h);
            d.home.1.to_bits().hash(&mut h);
            d.dmg_min.hash(&mut h);
            d.dmg_max.hash(&mut h);
            d.th.hash(&mut h);
            d.ac.hash(&mut h);
            d.xp.hash(&mut h);
            d.speed.to_bits().hash(&mut h);
            d.kind.hash(&mut h);
            d.tc.hash(&mut h);
            d.res.hash(&mut h);
        }
        for it in &self.items {
            it.def.code.hash(&mut h);
            it.def.amount.hash(&mut h);
            it.x.to_bits().hash(&mut h);
            it.y.to_bits().hash(&mut h);
        }
        for m in self.missiles.missiles() {
            m.id.hash(&mut h);
            m.x.to_bits().hash(&mut h);
            m.y.to_bits().hash(&mut h);
        }
        for (mid, slot) in &self.missile_owner {
            mid.hash(&mut h);
            slot.hash(&mut h);
        }
        if let Some(m) = &self.merc {
            m.ent.x.to_bits().hash(&mut h);
            m.ent.y.to_bits().hash(&mut h);
            m.hp.hash(&mut h);
            m.alive.hash(&mut h);
        }
        h.finish()
    }

    /// 3b-1 : hash d'état PAR SLOT — isole un désync : quel joueur a divergé.
    /// `None` hors bornes.
    pub fn state_hash_slot(&self, slot: usize) -> Option<u64> {
        use std::hash::{Hash, Hasher};
        let p = self.players.get(slot)?;
        let mut h = std::hash::DefaultHasher::new();
        0x3B32u64.hash(&mut h);
        hash_player(&mut h, p);
        Some(h.finish())
    }

    /// Point d'apparition du héros.
    pub fn hero_spawn(&self) -> (f32, f32) {
        self.players[self.local_slot].data.spawn
    }

    /// Code du héros effectif (ex. `"BA"` — résolu via `heroes[code]`/`hero`).
    pub fn hero_code(&self) -> &str {
        &self.players[self.local_slot].code
    }

    /// Clé du niveau courant (`"town"`, `"bloodmoor"`…) — persistée v3.
    pub fn level_key(&self) -> &str {
        &self.level_key
    }

    /// Pose la clé du niveau courant (d2game au chargement / switch de niveau).
    /// Trim + minuscules (même normalisation que `GameCore::with_level`) ;
    /// chaîne vide ignorée (état inchangé).
    pub fn set_level_key(&mut self, key: &str) {
        let k = key.trim().to_ascii_lowercase();
        if !k.is_empty() {
            // 16b : les invocations ne traversent pas les warps (retail v1.12)
            // — DESPAWN documenté dans minions.rs §docs (re-summon requis).
            if k != self.level_key {
                self.minions.clear();
            }
            self.level_key = k;
        }
    }

    /// Le héros a-t-il un déplacement en cours ?
    pub fn hero_is_moving(&self) -> bool {
        !self.players[self.local_slot].data.path.is_empty()
    }

    /// Le héros court-il (touche Maj, anim RN, vitesse course) ?
    pub fn hero_is_running(&self) -> bool {
        self.players[self.local_slot].data.run
    }

    // ------------------------------------------------------------------
    // API difficulté (W08/K — additif impl-difficultes)
    // ------------------------------------------------------------------

    /// Fixe la difficulté (Normal/Nightmare/Hell). À appeler AVANT les
    /// spawns pour des monstres scalés par la difficulté (`SpawnRule` de
    /// d2game) ; après coup, seules les règles du tick (résists, pénalité
    /// XP à la mort) basculent — les monstres déjà posés gardent leurs
    /// stats. Normal par défaut : tout l'existant est inchangé.
    pub fn set_difficulty(&mut self, d: Difficulty) {
        self.difficulty = d;
    }

    /// Difficulté courante (lecture — HUD wasm, save, tests).
    pub fn difficulty(&self) -> Difficulty {
        self.difficulty
    }

    // ------------------------------------------------------------------
    // Entrées
    // ------------------------------------------------------------------

    fn handle_input(&mut self, input: PlayerInput, ev: &mut Vec<GameEvent>) {
        let ls = self.local_slot;
        self.handle_input_slot(ls, input, ev);
    }

    /// 3b : intention du joueur `slot` (slot 0 = flux historique inchangé).
    fn handle_input_slot(&mut self, slot: usize, input: PlayerInput, ev: &mut Vec<GameEvent>) {
        match input {
            PlayerInput::MoveTo { x, y } => {
                if let Some(path) = self.find_path_from_hero_slot(slot, (x, y)) {
                    if !path.is_empty() {
                        if let Some(d) = self.hero_data_slot_mut(slot) {
                            d.path = path;
                        }
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
                    self.merc_target = Some(i); // H : cible partagée au mercenaire
                    let (hx, hy) = self.hero_slot_pos(slot).unwrap_or((click.0, click.1));
                    let d = dist(self.mon_ent[i].x - hx, self.mon_ent[i].y - hy);
                    if d <= ATTACK_RANGE {
                        let swing_ready = self
                            .hero_data_slot_mut(slot)
                            .map(|d| d.swing_ms == 0)
                            .unwrap_or(false);
                        if swing_ready {
                            self.hero_attack_slot(slot, i, ev);
                        }
                    } else {
                        // hors de portée : le héros marche vers le monstre
                        let mt = (
                            self.mon_ent[i].x.floor() as i32,
                            self.mon_ent[i].y.floor() as i32,
                        );
                        if let Some(path) = self.find_path_from_hero_slot(slot, mt) {
                            if !path.is_empty() {
                                if let Some(d) = self.hero_data_slot_mut(slot) {
                                    d.path = path;
                                }
                            }
                        }
                    }
                }
            }
            PlayerInput::RangedAttack { x, y } => {
                let click = (x as f32 + 0.5, y as f32 + 0.5);
                // cible : monstre vivant le plus proche du clic (rayon 2),
                // sinon tir vers le point du clic (comme D2 : on tire toujours)
                let mut aim = click;
                let mut bd = PICK_RADIUS;
                for i in 0..self.mon_ent.len() {
                    if self.mon_dat[i].state == MonState::Dying {
                        continue;
                    }
                    let e = &self.mon_ent[i];
                    let d = dist(e.x - click.0, e.y - click.1);
                    if d <= bd {
                        bd = d;
                        aim = (e.x, e.y);
                        self.merc_target = Some(i); // H : cible partagée au mercenaire
                    }
                }
                self.hero_ranged_attack_slot(slot, aim, ev);
            }
            PlayerInput::Pickup => {
                let (hx, hy) = match self.hero_slot_pos(slot) {
                    Some(p) => p,
                    None => return,
                };
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
                    let mut item = self.items.remove(i);
                    // or/inventaire crédités au héros du slot (3b)
                    let Some(d) = self.hero_data_slot_mut(slot) else {
                        self.items.push(item);
                        return;
                    };
                    if item.def.kind == ItemKind::Gold {
                        // 11c : plafond porté 10 000 × niveau — la part qui
                        // dépasse RESTE au sol (pile tronquée).
                        let g =
                            crate::economy::gain_carry(d.stats.gold, d.stats.level, item.def.amount);
                        d.stats.gold += g.gained;
                        if g.gained > 0 {
                            ev.push(GameEvent::GoldGained { amount: g.gained });
                        }
                        if g.refused > 0 {
                            item.def.amount = g.refused;
                            self.items.push(item); // bourse pleine : reliquat au sol
                        }
                    } else if let Some(s) = d.inv.iter().position(|s| s.is_none()) {
                        d.inv[s] = Some(item.def);
                        ev.push(GameEvent::HeroPickedUp { slot: s });
                    } else {
                        ev.push(GameEvent::InventoryFull);
                        self.items.push(item); // reste au sol
                    }
                }
            }
            PlayerInput::SetRun(on) => {
                if let Some(d) = self.hero_data_slot_mut(slot) {
                    d.run = on;
                }
            }
            PlayerInput::UseItem(uslot) => {
                if uslot >= INV_SLOTS {
                    return;
                }
                let Some(d) = self.hero_data_slot_mut(slot) else { return };
                let Some(def) = d.inv[uslot].clone() else { return };
                if def.kind == ItemKind::Potion {
                    let heal = POTION_HEAL.min(d.stats.max_life - d.stats.life);
                    d.stats.life += heal;
                    d.inv[uslot] = None;
                    ev.push(GameEvent::PotionUsed { heal });
                }
            }
            // J1 (corpse.rs) : clic sur son corps — récup immédiate à
            // portée, sinon marche vers lui + grab auto à l'arrivée.
            // 3c-a (hook documenté, 2 lignes max world.rs) : routage PAR SLOT
            // dans players_coop.rs — slot 0 = corpse.rs (inchangé), slots ≥ 1
            // = corps par joueur (retail MP, cf. players_coop.rs).
            PlayerInput::RecoverCorpse => {
                crate::players_coop::handle_recover_corpse_input(self, slot, ev);
            }
        }
    }

    /// A* depuis la tuile du héros vers `to` (snap tuile marchable ≤ 4 si bloquée).
    pub(crate) fn find_path_from_hero(&self, to: (i32, i32)) -> Option<Vec<(f32, f32)>> {
        self.find_path_from_hero_slot(0, to)
    }

    /// 3b : A* depuis la tuile du héros `slot`.
    pub(crate) fn find_path_from_hero_slot(&self, slot: usize, to: (i32, i32)) -> Option<Vec<(f32, f32)>> {
        let (sx, sy) = self.hero_slot_pos(slot)?;
        let start = (sx.floor() as i32, sy.floor() as i32);
        self.path_between(start, to)
    }

    /// A* générique en centres de tuiles.
    pub(crate) fn path_between(&self, from: (i32, i32), to: (i32, i32)) -> Option<Vec<(f32, f32)>> {
        let g = Grid { w: self.w, h: self.h, blocked: &self.grid };
        let goal = snap_near(&g, to, 4)?;
        let tiles = astar(&g, from, goal)?;
        Some(tiles.into_iter().map(|(x, y)| (x as f32 + 0.5, y as f32 + 0.5)).collect())
    }

    // ------------------------------------------------------------------
    // Mise à jour héros (3b : slot 0 d'abord — flux historique strictement
    // identique — puis chaque coéquipier dans l'ordre des slots)
    // ------------------------------------------------------------------

    fn update_heroes(&mut self, dt: u32, ev: &mut Vec<GameEvent>) {
        for slot in 0..self.hero_count() {
            self.update_hero_slot(slot, dt, ev);
        }
    }

    fn update_hero_slot(&mut self, slot: usize, dt: u32, ev: &mut Vec<GameEvent>) {
        // gabarit copié (Clone) : vitesse course de la classe du slot
        let tpl = self.hero_tpl_of_slot(slot);
        let mut respawn_due = false;
        let mut corpse_grab = false;
        let mut moved = false;
        let mut arrived = false;
        {
            hero_parts!(self, slot, ent, data);
            if data.dead_ms > 0 {
                data.dead_ms += dt;
                set_anim(ent, AnimState::DD, &mut data.anim_ms);
                advance_anim(ent, &mut data.anim_ms, dt, false);
                if data.dead_ms >= RESPAWN_MS {
                    respawn_due = true;
                }
            } else {
                // swing = cooldown
                if data.swing_ms > 0 {
                    data.swing_ms = data.swing_ms.saturating_sub(dt);
                }

                // déplacement (course = RunVelocity du manifest, sinon marche)
                let pos_before = (ent.x, ent.y);
                let moving = !data.path.is_empty();
                if !data.path.is_empty() {
                    let speed = if data.run {
                        tpl.run_speed.max(data.stats.walk_speed)
                    } else {
                        data.stats.walk_speed
                    };
                    arrived = advance_along_path(ent, &mut data.path, speed, dt);
                }

                // EXP-H2 : drain/régénération d'endurance (`stamina.rs`).
                if crate::stamina::tick_stamina(
                    data.run,
                    moving,
                    dt,
                    data.stats.max_stamina,
                    &mut data.stamina_milli,
                    &mut data.stats.stamina,
                ) {
                    data.run = false;
                }

                // J1 (slot 0) : grab auto du corps à l'arrivée (traité hors
                // emprunt, cf. plus bas).
                if slot == 0 && self.corpse_pending && data.path.is_empty() {
                    corpse_grab = true;
                }

                // 11c : or au sol auto-ramassé EN ZÉONE pendant le déplacement
                // (traité hors emprunt pour garder l'ordre d'événements).
                moved = (ent.x, ent.y) != pos_before;
            }
        }
        if respawn_due {
            self.respawn_hero_slot(slot, ev);
            return;
        }
        if corpse_grab {
            self.corpse_pending = false;
            self.recover_corpse(ev);
        }
        if moved {
            if slot == 0 {
                crate::economy::world_auto_pickup(self, ev);
            } else {
                crate::economy::world_auto_pickup_slot(self, slot, ev);
            }
        }
        {
            hero_parts!(self, slot, ent, data);
            // sélection anim
            let st = if !data.path.is_empty() {
                if data.run {
                    AnimState::RN
                } else {
                    AnimState::WL
                }
            } else if data.swing_ms > 0 {
                // Étape 15b : cast de skill ⇒ SC ; attaque simple ⇒ A1.
                if data.swing_cast {
                    AnimState::SC
                } else {
                    AnimState::A1
                }
            } else {
                AnimState::NU
            };
            set_anim(ent, st, &mut data.anim_ms);
            advance_anim(
                ent,
                &mut data.anim_ms,
                dt,
                matches!(st, AnimState::NU | AnimState::WL | AnimState::RN),
            );

            if arrived {
                let end = (ent.x.floor() as i32, ent.y.floor() as i32);
                ev.push(GameEvent::HeroMoved { path_end: end });
            }
        }
    }

    /// Gabarit du joueur `slot` (copie — [`HeroTpl`] est `Clone`).
    pub(crate) fn hero_tpl_of_slot(&self, slot: usize) -> HeroTpl {
        self.players
            .get(slot)
            .map(|p| p.tpl.clone())
            .unwrap_or_else(|| self.hero_tpl.clone())
    }

    /// 3b : réapparition du héros `slot` (pleine vie, son point d'apparition).
    /// Le slot 0 conserve le flux historique (`on_respawn` auras + corps J1).
    fn respawn_hero_slot(&mut self, slot: usize, ev: &mut Vec<GameEvent>) {
        if slot == 0 {
            self.respawn_hero(ev);
            return;
        }
        let Some(c) = self.players.get_mut(slot) else { return };
        let (sx, sy) = c.data.spawn;
        c.ent.x = sx;
        c.ent.y = sy;
        c.ent.facing = 0;
        c.data.stats.life = c.data.stats.max_life;
        c.data.stats.mana = c.data.stats.max_mana;
        c.data.stats.stamina = c.data.stats.max_stamina;
        c.data.stamina_milli = (c.data.stats.max_stamina as i64) * 1000;
        c.data.dead_ms = 0;
        c.data.swing_ms = 0;
        c.data.path.clear();
        c.data.anim_ms = 0;
        c.ent.anim = AnimState::NU;
        c.ent.anim_frame = 0;
        // tout le monde perd l'aggro (même règle que le slot 0)
        for dat in self.mon_dat.iter_mut() {
            if dat.state != MonState::Dying {
                dat.state = MonState::Idle;
                dat.path.clear();
                dat.atk_cd_ms = MONSTER_ATK_GRACE_MS;
            }
        }
        ev.push(GameEvent::HeroRespawned { x: sx.floor() as i32, y: sy.floor() as i32 });
    }

    fn respawn_hero(&mut self, ev: &mut Vec<GameEvent>) {
        let (sx, sy) = self.players[0].data.spawn;
        // 16a : l'aura se coupe à la mort (états perdus au respawn).
        self.on_respawn();
        self.players[0].ent.x = sx;
        self.players[0].ent.y = sy;
        self.players[0].ent.facing = 0;
        self.players[0].data.stats.life = self.players[0].data.stats.max_life;
        self.players[0].data.stats.mana = self.players[0].data.stats.max_mana;
        // EXP-H2 : le respawn remet aussi l'endurance pleine.
        self.players[0].data.stats.stamina = self.players[0].data.stats.max_stamina;
        self.players[0].data.stamina_milli = (self.players[0].data.stats.max_stamina as i64) * 1000;
        self.players[0].data.dead_ms = 0;
        self.players[0].data.swing_ms = 0;
        self.players[0].data.path.clear();
        self.players[0].data.anim_ms = 0;
        self.players[0].ent.anim = AnimState::NU;
        self.players[0].ent.anim_frame = 0;
        // tout le monde perd l'aggro
        for dat in self.mon_dat.iter_mut() {
            if dat.state != MonState::Dying {
                dat.state = MonState::Idle;
                dat.path.clear();
                dat.atk_cd_ms = MONSTER_ATK_GRACE_MS;
            }
        }
        // J1 : plus de course vers un corps en cours (transitoire)
        self.corpse_pending = false;
        ev.push(GameEvent::HeroRespawned { x: sx.floor() as i32, y: sy.floor() as i32 });
    }

    /// Frappe héros → monstre `i` (déjà à portée, cooldown prêt).
    /// 3b : le héros `slot` frappe — slot 0 = flux historique strictement
    /// identique (mods 12c / auras 16a / 16c sont des états du slot 0) ;
    /// l'XP du kill est créditée au slot (via `last_hit_by`).
    fn hero_attack(&mut self, i: usize, ev: &mut Vec<GameEvent>) {
        self.hero_attack_slot(0, i, ev);
    }

    fn hero_attack_slot(&mut self, slot: usize, i: usize, ev: &mut Vec<GameEvent>) {
        let tpl = self.hero_tpl_of_slot(slot);
        if self.hero_slot_pos(slot).is_none() {
            return;
        }
        // 16c (additif) : swing IAS des formes druides — état du slot 0,
        // cohéris : swing standard. Hors emprunt (appel world).
        let swing = if slot == 0 { crate::assassin_druid::hero_swing_ms(self) } else { HERO_SWING_MS };
        {
            hero_parts!(self, slot, ent, data);
            let (dx, dy) = (self.mon_ent[i].x - ent.x, self.mon_ent[i].y - ent.y);
            ent.facing = facing_from(dx, dy);
            data.swing_ms = swing;
            data.anim_ms = 0;
        }
        let ac = self.monster_effective_ac(i); // 16a : Conviction (-AC) additif
        // 16c (additif) : +par4 (50) par charge de combo (progressive_tohit).
        let to_hit_base = {
            let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
            d.stats.to_hit
        };
        let to_hit = if slot == 0 {
            to_hit_base.saturating_add(crate::assassin_druid::hero_to_hit_bonus(self))
        } else {
            to_hit_base
        };
        let chance = hit_chance(to_hit, ac);
        if hero_roll_below(self, slot, 100) < chance {
            // dégâts héros = floor(rand(mindam..maxdam) × (1 + force/100))
            let (weapon_min, weapon_max, strg, clvl, mods) = {
                let d = self.hero_data_slot_mut(slot).expect("slot joueur valide");
                (tpl.weapon_min, tpl.weapon_max, d.stats.strength, d.stats.level, d.mods)
            };
            let base = hero_roll_range(self, slot, weapon_min, weapon_max) as f32;
            let dmg = (base * (1.0 + strg as f32 / 100.0)).floor() as u32;
            // 16a (additif) : auras de dégâts (Might/Concentration/Fanaticism)
            // — ×(100+pct)/100, zéro changement sans aura active.
            // 3b : l'aura active est celle du slot 0 (paladin du salon) —
            // un coéquipier n'en profite pas.
            let dmg = if slot == 0 { self.scale_hero_damage(dmg) } else { dmg };
            // 16c (additif) : multiplicateur Werebear + Tiger Strike — slot 0.
            let dmg = if slot == 0 {
                crate::assassin_druid::scaled_hero_melee(self, dmg, base as u32)
            } else {
                dmg
            };
            // 12c (additif) : mods de coup via `combat_mods::apply_combat_mods`
            // (posés par `set_hero_combat_mods` — état du slot 0 ; cohéris :
            // `HitMods::default()`).
            let rolls = if mods.any_offensive() {
                (hero_roll_below(self, slot, 100), hero_roll_below(self, slot, 100), hero_roll_below(self, slot, 100))
            } else {
                (0, 0, 0)
            };
            let (victim_res, victim_kind, victim_hp) = (
                self.monster_effective_res(i),
                combat_mods::VictimKind::from_mon_kind(&self.mon_dat[i].kind),
                self.mon_dat[i].hp,
            );
            let (wlife, wmax_life, wmana, wmax_mana) = {
                let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
                (d.stats.life, d.stats.max_life, d.stats.mana, d.stats.max_mana)
            };
            let hit = combat_mods::CombatHit {
                physical: dmg,
                elemental: 0,
                element: d2missiles::DamageType::Physical,
                mods,
                clvl,
                difficulty: self.difficulty,
                victim: combat_mods::Victim {
                    kind: victim_kind,
                    cur_hp: victim_hp,
                    // 16a (additif) : résists EFFECTIFS — malédiction
                    // (Amplify -100 % ResDm, Lower Resist, Decrepify) +
                    // aura Conviction dans le rayon ; identiques sans.
                    res: victim_res,
                },
                wearer_life: wlife,
                wearer_max_life: wmax_life,
                wearer_mana: wmana,
                wearer_max_mana: wmax_mana,
                melee: true,
                roll_deadly: rolls.0,
                roll_wounds: rolls.1,
                roll_crushing: rolls.2,
            };
            let out = combat_mods::apply_combat_mods(&hit);
            // Résist physique du monstre (ResDm — patch_monstats col 130/136/142,
            // ex. skeleton1 ResDm(H)=33, zombie1 ResDm(H)=50) — W08/K ; déjà
            // appliquée DANS apply_combat_mods (après le deadly ×2, ordre D2).
            let dmg = out.physical;
            // Crushing blow : fraction des PV actuels, hors résist (12c).
            let dealt = dmg.saturating_add(out.crushing);
            self.mon_dat[i].hp = self.mon_dat[i].hp.saturating_sub(dealt);
            self.mon_dat[i].last_hit_by = slot as u8; // 3b : XP créditée au slot
            // DoT plaies ouvertes (mêlée, 12c) : remplace l'ancienne plaie.
            if let Some(w) = out.wounds {
                self.mon_dat[i].wounds = Some(w);
            }
            ev.push(GameEvent::HeroAttacked { target: i, dmg: dealt });
            // Leech vie/mana (12c) : plafonné au manque, divisors difficulté.
            {
                hero_parts!(self, slot, _ent, data);
                if out.life_leech > 0 {
                    data.stats.life = (data.stats.life + out.life_leech).min(data.stats.max_life);
                }
                if out.mana_leech > 0 {
                    data.stats.mana = (data.stats.mana + out.mana_leech).min(data.stats.max_mana);
                }
            }
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
            // 16c (additif) : charge-up/finisher Assassine + knockback
            // Werebear — état 16c du slot 0 uniquement.
            if slot == 0 {
                crate::assassin_druid::note_hero_melee_hit(self, i, dealt, base as u32);
            }
        } else {
            ev.push(GameEvent::HeroMissed { target: i });
        }
    }

    /// Tir à distance : anime le swing, face à la cible, décoche le missile du
    /// skill actif. (3b : délègue au slot 0 — flux historique inchangé.)
    fn hero_ranged_attack(&mut self, aim: (f32, f32), ev: &mut Vec<GameEvent>) {
        self.hero_ranged_attack_slot(0, aim, ev);
    }

    /// 3b : tir du héros `slot` — le missile porte son propriétaire
    /// (`missile_owner`) pour créditer toucher/dégâts/XP au bon héros.
    fn hero_ranged_attack_slot(&mut self, slot: usize, aim: (f32, f32), ev: &mut Vec<GameEvent>) {
        let (swing_busy, dead) = match self.hero_data_slot_mut(slot) {
            Some(d) => (d.swing_ms > 0, d.dead_ms > 0),
            None => return,
        };
        if swing_busy || dead {
            return; // cooldown en cours / héros mort
        }
        let (hx, hy) = match self.hero_slot_pos(slot) {
            Some(p) => p,
            None => return,
        };
        let (strg, skills, level, active_idx) = {
            let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
            (d.stats.strength, d.skills.clone(), d.stats.level, d.active_skill)
        };
        {
            hero_parts!(self, slot, ent, data);
            let (dx, dy) = (aim.0 - ent.x, aim.1 - ent.y);
            ent.facing = facing_from(dx, dy);
            data.swing_ms = HERO_SWING_MS;
            // Étape 15b : skill lancé (porteur de missile) ⇒ cast SC ;
            // tir d'arme simple (repli arrow) ⇒ A1/TH non distingués au MVP.
            data.swing_cast = data
                .skills
                .get(data.active_skill)
                .map(|s| s.missile.is_some())
                .unwrap_or(false);
            data.anim_ms = 0;
        }
        let from = (hx, hy);
        let active = skills.get(active_idx).map(|s| (s.code.clone(), s.missile.clone()));
        let spawned = match active {
            // carrier Skill (C4) : dégâts résolus AU TIR = table (officiels
            // niv 1) × facteur de synergie du skill — les niveaux des AUTRES
            // skills du héros alimentent la formule CalcString.
            Some((skill_code, Some(missile_code))) => {
                let (dmin, dmax) = {
                    let levels = skills_hero::HeroLevels::new(&skills, level);
                    self.missiles
                        .def(&missile_code)
                        .map(|def| {
                            skills_hero::synergy_scaled_damage(&skill_code, def.dmg_min, def.dmg_max, &levels)
                        })
                        .unwrap_or((0, 0))
                };
                self.missiles.spawn_skill(&missile_code, from, aim, dmin, dmax)
            }
            // carrier Weapon : l'appelant résout floor(min/max × (1 + force/100))
            Some((_, None)) | None => {
                let tpl = self.hero_tpl_of_slot(slot);
                let mult = 1.0 + strg as f32 / 100.0;
                let dmin = ((tpl.weapon_min as f32 * mult).floor() as u32).max(1);
                let dmax = ((tpl.weapon_max as f32 * mult).floor() as u32).max(dmin);
                self.missiles.spawn_weapon(HERO_MISSILE, from, aim, dmin, dmax)
            }
        };
        if let Some(id) = spawned {
            // 3b : propriétaire du missile (crédit multi-héros).
            self.missile_owner.push((id, slot as u8));
            ev.push(GameEvent::MissileSpawned);
        }
    }

    /// Avance les missiles en vol : collision monstres (rayon
    /// MISSILE_HIT_RADIUS, DD exclus) puis murs (tuile bloquée ⇒ arrêt).
    fn update_missiles(&mut self, dt: u32, ev: &mut Vec<GameEvent>) {
        if self.missiles.is_empty() {
            return;
        }
        // Snapshot des cibles (monstres figés pendant le tick missiles —
        // update_monsters est déjà terminé) : split-borrow impossible sinon.
        let targets: Vec<(f32, f32, bool)> = self
            .mon_ent
            .iter()
            .zip(self.mon_dat.iter())
            .map(|(e, d)| (e.x, e.y, d.state == MonState::Dying))
            .collect();
        let (grid, gw, gh) = (&self.grid, self.w, self.h);
        let events = self.missiles.tick_walls(
            dt,
            &mut |x, y| {
                for (i, (ex, ey, dying)) in targets.iter().enumerate() {
                    if *dying {
                        continue;
                    }
                    if dist(*ex - x, *ey - y) <= MISSILE_HIT_RADIUS {
                        return Some(i);
                    }
                }
                None
            },
            &mut |x, y| {
                let (tx, ty) = (x.floor() as i32, y.floor() as i32);
                !(tx >= 0 && ty >= 0 && tx < gw && ty < gh && !grid[ty as usize][tx as usize])
            },
        );
        for me in events {
            // 3b : propriétaire du missile (slot héros) — absent = slot 0
            // (missiles merc/invocations : propriétaires historiques). La
            // propriété est consommée à la fin de vie du missile.
            let owner = match &me {
                d2missiles::MissileEvent::Hit { id, .. }
                | d2missiles::MissileEvent::Expired { id, .. }
                | d2missiles::MissileEvent::Blocked { id, .. } => {
                    match self.missile_owner.iter().position(|(mid, _)| mid == id) {
                        Some(p) => self.missile_owner.remove(p).1,
                        None => 0,
                    }
                }
                _ => 0,
            };
            let slot = owner as usize;
            match me {
                d2missiles::MissileEvent::Hit { id, target, dmg, .. } => {
                    // jet de toucher : attack rating du héros TIREUR vs AC
                    // monstre (§3) — 16a : AC EFFECTIF (Conviction).
                    let ac = self.monster_effective_ac(target);
                    let to_hit = self
                        .hero_data_slot_mut(slot)
                        .map(|d| d.stats.to_hit)
                        .unwrap_or(0);
                    let chance = hit_chance(to_hit, ac);
                    if hero_roll_below(self, slot, 100) < chance {
                        // 12c (additif) : résolution du coup missile via
                        // `apply_combat_mods` — SANS mods, identique à
                        // l'ancien `apply_resist` direct (W08/K) : ResFi/Co/Li…
                        // colonnes (N)/(H) de `patch_monstats.txt`, immunité
                        // (≥100) ⇒ 0 dégât, négatif (Hell) ⇒ amplifié.
                        let elem = self
                            .missiles
                            .missiles()
                            .iter()
                            .find(|m| m.id == id)
                            .map(|m| m.elem)
                            .unwrap_or_default();
                        // Deadly strike : arme PHYSIQUE seule (flèche) — jet
                        // tiré seulement si un mod est posé (zéro RNG sinon).
                        let (mods, clvl, wlife, wmax_life, wmana, wmax_mana) = {
                            let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
                            (
                                d.mods,
                                d.stats.level,
                                d.stats.life,
                                d.stats.max_life,
                                d.stats.mana,
                                d.stats.max_mana,
                            )
                        };
                        let roll_deadly = if elem == d2missiles::DamageType::Physical
                            && mods.deadly_strike_pct > 0
                        {
                            hero_roll_below(self, slot, 100)
                        } else {
                            0
                        };
                        let roll_crushing = if mods.crushing_blow_pct > 0 {
                            hero_roll_below(self, slot, 100)
                        } else {
                            0
                        };
                        let hit = combat_mods::CombatHit {
                            physical: 0,
                            elemental: dmg,
                            element: elem,
                            mods,
                            clvl,
                            difficulty: self.difficulty,
                            victim: combat_mods::Victim {
                                kind: combat_mods::VictimKind::from_mon_kind(&self.mon_dat[target].kind),
                                cur_hp: self.mon_dat[target].hp,
                                // 16a (additif) : résists EFFECTIFS (cf. mêlée).
                                res: self.monster_effective_res(target),
                            },
                            wearer_life: wlife,
                            wearer_max_life: wmax_life,
                            wearer_mana: wmana,
                            wearer_max_mana: wmax_mana,
                            melee: false, // plaies ouvertes : mêlée seule
                            roll_deadly,
                            roll_wounds: 0,
                            roll_crushing,
                        };
                        let out = combat_mods::apply_combat_mods(&hit);
                        let dmg = out.elemental;
                        // Poison avec durée (ELen) : converti en DoT (12c) au
                        // lieu d'instantané ; immunité ⇒ aucun des deux.
                        if let Some(p) = out.poison {
                            self.mon_dat[target].poison = Some(p);
                        }
                        // Froid : vitesse ×1/2 pendant coldlength ÷ divisor.
                        if out.chill_frames > 0 {
                            self.mon_dat[target].chill_ms =
                                out.chill_frames.saturating_mul(combat_mods::FRAME_MS_D2);
                        }
                        // Crushing blow s'ajoute hors résist (fraction PV).
                        let dealt = dmg.saturating_add(out.crushing);
                        if dealt == 0 {
                            // Immunité : le missile est consumé, rien ne passe.
                            ev.push(GameEvent::MissileMissed { target });
                            continue;
                        }
                        self.mon_dat[target].hp = self.mon_dat[target].hp.saturating_sub(dealt);
                        self.mon_dat[target].last_hit_by = slot as u8; // 3b : crédit au tireur
                        ev.push(GameEvent::MissileHit { target, dmg: dealt });
                        if self.mon_dat[target].hp == 0 {
                            let ent = &mut self.mon_ent[target];
                            let dat = &mut self.mon_dat[target];
                            dat.state = MonState::Dying;
                            dat.dying_ms = 0;
                            dat.anim_ms = 0;
                            dat.path.clear();
                            ent.anim = AnimState::DD;
                            ent.anim_frame = 0;
                        }
                    } else {
                        ev.push(GameEvent::MissileMissed { target });
                    }
                }
                d2missiles::MissileEvent::Spawned { .. }
                | d2missiles::MissileEvent::Expired { .. }
                | d2missiles::MissileEvent::Blocked { .. } => {}
            }
        }
    }

    /// Recalcule `xp_to_next` après une perte d'XP (pénalité de mort K3).
    fn refresh_xp_to_next(&mut self) {
        self.refresh_xp_to_next_slot(0);
    }

    /// 3b : variante par slot du recalcul d'XP.
    fn refresh_xp_to_next_slot(&mut self, slot: usize) {
        let code = self.hero_code_of_slot(slot).to_string();
        let (lvl, cur_xp) = {
            let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
            (d.stats.level, d.stats.xp)
        };
        let max = max_level_of(&code);
        let next = if lvl >= max {
            0
        } else {
            xp_threshold(&code, lvl + 1).saturating_sub(cur_xp)
        };
        if let Some(d) = self.hero_data_slot_mut(slot) {
            d.stats.xp_to_next = next;
        }
    }

    /// Code héros du slot (interne — slot inconnu : code du slot 0).
    pub(crate) fn hero_code_of_slot(&self, slot: usize) -> &str {
        self.players.get(slot).map(|p| p.code.as_str()).unwrap_or(&self.players[0].code)
    }

    /// Applique l'XP gagnée + paliers de niveau (événement par palier).
    /// 3b : délègue au slot 0 — flux historique inchangé.
    pub(crate) fn apply_xp(&mut self, xp: u32, ev: &mut Vec<GameEvent>) {
        self.apply_xp_slot(0, xp, ev);
    }

    /// 3b : applique l'XP au héros `slot` (paliers 1→`MaxLvl` de la table
    /// réelle de SA classe, vie/mana/skill points par niveau).
    pub(crate) fn apply_xp_slot(&mut self, slot: usize, mut xp: u32, ev: &mut Vec<GameEvent>) {
        let code = self.hero_code_of_slot(slot).to_string();
        let tpl = self.hero_tpl_of_slot(slot);
        let max = max_level_of(&code);
        // ajout unique (flux historique), puis paliers
        {
            let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
            d.stats.xp = d.stats.xp.saturating_add(xp);
            xp = d.stats.xp;
        }
        loop {
            let lvl = match self.hero_data_slot_mut(slot) {
                Some(d) => d.stats.level,
                None => break,
            };
            if lvl >= max {
                if let Some(d) = self.hero_data_slot_mut(slot) {
                    d.stats.xp_to_next = 0;
                }
                break;
            }
            let need = xp_threshold(&code, lvl + 1);
            if xp >= need {
                {
                    let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
                    d.stats.level = lvl + 1;
                    d.stats.max_life += tpl.life_per_level;
                    d.stats.life += tpl.life_per_level;
                    d.stats.max_mana += tpl.mana_per_level;
                    d.stats.mana += tpl.mana_per_level;
                    // ARBRE COMPÉTENCES (C2) : +1 point de skill par niveau de
                    // perso (v1.12) ; récompenses de quêtes EN PLUS via
                    // `grant_skill_points` (slot 0).
                    d.stats.skill_points = d.stats.skill_points.saturating_add(1);
                    // P26-1 (A2) : +`StatPerLevel` points de caractéristique
                    // par niveau (v1.12 : 5 pour les 7 classes) — dépensés
                    // via [`World::spend_stat`].
                    d.stats.stat_points = d.stats.stat_points.saturating_add(tpl.stat_per_level);
                }
                ev.push(GameEvent::HeroLeveled { level: lvl + 1 });
            } else {
                if let Some(d) = self.hero_data_slot_mut(slot) {
                    d.stats.xp_to_next = need - xp;
                }
                break;
            }
        }
    }

    // ------------------------------------------------------------------
    // Mise à jour monstres
    // ------------------------------------------------------------------

    fn update_monsters(&mut self, dt: u32, ev: &mut Vec<GameEvent>) {
        // 3b-1, DÉCISION DE FIDÉLITÉ : XP PARTAGÉE entre les joueurs VIVANTS
        // (part égale ; reliquat `xp % n` au TUEUR s'il est vivant, sinon au
        // premier vivant ; tout le monde mort ⇒ XP perdue). Solo : n = 1 ⇒
        // part pleine au tueur — flux historique bit-exact. ÉCART RETAIL
        // documenté : D2 partage entre les membres du groupe PROCHE du kill ;
        // MVP : tous les vivants, sans condition de distance. L'ensemble des
        // vivants est figé EN DÉBUT de tick (déterminisme lockstep : les
        // frappes de ce tick ne changent pas le partage).
        let living: Vec<usize> = (0..self.hero_count())
            .filter(|&s| !self.hero_slot_dead(s).unwrap_or(true))
            .collect();
        let n_living = living.len().max(1) as u32;
        // XP accumulée par monstre mort CE tick : (xp, tueur).
        let mut kills: Vec<(u32, usize)> = Vec::new();

        let mut i = 0;
        while i < self.mon_ent.len() {
            // --- branche DD : anim puis disparition + drop + XP ---
            if self.mon_dat[i].state == MonState::Dying {
                self.mon_dat[i].dying_ms += dt;
                advance_anim(&mut self.mon_ent[i], &mut self.mon_dat[i].anim_ms, dt, false);
                if self.mon_dat[i].dying_ms >= DYING_MS {
                    let xp = self.mon_dat[i].xp;
                    let killer = self.mon_dat[i].last_hit_by as usize;
                    let mlvl = self.mon_tpls[self.mon_dat[i].tpl].level;
                    // TC effective : surcharge du spawn (champion/unique/boss)
                    // sinon celle du gabarit manifest.
                    let tc = self.mon_dat[i]
                        .tc
                        .clone()
                        .unwrap_or_else(|| self.mon_tpls[self.mon_dat[i].tpl].treasure_class.clone());
                    let (x, y) = (self.mon_ent[i].x, self.mon_ent[i].y);
                    self.roll_drop(x, y, mlvl, &tc, ev);
                    let kslot = killer.min(MAX_HEROES - 1);
                    if kslot < self.hero_count() {
                        kills.push((xp, kslot));
                    }
                    ev.push(GameEvent::MonsterDied { index: i, xp });
                    // IA P1 (ai_behaviors) : cadavre persistant + panique des
                    // Fallen vivants proches — AVANT le retrait (indices).
                    crate::ai_behaviors::register_corpse(self, i, ev);
                    crate::minions::register_raisable_corpse(self, i); // 16b : cadavre invocable
                    self.mon_dat.remove(i);
                    self.mon_ent.remove(i);
                    self.ai.ext.remove(i); // IA P1 : réaligne l'extension AI
                    self.merc_target = None; // H : indice périmé (décalage), on relâche
                    continue; // pas de i += 1 : l'élément suivant prend l'index i
                }
                i += 1;
                continue;
            }

            // IA P1 (ai_behaviors) : overlays Fallen (panique) / FallenShaman
            // (résurrection). `false` = comportement historique strictement
            // inchangé (archétype None ou rien à faire ce frame).
            if crate::ai_behaviors::tick(self, i, dt, ev) {
                i += 1;
                continue;
            }

            // 12c (additif) : DoT plaies/poison (ticks de 25 frames = 1 s) et
            // refroidissement (décompte). Un DoT peut abattre le monstre —
            // même bloc de mort que la mêlée, puis saut du reste du tour.
            let mut dot_dmg = 0;
            {
                let dat = &mut self.mon_dat[i];
                if let Some(d) = dat.wounds.as_mut() {
                    dot_dmg += d.tick(dt);
                }
                if let Some(d) = dat.poison.as_mut() {
                    dot_dmg += d.tick(dt);
                }
                dat.chill_ms = dat.chill_ms.saturating_sub(dt);
            }
            if dot_dmg > 0 && self.mon_dat[i].hp > 0 {
                self.mon_dat[i].hp = self.mon_dat[i].hp.saturating_sub(dot_dmg);
                if self.mon_dat[i].hp == 0 {
                    let ent = &mut self.mon_ent[i];
                    let dat = &mut self.mon_dat[i];
                    dat.state = MonState::Dying;
                    dat.dying_ms = 0;
                    dat.anim_ms = 0;
                    dat.path.clear();
                    ent.anim = AnimState::DD;
                    ent.anim_frame = 0;
                    i += 1;
                    continue;
                }
            }

            // 16a (additif) : malédictions nécromancien — Dim Vision / Terror /
            // in-fighting Attract/Confuse prennent la main (auras.rs) ;
            // `false` = flux historique strictement inchangé.
            if crate::auras::tick_monster(self, i, dt, ev) {
                i += 1;
                continue;
            }

            // H (mercenaire, additif) + 3b (coop) : la cible effective de
            // poursuite/frappe est la PLUS PROCHE de (héros vivants, mercenaire
            // vivant) — parmi les héros, le plus proche gagne (slots dans
            // l'ordre, stricte < : à égalité le slot le plus bas). Sans
            // mercenaire (ou KO) : héros vivant le plus proche, flux
            // historique inchangé mono-héros, zéro RNG ajouté.
            let nearest_hero = self.nearest_living_hero_pos(self.mon_ent[i].x, self.mon_ent[i].y);
            let (tgt_pos, _tgt_is_merc) = match self.merc.as_ref().filter(|m| m.alive) {
                Some(m) => {
                    let dm = dist(self.mon_ent[i].x - m.ent.x, self.mon_ent[i].y - m.ent.y);
                    let dh = dist(self.mon_ent[i].x - nearest_hero.0, self.mon_ent[i].y - nearest_hero.1);
                    if dm < dh { ((m.ent.x, m.ent.y), true) } else { (nearest_hero, false) }
                }
                None => (nearest_hero, false),
            };
            // 16b (additif) : une invocation vivante plus PROCHE devient la
            // cible de poursuite (strict — à égalité le flux historique gagne).
            let tgt_pos = crate::minions::nearest_minion_override(
                self,
                tgt_pos,
                self.mon_ent[i].x,
                self.mon_ent[i].y,
            );
            let d = dist(self.mon_ent[i].x - tgt_pos.0, self.mon_ent[i].y - tgt_pos.1);
            let hero_dead = self.all_heroes_dead();

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
                            let ht = (tgt_pos.0.floor() as i32, tgt_pos.1.floor() as i32);
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
                        let (dx, dy) = (tgt_pos.0 - self.mon_ent[i].x, tgt_pos.1 - self.mon_ent[i].y);
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
                    // 12c (additif) : froid — vitesse ×1/2 tant que le
                    // refroidissement reste actif (`chill_ms` > 0).
                    let base = self.mon_dat[i].speed;
                    let speed = if self.mon_dat[i].chill_ms > 0 {
                        base
                            * combat_mods::CHILL_SPEED_NUM as f32
                            / combat_mods::CHILL_SPEED_DEN as f32
                    } else {
                        base
                    };
                    // 16a (additif) : Decrepify — vitesse ×1/2
                    // (velocitypercent -50, L89).
                    let speed = if self.monster_slowed(i) { speed / 2.0 } else { speed };
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

        // 3b-1 : application de l'XP PARTAGÉE (cf. décision en tête de
        // fonction) — par monstre mort : part entière à chaque vivant,
        // reliquat au tueur (s'il est vivant, sinon au premier vivant).
        for (xp, killer) in kills {
            let share = xp / n_living;
            let rem = xp % n_living;
            if share > 0 {
                for &slot in &living {
                    self.apply_xp_slot(slot, share, ev);
                }
            }
            if rem > 0 {
                // tueur vivant ? sinon le premier vivant ; personne sinon.
                let rem_slot = if living.contains(&killer) {
                    killer
                } else {
                    *living.first().unwrap_or(&usize::MAX)
                };
                if rem_slot < self.hero_count() {
                    self.apply_xp_slot(rem_slot, rem, ev);
                }
            }
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
    /// 3b : la cible est le héros VIVANT le plus proche (slots dans l'ordre,
    /// stricte < : à égalité le slot le plus bas) — mono-héros : flux
    /// historique inchangé.
    fn monster_strike(&mut self, i: usize, ev: &mut Vec<GameEvent>) {
        // H (mercenaire, additif) : le monstre frappe la cible la plus PROCHE —
        // héros vivants ou mercenaire VIVANT. Sans mercenaire (ou KO), le flux
        // historique est inchangé (aucun tirage RNG supplémentaire).
        let (mx, my) = (self.mon_ent[i].x, self.mon_ent[i].y);
        // 3b : héros vivant le plus proche (ordre des slots pour l'égalité)
        let mut target_slot = 0usize;
        let mut bd = f32::INFINITY;
        for s in 0..self.hero_count() {
            if self.hero_slot_dead(s).unwrap_or(true) {
                continue;
            }
            let (hx, hy) = self.hero_slot_pos(s).unwrap_or((mx, my));
            let dh = dist(hx - mx, hy - my);
            if dh < bd {
                bd = dh;
                target_slot = s;
            }
        }
        // 3c-a (hook documenté, 2 lignes max world.rs) : résolution dans
        // mercenary.rs — le merc le plus PROCHE parmi TOUS les slots encaisse
        // (slot 0 = flux historique bit-exact) ; false ⇒ flux 3b inchangé.
        if crate::mercenary::monster_strikes_nearest_merc(self, i, bd, ev) { return; }
        // 16b (additif) : une invocation vivante plus PROCHE que le héros
        // encaisse le coup (strict — à égalité le flux historique gagne).
        if crate::minions::monster_strikes_minion(self, i, ev) {
            return;
        }
        self.monster_strike_hero_slot(i, target_slot, ev);
    }

    /// 3b : le monstre `i` frappe le héros `slot` (défense/jets/dégâts du
    /// slot ; corps J1 et auras 16a restent des mécaniques du slot 0).
    fn monster_strike_hero_slot(&mut self, i: usize, slot: usize, ev: &mut Vec<GameEvent>) {
        let (dmin, dmax, th) = {
            let d = &self.mon_dat[i];
            (d.dmg_min, d.dmg_max, d.th)
        };
        let dex = {
            let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
            d.stats.dexterity
        };
        // défense du héros pour ce coup : dex/4 + rand(minac..maxac bouclier)
        let tpl = self.hero_tpl_of_slot(slot);
        let def =
            dex / 4 + self.rng.range_u32(tpl.shield_ac_min, tpl.shield_ac_max);
        // 16a (additif) : Defiance — ×(100+pct)/100, inchangé sans aura.
        // 3b : l'aura active est celle du slot 0 — cohéris neutres.
        let def = if slot == 0 { self.scale_hero_defense(def) } else { def };
        // 16c (additif) : Werebear skill_armor_percent (ln34) — 0 sans forme.
        let def = if slot == 0 {
            def.saturating_add(crate::assassin_druid::hero_defense_bonus(self, def))
        } else {
            def
        };
        let chance = hit_chance(th, def);
        if (self.rng.below(100)) < chance {
            let raw = self.rng.range_u32(dmin, dmax).max(1);
            // 16a (additif) : Weaken/Decrepify — dégâts du monstre scalés.
            let dmg = self.scale_monster_damage(i, raw);
            {
                let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
                d.stats.life = d.stats.life.saturating_sub(dmg);
            }
            ev.push(GameEvent::HeroHit { index: i, dmg });
            // 16a (additif) : renvois Thorns (aura) + Iron Maiden
            // (malédiction) — peut abattre le monstre, même si le héros meurt.
            // 3b : les renvois sont l'aura du slot 0 (crédit slot 0).
            if slot == 0 {
                self.reflect_on_hero_hit(i, dmg);
            }
            let life = {
                let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
                d.stats.life
            };
            if life == 0 {
                let code = self.hero_code_of_slot(slot).to_string();
                // Pénalité XP à la mort (K3) : DeathExpPenalty 0/5/10 %
                // (`patch_difficultylevels.txt`), plafonnée au plancher du
                // niveau courant (le retail ne fait jamais descendre de
                // niveau). Normal : 0 % — zéro changement.
                let pct = difficulty::embedded().row(self.difficulty).death_exp_penalty;
                {
                    let d = self.hero_data_slot_mut(slot).expect("slot cohéros valide");
                    d.dead_ms = 1; // démarre le compte à rebours de respawn
                    d.path.clear();
                    d.swing_ms = 0;
                    d.anim_ms = 0;
                    if pct > 0 {
                        let lost = d.stats.xp.saturating_mul(pct) / 100;
                        let floor = if d.stats.level > 1 {
                            xp_threshold(&code, d.stats.level)
                        } else {
                            0
                        };
                        d.stats.xp = d.stats.xp.saturating_sub(lost).max(floor);
                    }
                }
                self.refresh_xp_to_next_slot(slot);
                // J1 (corpse.rs) : le corps du SLOT 0 reste au sol AVEC
                // l'inventaire ; l'or porté subit le péage officiel et le
                // reliquat tombe au sol à cet endroit. 3b : les coéquipiers
                // gardent leur inventaire au respawn (pas de corps).
                if slot == 0 {
                    self.drop_corpse(ev);
                }
                ev.push(GameEvent::HeroDied);
            }
        }
        // raté : pas d'événement (flux volontairement minimal)
    }

    /// Drop réel : TreasureClassEx v1.12 (`loot.rs`, feature #34 / D1) — la TC
    /// du monstre vient du manifest (`stats.treasureClass`, ex. "Act 1 H2H A")
    /// ou de la surcharge de spawn (champion/unique/boss). En NM/Enfer, la
    /// VARIANTE retail de la TC est appliquée au drop
    /// ([`difficulty::Difficulty::tc_variant`] : « Act 1 H2H A » →
    /// « Act 1 (N) H2H A », « Duriel » → « Duriel (N) ») UNIQUEMENT si la
    /// table la connaît — jamais pire que la base (repli MVP sinon). Les TC
    /// déjà suffixées par le plan de spawn d2game (monstres normaux) portent
    /// le marqueur `(N)`/`(H)` et restent inchangées. Chaîne : TC →
    /// picks/NoDrop → sous-TC → code d'item final ; or quantifié par mlvl.
    /// Repli (table absente, TC vide/inconnue) : 60 % rien / 25 % or
    /// (~niveau×10) / 15 % potion hp1 — déterministe au seed dans tous les cas.
    fn roll_drop(&mut self, x: f32, y: f32, monster_level: u32, tc_name: &str, ev: &mut Vec<GameEvent>) {
        let mlvl = monster_level.max(1);
        // 13a : variante difficulté de la TC (boss/superuniques/champions
        // spawnés avec leur TC de base — le retail dropne « Duriel (N) » en
        // NM, « Baal (H) » en Enfer ; données TreasureClassEx vérifiées par
        // golden). Garde : marqueur déjà présent ⇒ inchangé ; variante
        // absente de la table ⇒ base (comportement 13a-1 conservé).
        let tc_name = match self.difficulty.tc_suffix() {
            Some(_) if !tc_name.is_empty() && !tc_name.contains("(N)") && !tc_name.contains("(H)") => {
                let variant = Difficulty::tc_variant(tc_name, self.difficulty);
                let known = self.loot.as_ref().is_some_and(|l| l.has_tc(&variant));
                if known { variant } else { tc_name.to_string() }
            }
            _ => tc_name.to_string(),
        };
        let tc_name = tc_name.as_str();
        let loot = self.loot.as_ref();
        let drops = match loot {
            Some(l) if l.has_tc(tc_name) => l.roll(&mut self.rng, tc_name, mlvl),
            _ => loot::Loot::roll_fallback(&mut self.rng, mlvl),
        };
        for d in drops {
            let def = match loot {
                Some(l) => l.to_def(&d),
                None => {
                    if d.code == "gld" {
                        ItemDef::gold(d.gold)
                    } else {
                        ItemDef::potion()
                    }
                }
            };
            ev.push(GameEvent::ItemDropped {
                item: def.clone(),
                x: x.floor() as i32,
                y: y.floor() as i32,
            });
            self.items.push(ItemOnGround { def, x, y });
        }
    }

    // ------------------------------------------------------------------
    // Étape 16c (assassin_druid.rs) — pièges/combos Assassine + formes
    // Druide : API additive. État & logique dans assassin_druid.rs ;
    // événements du module via [`World::drain_ad_events`] (pattern
    // `pending_events`), état TRANSITOIRE (jamais persisté, comme `ai`).
    // ------------------------------------------------------------------

    /// Lance un skill du périmètre 16c : pièges (posés au héros),
    /// `Shock Field` (« Shock Web » — volée vers `aim`), formes druides
    /// (`Wearwolf`/`Wearbear`). Valide apprentissage, castabilité, classe et
    /// mana (coût `mana_cost_at` de la table). `Err` = raison exacte
    /// ([`crate::assassin_druid::AdError`]) ; état inchangé sur refus.
    pub fn cast_ad_skill(
        &mut self,
        code: &str,
        aim: Option<(i32, i32)>,
    ) -> Result<crate::assassin_druid::AdCast, crate::assassin_druid::AdError> {
        crate::assassin_druid::cast(self, code, aim)
    }

    /// Pièges posés (lecture — positions/états pour le rendu d2game).
    pub fn ad_traps(&self) -> &[crate::assassin_druid::TrapRec] {
        &self.ad.traps
    }

    /// Forme druide courante (`None` = humain).
    pub fn ad_form(&self) -> Option<crate::assassin_druid::AdForm> {
        self.ad.form.as_ref().map(|f| f.form)
    }

    /// Charges actives d'un charge-up (0 si aucune pile).
    pub fn ad_charges(&self, code: &str) -> u32 {
        self.ad.combos.iter().find(|c| c.skill.eq_ignore_ascii_case(code)).map(|c| c.charges).unwrap_or(0)
    }

    /// Draine les événements du module 16c ([`crate::assassin_druid::AdEvent`]
    /// : poses/tirs de pièges, charges/releases de combos, transformations,
    /// knockback) — contrat UI/d2game, même pattern que `pending_events`.
    pub fn drain_ad_events(&mut self) -> Vec<crate::assassin_druid::AdEvent> {
        std::mem::take(&mut self.ad.events)
    }
}
