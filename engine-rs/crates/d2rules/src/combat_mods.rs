//! `combat_mods` — étape 12c : formules D2 de mods de coup + éléments (solo).
//!
//! Module PUR (aucune IO, aucun RNG interne — les jets arrivent en entrée,
//! `roll_*` 0..=99 produits par l'appelant) et DÉTERMINISTE. Branché
//! ADDITIVEMENT dans le pipeline de dégâts de `world.rs` : sans mods
//! ([`HitMods::default()`], l'état du port aujourd'hui) l'arithmétique est
//! strictement celle d'avant (résist puis soustraction).
//!
//! # Sources réelles v1.12 (chaque golden test cite `fichier:ligne:colonne`)
//!
//! * `assets/patch_itemstatcost.txt` — ids des mods : `item_deadlystrike`
//!   ligne 143 (id 141), `item_crushingblow` ligne 138 (id 136),
//!   `item_openwounds` ligne 137 (id 135), `coldlength` ligne 58 (id 56),
//!   `poisonlength` ligne 61 (id 59) ; plafonds de résist héros :
//!   `maxfireresist`/`maxlightresist`/`maxcoldresist`/`maxpoisonresist`
//!   (ids 40/42/44/46) et `maxmagicresist` (id 38).
//! * `assets/patch_difficultylevels.txt` (lignes 2-4) — `ResistPenalty`
//!   0/−40/−100 (pénalité héros), `LifeStealDivisor`/`ManaStealDivisor`
//!   1/2/3 (vole-de-vie/mana), `MonsterColdDivisor` 1/2/4 (durée de froid
//!   subie par les monstres) — consommés via [`crate::difficulty::embedded`].
//! * `assets/patch_monstats.txt` — résists monstres par difficulté
//!   `ResDm..ResPo` ($130-135) ± `(N)` ($136-141) ± `(H)` ($142-147) ;
//!   convention tables v1.12 : vide = 0, ≥ 100 = IMMUNITÉ (le type est
//!   sauté : 0 dégât, pas de DoT/poison, pas de refroidissement).
//!   Ancre réelle A1 : `skeleton2` (Returned, ligne 3, TC1(H)
//!   `Act 1 (H) H2H B`) `ResPo(N)`=75 / `ResPo(H)`=100 ⇒ immunisé poison
//!   en Enfer uniquement.
//! * `assets/patch_skills.txt` — durées élémentaires `ELen` en frames
//!   (poison) : `Poison Javelin` ligne 17 ELen=200 (8 s à 25 fps),
//!   `Poison Nova` ligne 94 ELen=50 (2 s), `Poison Dagger` ligne 75
//!   ELen=50 ; `HitShift` (jet de toucher) ligne `Attack`=8.
//!
//! # Formules issues des docs publiques NON FIABLES (Chaos Sanctuary /
//! Phrozen Keep, convention 1.10+) — citées pour mémoire, plafonnées et
//! testées en golden, jamais inventées :
//!
//! * **Deadly strike** : `roll < deadly_strike_pct` ⇒ dégâts d'ARME
//!   physiques ×2 ; JAMAIS sur la portion élémentaire/magique (ni sur les
//!   crits de sort) ; exclusif avec Critical Strike (un seul ×2 s'applique —
//!   un seul jet ici).
//! * **Crushing blow** : chance `item_crushingblow` % ; si proc, la victime
//!   perd une fraction de ses PV ACTUELS, ignorante des résists :
//!   1/8 vs monstre normal, 1/16 vs champion/unique/super-unique/boss,
//!   1/32 vs joueur (N/A solo — gardé dans l'API). Part entière.
//! * **Open wounds** (mêlée seule, chance %) : saignement 0,25 × clvl par
//!   seconde pendant 8..200 frames selon le clvl (interpolation linéaire
//!   8 → 200 sur clvl 1 → 99).
//! * **Leech** : `pct × dégâts physiques FINAUX / 100 / diviseur de la
//!   difficulté` (`LifeStealDivisor`/`ManaStealDivisor` 1/2/3), plafonné
//!   au MANQUE de vie/mana (pas de sur-soin). (Règle anti-leech morts-vivants
//!   NM/Hell : hors port — aucune colonne de donnée undead dans l'arbre.)
//! * **Poison** : dégâts totaux étalés sur `ELen/25` ticks de 25 frames
//!   (1 s à 25 fps v1.12) — arrondis entiers documentés.
//! * **Froid** : vitesse de déplacement ×1/2 pendant `coldlength` frames
//!   DIVISÉES par `MonsterColdDivisor` (1/2/4) de la difficulté.
//!
//! Les DoT d'une même source ne s'empilent pas : le nouveau remplace
//! l'ancien ([`Dot`] unique par type — plaies / poison) ; simplification
//! documentée (le stacking poison v1.12 bit-rate est hors port).

use crate::difficulty::{self, Difficulty, RES_IMMUNE_THRESHOLD};
use d2missiles::DamageType;

// --- Ids `patch_itemstatcost.txt` (preuves fichier:ligne dans l'en-tête) ---
/// `item_deadlystrike` (ligne 143).
pub const ITEM_DEADLYSTRIKE: i32 = 141;
/// `item_crushingblow` (ligne 138).
pub const ITEM_CRUSHINGBLOW: i32 = 136;
/// `item_openwounds` (ligne 137).
pub const ITEM_OPENWOUNDS: i32 = 135;
/// `coldlength` — durée de refroidissement en frames (ligne 58).
pub const COLDLENGTH: i32 = 56;
/// `poisonlength` (ligne 61).
pub const POISONLENGTH: i32 = 59;
/// Plafond de résistance de base du héros (75).
pub const RES_CAP_BASE: i32 = 75;
/// Plafond absolu via `max*resist` (ids 38/40/42/44/46) : 90 au retail.
pub const RES_CAP_MAX: i32 = 90;
/// Bonus maximal cumulable de `max*resist` (+25 au retail).
pub const RES_MAX_BONUS: i32 = 25;

/// Multiplicateur de coup mortel (deadly strike).
pub const DEADLY_MULT: u32 = 2;
/// Froid : multiplicateur de vitesse 1/2 (`CHILL_SPEED_NUM/DEN`).
pub const CHILL_SPEED_NUM: u32 = 1;
/// Froid : dénominateur (vitesse ×0.5).
pub const CHILL_SPEED_DEN: u32 = 2;
/// Frames v1.12 : 25 fps ⇒ 40 ms par frame (DoT/refroidissement).
pub const FRAME_MS_D2: u32 = 40;
/// Tick des DoT : 25 frames = 1 s.
pub const DOT_TICK_FRAMES: u32 = 25;
/// Open wounds : 0,25 × clvl par seconde (num/den).
pub const OW_DMG_PER_SEC_NUM: u32 = 1;
/// Open wounds : dénominateur (¼ clvl).
pub const OW_DMG_PER_SEC_DEN: u32 = 4;
/// Open wounds : durée minimale 8 frames (clvl 1).
pub const OW_FRAMES_MIN: u32 = 8;
/// Open wounds : durée maximale 200 frames (clvl 99).
pub const OW_FRAMES_MAX: u32 = 200;

// --- Fractions de crushing blow (docs non fiables, convention 1.10+) ---
/// Crushing blow vs monstre normal : 1/8 des PV actuels.
pub const CB_NORMAL: (u32, u32) = (1, 8);
/// Crushing blow vs champion/unique/super-unique/boss : 1/16.
pub const CB_CHAMPION_UNIQUE: (u32, u32) = (1, 16);
/// Crushing blow vs joueur : 1/32 (N/A solo — API complète).
pub const CB_PLAYER: (u32, u32) = (1, 32);

/// Classe de la victime — pilote la fraction de crushing blow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VictimKind {
    /// Monstre normal : 1/8.
    #[default]
    Normal,
    /// Champion / unique / super-unique / boss : 1/16.
    ChampionUnique,
    /// Joueur : 1/32 (N/A solo).
    Player,
}

impl VictimKind {
    /// Fraction de crushing blow (numérateur, dénominateur).
    pub fn crushing_fraction(&self) -> (u32, u32) {
        match self {
            VictimKind::Normal => CB_NORMAL,
            VictimKind::ChampionUnique => CB_CHAMPION_UNIQUE,
            VictimKind::Player => CB_PLAYER,
        }
    }

    /// Rang [`crate::world::MonKind`] → classe de victime (les 4 rangs
    /// champion/unique/super-unique/boss partagent la fraction 1/16).
    pub fn from_mon_kind(kind: &crate::world::MonKind) -> VictimKind {
        match kind {
            crate::world::MonKind::Normal => VictimKind::Normal,
            _ => VictimKind::ChampionUnique,
        }
    }
}

/// État de la victime au moment du coup (copié — la fonction est pure).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Victim {
    /// Classe (fraction de crushing blow).
    pub kind: VictimKind,
    /// PV actuels (base du crushing blow).
    pub cur_hp: u32,
    /// Résists effectifs `[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]`
    /// (colonnes `patch_monstats.txt` 130-147 ; ≥100 = immunité).
    pub res: [i32; 6],
}

/// Mods de coup actifs du porteur (futur branchement d'équipement ;
/// aujourd'hui posés via `World::set_hero_combat_mods`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HitMods {
    /// `item_deadlystrike` (id 141) — chance % de dégâts ×2 (arme physique).
    pub deadly_strike_pct: u32,
    /// `item_crushingblow` (id 136) — chance % de CB.
    pub crushing_blow_pct: u32,
    /// `item_openwounds` (id 135) — chance % de plaie (mêlée).
    pub open_wounds_pct: u32,
    /// % vie volée par coup (arme physique).
    pub life_leech_pct: u32,
    /// % mana volé par coup (arme physique).
    pub mana_leech_pct: u32,
    /// Durée de poison infligé (`ELen` `patch_skills.txt`, frames) —
    /// 0 = le poison reste instantané (comportement historique).
    pub elen_frames: u32,
    /// Durée de froid infligé (`coldlength` id 56, frames) — 0 = aucun.
    pub cold_len_frames: u32,
}

impl HitMods {
    /// Au moins un mod offensif actif — l'appelant ne tire des jets RNG que
    /// dans ce cas (zéro tirage ajouté sur le chemin historique sans mods).
    pub fn any_offensive(&self) -> bool {
        self.deadly_strike_pct > 0
            || self.crushing_blow_pct > 0
            || self.open_wounds_pct > 0
            || self.life_leech_pct > 0
            || self.mana_leech_pct > 0
            || self.elen_frames > 0
            || self.cold_len_frames > 0
    }
}

/// Un coup entrant sur une victime (toutes valeurs copiées — pur).
#[derive(Debug, Clone, Copy)]
pub struct CombatHit {
    /// Dégâts d'ARME physiques (après force, avant résist/deadly).
    pub physical: u32,
    /// Portion élémentaire (missile/sort) — résistée via `victim.res[element]`.
    pub elemental: u32,
    /// Élément de la portion élémentaire (`Physical` = coup d'arme pur).
    pub element: DamageType,
    /// Mods du porteur.
    pub mods: HitMods,
    /// Niveau du porteur (open wounds : 0,25 × clvl/s).
    pub clvl: u32,
    /// Difficulté courante (divisors leech / froid, `patch_difficultylevels`).
    pub difficulty: Difficulty,
    /// État de la victime.
    pub victim: Victim,
    /// Vie actuelle du porteur (plafond de leech).
    pub wearer_life: u32,
    /// Vie max du porteur.
    pub wearer_max_life: u32,
    /// Mana actuel du porteur.
    pub wearer_mana: u32,
    /// Mana max du porteur.
    pub wearer_max_mana: u32,
    /// Coup de mêlée (seul l'open wounds est limité à la mêlée ; deadly
    /// strike s'applique aussi à une flèche physique, crushing blow partout).
    pub melee: bool,
    /// Jet deadly strike 0..=99 (produit par l'appelant — zéro RNG ici).
    pub roll_deadly: u32,
    /// Jet open wounds 0..=99.
    pub roll_wounds: u32,
    /// Jet crushing blow 0..=99.
    pub roll_crushing: u32,
}

/// Résultat d'un coup après mods (le site appelant soustrait `total`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CombatResult {
    /// Dégâts physiques FINAUX (résistés ResDm, ×2 si deadly strike).
    pub physical: u32,
    /// Portion élémentaire FINALE (résistée, 0 si immunisé ou convertie en
    /// poison DoT).
    pub elemental: u32,
    /// Deadly strike a proc (le ×2 est déjà dans `physical`).
    pub deadly: bool,
    /// Dégâts de crushing blow (fraction des PV actuels, hors résist) —
    /// s'ajoute au total infligé.
    pub crushing: u32,
    /// Plaie ouverte à poser sur la victime (mêlée, chance %).
    pub wounds: Option<Dot>,
    /// Poison à poser sur la victime (portion élémentaire convertie).
    pub poison: Option<Dot>,
    /// Refroidissement infligé (frames — ×[`FRAME_MS_D2`] côté moteur).
    pub chill_frames: u32,
    /// Vie volée (plafonnée au manque, diviseur de difficulté appliqué).
    pub life_leech: u32,
    /// Mana volé (idem).
    pub mana_leech: u32,
    /// Dégâts INSTANTANÉS totaux = `physical + elemental + crushing`.
    pub total: u32,
}

/// DoT générique (plaie ouverte / poison) — ticks de 25 frames (1 s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Dot {
    /// Dégâts par tick (1 s).
    pub dmg_per_tick: u32,
    /// Ticks restants.
    pub remaining_ticks: u32,
    /// Accumulateur sous-tick (ms).
    pub acc_ms: u32,
}

impl Dot {
    /// Durée d'un tick : 25 frames × 40 ms = 1000 ms (25 fps v1.12).
    pub const TICK_MS: u32 = DOT_TICK_FRAMES * FRAME_MS_D2;

    /// Avance le DoT de `dt_ms` et renvoie les dégâts de ce pas.
    pub fn tick(&mut self, dt_ms: u32) -> u32 {
        if self.remaining_ticks == 0 {
            return 0;
        }
        self.acc_ms = self.acc_ms.saturating_add(dt_ms);
        let mut dmg = 0;
        while self.acc_ms >= Self::TICK_MS && self.remaining_ticks > 0 {
            self.acc_ms -= Self::TICK_MS;
            self.remaining_ticks -= 1;
            dmg += self.dmg_per_tick;
        }
        dmg
    }
}

/// Résolution PURE d'un coup : deadly strike → résists → crushing blow →
/// open wounds → poison (DoT) → froid → leech. Sans mods, `physical` =
/// `apply_resist(dmg, ResDm)` et `elemental` = `apply_resist(dmg, res[elem])`
/// — l'arithmétique historique, à l'identique.
pub fn apply_combat_mods(hit: &CombatHit) -> CombatResult {
    let m = hit.mods;
    let v = hit.victim;
    let res_elem = v.res[Difficulty::resist_index(hit.element)];
    let elem_immune = res_elem >= RES_IMMUNE_THRESHOLD;

    // 1) Deadly strike : arme PHYSIQUE uniquement (jamais élémentaire/magique),
    //    chance %, dégâts ×2 AVANT résist.
    let deadly =
        hit.element == DamageType::Physical && m.deadly_strike_pct > 0 && hit.roll_deadly < m.deadly_strike_pct;
    let phys_raw = hit.physical.saturating_mul(if deadly { DEADLY_MULT } else { 1 });
    let physical = difficulty::apply_resist(phys_raw, v.res[0]);

    // 2) Portion élémentaire. Immunité (≥100) : le TYPE est sauté — 0 dégât,
    //    pas de DoT poison, pas de froid. Poison avec durée (ELen) : converti
    //    en DoT (le total résisté est étalé), rien d'instantané.
    let (elemental, poison) = if hit.elemental == 0 || elem_immune {
        (0, None)
    } else if hit.element == DamageType::Poison && m.elen_frames > 0 {
        let total = difficulty::apply_resist(hit.elemental, res_elem);
        (0, poison_dot(total, m.elen_frames))
    } else {
        (difficulty::apply_resist(hit.elemental, res_elem), None)
    };

    // 3) Crushing blow : chance %, fraction des PV ACTUELS selon la classe,
    //    insensible aux résists ; part entière.
    let crushing = if m.crushing_blow_pct > 0 && hit.roll_crushing < m.crushing_blow_pct {
        let (num, den) = v.kind.crushing_fraction();
        ((v.cur_hp as u64 * num as u64) / den as u64) as u32
    } else {
        0
    };

    // 4) Open wounds : MÊLÉE seulement, chance %, saignement 0,25 × clvl/s
    //    pendant 8..200 frames selon clvl.
    let wounds = if hit.melee && m.open_wounds_pct > 0 && hit.roll_wounds < m.open_wounds_pct {
        Some(open_wound(hit.clvl))
    } else {
        None
    };

    // 5) Leech : % des dégâts physiques FINAUX, ÷ diviseur de la difficulté
    //    (`LifeStealDivisor`/`ManaStealDivisor` 1/2/3), plafonné au manque.
    let row = difficulty::embedded().row(hit.difficulty);
    let life_leech = cap_leech(
        leech_raw(physical, m.life_leech_pct, row.life_steal_divisor),
        hit.wearer_max_life.saturating_sub(hit.wearer_life),
    );
    let mana_leech = cap_leech(
        leech_raw(physical, m.mana_leech_pct, row.mana_steal_divisor),
        hit.wearer_max_mana.saturating_sub(hit.wearer_mana),
    );

    // 6) Froid : vitesse ×1/2 pendant coldlength ÷ MonsterColdDivisor.
    let chill_frames = if hit.element == DamageType::Cold && m.cold_len_frames > 0 && !elem_immune {
        chill_frames(m.cold_len_frames, hit.difficulty)
    } else {
        0
    };

    CombatResult {
        total: physical.saturating_add(elemental).saturating_add(crushing),
        physical,
        elemental,
        deadly,
        crushing,
        wounds,
        poison,
        chill_frames,
        life_leech,
        mana_leech,
    }
}

/// Open wounds : un [`Dot`] 0,25 × clvl/s (tick 1 s, `clvl/4` arrondi bas,
/// min 1) pendant l'interpolation linéaire 8 → 200 frames sur clvl 1 → 99.
pub fn open_wound(clvl: u32) -> Dot {
    let clvl = clvl.max(1).min(99);
    let frames = OW_FRAMES_MIN
        + (((OW_FRAMES_MAX - OW_FRAMES_MIN) as u64 * (clvl - 1) as u64 + 49) / 98) as u32;
    let dmg_per_tick = ((clvl * OW_DMG_PER_SEC_NUM) / OW_DMG_PER_SEC_DEN).max(1);
    Dot { dmg_per_tick, remaining_ticks: (frames / DOT_TICK_FRAMES).max(1), acc_ms: 0 }
}

/// Poison : `total` dégâts étalés sur `elen/25` ticks de 1 s (part entière ;
/// total inférieur au nombre de ticks ⇒ un seul tick — arrondis entiers
/// documentés). `elen` en frames (`patch_skills.txt` ELen).
pub fn poison_dot(total: u32, elen_frames: u32) -> Option<Dot> {
    if total == 0 || elen_frames == 0 {
        return None;
    }
    let ticks = (elen_frames / DOT_TICK_FRAMES).max(1);
    if total < ticks {
        return Some(Dot { dmg_per_tick: total, remaining_ticks: 1, acc_ms: 0 });
    }
    Some(Dot { dmg_per_tick: total / ticks, remaining_ticks: ticks, acc_ms: 0 })
}

/// Froid : `cold_len_frames ÷ MonsterColdDivisor` (1/2/4 selon la difficulté,
/// `patch_difficultylevels.txt` lignes 2-4) — la vitesse est ×1/2 côté
/// moteur tant que la durée reste > 0.
pub fn chill_frames(cold_len_frames: u32, d: Difficulty) -> u32 {
    cold_len_frames / difficulty::embedded().row(d).monster_cold_divisor
}

/// Leech brut : `pct × dmg / 100 / diviseur` (part entière).
fn leech_raw(dmg: u32, pct: u32, divisor: u32) -> u32 {
    (dmg as u64 * pct as u64 / 100 / divisor.max(1) as u64) as u32
}

/// Leech plafonné au manque de vie/mana (jamais de sur-soin).
pub fn cap_leech(raw: u32, missing: u32) -> u32 {
    raw.min(missing)
}

/// Résist HÉROS effective : `res + ResistPenalty` (0/−40/−100), plafonnée à
/// 75 + `max_res` (`maxfireresist`… id 38/40/42/44/46, bonus lui-même
/// plafonné à +25) sans dépasser 90, plancher −100 (Hell).
pub fn hero_resist_effective(res: i32, max_res: i32, d: Difficulty) -> i32 {
    let penalty = difficulty::embedded().row(d).resist_penalty;
    let cap = (RES_CAP_BASE + max_res.clamp(0, RES_MAX_BONUS)).min(RES_CAP_MAX);
    (res + penalty).clamp(-100, cap.min(RES_CAP_MAX))
}

// ---------------------------------------------------------------------------
// Tests golden — chaque assertion cite sa source réelle fichier:ligne
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Coup d'arme pur, victime sans résist, aucun mod.
    fn bare_hit(physical: u32) -> CombatHit {
        CombatHit {
            physical,
            elemental: 0,
            element: DamageType::Physical,
            mods: HitMods::default(),
            clvl: 1,
            difficulty: Difficulty::Normal,
            victim: Victim { kind: VictimKind::Normal, cur_hp: 1000, res: [0; 6] },
            wearer_life: 55,
            wearer_max_life: 55,
            wearer_mana: 15,
            wearer_max_mana: 15,
            melee: true,
            roll_deadly: 0,
            roll_wounds: 0,
            roll_crushing: 0,
        }
    }

    #[test]
    fn deadly_strike_golden() {
        // Doc non fiable (convention 1.10+) : roll < pct ⇒ arme physique ×2.
        let mut h = bare_hit(100);
        h.mods.deadly_strike_pct = 100; // 100 % : tout roll < 100 proc
        let out = apply_combat_mods(&h);
        assert!(out.deadly);
        assert_eq!(out.physical, 200);
        assert_eq!(out.total, 200);
        // roll 100 ≥ 100 : pas de proc (chance = pct).
        h.roll_deadly = 100;
        assert!(!apply_combat_mods(&h).deadly);
        // JAMAIS sur l'élémentaire/magique : la portion feu n'est pas doublée.
        h.roll_deadly = 0;
        h.element = DamageType::Fire;
        h.elemental = 50;
        let out = apply_combat_mods(&h);
        assert!(!out.deadly);
        assert_eq!(out.elemental, 50, "élémentaire jamais ×2");
    }

    #[test]
    fn crushing_blow_golden() {
        // Doc non fiable (1.10+) : 1/8 normal, 1/16 champion/unique, 1/32 joueur.
        let mut h = bare_hit(0);
        h.mods.crushing_blow_pct = 100;
        h.victim.cur_hp = 800;
        // 800/8 = 100 (monstre normal).
        assert_eq!(apply_combat_mods(&h).crushing, 100);
        // 800/16 = 50 (champion/unique/super-unique/boss).
        h.victim.kind = VictimKind::ChampionUnique;
        assert_eq!(apply_combat_mods(&h).crushing, 50);
        // 800/32 = 25 (joueur — N/A solo, API complète).
        h.victim.kind = VictimKind::Player;
        assert_eq!(apply_combat_mods(&h).crushing, 25);
        // Part entière : 801/16 = 50 (50,06).
        h.victim.kind = VictimKind::ChampionUnique;
        h.victim.cur_hp = 801;
        assert_eq!(apply_combat_mods(&h).crushing, 50);
        // Chance non proc : roll ≥ pct ⇒ 0.
        h.mods.crushing_blow_pct = 50;
        h.roll_crushing = 50;
        assert_eq!(apply_combat_mods(&h).crushing, 0);
        // Insensible aux résists : immunité feu mais CB passe sur un missile feu.
        h.victim.res = [0, 0, 100, 0, 0, 0];
        h.element = DamageType::Fire;
        h.elemental = 100;
        h.roll_crushing = 0;
        h.mods.crushing_blow_pct = 100;
        let out = apply_combat_mods(&h);
        assert_eq!(out.elemental, 0, "immunité feu : type sauté");
        assert_eq!(out.crushing, 50, "CB ignore les résists");
        assert_eq!(out.total, 50);
    }

    #[test]
    fn open_wounds_golden() {
        // Doc non fiable : 0,25 × clvl/s, durée 8..200 frames selon clvl.
        // clvl 1 : 8 frames → 1 tick, 0,25 ⇒ 1 dégât/tick.
        let ow = open_wound(1);
        assert_eq!((ow.dmg_per_tick, ow.remaining_ticks), (1, 1));
        // clvl 50 : 8 + 192×49/98 = 104 frames → 4 ticks ; 50/4 = 12 dmg/s.
        let ow = open_wound(50);
        assert_eq!((ow.dmg_per_tick, ow.remaining_ticks), (12, 4));
        // clvl 99 : 200 frames → 8 ticks ; 99/4 = 24 dmg/tick.
        let ow = open_wound(99);
        assert_eq!((ow.dmg_per_tick, ow.remaining_ticks), (24, 8));
        // Borne : clvl 200 clampe à 99 (jamais > 200 frames).
        assert_eq!(open_wound(200).remaining_ticks, 8);
        // Proc mêlée : chance % + jet.
        let mut h = bare_hit(10);
        h.clvl = 50;
        h.mods.open_wounds_pct = 100;
        let out = apply_combat_mods(&h);
        assert_eq!(out.wounds, Some(open_wound(50)));
        h.roll_wounds = 100; // 100 ≥ 100 : pas de proc
        assert!(apply_combat_mods(&h).wounds.is_none());
        // Distance : JAMAIS de plaie (mêlée seule).
        h.roll_wounds = 0;
        h.melee = false;
        assert!(apply_combat_mods(&h).wounds.is_none());
        // tick() : 1 s de sim = 1 tick de dégâts.
        let mut d = open_wound(50);
        assert_eq!(d.tick(Dot::TICK_MS), 12);
        assert_eq!(d.remaining_ticks, 3);
        assert_eq!(d.tick(500), 0, "sous-tick");
        assert_eq!(d.tick(500), 12);
    }

    #[test]
    fn leech_golden() {
        // Divisors RÉELS `patch_difficultylevels.txt` lignes 2-4 :
        // LifeStealDivisor/ManaStealDivisor 1/2/3 (Normal/Nightmare/Hell).
        // 5 % de 200 phys = 10 → 10 / 5 / 3 (part entière de 10/3).
        // FIXTURE : 10 PV/mana manquants — le leech est plafonné au manque
        // (« plafonnés »), un porteur pleine vie vole 0 (assert dédié plus bas).
        let mut h = bare_hit(200);
        h.wearer_life = 45;
        h.wearer_mana = 5;
        h.mods.life_leech_pct = 5;
        h.mods.mana_leech_pct = 5;
        assert_eq!(apply_combat_mods(&h).life_leech, 10);
        h.difficulty = Difficulty::Nightmare;
        assert_eq!(apply_combat_mods(&h).life_leech, 5);
        h.difficulty = Difficulty::Hell;
        assert_eq!(apply_combat_mods(&h).mana_leech, 3);
        // Leech sur les dégâts FINAUX : résist ResDm 50 ⇒ base 100.
        h.difficulty = Difficulty::Normal;
        h.victim.res[0] = 50;
        assert_eq!(apply_combat_mods(&h).life_leech, 5);
        // Plafonné au manque (pas de sur-soin) : plein ⇒ 0.
        h.victim.res[0] = 0;
        h.wearer_life = h.wearer_max_life;
        h.wearer_mana = h.wearer_max_mana;
        let out = apply_combat_mods(&h);
        assert_eq!((out.life_leech, out.mana_leech), (0, 0));
        // Manque partiel : 3 PV manquants ⇒ gain 3 (10 brut).
        h.wearer_life = 52;
        assert_eq!(apply_combat_mods(&h).life_leech, 3);
        // Leech sur l'arme seule : un missile purement élémentaire ne vole rien.
        let mut m = bare_hit(0);
        m.melee = false;
        m.element = DamageType::Fire;
        m.elemental = 500;
        m.mods.life_leech_pct = 10;
        assert_eq!(apply_combat_mods(&m).life_leech, 0);
    }

    #[test]
    fn resists_hero_golden() {
        // Pénalité RÉELLE `patch_difficultylevels.txt` lignes 2-4 :
        // ResistPenalty 0/−40/−100.
        assert_eq!(hero_resist_effective(50, 0, Difficulty::Normal), 50);
        assert_eq!(hero_resist_effective(50, 0, Difficulty::Nightmare), 10);
        assert_eq!(hero_resist_effective(50, 0, Difficulty::Hell), -50);
        // Plafond 75 de base (retail), +maxres jusqu'à 90.
        assert_eq!(hero_resist_effective(80, 0, Difficulty::Normal), 75);
        assert_eq!(hero_resist_effective(150, 25, Difficulty::Normal), 90);
        // maxres ne dépasse jamais le plafond absolu 90 (ids 38/40/42/44/46).
        assert_eq!(hero_resist_effective(150, 60, Difficulty::Normal), 90);
        // Plancher −100 (pénalité Hell pleine).
        assert_eq!(hero_resist_effective(-40, 0, Difficulty::Hell), -100);
    }

    #[test]
    fn immunities_monstre_golden() {
        // ANCRE RÉELLE A1 : `patch_monstats.txt` ligne 3 `skeleton2`
        // (Returned, TC1(H) `Act 1 (H) H2H B`) — ResPo(N)=75, ResPo(H)=100
        // (colonnes $141/$147) ⇒ immunisé poison en Enfer UNIQUEMENT.
        let mut h = bare_hit(0);
        h.element = DamageType::Poison;
        h.elemental = 100;
        h.mods.elen_frames = 50; // Poison Nova (ELen=50, ligne 94)
        // Normal : ResPo 75 → pas immunité ; avec ELen le poison est
        // CONVERTI en DoT : total résisté 25 étalé (élémentaire instantané 0).
        h.victim.res[Difficulty::resist_index(DamageType::Poison)] = 75;
        h.difficulty = Difficulty::Normal;
        let out = apply_combat_mods(&h);
        assert_eq!(out.elemental, 0, "poison à durée : rien d'instantané");
        assert_eq!(out.poison, poison_dot(25, 50), "25 résistés étalés sur 2 ticks");
        // Hell : ResPo 100 → IMMUNITÉ : le type est sauté (0 dégât, pas de DoT).
        h.victim.res[Difficulty::resist_index(DamageType::Poison)] = 100;
        h.difficulty = Difficulty::Hell;
        let out = apply_combat_mods(&h);
        assert_eq!(out.elemental, 0);
        assert!(out.poison.is_none());
        assert_eq!(out.total, 0);
        // Sans ELen (mods par défaut) : le poison reste instantané résisté.
        h.mods.elen_frames = 0;
        assert_eq!(apply_combat_mods(&h).elemental, 0);
        h.victim.res[Difficulty::resist_index(DamageType::Poison)] = 50;
        assert_eq!(apply_combat_mods(&h).elemental, 50);
    }

    #[test]
    fn poison_dot_golden() {
        // Durées RÉELLES `patch_skills.txt` : `Poison Nova` ligne 94
        // ELen=50 (2 s à 25 fps), `Poison Javelin` ligne 17 ELen=200 (8 s).
        // 100 dégâts / ELen 50 → 2 ticks de 50.
        let d = poison_dot(100, 50).unwrap();
        assert_eq!((d.dmg_per_tick, d.remaining_ticks), (50, 2));
        // 100 dégâts / ELen 200 → 8 ticks de 12 (part entière de 12,5).
        let d = poison_dot(100, 200).unwrap();
        assert_eq!((d.dmg_per_tick, d.remaining_ticks), (12, 8));
        // Total < ticks ⇒ un seul tick (arrondi entiers documenté).
        let d = poison_dot(3, 200).unwrap();
        assert_eq!((d.dmg_per_tick, d.remaining_ticks), (3, 1));
        // tick() : un pas de 2,5 s consomme 2 ticks puis 1.
        let mut d = poison_dot(100, 50).unwrap();
        assert_eq!(d.tick(2500), 100);
        assert_eq!(d.remaining_ticks, 0);
        assert_eq!(d.tick(1000), 0);
        // Pas de poison sans dégâts ni durée.
        assert!(poison_dot(0, 50).is_none());
        assert!(poison_dot(100, 0).is_none());
    }

    #[test]
    fn cold_chill_golden() {
        // MonsterColdDivisor RÉEL `patch_difficultylevels.txt` lignes 2-4 :
        // 1/2/4 — coldlength 100 frames → 100/50/25 frames.
        assert_eq!(chill_frames(100, Difficulty::Normal), 100);
        assert_eq!(chill_frames(100, Difficulty::Nightmare), 50);
        assert_eq!(chill_frames(100, Difficulty::Hell), 25);
        // Vitesse ×1/2 (constantes du module).
        let mut h = bare_hit(0);
        h.element = DamageType::Cold;
        h.elemental = 30;
        h.mods.cold_len_frames = 100;
        let out = apply_combat_mods(&h);
        assert_eq!(out.chill_frames, 100);
        // Immunisé froid (zombie1 ResCo(H)=120, ligne 8) : pas de refroidissement.
        h.victim.res[Difficulty::resist_index(DamageType::Cold)] = 120;
        h.difficulty = Difficulty::Hell;
        assert_eq!(apply_combat_mods(&h).chill_frames, 0);
        assert_eq!(apply_combat_mods(&h).elemental, 0, "immunité : 0 dégât froid");
    }

    #[test]
    fn sans_mods_arithmetique_historique_inchange() {
        // Garde-fou de branchement : sans mods, le résultat EST l'ancien
        // `apply_resist` (mêlée ResDm / missile res[elem]) — les 708 tests
        // existants doivent rester verts sans décalage.
        let mut h = bare_hit(100);
        h.victim.res[0] = 50;
        assert_eq!(apply_combat_mods(&h).physical, difficulty::apply_resist(100, 50));
        h.element = DamageType::Lightning;
        h.elemental = 80;
        h.physical = 0;
        h.victim.res[Difficulty::resist_index(DamageType::Lightning)] = -50; // Hell amplifié
        assert_eq!(apply_combat_mods(&h).elemental, difficulty::apply_resist(80, -50));
    }

    #[test]
    fn determinisme() {
        // Pur & déterministe : mêmes entrées ⇒ même sortie (aucun RNG interne).
        let mut h = bare_hit(120);
        h.mods = HitMods {
            deadly_strike_pct: 50,
            crushing_blow_pct: 40,
            open_wounds_pct: 30,
            life_leech_pct: 6,
            mana_leech_pct: 4,
            elen_frames: 50,
            cold_len_frames: 75,
        };
        h.elemental = 45;
        h.element = DamageType::Cold;
        h.clvl = 40;
        let a = apply_combat_mods(&h);
        let b = apply_combat_mods(&h);
        assert_eq!(a, b);
        // Le résultat est reproductible valeur par valeur.
        assert_eq!(a.total, a.physical + a.elemental + a.crushing);
        assert!(a.wounds.is_some() && a.poison.is_none() && a.chill_frames > 0);
    }

    #[test]
    fn victim_kind_depuis_mon_kind() {
        use crate::world::MonKind;
        // 4 rangs « élite » (champion/unique/super-unique/boss) → 1/16.
        assert_eq!(VictimKind::from_mon_kind(&MonKind::Normal), VictimKind::Normal);
        for k in [MonKind::Champion, MonKind::Unique, MonKind::SuperUnique, MonKind::Boss] {
            assert_eq!(VictimKind::from_mon_kind(&k), VictimKind::ChampionUnique);
        }
        assert_eq!(VictimKind::Normal.crushing_fraction(), CB_NORMAL);
    }

    // --- Branchement World (additif) --------------------------------------

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Premier coup porté (mêlée) contre un Fallen spawné à côté du héros.
    fn first_melee_hit(deadly_pct: u32) -> u32 {
        let mut w = crate::World::with_seed(
            open_grid(56, 40),
            serde_json::from_str(MANIFEST_JSON).unwrap(),
            7,
        );
        w.clear_monsters();
        let (hx, hy) = w.hero_spawn();
        let (hx, hy) = (hx.floor() as i32, hy.floor() as i32);
        let spot = [(hx + 2, hy), (hx - 2, hy), (hx, hy + 2), (hx, hy - 2)]
            .into_iter()
            .find(|(x, y)| w.is_walkable(*x, *y) && w.spawn_monster("FA", *x, *y))
            .expect("spawn Fallen à côté du héros");
        let _ = spot;
        if deadly_pct > 0 {
            w.set_hero_combat_mods(HitMods { deadly_strike_pct: deadly_pct, ..Default::default() });
        }
        // Attaque le monstre chaque tick jusqu'au premier coup porté. Les
        // deux mondes partagent la graine : historique de RNG identique
        // jusqu'au coup (les jets de mod n'existent qu'avec des mods).
        for _ in 0..200 {
            let t = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
            let ev = w.tick(50, t.map(|(x, y)| crate::PlayerInput::Attack { x, y }));
            if let Some(crate::GameEvent::HeroAttacked { dmg, .. }) =
                ev.iter().find(|e| matches!(e, crate::GameEvent::HeroAttacked { .. }))
            {
                return *dmg;
            }
        }
        panic!("aucun coup porté en 200 ticks");
    }

    #[test]
    fn world_branchement_deadly_strike_x2() {
        // Même graine, même monstre : le coup de base D (sans mods) devient
        // 2×D avec deadly strike 100 % — le branchement additif de
        // `world.rs::hero_attack` passe bien par `apply_combat_mods`.
        let base = first_melee_hit(0);
        let crit = first_melee_hit(100);
        assert_eq!(crit, base * 2, "deadly strike ×2 appliqué dans World");
    }
}
