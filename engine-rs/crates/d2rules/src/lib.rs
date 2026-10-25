//! `d2rules` — logique de jeu PURE du port Diablo 2 v1.12 (MVP Phase 2).
//!
//! * Pas de rendu, pas de wasm, pas d'IO fichier : `World` consomme une grille de
//!   collision (produite par d2game) + le `assets/manifest.json`, et rend des
//!   événements. Testable en `cargo test`, déterministe à graine fixe.
//!
//! # Contrat avec d2game
//! * Grille : `collision[y][x] == true` ⇒ tuile BLOQUÉE ; `false` ⇒ marchable.
//!   Hors-bornes = bloqué. Origine (0,0) en haut-gauche, y vers le bas.
//! * Coordonnées entités : espace tuile continu, centre de la tuile `(tx,ty)` =
//!   `(tx + 0.5, ty + 0.5)` ; `Entity.x/y` sont ces f32.
//! * `facing` : 0..=7, sens horaire depuis le Sud : 0=S, 1=SW, 2=W, 3=NW, 4=N,
//!   5=NE, 6=E, 7=SE (sous-ensemble 8 directions du standard D2).
//! * `anim_frame` : 16 frames par état, `FRAME_MS = 100 ms/frame` ; états
//!   bouclés (NU/WL/RN) modulo 16, A1/DD joués une fois (clamp frame 15).
//! * Vitesse : 1 tuile = 1 yard ⇒ héros marche 6 tuiles/s (`walkSpeed` manifest).
//! * `GameEvent::MonsterDied{index}` : `index` = position dans `monsters()` AU
//!   MOMENT de l'événement ; un monstre retiré plus tôt DANS LE MÊME tick décale
//!   les indices suivants (caveat documenté REPORT.md).
//! * `ItemDef` porte `amount` (or empilé ; 0 sinon) — construire un littéral avec
//!   `..Default::default()` ou les constructeurs `ItemDef::gold`/`ItemDef::potion`.
//!
//! # Données réelles (manifest + GAME-DATA.md)
//! * Héros barbare niv 1 : 55 vie, 15 mana, force 30, dex 20 (`patch_charstats`).
//! * Monstres : valeurs brutes manifest (`patch_monstats`) scalées par la ligne
//!   niveau 1 Normal de `assets/MonLvl.txt` : HP ×7/100, dégâts ×2/100,
//!   XP ×30/100, AC effectif 6, TH effectif 8 (§2.1 de GAME-DATA.md).
//! * XP : seuils cumulés niv 2..10, colonne Barbarian de `patch_experience.txt`.
//! * Drops : TreasureClassEx réel v1.12 — TC du monstre (manifest
//!   `stats.treasureClass`, ex. "Act 1 H2H A") → picks/NoDrop → sous-TC →
//!   item final (`loot.rs`, données embarquées) ; repli 60 % rien / 25 % or /
//!   15 % potion si la table est absente ou la TC inconnue.

#![forbid(unsafe_code)]

mod data;
mod loot;
/// Pont `.d2s` ⇄ save v3 (étape 6b) — format d'échange OPTIONNEL par bouton
/// (export monde → fichier v1.12 valide, import fichier → héros) ; le save
/// JSON v3 reste le défaut. Pur : octets seuls, zéro IO.
pub mod save_d2s;

/// Étape 7a « loot D2 réel » : contrat `RolledItem` (Affixes/Équipement) +
/// roll de qualité ItemRatio v1.12 + branchement sur la chaîne `loot`.
mod item_roll;
mod path;
mod rng;
mod save;
/// Tables de chaînes TBL v1.12 (W18, port `tbl_decode.py`) — texte fr-FR
/// in-engine. Pur ; les 3 tables fr réelles sont embarquées (`bundled_fra`).
pub mod tbl;
mod skills;
mod stamina;
mod world;
/// Waypoints W05/W19 (table réelle v1.12, activation au contact, voyage,
/// save additive) — état dans `World`, blocs `impl World` additifs.
pub mod waypoints;

// --- Difficultés Normal/Nightmare/Hell (W08 + K, impl-difficultes, 2026-10-23) ---
/// Enum difficulté + pénalités `patch_difficultylevels.txt` + résists v1.12.
pub mod difficulty;

// --- Formules D2 : mods de coup + éléments (étape 12c, 2026-10-24) ---
/// Deadly strike / crushing blow / open wounds / leech / résists+immunités /
/// poison DoT / froid — pur, déterministe, branché additif dans `world.rs`.
pub mod combat_mods;

// --- Quêtes FINALES A4-A5 spec (quests-actes.md §5/§6, étape 10b, 2026-10-24) ---
/// Machines a4q1→a4q3 / a5q5-a5q6 (slots de sauvegarde .d2s 395-399 / 423-425,
/// events [`ActEvent`] partagés, gating acte V + porte Worldstone, boss
/// finaux Diablo/Anciens/vagues du trône/Baal stats réelles, vagues
/// séquentielles) + branchement `World`/save additif (`quest_finale`).
/// La mort de Baal émet [`GameEvent::GameWon`] (fin du jeu).
pub mod quests_finale;

// --- Quêtes Actes IV-V + fin de jeu (quests-actes.md §5/§6, étape 10c, 2026-10-24) ---
/// Machines a4q1→a4q3 / a5q1→a5q6 (slots de sauvegarde, events [`ActEvent`]
/// partagés, gating acte V, boss de quête Izual/Hephasto/3 Anciens stats
/// réelles, fin de jeu `GameEvent::GameWon`) + branchement `World`/save
/// additif (`quest_actes45`). Coexiste avec les journaux W13/A1/A2-A3.
pub mod quests_actes45;

// --- Socles règles data-driven v1.12 (impl-regles-skills, 2026-10-22) ---
/// Parseur/évaluateur CalcString (socle A2 — synergies, ToHitCalc, passives).
pub mod calc;
/// Arbre de compétences 210 skills (C1-C4 : prereqs, synergies, missiles, dégâts).
pub mod skills_tree;
/// Glue héros ⇄ arbre (C2/C3/C4) : apprentissage dans `Vec<HeroSkill>`,
/// niveaux vus par les formules, dégâts × synergie — pur, testable.
pub mod skills_hero;
/// Stat/StatList génériques + dictionnaire ItemStatCost (socle A1).
pub mod stats;
/// Table XP 1-99 × 7 classes (D5).
pub mod xp;

// --- Items & loot v1.12 (impl-items-loot, F1-F7 / F2-F5, 2026-10-22) ---
/// Affixes magiques MagicPrefix/Suffix (+AutoMagic parsé) — filtres, tirage pondéré.
pub mod affixes;
/// Item générique X×Y, grille 10×4 canFit, 8 slots + belt 4×4, stats dérivées.
pub mod inventory;
/// Descripteurs de propriétés (prop/param/min/max) + liste typée (socle Stat).
pub mod item_props;
/// Qualités de drop via ItemRatio.txt (unique/set/rare/magic/supérieur/normal/low).
pub mod item_quality;
/// Uniques + sets identifiés (uniqueitems/setitems/sets v1.12).
pub mod uniques_sets;
/// Marchands & gamble v1.12 (domaine G — stock/prix/réserve, impl-vendors).
pub mod vendor;

// --- Quêtes W13 (moteur d'états + authoring Acte I, 2026-10-23) ---
/// Moteur de quêtes : QuestEvent typé → états → QuestReward ; authoring A1.
/// Pur (aucune dépendance World) ; câblage world/game + save EN INTÉGRATION.
pub mod quests;

// --- NPC de ville W16 (villes vivantes, 2026-10-23) ---
/// Roster réel Acte I (monstats + PATH TownN1.ds1), spawn, patrouilles,
/// rôles (soin/forge/mercenaires/gamble/caravane/érudit), clic → NpcTalked.
pub mod npc;
// --- Mercenaires H (hireling.txt v1.12, 2026-10-23) ---
/// Mercenaires : embauche/combat/KO/résurrection data-driven (tables réelles
/// `assets/patch_hireling.txt`), état porté par `World`, save additive.
pub mod mercenary;
// --- Corps au sol J1 (mort : corps + équipement, or, récup, 2026-10-23) ---
/// État `Corpse` + règles officielles (péage d'or, respawn nu, un seul
/// corps actif) — sources citées dans l'en-tête du fichier.
pub mod corpse;

// --- Socle data-driven W01/A3 (tables réelles, 2026-10-24) ---
/// Parseur TSV générique runtime (CRLF, quotes Excel, tabs, en-têtes
/// nommés) — consommé par les records typés et par d2game (`spawn.rs`).
pub mod tsv;
/// Records typés des tables réelles v1.12 (monstres/monde/items-ui/assets)
/// + golden tests record-à-record contre les fichiers réels.
pub mod tables;

// --- IA P1 monstres (monai-behaviors.md, étape 4 RE d2w-re, 2026-10-24) ---
/// FallenShaman résurrection + Fallen fuite de panique — overlay ADDITIF sur
/// la FSM `MonState` (world.rs), registre de cadavres persistant, valeurs
/// réelles `patch_monstats.txt` (aip1/aip4/aidel). État non persisté.
pub mod ai_behaviors;

// --- Sons monstres data-driven (monsounds.txt v1.12, étape 14a, 2026-10-24) ---
/// Table officielle monstre → événements sonores (attaque/touché/mort/annonce),
/// résolution jeton→famille + alias, fichiers locaux extraits (index.json) et
/// parité avec le lecteur `demo/mon-sfx.js`.
pub mod mon_audio;

pub mod statlist; // 7c A1 : StatList par ids ItemStatCost (socle loot D2 réel)
pub mod equipment; // 7c F6/F7 : 10 slots bodyloc d2s + canFit 10×4 + belts slots
pub mod economy; // 11c économie & inventaire : or (cap 10k×niveau, stash), durabilité, vente, auto-pickup

// --- Quêtes Acte I spec (quests-actes.md, étape 4 RE d2w-re, 2026-10-24) ---
/// Machine A1Q1→A1Q6 (phases, events typés, flags u16 façon .d2s, gating
/// d'acte II) + branchement `World`/save additif (`quest_act1`). Coexiste
/// avec le journal W13 ([`crate::quests`]) — l'intégrateur branche l'un ou
/// l'autre.
pub mod quests_actes;

// --- Quêtes Actes II-III spec (quests-actes.md §3/§4, étape 8d, 2026-10-24) ---
/// Machines a2q1→a2q6 / a3q1→a3q6 (slots de sauvegarde, events [`ActEvent`]
/// partagés avec l'acte I, gating d'actes III/IV, boss scriptés duriel +
/// mephisto stats réelles) + branchement `World`/save additif
/// (`quest_actes2`). Coexiste avec les journaux W13/A1.
pub mod quests_actes2;

// --- Monde vivant 9c (shrines.txt v1.12 + usage des objets interactifs) ---
/// Sanctuaires data-driven (23 codes `shrines.txt`, tirage pondéré par
/// `rarity`, effets temporisés branchés au socle `statlist`) + règles
/// `World` d'usage : recharge instantanée et butin de conteneurs via
/// `pending_events` (pattern `NpcTalked`). Table embarquée en l'état depuis
/// `d2w-re/tables/monde/shrines.txt`.
pub mod shrines;

// --- Consommables 11b (patch_misc.txt v1.12 : potions/tomes/parchemins) ---
/// Consommables complets v1.12 : effets réels par code (hp1-5/mp1-5/rvs/rvl,
/// buffs antidote/dégel/endurance via le système d'états), porte de ville
/// (tsc/tbk, wire `switch_level` côté d2game), identification (isc/ibk +
/// règles pures, hook inventaire riche documenté), ceinture 4×4 auto-ranger
/// + touches 1-4, noms fr depuis les TBL réelles. Tables parsées EN L'ÉTAT
/// depuis `assets/patch_misc.txt` (zéro copie).
pub mod consumables;

// --- Cube horadrim 11a (cubemain.txt v1.12 + transmutation, 2026-10-24) ---
pub mod cube;

// --- Runewords 17a (PLAN-FIDELITE phase 17, 2026-10-24) ---
/// Runewords v1.12 : `patch_runes.txt` (169 recettes) → résolution socle
/// sertie + runes insérées DANS L'ORDRE (règles retail documentées en tête
/// de module, preuve réelle : Enigma du save Javazon, RWid 59) → nom +
/// mods (ids ItemStatCost via `item_props`) + niveau requis. Pur,
/// déterministe.
pub mod runewords;

/// Items fin de chaîne 17c (PLAN-FIDELITÉ) : crafted (recettes cubemain
/// `usetype,crf` : mods fixes + 1-4 affixes au niveau héros), éthéré (jet
/// 1/20, ÷2 durabilité, +50 % base, non réparable), charges d'usage
/// (hook décrément + recharge vendeur), personnalisé + ear — cf. item_special.rs.
pub mod item_special;

/// Invocations 16b (PLAN-FIDÉLITÉ) : summons necro/druid, IA compagnon, corpse registry — cf. minions.rs.
pub mod minions;

// --- Auras & malédictions 16a (PLAN-FIDELITE phase 16, 2026-10-24) ---
/// Auras de paladin (toggle unique, pulse 1 s, pont statlist `aurastat`) +
/// malédictions de nécromancien (zone, durée `auralen`, effets FSM/combat).
/// Données réelles `patch_skills.txt` (aurastate/aurastat/aurarange/
/// auralen) + états `states.txt` ; état transitoire porté par `World`
/// (`AuraState` + `MonData::curse`, jamais persisté).
pub mod auras;

/// 18c : couche tables/endpoints du cube pour l UI (d2game/src/cube_ui.rs).
pub mod cube_ui;

// --- Comportements Assassine/Druide 16c (PLAN-FIDELITE phase 16, 2026-10-24) ---
pub mod assassin_druid;

// --- Socketing 17b (PLAN-FIDELITE phase 17, 2026-10-24) : gemmes/jewels/runes — cf. socketing.rs
pub mod socketing;

// --- Divers fidélité 21a (PLAN-FIDELITE phase 21, 2026-10-25) : gardes/gaps
//     Fangskin (H) + Andariel + MonLvl 25/26 — cf. fidelity_gaps.rs
pub mod fidelity_gaps;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod parity_tests;

pub use skills::HeroSkill;
/// Erreurs d'apprentissage de l'arbre (prérequis/plafond/points…) —
/// consommées par d2game (learnSkill) et affichables telles quelles en UI.
pub use skills_tree::SkillTreeError;
pub use world::World;
/// Corps au sol (J1) — consommé par d2game (exposition UI) et la save.
pub use corpse::Corpse;
/// Rang de monstre + règle de spawn data-driven (impl-monstres-boss) —
/// consommés par d2game (tables réelles → spawn_monster_rule).
pub use world::{MonKind, SpawnRule};
/// Mercenaires H — types/records/erreurs consommés par d2game (fiche HUD
/// wasm `mercInfo`) et par la save.
pub use mercenary::{
    default_subtype as merc_default_subtype, offer_record as merc_offer_record,
    record_of as merc_record_of, records_for as merc_records_for, HireError, MercAttack, MercData,
    MercKind, MercRec, MercSave, ResurrectError,
};
/// Difficulté (W08/K) — consommée par d2game (wasm `setDifficulty`, plan de
/// spawn) et persistée dans la save.
pub use difficulty::Difficulty;
/// Paliers/densités par difficulté (`patch_difficultylevels.txt`) — consommé
/// par d2game/elite_mods (9c).
pub use difficulty::DifficultyLevels;

/// Waypoints W05 : table réelle v1.12 + état (d2game attache au chargement
/// du niveau et branche le voyage UI).
pub use waypoints::{for_level as waypoint_for_level, by_slot as waypoint_by_slot, WaypointDef, WAYPOINTS};

/// Waypoints des actes 8c : 38 slots retail (colonne `Waypoint`), attach des
/// niveaux réels, état PAR DIFFICULTÉ additif (`waypoints_actes.rs`).
pub mod waypoints_actes;

/// Étape 14c (audio) : résolution pure région → ambiance (SoundEnviron v1.12,
/// golden A1-A5) — contrat data de la couche JS `demo/music.js`.
pub mod audio_env;

pub mod game_audio; // 14b audio : table data-driven GameEvent → WAV réel v1.12 (contrat demo/game-sfx.js)

// -- Types items/loot purs, consommables par d2game (F1-F7) --
pub use affixes::{AffixKind, AffixRec, AffixTables, ItemTypeSet};
pub use inventory::{
    BaseInfo, Belt, DerivedStats, EquipSlot, Equipment, InventoryGrid, Item, GRID_H, GRID_W,
};
pub use item_props::{PropDescriptor, RolledProp, StatProp};
pub use item_quality::{DropRatioInfo, Quality, QualityFactors, QualityTables, RatioRow};
pub use loot::GeneratedItem;
pub use quests::{QuestDef, QuestEvent, QuestLog, QuestProgress, QuestReward, QuestState, QuestTrigger};
pub use uniques_sets::{SetItemRec, SetRec, UniqueRec, UniquesSets};

// -- Consommables 11b — consommés par d2game (wasm useConsumable/useBeltKey) --
pub use consumables::{ConsumError, ConsumState, ConsumUse};

// -- NPC de ville W16 (roster réel, rôles, interaction) — consommés par d2game --
pub use npc::{
    MercenaryResurrect, NpcDef, NpcInteraction, NpcRepair, NpcRole, NpcState, NpcTalkError,
    NpcTravel, NPCS,
};

// -- IA P1 monstres + quêtes Acte I spec (étape 4 RE d2w-re) — consommés par d2game --
pub use ai_behaviors::{AiArchetype, AiState, CorpseRec, MonAiExt};
pub use quests_actes::{ActEvent, ActPhase, ActQuests, ActReward, ActRun};
pub use quests_actes2::{
    boss_stats, Act2Quests, Act2Reward, Act2Run, BossRec, BossStats, BOSSES_ACTS23,
};

/// Projectiles (crate sœur pure) : système embarqué dans le `World`,
/// re-exporté pour le rendu d2game (positions/sprites des missiles en vol).
pub use d2missiles::{DamageType, Missile, MissileDef, MissileSystem};

use serde::{Deserialize, Serialize};

/// Résout le code héros EFFECTIF du manifest : `heroes[CODE]` si l'objet
/// `heroes` existe et contient le code (insensible à la casse), sinon le bloc
/// unique `hero` — sa classe `hero.class`, défaut `"BA"`. Point d'entrée
/// unique partagé avec d2game (sprites + stats du même héros).
pub fn resolve_hero_code(manifest: &serde_json::Value, requested: &str) -> String {
    data::resolve_hero(manifest, requested)
}

/// Intention du joueur (contrat d2game).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerInput {
    /// Clic-déplacement : A* 8 directions vers la tuile visée.
    MoveTo { x: i32, y: i32 },
    /// Clic-attaque : monstre le plus proche du clic (rayon 2 tuiles) ;
    /// frappe si à portée (1,5 tuile), sinon le héros marche vers lui.
    Attack { x: i32, y: i32 },
    /// Attaque à distance (arc/bolt) : tire un missile [`Missile`] vers le
    /// monstre le plus proche du clic (rayon 2 tuiles) ou vers le point du
    /// clic à défaut. Même cooldown de swing que [`PlayerInput::Attack`] ;
    /// le missile vole en ligne droite (vitesse/portée réelles missiles.txt),
    /// toucher au contact (rayon 0,7 tuile), bloqué par les murs.
    RangedAttack { x: i32, y: i32 },
    /// Ramasser l'objet au sol le plus proche (rayon 2 tuiles).
    Pickup,
    /// Active/coupe la course (touche Maj) : anim RN + vitesse `RunVelocity`
    /// (manifest) ou marche ×1,5 à défaut. État transient, non sauvegardé.
    SetRun(bool),
    /// Utiliser l'objet du slot d'inventaire (grille 10×4, index rangée par rangée).
    UseItem(usize),
    /// J1 : clic sur son corps au sol — à portée ([`PICKUP_RANGE`]) la
    /// récupération est immédiate (ré-équipe tout) ; sinon le héros marche
    /// vers le corps et la récupération s'exécute à l'arrivée (comportement
    /// D2 « grab your body » : un clic suffit).
    RecoverCorpse,
}

/// Événements produits par [`World::tick`]. Les 6 premières variantes sont le
/// contrat ; les suivantes sont des extras additifs documentés (REPORT.md §4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum GameEvent {
    /// Le héros a atteint la fin de son chemin (tuile arrondie).
    HeroMoved { path_end: (i32, i32) },
    /// Le héros a touché le monstre `target` (index dans `monsters()`), dégâts `dmg`.
    HeroAttacked { target: usize, dmg: u32 },
    /// Le monstre `index` a fini son anim DD et disparaît ; `xp` gagné par le héros.
    MonsterDied { index: usize, xp: u32 },
    /// Un butin est apparu au sol (coordonnées tuiles).
    ItemDropped { item: ItemDef, x: i32, y: i32 },
    /// Le héros a passé le seuil d'expérience du niveau `level`.
    HeroLeveled { level: u32 },
    /// La vie du héros est tombée à 0.
    HeroDied,
    // --- extras additifs (au-delà du contrat minimal) ---
    /// Attaque du héros ratée (jet de toucher).
    HeroMissed { target: usize },
    /// Le monstre `index` a touché le héros.
    HeroHit { index: usize, dmg: u32 },
    /// Un missile du héros a touché le monstre `target` (dégâts `dmg`, jet de
    /// toucher réussi — [`MissileEvent::Hit`] résolu côté règles).
    MissileHit { target: usize, dmg: u32 },
    /// Un missile du héros a atteint le monstre `target` mais le jet de
    /// toucher a échoué (attack rating vs AC, formule §3 GAME-DATA).
    MissileMissed { target: usize },
    /// Un missile a été décoché (son "bow release", position via `missiles()`).
    MissileSpawned,
    /// Réapparition du héros (pleine vie, point d'apparition).
    HeroRespawned { x: i32, y: i32 },
    /// Or ramassé au sol.
    GoldGained { amount: u32 },
    /// Objet ramassé dans le slot d'inventaire `slot`.
    HeroPickedUp { slot: usize },
    /// Potion bue : soin réel appliqué (0 si pleine vie).
    PotionUsed { heal: u32 },
    /// Inventaire plein : l'objet reste au sol.
    InventoryFull,
    /// Waypoint activé au contact (W05) — `slot` = rang du panneau retail
    /// (colonne `Waypoint` de `patch_levels.txt` ; 39 = extension MVP Blood
    /// Moor, cf. `waypoints.rs`). Émis une seule fois par slot.
    WaypointActivated { slot: u8 },
    /// Le héros a parlé au NPC de ville `npc` (W16, code quête bas de casse,
    /// ex. `"akara"`) — émis par [`World::talk_to_npc`] (clic NPC) et consommé
    /// par les quêtes ([`QuestEvent::NpcTalked`] : étapes q3/q4).
    NpcTalked { npc: String },
    // --- Mercenaires (H, mercenary.rs) — extras additifs ---
    /// Mercenaire embauché : libellés de table (`hireling.txt` cols
    /// `Hireling`/`SubType`), niveau du palier offert et coût (col `Gold`).
    /// Livré au tick suivant l'appel à `World::hire_merc`.
    MercHired { kind: String, subtype: String, level: u32, cost: u32 },
    /// Le mercenaire a touché le monstre `target` (mêlée ou missile résolu).
    MercAttacked { target: usize, dmg: u32 },
    /// Attaque du mercenaire ratée (jet de toucher).
    MercMissed { target: usize },
    /// Le mercenaire a décoché missile/sort (flèche rogue, bolt sorceror —
    /// position via `World::merc_missiles()`).
    MercMissileSpawned,
    /// Le mercenaire a encaissé `dmg` dégâts.
    MercHit { dmg: u32 },
    /// Le mercenaire est KO (hp 0) — cadavre sur place, résurrectible chez
    /// le NPC de son type (`World::resurrect_merc`).
    MercDied,
    /// Mercenaire ressuscité (coût MVP 0 — colonne de résurrection absente
    /// de `hireling.txt` v1.12).
    MercResurrected { cost: u32 },
    /// Le mercenaire a gagné le niveau `level` (palier = col `Exp/Lvl`).
    MercLeveled { level: u32 },
    /// J1 : le corps du héros est posé au sol à sa mort — tuile `(x, y)` et
    /// `gold` = or porté tombé au sol à côté (péage officiel déjà déduit,
    /// cf. `corpse.rs` pour les sources). Le corps porte l'inventaire ; le
    /// héros respawn nu au camp.
    CorpseDropped { x: i32, y: i32, gold: u32 },
    /// J1 : au moins un objet du corps a été repris (ré-équipé) ; le corps
    /// disparaît si TOUT son contenu a été repris, sinon le surplus y reste.
    CorpseRecovered,
    // --- IA P1 (ai_behaviors.rs, monai-behaviors étape 4) — extras additifs ---
    /// Un cadavre est redevenu une unité vivante (shaman fallen, spec §3.4) :
    /// `index` = position de la NOUVELLE unité dans `monsters()` à l'émission
    /// (les indices ne bougent plus : le retrait est antérieur).
    MonResurrected { index: usize },
    /// Le fallen `index` entre en panique (cadavre du groupe < 15 unités) et
    /// fuit à l'opposé — index au moment de l'émission (caveat REPORT.md §4).
    MonFled { index: usize },
    // --- Quêtes Acte I spec (quests_actes.rs) — extras additifs ---
    /// Une quête de l'acte I a changé de phase (`id` = `a1q1`…`a1q6`,
    /// `phase` = `inactive|active|reward_pending|done`).
    ActQuestChanged { id: String, phase: String },
    /// L'accès à l'acte `act` vient d'être débloqué (gating : remise Warriv
    /// A1Q6 → acte II, spec quests-actes.md §1.4).
    ActUnlocked { act: u8 },
    /// Fin de jeu : Baal est tombé et le héros a pris le portail de Tyrael
    /// (a5q6, spec quests-actes.md §6 « fin de jeu », étape 10c).
    GameWon,
}

/// État d'animation (code mode COF D2 : NU=neutral, WL=walk, RN=run, A1=attack1, DD=death).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnimState {
    /// Neutre (idle).
    NU,
    /// Marche.
    WL,
    /// Course (réservé — non utilisée au MVP, stamina non implémentée).
    RN,
    /// Attaque 1.
    A1,
    /// Mort.
    DD,
    /// Cast (lancement de skill — étape 15b).
    SC,
    /// Bloc (bouclier — étape 15b, non branché au gameplay).
    BL,
    /// Touché (get hit — étape 15b, non branché au gameplay).
    GH,
    /// Skill 1 (étape 15b, non branché au gameplay).
    S1,
    /// Skill 2 (étape 15b, non branché au gameplay).
    S2,
    /// Skill 3 (étape 15b, non branché au gameplay).
    S3,
    /// Skill 4 (étape 15b, non branché au gameplay).
    S4,
    /// Coup de pied (assassin — étape 15b, non branché au gameplay).
    KK,
    /// Lancer (javelot — étape 15b, non branché au gameplay).
    TH,
    /// Attaque à deux mains (étape 15b, non branché au gameplay).
    A2,
}

/// Entité visible : position, orientation, animation.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Entity {
    /// Position x (espace tuile continu, centre de tuile = tx + 0.5).
    pub x: f32,
    /// Position y (y vers le bas).
    pub y: f32,
    /// Orientation 0..=7 : 0=S, 1=SW, 2=W, 3=NW, 4=N, 5=NE, 6=E, 7=SE.
    pub facing: u8,
    /// Animation en cours.
    pub anim: AnimState,
    /// Frame courante 0..=15.
    pub anim_frame: u8,
}

/// Type d'objet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ItemKind {
    /// Potion (soin au MVP).
    #[default]
    Potion,
    /// Or.
    Gold,
    /// Arme (drop TreasureClassEx réel, `loot.rs`).
    Weapon,
    /// Armure (drop TreasureClassEx réel, `loot.rs`).
    Armor,
    /// Divers (gemmes, runes, clés, parchemins… — non utilisable au MVP).
    Misc,
}

/// Définition d'objet. `amount` = quantité d'or pour [`ItemKind::Gold`], 0 sinon.
/// Dériver `Default` : construire un littéral avec `..Default::default()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemDef {
    /// Code table D2 (`gld`, `hp1`, `hax`…).
    pub code: String,
    /// Nom affichable.
    pub name: String,
    /// Catégorie.
    pub kind: ItemKind,
    /// Montant (or uniquement), 0 sinon.
    pub amount: u32,
}

impl ItemDef {
    /// Pile d'or (`gld`).
    pub fn gold(amount: u32) -> ItemDef {
        ItemDef { code: "gld".into(), name: "Or".into(), kind: ItemKind::Gold, amount }
    }
    /// `hp1` — Lesser Healing Potion (soin 60, `patch_misc.txt`).
    pub fn potion() -> ItemDef {
        ItemDef { code: "hp1".into(), name: "Lesser Healing Potion".into(), kind: ItemKind::Potion, amount: 0 }
    }
}

/// Objet posé au sol.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ItemOnGround {
    /// Définition de l'objet.
    pub def: ItemDef,
    /// Position x (centre de tuile).
    pub x: f32,
    /// Position y.
    pub y: f32,
}

/// Stats lisibles du héros (HUD). `life`/`mana` sont les valeurs courantes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeroStats {
    /// Niveau courant (débute 1, MVP plafonné à 10).
    pub level: u32,
    /// XP cumulés.
    pub xp: u32,
    /// XP restants avant le prochain niveau (0 = niveau max MVP).
    pub xp_to_next: u32,
    /// Vie courante.
    pub life: u32,
    /// Vie max.
    pub max_life: u32,
    /// Mana courant.
    pub mana: u32,
    /// Mana max.
    pub max_mana: u32,
    /// Force (`patch_charstats` col `str`).
    pub strength: u32,
    /// Dextérité (col `dex`).
    pub dexterity: u32,
    /// Défense affichée = dex/4 + AC moyen bouclier.
    pub defense: u32,
    /// Toucher du héros = ToHitFactor + dex (simplification documentée).
    pub to_hit: u32,
    /// Or porté.
    pub gold: u32,
    /// Vitesse de marche (tuiles/s).
    pub walk_speed: f32,
    /// Soin d'une potion (`hp1` = 60).
    pub potion_heal: u32,
    /// Code du skill actif (`skills.txt`, ex. `"Fire Bolt"` ; `""` = aucun).
    #[serde(default)]
    pub skill: String,
    /// Niveau du skill actif (1 aux skills de départ).
    #[serde(default)]
    pub skill_level: u32,
    /// Endurance courante (EXP-H2, `patch_charstats` col `stamina`) ;
    /// `#[serde(default)]` : les saves antérieures chargent à 0 puis sont
    /// reconciliées au gabarit (`stamina::`/`save::load_json`).
    #[serde(default)]
    pub stamina: u32,
    /// Endurance max (gabarit du manifest).
    #[serde(default)]
    pub max_stamina: u32,
    /// Points de skill NON DÉPENSÉS (C2 : +1/niveau de perso + récompenses de
    /// quêtes — Den of Evil +1, Radament's Lair A2 +1, Izual A4 +2 ; od2
    /// audit A2/L7 `d2core/d2hero/hero_stats_state.go:9-30` skillPoints).
    /// `#[serde(default)]` OBLIGATOIRE : champ save additif (saves v1/v2/v3
    /// antérieures chargent à 0) — un autre chantier ajoute d'autres champs.
    #[serde(default)]
    pub skill_points: u32,
    /// Points de caractéristique NON DÉPENSÉS (récompenses de quêtes :
    /// Livre de Lam Esen A3 +5, spec quests-actes.md §4 [CITÉ Arreat
    /// Summit]). `#[serde(default)]` OBLIGATOIRE : champ save additif (les
    /// saves antérieures chargent à 0).
    #[serde(default)]
    pub stat_points: u32,
    /// Bonus PERMANENT en points de % aux TOUTES les résistances (récompense
    /// de quête « Prison de glace » A5Q3 : +10 par difficulté — Malah,
    /// parchemin de résistance [CITÉ trevin ; valeur confirmée par le README
    /// de l'oracle d2s-file-generator, étape 19b 2026-10-25 : « +10% to all
    /// resistances for each completed difficulty »]). S'additionne à travers
    /// les difficultés (max +30). `#[serde(default)]` OBLIGATOIRE : champ
    /// save additif (saves antérieures à 19b chargent à 0).
    #[serde(default)]
    pub res_all_pct: u32,
}

// --- Interop .d2s 19c : tests d'écriture durcie vs saves oracle (layout) ---
#[cfg(test)]
mod save_d2s_interop;
