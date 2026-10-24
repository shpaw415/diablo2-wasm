//! `item_props` — descripteurs de propriétés d'items (affixes, uniques, sets).
//!
//! Port Rust de `d2records/property_descriptor.go` (Code/Param/Min/Max) :
//! chaque colonne `propN`/`parN`/`minN`/`maxN` des tables v1.12
//! (`uniqueitems`, `setitems`, `sets`, `magicprefix`, `magicsuffix`,
//! `automagic`) devient un [`PropDescriptor`] brut. Le tirage de la valeur
//! (`min..=max`, D2 tire à la GÉNÉRATION de l'item) produit un
//! [`RolledProp`] portant l'interprétation typée [`StatProp`] — le socle
//! `Stat`/`ItemStatCost` (chantier d'un autre membre) consommera cette liste.
//!
//! Codes couverts en typé : le vocabulaire réel des tables v1.12
//! (dmg%, ac%, ac, dmg-min/max, str/dex/vit/enr, hp, mana, res-*,
//! mag%, gold%, att/att%, lifesteal…). Code inconnu ⇒ [`StatProp::Generic`]
//! (jamais perdu, juste non interprété).
//!
//! # Étape 12b — évaluation des propriétés D2 → ids ItemStatCost
//! [`evaluate`] évalue UN descripteur avec sa valeur déjà tirée : table de
//! dispatch `code prop → stat(s) émise(s)` — `(id ItemStatCost, param,
//! valeur)` ([`StatEmit`]) — sur TOUT le vocabulaire réel des 6 tables v1.12
//! embarquées (magicprefix/magicsuffix/automagic modNcode 1-3, uniqueitems
//! prop1-12, setitems prop1-9, sets PCode/FCode ; union extraite des
//! fichiers : LE FICHIER FAIT FOI). Port de la STRUCTURE de dispatch de
//! `opendiablo2/d2core/d2item/diablo2item/item_property.go:118` (`eval` :
//! switch sur l'id de fonction properties.txt, `repeat previous` inclus —
//! le seul morceau Go testé, cité ; properties.txt n'est pas embarquée, la
//! table fait foi sur les codes réels). `charged`/procs/+skills émettent
//! une structure minimale [`SkillProp`] (skill id + charges/prob — AUCUN
//! comportement runtime, exposé à l'intégrateur, cf. `fnChargeRelated`
//! item_property.go:304 et `fnProcs` :260 : param/min/max lus sans roll).
//! Codes non couverts (dmg-elem, ethereal) ⇒ `unknown = true` (skip
//! documenté — jamais perdu : descripteur + valeur restent portés).
//!
//! Quirks données réelles : `*code` (lignes LoD d'uniqueitems : `*mana -30`
//! = `-30 Mana`) évalué comme `code` ; colonne `max` vide (`mag%/lvl 10`,
//! `dmg/lvl 25` Windforce) ⇒ valeur FIXE `min` ([`roll`]) ; `par` de skill
//! = id numérique OU nom ([`resolve_skill_param`], patch_skills.txt).
//!
//! Zéro IO, zéro panique : cellule absente ⇒ `None`.

use crate::rng::Rng;
use crate::stats::StatCostTable;

/// Descripteur brut d'une propriété (port direct `property_descriptor.go`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PropDescriptor {
    /// Code propriété (`properties.txt` v1.12 : `dmg%`, `ac%`, `res-fire`…).
    pub code: String,
    /// Paramètre brut (id de skill, de state… ; souvent vide).
    pub param: String,
    /// Borne basse du tirage.
    pub min: i32,
    /// Borne haute du tirage.
    pub max: i32,
}

/// Valeur tirée d'un descripteur + interprétation typée.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RolledProp {
    /// Descripteur source (pour la re-génération et l'audit).
    pub desc: PropDescriptor,
    /// Valeur tirée dans `min..=max` (déterministe au seed du [`Rng`]).
    pub value: i32,
    /// Interprétation typée ([`None`] = code non couvert, reste en `Generic`).
    pub typed: Option<StatProp>,
}

/// Liste typée des stats d'item (socle du futur `Stat`/`ItemStatCost`).
/// Les variantes couvrent les codes réellement présents dans les tables
/// v1.12 embarquées ; tout le reste tombe dans [`StatProp::Generic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum StatProp {
    /// Défense plate (`ac`).
    Ac(i32),
    /// Défense % (`ac%`, affixes « Sturdy »… — s'applique à la base AC).
    AcPct(i32),
    /// Dégâts % (`dmg%` — Enhanced Damage, arme en main).
    EdamPct(i32),
    /// Dégâts min plats (`dmg-min`).
    DmgMin(i32),
    /// Dégâts max plats (`dmg-max`).
    DmgMax(i32),
    /// Force (`str`).
    Str(i32),
    /// Dextérité (`dex`).
    Dex(i32),
    /// Vitalité (`vit`).
    Vit(i32),
    /// Énergie (`enr`).
    Enr(i32),
    /// Vie max (`hp`).
    Life(i32),
    /// Mana max (`mana`).
    Mana(i32),
    /// Résistance feu (`res-fire`).
    ResFire(i32),
    /// Résistance froid (`res-cold`).
    ResCold(i32),
    /// Résistance foudre (`res-ltng`).
    ResLightning(i32),
    /// Résistance poison (`res-pois`).
    ResPoison(i32),
    /// Résistance à tout (`res-all`).
    ResAll(i32),
    /// Magic Find (`mag%`).
    MagicFind(i32),
    /// Gold Find (`gold%`).
    GoldFind(i32),
    /// Toucher plat (`att`).
    AttackRating(i32),
    /// Toucher % (`att%`).
    AttackRatingPct(i32),
    /// Vol de vie % (`lifesteal`).
    LifeSteal(i32),
    /// Vol de mana % (`manasteal`).
    ManaSteal(i32),
    /// +skills tous (`allskills`).
    AllSkills(i32),
    /// +skill unique (param = code skill, `skill`).
    Skill,
    /// +skills d'un onglet (param = 1..6, `skilltab`).
    SkillTab,
    /// Durabilité (`dur`).
    Durability(i32),
    /// Rayon de lumière (`light`).
    LightRadius(i32),
    /// Code non interprété (resté brut, jamais perdu).
    Generic,
}

/// Tire la valeur d'un descripteur (`min..=max` inclusif, ordre D2 : à la
/// génération) et résout l'interprétation typée. Colonne `max` vide (parsée
/// 0 — lignes réelles sans `maxN`) ⇒ valeur FIXE `min` ; [`Rng::range_i32`]
/// ne consomme alors aucun tirage (hi <= lo), le flux de chaque drop reste
/// inchangé. Les lignes `minN`/`maxN` VIDES (famille `/lvl` : `mag%/lvl 10`
/// de Skullder's Ire, `rep-dur 20`…) roulent 0 comme en 7a — le contrat
/// gelé [`crate::item_roll`] et son test en dépendent ; c'est
/// [`evaluate`] qui substitue le `parN` (porteur réel de la valeur).
#[allow(private_interfaces)] // même pattern que `affixes` (Rng interne pub-usage)
pub fn roll(desc: &PropDescriptor, rng: &mut Rng) -> RolledProp {
    let hi = if desc.max == 0 { desc.min } else { desc.max };
    let (lo, hi) = (desc.min.min(hi), desc.min.max(hi));
    let value = rng.range_i32(lo, hi);
    let typed = interpret(desc, value);
    RolledProp { desc: desc.clone(), value, typed }
}

/// Interprétation typée d'un descripteur avec sa valeur déjà tirée.
/// `param` peut porter l'identifiant (skill/onglet) — conservé dans le tuple
/// pour les codes paramétrés.
pub fn interpret(desc: &PropDescriptor, value: i32) -> Option<StatProp> {
    use StatProp::*;
    Some(match desc.code.as_str() {
        "ac" => Ac(value),
        "ac%" => AcPct(value),
        "dmg%" => EdamPct(value),
        "dmg-min" => DmgMin(value),
        "dmg-max" => DmgMax(value),
        "str" => Str(value),
        "dex" => Dex(value),
        "vit" => Vit(value),
        "enr" => Enr(value),
        "hp" => Life(value),
        "mana" => Mana(value),
        "res-fire" => ResFire(value),
        "res-cold" => ResCold(value),
        "res-ltng" => ResLightning(value),
        "res-pois" => ResPoison(value),
        "res-all" => ResAll(value),
        "mag%" => MagicFind(value),
        "gold%" => GoldFind(value),
        "att" => AttackRating(value),
        "att%" => AttackRatingPct(value),
        "lifesteal" => LifeSteal(value),
        "manasteal" => ManaSteal(value),
        "allskills" => AllSkills(value),
        "skill" => Skill,
        "skilltab" => SkillTab,
        "dur" => Durability(value),
        "light" => LightRadius(value),
        _ => return None,
    })
}

/// Lit une cellule `propN`/`parN`/`minN`/`maxN` ; `(code vide ou "0")` ⇒ None.
pub(crate) fn descriptor(cells: [&str; 4]) -> Option<PropDescriptor> {
    let code = cells[0].trim();
    if code.is_empty() || code == "0" {
        return None;
    }
    let parse = |s: &str| s.trim().parse::<i32>().unwrap_or(0);
    Some(PropDescriptor {
        code: code.to_string(),
        param: cells[1].trim().to_string(),
        min: parse(cells[2]),
        max: parse(cells[3]),
    })
}

// ---------------------------------------------------------------------------
// Étape 12b — évaluation : dispatch code → stat(s) ItemStatCost émise(s)
// ---------------------------------------------------------------------------

/// Une émission de stat : `(id ItemStatCost, param, valeur)` — l'id est
/// celui du dictionnaire `patch_itemstatcost.txt` (clé des saves `.d2s`).
/// `param` porte l'accroche paramétrée (classe, onglet, state, skill) ;
/// 0 pour une stat pure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StatEmit {
    /// Id numérique ItemStatCost (`-1` si la stat n'existe pas dans la
    /// table — le golden exhaustif garantit que ça n'arrive pas sur les
    /// données v1.12 réelles).
    pub stat_id: i32,
    /// Param (`parN` : classe/onglet/skill/state ; 0 à défaut).
    pub param: i32,
    /// Valeur émise (peut être négative : `mana -5` de Gull).
    pub value: i64,
}

/// Famille d'une accroche skill ([`SkillProp`]) — structure minimale exposée
/// à l'intégrateur, AUCUN comportement runtime ici (mission 12b ③).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum SkillPropKind {
    /// `charged` — charges sur l'item (niveau + nb charges).
    Charged,
    /// `hit-skill` — chance de cast à l'ATTAQUE.
    OnAttack,
    /// `gethit-skill` — chance de cast quand TOUCHÉ.
    OnGetHit,
    /// `kill-skill` — chance de cast sur KILL.
    OnKill,
    /// `death-skill` — chance de cast à la MORT du porteur.
    OnDeath,
    /// `levelup-skill` — chance de cast au PASSAGE DE NIVEAU.
    OnLevelUp,
    /// `skill`/`att-skill` — +N au skill (param = skill).
    Single,
    /// `oskill` — +N au skill pour TOUTES les classes.
    NonClass,
    /// `skill-rand` — +N à un skill TIRÉ dans [min,max] (Ormus' Robes :
    /// par = niveau 3, min/max = fenêtre d'ids — Go item_property.go:276
    /// `fnRandomSkill` : le skill est roulé, le niveau vient du param).
    RandomSkill,
    /// `randclassskill` — skill de classe tiré (valeur = index de classe
    /// 0..=6 ; stat NON émise — od2 `fnRandClassSkill` item_property.go:371
    /// renvoie `nil` aussi).
    RandomClassSkill,
}

/// Structure minimale d'une accroche skill (charges, procs, +skills).
/// Sémantique `param/min/max` portée par le GO TESTÉ (cité) :
/// `charged` = skill `par`, charges `min`, niveau `max` (`fnChargeRelated`
/// item_property.go:304 — « Level 33 Bone Prison (13 Charges) » de
/// Marrowwalk : min 13 / max 33) ; procs = skill `par`, chance % `min`,
/// niveau `max` (`fnProcs` item_property.go:260 : PAS de roll).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SkillProp {
    /// Famille de l'accroche.
    pub kind: SkillPropKind,
    /// Id du skill (`patch_skills.txt` col `Id` ; nom résolu par
    /// [`resolve_skill_param`], `-1` si introuvable).
    pub skill_id: i32,
    /// Niveau du skill (procs : `maxN` ; `skill` : valeur roulée).
    pub level: i32,
    /// Nb de charges (`charged` : `minN`) ; 0 sinon.
    pub charges: i32,
    /// Chance % (procs : `minN`) ; 0 sinon.
    pub prob: i32,
}

/// Résultat d'évaluation d'un descripteur : stats pures émises, stats
/// paramétrées, accroches skill, et flag de skip documenté.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PropEval {
    /// Stats pures (`param` 0) — agrégables telles quelles dans une
    /// [`crate::statlist::StatList`] par id.
    pub emits: Vec<StatEmit>,
    /// Stats paramétrées (`item_addclassskills`, `item_addskill_tab`,
    /// `item_aura`, `state`, `fade`, `item_reanimate`) — une map id seul
    /// les agrègerait FAUX (classes confondues) : exposées à part.
    pub param_stats: Vec<StatEmit>,
    /// Accroches skill (charges/procs/+skills) — pour l'intégrateur.
    pub skills: Vec<SkillProp>,
    /// `true` = code NON couvert (skip documenté : `dmg-elem` — masque
    /// élémentaire non testé, `ethereal` — flag d'item, pas une stat ISC).
    /// Descripteur + valeur restent portés par l'appelant (jamais perdus).
    pub unknown: bool,
}

impl PropEval {
    /// Fusionne une autre évaluation (ordre conservé).
    pub fn merge(&mut self, other: PropEval) {
        self.emits.extend(other.emits);
        self.param_stats.extend(other.param_stats);
        self.skills.extend(other.skills);
        self.unknown |= other.unknown;
    }
}

/// Normalise un code prop : préfixe `*` retiré (variantes LoD des lignes
/// d'uniqueitems : `*mana -30` = `-30 Mana`, `*charged` = `charged`) +
/// minuscules (`Light` Naj's Circlet = `light`).
fn normalize_code(code: &str) -> String {
    code.trim().trim_start_matches('*').to_ascii_lowercase()
}

/// Résout un `parN` de skill : id numérique tel quel, sinon NOM de skill
/// (`patch_skills.txt`, via [`crate::skills_tree::SkillTreeData`]) ;
/// repli sans espaces (`BloodGolem` → « Blood Golem » id 85). `-1` si
/// introuvable (jamais de panique).
pub fn resolve_skill_param(param: &str) -> i32 {
    let p = param.trim();
    if let Ok(id) = p.parse::<i32>() {
        return id;
    }
    if p.is_empty() {
        return -1;
    }
    let tree = crate::skills_tree::SkillTreeData::global();
    if let Some(rec) = tree.get(p) {
        return rec.id as i32;
    }
    let needle = p.to_ascii_lowercase().replace(' ', "");
    tree.records()
        .iter()
        .find(|r| r.code.to_ascii_lowercase().replace(' ', "") == needle)
        .map(|r| r.id as i32)
        .unwrap_or(-1)
}

/// Index de classe pour `item_addclassskills` (ordre v1.12 des onglets,
/// 3 par classe — `skillTabsPerClass` de item_property.go:22) : ama 0 …
/// ass 6.
fn class_index(code: &str) -> i32 {
    match code {
        "ama" => 0,
        "sor" => 1,
        "nec" => 2,
        "pal" => 3,
        "bar" => 4,
        "dru" => 5,
        _ => 6, // "ass"
    }
}

/// Évalue UN descripteur avec sa valeur DÉJÀ TIRÉE ([`roll`]) : dispatch
/// code → stat(s) émise(s) (mission 12b ①). PUR — aucun tirage ici : les
/// codes qui roulent (`skill-rand`) reçoivent la valeur rollée de [`roll`].
/// Codes inconnus ⇒ [`PropEval::unknown`] (skip documenté).
pub fn evaluate(desc: &PropDescriptor, value: i32) -> PropEval {
    let mut ev = PropEval::default();
    let code = normalize_code(&desc.code);
    let param_raw = desc.param.trim();
    let param_num = param_raw.parse::<i32>().unwrap_or(0);
    // Quirk v1.12 : les lignes `/lvl` + `rep-*` (`mag%/lvl 10` Skullder's
    // Ire, `dmg/lvl 25` Windforce, `rep-dur 20`, `regen-stam/lvl 8`…
    // 54 modN/mods réels minN/maxN VIDES) portent la VALEUR dans `parN` ;
    // [`roll`] roule alors 0 (contrat gelé 7a) — l'évaluation substitue.
    let value = if value == 0 && desc.min == 0 && desc.max == 0 && param_num != 0 {
        param_num
    } else {
        value
    };
    let isc = |name: &str| StatCostTable::global().get(name).map(|r| r.id).unwrap_or(-1);
    let mut emit = |name: &str| ev.emits.push(StatEmit { stat_id: isc(name), param: 0, value: value as i64 });
    let mut param_emit = |name: &str, param: i32| {
        ev.param_stats.push(StatEmit { stat_id: isc(name), param, value: value as i64 })
    };
    let mut pair = |a: &str, b: &str| {
        emit(a);
        emit(b);
    };
    let mut skill = |kind: SkillPropKind, skill_id: i32, level: i32, charges: i32, prob: i32| {
        ev.skills.push(SkillProp { kind, skill_id, level, charges, prob });
    };
    match code.as_str() {
        // ---- accroches skill (mission ③ — structures, aucune stat) ----
        "skill" | "att-skill" => {
            skill(SkillPropKind::Single, resolve_skill_param(param_raw), value, 0, 0)
        }
        "oskill" => skill(SkillPropKind::NonClass, resolve_skill_param(param_raw), value, 0, 0),
        "skill-rand" => {
            skill(SkillPropKind::RandomSkill, value, param_num, 0, 0)
        }
        "randclassskill" => skill(SkillPropKind::RandomClassSkill, -1, value, 0, 0),
        "charged" => {
            skill(SkillPropKind::Charged, resolve_skill_param(param_raw), desc.max, desc.min, 0)
        }
        "hit-skill" => {
            skill(SkillPropKind::OnAttack, resolve_skill_param(param_raw), desc.max, 0, desc.min)
        }
        "gethit-skill" => {
            skill(SkillPropKind::OnGetHit, resolve_skill_param(param_raw), desc.max, 0, desc.min)
        }
        "kill-skill" => {
            skill(SkillPropKind::OnKill, resolve_skill_param(param_raw), desc.max, 0, desc.min)
        }
        "death-skill" => {
            skill(SkillPropKind::OnDeath, resolve_skill_param(param_raw), desc.max, 0, desc.min)
        }
        "levelup-skill" => {
            skill(SkillPropKind::OnLevelUp, resolve_skill_param(param_raw), desc.max, 0, desc.min)
        }
        // ---- stats paramétrées (une map par id seul les agrègerait FAUX) ----
        "ama" | "sor" | "nec" | "pal" | "bar" | "dru" | "ass" => {
            param_emit("item_addclassskills", class_index(&code));
        }
        "skilltab" => param_emit("item_addskill_tab", param_num),
        "aura" => param_emit("item_aura", resolve_skill_param(param_raw)),
        "state" => param_emit("state", param_num),
        "fade" => param_emit("fade", param_num),
        "reanimate" => param_emit("item_reanimate", param_num),
        // ---- multi-stats (sémantique D2 réelle) ----
        "all-stats" => {
            for n in ["strength", "energy", "dexterity", "vitality"] {
                emit(n);
            }
        }
        "res-all" => {
            for n in ["fireresist", "lightresist", "coldresist", "poisonresist"] {
                emit(n);
            }
        }
        "res-all-max" => {
            for n in ["maxfireresist", "maxlightresist", "maxcoldresist", "maxpoisonresist"] {
                emit(n);
            }
        }
        "dmg%" => pair("item_mindamage_percent", "item_maxdamage_percent"),
        "dmg-mag" => pair("magicmindam", "magicmaxdam"),
        "dmg-fire" => pair("firemindam", "firemaxdam"),
        "dmg-ltng" => pair("lightmindam", "lightmaxdam"),
        // dmg-cold / dmg-pois : le `par` porte la DURÉE (frames — Azurewrath
        // dmg-cold par 250) : émise en plus sur `coldlength`/`poisonlength`.
        "dmg-cold" => {
            pair("coldmindam", "coldmaxdam");
            if param_num > 0 {
                ev.emits.push(StatEmit { stat_id: isc("coldlength"), param: 0, value: param_num as i64 });
            }
        }
        "dmg-pois" => {
            pair("poisonmindam", "poisonmaxdam");
            if param_num > 0 {
                ev.emits.push(StatEmit {
                    stat_id: isc("poisonlength"),
                    param: 0,
                    value: param_num as i64,
                });
            }
        }
        "lifesteal" => pair("lifedrainmindam", "lifedrainmaxdam"),
        "manasteal" => pair("manadrainmindam", "manadrainmaxdam"),
        // ---- skips documentés ----
        "dmg-elem" => ev.unknown = true, // masque d'éléments (bit par N) — non testé
        "ethereal" => ev.unknown = true, // flag d'item (RolledItem) — pas une stat ISC
        // ---- table générale ----
        other => match prop_stat_names(other) {
            [] => ev.unknown = true,
            names => {
                for n in names {
                    emit(n);
                }
            }
        },
    }
    ev
}

/// Évalue une liste de descripteurs avec leurs valeurs tirées (ordre
/// conservé) — la matière première du pont `statlist::ItemStats`.
pub fn evaluate_all(descs: &[PropDescriptor], values: &[i32]) -> PropEval {
    let mut ev = PropEval::default();
    for (d, v) in descs.iter().zip(values.iter()) {
        ev.merge(evaluate(d, *v));
    }
    ev
}

/// Roule les valeurs `min..=max` d'une liste de descripteurs (UN tirage
/// [`Rng`] par prop non fixe — ordre de la liste) puis les retourne en
/// [`RolledProp`] (le pont uniques/sets de `statlist` consomme ça).
#[allow(private_interfaces)]
pub fn roll_descs(descs: &[PropDescriptor], rng: &mut Rng) -> Vec<RolledProp> {
    descs.iter().map(|d| roll(d, rng)).collect()
}

/// Table de dispatch `code prop → nom(s) de stat ItemStatCost.txt` — le
/// vocabulaire RÉEL des 6 tables v1.12 embarquées (union extraite des
/// fichiers, golden exhaustif en test ; LE FICHIER FAIT FOI). Les codes à
/// double stat (dmg%, dmg-mag, lifesteal…) sont déjà éclatés en paire dans
/// [`evaluate`] ; cette table liste les codes à émission simple.
fn prop_stat_names(code: &str) -> &'static [&'static str] {
    match code {
        // --- défense ---
        "ac" => &["armorclass"],
        "ac%" => &["item_armor_percent"],
        "ac%/lvl" => &["item_armorpercent_perlevel"],
        "ac/lvl" => &["item_armor_perlevel"],
        "ac/time" => &["item_armor_bytime"],
        "ac-hth" => &["armorclass_vs_hth"],
        "ac-miss" => &["armorclass_vs_missile"],
        "reduce-ac" => &["item_fractionaltargetac"], // « -X % Target Defense » (Razortine 50)
        // --- dégâts ---
        "dmg" | "dmg-norm" => &["item_normaldamage"],
        "dmg-min" => &["mindamage"],
        "dmg-max" => &["maxdamage"],
        "dmg%/lvl" => &["item_maxdamage_percent_perlevel"],
        "dmg/lvl" => &["item_maxdamage_perlevel"],
        "fire-min" => &["firemindam"],
        "fire-max" => &["firemaxdam"],
        "ltng-min" => &["lightmindam"],
        "ltng-max" => &["lightmaxdam"],
        "cold-min" => &["coldmindam"],
        "cold-max" => &["coldmaxdam"],
        "pois-min" => &["poisonmindam"],
        "pois-max" => &["poisonmaxdam"],
        "cold-len" => &["coldlength"],
        "pois-len" => &["poisonlength"],
        "dmg-undead" => &["item_undeaddamage_percent"],
        "dmg-demon" => &["item_demondamage_percent"],
        "dmg-und/lvl" => &["item_damage_undead_perlevel"],
        "dmg-dem/lvl" => &["item_damage_demon_perlevel"],
        "dmg-ac" => &["item_fractionaltargetac"],
        "ignore-ac" => &["item_ignoretargetac"],
        "dmg-to-mana" => &["item_damagetomana"],
        // --- attributs / vie / mana / endurance ---
        "str" => &["strength"],
        "dex" => &["dexterity"],
        "vit" => &["vitality"],
        "enr" => &["energy"],
        "str/lvl" => &["item_strength_perlevel"],
        "dex/lvl" => &["item_dexterity_perlevel"],
        "vit/lvl" => &["item_vitality_perlevel"],
        "hp" => &["maxhp"],
        "mana" => &["maxmana"],
        "hp%" => &["item_maxhp_percent"],
        "mana%" => &["item_maxmana_percent"],
        "hp/lvl" => &["item_hp_perlevel"],
        "mana/lvl" => &["item_mana_perlevel"],
        "stam" => &["maxstamina"],
        "stam/lvl" => &["item_stamina_perlevel"],
        // --- régénérations ---
        "regen" => &["hpregen"],
        "regen-mana" => &["manarecovery"],
        "regen-stam" => &["staminarecoverybonus"],
        "regen-stam/lvl" => &["item_regenstamina_perlevel"],
        // --- résistances (res-all / -max : paires spéciales dans evaluate) ---
        "res-fire" => &["fireresist"],
        "res-ltng" => &["lightresist"],
        "res-cold" => &["coldresist"],
        "res-pois" => &["poisonresist"],
        "res-fire/lvl" => &["item_resist_fire_perlevel"],
        "res-ltng/lvl" => &["item_resist_ltng_perlevel"],
        "res-cold/lvl" => &["item_resist_cold_perlevel"],
        "res-pois/lvl" => &["item_resist_pois_perlevel"],
        "res-fire-max" => &["maxfireresist"],
        "res-ltng-max" => &["maxlightresist"],
        "res-cold-max" => &["maxcoldresist"],
        "res-pois-max" => &["maxpoisonresist"],
        "res-pois-len" => &["item_poisonlengthresist"],
        // --- absorptions (v1.12 : forme plate `abs-X` vs `%` = col ISC dédiée ;
        // divergence ASSUMÉE avec `affixes::prop_stat_names` qui met les
        // suffixes magiques sur la variante % — choix UI documenté là-bas) ---
        "abs-fire" => &["item_absorbfire"],
        "abs-fire%" => &["item_absorbfire_percent"],
        "abs-ltng" => &["item_absorblight"],
        "abs-ltng%" => &["item_absorblight_percent"],
        "abs-cold" => &["item_absorbcold"],
        "abs-cold%" => &["item_absorbcold_percent"],
        "abs-fire/lvl" => &["item_absorb_fire_perlevel"],
        "abs-ltng/lvl" => &["item_absorb_ltng_perlevel"],
        "abs-cold/lvl" => &["item_absorb_cold_perlevel"],
        // --- toucher / coups spéciaux ---
        "att" => &["tohit"],
        "att%" => &["item_tohit_percent"],
        "att/lvl" => &["item_tohit_perlevel"],
        "att%/lvl" => &["item_tohitpercent_perlevel"],
        "att-demon" => &["item_demon_tohit"],
        "att-undead" => &["item_undead_tohit"],
        "att-dem/lvl" => &["item_tohit_demon_perlevel"],
        "att-und/lvl" => &["item_tohit_undead_perlevel"],
        "thorns" => &["item_attackertakesdamage"],
        "thorns/lvl" => &["item_thorns_perlevel"],
        "light-thorns" => &["item_attackertakeslightdamage"],
        "openwounds" => &["item_openwounds"],
        "crush" => &["item_crushingblow"],
        "deadly" => &["item_deadlystrike"],
        "deadly/lvl" => &["item_deadlystrike_perlevel"],
        "slow" => &["item_slow"],
        "freeze" => &["item_freeze"],
        "nofreeze" => &["item_cannotbefrozen"],
        "half-freeze" => &["item_halffreezeduration"],
        "howl" => &["item_howl"],
        "stupidity" => &["item_stupidity"],
        "noheal" => &["item_preventheal"],
        "knock" => &["item_knockback"],
        "bloody" => &["item_extrablood"],
        "pierce" => &["item_pierce"],
        "pierce-fire" => &["item_pierce_fire"],
        "pierce-ltng" => &["item_pierce_ltng"],
        "pierce-cold" => &["item_pierce_cold"],
        "pierce-pois" => &["item_pierce_pois"],
        "magicarrow" => &["item_magicarrow"],
        "explosivearrow" => &["item_explosivearrow"],
        "fireskill" => &["item_elemskill"],
        "extra-fire" => &["passive_fire_mastery"],
        "extra-ltng" => &["passive_ltng_mastery"],
        "extra-cold" => &["passive_cold_mastery"],
        "extra-pois" => &["passive_pois_mastery"],
        // --- vitesses (paliers 1-3, mêmes stats) ---
        "swing1" | "swing2" | "swing3" => &["item_fasterattackrate"],
        "move1" | "move2" | "move3" => &["item_fastermovevelocity"],
        "cast1" | "cast2" | "cast3" => &["item_fastercastrate"],
        "balance1" | "balance2" | "balance3" => &["item_fastergethitrate"],
        // --- loot / économique / divers ---
        "mag%" => &["item_magicbonus"],
        "mag%/lvl" => &["item_find_magic_perlevel"],
        "gold%" => &["item_goldbonus"],
        "gold%/lvl" => &["item_find_gold_perlevel"],
        "cheap" => &["item_reducedprices"],
        "addxp" => &["item_addexperience"],
        "light" => &["item_lightradius"],
        "ease" => &["item_req_percent"],
        "dur" => &["maxdurability"],
        "indestruct" => &["item_indesctructible"],
        "rep-dur" => &["item_replenish_durability"],
        "rep-quant" => &["item_replenish_quantity"],
        "stack" => &["item_extra_stack"],
        "sock" => &["item_numsockets"],
        "red-dmg" => &["normal_damage_reduction"],
        "red-mag" => &["magic_damage_reduction"],
        "red-dmg%" => &["damageresist"],
        "rip" => &["item_restinpeace"],
        "mana-kill" => &["item_manaafterkill"],
        "heal-kill" => &["item_healafterkill"],
        "demon-heal" => &["item_healafterdemonkill"],
        "stamdrain" => &["item_staminadrainpct"],
        "block" | "block2" | "block3" => &["toblock"],
        "allskills" => &["item_allskills"],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Tests — tables réelles embarquées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    /// Le tirage est inclusif et déterministe au seed.
    #[test]
    fn roll_is_deterministic_and_bounded() {
        let d = PropDescriptor {
            code: "ac%".into(),
            param: String::new(),
            min: 20,
            max: 30,
        };
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..50 {
            let ra = roll(&d, &mut a);
            let rb = roll(&d, &mut b);
            assert_eq!(ra, rb);
            assert!((20..=30).contains(&ra.value));
            assert_eq!(ra.typed, Some(StatProp::AcPct(ra.value)));
        }
    }

    /// Valeurs fixes (min == max, ex. Gull `dmg-max 15 15`) et bornes inversées.
    #[test]
    fn roll_fixed_and_inverted_bounds() {
        let mut rng = Rng::new(7);
        let fixed = PropDescriptor {
            code: "dmg-max".into(),
            param: String::new(),
            min: 15,
            max: 15,
        };
        assert_eq!(roll(&fixed, &mut rng).value, 15);
        let inv = PropDescriptor {
            code: "hp".into(),
            param: String::new(),
            min: 25,
            max: 10,
        };
        let r = roll(&inv, &mut rng);
        assert!((10..=25).contains(&r.value), "bornes inversées gérées");
    }

    /// Codes inconnus ⇒ `Generic` (valeur conservée), cellules vides ⇒ None.
    #[test]
    fn unknown_codes_fall_back_to_generic() {
        assert_eq!(interpret(&desc("nofreeze", 1), 1), None);
        let d = desc("dmg-undead", 200);
        let mut rng = Rng::new(1);
        assert_eq!(roll(&d, &mut rng).typed, None);
        assert!(descriptor(["", "", "1", "2"]).is_none());
        assert!(descriptor(["0", "", "1", "2"]).is_none());
    }

    /// Le vocabulaire principal des tables v1.12 est bien typé.
    #[test]
    fn real_v112_codes_are_typed() {
        let cases = [
            ("dmg%", StatProp::EdamPct(0)),
            ("ac%", StatProp::AcPct(0)),
            ("ac", StatProp::Ac(0)),
            ("str", StatProp::Str(0)),
            ("res-all", StatProp::ResAll(0)),
            ("mag%", StatProp::MagicFind(0)),
            ("lifesteal", StatProp::LifeSteal(0)),
            ("skilltab", StatProp::SkillTab),
        ];
        for (code, want) in cases {
            assert_eq!(interpret(&desc(code, 0), 0), Some(want), "code {code}");
        }
    }

    fn desc(code: &str, v: i32) -> PropDescriptor {
        PropDescriptor { code: code.into(), param: String::new(), min: v, max: v }
    }

    // -------------------------------------------------------------------
    // Étape 12b — dispatch code → ids ItemStatCost + golden uniques réels
    // -------------------------------------------------------------------

    const UNIQUES_TXT: &str = include_str!("../../../../assets/patch_uniqueitems.txt");
    const SETITEMS_TXT: &str = include_str!("../../../../assets/patch_setitems.txt");
    const SETS_TXT: &str = include_str!("../../../../assets/patch_sets.txt");

    /// Descripteurs d'un unique par clé `index` (le nom du test cite la
    /// ligne réelle de `patch_uniqueitems.txt`).
    fn unique_props(index: &str) -> Vec<PropDescriptor> {
        crate::uniques_sets::parse(UNIQUES_TXT, SETITEMS_TXT, SETS_TXT)
            .expect("tables uniques/sets")
            .uniques
            .into_iter()
            .find(|u| u.index == index)
            .unwrap_or_else(|| panic!("{index} absent"))
            .props
    }

    fn fixed(code: &str, param: &str, min: i32, max: i32) -> PropDescriptor {
        PropDescriptor { code: code.into(), param: param.into(), min, max }
    }

    /// GOLDEN ids : les noms de la dispatch existent dans l'ISC v1.12 avec
    /// EXACTEMENT ces ids (échantillon — l'exhaustif est le test ci-après).
    #[test]
    fn dispatch_resolves_real_itemstatcost_ids() {
        let cases: [(&str, &str, i32); 12] = [
            ("dmg-norm", "item_normaldamage", 111),
            ("allskills", "item_allskills", 127),
            ("pal", "item_addclassskills", 83),
            ("skilltab", "item_addskill_tab", 188),
            ("aura", "item_aura", 151),
            ("regen-mana", "manarecovery", 26),
            ("cheap", "item_reducedprices", 87),
            ("addxp", "item_addexperience", 85),
            ("reanimate", "item_reanimate", 155),
            ("fireskill", "item_elemskill", 126),
            ("extra-fire", "passive_fire_mastery", 329),
            ("mag%/lvl", "item_find_magic_perlevel", 240),
        ];
        for (code, name, id) in cases {
            let ev = evaluate(&fixed(code, "", 1, 1), 1);
            assert_eq!(ev.emits.len() + ev.param_stats.len(), 1, "{code}");
            let e = ev.emits.first().or(ev.param_stats.first()).unwrap();
            assert_eq!(e.stat_id, id, "{code} : attendu {name}={id}");
            assert_eq!(StatCostTable::global().name_of_id(e.stat_id), Some(name));
        }
    }

    /// GOLDEN exhaustif : TOUT le vocabulaire réel des 6 tables v1.12
    /// (union modNcode + propN + PCode/FCode) s'évalue SANS id à -1, SANS
    /// skill à -1 (hors RandomClassSkill) et les seuls skips documentés
    /// sont dmg-elem (masque d'éléments) et ethereal (flag d'item).
    #[test]
    fn full_real_vocabulary_is_covered() {
        let us = crate::uniques_sets::parse(UNIQUES_TXT, SETITEMS_TXT, SETS_TXT)
            .expect("tables uniques/sets");
        let mut descs: Vec<PropDescriptor> = Vec::new();
        for u in &us.uniques {
            descs.extend(u.props.iter().cloned());
        }
        for s in &us.set_items {
            descs.extend(s.props.iter().cloned());
            descs.extend(s.additional.iter().flatten().cloned());
        }
        for s in &us.sets {
            descs.extend(s.partial_a.iter().cloned());
            descs.extend(s.partial_b.iter().cloned());
            descs.extend(s.full.iter().cloned());
        }
        assert!(descs.len() > 3000, "descripteurs réels : {}", descs.len());
        let mut unknown: Vec<String> = Vec::new();
        for d in &descs {
            let ev = evaluate(d, d.min);
            for e in &ev.emits {
                assert_ne!(e.stat_id, -1, "stat absente ISC pour {:?}", d);
            }
            for e in &ev.param_stats {
                assert_ne!(e.stat_id, -1, "stat param absente ISC pour {:?}", d);
            }
            for s in &ev.skills {
                if s.kind != SkillPropKind::RandomClassSkill {
                    assert_ne!(s.skill_id, -1, "skill non résolu pour {:?}", d);
                }
            }
            if ev.unknown {
                unknown.push(normalize_code(&d.code));
            }
        }
        unknown.sort();
        unknown.dedup();
        assert_eq!(unknown, vec!["dmg-elem", "ethereal"], "skips documentés (dédupliqués)");
    }

    /// GOLDEN L41 — Gull (`patch_uniqueitems.txt` ligne 41) : dmg-min 1,
    /// dmg-max 15, mag% 100, mana -5 (valeurs fixes, valeurs exactes).
    #[test]
    fn gull_line41_golden() {
        let mut rng = crate::rng::Rng::new(41);
        let props = crate::item_props::roll_descs(&unique_props("Gull"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        assert!(!ev.unknown, "Gull : aucun skip attendu");
        assert_eq!(ev.param_stats.len(), 0);
        assert!(ev.skills.is_empty());
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(21), Some(1), "mindamage 1");
        assert_eq!(get(22), Some(15), "maxdamage 15");
        assert_eq!(get(80), Some(100), "item_magicbonus 100 (LA table fait foi)");
        assert_eq!(get(9), Some(-5), "maxmana -5");
    }

    /// GOLDEN L122 — Nagelring : mag% 15-30 ROULÉ dans les bornes (rarity
    /// 15), thorns 3, att 50-75, red-mag 3 — déterministe au seed.
    #[test]
    fn nagelring_line122_golden() {
        let props_desc = unique_props("Nagelring");
        assert_eq!(props_desc.len(), 4);
        let mut rng = crate::rng::Rng::new(122);
        let props = roll_descs(&props_desc, &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let mag = props.iter().find(|p| p.desc.code == "mag%").unwrap();
        assert!((15..=30).contains(&mag.value), "mag% roulé {}", mag.value);
        let att = props.iter().find(|p| p.desc.code == "att").unwrap();
        assert!((50..=75).contains(&att.value));
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(80), Some(mag.value as i64));
        assert_eq!(get(78), Some(3), "item_attackertakesdamage 3");
        assert_eq!(get(19), Some(att.value as i64));
        assert_eq!(get(35), Some(3), "magic_damage_reduction 3");
        // déterminisme
        let mut rng2 = crate::rng::Rng::new(122);
        let again = roll_descs(&props_desc, &mut rng2);
        assert_eq!(props, again);
    }

    /// GOLDEN L384 — Annihilus : all-stats 10-20 s'éclate en 4 stats
    /// (strength/energy/dexterity/vitality), res-all 10-20 en 4
    /// résistances, allskills → item_allskills, addxp → item_addexperience.
    #[test]
    fn annihilus_line384_allstats_resall() {
        let mut rng = crate::rng::Rng::new(384);
        let props = roll_descs(&unique_props("Annihilus"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let allstats = props.iter().find(|p| p.desc.code == "all-stats").unwrap();
        assert!((10..=20).contains(&allstats.value));
        let vals: Vec<i64> = ev
            .emits
            .iter()
            .filter(|e| [0, 1, 2, 3].contains(&e.stat_id))
            .map(|e| e.value)
            .collect();
        assert_eq!(vals, vec![allstats.value as i64; 4], "4 attributs, même valeur");
        let res: Vec<i64> = ev
            .emits
            .iter()
            .filter(|e| [39, 41, 43, 45].contains(&e.stat_id))
            .map(|e| e.value)
            .collect();
        let resall = props.iter().find(|p| p.desc.code == "res-all").unwrap();
        assert_eq!(res, vec![resall.value as i64; 4]);
        assert_eq!(
            ev.emits.iter().find(|e| e.stat_id == 127).map(|e| e.value),
            Some(1),
            "item_allskills 1"
        );
        let xp = props.iter().find(|p| p.desc.code == "addxp").unwrap();
        assert!((5..=10).contains(&xp.value));
        assert_eq!(
            ev.emits.iter().find(|e| e.stat_id == 85).map(|e| e.value),
            Some(xp.value as i64),
            "item_addexperience"
        );
    }

    /// GOLDEN L373 — Marrowwalk : `charged` = structure minimale (mission
    /// ③) Bone Prison par=nom → id 88, charges=min 13, niveau=max 33 ;
    /// Life Tap id 82, 10/12 ; `skill` Skeleton Mastery id 69 niveau 1-2
    /// roulé ; regen-mana → manarecovery (26), regen-stam → 28.
    #[test]
    fn marrowwalk_line373_charges_and_skills() {
        let mut rng = crate::rng::Rng::new(373);
        let props = roll_descs(&unique_props("Marrowwalk"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let charged: Vec<&SkillProp> = ev
            .skills
            .iter()
            .filter(|s| s.kind == SkillPropKind::Charged)
            .collect();
        assert_eq!(charged.len(), 2, "Bone Prison + Life Tap");
        assert!(
            charged.iter().any(|s| s.skill_id == 88 && s.charges == 13 && s.level == 33),
            "Bone Prison (13/33) : {:?}",
            charged
        );
        assert!(
            charged.iter().any(|s| s.skill_id == 82 && s.charges == 10 && s.level == 12),
            "Life Tap (10/12)"
        );
        let single = ev
            .skills
            .iter()
            .find(|s| s.kind == SkillPropKind::Single)
            .expect("skill Skeleton Mastery");
        assert_eq!(single.skill_id, 69);
        assert!((1..=2).contains(&single.level));
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(26), Some(10), "manarecovery");
        assert_eq!(get(28), Some(10), "staminarecoverybonus");
        // La valeur ROULÉE du mod `charged` n'influence PAS la structure.
        let mut a = crate::rng::Rng::new(1);
        let mut b = crate::rng::Rng::new(99);
        let ra = roll_descs(&unique_props("Marrowwalk"), &mut a);
        let rb = roll_descs(&unique_props("Marrowwalk"), &mut b);
        let eva = evaluate_all(&ra.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(), &ra.iter().map(|p| p.value).collect::<Vec<_>>());
        let evb = evaluate_all(&rb.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(), &rb.iter().map(|p| p.value).collect::<Vec<_>>());
        let chg_a: Vec<_> = eva.skills.iter().filter(|s| s.kind == SkillPropKind::Charged).collect();
        let chg_b: Vec<_> = evb.skills.iter().filter(|s| s.kind == SkillPropKind::Charged).collect();
        assert_eq!(chg_a, chg_b, "charges/niveaux des charged = colonnes min/max FIXES");
        let lvl_a = eva.skills.iter().find(|s| s.kind == SkillPropKind::Single).unwrap().level;
        let lvl_b = evb.skills.iter().find(|s| s.kind == SkillPropKind::Single).unwrap().level;
        assert!((1..=2).contains(&lvl_a) && (1..=2).contains(&lvl_b), "le niveau du `skill` ROULE 1-2");
    }

    /// GOLDEN L269 — Windforce : `dmg/lvl 25` (col max VIDE) ⇒ valeur FIXE
    /// 25 émise sur item_maxdamage_perlevel (232) ; knock 1 ; dmg% 250 en
    /// paire min%/max% ; manasteal 6-8 en paire drain.
    #[test]
    fn windforce_line269_empty_max_is_fixed() {
        let props_desc = unique_props("Windforce");
        let dlvl = props_desc.iter().find(|p| p.code == "dmg/lvl").unwrap();
        assert_eq!((dlvl.min, dlvl.max), (0, 0), "minN/maxN vides, valeur dans parN");
        assert_eq!(dlvl.param, "25");
        let mut rng = crate::rng::Rng::new(269);
        let props = roll_descs(&props_desc, &mut rng);
        assert_eq!(
            props.iter().find(|p| p.desc.code == "dmg/lvl").unwrap().value,
            0,
            "roll gelé 7a : minN/maxN vides ⇒ 0, aucun tirage"
        );
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(218), Some(25), "item_maxdamage_perlevel");
        assert_eq!(get(81), Some(1), "item_knockback");
        assert_eq!(get(17), Some(250), "item_maxdamage_percent");
        assert_eq!(get(18), Some(250), "item_mindamage_percent");
        let ms = props.iter().find(|p| p.desc.code == "manasteal").unwrap();
        assert!((6..=8).contains(&ms.value));
    }

    /// GOLDEN L262 — Lightsabre : abs-ltng% → item_absorblight_percent,
    /// dmg-mag 60-120 → paire magicmindam/magicmaxdam, att-skill par=53 →
    /// SkillProp Single, ignore-ac (param 1 porté brut), light 7.
    #[test]
    fn lightsabre_line262_absorb_and_attskill() {
        let mut rng = crate::rng::Rng::new(262);
        let props = roll_descs(&unique_props("Lightsabre"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(144), Some(25), "item_absorblight_percent");
        let dmag = props.iter().find(|p| p.desc.code == "dmg-mag").unwrap();
        assert!((60..=120).contains(&dmag.value));
        assert_eq!(get(52), Some(dmag.value as i64), "magicmindam");
        assert_eq!(get(53), Some(dmag.value as i64), "magicmaxdam");
        let s = ev.skills.iter().find(|s| s.kind == SkillPropKind::Single).unwrap();
        assert_eq!(s.skill_id, 53, "par numérique direct");
        assert_eq!(get(115), Some(1), "item_ignoretargetac");
        assert_eq!(get(89), Some(7), "item_lightradius");
    }

    /// GOLDEN L304 — Azurewrath LoD (enabled) : `aura` par=Sanctuary →
    /// item_aura (151) param=119, all-stats 5-10, dmg-cold par=250 émet la
    /// durée coldlength, dmg-mag 250-500. (La ligne classic L31, disabled,
    /// n'est PAS prise — filtrage enabled.)
    #[test]
    fn azurewrath_lod_line304_aura_and_coldlen() {
        let us = crate::uniques_sets::parse(UNIQUES_TXT, SETITEMS_TXT, SETS_TXT).unwrap();
        let aw = us
            .uniques
            .iter()
            .find(|u| u.index == "Azurewrath" && u.enabled)
            .expect("Azurewrath LoD (enabled)");
        let mut rng = crate::rng::Rng::new(304);
        let props = roll_descs(&aw.props, &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let aura = ev.param_stats.iter().find(|e| e.stat_id == 151).unwrap();
        assert_eq!(aura.param, 119, "Sanctuary résolu par nom (patch_skills)");
        let dcold = aw.props.iter().find(|p| p.code == "dmg-cold").unwrap();
        assert_eq!(dcold.param, "250");
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(56), Some(250), "coldlength = par");
        let dmag = props.iter().find(|p| p.desc.code == "dmg-mag").unwrap();
        assert!((250..=500).contains(&dmag.value));
        let allstats = props.iter().find(|p| p.desc.code == "all-stats").unwrap();
        assert!((5..=10).contains(&allstats.value));
        assert!(ev.unknown == false);
    }

    /// GOLDEN L252 — Veil of Steel : le préfixe `*` est évalué comme le
    /// code (`*mana -30` → maxmana -30) ; res-all 50 ; light -4.
    #[test]
    fn veil_of_steel_line252_star_codes() {
        let mut rng = crate::rng::Rng::new(252);
        let props = roll_descs(&unique_props("Veil of Steel"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(9), Some(-30), "*mana -30 (étoile retirée)");
        assert_eq!(get(89), Some(-4), "light -4");
        for r in [39, 41, 43, 45] {
            assert_eq!(get(r), Some(50), "res-all 50 (id {r})");
        }
        assert!(!ev.unknown);
    }

    /// GOLDEN L220 — Skullder's Ire : `mag%/lvl 10` (max vide) FIXE 10,
    /// `*vit`/`*enr` -10, rep-dur 20 (max vide), allskills 1.
    #[test]
    fn skullders_ire_line220_perlevel_and_star() {
        let mut rng = crate::rng::Rng::new(220);
        let props = roll_descs(&unique_props("Skullder's Ire"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(240), Some(10), "item_find_magic_perlevel 10 (valeur = par)");
        assert_eq!(get(3), Some(-10), "*vit");
        assert_eq!(get(1), Some(-10), "*enr");
        assert_eq!(get(252), Some(20), "item_replenish_durability");
        assert_eq!(get(127), Some(1), "item_allskills");
    }

    /// GOLDEN L148 — The Fetid Sprinkler : `hit-skill` = proc (mission ③)
    /// skill 87 chance min 5 niveau max 1 (Go fnProcs : PAS de roll) ;
    /// `pal` 2 → item_addclassskills param=3 ; dmg-pois par=100 → durée
    /// poisonlength + paire poisonmindam/maxdam ; dmg-norm 15-25.
    #[test]
    fn fetid_sprinkler_line148_procs_and_poison() {
        let mut rng = crate::rng::Rng::new(148);
        let props = roll_descs(&unique_props("The Fetid Sprinkler"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let procs: Vec<&SkillProp> = ev
            .skills
            .iter()
            .filter(|s| s.kind == SkillPropKind::OnAttack)
            .collect();
        assert_eq!(procs.len(), 2);
        assert!(procs.iter().any(|s| s.skill_id == 87 && s.prob == 5 && s.level == 1));
        assert!(procs.iter().any(|s| s.skill_id == 81 && s.prob == 10 && s.level == 1));
        let pal = ev.param_stats.iter().find(|e| e.stat_id == 83).unwrap();
        assert_eq!((pal.param, pal.value), (3, 2), "item_addclassskills pal=index 3");
        let pois = props.iter().find(|p| p.desc.code == "dmg-pois").unwrap();
        assert_eq!(pois.value, 409, "valeur fixe (min=max=409)");
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(59), Some(100), "poisonlength = par");
        assert_eq!(get(57), Some(409), "poisonmindam");
        assert_eq!(get(58), Some(409), "poisonmaxdam");
        let dnorm = props.iter().find(|p| p.desc.code == "dmg-norm").unwrap();
        assert!((15..=25).contains(&dnorm.value));
        assert_eq!(get(111).is_some(), true, "item_normaldamage émis");
    }

    /// GOLDEN L362 + L301 — Gheed's Fortune (cheap → item_reducedprices,
    /// gold% 80-160) et Tomb Reaver (reanimate par=1, sock 1-3, heal-kill).
    #[test]
    fn gheed_line362_and_tombreaver_line301() {
        let mut rng = crate::rng::Rng::new(362);
        let props = roll_descs(&unique_props("Gheed's Fortune"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        let cheap = props.iter().find(|p| p.desc.code == "cheap").unwrap();
        assert!((10..=15).contains(&cheap.value), "cheap roulé {}", cheap.value);
        assert_eq!(get(87), Some(cheap.value as i64), "item_reducedprices");
        let gold = props.iter().find(|p| p.desc.code == "gold%").unwrap();
        assert!((80..=160).contains(&gold.value));

        let mut rng = crate::rng::Rng::new(301);
        let props = roll_descs(&unique_props("Tomb Reaver"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let rea = ev.param_stats.iter().find(|e| e.stat_id == 155).unwrap();
        assert_eq!((rea.param, rea.value), (1, 10), "item_reanimate par=1");
        let sock = props.iter().find(|p| p.desc.code == "sock").unwrap();
        assert!((1..=3).contains(&sock.value), "sock roulé {}", sock.value);
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(194), Some(sock.value as i64), "item_numsockets");
        let hk = props.iter().find(|p| p.desc.code == "heal-kill").unwrap();
        assert!((10..=14).contains(&hk.value));
        assert_eq!(get(86), Some(hk.value as i64), "item_healafterkill");
    }

    /// GOLDEN L243 + L46 — Wartraveler (dmg-norm 15-25 → item_normaldamage,
    /// stamdrain 40, move2 25) et Razortine (reduce-ac 50 →
    /// item_fractionaltargetac, slow 25 → item_slow).
    #[test]
    fn wartraveler_line243_and_razortine_line46() {
        let mut rng = crate::rng::Rng::new(243);
        let props = roll_descs(&unique_props("Wartraveler"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        let dnorm = props.iter().find(|p| p.desc.code == "dmg-norm").unwrap();
        assert!((15..=25).contains(&dnorm.value));
        assert_eq!(get(111), Some(dnorm.value as i64), "item_normaldamage");
        assert_eq!(get(154), Some(40), "item_staminadrainpct");
        assert_eq!(get(96), Some(25), "item_fastermovevelocity (move2)");

        let mut rng = crate::rng::Rng::new(46);
        let props = roll_descs(&unique_props("Razortine"), &mut rng);
        let ev = evaluate_all(
            &props.iter().map(|p| p.desc.clone()).collect::<Vec<_>>(),
            &props.iter().map(|p| p.value).collect::<Vec<_>>(),
        );
        let get = |id: i32| ev.emits.iter().find(|e| e.stat_id == id).map(|e| e.value);
        assert_eq!(get(116), Some(50), "item_fractionaltargetac (reduce-ac)");
        assert_eq!(get(150), Some(25), "item_slow");
        assert_eq!(get(0), Some(15), "strength 15");
    }

    /// Codes paramétrés élémentaires : skilltab (param porté), state/fade
    /// (param brut), oskill (nom résolu), skill-rand (Ormus' Robes :
    /// valeur = skill roulé, par = niveau), randclassskill (Torch),
    /// `Light` (Naj's Circlet, majuscule), skips documentés.
    #[test]
    fn param_codes_rand_and_skips() {
        let ev = evaluate(&fixed("skilltab", "4", 2, 2), 2);
        assert_eq!(
            (ev.param_stats[0].stat_id, ev.param_stats[0].param, ev.param_stats[0].value),
            (188, 4, 2)
        );
        let ev = evaluate(&fixed("state", "34", 18, 18), 18);
        assert_eq!((ev.param_stats[0].stat_id, ev.param_stats[0].param), (98, 34));
        let ev = evaluate(&fixed("fade", "", 1, 1), 1);
        assert_eq!(ev.param_stats[0].stat_id, 181);
        let ev = evaluate(&fixed("oskill", "Guided Arrow", 3, 5), 4);
        assert_eq!(ev.skills[0].kind, SkillPropKind::NonClass);
        assert_eq!(ev.skills[0].skill_id, 22, "Guided Arrow id 22");
        assert_eq!(ev.skills[0].level, 4);
        // skill-rand (Ormus' Robes par=3, min 36 max 60) : la VALEUR rollée
        // est l'id de skill, le PAR est le niveau.
        let ev = evaluate(&fixed("skill-rand", "3", 36, 60), 44);
        assert_eq!(ev.skills[0].kind, SkillPropKind::RandomSkill);
        assert_eq!((ev.skills[0].skill_id, ev.skills[0].level), (44, 3));
        // randclassskill (Hellfire Torch min 0 max 6) : valeur = classe, pas de stat.
        let ev = evaluate(&fixed("randclassskill", "", 0, 6), 4);
        assert_eq!(ev.skills[0].kind, SkillPropKind::RandomClassSkill);
        assert_eq!(ev.skills[0].level, 4);
        assert!(ev.emits.is_empty() && ev.param_stats.is_empty());
        // `Light` (Naj's Circlet, setitems prop3) = light.
        let ev = evaluate(&fixed("Light", "", 5, 5), 5);
        assert_eq!(ev.emits[0].stat_id, 89, "item_lightradius");
        // skips documentés : descripteur + valeur jamais perdus, juste non évalués.
        for c in ["dmg-elem", "ethereal", "*Quelconque"] {
            let ev = evaluate(&fixed(c, "", 1, 2), 1);
            assert!(ev.unknown, "{c}");
            assert!(ev.emits.is_empty());
        }
        // *-préfixes réels : évalués comme le code de base.
        assert_eq!(evaluate(&fixed("*hp", "", -10, -10), -10).emits[0].stat_id, 7);
        let ev = evaluate(&fixed("*gethit-skill", "54", 25, 1), 25);
        assert_eq!(ev.skills[0].kind, SkillPropKind::OnGetHit);
        assert_eq!(ev.skills[0].skill_id, 54);
    }

    /// roll_descs : valeurs dans les bornes (ou fixe si max vide),
    /// déterminisme au seed, `evaluate_all` = somme des évaluations.
    #[test]
    fn roll_descs_bounds_and_evaluate_all() {
        let descs = [
            fixed("dmg%", "", 60, 70),
            fixed("str", "", 8, 8),
            fixed("mag%/lvl", "10", 0, 0),
            fixed("openwounds", "", 50, 50),
        ];
        let mut a = crate::rng::Rng::new(12);
        let mut b = crate::rng::Rng::new(12);
        let ra = roll_descs(&descs, &mut a);
        let rb = roll_descs(&descs, &mut b);
        assert_eq!(ra, rb);
        for p in &ra {
            match p.desc.code.as_str() {
                "mag%/lvl" => assert_eq!(p.value, 0, "min/max vides ⇒ roll gelé 0"),
                "str" | "openwounds" => assert_eq!(p.value, p.desc.min, "min=max"),
                _ => assert!((60..=70).contains(&p.value), "dmg% roulé {}", p.value),
            }
        }
        let ev = evaluate_all(&descs, &[65, 8, 10, 50]);
        assert_eq!(ev.emits.len(), 1 + 1 + 1 + 1 + 1, "dmg% paire + str + perlevel + openwounds");
        assert!(!ev.unknown);
        let mut merged = PropEval::default();
        for d in &descs {
            merged.merge(evaluate(d, 1));
        }
        assert_eq!(merged.emits.len(), 5);
    }
}
