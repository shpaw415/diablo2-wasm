//! `minions` — INVOCATIONS v1.12 (PLAN-FIDÉLITÉ phase 16b) : squelettes/mages
//! du nécromancien, golems, revive, loups/grizzly/esprits/vignes du druide, et
//! l'IA compagnon générique (suit le héros — pattern `mercenary.rs`).
//!
//! # Données réelles (chaque valeur = `fichier:colonne`)
//! * **Skills** : `assets/patch_skills.txt` — colonnes `Id`, `summon` (Id
//!   monstats du summoneur), `pettype` (groupe de limite), `petmax` (formule
//!   de limite par niveau, portée telle quelle), `mana`/`lvlmana` (coût =
//!   `mana` + `lvlmana`×(lvl−1)), `Param1..5` + `calc1`/`aurastat*`/
//!   `passivecalc*` (scaling par niveau, portés tels quels).
//! * **Monstres summonés** : `assets/patch_monstats.txt` — colonnes `minHP`/
//!   `maxHP` (jet rand(min..max) au summon, comme le spawn monstre),
//!   `AC`, `A1MinD`/`A1MaxD`/`A1TH`, `Velocity` (tuiles/s, échelle manifest
//!   `stats.speed` : zombie1=1, fallen1=5) et `ResDm..ResPo` (W08/K). Les
//!   rangs loups/grizzly/raven portent leurs dégâts dans `patch_skills.txt`
//!   `MinDam`/`MaxDam` (A1 monstats vide : attaque de skill).
//! * **Missiles mage** : `necromage1-4` existent dans `patch_missiles.txt`
//!   (pois/cold/fire/ltng) mais ABSENTS de la table d2missiles embarquée
//!   (10 définitions MVP) — ÉCART documenté (même convention que le
//!   mercenaire sorceror, `mercenary.rs` §docs) : le mage tire le bolt de
//!   MÊME ÉLÉMENT de la table embarquée (`firebolt`/`icebolt`/`chargedbolt`,
//!   carriers `Skill` = dégâts officiels niveau 1 de `Fire Bolt` 3-8,
//!   `Ice Bolt` 3-7, `Charged Bolt` 2-6). Le poison (necromage1) est RESTANT.
//!
//! # Règles portées
//! * **Limite par `pettype`** (col `petmax`, formule exacte) : Raise
//!   Skeleton/Mage `(lvl<4)?lvl:(2+lvl/3)`, Raven `min(lvl,5)`, Spirit Wolf
//!   `min(lvl,5)`, Fenris `min(lvl,3)`, golem/totem/vine/grizzly `1`, Revive
//!   `lvl`. Au-delà de la limite, le cast REMPLACE le plus ANCIEN vivant du
//!   même `pettype` (règle D2 : le plus vieil invoqué est sacrifié).
//! * **Corpse registry** : les monstres normaux tués entrent dans un registre
//!   FIFO (16 — le shaman fallen garde 4/salle, `ai_behaviors.rs` ; le port
//!   des invocations suit le même pattern sur TOUTES les familles). Raise
//!   Skeleton/Mage et Revive CONSOMMENT le cadavre le plus proche à ≤
//!   [`CAST_RANGE`] (col `range=none` — portée MVP alignée
//!   `MERC_AGGRO_RADIUS`, restant) ; filtre rang Normal (« ni unique ni
//!   champion », même filtre que le shaman).
//! * **Scaling** (formules exactes, cf. fns `*_pct` ci-dessous) : squelette
//!   HP ×(256+8×SkelMastery)/256 (`passivecalc1`), dmg% +(lvl−3)×7 dès lvl 4
//!   (`aurastatcalc1`), TH (lvl+mastery)×15 (`aurastatcalc2`) ; mage AC
//!   +(lvl+mastery)×10 (`aurastat1`) ; golems HP% (`calc1` exacts : Clay
//!   composé, Blood/Fire = Golem Mastery `ln12`, synergie Blood `par8`
//!   absente = 0) et vitesse +40%×(gm−1) (`dm34`) ; Clay applique
//!   `item_slow` (mappé sur le refroidissement moteur ×1/2 — durée MVP 2 s,
//!   pourcentage réel restant) ; loups AC% `par4×lvl+ln56`, dmg% synergie
//!   Fenris `ln12` ; Revive HP% `200+mastery×5`, dmg% `mastery×10`, vitesse
//!   +50% (`par5`), durée 4500 frames ×[`combat_mods::FRAME_MS_D2`] =
//!   180 s retail.
//! * **Esprits (totems)** : entité IMMOBILE portant l'aura (`Oak Sage Aura`
//!   298 / `Wolverine Aura` 297 / `Barbs Aura` 296) — rayon
//!   `(30+(lvl−1)×2)` unités ÷ [`crate::ai_behaviors::UNITS_PER_TILE`]
//!   (conversion unités→tuiles du crate), effets `item_maxhp_percent`/
//!   `item_tohit_percent`+`damagepercent`/`thorns_percent` (ln34/ln56 des
//!   Param) appliqués MÉCANIQUEMENT aux invocations alliées dans le rayon
//!   (dégâts/toucher effectifs) et exposés pour le héros
//!   ([`World::active_aura_effects`] — l'application au héros (recalcul
//!   max_life au franchissement du rayon) est RESTANT).
//! * **IA compagnon générique** (pattern `mercenary.rs` §tick) : suit le
//!   héros (rayon 3,5 tuiles), attaque LA CIBLE DU HÉROS (`merc_target`)
//!   puis le monstre vivant le plus proche (rayon 8) ; les monstres
//!   retarget le plus proche de (héros, merc, invocation) — additif, sans
//!   invocation le flux est BIT-IDENTIQUE (aucun RNG ajouté). Anti-blocage :
//!   si le héros MARCHE et qu'une invocation est à < [`BLOCK_DIST`] tuile,
//!   elle s'écarte (spirale déterministe) — les couloirs restent praticables.
//! * **Durée de vie** : les invocations meurent AVEC le héros
//!   ([`World::tick`] → despawn quand `hero.dead_ms > 0`) et NE TRAVERSENT
//!   PAS les changements de niveau ([`World::set_level_key`] → despawn ;
//!   retail v1.12 : les pets ne traversent pas un warp — choix DESPAWN
//!   documenté, re-summon requis).
//! * **Persistance** : AUCUNE — les invocations ne survivent PAS au
//!   save/load (retail v1.12) : le champ `minions` n'est PAS dans la save
//!   (`save.rs` reconstruit un état vide, comme les champs transitoires).
//! * **Déterminisme** : tout jet (HP rand, élément du mage, toucher,
//!   dégâts) passe par le RNG du monde (`w.rng`) ou le système de missiles
//!   propre (seed = seed monde ^ [`MINION_MISSILE_SEED_XOR`], pattern merc)
//!   — deux mondes de même seed et mêmes actions sont identiques.
//!
//! Zéro IO, zéro rendu : état runtime porté par `World` (blocs `impl World`
//! additifs ici, comme `mercenary.rs`) ; AUCUN nouveau `GameEvent` (les
//! invocations s'observent via [`World::minions`] — le rendu d2game lira
//! ces entités, restant).

use crate::ai_behaviors::UNITS_PER_TILE;
use crate::combat_mods::{self, Dot};
use crate::path::{snap_near, DIRS, Grid};
use crate::world::{
    advance_along_path, advance_anim, dist, facing_from, hit_chance, set_anim, MonKind, MonState,
    World, ATTACK_RANGE, DYING_MS, MAX_DT_MS, MISSILE_HIT_RADIUS,
};
use crate::{AnimState, Entity, GameEvent};
use d2missiles::{DamageType, MissileSystem};

// ---------------------------------------------------------------------------
// Constantes gameplay (alignées mercenary.rs — décisions MVP documentées)
// ---------------------------------------------------------------------------

/// Rayon de suivi du héros (tuiles) — identique mercenaire.
pub(crate) const MINION_FOLLOW_RADIUS: f32 = 3.5;
/// Rayon d'aggro propre (tuiles) : cible de repli = monstre vivant le plus proche.
pub(crate) const MINION_AGGRO_RADIUS: f32 = 8.0;
/// Distance d'engagement du tir du mage (tuiles) — identique mercenaire.
pub(crate) const MINION_SHOOT_RANGE: f32 = 10.0;
/// Cooldown de swing mêlée (ms) — aligné `HERO_SWING_MS`.
pub(crate) const MINION_SWING_MS: u32 = 800;
/// Re-path périodique (ms) — aligné `REPATH_MS`.
pub(crate) const MINION_REPATH_MS: u32 = 400;
/// Portée d'incantation sur cadavre (tuiles) — MVP : `patch_skills.txt`
/// col `range=none` ne porte pas la portée réelle ; alignée
/// `MERC_AGGRO_RADIUS` (restant : portée retail du sort).
pub(crate) const CAST_RANGE: f32 = 8.0;
/// Registre des cadavres invocables — FIFO (pattern `MAX_CORPSES`=4 du
/// shaman, élargi aux invocations qui consomment).
pub(crate) const MAX_RAISABLE_CORPSES: usize = 16;
/// Anti-blocage : sous cette distance au héros EN MARCHE, l'invocation
/// s'écarte (couloirs praticables — brief 16b).
pub(crate) const BLOCK_DIST: f32 = 1.2;
/// Clay Golem `item_slow` (`aurastatcalc1=dm34`) — mappé sur le
/// refroidissement moteur (vitesse ×1/2), durée MVP 2 s (pourcentage réel
/// `Golem Mastery dm34` = (gm−1)×40 % — restant).
pub(crate) const CLAY_SLOW_MS: u32 = 2000;
/// Revive : durée `Param3`=4500 frames × 40 ms (25 fps) = 180 s retail.
pub(crate) const REVIVE_FRAMES: u32 = 4500;
/// Graine XOR du système de missiles des invocations (indépendant du héros
/// et du mercenaire — pattern `MERC_MISSILE_SEED_XOR`).
pub(crate) const MINION_MISSILE_SEED_XOR: u64 = 0x4D49_4E49_0000_0002;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Groupe de limite (col `pettype` — la limite `petmax` se compte PAR
/// GROUPE : un seul golem quel que soit son élément, un seul totem…).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetType {
    /// `skeleton` — Raise Skeleton / Raise Skeletal Mage partagent le groupe.
    Skeleton,
    /// `skeletonmage`.
    SkeletonMage,
    /// `golem` — un seul golem à la fois (toutes sortes).
    Golem,
    /// `revive`.
    Revive,
    /// `spiritwolf`.
    SpiritWolf,
    /// `fenris` (Summon Fenris = dire wolf v1.12).
    Fenris,
    /// `grizzly`.
    Grizzly,
    /// `raven` (druidhawk).
    Raven,
    /// `vine` — une seule vigne active (Poison Creeper/Carrion/Vines).
    Vine,
    /// `totem` — un seul esprit actif (Oak/Wolverine/Barbs).
    Totem,
}

/// Invocation (kind d'affichage HUD).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MinionKind {
    Skeleton,
    SkeletalMage,
    ClayGolem,
    BloodGolem,
    FireGolem,
    /// Monstre relevé (`Revive`) — stats du cadavre, boosts mastery.
    Revive,
    SpiritWolf,
    /// Summon Fenris (dire wolf).
    DireWolf,
    Grizzly,
    Raven,
    /// Vigne (Plague Poppy / Cycle of Life / Vines — mêmes règles).
    Vine,
    OakSage,
    Wolverine,
    Barbs,
}

impl MinionKind {
    /// Libellé court (HUD/debug).
    pub fn as_str(self) -> &'static str {
        match self {
            MinionKind::Skeleton => "skeleton",
            MinionKind::SkeletalMage => "skeletal_mage",
            MinionKind::ClayGolem => "clay_golem",
            MinionKind::BloodGolem => "blood_golem",
            MinionKind::FireGolem => "fire_golem",
            MinionKind::Revive => "revive",
            MinionKind::SpiritWolf => "spirit_wolf",
            MinionKind::DireWolf => "dire_wolf",
            MinionKind::Grizzly => "grizzly",
            MinionKind::Raven => "raven",
            MinionKind::Vine => "vine",
            MinionKind::OakSage => "oak_sage",
            MinionKind::Wolverine => "wolverine",
            MinionKind::Barbs => "barbs",
        }
    }

    /// Esprit immobile porteur d'aura ?
    pub fn is_totem(self) -> bool {
        matches!(self, MinionKind::OakSage | MinionKind::Wolverine | MinionKind::Barbs)
    }
}

/// Compétence d'invocation (une ligne `patch_skills.txt`).
#[derive(Debug, Clone, Copy)]
pub struct SummonSkill {
    /// Col `skill` — code exact attendu dans `hero.skills`.
    pub code: &'static str,
    /// Col `Id`.
    pub id: u32,
    /// Col `mana` — coût de base.
    pub mana: u32,
    /// Col `lvlmana` — + par niveau au-delà de 1.
    pub lvlmana: u32,
    /// Col `pettype`.
    pub pettype: PetType,
    /// Col `summon` — Id monstats du summoneur (vide pour Revive : le
    /// summoneur EST le cadavre).
    pub summon: &'static str,
    /// Kind produit.
    pub kind: MinionKind,
}

/// Les 16 compétences d'invocation portées (nécro 70-95, druide 221-247 ;
/// IronGolem 90 restant : il consume une arme de l'inventaire).
pub const SUMMON_SKILLS: [SummonSkill; 16] = [
    SummonSkill { code: "Raise Skeleton", id: 70, mana: 6, lvlmana: 1, pettype: PetType::Skeleton, summon: "necroskeleton", kind: MinionKind::Skeleton },
    SummonSkill { code: "Raise Skeletal Mage", id: 80, mana: 8, lvlmana: 1, pettype: PetType::SkeletonMage, summon: "necromage", kind: MinionKind::SkeletalMage },
    SummonSkill { code: "Clay Golem", id: 75, mana: 15, lvlmana: 3, pettype: PetType::Golem, summon: "claygolem", kind: MinionKind::ClayGolem },
    SummonSkill { code: "BloodGolem", id: 85, mana: 25, lvlmana: 4, pettype: PetType::Golem, summon: "bloodgolem", kind: MinionKind::BloodGolem },
    SummonSkill { code: "FireGolem", id: 94, mana: 50, lvlmana: 10, pettype: PetType::Golem, summon: "firegolem", kind: MinionKind::FireGolem },
    SummonSkill { code: "Revive", id: 95, mana: 45, lvlmana: 0, pettype: PetType::Revive, summon: "", kind: MinionKind::Revive },
    SummonSkill { code: "Raven", id: 221, mana: 6, lvlmana: 0, pettype: PetType::Raven, summon: "druidhawk", kind: MinionKind::Raven },
    SummonSkill { code: "Plague Poppy", id: 222, mana: 8, lvlmana: 0, pettype: PetType::Vine, summon: "plaguepoppy", kind: MinionKind::Vine },
    SummonSkill { code: "Oak Sage", id: 226, mana: 15, lvlmana: 1, pettype: PetType::Totem, summon: "oaksage", kind: MinionKind::OakSage },
    SummonSkill { code: "Summon Spirit Wolf", id: 227, mana: 15, lvlmana: 0, pettype: PetType::SpiritWolf, summon: "spiritwolf", kind: MinionKind::SpiritWolf },
    SummonSkill { code: "Cycle of Life", id: 231, mana: 10, lvlmana: 0, pettype: PetType::Vine, summon: "cycleoflife", kind: MinionKind::Vine },
    SummonSkill { code: "Heart of Wolverine", id: 236, mana: 20, lvlmana: 1, pettype: PetType::Totem, summon: "heartofwolverine", kind: MinionKind::Wolverine },
    SummonSkill { code: "Summon Fenris", id: 237, mana: 20, lvlmana: 0, pettype: PetType::Fenris, summon: "fenris", kind: MinionKind::DireWolf },
    SummonSkill { code: "Vines", id: 241, mana: 14, lvlmana: 1, pettype: PetType::Vine, summon: "vinecreature", kind: MinionKind::Vine },
    SummonSkill { code: "Spirit of Barbs", id: 246, mana: 25, lvlmana: 1, pettype: PetType::Totem, summon: "spiritofbarbs", kind: MinionKind::Barbs },
    SummonSkill { code: "Summon Grizzly", id: 247, mana: 40, lvlmana: 0, pettype: PetType::Grizzly, summon: "druidbear", kind: MinionKind::Grizzly },
];

/// Compétence d'invocation par code (col `skill`, insensible à la casse).
pub fn summon_skill(code: &str) -> Option<&'static SummonSkill> {
    SUMMON_SKILLS.iter().find(|s| s.code.eq_ignore_ascii_case(code))
}

/// Limite simultanée (col `petmax` — formules portées telles quelles ;
/// `lvl` = niveau du SKILL d'invocation).
pub fn petmax(def: &SummonSkill, lvl: u32) -> u32 {
    match def.pettype {
        // `(lvl < 4) ? lvl : (2 + lvl/3)` — Raise Skeleton & Mage
        PetType::Skeleton | PetType::SkeletonMage => {
            if lvl < 4 { lvl } else { 2 + lvl / 3 }
        }
        // `min(lvl,par2)` — Raven (par2=5)
        PetType::Raven => lvl.min(5),
        // `min(lvl,par3)` — Spirit Wolf (par3=5)
        PetType::SpiritWolf => lvl.min(5),
        // `min(lvl,par3)` — Summon Fenris (par3=3)
        PetType::Fenris => lvl.min(3),
        // `1` — golem / totem / vine / grizzly
        PetType::Golem | PetType::Totem | PetType::Vine | PetType::Grizzly => 1,
        // `lvl` — Revive
        PetType::Revive => lvl,
    }
}

// ---------------------------------------------------------------------------
// Stats monstres summonés (patch_monstats.txt v1.12, colonnes citées)
// ---------------------------------------------------------------------------

/// Ligne monstats d'un summoner (Normal ; `res` ordre ResDm/Ma/Fi/Li/Co/Po).
#[derive(Debug, Clone, Copy)]
pub(crate) struct SumStats {
    /// `minHP`.
    pub hp_min: u32,
    /// `maxHP`.
    pub hp_max: u32,
    /// `AC`.
    pub ac: u32,
    /// `A1MinD` — 0 pour les summoneurs à attaque de skill (loups/grizzly/
    /// raven : dégâts `MinDam`/`MaxDam` de patch_skills, posés au build).
    pub dmg_min: u32,
    /// `A1MaxD`.
    pub dmg_max: u32,
    /// `A1TH`.
    pub th: u32,
    /// `Velocity` (tuiles/s — même échelle que manifest `stats.speed`).
    pub velocity: f32,
    /// `ResDm/ResMa/ResFi/ResLi/ResCo/ResPo`.
    pub res: [i32; 6],
}

/// Table embarquée — patch_monstats.txt lignes des Id `summon` (Normal).
/// Zéro valeur inventée ; chaque ligne cite son Id monstats.
pub(crate) const SUMMON_MONSTATS: [(&str, SumStats); 15] = [
    ("necroskeleton", SumStats { hp_min: 21, hp_max: 21, ac: 5, dmg_min: 1, dmg_max: 2, th: 5, velocity: 13.0, res: [0; 6] }),
    ("necromage", SumStats { hp_min: 61, hp_max: 61, ac: 24, dmg_min: 1, dmg_max: 2, th: 5, velocity: 12.0, res: [0; 6] }),
    ("claygolem", SumStats { hp_min: 100, hp_max: 100, ac: 100, dmg_min: 2, dmg_max: 5, th: 40, velocity: 8.0, res: [25, 0, 0, 20, 50, 0] }),
    ("bloodgolem", SumStats { hp_min: 201, hp_max: 201, ac: 120, dmg_min: 6, dmg_max: 16, th: 60, velocity: 9.0, res: [0, 20, 0, 0, 0, 20] }),
    ("firegolem", SumStats { hp_min: 313, hp_max: 313, ac: 200, dmg_min: 10, dmg_max: 27, th: 120, velocity: 10.0, res: [0, 0, 100, 0, 0, 0] }),
    ("druidhawk", SumStats { hp_min: 20, hp_max: 32, ac: 25, dmg_min: 0, dmg_max: 0, th: 0, velocity: 10.0, res: [0; 6] }),
    ("spiritwolf", SumStats { hp_min: 60, hp_max: 82, ac: 67, dmg_min: 0, dmg_max: 0, th: 50, velocity: 5.0, res: [0; 6] }),
    ("fenris", SumStats { hp_min: 98, hp_max: 130, ac: 116, dmg_min: 0, dmg_max: 0, th: 150, velocity: 5.0, res: [0; 6] }),
    ("druidbear", SumStats { hp_min: 550, hp_max: 750, ac: 245, dmg_min: 0, dmg_max: 0, th: 300, velocity: 5.0, res: [0; 6] }),
    ("oaksage", SumStats { hp_min: 56, hp_max: 64, ac: 49, dmg_min: 0, dmg_max: 0, th: 0, velocity: 6.0, res: [0, 25, 25, 25, 25, 70] }),
    ("heartofwolverine", SumStats { hp_min: 128, hp_max: 144, ac: 123, dmg_min: 0, dmg_max: 0, th: 0, velocity: 6.0, res: [0, 25, 25, 25, 25, 70] }),
    ("spiritofbarbs", SumStats { hp_min: 200, hp_max: 226, ac: 196, dmg_min: 0, dmg_max: 0, th: 0, velocity: 6.0, res: [0, 25, 25, 25, 25, 70] }),
    ("plaguepoppy", SumStats { hp_min: 42, hp_max: 58, ac: 25, dmg_min: 0, dmg_max: 0, th: 0, velocity: 7.0, res: [0; 6] }),
    ("cycleoflife", SumStats { hp_min: 80, hp_max: 110, ac: 92, dmg_min: 0, dmg_max: 0, th: 0, velocity: 7.0, res: [0; 6] }),
    ("vinecreature", SumStats { hp_min: 138, hp_max: 192, ac: 165, dmg_min: 0, dmg_max: 0, th: 0, velocity: 7.0, res: [0; 6] }),
];

/// Stats par Id monstats (insensible à la casse).
pub(crate) fn summon_stats(id: &str) -> Option<&'static SumStats> {
    let low = id.to_ascii_lowercase();
    SUMMON_MONSTATS.iter().find(|(k, _)| *k == low).map(|(_, s)| s)
}

// ---------------------------------------------------------------------------
// Formules de scaling (patch_skills.txt — citées colonne par colonne)
// ---------------------------------------------------------------------------

/// `Skeleton Mastery` : multiplicateur HP summonés = 256+par1×mastery
/// (`passivecalc1 = mastery×par1×256`, par1=8) — HP ×mult/256.
pub(crate) fn mastery_hp_mult(mastery: u32) -> u32 {
    256 + 8 * mastery
}

/// Raise Skeleton `aurastatcalc1` : dmg% = (lvl<4)?0:(lvl−3)×par3 (par3=7).
pub(crate) fn raise_dmg_pct(lvl: u32) -> i32 {
    if lvl < 4 { 0 } else { ((lvl - 3) * 7) as i32 }
}

/// Raise Skeleton `aurastatcalc2` : TH = (lvl+mastery)×par4 (par4=15).
pub(crate) fn raise_th(lvl: u32, mastery: u32) -> u32 {
    (lvl + mastery) * 15
}

/// Raise Skeletal Mage `aurastat1` : AC = base+(lvl+mastery)×par5 (par5=10).
pub(crate) fn mage_ac(base: u32, lvl: u32, mastery: u32) -> u32 {
    base + (lvl + mastery) * 10
}

/// `Golem Mastery.ln12` = par1+(gm−1)×par2 = 20+(gm−1)×20 (HP% golems).
pub(crate) fn golem_mastery_ln12(gm: u32) -> i32 {
    if gm == 0 { 0 } else { (20 + (gm - 1) * 20) as i32 }
}

/// `Golem Mastery.dm34` = par3+(gm−1)×par4 = 0+(gm−1)×40 (vitesse% golems).
pub(crate) fn golem_mastery_dm34(gm: u32) -> i32 {
    if gm == 0 { 0 } else { ((gm - 1) * 40) as i32 }
}

/// Clay Golem `calc1` exact : (100+par1×(lvl−1))×(100+ln12+blvl×par8)/100−100
/// (par1=35 ; par8 de BloodGolem absent = 0 → synergie 0).
pub(crate) fn clay_hp_pct(lvl: u32, gm: u32) -> i32 {
    let a = 100 + 35 * (lvl.saturating_sub(1)) as i32;
    let b = 100 + golem_mastery_ln12(gm);
    a * b / 100 - 100
}

/// Esprit (totem) : rayon `aurarangecalc=ln12` = par1+(lvl−1)×par2 (unités),
/// converti en tuiles ÷ [`UNITS_PER_TILE`].
pub(crate) fn aura_range_tiles(lvl: u32) -> f32 {
    (30 + (lvl.saturating_sub(1)) * 2) as f32 / UNITS_PER_TILE
}

/// Effet d'aura actif (porté par un totem vivant).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuraEffect {
    /// Totem source.
    pub totem: MinionKind,
    /// Rayon en tuiles (col `aurarangecalc`).
    pub range_tiles: f32,
    /// `damagepercent` (Wolverine `ln56` : par5=20, par6 absent → 20).
    pub dmg_pct: i32,
    /// `item_tohit_percent` (Wolverine `ln34` : par3=25, par4=7).
    pub th_pct: i32,
    /// `item_maxhp_percent` (Oak Sage `ln34` : par3=30, par4=5).
    pub maxhp_pct: i32,
    /// `thorns_percent` (Barbs `ln34` : par3=50, par4=10) — exposé, NON
    /// mécanique (restant : renvoi de dégâts).
    pub thorns_pct: i32,
}

/// Aura d'un totem par kind (valeurs des skills 296/297/298).
pub(crate) fn totem_aura(kind: MinionKind, lvl: u32) -> Option<AuraEffect> {
    let l = lvl.saturating_sub(1);
    let range = aura_range_tiles(lvl);
    match kind {
        // Oak Sage Aura (298) : maxhp% ln34 = 30+(lvl−1)×5
        MinionKind::OakSage => Some(AuraEffect {
            totem: kind, range_tiles: range,
            dmg_pct: 0, th_pct: 0, maxhp_pct: (30 + l * 5) as i32, thorns_pct: 0,
        }),
        // Wolverine Aura (297) : tohit% ln34 = 25+(lvl−1)×7 ; dmg% ln56 = 20
        MinionKind::Wolverine => Some(AuraEffect {
            totem: kind, range_tiles: range,
            dmg_pct: 20, th_pct: (25 + l * 7) as i32, maxhp_pct: 0, thorns_pct: 0,
        }),
        // Barbs Aura (296) : thorns% ln34 = 50+(lvl−1)×10
        MinionKind::Barbs => Some(AuraEffect {
            totem: kind, range_tiles: range,
            dmg_pct: 0, th_pct: 0, maxhp_pct: 0, thorns_pct: (50 + l * 10) as i32,
        }),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// État runtime
// ---------------------------------------------------------------------------

/// Cadavre invocable (registre FIFO — TOUT monstre normal tué, toutes
/// familles ; le filtre famille fallen reste côté shaman `ai_behaviors`).
#[derive(Debug, Clone)]
pub struct RaisableCorpse {
    /// Index de gabarit manifest (Revive relance ce gabarit).
    pub tpl: usize,
    /// Position (tuiles).
    pub x: f32,
    pub y: f32,
    /// PV max du monstre à sa mort (base du boost Revive).
    pub max_hp: u32,
    /// AC/TH/dégâts effectifs du monstre à sa mort (Revive les reprend).
    pub ac: u32,
    pub th: u32,
    pub dmg_min: u32,
    pub dmg_max: u32,
    /// Horloge du monde à l'enregistrement.
    pub born_ms: u64,
}

/// Invocation runtime (entité + stats effectives + FSM compagnon).
#[derive(Debug, Clone)]
pub struct MinionData {
    /// Kind d'affichage.
    pub kind: MinionKind,
    /// Groupe de limite.
    pub pettype: PetType,
    /// Entité (position/facing/anim).
    pub ent: Entity,
    /// PV courants.
    pub hp: u32,
    /// PV max (base ; l'aura Oak Sage est EFFECTIVE — cf. docs).
    pub max_hp: u32,
    /// Dégâts mêlée effectifs (base, sans aura).
    pub dmg_min: u32,
    pub dmg_max: u32,
    /// Toucher effectif (base, sans aura).
    pub th: u32,
    /// AC effectif.
    pub ac: u32,
    /// Vitesse (tuiles/s, monstats Velocity × boosts).
    pub speed: f32,
    /// Élément du mage (None hors mage) — choisit le missile.
    pub elem: Option<DamageType>,
    /// Durée de vie restante (Revive : 4500 frames) — None = permanente.
    pub ttl_ms: Option<u32>,
    /// Aura portée (totems uniquement).
    pub aura: Option<AuraEffect>,
    /// Poison des vignes (total `calc1` = par3×(lvl−1) — 0 au niveau 1).
    pub(crate) poison_dmg: u32,
    pub(crate) path: Vec<(f32, f32)>,
    pub(crate) repath_ms: u32,
    pub(crate) swing_ms: u32,
    pub(crate) anim_ms: u32,
    /// > 0 : mort en cours (anim DD, puis retrait).
    pub(crate) dying_ms: u32,
}

/// État des invocations du monde (additif, JAMAIS persisté — retail : les
/// invocations ne survivent pas au save/load).
#[derive(Debug, Clone)]
pub struct Minions {
    /// Invocations vivantes (ordre d'invocation — le plus ancien d'abord).
    pub(crate) list: Vec<MinionData>,
    /// Cadavres invocables (FIFO).
    pub(crate) corpses: Vec<RaisableCorpse>,
    /// Missiles PROPRES aux mages (système séparé — pattern mercenaire).
    pub(crate) missiles: MissileSystem,
}

impl Minions {
    /// État vide, missiles seedés (déterminisme par partie).
    pub(crate) fn with_seed(seed: u64) -> Minions {
        Minions {
            list: Vec::new(),
            corpses: Vec::new(),
            missiles: MissileSystem::with_seed(seed ^ MINION_MISSILE_SEED_XOR),
        }
    }

    /// Purge complète (mort du héros / changement de niveau).
    pub(crate) fn clear(&mut self) {
        self.list.clear();
        self.corpses.clear();
        self.missiles.clear();
    }
}

impl Default for Minions {
    fn default() -> Minions {
        Minions::with_seed(0)
    }
}

// ---------------------------------------------------------------------------
// Erreurs
// ---------------------------------------------------------------------------

/// Échec d'invocation ([`World::cast_summon`]) — état inchangé.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SummonError {
    /// Le héros ne connaît pas la compétence (jamais apprise — la classe est
    /// déjà validée par `learn_skill`).
    SkillUnknown,
    /// Mana insuffisant (coût = `mana` + `lvlmana`×(lvl−1)).
    NotEnoughMana { cost: u32, have: u32 },
    /// Sort à cadavre (Raise Skeleton/Mage, Revive) sans cadavre à portée.
    NoCorpse,
}

// ---------------------------------------------------------------------------
// Hooks appelés par world.rs (points d'accroche additifs)
// ---------------------------------------------------------------------------

/// MORT d'un monstre (branche DD de `update_monsters`, après
/// `ai_behaviors::register_corpse`) : enregistre le cadavre invocable si
/// rang Normal (filtre « ni unique ni champion », même règle que le shaman).
/// Sans RNG, sans effet de bord sur le flux existant.
pub(crate) fn register_raisable_corpse(w: &mut World, i: usize) {
    if w.mon_dat[i].kind != MonKind::Normal {
        return;
    }
    if w.minions.corpses.len() >= MAX_RAISABLE_CORPSES {
        w.minions.corpses.remove(0); // FIFO
    }
    let d = &w.mon_dat[i];
    w.minions.corpses.push(RaisableCorpse {
        tpl: d.tpl,
        x: w.mon_ent[i].x,
        y: w.mon_ent[i].y,
        max_hp: d.max_hp,
        ac: d.ac,
        th: d.th,
        dmg_min: d.dmg_min,
        dmg_max: d.dmg_max,
        born_ms: w.now_ms,
    });
}

/// `update_monsters` : la cible de poursuite du monstre devient
/// l'invocation vivante la plus proche SI elle bat strictement la cible
/// courante (héros/merc). Sans invocation : identité — flux bit-identique.
pub(crate) fn nearest_minion_override(w: &World, cur: (f32, f32), mx: f32, my: f32) -> (f32, f32) {
    let dc = dist(cur.0 - mx, cur.1 - my);
    let mut best: Option<(f32, f32)> = None;
    for m in &w.minions.list {
        if m.dying_ms > 0 {
            continue;
        }
        let d = dist(m.ent.x - mx, m.ent.y - my);
        if d < dc && best.map_or(true, |(bx, by)| dist(bx - mx, by - my) > d) {
            best = Some((m.ent.x, m.ent.y));
        }
    }
    best.unwrap_or(cur)
}

/// `monster_strike` : le monstre `i` frappe l'invocation vivante la plus
/// proche SI elle bat strictement le héros (le merc reste prioritaire à
/// égale distance — bloc mercenaire inchangé, exécuté avant). Renvoie true
/// si le coup s'est porté sur une invocation (le héros n'est pas frappé) ;
/// false = flux historique (aucun RNG consommé si aucune invocation).
pub(crate) fn monster_strikes_minion(w: &mut World, i: usize, _ev: &mut Vec<GameEvent>) -> bool {
    if w.minions.list.iter().all(|m| m.dying_ms > 0) {
        return false;
    }
    let (mx, my) = (w.mon_ent[i].x, w.mon_ent[i].y);
    let dh = dist(w.hero_ent.x - mx, w.hero_ent.y - my);
    let mut best: Option<usize> = None;
    let mut bd = dh; // strict : à égalité le héros garde le coup (flux sûr)
    for (k, m) in w.minions.list.iter().enumerate() {
        if m.dying_ms > 0 {
            continue;
        }
        let d = dist(m.ent.x - mx, m.ent.y - my);
        if d < bd {
            bd = d;
            best = Some(k);
        }
    }
    let Some(k) = best else { return false };
    let (dmin, dmax, th) = (w.mon_dat[i].dmg_min, w.mon_dat[i].dmg_max, w.mon_dat[i].th);
    let ac = w.minions.list[k].ac;
    if (w.rng.below(100)) < hit_chance(th, ac) {
        let dmg = w.rng.range_u32(dmin, dmax).max(1);
        let m = &mut w.minions.list[k];
        m.hp = m.hp.saturating_sub(dmg);
        if m.hp == 0 {
            m.dying_ms = 1;
            m.path.clear();
            m.swing_ms = 0;
            m.anim_ms = 0;
            m.ent.anim = AnimState::DD;
            m.ent.anim_frame = 0;
        }
    }
    true // le monstre a engagé l'invocation (touchée ou ratée) — pas le héros
}

// ---------------------------------------------------------------------------
// Tick — IA compagnon (pattern mercenary.rs, additif)
// ---------------------------------------------------------------------------

/// FSM des invocations : vieillissement (DD/ttl), décision (attaque de la
/// cible du héros / plus proche monstre / suivi du héros / anti-blocage),
/// combat (mêlée ou missile du mage), puis missiles. Appelé par
/// `World::tick` après le mercenaire.
pub(crate) fn tick_minions(w: &mut World, dt: u32, _ev: &mut Vec<GameEvent>) {
    let dt = dt.min(MAX_DT_MS);
    if w.minions.list.is_empty() && w.minions.missiles.is_empty() {
        return; // sans invocation : zéro coût, zéro RNG — flux bit-identique
    }
    // Mort du héros : le lien pet meurt avec le maître (retail).
    if w.hero.dead_ms > 0 {
        w.minions.clear();
        return;
    }

    // --- vieillissement : anim DD des morts + ttl des Revivés + cooldowns ---
    let mut i = 0;
    while i < w.minions.list.len() {
        let m = &mut w.minions.list[i];
        if m.dying_ms > 0 {
            m.dying_ms = m.dying_ms.saturating_add(dt);
            advance_anim(&mut m.ent, &mut m.anim_ms, dt, false);
            if m.dying_ms >= DYING_MS {
                w.minions.list.remove(i);
                continue;
            }
            i += 1;
            continue;
        }
        m.swing_ms = m.swing_ms.saturating_sub(dt);
        m.repath_ms = m.repath_ms.saturating_sub(dt);
        if let Some(ttl) = m.ttl_ms.as_mut() {
            *ttl = ttl.saturating_sub(dt);
            if *ttl == 0 {
                w.minions.list.remove(i); // Revive : fin des 180 s retail
                continue;
            }
        }
        i += 1;
    }

    let hero_moving = !w.hero.path.is_empty();
    let (hx, hy) = (w.hero_ent.x, w.hero_ent.y);

    // --- décision + application par invocation ---
    enum Act {
        Melee(usize),
        Shoot(usize),
        Walk(f32, f32),
        Sidestep,
        Idle,
    }
    let mut i = 0;
    while i < w.minions.list.len() {
        let act = {
            let m = &w.minions.list[i];
            if m.kind.is_totem() {
                Act::Idle // esprit : immobile, porte l'aura (rien d'autre)
            } else {
                // cible : LA CIBLE DU HÉROS d'abord (pattern merc), sinon le
                // monstre vivant le plus proche de l'invocation (rayon aggro)
                let mut target: Option<usize> = w
                    .merc_target
                    .filter(|&t| t < w.mon_ent.len() && w.mon_dat[t].state != MonState::Dying);
                if target.is_none() {
                    let mut bd = MINION_AGGRO_RADIUS;
                    for (t, (e, d)) in w.mon_ent.iter().zip(w.mon_dat.iter()).enumerate() {
                        if d.state == MonState::Dying {
                            continue;
                        }
                        let dd = dist(e.x - m.ent.x, e.y - m.ent.y);
                        if dd <= bd {
                            bd = dd;
                            target = Some(t);
                        }
                    }
                }
                match target {
                    Some(t) => {
                        let (tx, ty) = (w.mon_ent[t].x, w.mon_ent[t].y);
                        let d = dist(tx - m.ent.x, ty - m.ent.y);
                        if m.elem.is_some() && d <= MINION_SHOOT_RANGE {
                            Act::Shoot(t)
                        } else if d <= ATTACK_RANGE {
                            Act::Melee(t)
                        } else {
                            Act::Walk(tx, ty)
                        }
                    }
                    None => {
                        let dh = dist(hx - m.ent.x, hy - m.ent.y);
                        if dh > MINION_FOLLOW_RADIUS {
                            Act::Walk(hx, hy)
                        } else if hero_moving && dh < BLOCK_DIST {
                            Act::Sidestep // ne bloque pas le couloir du héros
                        } else {
                            Act::Idle
                        }
                    }
                }
            }
        };
        match act {
            Act::Melee(t) => minion_melee(w, i, t),
            Act::Shoot(t) => minion_shoot(w, i, t),
            Act::Walk(wx, wy) => {
                let needs = w.minions.list[i].path.is_empty() || w.minions.list[i].repath_ms == 0;
                if needs {
                    let mt = (
                        w.minions.list[i].ent.x.floor() as i32,
                        w.minions.list[i].ent.y.floor() as i32,
                    );
                    if let Some(p) = w.path_between(mt, (wx.floor() as i32, wy.floor() as i32)) {
                        w.minions.list[i].path = p;
                    }
                    w.minions.list[i].repath_ms = MINION_REPATH_MS;
                }
            }
            Act::Sidestep => {
                if let Some(p) = sidestep_path(w, i) {
                    w.minions.list[i].path = p;
                }
            }
            Act::Idle => w.minions.list[i].path.clear(),
        }

        // déplacement + anim (totems immobiles)
        if !w.minions.list[i].kind.is_totem() {
            let speed = w.minions.list[i].speed.max(0.5);
            let m = &mut w.minions.list[i];
            if !m.path.is_empty() {
                advance_along_path(&mut m.ent, &mut m.path, speed, dt);
            }
            let st = if m.swing_ms > 0 {
                AnimState::A1
            } else if !m.path.is_empty() {
                AnimState::WL
            } else {
                AnimState::NU
            };
            set_anim(&mut m.ent, st, &mut m.anim_ms);
            advance_anim(&mut m.ent, &mut m.anim_ms, dt, matches!(st, AnimState::NU | AnimState::WL));
        } else {
            let m = &mut w.minions.list[i];
            set_anim(&mut m.ent, AnimState::NU, &mut m.anim_ms);
            advance_anim(&mut m.ent, &mut m.anim_ms, dt, true);
        }
        i += 1;
    }

    tick_minion_missiles(w, dt);
}

/// Mêlée d'invocation : jet `200×TH/(TH+AC)` [5,95] (§3 GAME-DATA), dégâts
/// effectifs (auras totems incluses), résist physique du monstre (ResDm).
/// Clay Golem : `item_slow` → refroidissement moteur (durée MVP). Vigne :
/// attaque de POISON (`EType=pois`) — DoT `calc1` résisté ResPo.
fn minion_melee(w: &mut World, k: usize, t: usize) {
    if w.minions.list[k].swing_ms > 0 {
        return; // cooldown
    }
    let (tx, ty) = (w.mon_ent[t].x, w.mon_ent[t].y);
    {
        let m = &mut w.minions.list[k];
        m.ent.facing = facing_from(tx - m.ent.x, ty - m.ent.y);
        m.swing_ms = MINION_SWING_MS;
        m.anim_ms = 0;
    }
    let Some((dmin, dmax, th)) = w.minion_effective_stats(k) else { return };
    let kind = w.minions.list[k].kind;
    let poison_total = w.minions.list[k].poison_dmg;
    let ac = w.mon_dat[t].ac;
    if (w.rng.below(100)) < hit_chance(th, ac) {
        if kind == MinionKind::Vine {
            // attaque de poison pure (Plague Poppy EType=pois) : DoT calc1
            if poison_total > 0 {
                let total = crate::difficulty::apply_resist(poison_total, w.mon_dat[t].res[5]);
                if total > 0 {
                    // durée MVP : 1 tick (25 frames — durée retail restante)
                    w.mon_dat[t].poison =
                        Some(Dot { dmg_per_tick: total, remaining_ticks: 1, acc_ms: 0 });
                }
            }
        } else {
            let dmg = w.rng.range_u32(dmin, dmax).max(1);
            let dmg = crate::difficulty::apply_resist(dmg, w.mon_dat[t].res[0]);
            w.mon_dat[t].hp = w.mon_dat[t].hp.saturating_sub(dmg);
            if kind == MinionKind::ClayGolem && dmg > 0 {
                w.mon_dat[t].chill_ms = CLAY_SLOW_MS; // item_slow (MVP 2 s)
            }
            if w.mon_dat[t].hp == 0 {
                start_mon_death(w, t);
            }
        }
    }
    // raté : pas d'événement (même convention que le mercenaire)
}

/// Tir du mage : bolt de MÊME ÉLÉMENT de la table d2missiles embarquée
/// (carrier Skill — dégâts officiels niveau 1 ; ÉCART poison, cf. docs).
fn minion_shoot(w: &mut World, k: usize, t: usize) {
    let (elem, ready) = {
        let m = &w.minions.list[k];
        (m.elem, m.swing_ms == 0)
    };
    let Some(elem) = elem else { return };
    if !ready {
        return; // cooldown
    }
    let (carrier, dmin, dmax) = match elem {
        DamageType::Fire => ("firebolt", 3, 8),
        DamageType::Cold => ("icebolt", 3, 7),
        _ => ("chargedbolt", 2, 6),
    };
    let (tx, ty) = (w.mon_ent[t].x, w.mon_ent[t].y);
    let from = (w.minions.list[k].ent.x, w.minions.list[k].ent.y);
    {
        let m = &mut w.minions.list[k];
        m.ent.facing = facing_from(tx - m.ent.x, ty - m.ent.y);
        m.swing_ms = MINION_SWING_MS;
        m.anim_ms = 0;
    }
    w.minions.missiles.spawn_skill(carrier, from, (tx, ty), dmin, dmax);
}

/// Missiles des mages : collision monstres (rayon partagé), jet de toucher
/// avec le TH du mage, résist élémentaire du monstre (W08/K). Séparé du
/// flux héros/merc — aucun changement dans d2missiles.
fn tick_minion_missiles(w: &mut World, dt: u32) {
    if w.minions.missiles.is_empty() {
        return;
    }
    // élément par missile (snapshot — tick_walls consomme les touches)
    let elem_by_id: Vec<(u32, DamageType)> =
        w.minions.missiles.missiles().iter().map(|m| (m.id, m.elem)).collect();
    let th = w
        .minions
        .list
        .iter()
        .find(|m| m.kind == MinionKind::SkeletalMage && m.dying_ms == 0)
        .map(|m| m.th)
        .unwrap_or(5);
    // snapshot des cibles (comme update_missiles : monstres figés au tick)
    let targets: Vec<(f32, f32, bool)> = w
        .mon_ent
        .iter()
        .zip(w.mon_dat.iter())
        .map(|(e, d)| (e.x, e.y, d.state == MonState::Dying))
        .collect();
    let (grid, gw, gh) = (&w.grid, w.w, w.h);
    let events = w.minions.missiles.tick_walls(
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
        if let d2missiles::MissileEvent::Hit { id, target, dmg, .. } = me {
            let elem = elem_by_id
                .iter()
                .find(|(i, _)| *i == id)
                .map(|(_, e)| *e)
                .unwrap_or_default();
            let ac = w.mon_dat[target].ac;
            if (w.rng.below(100)) < hit_chance(th, ac) {
                let dmg = crate::difficulty::apply_resist(
                    dmg,
                    w.mon_dat[target].res[crate::Difficulty::resist_index(elem)],
                );
                if dmg == 0 {
                    continue; // immunité : missile consumé, rien ne passe (W08/K)
                }
                w.mon_dat[target].hp = w.mon_dat[target].hp.saturating_sub(dmg);
                if w.mon_dat[target].hp == 0 {
                    start_mon_death(w, target);
                }
            }
        }
    }
}

/// Anti-blocage : écarte l'invocation `k` du passage du héros — première
/// tuile marchable de la spirale déterministe (rayons 2 puis 3 autour du
/// héros, ordre [`DIRS`]), atteinte par A*.
fn sidestep_path(w: &mut World, k: usize) -> Option<Vec<(f32, f32)>> {
    let (hx, hy) = (w.hero_ent.x.floor() as i32, w.hero_ent.y.floor() as i32);
    for r in [2i32, 3] {
        for &(dx, dy) in DIRS.iter() {
            let (tx, ty) = (hx + dx * r, hy + dy * r);
            if !w.is_walkable(tx, ty) {
                continue;
            }
            let mt = (
                w.minions.list[k].ent.x.floor() as i32,
                w.minions.list[k].ent.y.floor() as i32,
            );
            if let Some(p) = w.path_between(mt, (tx, ty)) {
                if !p.is_empty() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Passe le monstre `t` en anim DD (mort) — même bloc que les branches
/// héros/mercenaire ; le flux DD existant (drop, XP, cadavres) s'applique.
fn start_mon_death(w: &mut World, t: usize) {
    let ent = &mut w.mon_ent[t];
    let dat = &mut w.mon_dat[t];
    dat.state = MonState::Dying;
    dat.dying_ms = 0;
    dat.anim_ms = 0;
    dat.path.clear();
    ent.anim = AnimState::DD;
    ent.anim_frame = 0;
}

// ---------------------------------------------------------------------------
// Cast — impl World (bloc additif, pattern mercenary.rs)
// ---------------------------------------------------------------------------

/// Niveau d'un skill du héros (0 = non appris).
fn hero_level_of(w: &World, code: &str) -> u32 {
    w.hero
        .skills
        .iter()
        .find(|s| s.code.eq_ignore_ascii_case(code))
        .map(|s| s.level)
        .unwrap_or(0)
}

/// Ajoute ou monte un skill du héros (tests — la validation arbre réelle
/// reste `learn_skill`). `pub(crate)` : même crate uniquement.
#[cfg(test)]
pub(crate) fn grant_hero_skill(w: &mut World, code: &str, level: u32) {
    match w.hero.skills.iter_mut().find(|s| s.code.eq_ignore_ascii_case(code)) {
        Some(s) => s.level = level,
        None => w
            .hero
            .skills
            .push(crate::HeroSkill { code: code.to_string(), level, missile: None }),
    }
}

impl World {
    /// Invocations vivantes (ordre d'invocation) — rendu d2game/HUD.
    pub fn minions(&self) -> &[MinionData] {
        &self.minions.list
    }

    /// Cadavres invocables enregistrés (introspection/tests).
    pub fn raisable_corpses(&self) -> &[RaisableCorpse] {
        &self.minions.corpses
    }

    /// Missiles des mages en vol (rendu d2game).
    pub fn minion_missiles(&self) -> &[d2missiles::Missile] {
        self.minions.missiles.missiles()
    }

    /// Auras actives (totems vivants) — HUD/intégration héros (l'effet
    /// mécanique sur les invocations est appliqué dans le tick).
    pub fn active_aura_effects(&self) -> Vec<AuraEffect> {
        self.minions
            .list
            .iter()
            .filter(|m| m.dying_ms == 0)
            .filter_map(|m| m.aura)
            .collect()
    }

    /// Stats EFFECTIVES de l'invocation `index` — auras de totems alliés
    /// couvrant sa position appliquées : `(dmg_min, dmg_max, th)`.
    /// `None` hors bornes.
    pub fn minion_effective_stats(&self, index: usize) -> Option<(u32, u32, u32)> {
        let m = self.minions.list.get(index)?;
        let (dp, tp) = self.ally_aura_bonus(m.ent.x, m.ent.y);
        let dmin = ((m.dmg_min as i32 * (100 + dp)) / 100).max(1) as u32;
        let dmax = (((m.dmg_max as i32 * (100 + dp)) / 100).max(dmin as i32)).max(1) as u32;
        let th = ((m.th as i32 * (100 + tp)) / 100).max(1) as u32;
        Some((dmin, dmax, th))
    }

    /// Bonus d'aura cumulé des totems alliés couvrant `(x, y)` (dmg%, th%).
    pub(crate) fn ally_aura_bonus(&self, x: f32, y: f32) -> (i32, i32) {
        let mut dp = 0;
        let mut tp = 0;
        for m in &self.minions.list {
            if m.dying_ms > 0 {
                continue;
            }
            if let Some(a) = m.aura {
                if dist(m.ent.x - x, m.ent.y - y) <= a.range_tiles {
                    dp += a.dmg_pct;
                    tp += a.th_pct;
                }
            }
        }
        (dp, tp)
    }

    /// Incante une compétence d'invocation (`cast_summon("Raise Skeleton")`).
    /// Valide : héros connaît le skill (`hero.skills` — la classe est déjà
    /// validée par `learn_skill`), mana suffisant (`mana`+`lvlmana`×(lvl−1)),
    /// cadavre à portée pour les sorts à cadavre. Applique la limite
    /// `petmax` PAR `pettype` — au-delà, le plus ANCIEN vivant du groupe est
    /// remplacé (règle D2). Déduit le mana. Renvoie `Ok(nombre vivants du
    /// groupe après cast)` ou l'erreur (état inchangé).
    pub fn cast_summon(&mut self, skill_code: &str) -> Result<u32, SummonError> {
        let def = match summon_skill(skill_code) {
            Some(d) => d,
            None => return Err(SummonError::SkillUnknown),
        };
        let lvl = match hero_level_of(self, def.code) {
            0 => return Err(SummonError::SkillUnknown),
            l => l,
        };
        let cost = def.mana + def.lvlmana * (lvl - 1);
        if self.hero.stats.mana < cost {
            return Err(SummonError::NotEnoughMana { cost, have: self.hero.stats.mana });
        }

        // --- résolution de la position (+ consommation du cadavre) ---
        let (hx, hy) = (self.hero_ent.x, self.hero_ent.y);
        let corpse_based = matches!(
            def.kind,
            MinionKind::Skeleton | MinionKind::SkeletalMage | MinionKind::Revive
        );
        let mut minion = if corpse_based {
            // cadavre le plus PROCHE du héros à ≤ CAST_RANGE (rang Normal
            // déjà filtré à l'enregistrement)
            let mut best: Option<usize> = None;
            let mut bd = CAST_RANGE;
            for (k, c) in self.minions.corpses.iter().enumerate() {
                let d = dist(c.x - hx, c.y - hy);
                if d <= bd {
                    bd = d;
                    best = Some(k);
                }
            }
            let Some(k) = best else { return Err(SummonError::NoCorpse) };
            let c = self.minions.corpses.remove(k);
            self.build_summon(def, lvl, Some(&c), (c.x, c.y))
        } else {
            // à la volée : tuile marchable près du héros (pattern hire_merc)
            let g = Grid { w: self.w, h: self.h, blocked: &self.grid };
            let spot = snap_near(&g, (hx.floor() as i32, hy.floor() as i32), 2)
                .unwrap_or((hx.floor() as i32, hy.floor() as i32));
            let (x, y) = (spot.0 as f32 + 0.5, spot.1 as f32 + 0.5);
            self.build_summon(def, lvl, None, (x, y))
        };
        // orientation vers le héros (comme hire_merc)
        minion.ent.facing = facing_from(hx - minion.ent.x, hy - minion.ent.y);

        // --- limite petmax PAR GROUPE : remplace le plus ancien vivant ---
        let count_of =
            |w: &World| -> u32 {
                w.minions
                    .list
                    .iter()
                    .filter(|m| m.dying_ms == 0 && m.pettype == def.pettype)
                    .count() as u32
            };
        if count_of(self) >= petmax(def, lvl) {
            if let Some(idx) = self
                .minions
                .list
                .iter()
                .position(|m| m.dying_ms == 0 && m.pettype == def.pettype)
            {
                self.minions.list.remove(idx);
            }
        }

        self.minions.list.push(minion);
        self.hero.stats.mana -= cost;
        Ok(count_of(self))
    }

    /// Construit les stats de l'invocation (formules `patch_skills.txt`
    /// exactes — cf. docs de module). Consomme le RNG du monde (HP rand,
    /// élément du mage) — déterminisme par seed. `corpse` porte les stats
    /// du monstre relevé (Revive uniquement ; Raise SKELETON ne re-booste
    /// PAS depuis le cadavre : ses stats viennent des formules du skill).
    fn build_summon(
        &mut self,
        def: &SummonSkill,
        lvl: u32,
        corpse: Option<&RaisableCorpse>,
        at: (f32, f32),
    ) -> MinionData {
        let mastery = hero_level_of(self, "Skeleton Mastery");
        let gm = hero_level_of(self, "Golem Mastery");
        let fenris = hero_level_of(self, "Summon Fenris");
        let s = summon_stats(def.summon);
        let ent = Entity { x: at.0, y: at.1, facing: 0, anim: AnimState::NU, anim_frame: 0 };
        let base = MinionData {
            kind: def.kind,
            pettype: def.pettype,
            ent,
            hp: 1,
            max_hp: 1,
            dmg_min: s.map(|x| x.dmg_min).unwrap_or(1),
            dmg_max: s.map(|x| x.dmg_max).unwrap_or(1),
            th: s.map(|x| x.th).unwrap_or(5),
            ac: s.map(|x| x.ac).unwrap_or(5),
            speed: s.map(|x| x.velocity).unwrap_or(6.0),
            elem: None,
            ttl_ms: None,
            aura: None,
            poison_dmg: 0,
            path: Vec::new(),
            repath_ms: 0,
            swing_ms: 0,
            anim_ms: 0,
            dying_ms: 0,
        };
        match def.kind {
            // --- squelette : HP×(256+8m)/256, dmg% (lvl−3)×7, TH (lvl+m)×15
            MinionKind::Skeleton => {
                let base_hp = s.map(|x| x.hp_max).unwrap_or(21);
                let mut m = base;
                m.max_hp = (base_hp * mastery_hp_mult(mastery)) / 256;
                m.hp = m.max_hp;
                let pct = raise_dmg_pct(lvl).max(0) as u32;
                m.dmg_min = (m.dmg_min * (100 + pct)) / 100;
                m.dmg_max = (m.dmg_max * (100 + pct)) / 100;
                m.th = raise_th(lvl, mastery);
                m
            }
            // --- mage : HP comme le squelette, AC +(lvl+m)×10, élément rng
            MinionKind::SkeletalMage => {
                let base_hp = s.map(|x| x.hp_max).unwrap_or(61);
                let mut m = base;
                m.max_hp = (base_hp * mastery_hp_mult(mastery)) / 256;
                m.hp = m.max_hp;
                m.ac = mage_ac(m.ac, lvl, mastery);
                // ÉCART documenté : poison (necromage1) absent de la table
                // d2missiles embarquée — élément uniforme parmi les trois
                // bolts portés (fire/cold/lightning).
                m.elem = Some(match self.rng.below(3) {
                    0 => DamageType::Fire,
                    1 => DamageType::Cold,
                    _ => DamageType::Lightning,
                });
                // dégâts = carrier Skill niveau 1 officiel (convention docs
                // merc sorceror) : Fire Bolt 3-8, Ice Bolt 3-7, ChBolt 2-6
                let (dmin, dmax) = match m.elem.unwrap() {
                    DamageType::Fire => (3, 8),
                    DamageType::Cold => (3, 7),
                    _ => (2, 6),
                };
                m.dmg_min = dmin;
                m.dmg_max = dmax;
                m
            }
            // --- golems : HP% calc1 exacts + vitesse Golem Mastery dm34
            MinionKind::ClayGolem | MinionKind::BloodGolem | MinionKind::FireGolem => {
                let base_hp = s.map(|x| x.hp_max).unwrap_or(100);
                let pct = match def.kind {
                    MinionKind::ClayGolem => clay_hp_pct(lvl, gm),
                    // calc1 = skill('Golem Mastery'.ln12) — synergie Blood
                    // `par8` absente = 0 (FireGolem calc1 identique)
                    _ => golem_mastery_ln12(gm),
                };
                let mut m = base;
                m.max_hp = ((base_hp as i32 * (100 + pct)) / 100).max(1) as u32;
                m.hp = m.max_hp;
                let sp = golem_mastery_dm34(gm).max(0) as u32;
                m.speed = m.speed * (100 + sp) as f32 / 100.0;
                m
            }
            // --- revive : le cadavre REDEVIENT une invocation boostée
            MinionKind::Revive => {
                let mut m = base;
                if let Some(c) = corpse {
                    m.max_hp = (c.max_hp * (200 + mastery * 5)) / 100; // calc1
                    m.hp = m.max_hp;
                    let pct = mastery * 10; // aurastatcalc1 = mastery×SM.par4
                    m.dmg_min = (c.dmg_min * (100 + pct)) / 100;
                    m.dmg_max = (c.dmg_max * (100 + pct)) / 100;
                    m.ac = c.ac;
                    m.th = c.th;
                }
                m.speed = m.speed * 1.5; // passivestat1 velocitypercent par5=50
                m.ttl_ms = Some(REVIVE_FRAMES * combat_mods::FRAME_MS_D2);
                m
            }
            // --- loups / grizzly / raven : HP rand + formules de table
            MinionKind::SpiritWolf | MinionKind::DireWolf | MinionKind::Grizzly
            | MinionKind::Raven => {
                let mut m = base;
                if let Some(st) = s {
                    m.hp = self.rng.range_u32(st.hp_min, st.hp_max);
                    m.max_hp = m.hp;
                }
                match def.kind {
                    // dmg = skill 2-6 ×(100+Fenris ln12)/100 ; AC ×(100+7lvl+50)/100
                    MinionKind::SpiritWolf => {
                        let fen = if fenris == 0 { 0 } else { (50 + (fenris - 1) * 25) as i32 };
                        m.dmg_min = ((2 * (100 + fen)) / 100) as u32;
                        m.dmg_max = ((6 * (100 + fen)) / 100) as u32;
                        let armor = (7 * lvl + 50) as i32; // (par4×lvl)+ln56
                        m.ac = ((m.ac as i32 * (100 + armor)) / 100).max(1) as u32;
                    }
                    // dmg = skill 7-12 ×(100+ln12(own))/100 ; AC ×(100+50)/100
                    MinionKind::DireWolf => {
                        let own =
                            if lvl == 0 { 0 } else { (50 + (lvl.saturating_sub(1)) * 25) as i32 };
                        m.dmg_min = ((7 * (100 + own)) / 100) as u32;
                        m.dmg_max = ((12 * (100 + own)) / 100) as u32;
                        m.ac = (m.ac * 150) / 100; // par6(0)×lvl + SW.ln56(50)
                    }
                    // dmg = skill 30-60 ×(100+ln12(own)+Fenris ln12)/100
                    MinionKind::Grizzly => {
                        let own =
                            if lvl == 0 { 0 } else { (25 + (lvl.saturating_sub(1)) * 10) as i32 };
                        let fen = if fenris == 0 { 0 } else { (50 + (fenris - 1) * 25) as i32 };
                        m.dmg_min = ((30 * (100 + own + fen)) / 100) as u32;
                        m.dmg_max = ((60 * (100 + own + fen)) / 100) as u32;
                    }
                    // dmg = skill Raven MinDam/MaxDam = 2-4
                    MinionKind::Raven => {
                        m.dmg_min = 2;
                        m.dmg_max = 4;
                    }
                    _ => unreachable!(),
                }
                m
            }
            // --- vigne : poison calc1 = par3×(lvl−1) (0 au niveau 1)
            MinionKind::Vine => {
                let mut m = base;
                if let Some(st) = s {
                    m.hp = self.rng.range_u32(st.hp_min, st.hp_max);
                    m.max_hp = m.hp;
                }
                let par3 = match def.code {
                    "Vines" => 20, // Param3 (skill 241)
                    _ => 25,       // Plague Poppy / Cycle of Life Param3
                };
                m.poison_dmg = par3 * lvl.saturating_sub(1);
                m
            }
            // --- totems : immobiles, porteurs d'aura
            MinionKind::OakSage | MinionKind::Wolverine | MinionKind::Barbs => {
                let mut m = base;
                if let Some(st) = s {
                    m.hp = self.rng.range_u32(st.hp_min, st.hp_max);
                    m.max_hp = m.hp;
                }
                m.aura = totem_aura(def.kind, lvl);
                m
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — tables golden, chaînes par invocation, limites, IA, déterminisme
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::HeroSkill;

    /// Manifest réel du projet (compile-time, comme tests.rs).
    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Monde vidé de monstres, budget d'incantation large (tests).
    fn world_of(hero: &str, seed: u64) -> World {
        let mut w = World::with_seed_and_hero(
            open_grid(56, 40),
            serde_json::from_str(MANIFEST_JSON).unwrap(),
            seed,
            hero,
        );
        w.clear_monsters();
        w.hero.stats.mana = 1000;
        w
    }

    fn world_necro(seed: u64) -> World {
        world_of("NE", seed) // connaît Raise Skeleton (charstats StartSkill)
    }

    fn world_druid(seed: u64) -> World {
        world_of("DZ", seed)
    }

    fn grant(w: &mut World, code: &str, level: u32) {
        grant_hero_skill(w, code, level);
    }

    /// Tue proprement un monstre (branche DD) et laisse l'anim finir :
    /// le retrait enregistre le cadavre invocable. Le héros est posé à côté
    /// (à portée de cast — l'auto-spawn le met au centre de la carte).
    fn make_corpse(w: &mut World, x: i32, y: i32) {
        assert!(w.spawn_monster("ZM", x, y));
        let i = w.monsters().len() - 1;
        w.mon_dat[i].state = MonState::Dying;
        w.mon_dat[i].dying_ms = 0;
        w.mon_dat[i].path.clear();
        teleport_hero(w, x as f32 + 1.5, y as f32 + 0.5);
        let mut left = DYING_MS + 100;
        while left > 0 {
            let dt = left.min(100);
            w.tick(dt, None);
            left -= dt;
        }
    }

    fn run(w: &mut World, ms: u32) -> Vec<GameEvent> {
        let mut ev = Vec::new();
        let mut left = ms;
        while left > 0 {
            let dt = left.min(100);
            ev.extend(w.tick(dt, None));
            left -= dt;
        }
        ev
    }

    fn teleport_hero(w: &mut World, x: f32, y: f32) {
        w.hero_ent.x = x;
        w.hero_ent.y = y;
        w.hero.path.clear();
    }

    // -- golden : tables patch_skills.txt / patch_monstats.txt ---------------

    #[test]
    fn golden_summon_skills_table() {
        assert_eq!(SUMMON_SKILLS.len(), 16);
        let rs = summon_skill("Raise Skeleton").unwrap();
        assert_eq!((rs.id, rs.mana, rs.lvlmana, rs.summon), (70, 6, 1, "necroskeleton"));
        let rm = summon_skill("raise skeletal mage").unwrap(); // insensible casse
        assert_eq!((rm.id, rm.pettype), (80, PetType::SkeletonMage));
        assert_eq!(summon_skill("Clay Golem").unwrap().summon, "claygolem");
        assert_eq!(summon_skill("BloodGolem").unwrap().id, 85);
        assert_eq!(summon_skill("FireGolem").unwrap().mana, 50);
        assert_eq!(summon_skill("Revive").unwrap().summon, "");
        assert_eq!(summon_skill("Summon Fenris").unwrap().kind, MinionKind::DireWolf);
        assert_eq!(summon_skill("Summon Grizzly").unwrap().summon, "druidbear");
        assert_eq!(summon_skill("Plague Poppy").unwrap().pettype, PetType::Vine);
        assert_eq!(summon_skill("Cycle of Life").unwrap().pettype, PetType::Vine);
        assert_eq!(summon_skill("Spirit of Barbs").unwrap().pettype, PetType::Totem);
        assert!(summon_skill("IronGolem").is_none(), "restant (consume une arme)");
        assert!(summon_skill("Fire Bolt").is_none(), "pas une invocation");
        // coût = mana + lvlmana×(lvl−1)
        let fg = summon_skill("FireGolem").unwrap();
        assert_eq!(fg.mana + fg.lvlmana * 1, 60, "FireGolem lvl 2 = 50+10");
    }

    #[test]
    fn golden_petmax_formulas() {
        let sk = summon_skill("Raise Skeleton").unwrap();
        // `(lvl<4)?lvl:(2+lvl/3)`
        assert_eq!(petmax(sk, 1), 1);
        assert_eq!(petmax(sk, 3), 3);
        assert_eq!(petmax(sk, 4), 3);
        assert_eq!(petmax(sk, 7), 4);
        assert_eq!(petmax(sk, 20), 8);
        let rav = summon_skill("Raven").unwrap();
        assert_eq!((petmax(rav, 3), petmax(rav, 7)), (3, 5)); // min(lvl,5)
        let sw = summon_skill("Summon Spirit Wolf").unwrap();
        assert_eq!((petmax(sw, 5), petmax(sw, 9)), (5, 5)); // min(lvl,5)
        let fen = summon_skill("Summon Fenris").unwrap();
        assert_eq!((petmax(fen, 2), petmax(fen, 5)), (2, 3)); // min(lvl,3)
        let rev = summon_skill("Revive").unwrap();
        assert_eq!(petmax(rev, 9), 9); // lvl
        for code in ["Clay Golem", "Oak Sage", "Plague Poppy", "Summon Grizzly"] {
            assert_eq!(petmax(summon_skill(code).unwrap(), 20), 1, "{code} = 1");
        }
    }

    #[test]
    fn golden_monstats_rows() {
        let sk = summon_stats("necroskeleton").unwrap();
        assert_eq!(
            (sk.hp_min, sk.hp_max, sk.ac, sk.dmg_min, sk.dmg_max, sk.th),
            (21, 21, 5, 1, 2, 5)
        );
        assert_eq!(sk.velocity, 13.0);
        let mg = summon_stats("necromage").unwrap();
        assert_eq!((mg.hp_max, mg.ac, mg.velocity), (61, 24, 12.0));
        let clay = summon_stats("claygolem").unwrap();
        assert_eq!((clay.hp_max, clay.ac, clay.dmg_min, clay.dmg_max, clay.th), (100, 100, 2, 5, 40));
        assert_eq!(clay.res, [25, 0, 0, 20, 50, 0]);
        let fire = summon_stats("firegolem").unwrap();
        assert_eq!((fire.hp_max, fire.ac, fire.th), (313, 200, 120));
        assert_eq!(fire.res[2], 100, "FireGolem : immunité feu (ResFi=100)");
        let sw = summon_stats("spiritwolf").unwrap();
        assert_eq!((sw.hp_min, sw.hp_max, sw.ac, sw.th), (60, 82, 67, 50));
        let bear = summon_stats("druidbear").unwrap();
        assert_eq!((bear.hp_min, bear.hp_max, bear.ac, bear.th), (550, 750, 245, 300));
        let oak = summon_stats("oaksage").unwrap();
        assert_eq!(oak.res, [0, 25, 25, 25, 25, 70]);
        assert!(summon_stats("zombie1").is_none(), "seuls les summons sont portés");
    }

    #[test]
    fn golden_scaling_formulas() {
        assert_eq!(mastery_hp_mult(0), 256);
        assert_eq!(mastery_hp_mult(3), 280);
        assert_eq!((raise_dmg_pct(3), raise_dmg_pct(4), raise_dmg_pct(10)), (0, 7, 49));
        assert_eq!(raise_th(2, 3), 75);
        assert_eq!(mage_ac(24, 1, 2), 54);
        assert_eq!(
            (golem_mastery_ln12(0), golem_mastery_ln12(1), golem_mastery_ln12(3)),
            (0, 20, 60)
        );
        assert_eq!((golem_mastery_dm34(0), golem_mastery_dm34(3)), (0, 80));
        assert_eq!((clay_hp_pct(1, 0), clay_hp_pct(4, 0), clay_hp_pct(1, 2)), (0, 105, 40));
        assert_eq!(aura_range_tiles(1), 6.0);
        assert_eq!(aura_range_tiles(6), 8.0);
        let oak = totem_aura(MinionKind::OakSage, 1).unwrap();
        assert_eq!((oak.maxhp_pct, oak.range_tiles), (30, 6.0));
        let wol = totem_aura(MinionKind::Wolverine, 5).unwrap();
        assert_eq!((wol.dmg_pct, wol.th_pct), (20, 53));
        let barbs = totem_aura(MinionKind::Barbs, 1).unwrap();
        assert_eq!((barbs.thorns_pct, barbs.dmg_pct), (50, 0));
        assert!(totem_aura(MinionKind::Skeleton, 1).is_none());
    }

    // -- cast : validation, mana, cadavre ------------------------------------

    #[test]
    fn cast_requires_known_skill() {
        let mut ba = world_of("BA", 0xD2B4_0001); // le barbare ne connaît rien
        assert_eq!(ba.cast_summon("Raise Skeleton"), Err(SummonError::SkillUnknown));
        assert_eq!(ba.cast_summon("Summon Spirit Wolf"), Err(SummonError::SkillUnknown));
        assert_eq!(ba.cast_summon("Sort Inconnu"), Err(SummonError::SkillUnknown));
        assert!(ba.minions().is_empty(), "état inchangé sur refus");
    }

    #[test]
    fn raise_skeleton_from_corpse_chain() {
        let mut w = world_necro(0xD2A1_1001);
        make_corpse(&mut w, 6, 5);
        assert_eq!(w.raisable_corpses().len(), 1, "cadavre enregistré à la fin du DD");
        assert_eq!(w.raisable_corpses()[0].x, 6.5, "cadavre à la position de mort");
        // NE démarre AVEC Raise Skeleton (charstats StartSkill)
        assert_eq!(w.cast_summon("Raise Skeleton"), Ok(1));
        assert_eq!(w.minions().len(), 1);
        let m = &w.minions()[0];
        assert_eq!(m.kind, MinionKind::Skeleton);
        assert_eq!((m.hp, m.max_hp), (21, 21), "necroskeleton 21 HP (mastery 0)");
        assert_eq!((m.dmg_min, m.dmg_max), (1, 2), "A1MinD/MaxD ×(100+0)/100");
        assert_eq!(m.th, 15, "(lvl+mastery)×15");
        assert_eq!(m.ac, 5);
        assert_eq!(m.speed, 13.0, "Velocity");
        assert!((m.ent.x - 6.5).abs() < 0.01, "levé sur le cadavre");
        assert_eq!(w.raisable_corpses().len(), 0, "le cadavre est CONSOMMÉ");
        assert_eq!(w.hero.stats.mana, 994, "coût mana 6 débité");
        // plus de cadavre : refus net, état inchangé
        assert_eq!(w.cast_summon("Raise Skeleton"), Err(SummonError::NoCorpse));
        assert_eq!(w.minions().len(), 1);
    }

    #[test]
    fn raise_limit_replaces_oldest() {
        let mut w = world_necro(0xD2A1_1002);
        grant(&mut w, "Raise Skeleton", 8); // petmax = 2+8/3 = 4
        for x in [6, 7, 8, 9, 10] {
            make_corpse(&mut w, x, 5);
        }
        for k in 0..4 {
            assert_eq!(w.cast_summon("Raise Skeleton"), Ok(k + 1));
        }
        assert_eq!(w.minions().len(), 4);
        let oldest = w.minions()[0].ent.x;
        assert_eq!(w.cast_summon("Raise Skeleton"), Ok(4), "5e cast : limite atteinte");
        assert_eq!(w.minions().len(), 4, "le plus ANCIEN est remplacé");
        assert_ne!(w.minions()[0].ent.x, oldest, "le squelette du 1er cadavre est parti");
        assert!((w.minions()[3].ent.x - 6.5).abs() < 0.01, "le nouveau est en fin de liste");
    }

    #[test]
    fn skeletal_mage_fires_elemental_missiles() {
        let mut w = world_necro(0xD2A1_1003);
        grant(&mut w, "Raise Skeletal Mage", 1);
        make_corpse(&mut w, 6, 5);
        assert_eq!(w.cast_summon("Raise Skeletal Mage"), Ok(1));
        let m = &w.minions()[0];
        assert_eq!(m.kind, MinionKind::SkeletalMage);
        assert!(
            matches!(
                m.elem,
                Some(DamageType::Fire) | Some(DamageType::Cold) | Some(DamageType::Lightning)
            ),
            "élément parmi les bolts portés (poison restant)"
        );
        let elem = m.elem.unwrap();
        let (carrier, dmin, dmax) = match elem {
            DamageType::Fire => ("firebolt", 3, 8),
            DamageType::Cold => ("icebolt", 3, 7),
            _ => ("chargedbolt", 2, 6),
        };
        assert_eq!((m.dmg_min, m.dmg_max), (dmin, dmax), "carrier Skill niveau 1");
        assert_eq!(m.ac, 34, "24 + (1+0)×10");
        // cible à 4 tuiles : le mage décoche, le missile vole puis touche
        assert!(w.spawn_monster("ZM", 10, 5));
        let zm = w.monsters().len() - 1;
        w.mon_dat[zm].hp = 100;
        w.merc_target = Some(zm);
        w.minions.list[0].th = 1000; // jet de toucher ~95 % (tests : déterminisme)
        run(&mut w, 200); // premier tir parti
        assert!(
            w.minion_missiles().iter().any(|mm| mm.def_id == carrier)
                || w.mon_dat[zm].hp < 100,
            "missile {} décoché",
            carrier
        );
        run(&mut w, 5000);
        assert!(
            w.mon_dat[zm].hp < 100 || w.mon_dat[zm].state == MonState::Dying,
            "le bolt du mage touche (dégâts {}-{})",
            dmin,
            dmax
        );
    }

    #[test]
    fn golem_chain_limit_and_clay_slow() {
        let mut w = world_necro(0xD2A1_1004);
        grant(&mut w, "Clay Golem", 1);
        grant(&mut w, "Golem Mastery", 2);
        assert_eq!(w.cast_summon("Clay Golem"), Ok(1));
        let g = &w.minions()[0];
        assert_eq!(g.kind, MinionKind::ClayGolem);
        assert_eq!(g.max_hp, 140, "100×(100+clay_hp_pct(1,2)=40)/100");
        assert_eq!(g.speed, 11.2, "8×(100+dm34(2)=40)/100");
        // Clay ralentit sa cible (item_slow → refroidissement moteur)
        assert!(w.spawn_monster("ZM", 8, 5));
        let zm = w.monsters().len() - 1;
        w.mon_dat[zm].hp = 100_000;
        w.merc_target = Some(zm);
        w.minions.list[0].th = 1000;
        w.minions.list[0].ent.x = 8.5;
        w.minions.list[0].ent.y = 5.5;
        let mut seen_slow = false;
        for _ in 0..60 {
            w.tick(100, None);
            if w.mon_dat[zm].chill_ms > 0 {
                seen_slow = true;
                break;
            }
        }
        assert!(seen_slow, "le coup du Clay Golem ralentit (item_slow)");
        // un seul golem : BloodGolem REMPLACE le Clay
        grant(&mut w, "BloodGolem", 1);
        assert_eq!(w.cast_summon("BloodGolem"), Ok(1));
        assert_eq!(w.minions().len(), 1);
        assert_eq!(w.minions()[0].kind, MinionKind::BloodGolem);
        assert_eq!(w.minions()[0].max_hp, 281, "201×(100+ln12(2)=40)/100");
        // FireGolem : mana insuffisant (50 requis)
        grant(&mut w, "FireGolem", 1);
        w.hero.stats.mana = 10;
        assert_eq!(
            w.cast_summon("FireGolem"),
            Err(SummonError::NotEnoughMana { cost: 50, have: 10 })
        );
        assert_eq!(w.minions()[0].kind, MinionKind::BloodGolem, "état inchangé");
    }

    #[test]
    fn revive_chain_boost_and_ttl() {
        let mut w = world_necro(0xD2A1_1005);
        grant(&mut w, "Revive", 1);
        grant(&mut w, "Skeleton Mastery", 5);
        make_corpse(&mut w, 6, 5);
        let corpse_hp = w.raisable_corpses()[0].max_hp;
        let corpse_dmg = (w.raisable_corpses()[0].dmg_min, w.raisable_corpses()[0].dmg_max);
        assert_eq!(w.cast_summon("Revive"), Ok(1));
        let m = &w.minions()[0];
        assert_eq!(m.kind, MinionKind::Revive);
        assert_eq!(m.max_hp, corpse_hp * 225 / 100, "HP ×(200+mastery×5)/100");
        assert_eq!(
            (m.dmg_min, m.dmg_max),
            (corpse_dmg.0 * 150 / 100, corpse_dmg.1 * 150 / 100),
            "dmg ×(100+mastery×10)/100"
        );
        assert_eq!(m.ttl_ms, Some(4500 * 40), "4500 frames × 40 ms = 180 s retail");
        // le revenant combat puis EXPIRE (chaîne mort → combat → disparition)
        assert!(w.spawn_monster("ZM", 8, 5));
        let zm = w.monsters().len() - 1;
        w.mon_dat[zm].hp = 3;
        w.merc_target = Some(zm);
        w.minions.list[0].th = 1000;
        run(&mut w, 4000);
        assert!(
            w.monsters().is_empty() || w.mon_dat[zm].state == MonState::Dying,
            "le revenant a tué sa cible (DD puis retrait)"
        );
        run(&mut w, 181_000 - 4000);
        assert!(w.minions().is_empty(), "le Revive expire après 180 s");
    }

    // -- druide : loups, esprits, vignes -------------------------------------

    #[test]
    fn druid_wolves_limit_and_stats() {
        let mut w = world_druid(0xD2A1_1006);
        grant(&mut w, "Summon Spirit Wolf", 7); // petmax min(7,5)=5
        for k in 0..6 {
            assert_eq!(w.cast_summon("Summon Spirit Wolf"), Ok((k + 1).min(5)));
        }
        assert_eq!(w.minions().len(), 5, "min(lvl,5) : le 6e remplace le 1er");
        let wolf = &w.minions()[0];
        assert_eq!(wolf.kind, MinionKind::SpiritWolf);
        assert!((60..=82).contains(&wolf.max_hp), "HP rand monstats 60-82");
        assert_eq!((wolf.dmg_min, wolf.dmg_max), (2, 6), "skill MinDam/MaxDam (Fenris 0)");
        assert_eq!(wolf.ac, 133, "67×(100+7×7+50=199)/100 = 133,36 → 133");
        assert!((60..=82).contains(&w.minions()[4].max_hp), "le remplaçant a ses HP propres");
        // Fenris (dire wolf) : max 3
        grant(&mut w, "Summon Fenris", 5);
        for _ in 0..4 {
            w.cast_summon("Summon Fenris").unwrap();
        }
        let fens = w.minions().iter().filter(|m| m.kind == MinionKind::DireWolf).count();
        assert_eq!(fens, 3, "min(lvl,3)");
        let fen = w.minions().iter().find(|m| m.kind == MinionKind::DireWolf).unwrap();
        assert_eq!((fen.dmg_min, fen.dmg_max), (17, 30), "7-12 ×(100+ln12(5)=150)/100 = 17,5/30 → 17/30");
        assert_eq!(fen.ac, 174, "116×150/100");
    }

    #[test]
    fn totem_aura_immobile_and_effective() {
        let mut w = world_druid(0xD2A1_1007);
        grant(&mut w, "Summon Spirit Wolf", 1);
        assert_eq!(w.cast_summon("Summon Spirit Wolf"), Ok(1));
        grant(&mut w, "Heart of Wolverine", 5);
        assert_eq!(w.cast_summon("Heart of Wolverine"), Ok(1));
        // l'aura couvre le loup (spawns ≤ 2 tuiles du héros < 7,6 de rayon)
        let auras = w.active_aura_effects();
        assert_eq!(auras.len(), 1);
        assert_eq!((auras[0].dmg_pct, auras[0].th_pct), (20, 53));
        assert_eq!(auras[0].range_tiles, 7.6);
        assert_eq!(
            w.minion_effective_stats(0),
            Some((2, 7, 76)),
            "dmg 2-6 ×1,2 ; th 50×(100+53)/100"
        );
        // hors du rayon : stats de base intactes (coin opposé au spawn)
        w.minions.list[0].ent.x = 5.0;
        w.minions.list[0].ent.y = 5.0;
        assert_eq!(w.minion_effective_stats(0), Some((2, 6, 50)), "hors aura : base");
        // l'esprit est IMMOBILE : le héros s'éloigne, lui ne bouge pas
        let totem_pos = (w.minions()[1].ent.x, w.minions()[1].ent.y);
        teleport_hero(&mut w, 28.0, 20.0);
        run(&mut w, 3000);
        assert_eq!((w.minions()[1].ent.x, w.minions()[1].ent.y), totem_pos, "esprit immobile");
    }

    #[test]
    fn vine_applies_poison_dot() {
        let mut w = world_druid(0xD2A1_1008);
        grant(&mut w, "Vines", 3); // poison calc1 = par3×(lvl−1) = 20×2 = 40
        assert_eq!(w.cast_summon("Vines"), Ok(1));
        let vine = &w.minions()[0];
        assert_eq!(vine.kind, MinionKind::Vine);
        assert_eq!(vine.poison_dmg, 40);
        assert!(w.spawn_monster("ZM", 8, 5));
        let zm = w.monsters().len() - 1;
        w.mon_dat[zm].hp = 100_000;
        w.merc_target = Some(zm);
        w.minions.list[0].th = 1000;
        w.minions.list[0].ent.x = 8.5;
        w.minions.list[0].ent.y = 5.5;
        let mut poisoned = false;
        for _ in 0..60 {
            w.tick(100, None);
            if let Some(p) = w.mon_dat[zm].poison.as_ref() {
                assert_eq!(p.dmg_per_tick, 40, "calc1=40 ; ZM spawné res nuls (pas de ResPo=50 sans plan d2game)");
                poisoned = true;
                break;
            }
        }
        assert!(poisoned, "la vigne empoisonne sa cible (EType=pois)");
        // niveau 1 : calc1 = 0 — aucune dose de poison (table fait foi)
        let mut w2 = world_druid(0xD2A1_1009);
        grant(&mut w2, "Vines", 1);
        w2.cast_summon("Vines").unwrap();
        assert_eq!(w2.minions()[0].poison_dmg, 0);
    }

    // -- IA compagnon : suivi, cible du héros, anti-blocage -------------------

    #[test]
    fn companion_follows_hero_and_fights_his_target() {
        let mut w = world_druid(0xD2A1_100A);
        grant(&mut w, "Summon Spirit Wolf", 1);
        w.cast_summon("Summon Spirit Wolf").unwrap();
        // suivi : le héros part, le loup le rattrape
        teleport_hero(&mut w, 20.0, 20.0);
        let d0 = dist(w.hero_ent.x - w.minions()[0].ent.x, w.hero_ent.y - w.minions()[0].ent.y);
        run(&mut w, 2000);
        let d1 = dist(w.hero_ent.x - w.minions()[0].ent.x, w.hero_ent.y - w.minions()[0].ent.y);
        assert!(d1 < d0, "le compagnon suit le héros ({} → {})", d0, d1);
        // attaque LA CIBLE DU HÉROS (merc_target), pas le plus proche
        assert!(w.spawn_monster("ZM", 10, 20)); // HORS rayon d'aggro du loup (témoin)
        let zm2 = w.monsters().len() - 1;
        assert!(w.spawn_monster("ZM", 27, 21)); // la cible du héros
        let zm1 = w.monsters().len() - 1;
        w.mon_dat[zm1].hp = 3;
        w.mon_dat[zm2].hp = 100_000;
        w.merc_target = Some(zm1);
        w.minions.list[0].th = 1000;
        run(&mut w, 4000);
        assert_eq!(w.monsters().len(), 1, "la cible du héros est morte puis retirée (DD)");
        assert!(w.mon_dat[0].hp == 100_000, "l'autre monstre est intact");
    }

    #[test]
    fn companion_dies_to_monsters_full_chain() {
        let mut w = world_necro(0xD2A1_100B);
        make_corpse(&mut w, 6, 5);
        w.cast_summon("Raise Skeleton").unwrap();
        w.minions.list[0].hp = 2; // abattable en un coup (tests)
        // un zombie adjacenct, héros loin : le monstre cible le squelette
        assert!(w.spawn_monster("ZM", 7, 5));
        teleport_hero(&mut w, 28.0, 20.0);
        // chaîne : le zombie engage le squelette → le squelette meurt (DD) →
        // retrait ; et le squelette a frappé sa cible au passage.
        let ev = run(&mut w, 30_000);
        assert!(
            w.minions().is_empty(),
            "le squelette abattu disparaît après son anim DD"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::HeroHit { .. })) == false,
            "le héros loin n'encaisse rien (le monstre visait le minion)"
        );
    }

    #[test]
    fn minion_kill_gives_hero_xp_via_dd_flow() {
        let mut w = world_necro(0xD2A1_100C);
        make_corpse(&mut w, 6, 5);
        w.cast_summon("Raise Skeleton").unwrap();
        assert!(w.spawn_monster("ZM", 8, 5));
        let zm = w.monsters().len() - 1;
        w.mon_dat[zm].hp = 2;
        w.minions.list[0].th = 1000;
        teleport_hero(&mut w, 28.0, 20.0);
        let before = w.hero.stats.xp;
        run(&mut w, 8000);
        assert!(w.hero.stats.xp > before, "XP du kill par le minion (flux DD inchangé)");
    }

    #[test]
    fn hero_death_and_level_change_despawn_minions() {
        let mut w = world_druid(0xD2A1_100D);
        grant(&mut w, "Summon Spirit Wolf", 1);
        w.cast_summon("Summon Spirit Wolf").unwrap();
        assert_eq!(w.minions().len(), 1);
        // mort du héros : le compagnon disparaît
        w.hero.dead_ms = 1;
        w.tick(100, None);
        assert!(w.minions().is_empty(), "les invocations meurent avec le héros");
        // changement de niveau : despawn (les pets ne traversent pas les warps)
        w.hero.dead_ms = 0;
        w.hero.stats.mana = 1000;
        w.cast_summon("Summon Spirit Wolf").unwrap();
        assert_eq!(w.minions().len(), 1);
        w.set_level_key("bloodmoor");
        assert!(w.minions().is_empty(), "changement de niveau → despawn (retail)");
        // même clé : aucun despawn supplémentaire
        w.cast_summon("Summon Spirit Wolf").unwrap();
        w.set_level_key("bloodmoor");
        assert_eq!(w.minions().len(), 1, "même clé : les invocations restent");
    }

    #[test]
    fn save_load_drops_minions_retail() {
        let mut w = world_druid(0xD2A1_100E);
        grant(&mut w, "Summon Spirit Wolf", 1);
        w.cast_summon("Summon Spirit Wolf").unwrap();
        assert_eq!(w.minions().len(), 1);
        let json = w.save();
        let w2 = World::load(&json).expect("save valide");
        // retail v1.12 : les invocations ne survivent PAS au save/load
        assert!(w2.minions().is_empty(), "invocations absentes de la save (retail)");
        assert!(w2.raisable_corpses().is_empty());
    }

    #[test]
    fn determinism_same_seed_same_actions() {
        let script = |w: &mut World| {
            make_corpse(w, 6, 5);
            w.cast_summon("Raise Skeleton").unwrap();
            grant(w, "Raise Skeletal Mage", 1);
            make_corpse(w, 8, 7);
            w.cast_summon("Raise Skeletal Mage").unwrap();
            assert!(w.spawn_monster("ZM", 12, 6));
            w.merc_target = Some(w.monsters().len() - 1);
            run(w, 6000);
        };
        let mut a = world_necro(0x5EED_160B);
        script(&mut a);
        let mut b = world_necro(0x5EED_160B);
        script(&mut b);
        assert_eq!(a.minions().len(), b.minions().len());
        for (ma, mb) in a.minions().iter().zip(b.minions().iter()) {
            assert_eq!(ma.kind, mb.kind);
            assert_eq!(ma.elem, mb.elem, "élément du mage déterministe");
            assert_eq!((ma.ent.x, ma.ent.y), (mb.ent.x, mb.ent.y), "positions identiques");
            assert_eq!((ma.hp, ma.max_hp), (mb.hp, mb.max_hp));
        }
        let hpa: Vec<u32> = a.mon_dat.iter().map(|d| d.hp).collect();
        let hpb: Vec<u32> = b.mon_dat.iter().map(|d| d.hp).collect();
        assert_eq!(hpa, hpb, "séquences de combat identiques");
    }

    #[test]
    fn minion_struct_fields_visible_for_hud() {
        // les accès rendu/HUD existent (kind, entité, péttype, TTL)
        let mut w = world_necro(0xD2A1_100F);
        make_corpse(&mut w, 6, 5);
        w.cast_summon("Raise Skeleton").unwrap();
        let m = &w.minions()[0];
        assert_eq!(m.kind.as_str(), "skeleton");
        assert_eq!(m.pettype, PetType::Skeleton);
        assert_eq!(m.ent.anim, AnimState::NU);
        assert!(m.ttl_ms.is_none(), "seul Revive a une durée");
    }

    #[test]
    fn unused_import_guard() {
        // HeroSkill importé pour grant_hero_skill — garde de compilation
        let _ = |s: HeroSkill| s.code;
    }
}
