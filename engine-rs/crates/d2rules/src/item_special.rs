//! `item_special` — étape 17c « items fin de chaîne » (PLAN-FIDELITÉ
//! phase 17) : les quatre familles d'items spéciaux qui terminent la
//! chaîne d'items. PUR (zéro IO, zéro wasm), additif sur l'arbre.
//!
//! # ① CRAFTED — recettes cubemain `usetype,crf`
//! Les 36 recettes de craft RÉELLES de `assets/patch_cubemain.txt`
//! (LA TABLE FAIT FOI) : Hitpower L66-74 (perfect sapphire), Blood L75-83
//! (perfect ruby), Caster L84-92 (perfect amethyst), Safety L93-101
//! (perfect emerald) — 9 par famille, patterns « magic + jewel + rune +
//! gemme parfaite ». AUCUNE colonne `craftratings` dans le fichier réel
//! v1.12 : les mods fixes viennent directement des colonnes `mod 1..4` de
//! chaque recette ([`crate::cube::CubeOutput::mods`], parsées par le cube
//! 11a et non appliquées — ce module les applique).
//! * Base : [`HoradricCube::transmute`] (même code que l'input slot 1 —
//!   `usetype`, qualité Crafted, champs de base des tables réelles).
//! * **Mods fixes** = `mod 1..4` de la recette, roulés puis évalués par
//!   [`crate::item_props`] (même convention que les procs d'uniques :
//!   pour `gethit-skill`/`hit-skill`… `min` = chance %, `max` = niveau —
//!   calibré sur les données : `of Firebolts` MagicSuffix L420
//!   min 10/max 8 = « 10 % chance, level 8 »). Hitpower L66 :
//!   `gethit-skill` param 44 min 5 max 4 = « 5 % de caster niveau 4
//!   Frost Nova quand touché » (Frost Nova = `Id` 44 de
//!   `patch_skills.txt`, golden test).
//! * **1-4 affixes roulés**, budget par ilvl (règle retail « Crafting
//!   Basics », Arreat Summit — site Blizzard officiel HORS LIGNE ; miroirs
//!   fan : Basin Wiki « Crafted Items ») : ilvl 1-30 → 1, 31-50 → 2,
//!   51-70 → 3, 71+ → 4 ([`affix_budget`]) ; **ilvl = niveau du héros**
//!   (règle retail : les affixes se tirent au niveau du crafter). Le
//!   fichier réel porte plvl=50/ilvl=50 sur les 36 lignes — stockés non
//!   appliqués comme au cube 11a (divergence documentée : la sémantique
//!   exacte de ces colonnes n'est pas publique, la règle retail du brief
//!   fait foi). Pools = MagicPrefix/Suffix filtrés type + fenêtre
//!   [`crate::affixes::matches`] au niveau héros ; groupes exclusifs par
//!   côté ; pondération `frequency`. Flux [`Rng`] GELÉ par slot :
//!   ① côté (prefix/suffix, un tirage) ② pick pondéré ③ roll des valeurs
//!   (prefixes puis suffixes, ordre de tirage).
//! * Nom : `« <famille> <base> »` — clé TBL de la famille si elle existe
//!   (sondage réel : AUCUNE clé « Hitpower »/« Blood »/« Caster »/
//!   « Safety » dans string/expansionstring/patchstring fr v1.12 ⇒ repli
//!   libellé anglais) ; base fr = clé TBL du code (`fhl` → forme
//!   singulière), repli nom anglais.
//! * Les 2 recettes `blun`/`rod` (L74/L92) ne matchent jamais (groupes
//!   absents des tables embarquées — divergence déjà documentée cube.rs).
//!
//! # ② ÉTHEREAL — flag au drop
//! AUCUNE colonne éthéré dans weapons/armor/misc v1.12 (en-têtes sondés :
//! seules `nodurability`/`durability` existent) ⇒ règle 1/20 (5 %) :
//! [`roll_ethereal`], source Amazon Basin Wiki « Ethereal Items » (wiki
//! fan, le site officiel Arreat Summit est hors ligne) — règle v1.10+
//! confirmée par les données : éligible ⇔ durabilité réelle > 0
//! ([`can_be_ethereal`], donc armes/armures ; bagues, amulettes, joyaux,
//! misc et INDESTRUCTIBLES exclus). Effets (règles retail, même source) :
//! durabilité ÷2 plancher 1 ([`ethereal_durability`]), +50 % base
//! défense ([`ethereal_defense`]) / dégâts ([`ethereal_damage`], le
//! `%` des affixes s'applique ensuite PAR-DESSUS), NON RÉPARABLE chez le
//! forgeron ([`crate::economy`] : variante additive
//! `EconError::EtherealNotRepairable` + message). Socketable OUI — les
//! recettes de socketing L125-130 n'exigent PAS `noe` (seules les
//! recettes réparation/recharge L139-142 portent `noe`, cohérent avec
//! l'interdit de réparation) ; recharge des charges autorisée.
//!
//! # ③ CHARGES D'USAGE — skills sur item (`charged`)
//! Structure [`Charges`] (max/actuelles + skill/niveau) construite depuis
//! la structure minimale [`crate::item_props::SkillProp`] (convention Go
//! testée : charges = `min`, niveau = `max` — Marrowwalk : 13 charges de
//! Bone Prison niveau 33). Décrément pur [`use_charge`] — HOOK
//! DOCUMENTÉ pour l'intégrateur : brancher sur la consommation de
//! charges au cast (aucun runtime ici, pattern 12b). Recharge chez le
//! vendeur [`recharge`] : coût linéaire `floor(coût × (max − cur)/max)`
//! — ASSUMÉ, miroir documenté de [`crate::economy::repair_cost`] (même
//! statut : formule exacte du retail non publique) ; l'éthéré se
//! RECHARGE (seule la réparation est refusée).
//!
//! # ④ PERSONNALISÉ + EAR
//! * Personnalisation : [`personalize`] = `« <nom>'s <base> »` — le nom
//!   est stocké tel quel dans la save (bit 24 `personalized`,
//!   `d2d2s/src/d2r.rs:444`) ; le `'s` est le littéral retail d'affichage
//!   (le stocké = le nom seul, parse d2s cité).
//! * EAR : [`Ear`] = classe 3 bits + NIVEAU du héros mort 7 bits + nom —
//!   encodage du bitstream d2s réel (`d2d2s/src/d2r.rs:327-342`, code
//!   `ear`).
//!
//! # Branchement (contrat documenté, comme economy.rs)
//! * Craft : [`craft`] prend un cube [`HoradricCube`] (11a) — l'UI/d2game
//!   branchera sur la transmutation des 36 recettes craft.
//! * Éthéré : [`roll_ethereal`] au drop (chaîne `loot`/`item_roll`), les
//!   boosts s'appliquent aux champs de base du [`crate::item_roll::RolledItem`].
//! * Charges : hook [`use_charge`] ; recharge via l'API vendor F10.
//! * Pas d'UI — restant, comme les étapes précédentes.

#![allow(dead_code)] // contrat consommé par d2game à l'intégration (pattern item_roll/cube)

use crate::affixes::{self, AffixRec, AffixTables, ItemTypeSet};
use crate::cube::{CubeItem, CubeMod, CubeRecipe, HoradricCube};
use crate::economy::EconError;
use crate::item_props::{self, PropDescriptor, PropEval, SkillProp, SkillPropKind};
use crate::item_roll::{resolve_affix, RolledItem, RolledQuality};
use crate::rng::Rng;
use crate::tbl::TblSet;
use std::collections::HashMap;
use std::sync::OnceLock;

// ---------------------------------------------------------------------------
// ① CRAFTED — familles, budget d'affixes, moteur
// ---------------------------------------------------------------------------

/// Les 4 familles de craft v1.12 (mots réels des `description` L66-101).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CraftFamily {
    /// `hitpower` (perfect sapphire) — L66-74.
    Hitpower,
    /// `blood` (perfect ruby) — L75-83.
    Blood,
    /// `caster` (perfect amethyst) — L84-92.
    Caster,
    /// `safety` (perfect emerald) — L93-101.
    Safety,
}

impl CraftFamily {
    /// Libellé anglais (repli : aucune clé TBL fr v1.12 pour ces familles
    /// — sondage cité en tête de module).
    pub fn label(self) -> &'static str {
        match self {
            CraftFamily::Hitpower => "Hitpower",
            CraftFamily::Blood => "Blood",
            CraftFamily::Caster => "Caster",
            CraftFamily::Safety => "Safety",
        }
    }
}

/// Famille d'une recette craft, lue sur sa `description` réelle
/// (« … -> hitpower helm »). `None` si aucun mot de famille.
pub fn family_of(description: &str) -> Option<CraftFamily> {
    let d = description.to_ascii_lowercase();
    if d.contains("hitpower") {
        Some(CraftFamily::Hitpower)
    } else if d.contains("blood") {
        Some(CraftFamily::Blood)
    } else if d.contains("caster") {
        Some(CraftFamily::Caster)
    } else if d.contains("safety") {
        Some(CraftFamily::Safety)
    } else {
        None
    }
}

/// Budget d'affixes roulés d'un craft (règle retail « Crafting Basics »,
/// Arreat Summit hors ligne — miroirs fan cités en tête de module) :
/// ilvl 1-30 → 1, 31-50 → 2, 51-70 → 3, 71+ → 4.
pub fn affix_budget(ilvl: u32) -> usize {
    match ilvl {
        0..=30 => 1,
        31..=50 => 2,
        51..=70 => 3,
        _ => 4,
    }
}

/// Un mod FIXE de recette, roulé puis évalué ([`crate::item_props`]).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FixedProp {
    /// Descripteur source (colonnes `mod N`/`param`/`min`/`max` réelles).
    pub desc: PropDescriptor,
    /// Valeur tirée (insignifiante pour les procs skill : chance = `min`,
    /// niveau = `max` — cf. [`crate::item_props::evaluate`]).
    pub value: i32,
    /// Émission typée (stats, accroches skill — pour l'intégrateur).
    pub eval: PropEval,
}

/// Un item CRAFTED résolu : base (contrat gelé [`RolledItem`], qualité
/// Crafted, `affix_ids` = les 1-4 affixes roulés) + mods fixes de la
/// recette + famille + ilvl (= niveau du héros, doc module).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Crafted {
    /// Ligne 1-based de la recette dans `patch_cubemain.txt` (revue).
    pub recipe_line: usize,
    /// Famille (mot réel de la description).
    pub family: CraftFamily,
    /// Niveau du héros au craft — SERT de fenêtre d'affixes (règle retail,
    /// divergence plvl/ilvl=50 documentée en tête de module).
    pub ilvl: u32,
    /// Base + affixes roulés (contrat gelé coéquipiers).
    pub item: RolledItem,
    /// Mods fixes de la recette (dans l'ordre des colonnes `mod 1..4`).
    pub fixed: Vec<FixedProp>,
}

/// Types des familles armes exclues de `mele` (miroir item_roll.rs).
const NON_MELEE: &[&str] = &["bow", "xbow", "abow", "jave", "ajav", "tkni", "tpot"];
/// Familles d'armes à distance (miroir item_roll.rs).
const BOW_TYPES: &[&str] = &["bow", "xbow", "abow"];
/// Types portés par patch_armor.txt (miroir item_roll.rs).
const ARMOR_TYPES: &[&str] = &[
    "ashd", "belt", "boot", "circ", "glov", "head", "helm", "pelt", "phlm", "shie", "tors",
];

/// `code` de base → [`ItemTypeSet`] (type réel des tables v1.12, cache
/// [`OnceLock`] — zéro IO ; même forme que `item_roll::ItemRoller::type_set`).
/// MISC parsé aussi : bagues/amulettes/joyaux ont LEUR type réel
/// (`rin` → `ring`, `amu` → `amul`, `jew` → `jewl`) — nécessaire aux
/// pools d'affixes (itypes des tables).
fn type_set_of(code: &str) -> ItemTypeSet {
    static TABLE: OnceLock<HashMap<String, (&'static str, bool, bool, bool)>> = OnceLock::new();
    // (wtype interné, is_weap, is_bow, is_armo) — parse paresseux des 3 tables.
    let table = TABLE.get_or_init(|| {
        let mut out = HashMap::new();
        let leak = |t: &str| -> &'static str { Box::leak(t.to_string().into_boxed_str()) };
        for (txt, weapon) in [
            (
                include_str!("../../../../assets/patch_weapons.txt"),
                true,
            ),
            (include_str!("../../../../assets/patch_armor.txt"), false),
            (include_str!("../../../../assets/patch_misc.txt"), false),
        ] {
            let Ok(t) = crate::tsv::Tsv::parse(txt) else { continue };
            for i in 0..t.len() {
                let (Some(c), Some(ty)) = (t.cell(i, "code"), t.cell(i, "type")) else {
                    continue;
                };
                if c.is_empty() || ty.is_empty() || out.contains_key(c) {
                    continue;
                }
                let is_weap = weapon && crate::loot::is_weapon_type(ty);
                let is_bow = is_weap && BOW_TYPES.contains(&ty);
                let is_armo = !weapon && ARMOR_TYPES.contains(&ty);
                out.insert(c.to_string(), (leak(ty), is_weap, is_bow, is_armo));
            }
        }
        out
    });
    let (wtype, is_weap, is_bow, is_armo) =
        table.get(code).copied().unwrap_or(("", false, false, false));
    ItemTypeSet {
        wtype,
        is_weap,
        is_mele: is_weap && !NON_MELEE.contains(&wtype),
        is_bow,
        is_armo,
    }
}

/// Pick pondéré par `frequency` (miroir `affixes::weighted`, privé là-bas ;
/// None si vide). Un tirage [`Rng`].
fn weighted<'a>(rng: &mut Rng, cands: &[&'a AffixRec]) -> Option<&'a AffixRec> {
    if cands.is_empty() {
        return None;
    }
    let total: u64 = cands.iter().map(|a| a.frequency.max(1) as u64).sum();
    let mut pick = (rng.below(total as u32) as u64).min(total - 1);
    for a in cands {
        let w = a.frequency.max(1) as u64;
        if pick < w {
            return Some(a);
        }
        pick -= w;
    }
    None
}

/// Les libellés de craft réels (`output.quality == Crafted`).
fn is_craft_recipe(rec: &CubeRecipe) -> bool {
    rec.output.quality == Some(RolledQuality::Crafted)
}

/// Nom fr d'un craft : `« <famille> <base> »` (ordre du brief). Famille :
/// clé TBL si elle existe, sinon libellé anglais ; base : clé TBL du code
/// (forme singulière, parse [`affixes::parse_gendered`]), sinon le nom
/// anglais posé par le cube.
fn craft_name(strings: &TblSet, family: CraftFamily, item: &RolledItem) -> String {
    let fam = strings.get(family.label()).unwrap_or_else(|| family.label().to_string());
    let base_raw = strings.get(&item.code).unwrap_or_default();
    let base = if base_raw.is_empty() {
        item.name_resolved.clone()
    } else {
        affixes::parse_gendered(&base_raw).first().to_string()
    };
    format!("{fam} {base}").trim().to_string()
}

/// Spécialise un item de sortie du cube (`usetype,crf`) : mods fixes de la
/// recette + 1-4 affixes roulés au niveau du héros + nom `« <famille>
/// <base> »`. Les tirages consommés sur `rng` sont DÉTERMINISTES (ordre
/// gelé : mods fixes → affixes par slot).
fn specialize(
    rec: &CubeRecipe,
    mut item: RolledItem,
    hero_level: u32,
    tables: &AffixTables,
    strings: &TblSet,
    rng: &mut Rng,
) -> Crafted {
    // Aucune des 36 lignes réelles n'a de description sans mot de famille ;
    // repli défensif Hitpower (jamais paniquer).
    let family = family_of(&rec.description).unwrap_or(CraftFamily::Hitpower);
    // ① Mods fixes (colonnes mod 1..4 réelles) : chance éventuelle, roll,
    //    évaluation typée (procs : chance = min, niveau = max).
    let mut fixed = Vec::new();
    for m in &rec.output.mods {
        if let Some(chance) = m.chance {
            if chance < 100 && rng.below(100) >= chance.max(0) as u32 {
                continue;
            }
        }
        fixed.push(fixed_prop(m, rng));
    }
    // ② Affixes roulés : budget = bandes retail au NIVEAU DU HÉROS ;
    //    pools filtrés type + fenêtre au même niveau ; groupes exclusifs
    //    par côté ; flux gelé : côté → pick → valeurs.
    let types = type_set_of(&item.code);
    let pool = affixes::pools(tables, &types, hero_level);
    let mut used_p: Vec<i32> = Vec::new();
    let mut used_s: Vec<i32> = Vec::new();
    item.affix_ids.clear();
    for _ in 0..affix_budget(hero_level) {
        // côté : 0 = prefix, 1 = suffix (flux gelé, doc module).
        let suffix_side = rng.below(2) == 1;
        let mut pick_one = |suffix: bool, used: &mut Vec<i32>| -> Option<&AffixRec> {
            let (side, other) = if suffix { (&pool.suffixes, &pool.prefixes) } else { (&pool.prefixes, &pool.suffixes) };
            let cands: Vec<&AffixRec> =
                side.iter().copied().filter(|a| !used.contains(&a.group)).collect();
            let pick = weighted(rng, &cands).or_else(|| {
                let alt: Vec<&AffixRec> =
                    other.iter().copied().filter(|a| !used.contains(&a.group)).collect();
                weighted(rng, &alt)
            })?;
            used.push(pick.group);
            Some(pick)
        };
        let Some(rec_a) = (if suffix_side { pick_one(true, &mut used_s) } else { pick_one(false, &mut used_p) })
        else {
            continue; // deux côtés à sec : slot perdu (pools épuisés)
        };
        let Some(id) = affixes::row_id(tables, rec_a) else { continue };
        for d in &rec_a.mods {
            let value = item_props::roll(d, rng).value.max(0) as u32;
            let param: u32 = d.param.trim().parse().unwrap_or(0);
            if d.code == "sock" {
                let eff = if param > 0 { param } else { value };
                item.sockets = eff.min(u8::MAX as u32) as u8;
            }
            item.affix_ids.push((id, value));
        }
    }
    // ③ Nom fr « <famille> <base> ».
    item.name_resolved = craft_name(strings, family, &item);
    Crafted { recipe_line: rec.line, family, ilvl: hero_level, item, fixed }
}

/// Roll + évaluation d'UN mod fixe (`CubeMod` du cube 11a).
fn fixed_prop(m: &CubeMod, rng: &mut Rng) -> FixedProp {
    let desc = PropDescriptor {
        code: m.code.clone(),
        param: m.param.clone(),
        min: m.min,
        max: m.max,
    };
    let value = item_props::roll(&desc, rng).value;
    let eval = item_props::evaluate(&desc, value);
    FixedProp { desc, value, eval }
}

/// Tente UNE transmutation de CRAFT sur les objets posés dans le cube :
/// première recette compatible (ordre du fichier — premier match gagne,
/// moteur 11a), puis spécialisation [`specialize`]. `None` si aucune
/// recette (ou une recette NON craft — tirages consommés, comme
/// [`HoradricCube::transmute`] ; avec des inputs « magic + jewel + rune +
/// gemme » seules les 36 recettes craft peuvent matcher).
pub fn craft(
    cube: &HoradricCube,
    items: &[CubeItem],
    hero_level: u32,
    tables: &AffixTables,
    strings: &TblSet,
    rng: &mut Rng,
) -> Option<Crafted> {
    let outcome = cube.transmute(items, rng)?;
    let rec = cube.recipes().iter().find(|r| r.line == outcome.line)?;
    if !is_craft_recipe(rec) {
        return None;
    }
    let item = outcome.item?;
    Some(specialize(rec, item, hero_level, tables, strings, rng))
}

// ---------------------------------------------------------------------------
// ② ÉTHEREAL — flag au drop (1/20), boosts, non-réparable
// ---------------------------------------------------------------------------

/// Diviseur du jet éthéré au drop : 1 chance sur 20 (5 %) — Amazon Basin
/// Wiki « Ethereal Items » (wiki fan ; Arreat Summit hors ligne), règle
/// v1.10+ ; AUCUNE colonne éthéré dans les tables v1.12 (sondage en-tête).
pub const ETHEREAL_DIVISOR: u32 = 20;

/// Le code peut-il spawn éthéré ? Règle données : durabilité réelle > 0
/// (armes/armures des tables v1.12 — [`crate::economy::durability_of`]) ;
/// bagues/amulettes/joyaux/misc (dur 0) et INDESTRUCTIBLES (`nodurability`
/// col réelle ⇒ dur 0) exclus — fidèle au retail.
pub fn can_be_ethereal(code: &str) -> bool {
    crate::economy::durability_of(code) > 0
}

/// Jet éthéré au drop : 1/20 pour un code éligible, JAMAIS sinon. Un
/// tirage [`Rng`] (consommé même pour un code non éligible ? NON : aucun
/// tirage consommé si [`can_be_ethereal`] est faux — flux minimal).
pub fn roll_ethereal(rng: &mut Rng, code: &str) -> bool {
    can_be_ethereal(code) && rng.below(ETHEREAL_DIVISOR) == 0
}

/// Boost éthéré +50 % sur une valeur de base : `v + v/2` (plancher — le
/// `%` des affixes s'applique PAR-DESSUS par la statlist).
pub fn ethereal_boost(v: u32) -> u32 {
    v + v / 2
}

/// Défense de base éthérée (+50 % sur l'AC de base roulé).
pub fn ethereal_defense(base_ac: u32) -> u32 {
    ethereal_boost(base_ac)
}

/// Dégâts de base éthérés (+50 % sur min et max indépendamment).
pub fn ethereal_damage(min: u32, max: u32) -> (u32, u32) {
    (ethereal_boost(min), ethereal_boost(max))
}

/// Durabilité éthérée : ÷2 PLANCHER 1 (le 0 = indestructible reste 0,
/// mais [`can_be_ethereal`] l'exclut déjà — défensif).
pub fn ethereal_durability(max_durability: u32) -> u32 {
    if max_durability == 0 {
        0
    } else {
        (max_durability / 2).max(1)
    }
}

// ---------------------------------------------------------------------------
// ③ CHARGES D'USAGE — max/actuelles, décrément hook, recharge vendeur
// ---------------------------------------------------------------------------

/// Charges d'usage d'un item (`charged`) : skill + niveau + max/actuelles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Charges {
    /// Id du skill (`patch_skills.txt` col `Id`).
    pub skill_id: i32,
    /// Niveau du skill chargé (col `max` — convention Go testée 12b).
    pub level: i32,
    /// Charges actuelles (max à la création / après recharge).
    pub cur: i32,
    /// Charges max (col `min` — convention Go testée 12b).
    pub max: i32,
}

/// Extrait les charges d'une accroche skill ([`SkillPropKind::Charged`]
/// uniquement) ; `None` pour procs/+skills (pas de charges).
pub fn charges_of(prop: &SkillProp) -> Option<Charges> {
    (prop.kind == SkillPropKind::Charged).then(|| Charges {
        skill_id: prop.skill_id,
        level: prop.level,
        cur: prop.charges,
        max: prop.charges,
    })
}

/// HOOK INTÉGRATEUR (documenté, aucun runtime) — consomme UNE charge à
/// l'usage : `true` si une charge était disponible (décrémentée), `false`
/// si épuisé (état intact). Branchement prévu : cast d'un skill chargé
/// depuis l'item (cf. `SkillProp` 12b, `fnChargeRelated` Go cité).
pub fn use_charge(c: &mut Charges) -> bool {
    if c.cur > 0 {
        c.cur -= 1;
        true
    } else {
        false
    }
}

/// Coût de recharge vendeur : `floor(coût × (max − cur) / max)` — linéaire
/// ASSUMÉ, miroir documenté de [`crate::economy::repair_cost`] (même
/// statut : la formule exacte du retail n'est pas publique) ; plein ou
/// `max` nul ⇒ 0.
pub fn recharge_cost(unit_cost: u32, cur: i32, max: i32) -> u32 {
    if max <= 0 || cur >= max {
        return 0;
    }
    (unit_cost as u64 * (max - cur) as u64 / max as u64) as u32
}

/// Résultat d'une recharge : coût payé + charges restaurées (= max).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RechargeOutcome {
    /// Coût débité.
    pub cost: u32,
    /// Charges après recharge (= max).
    pub cur: i32,
}

/// RECHARGE chez le vendeur : `cur → max` si l'or du héros suffit ; rien
/// ne bouge sinon (miroir [`crate::economy::repair`]). Plein ⇒
/// [`EconError::NothingToRepair`] (même sémantique « rien à faire »).
/// L'ÉTHÉRÉ se recharge (seule la RÉPARATION lui est refusée).
pub fn recharge(c: &mut Charges, unit_cost: u32, hero_gold: u32) -> Result<RechargeOutcome, EconError> {
    let price = recharge_cost(unit_cost, c.cur, c.max);
    if price == 0 {
        return Err(EconError::NothingToRepair);
    }
    if hero_gold < price {
        return Err(EconError::NotEnoughGold { need: price, have: hero_gold });
    }
    c.cur = c.max;
    Ok(RechargeOutcome { cost: price, cur: c.max })
}

// ---------------------------------------------------------------------------
// ④ PERSONNALISÉ + EAR
// ---------------------------------------------------------------------------

/// Personnalise un item : `« <nom>'s <base> »` (« Justin's Petit Poulet »).
/// Le nom stocké dans la save est le nom SEUL (bit `personalized`,
/// `d2d2s/src/d2r.rs:444`) ; le `'s` est le littéral retail d'affichage.
pub fn personalize(name: &str, base_name: &str) -> String {
    format!("{name}'s {base_name}")
}

/// Un EAR (oreille du héros mort) — encodage du bitstream d2s réel
/// (`d2d2s/src/d2r.rs:327-342`) : classe 3 bits, NIVEAU 7 bits, nom.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ear {
    /// Classe du héros mort (3 bits, 0..=7 — ordre d2s).
    pub class: u8,
    /// NIVEAU du héros mort (7 bits — le payload demandé par la mission ;
    /// clampé à 127).
    pub level: u8,
    /// Nom du héros mort (nom d'affichage de l'item).
    pub name: String,
}

/// Fabrique l'ear d'un héros mort : niveau encodé sur 7 bits (clamp 127 —
/// le max retail 99 tient largement).
pub fn make_ear(name: &str, class: u8, level: u8) -> Ear {
    Ear { class: class.min(7), level: level.min(127), name: name.to_string() }
}

impl Ear {
    /// Nom d'affichage : le nom du héros mort (c'est ce que stocke la
    /// save pour un ear — code `ear`, `d2d2s` cité) ; le suffixe « Ear »
    /// du libellé est UI-restant.
    pub fn display_name(&self) -> &str {
        &self.name
    }
}

// ---------------------------------------------------------------------------
// Tests — golden par famille, lignes de tables citées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::cube::CubeItem;

    const CUBEMAIN_TXT: &str = include_str!("../../../../assets/patch_cubemain.txt");
    const SKILLS_TXT: &str = include_str!("../../../../assets/patch_skills.txt");
    const PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
    const SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
    const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");

    fn cube() -> HoradricCube {
        HoradricCube::embedded().expect("cubemain v1.12 parsable")
    }

    fn affixes() -> AffixTables {
        affixes::parse(PREFIX_TXT, SUFFIX_TXT, AUTOMAGIC_TXT).expect("tables affixes")
    }

    fn strings() -> TblSet {
        TblSet::bundled_fra()
    }

    fn craft_lines<'a>(c: &'a HoradricCube) -> Vec<&'a CubeRecipe> {
        c.recipes().iter().filter(|r| is_craft_recipe(r)).collect()
    }

    // -- ① CRAFTED : les 36 recettes réelles, 4 familles × 9 -----------------

    /// GOLDEN — les 36 recettes craft réelles (`usetype,crf`) : lignes
    /// L66-74 hitpower, L75-83 blood, L84-92 caster, L93-101 safety
    /// (9 par famille) ; patterns « magic + jewel + rune + perfect gem »
    /// (4 inputs) ; plvl/ilvl = 50/50 sur TOUTES les lignes (stockés non
    /// appliqués — divergence documentée).
    #[test]
    fn craft_recipes_golden_36_lines_four_families() {
        let c = cube();
        let crf = craft_lines(&c);
        assert_eq!(crf.len(), 36, "36 recettes craft réelles v1.12");
        let fams = [CraftFamily::Hitpower, CraftFamily::Blood, CraftFamily::Caster, CraftFamily::Safety];
        for f in fams {
            let n = crf.iter().filter(|r| family_of(&r.description) == Some(f)).count();
            assert_eq!(n, 9, "{f:?} : 9 recettes");
        }
        // Lignes exactes (LA TABLE FAIT FOI) : la première de chaque famille.
        let first = |f: CraftFamily| crf.iter().find(|r| family_of(&r.description) == Some(f)).unwrap();
        assert_eq!(first(CraftFamily::Hitpower).line, 66, "hitpower helm");
        assert_eq!(first(CraftFamily::Blood).line, 75, "blood helm");
        assert_eq!(first(CraftFamily::Caster).line, 84, "caster helm");
        assert_eq!(first(CraftFamily::Safety).line, 93, "safety helm");
        // 4 inputs partout : base magic (+upg non discriminant), jewel,
        // rune, gemme parfaite ; numinputs = 4.
        for r in &crf {
            assert_eq!(r.numinputs, 4, "L{} : {}", r.line, r.description);
            assert_eq!(r.inputs.len(), 4);
            assert!(matches!(r.inputs[0].quality, Some(crate::cube::InputQuality::Mag)), "base magic");
            assert_eq!(r.inputs[1].code, "jew", "jewel");
            assert!(r.inputs[2].code.starts_with('r'), "rune rNN");
        }
        // plvl/ilvl stockés : 50/50 sur les 36 (données réelles).
        assert!(crf.iter().all(|r| r.plvl == 50 && r.ilvl == 50), "plvl=ilvl=50 partout");
        // Aucune colonne `craftratings` dans le fichier réel (le brief la
        // soupçonnait) : les mods fixes viennent de `mod 1..4`.
        assert!(!CUBEMAIN_TXT.contains("craftratings"), "pas de craftratings v1.12");
    }

    /// GOLDEN mods fixes par famille — la PREMIÈRE ligne de chaque famille,
    /// cellules réelles citées :
    /// * L66 hitpower helm : gethit-skill param 44 [min 5, max 4] =
    ///   « 5 % de caster niveau 4 Frost Nova quand touché » (Frost Nova =
    ///   `Id` 44 de patch_skills.txt — golden ci-dessous), thorns [3,7],
    ///   ac-miss [25,50] ;
    /// * L75 blood helm : lifesteal [1,3], hp [10,20], deadly [5,10] ;
    /// * L84 caster helm : regen-mana [4,10], mana [10,20], manasteal [1,4] ;
    /// * L93 safety helm : red-dmg [1,4], red-mag [1,2], res-ltng [5,10],
    ///   ac% [10,30] (4 mods fixes — la seule famille à mod 4).
    #[test]
    fn craft_fixed_mods_golden_per_family() {
        let c = cube();
        fn m(r: &CubeRecipe, i: usize) -> &crate::cube::CubeMod {
            &r.output.mods[i]
        }
        let line = |n: usize| c.recipes().iter().find(|r| r.line == n).unwrap();
        // Hitpower L66.
        let hp = line(66);
        assert_eq!((hp.output.mods[0].code.as_str(), hp.output.mods[0].param.as_str()), ("gethit-skill", "44"));
        assert_eq!((hp.output.mods[0].min, hp.output.mods[0].max), (5, 4), "min 5 = chance %, max 4 = niveau");
        assert_eq!((m(hp, 1).code.as_str(), m(hp, 1).min, m(hp, 1).max), ("thorns", 3, 7));
        assert_eq!((m(hp, 2).code.as_str(), m(hp, 2).min, m(hp, 2).max), ("ac-miss", 25, 50));
        assert_eq!(hp.output.mods.len(), 3);
        // Blood L75.
        let bl = line(75);
        assert_eq!((m(bl, 0).code.as_str(), m(bl, 0).min, m(bl, 0).max), ("lifesteal", 1, 3));
        assert_eq!((m(bl, 1).code.as_str(), m(bl, 1).min, m(bl, 1).max), ("hp", 10, 20));
        assert_eq!((m(bl, 2).code.as_str(), m(bl, 2).min, m(bl, 2).max), ("deadly", 5, 10));
        assert_eq!(bl.output.mods.len(), 3);
        // Caster L84.
        let ca = line(84);
        assert_eq!((m(ca, 0).code.as_str(), m(ca, 0).min, m(ca, 0).max), ("regen-mana", 4, 10));
        assert_eq!((m(ca, 1).code.as_str(), m(ca, 1).min, m(ca, 1).max), ("mana", 10, 20));
        assert_eq!((m(ca, 2).code.as_str(), m(ca, 2).min, m(ca, 2).max), ("manasteal", 1, 4));
        assert_eq!(ca.output.mods.len(), 3);
        // Safety L93 (4 mods).
        let sa = line(93);
        assert_eq!((m(sa, 0).code.as_str(), m(sa, 0).min, m(sa, 0).max), ("red-dmg", 1, 4));
        assert_eq!((m(sa, 1).code.as_str(), m(sa, 1).min, m(sa, 1).max), ("red-mag", 1, 2));
        assert_eq!((m(sa, 2).code.as_str(), m(sa, 2).min, m(sa, 2).max), ("res-ltng", 5, 10));
        assert_eq!((m(sa, 3).code.as_str(), m(sa, 3).min, m(sa, 3).max), ("ac%", 10, 30));
        assert_eq!(sa.output.mods.len(), 4, "safety : 4 mods fixes");
        // Frost Nova = Id 44 (patch_skills.txt col `Id`) — le param réel de
        // gethit-skill hitpower.
        let sk = crate::tsv::Tsv::parse(SKILLS_TXT).expect("patch_skills parsable");
        let id44 = (0..sk.len()).find_map(|i| {
            (sk.cell(i, "Id") == Some("44")).then(|| sk.cell(i, "skilldesc").unwrap_or("").to_string())
        });
        assert_eq!(id44.as_deref(), Some("frost nova"), "Id 44 = Frost Nova");
    }

    /// Budget d'affixes retail : bandes 1-30/31-50/51-70/71+ → 1/2/3/4.
    #[test]
    fn affix_budget_retail_bands() {
        assert_eq!(affix_budget(1), 1);
        assert_eq!(affix_budget(30), 1);
        assert_eq!(affix_budget(31), 2);
        assert_eq!(affix_budget(50), 2);
        assert_eq!(affix_budget(51), 3);
        assert_eq!(affix_budget(70), 3);
        assert_eq!(affix_budget(71), 4);
        assert_eq!(affix_budget(99), 4);
    }

    /// Inputs craft d'un helm hitpower (L66 réelle) : fhl magic + jewel +
    /// r06 + gpb (perfect sapphire).
    fn hitpower_helm_inputs() -> Vec<CubeItem> {
        vec![
            CubeItem { code: "fhl".into(), name: "Full Helm".into(), quality: RolledQuality::Magic, sockets: 0 },
            CubeItem::base("jew", "Jewel"),
            CubeItem::base("r06", "Ith Rune"),
            CubeItem::base("gpb", "Perfect Sapphire"),
        ]
    }

    /// Craft END-TO-END L66 : famille Hitpower, base fhl (champs de table),
    /// qualité Crafted, 3 mods fixes dont le proc « 5 % / niveau 4 /
    /// skill 44 » typé par [`crate::item_props`], affixes roulés au
    /// NIVEAU DU HÉROS (60 → 3), nom fr « Hitpower <base> ».
    #[test]
    fn craft_hitpower_helm_end_to_end() {
        let c = cube();
        let t = affixes();
        let s = strings();
        let mut rng = Rng::new(0x17C_001);
        let crf = craft(&c, &hitpower_helm_inputs(), 60, &t, &s, &mut rng).expect("L66 doit matcher");
        assert_eq!(crf.recipe_line, 66);
        assert_eq!(crf.family, CraftFamily::Hitpower);
        assert_eq!(crf.ilvl, 60);
        assert_eq!(crf.item.code, "fhl", "usetype : même base que le slot 1");
        assert_eq!(crf.item.quality, RolledQuality::Crafted);
        // AC de base roulé dans les bornes réelles de fhl (patch_armor.txt).
        let armo = crate::tsv::Tsv::parse(include_str!("../../../../assets/patch_armor.txt")).unwrap();
        let (min_ac, max_ac) = (0..armo.len())
            .find_map(|i| {
                (armo.cell(i, "code") == Some("fhl"))
                    .then(|| (armo.rec(i).unwrap().u("minac"), armo.rec(i).unwrap().u("maxac")))
            })
            .expect("fhl dans patch_armor.txt");
        assert!((min_ac..=max_ac).contains(&crf.item.ac), "ac {} ∈ [{min_ac},{max_ac}]", crf.item.ac);
        // Mods fixes : 3, dont le proc hitpower typé.
        assert_eq!(crf.fixed.len(), 3);
        let proc = &crf.fixed[0];
        assert_eq!(proc.desc.code, "gethit-skill");
        let sk: Vec<_> = proc.eval.skills.iter().collect();
        assert_eq!(sk.len(), 1);
        assert_eq!(sk[0].kind, SkillPropKind::OnGetHit);
        assert_eq!(sk[0].skill_id, 44, "Frost Nova");
        assert_eq!(sk[0].prob, 5, "chance = min réel");
        assert_eq!(sk[0].level, 4, "niveau = max réel");
        // Affixes roulés : budget niveau 60 = 3, chaque id résoluble et de
        // fenêtre ≤ niveau héros.
        assert_eq!(crf.item.affix_ids.len(), affix_budget(60));
        for &(id, v) in &crf.item.affix_ids {
            let (rec_a, _) = resolve_affix(&t, id).expect("id résoluble");
            assert!(rec_a.level <= 60, "fenêtre affixe {} > héros 60", rec_a.level);
            let ok = rec_a.mods.iter().any(|d| {
                v as i32 >= d.min.min(d.max) && v as i32 <= d.max.max(d.min)
            });
            assert!(ok, "valeur {v} hors bornes de {}", rec_a.name);
        }
        // Nom fr : « Hitpower <base fr de fhl> » (aucune clé TBL « Hitpower »
        // ⇒ libellé anglais, sondage cité en tête de module).
        let base_fr = {
            let raw = s.get("fhl").expect("clé TBL fhl");
            affixes::parse_gendered(&raw).first().to_string()
        };
        assert_eq!(crf.item.name_resolved, format!("Hitpower {base_fr}"));
    }

    /// Déterminisme : même seed ⇒ même Crafted champ par champ (AC, fixes,
    /// affixes, nom) ; seeds distincts ⇒ affixes potentiellement différents
    /// (couverture de tirage réelle).
    #[test]
    fn craft_is_deterministic_per_seed() {
        let c = cube();
        let t = affixes();
        let s = strings();
        let mut a = Rng::new(0xD2_C0DE);
        let mut b = Rng::new(0xD2_C0DE);
        let ca = craft(&c, &hitpower_helm_inputs(), 75, &t, &s, &mut a).unwrap();
        let cb = craft(&c, &hitpower_helm_inputs(), 75, &t, &s, &mut b).unwrap();
        assert_eq!(ca, cb, "même seed ⇒ identique");
        assert_eq!(ca.item.affix_ids.len(), 4, "budget niveau 75 = 4");
        // Un autre seed donne un tirage vivant (au moins l'AC ou un affixe
        // diffère sur 20 essais — le pool à 75 est large).
        let mut different = false;
        for seed in 1..20u64 {
            let mut r = Rng::new(seed * 0x9E37_79B9);
            let cx = craft(&c, &hitpower_helm_inputs(), 75, &t, &s, &mut r).unwrap();
            if cx.item.affix_ids != ca.item.affix_ids || cx.item.ac != ca.item.ac {
                different = true;
                break;
            }
        }
        assert!(different, "le tirage varie au moins une fois sur 20 seeds");
    }

    /// Les affixes roulés respectent la FENÊTRE du niveau héros (budget 1
    /// au niveau 5 : chaque affixe a `level ≤ 5`) et le budget croît avec
    /// le niveau (31 → 2, 51 → 3, 71 → 4) sur des crafts réels blood ring
    /// (L82 : ring,mag + jew + r12 + gpr).
    #[test]
    fn craft_affixes_roll_at_hero_level_window() {
        let c = cube();
        let t = affixes();
        let s = strings();
        let inputs = || {
            vec![
                CubeItem { code: "rin".into(), name: "Ring".into(), quality: RolledQuality::Magic, sockets: 0 },
                CubeItem::base("jew", "Jewel"),
                CubeItem::base("r12", "Thul Rune"),
                CubeItem::base("gpr", "Perfect Ruby"),
            ]
        };
        for (level, budget) in [(5u32, 1usize), (31, 2), (51, 3), (71, 4)] {
            let mut rng = Rng::new(0xB100D + level as u64);
            let crf = craft(&c, &inputs(), level, &t, &s, &mut rng).expect("blood ring doit matcher");
            assert_eq!(crf.recipe_line, 82, "L82 blood ring");
            assert_eq!(crf.family, CraftFamily::Blood);
            assert_eq!(crf.item.code, "rin");
            assert_eq!(crf.item.affix_ids.len(), budget, "niveau {level}");
            for &(id, _) in &crf.item.affix_ids {
                let (rec_a, _) = resolve_affix(&t, id).unwrap();
                assert!(rec_a.level <= level, "affixe fenêtre {} > héros {level}", rec_a.level);
            }
        }
    }

    // -- ② ÉTHEREAL ----------------------------------------------------------

    /// Éligibilité données réelles : armes/armures oui (hax 28, cap 12,
    /// fhl), bagues/amulettes/joyaux/misc NON (dur 0), indestructible NON.
    #[test]
    fn ethereal_eligibility_from_real_tables() {
        assert!(can_be_ethereal("hax"), "Hand Axe : dur 28");
        assert!(can_be_ethereal("cap"), "Cap : dur 12");
        assert!(can_be_ethereal("fhl"), "Full Helm");
        assert!(!can_be_ethereal("rin"), "bague : misc");
        assert!(!can_be_ethereal("amu"), "amulette : misc");
        assert!(!can_be_ethereal("jew"), "joyau : misc");
        assert!(!can_be_ethereal("hp1"), "potion : misc");
        assert!(!can_be_ethereal("xxx"), "code inconnu");
    }

    /// Jet 1/20 : déterministe au seed ; distribution ≈ 5 % sur 200 000
    /// jets éligibles ; un tirage SEUL consommé par appel éligible.
    #[test]
    fn ethereal_roll_is_1_in_20() {
        let mut a = Rng::new(0xE7E);
        let mut b = Rng::new(0xE7E);
        assert_eq!(roll_ethereal(&mut a, "hax"), roll_ethereal(&mut b, "hax"));
        let n = 200_000u32;
        let mut rng = Rng::new(0x5A5A_5A5A);
        let mut hits = 0;
        for _ in 0..n {
            hits += roll_ethereal(&mut rng, "hax") as u32;
        }
        let ppm = hits as u64 * 1_000_000 / n as u64;
        assert!((42_000..=58_000).contains(&ppm), "5 % ± 0,8 % : {ppm} ppm");
        // Code non éligible : JAMAIS vrai, AUCUN tirage consommé.
        let mut r1 = Rng::new(7);
        let mut r2 = Rng::new(7);
        assert!(!roll_ethereal(&mut r1, "rin"));
        assert_eq!(r1.below(1000), r2.below(1000), "aucun tirage consommé");
    }

    /// Boosts éthérés : durabilité ÷2 plancher 1 (28→14, 1→1, 3→1),
    /// défense +50 % (10→15, 0→0), dégâts +50 % (3-6 → 4-9).
    #[test]
    fn ethereal_boosts_halve_durability_and_boost_fifty() {
        assert_eq!(ethereal_durability(28), 14, "hax 28");
        assert_eq!(ethereal_durability(1), 1, "plancher 1");
        assert_eq!(ethereal_durability(3), 1, "3/2 = 1 (plancher)");
        assert_eq!(ethereal_durability(0), 0, "indestructible (exclu du jet, défensif)");
        assert_eq!(ethereal_defense(10), 15);
        assert_eq!(ethereal_defense(0), 0);
        assert_eq!(ethereal_damage(3, 6), (4, 9), "3+1, 6+3");
        assert_eq!(ethereal_boost(1), 1);
    }

    /// L'éthéré n'est PAS socketable-exclu : les recettes de socketing
    /// réelles L125-130 n'exigent PAS `noe` (seules réparation/recharge
    /// L139-142 portent `noe` — zieuté, données réelles).
    #[test]
    fn ethereal_socketing_not_excluded_by_real_recipes() {
        let c = cube();
        let noe_of = |n: usize| {
            c.recipes().iter().find(|r| r.line == n).unwrap().inputs.iter().any(|i| i.noe)
        };
        for l in [125, 126, 127, 128] {
            assert!(!noe_of(l), "L{l} : socketing SANS noe (éthéré socketable)");
        }
        for l in [139, 140, 141, 142] {
            assert!(noe_of(l), "L{l} : réparation/recharge EXIGE noe (éthéré refusé)");
        }
    }

    // -- ③ CHARGES -----------------------------------------------------------

    /// Charges Marrowwalk (données réelles 12b) : 13 charges de Bone
    /// Prison (skill 88) niveau 33 ; décrément hook : 13 usages OK puis
    /// refus à sec ; état intact après refus.
    #[test]
    fn charges_marrowwalk_use_until_empty() {
        let prop = SkillProp {
            kind: SkillPropKind::Charged,
            skill_id: 88,
            level: 33,
            charges: 13,
            prob: 0,
        };
        let mut c = charges_of(&prop).expect("charged ⇒ charges");
        assert_eq!(c, Charges { skill_id: 88, level: 33, cur: 13, max: 13 });
        for i in 1..=13 {
            assert!(use_charge(&mut c), "usage {i}");
            assert_eq!(c.cur, 13 - i);
        }
        assert_eq!(c.cur, 0);
        assert!(!use_charge(&mut c), "épuisé : refus");
        assert_eq!(c.cur, 0, "état intact après refus");
        // Procs/+skills : pas de charges.
        let on_hit = SkillProp { kind: SkillPropKind::OnGetHit, skill_id: 44, level: 4, charges: 0, prob: 5 };
        assert!(charges_of(&on_hit).is_none());
    }

    /// Recharge vendeur : coût linéaire miroir de repair_cost
    /// (1000 × 8/13 = 615 plancher), débit réel, plein ⇒ rien à faire,
    /// or insuffisant ⇒ refus état intact ; l'éthéré se recharge (règle :
    /// seule la RÉPARATION lui est refusée — pas de garde ici).
    #[test]
    fn charges_vendor_recharge_cost_and_refusals() {
        assert_eq!(recharge_cost(1000, 5, 13), 615, "1000×8/13 plancher");
        assert_eq!(recharge_cost(1000, 13, 13), 0, "plein");
        assert_eq!(recharge_cost(1000, 0, 0), 0, "sans charges");
        let mut c = Charges { skill_id: 88, level: 33, cur: 5, max: 13 };
        assert_eq!(
            recharge(&mut c, 1000, 614),
            Err(EconError::NotEnoughGold { need: 615, have: 614 })
        );
        assert_eq!(c.cur, 5, "refus : état intact");
        let out = recharge(&mut c, 1000, 615).expect("payée");
        assert_eq!(out, RechargeOutcome { cost: 615, cur: 13 });
        assert_eq!(c.cur, 13);
        assert_eq!(recharge(&mut c, 1000, 10_000), Err(EconError::NothingToRepair));
    }

    // -- ④ PERSONNALISÉ + EAR -------------------------------------------------

    /// Personnalisation retail : « Justin's Petit Poulet » ; ear : niveau
    /// du héros mort encodé (clamp 7 bits), nom = affichage.
    #[test]
    fn personalize_and_ear_level_encoded() {
        assert_eq!(personalize("Justin", "Petit Poulet"), "Justin's Petit Poulet");
        assert_eq!(personalize("Coralie", "War Axe"), "Coralie's War Axe");
        let ear = make_ear("Justin", 0, 12);
        assert_eq!(ear, Ear { class: 0, level: 12, name: "Justin".into() });
        assert_eq!(ear.display_name(), "Justin");
        assert_eq!(ear.level, 12, "niveau du héros mort encodé");
        assert_eq!(make_ear("Zod", 3, 200).level, 127, "clamp 7 bits");
        assert_eq!(make_ear("Zod", 9, 50).class, 7, "clamp 3 bits");
        assert_eq!(make_ear("Zod", 4, 99).level, 99, "max retail 99 tient sur 7 bits");
    }
}
