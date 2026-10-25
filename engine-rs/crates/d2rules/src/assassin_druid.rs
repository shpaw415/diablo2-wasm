//! Étape 16c — comportements des 210 skills : **Assassine** (pièges + combos
//! charge-up) et **Druide** (transformations), port data-driven v1.12 du
//! PLAN-FIDELITE (phase 16).
//!
//! Module PUR (aucune IO, aucun rendu) : l'état vit dans [`AdState`] porté par
//! `World` (champ additif, TRANSITOIRE — jamais persisté, comme `ai`), la
//! logique est appelée par `world.rs` en blocs additifs et les événements
//! sortent par [`World::drain_ad_events`] (pattern `pending_events`).
//!
//! # 1. Pièges Assassine — la vraie règle : TIR PÉRIODIQUE
//! Les 6 pièges `pettype=assassintrap` (`patch_skills.txt` col `summon`) sont
//! des ENTITÉS STATIQUES qui tirent périodiquement, pas des munitions posées :
//! * Période = `aidel` des monstres pièges (`patch_monstats.txt` lignes
//!   413-419 : `wakeofdestruction`, `chargeboltsentry`, `lightningsentry`,
//!   `bladecreeper`, `infernosentry`, `deathsentry` — TOUS `aidel=15` frames)
//!   ⇒ [`SENTRY_FIRE_PERIOD_MS`] = 15 × 40 ms = 600 ms.
//! * Munitions : « Shots fired » `Param1` — `Charged Bolt Sentry` L263
//!   `calc4 = par1 + skill('Lightning Sentry'.blvl)/4` (division entière D2,
//!   les autres = `par1`) ; `Blade Sentinel` L259 est limité par SA DURÉE
//!   `calc4 = ln12` (« duration ») = 100 + 12×(niv−1) frames.
//! * Plafond simultané : `petmax=5` (col `petmax`, les 6 lignes) — la 6e pose
//!   remplace la PLUS ANCIENNE.
//! * Dégâts par niveau : courbes officielles `EMin/EMax` + paliers
//!   (`EMinLev/EMaxLev1-5`) × synergie `EDmgSymPerCalc` évaluée via
//!   [`crate::calc`] (ex. `Lightning Sentry` L273 :
//!   `(Shock Field + Charged Bolt Sentry + Death Sentry) × par8`, par8=12).
//! * Portée d'acquisition : [`crate::world::AGGRO_RADIUS`] (6 tuiles, rayon
//!   d'aggro du moteur ; la portée réelle « écran » est hors port MVP).
//! * `Shock Field` (« Shock Web ») L258 N'EST PAS un piège : `srvdofunc=43`,
//!   un jet de `par1 + lvl/par2 + skill('Fire Trauma'.blvl)/3` carreaux
//!   (`prgcalc1` verbatim) — traité comme une volée immédiate ([`cast`]).
//! * `Death Sentry` L278 : l'explosion de cadavre est HORS PORT MVP (aucune
//!   consommation de cadavre) — ses carreaux de foudre restent fonctionnels.
//!
//! # 2. Combos Assassine (charge-up → finisher)
//! Machine à états : N coups RÉUSSIS (1 par coup, plafond 3) chargent un
//! skill `progressive=1` (col 184) ; un coup de finisher (`finishing=1`,
//! col 182) LIBÈRE les charges. Les charges de PLUSIEURS charge-ups
//! coexistent (états `progressive_*` indépendants du retail) et sont toutes
//! libérées par le finisher. Durée des charges : `auralencalc` = `par3` = 225
//! frames (les 5) / `225` littéral (`Royal Strike` L282) ⇒ 9 s, relancé à
//! chaque charge. Bonus de toucher par charge : `par4` = 50
//! (`progressive_tohit`).
//! * `Tiger Strike` L256 : bonus PHYSIQUE `calc1=ln12` = 100+20×(niv−1) %
//!   PAR charge, appliqué aux dégâts d'ARME du finisher (via
//!   [`scaled_hero_melee`], branché additif dans `hero_attack`).
//! * `Fists of Fire` L261 : feu `EMin/EMax` × charges ; rayon d'explosion
//!   `par1`=4 (`prgcalc2`), anneau de mur de feu `par2`=4 (`prgcalc3`) au
//!   3e charge.
//! * `Claws of Thunder` L271 : foudre × charges ; 1 charge = bolt (cible),
//!   2 = nova (AoE — rayon absent des tables v1.12, constante MVP
//!   [`COT_NOVA_RADIUS_MVP`]), 3 = nova + bolt.
//! * `Blades of Ice` L276 : froid × charges ; rayons `par1`=6 (2e charge) et
//!   `par2`=3 (3e), gel `ELen`=100 + 10/niveau (chill ×1/2 vitesse).
//! * `Cobra Strike` L267 : vol de vie ET mana `par1`=40 + 5×(niv−1) % PAR
//!   charge des dégâts physiques du finisher (diviseur de difficulté).
//! * `Royal Strike` L282 (Phoenix Strike) : charge 1 = météor feu rayon
//!   `par2`=6, charge 2 = chaîne de foudre, charge 3 = 16 (`par5`) éclats de
//!   glace — dégâts PAR CHARGE portés par les missiles réels
//!   (`patch_missiles.txt` L565 `royalstrikemeteor` `EMin=20` feu,
//!   L570 `royalstrikechainlightning` `EMin=1` foudre,
//!   L571 `royalstrikechaosice` `EMin=16` froid + `ELen=100`).
//! * Finishers : `Dragon Talon` L257, `Dragon Claw` L262, `Dragon Tail`
//!   L272, `Dragon Flight` L277 — tous marqués `finishing=1`.
//!
//! # 3. Transformations Druide
//! `Wearwolf` L225 / `Wearbear` L230 (`srvdofunc=116`, `aurastate=wolf|bear`)
//! : buff temporisé, re-cast = retour humain (règle D2), formes exclusives.
//! Durée `auralencalc = 1000 + skill('Shape Shifting'.ln12)` frames × 40 ms —
//! `Shape Shifting` L226 (« Lycanthropy ») : `Param1`=1000 base +
//! `Param2`=500/niveau de durée, `Param3/4` = 20+5/niveau % de vie (`ln34`,
//! aussi consommé par les formes). Effets par forme (paires
//! `aurastatN`/`aurastatcalcN` portées verbatim) :
//! * Loup : vitesse d'ATTAQUE `aurastat2=attackrate` `dm34` = 10+80×(niv−1) %
//!   (appliquée au swing, clamp MVP documenté) ; vie max
//!   `aurastat4=item_maxhp_percent` `par2 + skill('Shape Shifting'.ln34)`
//!   = 25 + 20+5×… %.
//! * Ours : dégâts mêlée `aurastat1=damagepercent` `ln12` = 50+7×(niv−1) %,
//!   défense `aurastat2=skill_armor_percent` `ln34` = 25+5×(niv−1) %, vie max
//!   `aurastat3=item_maxhp_percent` `par5 + skill('Shape Shifting'.ln34)`
//!   = 50 + 20+5×… %, KNOCKBACK des coups de mêlée (règle Werebear du
//!   retail — repoussée d'1 tuile, MVP). PAS d'accélération d'attaque (aucune
//!   stat `attackrate` sur L230 — l'ours frappe moins vite, plus fort).
//! * Le bonus de vie est appliqué en FLAT sur `max_life` à la transformation
//!   et retiré EXACTEMENT à l'expiration (compteur `life_bonus_applied`).
//!
//! # 4. Hors scope (brief)
//! Vampires/monstres transformés : hors port. `Fire Trauma` L253 (grenade)
//! et les skills monstres : non traités ici.
//!
//! # 5. Sprite (hook étape 15)
//! Aucun COF loup/ours n'existe dans `assets/dcc/` (tokens présents : AI, AM,
//! AN, BA, DZ, FA, FS, NE, PA, SI, SK, SO, YE, ZM — pas de druide loup/ours).
//! [`AdForm::token`] expose le jeton monstre réel (`patch_monstats.txt`
//! L433 `wolf` Code=40 / L434 `bear` Code=TG) pour le futur swap ; le rendu
//! reste À CÂBLER par l'étape 15 (hook documenté, rien n'est rendu ici).
//!
//! # Déterminisme
//! Tous les jets (toucher, variance de dégâts) passent par le RNG du monde
//! (`World::rng`) — même graine ⇒ mêmes événements, tick par tick.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::combat_mods::{self, CombatHit, Victim, VictimKind};
use crate::skills_hero::{self, HeroLevels};
use crate::skills_tree::{ElemType, SkillRecord, SkillTree, SkillTreeData, SKILLS_TXT};
use crate::world::{dist, hit_chance, MonState, World, AGGRO_RADIUS, HERO_SWING_MS};
use crate::AnimState;
use d2missiles::DamageType;

// ---------------------------------------------------------------------------
// Constantes réelles (preuves en en-tête de module)
// ---------------------------------------------------------------------------

/// Période de tir des pièges : `aidel`=15 frames des 6 monstres pièges
/// (`patch_monstats.txt` L413-419) × 40 ms (25 fps v1.12).
pub const SENTRY_FIRE_PERIOD_MS: u32 = 15 * combat_mods::FRAME_MS_D2;
/// Frame D2 réutilisée (40 ms — `combat_mods::FRAME_MS_D2`).
pub const FRAME_MS: u32 = combat_mods::FRAME_MS_D2;
/// Plafond de pièges simultanés : col `petmax=5` (les 6 lignes de pièges).
pub const MAX_TRAPS: usize = 5;
/// Charges max d'un charge-up (plafond retail : 3 orbes).
pub const MAX_CHARGES: u32 = 3;
/// Charge-up : durée des charges = `par3`=225 frames (les 5) / `225`
/// littéral (`Royal Strike` L282) — 9 s.
pub const COMBO_DURATION_FRAMES: i64 = 225;
/// Bonus de toucher PAR charge (`progressive_tohit`, `par4`=50 des 6 lignes).
pub const COMBO_TOHIT_PER_CHARGE: u32 = 50;
/// Nova `Claws of Thunder` : rayon absent des tables v1.12 — constante MVP
/// documentée (2 tuiles), à reconfirmer côté DLL.
pub const COT_NOVA_RADIUS_MVP: f32 = 2.0;
/// Knockback Werebear : 1 tuile (MVP — règle retail « l'attaque repousse »).
pub const BEAR_KNOCKBACK_TILES: f32 = 1.0;
/// Clamp MVP de la vitesse d'attaque transformée (swing ≥ moitié du normal).
pub const IAS_SWING_CAP_DIV: u32 = 2;

/// Les 6 charge-ups réels v1.12 (col `progressive=1`, L256/261/267/271/276/282).
const CHARGE_UPS: [&str; 6] =
    ["Tiger Strike", "Fists of Fire", "Cobra Strike", "Claws of Thunder", "Blades of Ice", "Royal Strike"];

/// Les 6 pièges réels v1.12 (col `pettype=assassintrap`, L259/263/264/273/274/278).
const TRAPS: [&str; 6] = [
    "Blade Sentinel",
    "Charged Bolt Sentry",
    "Wake of Fire Sentry",
    "Lightning Sentry",
    "Inferno Sentry",
    "Death Sentry",
];

// ---------------------------------------------------------------------------
// Colonnes brutes absentes de `SkillRecord` (progressive/finishing/summon/
// petmax/auralencalc/aurastat) — mini-parseur du TSV réel, caché.
// ---------------------------------------------------------------------------

/// Colonnes v1.12 dont ce module a besoin et que `SkillRecord` ne porte pas.
#[derive(Debug, Clone, Default)]
pub struct RawCols {
    /// Col `progressive` (184) — skill charge-up.
    pub progressive: bool,
    /// Col `finishing` (182) — skill finisher.
    pub finishing: bool,
    /// Col `summon` (61) — code monstre du piège.
    pub summon: Option<String>,
    /// Col `petmax` (63) — plafond simultané.
    pub petmax: u32,
    /// Col `auralencalc` (25) — formule de durée (frames).
    pub auralencalc: Option<String>,
    /// Cols `aurastat1-4` (27/29/31/33) — NOMS des stats (ex. `attackrate`).
    pub aurastats: [Option<String>; 4],
    /// Cols `aurastatcalc1-4` (28/30/32/34) — formules des effets.
    pub aurastatcalcs: [Option<String>; 4],
    /// Col `aurastate` (23) — `wolf` / `bear` / `progressive_*`.
    pub aurastate: String,
}

impl RawCols {
    /// Formule de la stat d'aura `name` (ex. `attackrate`, `item_maxhp_percent`).
    fn stat_calc(&self, name: &str) -> Option<&str> {
        self.aurastats
            .iter()
            .position(|s| s.as_deref() == Some(name))
            .and_then(|i| self.aurastatcalcs[i].as_deref())
    }
}

fn raw_table() -> &'static HashMap<String, RawCols> {
    static RAW: OnceLock<HashMap<String, RawCols>> = OnceLock::new();
    RAW.get_or_init(|| {
        let mut rows =
            SKILLS_TXT.lines().filter(|l| !l.trim().is_empty()).map(crate::stats::split_tsv);
        let Some(hdr) = rows.next() else { return HashMap::new() };
        let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
        let (c_code, c_prog, c_fin) = (col("skill"), col("progressive"), col("finishing"));
        let (c_sum, c_pet) = (col("summon"), col("petmax"));
        let (c_alen, c_astate) = (col("auralencalc"), col("aurastate"));
        let c_asts: Vec<Option<usize>> =
            ["aurastat1", "aurastat2", "aurastat3", "aurastat4"].iter().map(|c| col(c)).collect();
        let c_acalcs: Vec<Option<usize>> =
            ["aurastatcalc1", "aurastatcalc2", "aurastatcalc3", "aurastatcalc4"]
                .iter()
                .map(|c| col(c))
                .collect();
        let cell = |r: &[String], c: Option<usize>| -> String {
            c.and_then(|c| r.get(c)).map(|s| s.trim().to_string()).unwrap_or_default()
        };
        let opt_cell = |r: &[String], c: Option<usize>| -> Option<String> {
            let s = cell(r, c);
            (!s.is_empty()).then_some(s)
        };
        let mut map = HashMap::new();
        for r in rows {
            let code = cell(&r, c_code);
            if code.is_empty() || code.starts_with('*') {
                continue;
            }
            map.insert(
                code.to_ascii_lowercase(),
                RawCols {
                    progressive: cell(&r, c_prog) == "1",
                    finishing: cell(&r, c_fin) == "1",
                    summon: opt_cell(&r, c_sum),
                    petmax: cell(&r, c_pet).parse().unwrap_or(0),
                    auralencalc: opt_cell(&r, c_alen),
                    aurastats: (0..4)
                        .map(|i| opt_cell(&r, c_asts[i]))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                    aurastatcalcs: (0..4)
                        .map(|i| opt_cell(&r, c_acalcs[i]))
                        .collect::<Vec<_>>()
                        .try_into()
                        .unwrap(),
                    aurastate: cell(&r, c_astate),
                },
            );
        }
        map
    })
}

/// Colonnes brutes d'un skill (insensible à la casse).
pub fn raw_cols(code: &str) -> Option<&'static RawCols> {
    raw_table().get(&code.to_ascii_lowercase())
}

/// Numéro de ligne (1-based) d'un code dans `patch_skills.txt` — preuve des
/// golden tests (même helper que `skills_tree::tests`).
pub fn line_of(code: &str) -> usize {
    SKILLS_TXT
        .lines()
        .enumerate()
        .find(|(_, l)| crate::stats::split_tsv(l).first().map(|c| c == code).unwrap_or(false))
        .map(|(i, _)| i + 1)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Types publics (état + événements + API)
// ---------------------------------------------------------------------------

/// Forme druide courante (jeton monstre réel pour le hook sprite étape 15).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdForm {
    /// `Wearwolf` L225 — jeton `wolf` (`patch_monstats.txt` L433, Code=40).
    Wolf,
    /// `Wearbear` L230 — jeton `bear` (`patch_monstats.txt` L434, Code=TG).
    Bear,
}

impl AdForm {
    /// Code `skills.txt` de la forme.
    pub fn skill(self) -> &'static str {
        match self {
            AdForm::Wolf => "Wearwolf",
            AdForm::Bear => "Wearbear",
        }
    }
    /// Jeton monstre réel (hook sprite étape 15 — aucun COF dans assets/dcc).
    pub fn token(self) -> &'static str {
        match self {
            AdForm::Wolf => "wolf",
            AdForm::Bear => "bear",
        }
    }
    /// Forme depuis le code table (insensible à la casse).
    fn from_code(code: &str) -> Option<AdForm> {
        match code.to_ascii_lowercase().as_str() {
            "wearwolf" => Some(AdForm::Wolf),
            "wearbear" => Some(AdForm::Bear),
            _ => None,
        }
    }
}

/// Un piège posé (entité statique — tir périodique, cf. en-tête §1).
#[derive(Debug, Clone)]
pub struct TrapRec {
    /// Code `skills.txt` (ex. `"Lightning Sentry"`).
    pub skill: &'static str,
    /// Code `summon` du monstre piège (ex. `lightningsentry`).
    pub summon: String,
    /// Niveau du skill À LA POSE (dégâts/munitions figés — règle D2).
    pub level: u32,
    /// Position (tuiles continues).
    pub x: f32,
    pub y: f32,
    /// Munitions restantes (« Shots fired » ; u32::MAX = limité par durée).
    pub shots_left: u32,
    /// Compte à rebours avant le prochain tir (ms).
    pub fire_cd_ms: u32,
    /// Durée restante (ms ; u32::MAX = sans durée — munitions seules).
    pub expire_ms: u32,
}

/// Une pile de charges d'un charge-up (plusieurs coexistent — états
/// `progressive_*` indépendants du retail).
#[derive(Debug, Clone)]
pub struct ComboState {
    /// Code `skills.txt` du charge-up.
    pub skill: &'static str,
    /// Charges 1..=3.
    pub charges: u32,
    /// Durée restante avant dissipation (ms, relancée à chaque charge).
    pub ttl_ms: u32,
}

/// Transformation active (buff temporisé).
#[derive(Debug, Clone)]
pub struct AdFormState {
    /// Forme courante.
    pub form: AdForm,
    /// Niveau du skill de forme À LA TRANSFORMATION.
    pub level: u32,
    /// Durée restante (ms).
    pub ttl_ms: u32,
    /// Bonus de vie max APPLIQUÉ en flat (retiré exactement à la fin).
    pub life_bonus_applied: u32,
}

/// État additif du monde (transitoire — jamais persisté, comme `ai`).
#[derive(Debug, Clone, Default)]
pub struct AdState {
    /// Pièges posés (ordre de pose — la 6e pose remplace la 1re).
    pub traps: Vec<TrapRec>,
    /// Piles de charges actives (une par charge-up chargé).
    pub combos: Vec<ComboState>,
    /// Transformation active (`None` = humain).
    pub form: Option<AdFormState>,
    /// Événements du module — drainés par [`World::drain_ad_events`].
    pub events: Vec<AdEvent>,
}

/// Événements du module (contrat UI/d2game — drain par tick).
#[derive(Debug, Clone, PartialEq)]
pub enum AdEvent {
    /// Piège posé en `(x, y)` (tuile).
    TrapPlaced { skill: &'static str, x: i32, y: i32 },
    /// Piège parti (durée écoulée ou munitions épuisées).
    TrapExpired { skill: &'static str },
    /// Le piège a tiré sur `target` (dégâts `dmg` après résist, `elem`).
    TrapFired { skill: &'static str, target: usize, dmg: u32, elem: DamageType },
    /// Tir de piège raté (jet de toucher).
    TrapMissed { skill: &'static str, target: usize },
    /// Un charge-up gagne une charge (`charges` = nouveau total).
    Charges { skill: &'static str, charges: u32 },
    /// Charges dissipées par le temps (aucune libération).
    ChargeExpired { skill: &'static str },
    /// Finisher a libéré `charges` charges : `hits` coups, `dmg` totaux.
    ComboRelease { skill: &'static str, charges: u32, hits: usize, dmg: u32 },
    /// Transformation en forme (durée `duration_ms`).
    Transform { form: AdForm, duration_ms: u32 },
    /// Retour en forme humaine (expiration ou re-cast).
    TransformEnded { form: AdForm },
    /// Werebear a repoussé le monstre `target`.
    Knockback { target: usize },
}

/// Résultat d'un cast réussi.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdCast {
    /// Piège posé (index dans [`World::ad_traps`]).
    Trap(usize),
    /// Volée `Shock Field` : `bolts` carreaux, `hits` touchés.
    ShockWeb { bolts: u32, hits: usize },
    /// Transformation (forme, durée ms).
    Transform { form: AdForm, duration_ms: u32 },
    /// Retour à la forme humaine (re-cast de la forme courante).
    RevertHuman,
}

/// Erreurs de cast (affichables telles quelles en UI).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdError {
    /// Code absent des tables v1.12.
    UnknownSkill,
    /// Ni piège, ni Shock Field, ni forme druide.
    NotCastable,
    /// Le héros n'a aucun point dans ce skill.
    NotLearned,
    /// Mauvaise classe (pièges = assassine, formes = druide).
    WrongClass,
    /// Mana insuffisant (coût `mana_cost_at` de la table).
    NotEnoughMana { cost: u32 },
}

// ---------------------------------------------------------------------------
// Helpers données (courbes × synergies, formules aurastatcalc)
// ---------------------------------------------------------------------------

/// Niveaux du héros courant (vue [`HeroLevels`] — consommée par les formules).
fn hero_levels(w: &World) -> HeroLevels<'_> {
    HeroLevels::new(&w.players[0].data.skills, w.players[0].data.stats.level)
}

/// Arbre temporaire (contexte complet des formules `skill('X'.ln12)`).
fn hero_tree(w: &World) -> SkillTree<'static> {
    let data = SkillTreeData::global();
    let class = skills_hero::class_code(&w.players[0].code);
    let mut tree = SkillTree::with_data(&class, data);
    for s in &w.players[0].data.skills {
        tree.set_level(&s.code, s.level);
    }
    tree.set_char_level(w.players[0].data.stats.level);
    tree
}

/// Évalue une formule `aurastatcalc*`/`auralencalc` DANS le contexte du héros
/// (record propriétaire = `owner`).
fn eval_formula(w: &World, owner: &SkillRecord, formula: &str) -> i64 {
    let calc = crate::calc::CalcString::parse(formula);
    let tree = hero_tree(w);
    let mut ctx = tree.eval_ctx(owner);
    calc.eval(&mut ctx)
}

/// Courbe élémentaire (min, max) × synergie au niveau `lvl` — pièges et
/// combos tirent leurs dégâts des courbes OFFICIELLES du skill.
fn elem_curve(w: &World, rec: &SkillRecord, lvl: u32) -> (u32, u32) {
    let data = SkillTreeData::global();
    let (mn, mx) = data.elem_damage_at(rec, lvl);
    let pct = data.synergy_percent(rec, skills_hero::synergy_kind(rec), &hero_levels(w));
    let f = 1.0 + pct as f64 / 100.0;
    (((mn as f64 * f).floor() as u32).max(1), ((mx as f64 * f).floor() as u32).max(1))
}

/// Courbe physique (min, max) × synergie physique (`Blade Sentinel`).
fn phys_curve(w: &World, rec: &SkillRecord, lvl: u32) -> (u32, u32) {
    let data = SkillTreeData::global();
    let (mn, mx) = data.damage_at(rec, lvl);
    let pct = data.synergy_percent(rec, skills_hero::synergy_kind(rec), &hero_levels(w));
    let f = 1.0 + pct as f64 / 100.0;
    (((mn as f64 * f).floor() as u32).max(1), ((mx as f64 * f).floor() as u32).max(1))
}

/// `ElemType` des tables skills → [`DamageType`] du pipeline de combat.
fn elem_of(rec: &SkillRecord) -> DamageType {
    match rec.e_type {
        ElemType::Fire => DamageType::Fire,
        ElemType::Lightning => DamageType::Lightning,
        ElemType::Cold => DamageType::Cold,
        ElemType::Poison => DamageType::Poison,
        ElemType::Magic => DamageType::Magic,
        _ => DamageType::Physical,
    }
}

/// Munitions du piège à la pose : `calc4` (« shots fired ») si présent, sinon
/// `Param1` (L263 `Charged Bolt Sentry` : `par1 + skill('Lightning
/// Sentry'.blvl)/4`, division entière ; les autres : `par1`).
fn trap_shots(w: &World, rec: &SkillRecord) -> u32 {
    if let Some(calc) = rec.calcs[3].as_ref() {
        let tree = hero_tree(w);
        let mut ctx = tree.eval_ctx(rec);
        let v = calc.eval(&mut ctx);
        return u32::try_from(v.max(0)).unwrap_or(u32::MAX);
    }
    rec.params[0].max(0) as u32
}

/// Durée `Blade Sentinel` (L259 `calc4=ln12` = `Param1 + (lvl-1)×Param2`
/// frames) — l'unique piège limité par DURÉE.
fn blade_sentinel_duration_ms(lvl: u32) -> u32 {
    let frames = 100 + (lvl.max(1) as i64 - 1) * 12; // Param1=100, Param2=12 (L259)
    (frames as u32).saturating_mul(FRAME_MS)
}

/// Tue proprement un monstre (même bloc que la branche mêlée de world.rs).
fn kill_monster(w: &mut World, i: usize) {
    let ent = &mut w.mon_ent[i];
    let dat = &mut w.mon_dat[i];
    dat.state = MonState::Dying;
    dat.dying_ms = 0;
    dat.anim_ms = 0;
    dat.path.clear();
    ent.anim = AnimState::DD;
    ent.anim_frame = 0;
}

/// Applique un coup élémentaire/physique PUR (pièges, releases) via le
/// pipeline officiel `combat_mods` : résists, immunités, froid. Aucun mod
/// héro (mods par défaut, rolls 0 — zéro RNG de mod ajouté). Renvoie les
/// dégâts finaux (0 si immunisé) ; abat la cible à 0 PV (branche DD).
fn apply_elem_hit(w: &mut World, target: usize, dmg: u32, elem: DamageType, cold_len_frames: u32) -> u32 {
    let hit = CombatHit {
        physical: 0,
        elemental: dmg,
        element: elem,
        mods: combat_mods::HitMods { cold_len_frames, ..Default::default() },
        clvl: w.players[0].data.stats.level,
        difficulty: w.difficulty,
        victim: Victim {
            kind: VictimKind::from_mon_kind(&w.mon_dat[target].kind),
            cur_hp: w.mon_dat[target].hp,
            res: w.mon_dat[target].res,
        },
        wearer_life: w.players[0].data.stats.life,
        wearer_max_life: w.players[0].data.stats.max_life,
        wearer_mana: w.players[0].data.stats.mana,
        wearer_max_mana: w.players[0].data.stats.max_mana,
        melee: false,
        roll_deadly: 0,
        roll_wounds: 0,
        roll_crushing: 0,
    };
    let out = combat_mods::apply_combat_mods(&hit);
    let dealt = out.elemental.saturating_add(out.crushing);
    if out.chill_frames > 0 {
        w.mon_dat[target].chill_ms = out.chill_frames.saturating_mul(combat_mods::FRAME_MS_D2);
    }
    if dealt > 0 {
        w.mon_dat[target].hp = w.mon_dat[target].hp.saturating_sub(dealt);
        if w.mon_dat[target].hp == 0 {
            kill_monster(w, target);
        }
    }
    dealt
}

/// Cible vivante la plus PROCHE du point (hors mourants, ≤ `range`).
fn nearest_target(w: &World, x: f32, y: f32, range: f32, exclude: Option<usize>) -> Option<usize> {
    let mut best: Option<(usize, f32)> = None;
    for j in 0..w.mon_ent.len() {
        if w.mon_dat[j].state == MonState::Dying || Some(j) == exclude {
            continue;
        }
        let d = dist(w.mon_ent[j].x - x, w.mon_ent[j].y - y);
        if d <= range && best.map_or(true, |(_, bd)| d < bd) {
            best = Some((j, d));
        }
    }
    best.map(|(j, _)| j)
}

/// Monstres vivants dans le rayon (ordre des indices — déterminisme).
fn targets_in_radius(w: &World, x: f32, y: f32, radius: f32, exclude: Option<usize>) -> Vec<usize> {
    let mut out = Vec::new();
    for j in 0..w.mon_ent.len() {
        if w.mon_dat[j].state == MonState::Dying || Some(j) == exclude {
            continue;
        }
        if dist(w.mon_ent[j].x - x, w.mon_ent[j].y - y) <= radius {
            out.push(j);
        }
    }
    out
}

/// Niveau du skill `code` chez le héros (0 si inconnu).
fn rec_level(w: &World, code: &str) -> u32 {
    w.players[0].data.skills.iter().find(|s| s.code.eq_ignore_ascii_case(code)).map(|s| s.level).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tick (pièges → charges → transformation), appelé par World::tick
// ---------------------------------------------------------------------------

/// Un pas de simulation du module (ordres : pièges, charges, forme).
pub(crate) fn tick(w: &mut World, dt: u32) {
    tick_traps(w, dt);
    tick_combos(w, dt);
    tick_form(w, dt);
}

/// Tir planifié par [`tick_traps`] (timers et résolution dans des emprunts
/// séparés).
struct PlannedShot {
    skill: &'static str,
    target: usize,
    dmg_min: u32,
    dmg_max: u32,
    elem: DamageType,
}

fn tick_traps(w: &mut World, dt: u32) {
    // 1. Timers (durée + cooldown de tir) et planification des tirs.
    let mut expired: Vec<usize> = Vec::new();
    let mut planned: Vec<PlannedShot> = Vec::new();
    for ti in 0..w.ad.traps.len() {
        let (alive, can_fire, x, y, skill) = {
            let t = &mut w.ad.traps[ti];
            if t.expire_ms != u32::MAX {
                if t.expire_ms <= dt {
                    t.expire_ms = 0;
                } else {
                    t.expire_ms -= dt;
                }
            }
            t.fire_cd_ms = t.fire_cd_ms.saturating_sub(dt);
            let alive = t.expire_ms != 0 && t.shots_left > 0;
            (alive, alive && t.fire_cd_ms == 0, t.x, t.y, t.skill)
        };
        if !alive {
            expired.push(ti);
            continue;
        }
        if !can_fire {
            continue;
        }
        // Rien à portée : la munition est CONSERVÉE (règle retail — le
        // piège ne tire qu'à l'apparition d'une cible, cd maintenu à 0).
        let Some(target) = nearest_target(w, x, y, AGGRO_RADIUS, None) else { continue };
        let data = SkillTreeData::global();
        let Some(rec) = data.get(skill) else { continue };
        let (dmg_min, dmg_max) = if rec.e_type == ElemType::None {
            phys_curve(w, rec, w.ad.traps[ti].level) // Blade Sentinel : physique
        } else {
            elem_curve(w, rec, w.ad.traps[ti].level)
        };
        planned.push(PlannedShot { skill, target, dmg_min, dmg_max, elem: elem_of(rec) });
        // Munition consommée + période de tir réelle (600 ms — §1).
        let t = &mut w.ad.traps[ti];
        if t.shots_left != u32::MAX {
            t.shots_left -= 1;
        }
        t.fire_cd_ms = SENTRY_FIRE_PERIOD_MS;
    }
    // 2. Résolution des tirs (jet de toucher + pipeline combat_mods).
    for s in planned {
        let chance = hit_chance(w.players[0].data.stats.to_hit, w.mon_dat[s.target].ac);
        if (w.rng.below(100)) < chance {
            let dmg = w.rng.range_u32(s.dmg_min, s.dmg_max).max(1);
            let dealt = apply_elem_hit(w, s.target, dmg, s.elem, 0);
            w.ad.events.push(AdEvent::TrapFired { skill: s.skill, target: s.target, dmg: dealt, elem: s.elem });
        } else {
            w.ad.events.push(AdEvent::TrapMissed { skill: s.skill, target: s.target });
        }
    }
    // 3. Retraits (durée écoulée OU munitions épuisées), ordre inverse.
    let mut done = expired;
    for (ti, t) in w.ad.traps.iter().enumerate() {
        if t.shots_left == 0 {
            done.push(ti);
        }
    }
    done.sort_unstable();
    done.dedup();
    for ti in done.into_iter().rev() {
        let trap = w.ad.traps.remove(ti);
        w.ad.events.push(AdEvent::TrapExpired { skill: trap.skill });
    }
}

fn tick_combos(w: &mut World, dt: u32) {
    let mut faded: Vec<&'static str> = Vec::new();
    for c in w.ad.combos.iter_mut() {
        c.ttl_ms = c.ttl_ms.saturating_sub(dt);
        if c.ttl_ms == 0 {
            faded.push(c.skill);
        }
    }
    for skill in faded {
        w.ad.combos.retain(|c| c.skill != skill);
        w.ad.events.push(AdEvent::ChargeExpired { skill });
    }
}

fn tick_form(w: &mut World, dt: u32) {
    let Some(form) = w.ad.form.as_mut() else { return };
    form.ttl_ms = form.ttl_ms.saturating_sub(dt);
    if form.ttl_ms > 0 {
        return;
    }
    revert_form(w);
}

/// Retour en forme humaine : retire EXACTEMENT le bonus de vie appliqué.
fn revert_form(w: &mut World) {
    let Some(state) = w.ad.form.take() else { return };
    w.players[0].data.stats.max_life = w.players[0].data.stats.max_life.saturating_sub(state.life_bonus_applied);
    w.players[0].data.stats.life = w.players[0].data.stats.life.min(w.players[0].data.stats.max_life);
    w.ad.events.push(AdEvent::TransformEnded { form: state.form });
}

// ---------------------------------------------------------------------------
// Hooks mêlée (appelés depuis world.rs::hero_attack — touches additives)
// ---------------------------------------------------------------------------

/// Toucher EFFECTIF du héros : +`par4` (50) par charge active
/// (`progressive_tohit`, toutes piles confondues).
pub(crate) fn hero_to_hit_bonus(w: &World) -> u32 {
    w.ad.combos.iter().map(|c| c.charges * COMBO_TOHIT_PER_CHARGE).sum()
}

/// Swing de mêlée EFFECTIF : `HERO_SWING_MS` × 100/(100+IAS) tant qu'une
/// stat `aurastat=attackrate` est portée par la forme (loup : `dm34` = 10 +
/// 80×(niv−1) %, L225). L'ours n'en porte PAS (L230) : swing normal. Clamp
/// MVP : gain plafonné à +100 % ([`IAS_SWING_CAP_DIV`]).
pub(crate) fn hero_swing_ms(w: &World) -> u32 {
    let Some(f) = w.ad.form.as_ref() else { return HERO_SWING_MS };
    let data = SkillTreeData::global();
    let Some(rec) = data.get(f.form.skill()) else { return HERO_SWING_MS };
    let Some(ias) = raw_cols(f.form.skill())
        .and_then(|c| c.stat_calc("attackrate").map(|f| eval_formula(w, rec, f)))
    else {
        return HERO_SWING_MS;
    };
    let ias = ias.clamp(0, 100) as u32;
    let ms = (HERO_SWING_MS as u64 * 100 / (100 + ias as u64).max(1)) as u32;
    ms.max(HERO_SWING_MS / IAS_SWING_CAP_DIV)
}

/// Bonus de défense Werebear : `skill_armor_percent` = `ln34` = 25+5×(niv−1) %
/// de la défense de base (L230). 0 hors forme ours.
pub(crate) fn hero_defense_bonus(w: &World, base_def: u32) -> u32 {
    let Some(f) = w.ad.form.as_ref() else { return 0 };
    if f.form != AdForm::Bear {
        return 0;
    }
    let data = SkillTreeData::global();
    let Some(rec) = data.get("Wearbear") else { return 0 };
    let pct = rec.params[2] + (f.level.max(1) as i64 - 1) * rec.params[3]; // ln34
    ((base_def as i64 * pct.max(0) / 100) as u32)
}

/// Dégâts de mêlée EFFECTIFS : multiplicateur Werebear (`damagepercent` =
/// `ln12` = 50+7×(niv−1) % des dégâts, L230) puis bonus des piles
/// `Tiger Strike` (`calc1=ln12` % PAR charge des dégâts d'ARME `base`).
/// Sans forme ni charges : `scaled` INCHANGÉ (zéro régression).
pub(crate) fn scaled_hero_melee(w: &World, scaled: u32, base: u32) -> u32 {
    let mut dmg = scaled;
    if let Some(f) = w.ad.form.as_ref() {
        if f.form == AdForm::Bear {
            let data = SkillTreeData::global();
            if let Some(rec) = data.get("Wearbear") {
                let pct = rec.params[0] + (f.level.max(1) as i64 - 1) * rec.params[1]; // ln12
                dmg = ((dmg as i64 * (100 + pct).max(0) / 100) as u32);
            }
        }
    }
    if let Some(c) = w.ad.combos.iter().find(|c| c.skill == "Tiger Strike") {
        let data = SkillTreeData::global();
        if let Some(rec) = data.get("Tiger Strike") {
            let lvl = rec_level(w, "Tiger Strike").max(1) as i64;
            let pct = rec.params[0] + (lvl - 1) * rec.params[1]; // ln12
            dmg = dmg.saturating_add(((base as i64 * pct.max(0) * c.charges as i64) / 100) as u32);
        }
    }
    dmg
}

/// Coup de mêlée RÉUSSI du héros sur `target` (`dealt` = dégâts infligés,
/// `base` = jet d'arme brut avant force) : charge un charge-up actif, ou
/// LIBÈRE toutes les piles si le skill actif est un finisher ; knockback
/// Werebear. Appelé APRÈS l'application des dégâts (touches additives de
/// `hero_attack`).
pub(crate) fn note_hero_melee_hit(w: &mut World, target: usize, dealt: u32, base: u32) {
    let Some(active) = w.players[0].data.skills.get(w.players[0].data.active_skill) else { return };
    let active_code = active.code.clone();
    let Some(cols) = raw_cols(&active_code) else { return };
    if cols.finishing {
        release_combos(w, target, dealt, base);
    } else if cols.progressive {
        let Some(code) =
            CHARGE_UPS.iter().find(|c| c.eq_ignore_ascii_case(&active_code)).copied()
        else {
            return;
        };
        charge_up(w, code);
    }
    // Knockback Werebear (règle retail — en-tête §3) : 1 tuile à l'opposé.
    if w.ad.form.as_ref().map(|f| f.form) == Some(AdForm::Bear) {
        knockback_away_from_hero(w, target);
    }
}

/// Ajoute 1 charge au charge-up `code` (plafond 3, timer relancé — §2).
fn charge_up(w: &mut World, code: &'static str) {
    let duration_ms = combo_duration_ms(code);
    if let Some(c) = w.ad.combos.iter_mut().find(|c| c.skill == code) {
        c.charges = (c.charges + 1).min(MAX_CHARGES);
        c.ttl_ms = duration_ms;
        let charges = c.charges;
        w.ad.events.push(AdEvent::Charges { skill: code, charges });
    } else {
        w.ad.combos.push(ComboState { skill: code, charges: 1, ttl_ms: duration_ms });
        w.ad.events.push(AdEvent::Charges { skill: code, charges: 1 });
    }
}

/// Durée des charges : `auralencalc` = `par3` (params[2]) ou littéral `225`
/// (frames) — les 6 valeurs réelles des charge-ups (§2, golden testé).
fn combo_duration_ms(code: &str) -> u32 {
    let data = SkillTreeData::global();
    let frames = match (data.get(code), raw_cols(code).and_then(|c| c.auralencalc.clone())) {
        (Some(rec), Some(f)) if f.eq_ignore_ascii_case("par3") => rec.params[2],
        (Some(_), Some(f)) => f.trim().parse::<i64>().unwrap_or(COMBO_DURATION_FRAMES),
        _ => COMBO_DURATION_FRAMES,
    };
    (frames.max(0) as u32).saturating_mul(FRAME_MS)
}

/// Rayon de release du charge-up (§2) : FoF `par1`=4 / BoI `par1`=6 / CoT
/// nova MVP (constante). Royal Strike possède son propre bloc.
fn release_radius(skill: &str) -> f32 {
    match skill {
        "Fists of Fire" => 4.0,
        "Blades of Ice" => 6.0,
        _ => COT_NOVA_RADIUS_MVP, // Claws of Thunder (nova)
    }
}

/// Libère TOUTES les piles de charges sur un coup de finisher (§2) :
/// dégâts élémentaires par charge (cible + AoE par palier), vol Cobra,
/// payloads réels Royal Strike ; consommation complète des piles.
fn release_combos(w: &mut World, target: usize, dealt: u32, base: u32) {
    struct Pile {
        skill: &'static str,
        charges: u32,
        dmg_min: u32,
        dmg_max: u32,
        elem: DamageType,
        cold_len: u32,
        dmg: u32,
        hits: usize,
    }
    let mut piles: Vec<Pile> = Vec::new();
    let mut cobra_leech_pct = 0_i64;
    {
        let data = SkillTreeData::global();
        for c in w.ad.combos.iter() {
            let Some(rec) = data.get(c.skill) else { continue };
            let lvl = rec_level(w, c.skill).max(1) as i64;
            let mut push = |dmg_min: u32, dmg_max: u32, elem: DamageType, cold_len: u32, dmg: u32, hits: usize| {
                piles.push(Pile {
                    skill: c.skill,
                    charges: c.charges,
                    dmg_min,
                    dmg_max,
                    elem,
                    cold_len,
                    dmg,
                    hits,
                });
            };
            match rec.code.as_str() {
                // Tiger : bonus déjà compté DANS le coup (scaled_hero_melee) —
                // la pile porte ce bonus pour l'événement de release.
                "Tiger Strike" => {
                    let pct = rec.params[0] + (lvl - 1) * rec.params[1];
                    let bonus = ((base as i64 * pct.max(0) * c.charges as i64) / 100) as u32;
                    push(0, 0, DamageType::Physical, 0, bonus, 1);
                }
                // Cobra : vol de vie/mana par charge (par1 + par2×(lvl-1)).
                "Cobra Strike" => {
                    cobra_leech_pct += (rec.params[0] + (lvl - 1) * rec.params[1]) * c.charges as i64;
                    push(0, 0, DamageType::Physical, 0, 0, 1);
                }
                // Royal Strike : payloads missiles réels (bloc dédié plus bas).
                "Royal Strike" => push(0, 0, DamageType::Physical, 0, 0, 0),
                // Élémentaires : courbe × synergie × charges, AoE par palier.
                _ => {
                    let (dmin, dmax) = elem_curve(w, rec, rec_level(w, c.skill));
                    let cold_len = if rec.e_type == ElemType::Cold {
                        u32::try_from(data.elem_len_at(rec, rec_level(w, c.skill)).max(0)).unwrap_or(0)
                    } else {
                        0
                    };
                    push(dmin, dmax, elem_of(rec), cold_len, 0, 0);
                }
            }
        }
    }
    // Résolution élémentaire (cible directe + AoE par palier ≥ 2 charges).
    for p in piles.iter_mut() {
        if p.dmg_max == 0 {
            continue; // Tiger (déjà compté) / Cobra (leech) / Royal (bloc dédié)
        }
        let radius = if p.charges >= 2 { release_radius(p.skill) } else { 0.0 };
        for _ in 0..p.charges {
            if w.mon_dat[target].state != MonState::Dying {
                let dmg = w.rng.range_u32(p.dmg_min, p.dmg_max).max(1);
                p.dmg += apply_elem_hit(w, target, dmg, p.elem, p.cold_len);
                p.hits += 1;
            }
            if radius > 0.0 {
                let (tx, ty) = (w.mon_ent[target].x, w.mon_ent[target].y);
                let victims = targets_in_radius(w, tx, ty, radius, Some(target));
                for j in victims {
                    if w.mon_dat[j].state == MonState::Dying {
                        continue;
                    }
                    let dmg = w.rng.range_u32(p.dmg_min, p.dmg_max).max(1);
                    p.dmg += apply_elem_hit(w, j, dmg, p.elem, p.cold_len);
                    p.hits += 1;
                }
            }
        }
    }
    // Royal Strike (Phoenix Strike) : payloads réels PAR CHARGE (§2).
    if let Some(charges) =
        w.ad.combos.iter().find(|c| c.skill == "Royal Strike").map(|c| c.charges)
    {
        let data = SkillTreeData::global();
        let Some(rec) = data.get("Royal Strike") else { return };
        let (tx, ty) = (w.mon_ent[target].x, w.mon_ent[target].y);
        if let Some(pile) = piles.iter_mut().find(|p| p.skill == "Royal Strike") {
            let mut release_at =
                |w: &mut World, j: usize, dmg: u32, elem: DamageType, cold: u32, p: &mut Pile| {
                    if w.mon_dat[j].state != MonState::Dying {
                        p.dmg += apply_elem_hit(w, j, dmg, elem, cold);
                        p.hits += 1;
                    }
                };
            // Charge 1 — météor feu : EMin=20 (`royalstrikemeteor` L565),
            // rayon d'explosion par2=6 (« radius of meteor explosion »).
            if charges >= 1 {
                let r = rec.params[1] as f32;
                for j in targets_in_radius(w, tx, ty, r, Some(target)) {
                    release_at(w, j, 20, DamageType::Fire, 0, pile);
                }
                release_at(w, target, 20, DamageType::Fire, 0, pile);
            }
            // Charge 2 — chaîne de foudre : EMin=1
            // (`royalstrikechainlightning` L570) ; MVP : cible unique (le
            // « skip » par6=10 chaîne de cible en cible — hors port).
            if charges >= 2 {
                release_at(w, target, 1, DamageType::Lightning, 0, pile);
            }
            // Charge 3 — 16 éclats de glace (par5, L282) : EMin=16 froid +
            // ELen=100 (`royalstrikechaosice` L571) ; MVP : répartis en
            // cercle de rayon 2 autour de la cible (cyclage déterministe).
            if charges >= 3 {
                let victims = targets_in_radius(w, tx, ty, 2.0, None);
                for b in 0..rec.params[4] {
                    if victims.is_empty() {
                        break;
                    }
                    let j = victims[(b as usize) % victims.len()];
                    release_at(w, j, 16, DamageType::Cold, 100, pile);
                }
            }
        }
    }
    // Cobra : vol de vie/mana (% des dégâts physiques du finisher, diviseur
    // de difficulté — même formule que le leech d'arme, combat_mods §5).
    if cobra_leech_pct > 0 && dealt > 0 {
        let row = crate::difficulty::embedded().row(w.difficulty);
        let life = combat_mods::cap_leech(
            (dealt as u64 * cobra_leech_pct as u64 / 100
                / row.life_steal_divisor.max(1) as u64) as u32,
            w.players[0].data.stats.max_life - w.players[0].data.stats.life,
        );
        let mana = combat_mods::cap_leech(
            (dealt as u64 * cobra_leech_pct as u64 / 100
                / row.mana_steal_divisor.max(1) as u64) as u32,
            w.players[0].data.stats.max_mana - w.players[0].data.stats.mana,
        );
        w.players[0].data.stats.life += life;
        w.players[0].data.stats.mana += mana;
    }
    // Consommation des piles (règle retail : le finisher libère tout) +
    // événements par pile (dmg/hits propres ; Tiger porte son %-bonus).
    for p in piles {
        w.ad.events.push(AdEvent::ComboRelease {
            skill: p.skill,
            charges: p.charges,
            hits: p.hits,
            dmg: p.dmg,
        });
    }
    w.ad.combos.clear();
}

/// Knockback Werebear : repousse d'1 tuile à l'opposé du héros (tuile
/// marchable requise, sinon le monstre reste sur place — MVP déterministe).
fn knockback_away_from_hero(w: &mut World, target: usize) {
    let (dx, dy) = (w.mon_ent[target].x - w.players[0].ent.x, w.mon_ent[target].y - w.players[0].ent.y);
    let d = dist(dx, dy);
    if d <= f32::EPSILON {
        return;
    }
    let nx = w.mon_ent[target].x + dx / d * BEAR_KNOCKBACK_TILES;
    let ny = w.mon_ent[target].y + dy / d * BEAR_KNOCKBACK_TILES;
    let (tx, ty) = (nx.floor() as i32, ny.floor() as i32);
    if w.is_walkable(tx, ty) {
        w.mon_ent[target].x = nx;
        w.mon_ent[target].y = ny;
        w.ad.events.push(AdEvent::Knockback { target });
    }
}

// ---------------------------------------------------------------------------
// Cast (API publique World::cast_ad_skill)
// ---------------------------------------------------------------------------

/// Lance un skill du module : pièges (posés au héros), `Shock Field`
/// (volée), formes druides. Valide apprentissage + castabilité + classe +
/// mana (coût table) ; la pose d'une 6e pièce remplace la PLUS ANCIENNE
/// (`petmax=5`).
pub fn cast(w: &mut World, code: &str, aim: Option<(i32, i32)>) -> Result<AdCast, AdError> {
    let data = SkillTreeData::global();
    let Some(rec) = data.get(code) else { return Err(AdError::UnknownSkill) };
    let Some(cols) = raw_cols(code) else { return Err(AdError::UnknownSkill) };
    let level = rec_level(w, &rec.code);
    if level == 0 {
        return Err(AdError::NotLearned);
    }
    let form = AdForm::from_code(&rec.code);
    let is_form = form.is_some();
    let is_trap =
        TRAPS.iter().any(|t| t.eq_ignore_ascii_case(&rec.code)) && cols.petmax == MAX_TRAPS as u32;
    let is_web = rec.code == "Shock Field";
    if !is_trap && !is_web && !is_form {
        return Err(AdError::NotCastable);
    }
    let class = skills_hero::class_code(&w.players[0].code);
    let expected = if is_form { "dru" } else { "ass" };
    if class != expected {
        return Err(AdError::WrongClass);
    }
    let cost = data.mana_cost_at(rec, level).max(0) as u32;
    if (w.players[0].data.stats.mana as u32) < cost {
        return Err(AdError::NotEnoughMana { cost });
    }
    w.players[0].data.stats.mana -= cost;

    if let Some(form) = form {
        return cast_form(w, form, level);
    }
    if is_web {
        return cast_shock_field(w, rec, level, aim);
    }
    // Piège : posé sur la tuile du héros (d2game passera le clic plus tard).
    let skill = TRAPS.iter().find(|t| t.eq_ignore_ascii_case(&rec.code)).copied().unwrap();
    let blade = skill == "Blade Sentinel";
    let trap = TrapRec {
        skill,
        summon: cols.summon.clone().unwrap_or_default(),
        level,
        x: w.players[0].ent.x,
        y: w.players[0].ent.y,
        shots_left: if blade { u32::MAX } else { trap_shots(w, rec) }, // Blade : durée seule
        fire_cd_ms: 0,
        expire_ms: if blade { blade_sentinel_duration_ms(level) } else { u32::MAX },
    };
    if w.ad.traps.len() >= MAX_TRAPS {
        w.ad.traps.remove(0); // la plus ancienne disparaît (règle retail)
    }
    w.ad.traps.push(trap);
    let i = w.ad.traps.len() - 1;
    let t = &w.ad.traps[i];
    w.ad.events.push(AdEvent::TrapPlaced {
        skill: t.skill,
        x: t.x.floor() as i32,
        y: t.y.floor() as i32,
    });
    Ok(AdCast::Trap(i))
}

/// Transformation druide : buff temporisé, re-cast de la forme COURANTE =
/// retour humain (règle D2), switch direct Loup↔Ours (le bonus de vie de
/// l'ancienne forme est retiré d'abord — formes exclusives).
fn cast_form(w: &mut World, form: AdForm, level: u32) -> Result<AdCast, AdError> {
    let current = w.ad.form.as_ref().map(|f| f.form);
    if let Some(cur) = current {
        revert_form(w);
        if cur == form {
            return Ok(AdCast::RevertHuman); // re-cast : simple retour humain
        }
    }
    let data = SkillTreeData::global();
    let Some(rec) = data.get(form.skill()) else { return Err(AdError::UnknownSkill) };
    let cols = raw_cols(form.skill());
    // Durée : auralencalc = 1000 + skill('Shape Shifting'.ln12) (frames).
    let frames = cols
        .and_then(|c| c.auralencalc.clone())
        .map(|f| eval_formula(w, rec, f.as_str()))
        .unwrap_or(1000);
    let duration_ms = (frames.max(0) as u32).saturating_mul(FRAME_MS);
    // Bonus de vie max : aurastat=item_maxhp_percent (par2/par5 +
    // skill('Shape Shifting'.ln34) — L225/L230).
    let hp_pct = cols
        .and_then(|c| c.stat_calc("item_maxhp_percent").map(|f| eval_formula(w, rec, f)))
        .unwrap_or(0);
    let bonus = ((w.players[0].data.stats.max_life as i64 * hp_pct.max(0) / 100) as u32);
    w.players[0].data.stats.max_life += bonus;
    w.players[0].data.stats.life += bonus;
    w.ad.form = Some(AdFormState { form, level, ttl_ms: duration_ms, life_bonus_applied: bonus });
    w.ad.events.push(AdEvent::Transform { form, duration_ms });
    Ok(AdCast::Transform { form, duration_ms })
}

/// `Shock Field` L258 (« Shock Web ») : volée IMMÉDIATE de
/// `par1 + lvl/par2 + skill('Fire Trauma'.blvl)/3` carreaux (`prgcalc1`
/// verbatim, division entière) sur les monstres les plus proches du point
/// visé (rayon 2 tuiles, repli : monstre le plus proche du héros ; carreaux
/// en trop = perdus — la dispersion réelle du shock field est hors port MVP).
fn cast_shock_field(
    w: &mut World,
    rec: &SkillRecord,
    level: u32,
    aim: Option<(i32, i32)>,
) -> Result<AdCast, AdError> {
    let bolts = {
        let tree = hero_tree(w);
        let mut ctx = tree.eval_ctx(rec);
        rec.prg_calcs[0]
            .as_ref()
            .map(|c| c.eval(&mut ctx))
            .unwrap_or(rec.params[0])
            .max(1) as u32
    };
    let (ax, ay) = match aim {
        Some((tx, ty)) => (tx as f32 + 0.5, ty as f32 + 0.5),
        None => match nearest_target(w, w.players[0].ent.x, w.players[0].ent.y, AGGRO_RADIUS, None) {
            Some(j) => (w.mon_ent[j].x, w.mon_ent[j].y),
            None => (w.players[0].ent.x, w.players[0].ent.y),
        },
    };
    let (dmin, dmax) = elem_curve(w, rec, level);
    let elem = elem_of(rec);
    let mut victims = targets_in_radius(w, ax, ay, 2.0, None);
    victims.sort_by_key(|&j| {
        let (jx, jy) = (w.mon_ent[j].x, w.mon_ent[j].y);
        (dist(jx - ax, jy - ay) * 100.0) as i64 // tri stable par distance
    });
    let mut hits = 0_usize;
    for j in victims.into_iter().take(bolts as usize) {
        let chance = hit_chance(w.players[0].data.stats.to_hit, w.mon_dat[j].ac);
        if (w.rng.below(100)) < chance {
            let dmg = w.rng.range_u32(dmin, dmax).max(1);
            apply_elem_hit(w, j, dmg, elem, 0);
            hits += 1;
        }
    }
    Ok(AdCast::ShockWeb { bolts, hits })
}

// ---------------------------------------------------------------------------
// Tests : machine combo, sentries périodiques, transformation buff/expiration,
// Shock Field, goldens données réelles, déterminisme.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GameEvent;

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn manifest() -> serde_json::Value {
        serde_json::from_str(MANIFEST_JSON).unwrap()
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Héros assassine (AS) au centre, SANS monstres.
    fn world_ass(seed: u64) -> World {
        let mut w = World::with_seed_and_hero(open_grid(56, 40), manifest(), seed, "AS");
        w.clear_monsters();
        w
    }

    /// Héros druide (DZ) au centre, SANS monstres.
    fn world_dru(seed: u64) -> World {
        let mut w = World::with_seed_and_hero(open_grid(56, 40), manifest(), seed, "DZ");
        w.clear_monsters();
        w
    }

    /// Monte le héros au niveau `lvl` (XP table réelle) et accorde `pts`
    /// points de skill.
    fn level_up(w: &mut World, lvl: u32, pts: u32) {
        let need = crate::world::xp_threshold(&w.players[0].code.clone(), lvl);
        w.grant_xp(need);
        w.grant_skill_points(pts);
    }

    /// Avance `ms` de simulation par pas de 100 ms.
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

    /// Compte les événements AD drainés par type (UN SEUL drain).
    fn count_ad(w: &mut World) -> (usize, usize, usize, usize) {
        let ev = w.drain_ad_events();
        let fired = ev.iter().filter(|e| matches!(e, AdEvent::TrapFired { .. })).count();
        let missed = ev.iter().filter(|e| matches!(e, AdEvent::TrapMissed { .. })).count();
        let placed = ev.iter().filter(|e| matches!(e, AdEvent::TrapPlaced { .. })).count();
        let expired = ev.iter().filter(|e| matches!(e, AdEvent::TrapExpired { .. })).count();
        (fired, missed, placed, expired)
    }

    /// Total des tirs partis (touchés + ratés) — un seul drain.
    fn count_shots(w: &mut World) -> usize {
        let (fired, missed, _, _) = count_ad(w);
        fired + missed
    }

    /// Force un « coup de mêlée réussi » via le hook réel (bypass du RNG de
    /// toucher de hero_attack — la machine combo est testée pur ;
    /// dealt=base=10).
    fn melee_hit(w: &mut World, target: usize) {
        crate::assassin_druid::note_hero_melee_hit(w, target, 10, 10);
    }

    // -- GOLDEN : colonnes réelles du TSV (chaque assertion cite sa ligne) --

    #[test]
    fn golden_real_columns_citees_ligne() {
        // Pièges (patch_skills.txt).
        assert_eq!(line_of("Blade Sentinel"), 259);
        assert_eq!(line_of("Charged Bolt Sentry"), 263);
        assert_eq!(line_of("Wake of Fire Sentry"), 264);
        assert_eq!(line_of("Lightning Sentry"), 273);
        assert_eq!(line_of("Inferno Sentry"), 274);
        assert_eq!(line_of("Death Sentry"), 278);
        // Charge-ups + finishers + formes + Shock Field.
        assert_eq!(line_of("Tiger Strike"), 256);
        assert_eq!(line_of("Fists of Fire"), 261);
        assert_eq!(line_of("Cobra Strike"), 267);
        assert_eq!(line_of("Claws of Thunder"), 271);
        assert_eq!(line_of("Blades of Ice"), 276);
        assert_eq!(line_of("Royal Strike"), 282);
        assert_eq!(line_of("Dragon Talon"), 257);
        assert_eq!(line_of("Dragon Claw"), 262);
        assert_eq!(line_of("Dragon Tail"), 272);
        assert_eq!(line_of("Dragon Flight"), 277);
        assert_eq!(line_of("Wearwolf"), 225);
        assert_eq!(line_of("Shape Shifting"), 226);
        assert_eq!(line_of("Wearbear"), 230);
        assert_eq!(line_of("Shock Field"), 258);
        // Flags progressifs/finishers + summon/petmax réels.
        for code in CHARGE_UPS {
            assert!(raw_cols(code).unwrap().progressive, "{code} progressive=1");
        }
        for code in ["Dragon Talon", "Dragon Claw", "Dragon Tail", "Dragon Flight"] {
            assert!(raw_cols(code).unwrap().finishing, "{code} finishing=1");
        }
        assert_eq!(raw_cols("Lightning Sentry").unwrap().summon.as_deref(), Some("lightningsentry"));
        assert_eq!(raw_cols("Lightning Sentry").unwrap().petmax, 5);
        assert_eq!(raw_cols("Blade Sentinel").unwrap().summon.as_deref(), Some("bladecreeper"));
        assert_eq!(raw_cols("Wearwolf").unwrap().aurastate, "wolf");
        assert_eq!(raw_cols("Wearbear").unwrap().aurastate, "bear");
        assert_eq!(
            raw_cols("Wearwolf").unwrap().auralencalc.as_deref(),
            Some("1000+skill('Shape Shifting'.ln12)")
        );
        // Stats d'aura réelles (paires aurastat/aurastatcalc).
        assert_eq!(raw_cols("Wearwolf").unwrap().aurastats[1].as_deref(), Some("attackrate"));
        assert_eq!(
            raw_cols("Wearwolf").unwrap().aurastatcalcs[1].as_deref(),
            Some("dm34"),
            "dm34 = Param3 + (niv-1)×Param4 (10/80)"
        );
        assert!(raw_cols("Wearbear").unwrap().stat_calc("attackrate").is_none(), "l'ours n'accélère pas");
        assert_eq!(raw_cols("Wearbear").unwrap().aurastats[0].as_deref(), Some("damagepercent"));
        assert_eq!(raw_cols("Wearbear").unwrap().aurastats[1].as_deref(), Some("skill_armor_percent"));
        assert_eq!(raw_cols("Wearbear").unwrap().aurastats[2].as_deref(), Some("item_maxhp_percent"));
        assert_eq!(raw_cols("Wearwolf").unwrap().aurastats[3].as_deref(), Some("item_maxhp_percent"));
        // Constantes du module.
        assert_eq!(SENTRY_FIRE_PERIOD_MS, 600, "aidel 15 frames × 40 ms");
        assert_eq!(MAX_TRAPS, 5);
        assert_eq!(COMBO_DURATION_FRAMES, 225);
        assert_eq!(COMBO_TOHIT_PER_CHARGE, 50);
        // Durées de charge : par3 (225) ×5, littéral 225 (Royal Strike L282).
        for code in
            ["Tiger Strike", "Fists of Fire", "Cobra Strike", "Claws of Thunder", "Blades of Ice"]
        {
            assert_eq!(raw_cols(code).unwrap().auralencalc.as_deref(), Some("par3"));
        }
        assert_eq!(raw_cols("Royal Strike").unwrap().auralencalc.as_deref(), Some("225"));
    }

    #[test]
    fn golden_trap_curves_shots_and_duration() {
        let data = SkillTreeData::global();
        // Lightning Sentry L273 : (1, 20) foudre niv 1 ; niv 10 : 20+70+32.
        let ls = data.get("Lightning Sentry").unwrap();
        assert_eq!(data.elem_damage_at(ls, 1), (1, 20));
        assert_eq!(data.elem_damage_at(ls, 10), (1, 122), "paliers 2-8 ×10 puis 9-10 ×16");
        // Blade Sentinel L259 : physique (6, 10) niv 1 ; durée 100+12×(n-1).
        let bs = data.get("Blade Sentinel").unwrap();
        assert_eq!(bs.e_type, ElemType::None);
        assert_eq!(data.damage_at(bs, 1), (6, 10));
        assert_eq!(blade_sentinel_duration_ms(1), 4_000, "100 frames × 40 ms");
        assert_eq!(blade_sentinel_duration_ms(5), 5_920, "148 frames × 40 ms");
        // Munitions CBS : calc4 = par1 + LS blvl/4 (L263, division entière).
        let mut w = world_ass(7);
        level_up(&mut w, 24, 7);
        w.learn_skill("Fire Trauma").unwrap();
        w.learn_skill("Shock Field").unwrap();
        w.learn_skill("Charged Bolt Sentry").unwrap();
        let cbs = data.get("Charged Bolt Sentry").unwrap();
        assert_eq!(trap_shots(&w, cbs), 5, "LS=0 : par1 seul");
        for _ in 0..4 {
            w.learn_skill("Lightning Sentry").unwrap(); // LS niv 4
        }
        assert_eq!(trap_shots(&w, cbs), 6, "calc4 réel : 5 + 4/4");
    }

    // -- Sentries : tir périodique, munitions, plafond 5, durée Blade -------

    #[test]
    fn sentry_fires_periodically() {
        let mut w = world_ass(0xD2A5_5001);
        level_up(&mut w, 24, 4);
        // Chaîne réelle : Fire Trauma → Shock Field → CBS → Lightning Sentry.
        w.learn_skill("Fire Trauma").unwrap();
        w.learn_skill("Shock Field").unwrap();
        w.learn_skill("Charged Bolt Sentry").unwrap();
        w.learn_skill("Lightning Sentry").unwrap();
        assert!(w.spawn_monster("FA", 30, 20)); // à ~2 tuiles du héros central
        assert!(w.spawn_monster("FA", 33, 22)); // 2e cible (PV niv 1 ≈ 4 : le
        // premier tir tue la plus proche, la périodicité continue sur l'autre)
        let cast = w.cast_ad_skill("Lightning Sentry", None).expect("cast LS");
        assert_eq!(cast, AdCast::Trap(0));
        assert_eq!(w.ad_traps().len(), 1);
        assert_eq!(w.ad_traps()[0].shots_left, 10, "Param1=10 shots fired (L273)");
        // Premier tir immédiat (piège armé), puis période 600 ms.
        run(&mut w, 100);
        let (shots1, missed1, placed, _) = count_ad(&mut w);
        assert_eq!(placed, 1);
        assert_eq!(shots1 + missed1, 1, "un tir à t=100 ms");
        run(&mut w, 500);
        assert_eq!(count_shots(&mut w), 0, "période 600 ms : rien à t=600");
        run(&mut w, 200);
        assert_eq!(count_shots(&mut w), 1, "2e tir à t=700 ms");
        assert_eq!(w.ad_traps()[0].shots_left, 8, "2 munitions consommées (tirés OU ratés)");
    }

    #[test]
    fn sentry_expires_after_shots_and_blade_after_duration() {
        // Sentries : expiration aux munitions (« shots fired »), pas de cible
        // ⇒ munitions CONSERVÉES.
        let mut w = world_ass(0xD2A5_5002);
        level_up(&mut w, 12, 3);
        w.learn_skill("Fire Trauma").unwrap();
        w.learn_skill("Shock Field").unwrap();
        w.learn_skill("Charged Bolt Sentry").unwrap();
        w.cast_ad_skill("Charged Bolt Sentry", None).unwrap();
        assert_eq!(w.ad_traps()[0].shots_left, 5, "par1=5 (LS=0)");
        run(&mut w, 5_000);
        assert_eq!(w.ad_traps().len(), 1, "sans cible le piège tient ses 5 tirs");
        // 5 monstres : chaque tir tue le plus proche (PV niv 1 ≈ 4), le
        // suivant devient la cible — les 5 munitions partent.
        for x in 30..35 {
            assert!(w.spawn_monster("FA", x, 20));
        }
        run(&mut w, 3_500);
        assert!(w.ad_traps().is_empty(), "5 tirs à 600 ms ⇒ épuisé ≤ 3 s");
        let (_, _, _, expired) = count_ad(&mut w);
        assert_eq!(expired, 1, "TrapExpired émis");

        // Blade Sentinel : durée réelle 100+12×(niv−1) frames (L259).
        let mut w2 = world_ass(0xD2A5_5003);
        level_up(&mut w2, 6, 1);
        w2.learn_skill("Blade Sentinel").unwrap();
        w2.cast_ad_skill("Blade Sentinel", None).unwrap();
        assert_eq!(w2.ad_traps()[0].expire_ms, 4_000, "niveau 1 : 100 frames");
        run(&mut w2, 3_900);
        assert_eq!(w2.ad_traps().len(), 1, "encore vivante à 3,9 s");
        run(&mut w2, 200);
        assert!(w2.ad_traps().is_empty(), "expirée à 4,0 s");
        assert!(matches!(
            w2.drain_ad_events().iter().find(|e| matches!(e, AdEvent::TrapExpired { .. })),
            Some(AdEvent::TrapExpired { skill: "Blade Sentinel" })
        ));
    }

    #[test]
    fn max_five_traps_oldest_replaced() {
        let mut w = world_ass(0xD2A5_5004);
        level_up(&mut w, 12, 4);
        w.players[0].data.stats.mana = 1_000; // 6 casts : le mana n'est pas l'objet du test
        w.learn_skill("Fire Trauma").unwrap();
        w.learn_skill("Shock Field").unwrap();
        w.learn_skill("Charged Bolt Sentry").unwrap();
        w.learn_skill("Blade Sentinel").unwrap();
        for i in 0..6 {
            let code = if i < 5 { "Charged Bolt Sentry" } else { "Blade Sentinel" };
            w.cast_ad_skill(code, None).expect("cast");
        }
        assert_eq!(w.ad_traps().len(), 5, "petmax=5");
        assert_eq!(w.ad_traps()[0].skill, "Charged Bolt Sentry", "les CBS restent");
        assert_eq!(w.ad_traps().last().unwrap().skill, "Blade Sentinel", "la 6e remplace la 1re");
        assert_eq!(w.ad_traps().iter().filter(|t| t.skill == "Charged Bolt Sentry").count(), 4);
    }

    // -- Machine combo complète ---------------------------------------------

    #[test]
    fn combo_machine_charge_cap_expire_release() {
        let mut w = world_ass(0xD2C0_6001);
        level_up(&mut w, 30, 3);
        w.learn_skill("Tiger Strike").unwrap();
        w.learn_skill("Dragon Talon").unwrap();
        assert!(w.spawn_monster("FA", 30, 20));
        // Charge-up TIGER actif : les coups réussis chargent (1/coup).
        let tiger_idx = w.hero_skills().iter().position(|s| s.code == "Tiger Strike").unwrap();
        w.select_skill(tiger_idx);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        assert_eq!(w.ad.combos.len(), 1);
        assert_eq!(w.ad.combos[0].charges, 3, "plafond 3 orbes");
        assert_eq!(w.ad.combos[0].ttl_ms, 9_000, "par3=225 frames × 40 ms");
        melee_hit(&mut w, 0);
        assert_eq!(w.ad.combos[0].charges, 3, "4e coup : pas de dépassement");
        // Dissipation par le temps (9 s) sans finisher.
        run(&mut w, 9_100);
        assert!(w.ad.combos.is_empty());
        assert!(matches!(
            w.drain_ad_events().iter().find(|e| matches!(e, AdEvent::ChargeExpired { .. })),
            Some(AdEvent::ChargeExpired { skill: "Tiger Strike" })
        ));
        // Recharge (2) puis FINISHER : libération + reset.
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        let dragon_idx = w.hero_skills().iter().position(|s| s.code == "Dragon Talon").unwrap();
        w.select_skill(dragon_idx);
        melee_hit(&mut w, 0); // le finisher libère
        assert!(w.ad.combos.is_empty(), "charges consommées");
        assert!(matches!(
            w.drain_ad_events().iter().find(|e| matches!(e, AdEvent::ComboRelease { .. })),
            Some(AdEvent::ComboRelease { skill: "Tiger Strike", charges: 2, dmg: 20, hits: 1 })
        ), "ln12 niv 1 = 100 %/charge × base 10 × 2 charges = 20");
    }

    #[test]
    fn combo_stacks_multiple_charge_ups_and_releases_all() {
        let mut w = world_ass(0xD2C0_6002);
        level_up(&mut w, 30, 4);
        w.learn_skill("Tiger Strike").unwrap();
        w.learn_skill("Fists of Fire").unwrap();
        w.learn_skill("Dragon Talon").unwrap();
        w.learn_skill("Dragon Claw").unwrap(); // reqskill Dragon Talon (L262)
        assert!(w.spawn_monster("FA", 30, 20));
        let tiger_idx = w.hero_skills().iter().position(|s| s.code == "Tiger Strike").unwrap();
        let fof_idx = w.hero_skills().iter().position(|s| s.code == "Fists of Fire").unwrap();
        w.select_skill(tiger_idx);
        melee_hit(&mut w, 0);
        w.select_skill(fof_idx);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        assert_eq!(w.ad.combos.len(), 2, "piles Tiger + Fists coexistent (états progressifs)");
        // Finisher : TOUTES les piles libérées.
        let dc_idx = w.hero_skills().iter().position(|s| s.code == "Dragon Claw").unwrap();
        w.select_skill(dc_idx);
        let hp_before = w.mon_dat[0].hp;
        melee_hit(&mut w, 0);
        assert!(w.ad.combos.is_empty());
        assert_eq!(
            w.drain_ad_events().iter().filter(|e| matches!(e, AdEvent::ComboRelease { .. })).count(),
            2,
            "Tiger + Fists libérés"
        );
        assert!(w.mon_dat[0].hp < hp_before, "dégâts feu appliqués (ResFi FA = 0)");
    }

    #[test]
    fn combo_fists_of_fire_aoe_radius() {
        let mut w = world_ass(0xD2C0_6003);
        level_up(&mut w, 30, 3);
        w.learn_skill("Fists of Fire").unwrap();
        w.learn_skill("Dragon Talon").unwrap();
        w.learn_skill("Dragon Claw").unwrap();
        // Cible + voisin à 3 tuiles (dans le rayon par1=4) + un à 8 tuiles.
        assert!(w.spawn_monster("FA", 30, 20)); // cible
        assert!(w.spawn_monster("FA", 33, 20)); // à 3 tuiles
        assert!(w.spawn_monster("FA", 38, 20)); // à 8 tuiles (hors rayon)
        let fof_idx = w.hero_skills().iter().position(|s| s.code == "Fists of Fire").unwrap();
        let dc_idx = w.hero_skills().iter().position(|s| s.code == "Dragon Claw").unwrap();
        w.select_skill(fof_idx);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0); // 2 charges ⇒ AoE rayon 4
        w.select_skill(dc_idx);
        let hp2 = w.mon_dat[2].hp;
        melee_hit(&mut w, 0); // release : cible + AoE
        assert!(w.mon_dat[1].hp < w.mon_dat[1].max_hp, "AoE rayon 4 touche le voisin à 3 tuiles");
        assert_eq!(w.mon_dat[2].hp, hp2, "à 8 tuiles : hors AoE");
        let ev = w.drain_ad_events();
        assert!(matches!(
            ev.iter().find(|e| matches!(e, AdEvent::ComboRelease { .. })),
            Some(AdEvent::ComboRelease { skill: "Fists of Fire", charges: 2, .. })
        ));
    }

    #[test]
    fn combo_blades_of_ice_chills_single_target() {
        let mut w = world_ass(0xD2C0_6004);
        level_up(&mut w, 30, 4);
        // Chaîne réelle : Fists of Fire (L261) → Claws of Thunder (L271) →
        // Blades of Ice (L276).
        w.learn_skill("Fists of Fire").unwrap();
        w.learn_skill("Claws of Thunder").unwrap();
        w.learn_skill("Blades of Ice").unwrap();
        w.learn_skill("Dragon Talon").unwrap();
        assert!(w.spawn_monster("FA", 30, 20));
        let boi_idx = w.hero_skills().iter().position(|s| s.code == "Blades of Ice").unwrap();
        let dt_idx = w.hero_skills().iter().position(|s| s.code == "Dragon Talon").unwrap();
        w.select_skill(boi_idx);
        melee_hit(&mut w, 0); // 1 charge
        w.select_skill(dt_idx);
        let hp0 = w.mon_dat[0].hp;
        melee_hit(&mut w, 0); // release : cible SEULE (1 charge) + gel
        assert!(matches!(
            w.drain_ad_events().iter().find(|e| matches!(e, AdEvent::ComboRelease { .. })),
            Some(AdEvent::ComboRelease { skill: "Blades of Ice", charges: 1, hits: 1, .. })
        ));
        assert!(w.mon_dat[0].hp < hp0, "dégâts froid appliqués");
        assert_eq!(w.mon_dat[0].chill_ms, 4_000, "ELen=100 frames × 40 ms (div Normal = 1)");
    }

    #[test]
    fn combo_cobra_strike_leeches_life_and_mana() {
        let mut w2 = world_ass(0xD2C0_6005);
        level_up(&mut w2, 30, 1);
        // Prérequis réel : Cobra Strike exige Tiger Strike (L267).
        assert!(matches!(
            w2.learn_skill("Cobra Strike"),
            Err(crate::SkillTreeError::MissingPrereq { skill, required })
                if skill == "Cobra Strike" && required == "Tiger Strike"
        ));
        let mut w = world_ass(0xD2C0_6006);
        level_up(&mut w, 30, 3);
        w.learn_skill("Tiger Strike").unwrap();
        w.learn_skill("Cobra Strike").unwrap();
        w.learn_skill("Dragon Talon").unwrap();
        assert!(w.spawn_monster("FA", 30, 20));
        // Héros blessé + mana entamé pour mesurer le vol.
        w.players[0].data.stats.life -= 30;
        w.players[0].data.stats.mana -= 10;
        let cobra_idx = w.hero_skills().iter().position(|s| s.code == "Cobra Strike").unwrap();
        let dt_idx = w.hero_skills().iter().position(|s| s.code == "Dragon Talon").unwrap();
        w.select_skill(cobra_idx);
        melee_hit(&mut w, 0); // 1 charge (par1=40 %)
        w.select_skill(dt_idx);
        let (life0, mana0) = (w.players[0].data.stats.life, w.players[0].data.stats.mana);
        melee_hit(&mut w, 0); // finisher : vol 40 % de dealt=10 → 4 (div N=1)
        assert_eq!(w.players[0].data.stats.life - life0, 4, "40 % × dealt 10 (Normal : div=1)");
        assert_eq!(w.players[0].data.stats.mana - mana0, 4, "Cobra vole vie ET mana");
    }

    #[test]
    fn combo_royal_strike_real_payloads_per_charge() {
        let mut w = world_ass(0xD2C0_6007);
        level_up(&mut w, 30, 10);
        // Chaînes réelles : Royal Strike (L282) exige Cobra Strike + Blades
        // of Ice (⇒ Tiger, FoF→CoT) ; Dragon Flight (L277) exige la lignée
        // Dragon Talon → Claw → Tail.
        for code in [
            "Tiger Strike",
            "Fists of Fire",
            "Claws of Thunder",
            "Blades of Ice",
            "Cobra Strike",
            "Royal Strike",
            "Dragon Talon",
            "Dragon Claw",
            "Dragon Tail",
            "Dragon Flight",
        ] {
            w.learn_skill(code).unwrap_or_else(|e| panic!("{code} : {e}"));
        }
        assert!(w.spawn_monster("FA", 30, 20));
        let royal_idx = w.hero_skills().iter().position(|s| s.code == "Royal Strike").unwrap();
        let fin_idx = w.hero_skills().iter().position(|s| s.code == "Dragon Flight").unwrap();
        // Cycle 1 : 1 charge → météor feu 20 (royalstrikemeteor EMin, L565).
        w.select_skill(royal_idx);
        melee_hit(&mut w, 0);
        w.select_skill(fin_idx);
        let hp0 = w.mon_dat[0].hp;
        melee_hit(&mut w, 0);
        assert!(w.mon_dat[0].hp < hp0, "météor feu appliqué");
        w.drain_ad_events(); // isole le cycle 2
        // Cycle 2 : 2 charges → météor + chaîne de foudre (EMin=1, L570).
        w.select_skill(royal_idx);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        w.select_skill(fin_idx);
        melee_hit(&mut w, 0);
        assert!(matches!(
            w.drain_ad_events().iter().find(|e| matches!(e, AdEvent::ComboRelease { .. })),
            Some(AdEvent::ComboRelease { skill: "Royal Strike", charges: 2, .. })
        ), "charges 1+2 libérées ensemble");
    }

    #[test]
    fn combo_tiger_phys_bonus_scales_weapon_base() {
        let mut w = world_ass(0xD2C0_6008);
        level_up(&mut w, 30, 1);
        w.learn_skill("Tiger Strike").unwrap();
        assert!(w.spawn_monster("FA", 30, 20));
        let tiger_idx = w.hero_skills().iter().position(|s| s.code == "Tiger Strike").unwrap();
        w.select_skill(tiger_idx);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        melee_hit(&mut w, 0);
        // ln12 niv 1 = 100 % PAR charge × 3 charges = +300 % de la base (10).
        assert_eq!(scaled_hero_melee(&w, 50, 10), 80, "50 + 10×3");
        // 0 charge : inchangé (comportement historique bit-identique).
        w.ad.combos.clear();
        assert_eq!(scaled_hero_melee(&w, 50, 10), 50);
    }

    #[test]
    fn combo_tohit_bonus_per_charge() {
        let mut w = world_ass(0xD2C0_6009);
        assert_eq!(hero_to_hit_bonus(&w), 0);
        w.ad.combos.push(ComboState { skill: "Tiger Strike", charges: 2, ttl_ms: 9_000 });
        w.ad.combos.push(ComboState { skill: "Fists of Fire", charges: 3, ttl_ms: 9_000 });
        assert_eq!(hero_to_hit_bonus(&w), (2 + 3) * 50, "par4=50 par charge (progressive_tohit)");
    }

    // -- Shock Field (« Shock Web ») : volée de carreaux ---------------------

    #[test]
    fn shock_field_volley_counts_bolts() {
        let mut w = world_ass(0xD2A5_5005);
        level_up(&mut w, 12, 2);
        w.learn_skill("Fire Trauma").unwrap();
        w.learn_skill("Shock Field").unwrap();
        // prgcalc1 = 6 + lvl/4 + Fire Trauma blvl/3 (L258, division entière).
        assert!(w.spawn_monster("FA", 30, 20));
        assert!(w.spawn_monster("FA", 31, 20));
        let cast = w.cast_ad_skill("Shock Field", Some((30, 20))).expect("cast Shock Field");
        // 6 carreaux réels (division entière : 1/4 = 0, 1/3 = 0) ; 2 cibles
        // dans le rayon ⇒ au plus 2 touchés.
        assert!(matches!(cast, AdCast::ShockWeb { bolts: 6, hits } if hits <= 2));
    }

    #[test]
    fn cast_guards_not_learned_wrong_class_mana() {
        let mut w = world_ass(0xD2A5_5006);
        assert_eq!(w.cast_ad_skill("Lightning Sentry", None), Err(AdError::NotLearned));
        assert_eq!(w.cast_ad_skill("N'importe Quoi", None), Err(AdError::UnknownSkill));
        assert_eq!(w.cast_ad_skill("Fire Bolt", None), Err(AdError::NotLearned), "hors périmètre");
        // Druide : les pièges restent hors de sa classe.
        let mut d = world_dru(0xD2A5_5007);
        level_up(&mut d, 30, 1);
        d.learn_skill("Wearwolf").unwrap();
        assert!(matches!(
            d.cast_ad_skill("Blade Sentinel", None),
            Err(AdError::NotLearned) | Err(AdError::WrongClass)
        ));
        // Mana insuffisant : LS coûte 20 (L273).
        let mut w2 = world_ass(0xD2A5_5008);
        level_up(&mut w2, 24, 4);
        w2.learn_skill("Fire Trauma").unwrap();
        w2.learn_skill("Shock Field").unwrap();
        w2.learn_skill("Charged Bolt Sentry").unwrap();
        w2.learn_skill("Lightning Sentry").unwrap();
        w2.players[0].data.stats.mana = 5;
        assert_eq!(
            w2.cast_ad_skill("Lightning Sentry", None),
            Err(AdError::NotEnoughMana { cost: 20 })
        );
    }

    // -- Transformations druide : buff / expiration / knockback --------------

    #[test]
    fn wearwolf_buff_and_expiry() {
        let mut w = world_dru(0xD2D2_7001);
        level_up(&mut w, 30, 1);
        w.learn_skill("Wearwolf").unwrap();
        let base_max = w.hero_stats().max_life;
        let cast = w.cast_ad_skill("Wearwolf", None).expect("cast Wearwolf");
        assert_eq!(
            cast,
            AdCast::Transform { form: AdForm::Wolf, duration_ms: 80_000 },
            "1000 + SS.ln12(0→clamp 1000) frames × 40 ms"
        );
        assert_eq!(w.ad_form(), Some(AdForm::Wolf));
        assert_eq!(AdForm::Wolf.token(), "wolf", "hook sprite étape 15");
        // Vie max : +25 % (par2) + 20 % (SS ln34 sans point → clamp niv 1).
        assert_eq!(w.hero_stats().max_life, base_max + base_max * 45 / 100);
        assert_eq!(w.hero_stats().life, w.hero_stats().max_life, "la vie courante suit le max");
        // Expiration exacte : max_life redevient la base.
        run(&mut w, 80_100);
        assert_eq!(w.ad_form(), None);
        assert_eq!(w.hero_stats().max_life, base_max, "bonus retiré exactement");
        assert!(matches!(
            w.drain_ad_events().iter().rfind(|e| matches!(e, AdEvent::TransformEnded { .. })),
            Some(AdEvent::TransformEnded { form: AdForm::Wolf })
        ));
    }

    #[test]
    fn lycanthropy_extends_duration_and_life() {
        let mut w = world_dru(0xD2D2_7002);
        level_up(&mut w, 30, 3);
        w.learn_skill("Wearwolf").unwrap();
        w.learn_skill("Shape Shifting").unwrap();
        w.learn_skill("Shape Shifting").unwrap(); // Lycanthropy niv 2
        let base_max = w.hero_stats().max_life;
        let AdCast::Transform { duration_ms, .. } = w.cast_ad_skill("Wearwolf", None).unwrap()
        else {
            panic!("transformation attendue")
        };
        // SS niv 2 : ln12 = 1000 + 1×500 = 1500 ⇒ (1000+1500)×40 = 100 s.
        assert_eq!(duration_ms, 100_000, "auralencalc réel avec Lycanthropy niv 2");
        // Vie : par2 25 + ln34 (20 + 5×1) = 50 %.
        assert_eq!(w.hero_stats().max_life, base_max + base_max / 2);
        run(&mut w, 100_100);
        assert_eq!(w.hero_stats().max_life, base_max);
    }

    #[test]
    fn recast_same_form_reverts_switch_direct() {
        let mut w = world_dru(0xD2D2_7003);
        level_up(&mut w, 30, 2);
        w.learn_skill("Wearwolf").unwrap();
        w.learn_skill("Wearbear").unwrap();
        let base_max = w.hero_stats().max_life;
        w.cast_ad_skill("Wearwolf", None).unwrap();
        // Re-cast Loup → retour humain (règle D2).
        assert_eq!(w.cast_ad_skill("Wearwolf", None).unwrap(), AdCast::RevertHuman);
        assert_eq!(w.ad_form(), None);
        assert_eq!(w.hero_stats().max_life, base_max);
        // Switch direct Loup → Ours (formes exclusives).
        w.cast_ad_skill("Wearwolf", None).unwrap();
        w.cast_ad_skill("Wearbear", None).unwrap();
        assert_eq!(w.ad_form(), Some(AdForm::Bear));
        assert_eq!(AdForm::Bear.token(), "bear", "hook sprite étape 15");
        assert!(matches!(
            w.drain_ad_events().iter().find(|e| matches!(e, AdEvent::TransformEnded { .. })),
            Some(AdEvent::TransformEnded { form: AdForm::Wolf })
        ));
    }

    #[test]
    fn werebear_melee_mult_defense_and_knockback() {
        let mut w = world_dru(0xD2D2_7004);
        level_up(&mut w, 30, 1);
        w.learn_skill("Wearbear").unwrap();
        w.ad.form = Some(AdFormState {
            form: AdForm::Bear,
            level: 1,
            ttl_ms: 80_000,
            life_bonus_applied: 0,
        });
        // Multiplicateur dégâts niv 1 : ln12 = 50 % (damagepercent, L230).
        assert_eq!(scaled_hero_melee(&w, 100, 10), 150);
        // Défense : +25 % (skill_armor_percent ln34).
        assert_eq!(hero_defense_bonus(&w, 40), 10);
        // Knockback : le monstre à 1 tuile Est est repoussé d'1 tuile.
        assert!(w.spawn_monster("FA", 31, 25));
        w.players[0].ent.x = 30.5;
        w.players[0].ent.y = 25.5;
        w.mon_ent[0].x = 31.5;
        w.mon_ent[0].y = 25.5;
        crate::assassin_druid::note_hero_melee_hit(&mut w, 0, 10, 10);
        assert!((w.mon_ent[0].x - 32.5).abs() < 0.01, "repoussé d'1 tuile à l'Est");
        assert!(matches!(
            w.drain_ad_events().iter().find(|e| matches!(e, AdEvent::Knockback { .. })),
            Some(AdEvent::Knockback { target: 0 })
        ));
        // Hors forme : aucun effet (zéro régression).
        w.ad.form = None;
        assert_eq!(scaled_hero_melee(&w, 100, 10), 100);
        assert_eq!(hero_defense_bonus(&w, 40), 0);
        assert_eq!(hero_swing_ms(&w), crate::world::HERO_SWING_MS);
    }

    #[test]
    fn werewolf_speeds_up_swing_bear_does_not() {
        let mut w = world_dru(0xD2D2_7005);
        level_up(&mut w, 30, 6);
        w.learn_skill("Wearwolf").unwrap();
        w.learn_skill("Wearbear").unwrap();
        assert_eq!(hero_swing_ms(&w), crate::world::HERO_SWING_MS, "humain : swing normal");
        // Loup niv 1 : dm34 = 10 % ⇒ 800×100/110 = 727 ms.
        w.cast_ad_skill("Wearwolf", None).unwrap();
        assert_eq!(
            hero_swing_ms(&w),
            (crate::world::HERO_SWING_MS as u64 * 100 / 110) as u32
        );
        // Loup niv 5 : dm34 = 10+80×4 = 330 → clamp 100 ⇒ plancher ×0.5.
        for _ in 0..4 {
            w.grant_skill_points(1);
            w.learn_skill("Wearwolf").unwrap();
        }
        assert_eq!(hero_swing_ms(&w), crate::world::HERO_SWING_MS / 2);
        // Ours : PAS de stat attackrate (L230) ⇒ swing normal.
        w.cast_ad_skill("Wearbear", None).unwrap();
        assert_eq!(hero_swing_ms(&w), crate::world::HERO_SWING_MS);
    }

    // -- Déterminisme + persistance -------------------------------------------

    #[test]
    fn determinism_same_seed_same_trace() {
        let trace = |seed: u64| -> Vec<String> {
            let mut w = world_ass(seed);
            level_up(&mut w, 24, 4);
            w.learn_skill("Fire Trauma").unwrap();
            w.learn_skill("Shock Field").unwrap();
            w.learn_skill("Charged Bolt Sentry").unwrap();
            w.learn_skill("Lightning Sentry").unwrap();
            assert!(w.spawn_monster("FA", 30, 20));
            assert!(w.spawn_monster("FA", 33, 22));
            w.cast_ad_skill("Lightning Sentry", None).unwrap();
            run(&mut w, 5_000);
            let mut out: Vec<String> =
                w.drain_ad_events().iter().map(|e| format!("{e:?}")).collect();
            out.push(format!("hp={:?}", w.monsters().iter().map(|m| (m.x, m.y)).collect::<Vec<_>>()));
            out
        };
        assert_eq!(trace(0xD2DE_AD01), trace(0xD2DE_AD01), "même graine ⇒ même trace");
        assert_ne!(trace(0xD2DE_AD01), trace(0xD2DE_AD02), "graines différentes ⇒ traces distinctes");
    }

    #[test]
    fn save_load_stays_valid_ad_transient() {
        // L'état AD est transitoire : un cycle save/load reste VALIDE (ad
        // vide au rechargement — même contrat que l'IA P1).
        let mut w = world_ass(0xD2A5_5009);
        level_up(&mut w, 12, 3);
        w.learn_skill("Fire Trauma").unwrap();
        w.learn_skill("Shock Field").unwrap();
        w.learn_skill("Charged Bolt Sentry").unwrap();
        w.cast_ad_skill("Charged Bolt Sentry", None).unwrap();
        let json = w.save();
        let w2 = World::load(&json).expect("save/load valide avec le champ ad");
        assert!(w2.ad_traps().is_empty(), "pièges transitoires non persistés");
    }
}
