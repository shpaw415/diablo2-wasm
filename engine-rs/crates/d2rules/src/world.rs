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
/// Version du format de sauvegarde (3 = clé du niveau courant, save P0).
pub(crate) const SAVE_VERSION: u32 = 3;
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

/// Rang d'un monstre spawné (W08/W10/W11, impl-monstres-boss) : gouverne la
/// teinte de rendu d2game et le boost appliqué au spawn. Les multiplicateurs
/// (HP/dégâts/toucher) viennent des constantes réelles de `monumod.txt`,
/// portées par [`SpawnRule`] au moment du spawn — ici on ne garde que le
/// rang (sauvegardé, exposé par [`World::monster_kind`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
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

/// Données héros (l'`Entity` est à côté, cf. `World.hero_ent`).
pub(crate) struct HeroData {
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
    /// 16a (additif, transitoire non persisté — comme les DoT ci-dessus) :
    /// malédiction de nécromancien active ([`crate::auras::ActiveCurse`] —
    /// UNE par monstre, durée `auralen`, jamais persistée).
    pub curse: Option<crate::auras::ActiveCurse>,
}

/// Monde de jeu complet (pur : aucune IO, aucune dépendance de rendu).
pub struct World {
    pub(crate) grid: Vec<Vec<bool>>,
    pub(crate) w: i32,
    pub(crate) h: i32,
    pub(crate) manifest: serde_json::Value,
    /// Code héros effectif (ex. "BA") — persisté dans la sauvegarde.
    pub(crate) hero_code: String,
    /// Clé du niveau courant dans le manifest (`"town"`, `"bloodmoor"`…) —
    /// persistée dans la sauvegarde v3 pour reprendre dans la bonne zone
    /// (warps multi-niveaux Town<->Blood Moor). La simulation n'y touche pas :
    /// c'est d2game qui la pose au chargement / switch de niveau.
    pub(crate) level_key: String,
    pub(crate) hero_tpl: HeroTpl,
    pub(crate) mon_tpls: Vec<MonTpl>,
    pub(crate) hero_ent: Entity,
    pub(crate) hero: HeroData,
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
        let hero_skills = starting_skills(&hero_code);
        let world = World {
            w,
            h,
            grid: level_collision,
            hero_code: hero_code.clone(),
            level_key: DEFAULT_LEVEL_KEY.to_string(),
            manifest: manifest.clone(),
            hero_tpl: data::parse_hero_code(&manifest, &hero_code),
            mon_tpls: data::parse_monsters(&manifest),
            loot: loot::build(),
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
                    xp_to_next: xp_threshold(&hero_code, 2),
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
                    skill: String::new(),
                    skill_level: 0,
                    stamina: 0,
                    max_stamina: 0,
                    skill_points: 0,
                    stat_points: 0,
                },
                path: Vec::new(),
                swing_ms: 0,
                swing_cast: false,
                dead_ms: 0,
                anim_ms: 0,
                run: false,
                // 12c : aucun mod de coup au démarrage (branchement
                // d'équipement futur) — chemin historique strictement identique.
                mods: combat_mods::HitMods::default(),
                spawn: (0.0, 0.0),
                inv: vec![None; INV_SLOTS],
                active_skill: default_active(&hero_code),
                skills: hero_skills,
                // EXP-H2 : accumulateur sous-unitaire (stamina.rs) — transitoire.
                stamina_milli: 0,
            },
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
        let (skill, skill_level) = self
            .hero
            .skills
            .get(self.hero.active_skill)
            .map(|s| (s.code.clone(), s.level))
            .unwrap_or_else(|| (String::new(), 0));
        self.hero.stats = HeroStats {
            level: 1,
            xp: 0,
            xp_to_next: xp_threshold(&self.hero_code, 2),
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
        };
        self.hero.stamina_milli = (t.stamina as i64) * 1000;
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
    pub fn tick(&mut self, dt_ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent> {
        // Événements produits hors tick (clic NPC W16…) d'abord, ordre réel.
        let mut ev = std::mem::take(&mut self.pending_events);
        let mut deferred_ev = std::mem::take(&mut self.deferred);
        ev.extend(deferred_ev);
        let dt = dt_ms.min(MAX_DT_MS);
        self.now_ms += dt as u64;
        if let Some(input) = input {
            if self.hero.dead_ms == 0 {
                self.handle_input(input, &mut ev);
            }
        }
        self.update_hero(dt, &mut ev);
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
        &self.hero.stats
    }

    /// Compétences connues du héros (lecture ; vide = classe sans skill de départ).
    pub fn hero_skills(&self) -> &[HeroSkill] {
        &self.hero.skills
    }

    /// Compétence actuellement sélectionnée (lecture ; `None` si aucune).
    pub fn active_skill(&self) -> Option<&HeroSkill> {
        self.hero.skills.get(self.hero.active_skill)
    }

    /// Sélectionne la compétence `index` — false si hors bornes (état inchangé).
    pub fn select_skill(&mut self, index: usize) -> bool {
        if index >= self.hero.skills.len() {
            return false;
        }
        self.hero.active_skill = index;
        let s = &self.hero.skills[index];
        self.hero.stats.skill = s.code.clone();
        self.hero.stats.skill_level = s.level;
        true
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
        self.hero.mods = mods;
    }

    pub fn unspent_skill_points(&self) -> u32 {
        self.hero.stats.skill_points
    }

    /// Accorde `n` points de skill non dépensés — hook des RÉCOMPENSES DE
    /// QUÊTES v1.12 : Den of Evil A1 +1 (`QuestReward::Unlock{flag:
    /// "skill_point"}`, quests.rs a1q1), Radament's Lair A2 +1, Izual A4 +2.
    /// L'intégration quêtes (W13) appelle ceci en consommant le flag.
    pub fn grant_skill_points(&mut self, n: u32) {
        self.hero.stats.skill_points = self.hero.stats.skill_points.saturating_add(n);
    }

    /// Injecte de l'XP au héros HORS tick (import d2s futurs, outils,
    /// garde-fous de tests) — MÊME chemin [`Self::apply_xp`] que les kills
    /// (paliers 1→`MaxLvl` de la table réelle, vie/mana/skill points par
    /// niveau) ; les [`GameEvent::HeroLeveled`] arrivent au prochain `tick`
    /// (contrat : tout événement passe par le tick).
    pub fn grant_xp(&mut self, xp: u32) {
        let mut ev = Vec::new();
        self.apply_xp(xp, &mut ev);
        self.pending_events.extend(ev);
    }

    /// Apprend (ou monte d'un niveau) le skill `code` en dépensant 1 point —
    /// validation RÉELLE : classe du héros, prérequis `reqskill1-3`,
    /// `reqlevel`, plafond `maxlvl` (20). `Err` = raison exacte
    /// ([`SkillTreeError`], affichable UI arbre) ; état inchangé sur refus.
    /// Le skill appris devient sélectionnable via [`World::select_skill`]
    /// (index = position dans [`World::hero_skills`]).
    pub fn learn_skill(&mut self, code: &str) -> Result<u32, SkillTreeError> {
        let points = self.hero.stats.skill_points;
        let class = skills_hero::class_code(&self.hero_code);
        let level =
            skills_hero::learn(&mut self.hero.skills, &class, self.hero.stats.level, code, points)?;
        self.hero.stats.skill_points = points.saturating_sub(1);
        // Skill ACTIF monté : rafraîchit le miroir HUD (stats.skill/_level),
        // même contrat que `select_skill`.
        if let Some(s) = self.hero.skills.get(self.hero.active_skill) {
            if s.code.eq_ignore_ascii_case(code) {
                self.hero.stats.skill = s.code.clone();
                self.hero.stats.skill_level = s.level;
            }
        }
        Ok(level)
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
        !self.hero.path.is_empty()
    }

    /// Le héros court-il (touche Maj, anim RN, vitesse course) ?
    pub fn hero_is_running(&self) -> bool {
        self.hero.run
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
                    self.merc_target = Some(i); // H : cible partagée au mercenaire
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
                self.hero_ranged_attack(aim, ev);
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
                    let mut item = self.items.remove(i);
                    if item.def.kind == ItemKind::Gold {
                        // 11c : plafond porté 10 000 × niveau — la part qui
                        // dépasse RESTE au sol (pile tronquée).
                        let g =
                            crate::economy::gain_carry(self.hero.stats.gold, self.hero.stats.level, item.def.amount);
                        self.hero.stats.gold += g.gained;
                        if g.gained > 0 {
                            ev.push(GameEvent::GoldGained { amount: g.gained });
                        }
                        if g.refused > 0 {
                            item.def.amount = g.refused;
                            self.items.push(item); // bourse pleine : reliquat au sol
                        }
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
            // J1 (corpse.rs) : clic sur son corps — récup immédiate à
            // portée, sinon marche vers lui + grab auto à l'arrivée.
            PlayerInput::RecoverCorpse => crate::corpse::handle_recover_corpse(self, ev),
        }
    }

    /// A* depuis la tuile du héros vers `to` (snap tuile marchable ≤ 4 si bloquée).
    pub(crate) fn find_path_from_hero(&self, to: (i32, i32)) -> Option<Vec<(f32, f32)>> {
        let start = (self.hero_ent.x.floor() as i32, self.hero_ent.y.floor() as i32);
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
        let pos_before = (self.hero_ent.x, self.hero_ent.y);
        let mut arrived = false;
        let moving = !self.hero.path.is_empty();
        if !self.hero.path.is_empty() {
            let speed = if self.hero.run {
                self.hero_tpl.run_speed.max(self.hero.stats.walk_speed)
            } else {
                self.hero.stats.walk_speed
            };
            arrived = advance_along_path(&mut self.hero_ent, &mut self.hero.path, speed, dt);
        }

        // EXP-H2 : drain/régénération d'endurance (`stamina.rs`). Course +
        // déplacement ⇒ drain ; à zéro, la course est forcée à la marche
        // (idempotent — Maj relancera quand il y aura de l'endurance).
        if crate::stamina::tick_stamina(
            self.hero.run,
            moving,
            dt,
            self.hero.stats.max_stamina,
            &mut self.hero.stamina_milli,
            &mut self.hero.stats.stamina,
        ) {
            self.hero.run = false;
        }

        // J1 (corpse.rs) : grab auto du corps à l'arrivée — le clic sur un
        // corps hors de portée a posé `corpse_pending` + le chemin ; quand le
        // chemin est épuisé, on tente la récupération (la garde de distance
        // PICKUP_RANGE est dans `recover_corpse`).
        if self.corpse_pending && self.hero.path.is_empty() {
            self.corpse_pending = false;
            self.recover_corpse(ev);
        }

        // 11c (`economy.rs`) : or au sol auto-ramassé EN ZONE
        // ([`PICKUP_RANGE`]) pendant le déplacement, sous plafond porté
        // 10 000 × niveau (immobile : rien — comportement D2 « marcher
        // sur l'or »).
        if (self.hero_ent.x, self.hero_ent.y) != pos_before {
            crate::economy::world_auto_pickup(self, ev);
        }

        // sélection anim
        let st = if !self.hero.path.is_empty() {
            if self.hero.run {
                AnimState::RN
            } else {
                AnimState::WL
            }
        } else if self.hero.swing_ms > 0 {
            // Étape 15b : cast de skill ⇒ SC (mode COF cast, extrait pour les
            // 7 classes) ; attaque simple ⇒ A1 (comportement historique).
            if self.hero.swing_cast {
                AnimState::SC
            } else {
                AnimState::A1
            }
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
        // 16a : l'aura se coupe à la mort (états perdus au respawn).
        self.on_respawn();
        self.hero_ent.x = sx;
        self.hero_ent.y = sy;
        self.hero_ent.facing = 0;
        self.hero.stats.life = self.hero.stats.max_life;
        self.hero.stats.mana = self.hero.stats.max_mana;
        // EXP-H2 : le respawn remet aussi l'endurance pleine.
        self.hero.stats.stamina = self.hero.stats.max_stamina;
        self.hero.stamina_milli = (self.hero.stats.max_stamina as i64) * 1000;
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
        // J1 : plus de course vers un corps en cours (transitoire)
        self.corpse_pending = false;
        ev.push(GameEvent::HeroRespawned { x: sx.floor() as i32, y: sy.floor() as i32 });
    }

    /// Frappe héros → monstre `i` (déjà à portée, cooldown prêt).
    fn hero_attack(&mut self, i: usize, ev: &mut Vec<GameEvent>) {
        let (dx, dy) = (self.mon_ent[i].x - self.hero_ent.x, self.mon_ent[i].y - self.hero_ent.y);
        self.hero_ent.facing = facing_from(dx, dy);
        // 16c (additif) : swing IAS des formes druides (loup dm34) —
        // HERO_SWING_MS sans transformation (comportement historique).
        self.hero.swing_ms = crate::assassin_druid::hero_swing_ms(self);
        self.hero.anim_ms = 0;
        let ac = self.monster_effective_ac(i); // 16a : Conviction (-AC) additif
        // 16c (additif) : +par4 (50) par charge de combo (progressive_tohit).
        let chance =
            hit_chance(self.hero.stats.to_hit.saturating_add(crate::assassin_druid::hero_to_hit_bonus(self)), ac);
        if (self.rng.below(100)) < chance {
            // dégâts héros = floor(rand(mindam..maxdam) × (1 + force/100))
            let base = self.rng.range_u32(self.hero_tpl.weapon_min, self.hero_tpl.weapon_max) as f32;
            let dmg = (base * (1.0 + self.hero.stats.strength as f32 / 100.0)).floor() as u32;
            // 16a (additif) : auras de dégâts (Might/Concentration/Fanaticism)
            // — ×(100+pct)/100, zéro changement sans aura active.
            let dmg = self.scale_hero_damage(dmg);
            // 16c (additif) : multiplicateur Werebear (damagepercent ln12) +
            // bonus des charges Tiger Strike (ln12 % PAR charge de l'arme) —
            // inchangé sans forme ni charges (zéro régression).
            let dmg = crate::assassin_druid::scaled_hero_melee(self, dmg, base as u32);
            // 12c (additif) : mods de coup via `combat_mods::apply_combat_mods`.
            // SANS mods, l'arithmétique est STRICTEMENT l'ancienne
            // (résist ResDm puis soustraction) ; les 3 jets de mod ne sont
            // tirés QUE si des mods sont posés — zéro RNG ajouté sur le
            // chemin historique (les suites de graines restent identiques).
            let rolls = if self.hero.mods.any_offensive() {
                (self.rng.below(100), self.rng.below(100), self.rng.below(100))
            } else {
                (0, 0, 0)
            };
            let hit = combat_mods::CombatHit {
                physical: dmg,
                elemental: 0,
                element: d2missiles::DamageType::Physical,
                mods: self.hero.mods,
                clvl: self.hero.stats.level,
                difficulty: self.difficulty,
                victim: combat_mods::Victim {
                    kind: combat_mods::VictimKind::from_mon_kind(&self.mon_dat[i].kind),
                    cur_hp: self.mon_dat[i].hp,
                    // 16a (additif) : résists EFFECTIFS — malédiction
                    // (Amplify -100 % ResDm, Lower Resist, Decrepify) +
                    // aura Conviction dans le rayon ; identiques sans.
                    res: self.monster_effective_res(i),
                },
                wearer_life: self.hero.stats.life,
                wearer_max_life: self.hero.stats.max_life,
                wearer_mana: self.hero.stats.mana,
                wearer_max_mana: self.hero.stats.max_mana,
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
            // DoT plaies ouvertes (mêlée, 12c) : remplace l'ancienne plaie.
            if let Some(w) = out.wounds {
                self.mon_dat[i].wounds = Some(w);
            }
            ev.push(GameEvent::HeroAttacked { target: i, dmg: dealt });
            // Leech vie/mana (12c) : plafonné au manque, divisors difficulté.
            if out.life_leech > 0 {
                self.hero.stats.life =
                    (self.hero.stats.life + out.life_leech).min(self.hero.stats.max_life);
            }
            if out.mana_leech > 0 {
                self.hero.stats.mana =
                    (self.hero.stats.mana + out.mana_leech).min(self.hero.stats.max_mana);
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
            // 16c (additif) : charge-up/finisher Assassine (machines combo) +
            // knockback Werebear — état 16c uniquement, zéro effet sinon.
            crate::assassin_druid::note_hero_melee_hit(self, i, dealt, base as u32);
        } else {
            ev.push(GameEvent::HeroMissed { target: i });
        }
    }

    /// Tir à distance : anime le swing, face à la cible, décoche le missile du
    /// skill actif. Skill porteur (`firebolt` SOR) : dégâts/élément OFFICIELS
    /// de la table d2missiles (`dmgSrc=Skill`, firebolt 3–8 feu), MULTIPLIÉS
    /// par la synergie du skill (skills.txt `EDmgSymPerCalc`/`DmgSymPerCalc`
    /// évaluée via calc.rs — C4 ; sans synergie : base identique, zéro
    /// régression). Skill de mêlée (BA/PA) ou aucun skill : repli `arrow`
    /// transportant les dégâts d'arme (×force, résolus AU TIR — règle D2
    /// « la flèche sait »).
    fn hero_ranged_attack(&mut self, aim: (f32, f32), ev: &mut Vec<GameEvent>) {
        if self.hero.swing_ms > 0 || self.hero.dead_ms > 0 {
            return; // cooldown en cours / héros mort
        }
        let (dx, dy) = (aim.0 - self.hero_ent.x, aim.1 - self.hero_ent.y);
        self.hero_ent.facing = facing_from(dx, dy);
        self.hero.swing_ms = HERO_SWING_MS;
        // Étape 15b : skill lancé (porteur de missile) ⇒ cast SC ;
        // tir d'arme simple (repli arrow) ⇒ A1/TH non distingués au MVP.
        self.hero.swing_cast = self
            .active_skill()
            .map(|s| s.missile.is_some())
            .unwrap_or(false);
        self.hero.anim_ms = 0;
        let from = (self.hero_ent.x, self.hero_ent.y);
        let active = self.active_skill().map(|s| (s.code.clone(), s.missile.clone()));
        let spawned = match active {
            // carrier Skill (C4) : dégâts résolus AU TIR = table (officiels
            // niv 1) × facteur de synergie du skill — les niveaux des AUTRES
            // skills du héros alimentent la formule CalcString.
            Some((skill_code, Some(missile_code))) => {
                let (dmin, dmax) = {
                    let levels = skills_hero::HeroLevels::new(
                        &self.hero.skills,
                        self.hero.stats.level,
                    );
                    self.missiles
                        .def(&missile_code)
                        .map(|def| {
                            skills_hero::synergy_scaled_damage(
                                &skill_code,
                                def.dmg_min,
                                def.dmg_max,
                                &levels,
                            )
                        })
                        .unwrap_or((0, 0))
                };
                self.missiles.spawn_skill(&missile_code, from, aim, dmin, dmax)
            }
            // carrier Weapon : l'appelant résout floor(min/max × (1 + force/100))
            Some((_, None)) | None => {
                let mult = 1.0 + self.hero.stats.strength as f32 / 100.0;
                let dmin = ((self.hero_tpl.weapon_min as f32 * mult).floor() as u32).max(1);
                let dmax = ((self.hero_tpl.weapon_max as f32 * mult).floor() as u32).max(dmin);
                self.missiles.spawn_weapon(HERO_MISSILE, from, aim, dmin, dmax)
            }
        };
        if spawned.is_some() {
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
            match me {
                d2missiles::MissileEvent::Hit { id, target, dmg, .. } => {
                    // jet de toucher : attack rating héros vs AC monstre (§3)
                    // 16a : AC EFFECTIF (Conviction -AC dans le rayon).
                    let ac = self.monster_effective_ac(target);
                    let chance = hit_chance(self.hero.stats.to_hit, ac);
                    if (self.rng.below(100)) < chance {
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
                        let roll_deadly = if elem == d2missiles::DamageType::Physical
                            && self.hero.mods.deadly_strike_pct > 0
                        {
                            self.rng.below(100)
                        } else {
                            0
                        };
                        let roll_crushing = if self.hero.mods.crushing_blow_pct > 0 {
                            self.rng.below(100)
                        } else {
                            0
                        };
                        let hit = combat_mods::CombatHit {
                            physical: 0,
                            elemental: dmg,
                            element: elem,
                            mods: self.hero.mods,
                            clvl: self.hero.stats.level,
                            difficulty: self.difficulty,
                            victim: combat_mods::Victim {
                                kind: combat_mods::VictimKind::from_mon_kind(&self.mon_dat[target].kind),
                                cur_hp: self.mon_dat[target].hp,
                                // 16a (additif) : résists EFFECTIFS (cf. mêlée).
                                res: self.monster_effective_res(target),
                            },
                            wearer_life: self.hero.stats.life,
                            wearer_max_life: self.hero.stats.max_life,
                            wearer_mana: self.hero.stats.mana,
                            wearer_max_mana: self.hero.stats.max_mana,
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
        let lvl = self.hero.stats.level;
        if lvl >= max_level_of(&self.hero_code) {
            self.hero.stats.xp_to_next = 0;
            return;
        }
        let need = xp_threshold(&self.hero_code, lvl + 1);
        self.hero.stats.xp_to_next = need.saturating_sub(self.hero.stats.xp);
    }

    /// Applique l'XP gagnée + paliers de niveau (événement par palier).
    pub(crate) fn apply_xp(&mut self, mut xp: u32, ev: &mut Vec<GameEvent>) {
        self.hero.stats.xp = self.hero.stats.xp.saturating_add(xp);
        xp = self.hero.stats.xp;
        let max = max_level_of(&self.hero_code);
        loop {
            let lvl = self.hero.stats.level;
            if lvl >= max {
                self.hero.stats.xp_to_next = 0;
                break;
            }
            let need = xp_threshold(&self.hero_code, lvl + 1);
            if xp >= need {
                self.hero.stats.level = lvl + 1;
                self.hero.stats.max_life += self.hero_tpl.life_per_level;
                self.hero.stats.life += self.hero_tpl.life_per_level;
                self.hero.stats.max_mana += self.hero_tpl.mana_per_level;
                self.hero.stats.mana += self.hero_tpl.mana_per_level;
                // ARBRE COMPÉTENCES (C2) : +1 point de skill par niveau de
                // perso (v1.12, od2 audit A2/L7 — d2core
                // `hero_stats_state.go:9-30` skillPointsPerLevel = 1) ;
                // récompenses de quêtes EN PLUS via `grant_skill_points`.
                self.hero.stats.skill_points = self.hero.stats.skill_points.saturating_add(1);
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
                    // TC effective : surcharge du spawn (champion/unique/boss)
                    // sinon celle du gabarit manifest.
                    let tc = self.mon_dat[i]
                        .tc
                        .clone()
                        .unwrap_or_else(|| self.mon_tpls[self.mon_dat[i].tpl].treasure_class.clone());
                    let (x, y) = (self.mon_ent[i].x, self.mon_ent[i].y);
                    self.roll_drop(x, y, mlvl, &tc, ev);
                    xp_gain += xp;
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

            // H (mercenaire, additif) : la cible effective de poursuite/frappe
            // est la PLUS PROCHE de (héros, mercenaire vivant). Sans mercenaire
            // (ou KO) : héros, flux historique inchangé, zéro RNG ajouté.
            let (tgt_pos, _tgt_is_merc) = match self.merc.as_ref().filter(|m| m.alive) {
                Some(m) => {
                    let dm = dist(self.mon_ent[i].x - m.ent.x, self.mon_ent[i].y - m.ent.y);
                    let dh = dist(self.mon_ent[i].x - hero_pos.0, self.mon_ent[i].y - hero_pos.1);
                    if dm < dh { ((m.ent.x, m.ent.y), true) } else { (hero_pos, false) }
                }
                None => (hero_pos, false),
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
        // H (mercenaire, additif) : le monstre frappe la cible la plus PROCHE —
        // héros ou mercenaire VIVANT. Sans mercenaire (ou KO), le flux
        // historique est inchangé (aucun tirage RNG supplémentaire).
        let (mx, my) = (self.mon_ent[i].x, self.mon_ent[i].y);
        if let Some(m) = self.merc.as_ref().filter(|m| m.alive) {
            let dm = dist(m.ent.x - mx, m.ent.y - my);
            let dh = dist(self.hero_ent.x - mx, self.hero_ent.y - my);
            // Égalité → mercenaire : le merc spawn collé au héros, l'arbitrer
            // sur un strict < ferait du héros la cible permanente (le merc
            // ne servirait jamais d'encaisseur, son rôle D2).
            if dm <= dh {
                return crate::mercenary::monster_strike_merc(self, i, ev);
            }
        }
        // 16b (additif) : une invocation vivante plus PROCHE que le héros
        // encaisse le coup (strict — à égalité le flux historique gagne).
        if crate::minions::monster_strikes_minion(self, i, ev) {
            return;
        }
        let (dmin, dmax, th) = {
            let d = &self.mon_dat[i];
            (d.dmg_min, d.dmg_max, d.th)
        };
        // défense du héros pour ce coup : dex/4 + rand(minac..maxac bouclier)
        let def = self.hero.stats.dexterity / 4
            + self.rng.range_u32(self.hero_tpl.shield_ac_min, self.hero_tpl.shield_ac_max);
        // 16a (additif) : Defiance — ×(100+pct)/100, inchangé sans aura.
        let def = self.scale_hero_defense(def);
        // 16c (additif) : Werebear skill_armor_percent (ln34) — 0 sans forme.
        let def = def.saturating_add(crate::assassin_druid::hero_defense_bonus(self, def));
        let chance = hit_chance(th, def);
        if (self.rng.below(100)) < chance {
            let raw = self.rng.range_u32(dmin, dmax).max(1);
            // 16a (additif) : Weaken/Decrepify — dégâts du monstre scalés.
            let dmg = self.scale_monster_damage(i, raw);
            self.hero.stats.life = self.hero.stats.life.saturating_sub(dmg);
            ev.push(GameEvent::HeroHit { index: i, dmg });
            // 16a (additif) : renvois Thorns (aura) + Iron Maiden
            // (malédiction) — peut abattre le monstre, même si le héros meurt.
            self.reflect_on_hero_hit(i, dmg);
            if self.hero.stats.life == 0 {
                self.hero.dead_ms = 1; // démarre le compte à rebours de respawn
                self.hero.path.clear();
                self.hero.swing_ms = 0;
                self.hero.anim_ms = 0;
                // Pénalité XP à la mort (K3) : DeathExpPenalty 0/5/10 %
                // (`patch_difficultylevels.txt`), plafonnée au plancher du
                // niveau courant (le retail ne fait jamais descendre de
                // niveau). Normal : 0 % — zéro changement.
                let pct = difficulty::embedded().row(self.difficulty).death_exp_penalty;
                if pct > 0 {
                    let lost = self.hero.stats.xp.saturating_mul(pct) / 100;
                    let floor = if self.hero.stats.level > 1 {
                        xp_threshold(&self.hero_code, self.hero.stats.level)
                    } else {
                        0
                    };
                    self.hero.stats.xp = self.hero.stats.xp.saturating_sub(lost).max(floor);
                    self.refresh_xp_to_next();
                }
                // J1 (corpse.rs) : le corps reste au sol AVEC l'inventaire ;
                // l'or porté subit le péage officiel et le reliquat tombe au
                // sol à cet endroit (règles citées dans corpse.rs).
                self.drop_corpse(ev);
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
