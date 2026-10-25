//! IA P1 monstres — implémentation de la spec `d2w-re/specs/monai-behaviors.md`
//! (campagne RE d2w-re, étape 4) : **FallenShaman résurrection** (§3.4 ★) et
//! **Fallen fuite de panique** (§3.3), branchées en OVERLAY additif sur la FSM
//! `MonState` existante (world.rs — Idle/Chase/Attack/Dying inchangés : les
//! états d'action vivent dans [`MonAiExt`], aligné aux indices monstres).
//!
//! # Contrat spec (§5, extrait P1)
//! * `corpse_register` : à la mort d'un monstre à famille Fallen, le cadavre
//!   entre dans le registre persistant ([`AiState::corpses`] — 4 derniers,
//!   spec corpse_scan « max 4/room »). Le port détruit l'unité à la fin de
//!   l'anim DD (world.rs) : le registre EST l'état cadavre persistant.
//! * `resurrect` : le shaman scanne les cadavres de base `fallen` /
//!   `fallenshaman`, **ni unique ni champion**, à ≤ `aip4` unités, tire
//!   `aip1` % par think (`aidel` = 15 frames), incante `seq_shamanresurrect`
//!   (16 frames), puis le cadavre REDEVIENT une unité vivante (même gabarit,
//!   HP pleines, rang Normal).
//! * Fuite Fallen : à chaque mort du groupe, tout fallen vivant à < 15 unités
//!   d'un cadavre panique et FUIT de 12 unités à l'opposé (spec §3.3), puis
//!   revient à l'état neutre ; l'aggro est ignoré pendant la panique.
//!
//! # Données réelles ([V] patch_monstats.txt — annexe spec §8)
//! * `fallenshaman1` (ligne 60) : `aip1` = 45/50/55 (REZ+CMD %), `aip4` =
//!   24/24/26 (REZ distance, unités), `aidel` = 15 (frames), Skill1 =
//!   `Resurrect` (`seq_shamanresurrect`).
//! * `fallen1` (ligne 21) : `aip1` = 30/40/50 (commande %, non utilisé en P1
//!   flee), rayon de panique 15 unités, fuite 12 unités (~1/20 de cri — pas
//!   d'audio au port, non émis).
//! * Conversion unités D2 → tuiles : 35 unités ≈ 7 tuiles ⇒ 5 unités/tuile
//!   (hypothèse spec §1.4/§7, flaggée ? à reconfirmer côté DLL).
//!
//! # Événements émis
//! * [`GameEvent::MonResurrected`] : un cadavre est redevenu une unité
//!   vivante (`index` = position dans `monsters()`).
//! * [`GameEvent::MonFled`] : un fallen entre en panique.
//!
//! # Persistance
//! AUCUNE : l'état AI est dérivé au chargement ([`AiState::rebuild`] — les
//! archétypes viennent des codes gabarits) ; panique/incantation et cadavres
//! sont TRANSITOIRES (comme `corpse_pending`) — la save v3 reste inchangée.

use crate::world::{
    advance_along_path, dist, set_anim, advance_anim, MonKind, MonState, SpawnRule, World,
    AGGRO_RADIUS, FRAME_MS, FRAMES,
};
use crate::{AnimState, GameEvent};

// ---------------------------------------------------------------------------
// Constantes réelles (patch_monstats.txt / monai-behaviors.md §3.3-3.4, §8)
// ---------------------------------------------------------------------------

/// Conversion unités D2 → tuiles (spec §1.4 : 35 unités ≈ 7 tuiles).
pub const UNITS_PER_TILE: f32 = 5.0;
/// `aidel` réel = 15 frames (patch_monstats, col `aidel`) — période de think
/// neutre ; 15 × 100 ms.
pub const AIDEL_MS: u32 = 15 * FRAME_MS;
/// Durée d'incantation `seq_shamanresurrect` = 16 frames × 100 ms.
pub const RESURRECT_CAST_MS: u32 = FRAMES * FRAME_MS;
/// Rayon de panique Fallen = 15 unités (spec §3.3 « cadavre < 15 »).
pub const PANIC_RADIUS_UNITS: f32 = 15.0;
/// Distance de fuite = 12 unités (spec §3.3 « ESCAPE de 12 »).
pub const FLEE_DISTANCE_UNITS: f32 = 12.0;
/// Cadavres suivis : 4 derniers par salle (spec §5 corpse_scan, D2MOO
/// AiThink.cpp:28-29).
pub const MAX_CORPSES: usize = 4;
/// Cap de sécurité de la panique (2 s de course puis retour au neutre).
const PANIC_MAX_MS: u32 = 2000;

/// `fallenshaman1` aip1 = RESURRECT_AND_COMMAND_CHANCE_PCT (N/NM/H).
pub const SHAMAN_REZ_CHANCE: [u32; 3] = [45, 50, 55];
/// `fallenshaman1` aip4 = RESURRECT_DISTANCE en unités (le code compare d² à
/// aip4² — spec §3.4).
pub const SHAMAN_REZ_DISTANCE: [u32; 3] = [24, 24, 26];

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Archétype IA P1 (résolu du code gabarit manifest — spec §1 : le nom AI
/// choisit la fonction de think).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AiArchetype {
    /// Aucun comportement P1 (zombie, squelette, quillrat, boss…).
    #[default]
    None,
    /// `Fallen` (§3.3) : fuite de panique.
    Fallen,
    /// `FallenShaman` (§3.4) : résurrection.
    FallenShaman,
}

/// Famille de base (filtre corpse_scan spec : baseId ∈ {fallen*,
/// fallenshaman*}).
pub(crate) fn family_of(code: &str) -> (&'static str, AiArchetype) {
    match code.to_ascii_lowercase().as_str() {
        "fa" => ("fallen", AiArchetype::Fallen),             // fallen1 (manifest)
        "fs" => ("fallenshaman", AiArchetype::FallenShaman), // fallenshaman1
        _ => ("", AiArchetype::None),
    }
}

/// Cadavre persistant (registre — l'unité est retirée de `mon_ent` à la fin
/// du DD ; le registre garde de quoi la relancer).
#[derive(Debug, Clone)]
pub struct CorpseRec {
    /// Index de gabarit d'origine (relance du même monstre).
    pub tpl: usize,
    /// Famille de base (`"fallen"` / `"fallenshaman"`).
    pub family: &'static str,
    /// Rang au spawn — la résurrection exclut unique/champion (spec §3.4).
    pub kind: MonKind,
    /// Position du cadavre (tuiles).
    pub x: f32,
    pub y: f32,
    /// Horloge du monde à l'enregistrement.
    pub born_ms: u64,
}

/// Extension AI d'un monstre (overlay — aligné aux indices `mon_ent`/`mon_dat`).
#[derive(Debug, Clone, Default)]
pub struct MonAiExt {
    /// Archétype (dérogation : `None` = comportement historique inchangé).
    pub archetype: AiArchetype,
    /// Famille de base (`""` hors Fallen).
    pub family: &'static str,
    /// Accumulateur de think neutre (shaman).
    pub think_ms: u32,
    /// Incantation en cours (ms restants).
    pub rez_cast_ms: u32,
    /// Cadavre visé par l'incantation (index registre).
    pub rez_target: Option<usize>,
    /// Cooldown post-résurrection (un cycle think réel `aidel`).
    pub rez_cd_ms: u32,
    /// Panique en cours (ms restants).
    pub panic_ms: u32,
}

/// État AI du monde (registre des cadavres + extensions par monstre).
#[derive(Debug, Clone, Default)]
pub struct AiState {
    /// Cadavres persistants (FIFO plafonné [`MAX_CORPSES`]).
    pub corpses: Vec<CorpseRec>,
    /// Extensions — MÊME ordre / longueur que `mon_ent`/`mon_dat`.
    pub ext: Vec<MonAiExt>,
}

impl AiState {
    /// Reconstruit l'état dérivable après un chargement de save (archétypes
    /// depuis les codes gabarits) — panique/incantation/cadavres non persistés.
    pub(crate) fn rebuild(w: &World) -> AiState {
        AiState {
            corpses: Vec::new(),
            ext: w
                .mon_dat
                .iter()
                .map(|d| {
                    let (family, archetype) = family_of(&w.mon_tpls[d.tpl].code);
                    MonAiExt { archetype, family, ..MonAiExt::default() }
                })
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// Hooks appelés par world.rs (points d'accroche additifs)
// ---------------------------------------------------------------------------

/// Aligne l'extension AI après un push monstre ([`World::spawn_monster_at_rule`]
/// — point d'entrée unique des spawns, boss inclus).
pub(crate) fn on_spawned(w: &mut World) {
    let code = w.mon_tpls[w.mon_dat[w.mon_dat.len() - 1].tpl].code.clone();
    let (family, archetype) = family_of(&code);
    w.ai.ext.push(MonAiExt { archetype, family, ..MonAiExt::default() });
}

/// MORT d'un monstre (appelé juste avant le retrait dans la branche DD
/// finale de `update_monsters`) : enregistre le cadavre si la famille le
/// permet, puis déclenche la panique des Fallen vivants proches (spec §3.3).
pub(crate) fn register_corpse(w: &mut World, i: usize, ev: &mut Vec<GameEvent>) {
    let (tpl, kind, x, y) = (w.mon_dat[i].tpl, w.mon_dat[i].kind, w.mon_ent[i].x, w.mon_ent[i].y);
    let family = w.ai.ext.get(i).map(|e| e.family).unwrap_or("");
    if family.is_empty() {
        return; // hors famille Fallen : pas de cadavre suivi
    }
    if w.ai.corpses.len() >= MAX_CORPSES {
        w.ai.corpses.remove(0); // FIFO : les 4 derniers GUIDs (spec §5)
    }
    w.ai.corpses.push(CorpseRec { tpl, family, kind, x, y, born_ms: w.now_ms });

    // Panique : tout fallen vivant à < PANIC_RADIUS fuit à l'opposé.
    let radius = PANIC_RADIUS_UNITS / UNITS_PER_TILE;
    for j in 0..w.mon_dat.len() {
        if j == i || w.mon_dat[j].state == MonState::Dying {
            continue;
        }
        let (arch, already) = match w.ai.ext.get(j) {
            Some(e) => (e.archetype, e.panic_ms > 0),
            None => continue,
        };
        if arch != AiArchetype::Fallen || already {
            continue;
        }
        if dist(w.mon_ent[j].x - x, w.mon_ent[j].y - y) <= radius {
            start_flee(w, j, (x, y), ev);
        }
    }
}

/// Tick AI d'un monstre (appelé en tête d'itération de `update_monsters`,
/// après la branche DD). Renvoie `true` si l'overlay a consommé la mise à
/// jour du monstre ce frame (la FSM historique est sautée) ; `false` sinon
/// (comportement historique strictement inchangé).
pub(crate) fn tick(w: &mut World, i: usize, dt: u32, ev: &mut Vec<GameEvent>) -> bool {
    let Some(ext) = w.ai.ext.get(i) else { return false };
    if ext.archetype == AiArchetype::None || w.mon_dat[i].state == MonState::Dying {
        return false;
    }
    let casting = ext.rez_cast_ms > 0;
    let panicking = ext.panic_ms > 0;
    let archetype = ext.archetype;

    // 1. Incantation de résurrection (shaman immobile, anim A1 non bouclée).
    if casting {
        w.ai.ext[i].rez_cast_ms = w.ai.ext[i].rez_cast_ms.saturating_sub(dt);
        set_anim(&mut w.mon_ent[i], AnimState::A1, &mut w.mon_dat[i].anim_ms);
        advance_anim(&mut w.mon_ent[i], &mut w.mon_dat[i].anim_ms, dt, false);
        if w.ai.ext[i].rez_cast_ms == 0 {
            finish_resurrect(w, i, ev);
        }
        return true;
    }

    // 2. Panique (fuite à l'opposé du cadavre, aggro ignoré).
    if panicking {
        w.ai.ext[i].panic_ms = w.ai.ext[i].panic_ms.saturating_sub(dt);
        let speed = w.mon_dat[i].speed;
        let path_done = advance_along_path(&mut w.mon_ent[i], &mut w.mon_dat[i].path, speed, dt);
        if !w.mon_dat[i].path.is_empty() {
            set_anim(&mut w.mon_ent[i], AnimState::WL, &mut w.mon_dat[i].anim_ms);
            advance_anim(&mut w.mon_ent[i], &mut w.mon_dat[i].anim_ms, dt, true);
        }
        if w.ai.ext[i].panic_ms == 0 || (path_done && w.mon_dat[i].path.is_empty()) {
            // Retour au neutre (spec : PANIQUE → NEUTRE).
            w.mon_dat[i].path.clear();
            w.ai.ext[i].panic_ms = 0;
            set_anim(&mut w.mon_ent[i], AnimState::NU, &mut w.mon_dat[i].anim_ms);
        }
        return true;
    }

    // 3. Think neutre du shaman : scan cadavres + tirage aip1 (chance réelle).
    if archetype == AiArchetype::FallenShaman {
        if w.ai.ext[i].rez_cd_ms > 0 {
            w.ai.ext[i].rez_cd_ms = w.ai.ext[i].rez_cd_ms.saturating_sub(dt);
        } else if w.mon_dat[i].state == MonState::Idle {
            // 3b-1 (coop) : le héros de référence est le vivant le plus
            // proche (déterministe ; solo : slot 0, identique).
            let near = w.nearest_living_hero_pos(w.mon_ent[i].x, w.mon_ent[i].y);
            let hero_far =
                dist(w.mon_ent[i].x - near.0, w.mon_ent[i].y - near.1) > AGGRO_RADIUS;
            if hero_far {
                w.ai.ext[i].think_ms += dt;
                if w.ai.ext[i].think_ms >= AIDEL_MS {
                    w.ai.ext[i].think_ms = 0;
                    try_start_resurrect(w, i);
                }
            }
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Comportements
// ---------------------------------------------------------------------------

/// Démarre la fuite du fallen `i` à l'opposé du point `from` (spec : ESCAPE
/// de 12 unités). Sans chemin praticable : panique courte sur place.
fn start_flee(w: &mut World, i: usize, from: (f32, f32), ev: &mut Vec<GameEvent>) {
    let (mx, my) = (w.mon_ent[i].x, w.mon_ent[i].y);
    let (dx, dy) = (mx - from.0, my - from.1);
    let d = dist(dx, dy);
    let flee_tiles = FLEE_DISTANCE_UNITS / UNITS_PER_TILE;
    let target = if d > 0.001 {
        (mx + dx / d * flee_tiles, my + dy / d * flee_tiles)
    } else {
        (mx + flee_tiles, my) // cadavre sous les pieds : Est par défaut
    };
    let tile = (target.0.floor() as i32, target.1.floor() as i32);
    w.mon_dat[i].path = w.path_between(
        (mx.floor() as i32, my.floor() as i32),
        tile,
    )
    .unwrap_or_default();
    w.mon_dat[i].repath_ms = 0;
    w.ai.ext[i].panic_ms = PANIC_MAX_MS;
    w.mon_ent[i].facing = crate::world::facing_from(dx, dy);
    ev.push(GameEvent::MonFled { index: i });
}

/// Cherche un cadavre résurrectible (spec §3.4 : baseId ∈ {fallen*,
/// fallenshaman*}, ni unique ni champion, ≤ aip4 unités) et tire aip1 % ;
/// succès ⇒ début d'incantation (`seq_shamanresurrect`).
fn try_start_resurrect(w: &mut World, i: usize) {
    let diff = w.difficulty.index().min(2);
    let dist_tiles = SHAMAN_REZ_DISTANCE[diff.min(SHAMAN_REZ_DISTANCE.len() - 1)] as f32
        / UNITS_PER_TILE;
    let (mx, my) = (w.mon_ent[i].x, w.mon_ent[i].y);
    let mut best: Option<(usize, f32)> = None;
    for (ci, c) in w.ai.corpses.iter().enumerate() {
        // « ni unique ni champion » (spec, D2MOO AiThink.cpp:1183-1229) :
        // seuls les rangs Normaux (et SuperUnique nominé = classe de base
        // rang normal) sont relevables ; MVP : rang Normal strict.
        if c.kind != MonKind::Normal {
            continue;
        }
        if c.family != "fallen" && c.family != "fallenshaman" {
            continue;
        }
        let d = dist(c.x - mx, c.y - my);
        if d <= dist_tiles && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((ci, d));
        }
    }
    let Some((ci, _)) = best else { return };
    // AIRollChanceParam : roll(0-99) < aipN (spec §2).
    let chance = SHAMAN_REZ_CHANCE[diff.min(SHAMAN_REZ_CHANCE.len() - 1)];
    if w.rng.below(100) >= chance {
        return;
    }
    let ext = &mut w.ai.ext[i];
    ext.rez_cast_ms = RESURRECT_CAST_MS;
    ext.rez_target = Some(ci);
    ext.think_ms = 0;
    w.mon_dat[i].path.clear();
    w.mon_dat[i].anim_ms = 0;
    w.mon_ent[i].anim = AnimState::A1;
    w.mon_ent[i].anim_frame = 0;
}

/// Fin d'incantation : le cadavre REDEVIENT une unité vivante — même gabarit,
/// position du cadavre, HP PLEINES, rang Normal (spec §3.4 contrat clé).
fn finish_resurrect(w: &mut World, i: usize, ev: &mut Vec<GameEvent>) {
    let Some(ci) = w.ai.ext[i].rez_target.take() else { return };
    if ci >= w.ai.corpses.len() {
        return; // cadavre évicté pendant l'incantation : rien
    }
    let c = w.ai.corpses.remove(ci);
    w.spawn_monster_at_rule(c.tpl, c.x, c.y, SpawnRule { kind: MonKind::Normal, ..SpawnRule::default() });
    let ni = w.mon_ent.len() - 1;
    w.mon_dat[ni].hp = w.mon_dat[ni].max_hp; // HP complètes
    w.mon_dat[ni].state = MonState::Idle;
    // Cooldown : un cycle think réel (`aidel`) avant la prochaine tentative.
    w.ai.ext[i].rez_cd_ms = AIDEL_MS;
    ev.push(GameEvent::MonResurrected { index: ni });
}

// ---------------------------------------------------------------------------
// Tests : résurrection (shaman), panique (fallen), filtres, cooldown, golden.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::DYING_MS;

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn manifest() -> serde_json::Value {
        serde_json::from_str(MANIFEST_JSON).unwrap()
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Monde vide de monstres, héros au centre (auto-spawn annulé).
    fn world_ai(seed: u64) -> World {
        let mut w = World::with_seed(open_grid(56, 40), manifest(), seed);
        w.clear_monsters();
        w
    }

    /// Tue proprement un monstre (branche DD de update_monsters) : l'anim DD
    /// joue puis le retrait enregistre le cadavre.
    fn kill(w: &mut World, i: usize) {
        w.mon_dat[i].state = MonState::Dying;
        w.mon_dat[i].dying_ms = 0;
        w.mon_dat[i].path.clear();
    }

    /// Avance `ms` de simulation par pas de 100 ms, événements cumulés.
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

    /// Téléporte le héros (tests : simule un déplacement hors aggro).
    fn teleport_hero(w: &mut World, x: f32, y: f32) {
        w.players[0].ent.x = x;
        w.players[0].ent.y = y;
        w.players[0].data.path.clear();
    }

    // -- golden : valeurs réelles des tables (patch_monstats §8) -------------

    #[test]
    fn golden_real_aip_values() {
        assert_eq!(SHAMAN_REZ_CHANCE, [45, 50, 55]);
        assert_eq!(SHAMAN_REZ_DISTANCE, [24, 24, 26]);
        assert_eq!(AIDEL_MS, 1500, "aidel = 15 frames × 100 ms");
        assert_eq!(RESURRECT_CAST_MS, 1600, "seq_shamanresurrect = 16 frames");
        assert_eq!(PANIC_RADIUS_UNITS, 15.0);
        assert_eq!(FLEE_DISTANCE_UNITS, 12.0);
        assert_eq!(MAX_CORPSES, 4);
    }

    #[test]
    fn family_resolution_from_manifest_codes() {
        assert_eq!(family_of("FA"), ("fallen", AiArchetype::Fallen));
        assert_eq!(family_of("fs"), ("fallenshaman", AiArchetype::FallenShaman));
        assert_eq!(family_of("ZM"), ("", AiArchetype::None));
        assert_eq!(family_of("AN"), ("", AiArchetype::None));
    }

    // -- FallenShaman : résurrection complète --------------------------------

    #[test]
    fn shaman_resurrects_fallen_corpse() {
        let mut w = world_ai(0xD2A1_0001);
        // Fallen victime à (6,5), shaman à (10,6) : distance ≈ 4,1 ≤ aip4
        // (24 unités = 4,8 tuiles) ; héros loin des deux.
        assert!(w.spawn_monster("FA", 6, 5));
        assert!(w.spawn_monster("FS", 10, 6));
        teleport_hero(&mut w, 28.0, 20.0);
        assert_eq!(w.monsters().len(), 2);
        kill(&mut w, 0); // le fallen meurt
        let ev = run(&mut w, DYING_MS + 100);
        // Fin du DD : retrait + cadavre enregistré.
        assert_eq!(w.monsters().len(), 1, "fallen retiré après le DD");
        assert_eq!(w.ai.corpses.len(), 1);
        assert_eq!(w.ai.corpses[0].family, "fallen");
        assert_eq!(w.ai.corpses[0].kind, MonKind::Normal);
        assert!((w.ai.corpses[0].x - 6.5).abs() < 0.01, "cadavre à la position de mort");
        // Le shaman pense toutes les aidel (1,5 s) à aip1=45 % : tique par
        // pas de 100 ms et s'arrête DÈS la résurrection (l'unité relancée
        // n'a pas eu le temps d'errer).
        let mut ev = Vec::new();
        for _ in 0..400 {
            let e = w.tick(100, None);
            let hit = e.iter().any(|x| matches!(x, GameEvent::MonResurrected { .. }));
            ev.extend(e);
            if hit {
                break;
            }
        }
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MonResurrected { .. })),
            "le shaman doit ressusciter le cadavre fallen"
        );
        assert_eq!(w.monsters().len(), 2, "le cadavre redevient une unité vivante");
        assert!(w.ai.corpses.is_empty(), "le cadavre est consommé");
        // L'unité relancée : même gabarit (FA), HP pleines, rang Normal,
        // position du cadavre d'origine.
        let ni = w.monsters().len() - 1;
        let (hp, max_hp) = w.monster_hp(ni).unwrap();
        assert_eq!(hp, max_hp, "HP complètes (spec §3.4)");
        assert_eq!(w.monster_kind(ni), Some(MonKind::Normal));
        assert_eq!(w.ai.ext[ni].archetype, AiArchetype::Fallen);
        assert!((w.monsters()[ni].x - 6.5).abs() < 0.01, "relancé sur la tuile du cadavre");
        assert!((w.monsters()[ni].y - 5.5).abs() < 0.01);
    }

    #[test]
    fn shaman_resurrect_cooldown_then_second_corpse() {
        let mut w = world_ai(0xD2A1_0002);
        assert!(w.spawn_monster("FA", 6, 5));
        assert!(w.spawn_monster("FA", 7, 6));
        assert!(w.spawn_monster("FS", 10, 6));
        teleport_hero(&mut w, 28.0, 20.0);
        kill(&mut w, 0);
        run(&mut w, DYING_MS + 100);
        // Tique par pas de 100 ms jusqu'à la résurrection, puis s'arrête
        // IMMÉDIATEMENT : le cooldown post-résurrection est observable.
        let mut ev = Vec::new();
        for _ in 0..400 {
            let e = w.tick(100, None);
            let hit = e.iter().any(|x| matches!(x, GameEvent::MonResurrected { .. }));
            ev.extend(e);
            if hit {
                break;
            }
        }
        assert!(ev.iter().any(|e| matches!(e, GameEvent::MonResurrected { .. })));
        let shaman = (0..w.monsters().len())
            .find(|i| w.ai.ext[*i].archetype == AiArchetype::FallenShaman)
            .unwrap();
        assert!(w.ai.ext[shaman].rez_cd_ms > 0, "cooldown réel (un cycle aidel) après le sort");
        // Un second cadavre est ressuscité après le cooldown.
        kill(&mut w, 0); // l'autre fallen d'origine
        run(&mut w, DYING_MS + 100);
        let ev = run(&mut w, 60_000);
        let rez = ev.iter().filter(|e| matches!(e, GameEvent::MonResurrected { .. })).count();
        assert_eq!(rez, 1, "le second cadavre est ressuscité après cooldown");
        assert!(w.ai.corpses.is_empty());
    }

    #[test]
    fn resurrect_filters_unique_and_distance() {
        // Cadavre UNIQUE : jamais ressuscité (« ni unique ni champion »).
        let mut w = world_ai(0xD2A1_0003);
        let rule = SpawnRule { kind: MonKind::Unique, ..SpawnRule::default() };
        assert!(w.spawn_monster_rule("FA", 6, 5, rule));
        assert!(w.spawn_monster("FS", 10, 6));
        teleport_hero(&mut w, 28.0, 20.0);
        kill(&mut w, 0);
        run(&mut w, DYING_MS + 100);
        assert_eq!(w.ai.corpses[0].kind, MonKind::Unique);
        let ev = run(&mut w, 40_000);
        assert!(
            !ev.iter().any(|e| matches!(e, GameEvent::MonResurrected { .. })),
            "un cadavre unique n'est pas relevable"
        );
        assert_eq!(w.ai.corpses.len(), 1);

        // Cadavre trop loin (> 4,8 tuiles du shaman) : pas de résurrection.
        let mut w = world_ai(0xD2A1_0004);
        assert!(w.spawn_monster("FA", 5, 5));
        assert!(w.spawn_monster("FS", 16, 6)); // ≈ 11 tuiles
        teleport_hero(&mut w, 28.0, 20.0);
        kill(&mut w, 0);
        run(&mut w, DYING_MS + 100);
        let ev = run(&mut w, 40_000);
        assert!(!ev.iter().any(|e| matches!(e, GameEvent::MonResurrected { .. })));
    }

    #[test]
    fn shaman_does_not_think_when_hero_near() {
        let mut w = world_ai(0xD2A1_0005);
        assert!(w.spawn_monster("FA", 6, 5));
        assert!(w.spawn_monster("FS", 7, 6));
        kill(&mut w, 0);
        run(&mut w, DYING_MS + 100);
        // Héros DANS le rayon d'aggro du shaman : aucun think, aucun sort.
        teleport_hero(&mut w, 8.0, 7.0);
        let ev = run(&mut w, 20_000);
        assert!(!ev.iter().any(|e| matches!(e, GameEvent::MonResurrected { .. })));
        assert!(w.ai.corpses.len() == 1, "le cadavre attend");
    }

    // -- Fallen : panique + fuite + retour ------------------------------------

    #[test]
    fn fallen_flees_when_pack_member_dies() {
        let mut w = world_ai(0xD2A1_0006);
        // Deux fallen adjacents, héros loin.
        assert!(w.spawn_monster("FA", 6, 5));
        assert!(w.spawn_monster("FA", 7, 5));
        teleport_hero(&mut w, 28.0, 20.0);
        kill(&mut w, 0);
        // Fin du DD : le retrait enregistre le cadavre et déclenche la panique
        // du survivant (MonFled ; l'index est celui d'ÉMISSION, avant retrait).
        let ev = run(&mut w, DYING_MS + 100);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MonFled { .. })),
            "le fallen survivant entre en panique (MonFled)"
        );
        // Il FUIT : il s'éloigne du cadavre (cadavre à (6,5), survivant à (7,5)).
        let survivor = 0; // après retrait du mort, le survivant est l'index 0
        let (sx, sy) = (w.monsters()[survivor].x, w.monsters()[survivor].y);
        let d0 = dist(6.5 - 7.5, 5.5 - 5.5); // distance initiale survivant↔cadavre
        let d1 = dist(sx - 6.5, sy - 5.5);
        assert!(d1 > d0, "le fallen s'éloigne du cadavre ({} → {})", d0, d1);
        assert!(w.ai.ext[survivor].panic_ms > 0, "panique en cours");
        assert_eq!(w.ai.corpses.len(), 1);
        // Et il RETOURNE au neutre une fois la panique passée.
        run(&mut w, PANIC_MAX_MS + 1000);
        assert_eq!(w.ai.ext[survivor].panic_ms, 0, "panique terminée");
        assert_eq!(w.mon_dat[survivor].state, MonState::Idle);
    }

    #[test]
    fn non_fallen_neither_panics_nor_leaves_corpse() {
        let mut w = world_ai(0xD2A1_0007);
        assert!(w.spawn_monster("ZM", 6, 5));
        assert!(w.spawn_monster("FA", 7, 5)); // un fallen à côté
        teleport_hero(&mut w, 28.0, 20.0);
        kill(&mut w, 0); // le ZOMBIE meurt
        let ev = run(&mut w, DYING_MS + 100);
        assert!(
            !ev.iter().any(|e| matches!(e, GameEvent::MonFled { .. })),
            "un cadavre hors famille Fallen ne panique personne"
        );
        assert!(w.ai.corpses.is_empty(), "pas de cadavre suivi hors famille Fallen");
    }

    #[test]
    fn corpse_registry_caps_at_four() {
        let mut w = world_ai(0xD2A1_0008);
        // Espacés > rayon de panique (3 tuiles) : les survivants ne paniquent
        // pas. Positions de mort relevées au vol (les Idle errent : la
        // position de mort ≠ position de spawn).
        for x in [5, 10, 15, 20, 25] {
            assert!(w.spawn_monster("FA", x, 5));
        }
        teleport_hero(&mut w, 28.0, 20.0);
        let mut deaths: Vec<(f32, f32)> = Vec::new();
        for i in (0..5).rev() {
            deaths.push((w.mon_ent[i].x, w.mon_ent[i].y));
            kill(&mut w, i);
            run(&mut w, DYING_MS + 100);
        }
        assert_eq!(w.ai.corpses.len(), MAX_CORPSES, "FIFO : 4 derniers GUIDs");
        // Les 4 PLUS RÉCENTS sont gardés, du plus ancien au plus récent —
        // le premier mort (déaths[0]) a été évicté.
        for (k, c) in w.ai.corpses.iter().enumerate() {
            assert!((c.x - deaths[k + 1].0).abs() < 0.01 && (c.y - deaths[k + 1].1).abs() < 0.01);
        }
    }

    // -- intégration World : alignement après retrait / clear -----------------

    #[test]
    fn ext_stays_aligned_through_spawns_and_removals() {
        let mut w = world_ai(0xD2A1_0009);
        assert!(w.spawn_monster("FA", 6, 5));
        assert!(w.spawn_monster("FS", 7, 6));
        assert!(w.spawn_monster("ZM", 8, 7));
        assert_eq!(w.ai.ext.len(), w.monsters().len());
        kill(&mut w, 1); // le shaman meurt (cadavre fallenshaman suivi)
        run(&mut w, DYING_MS + 100);
        assert_eq!(w.ai.ext.len(), w.monsters().len(), "alignement après retrait");
        assert_eq!(w.ai.corpses[0].family, "fallenshaman", "un shaman mort est un cadavre relevable");
        w.clear_monsters();
        assert!(w.ai.ext.is_empty() && w.ai.corpses.is_empty(), "clear_monsters purge l'AI");
    }

    #[test]
    fn save_load_rebuilds_ai_and_keeps_quests_act1() {
        let mut w = World::with_seed(open_grid(56, 40), manifest(), 0xD2A1_000A);
        w.set_quest_act1(crate::quests_actes::ActQuests::new());
        w.quest_act1_event(&crate::quests_actes::ActEvent::NpcTalked { npc: "akara".into() });
        w.quest_act1_event(&crate::quests_actes::ActEvent::ZoneEntered { level_id: 8 });
        let json = w.save();
        let w2 = World::load(&json).expect("save valide");
        // AI dérivée (archétypes) + quêtes acte I restaurées.
        assert_eq!(w2.ai.ext.len(), w2.monsters().len());
        let log = w2.quest_act1().expect("journal restauré");
        assert_eq!(log.run("a1q1").unwrap().phase, crate::quests_actes::ActPhase::Active);
        // bit 2 « donnée par PNJ » (intro Akara) + bit 4 « repaire entré »
        // (Cave 1 id 8) — machine A1Q1 golden.
        assert_eq!(
            log.run("a1q1").unwrap().flags,
            crate::quests_actes::FLAG_GIVEN_BY_NPC | crate::quests_actes::FLAG_DEN_ENTERED
        );
        // L'ancienne save (sans le champ) charge toujours.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("quest_act1");
        let w3 = World::load(&v.to_string()).expect("save v3 sans champ additif");
        assert!(w3.quest_act1().is_none());
    }
}
