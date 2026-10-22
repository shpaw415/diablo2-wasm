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
//! * Drops MVP : 60 % rien / 25 % or (~niveau×10) / 15 % potion heal 60 —
//!   structure prête pour TreasureClassEx réel (phase 3).

#![forbid(unsafe_code)]

mod data;
mod path;
mod rng;
mod save;
mod world;

#[cfg(test)]
mod tests;

pub use world::World;

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
    /// Arme (réservé phase 3).
    Weapon,
    /// Armure (réservé phase 3).
    Armor,
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
}
