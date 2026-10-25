//! `cube_ui` — Étape 18c « UI fidèle » (PLAN-FIDÉLITÉ phase 18, 2026-10-25) :
//! la couche UI du **Cube Horadrim** et des **tooltips d'objets** (fr) du
//! projet diablo-2-wasm. Module PUR côté règles + un bloc `impl World`
//! additif (même pattern que `consumables.rs`) : la page démo
//! `demo/cube-ui.js` consomme ces contrats via les bindings wasm
//! `cubeTransmute` / `cubeItemTooltip` (d2game, append-only).
//!
//! # Tooltips fidèles (mission 18c ②)
//! * **Sources — LES DONNÉES FONT FOI** : `patch_itemstatcost.txt` v1.12
//!   (colonnes `descfunc`/`descval`/`descstrpos`/`descstrneg`/`descstr2` —
//!   exposées par `stats::StatCostTable`) + les 3 tables STRING **fr
//!   réelles** (`tbl::TblSet::bundled_fra` : patchstring > expansionstring >
//!   string) + la résolution affixe/prop → stats v1.12 de
//!   `item_props::evaluate` (le vocabulaire 105 codes des affixes ET le
//!   vocabulaire runewords 117 codes passent par le même évaluateur testé
//!   17a/12b). Aucun libellé fr écrit à la main : tout vient des TBL.
//! * Format d'une ligne de mod : `descfunc` v1.12 (1 = « +N str », 2 =
//!   « N% str », 3 = « str N », 4 = « str N% », 5, 6/7/8 par niveau (+ la
//!   chaîne `increaseswithplaylevelX` « (Selon niv. perso.) »), 13 = +N aux
//!   skills de classe (param → ModStr3a-e/ModStre8a-b), 14 = +N à l'onglet
//!   (StrSklTabItemN, %d substitué), 20 = −N%…) ; `descval = 0` ⇒ la valeur
//!   est cachée (lignes « flag » : Indestructible, Empêche les monstres de
//!   guérir…). `val_shift` appliqué (stats fractionnaires ÷256).
//! * Accroches skill (`hit-skill`, `gethit-skill`, `kill-skill`,
//!   `death-skill`, `levelup-skill`, `charged`, `oskill`, `skill`) : les
//!   chaînes TBL réelles « %d%% chances de lancer niveau %d %s … »
//!   (`ItemExpansiveChanc1/2/X`, `ModitemskonKill/Level/death`,
//!   `ModStre10d` « (%d/%d charges) ») ; le NOM du skill vient de
//!   `patch_skilldesc.txt` (clé `str name`) → TBL fr (ex. oskill Teleport →
//!   « Téléport »). Divergences documentées : funcs 11/12/23 rendues en
//!   texte seul ou valeur substituée sur chaque %d (les 2 emplacements de
//!   `ModStre9u` ne sont pas désambiguïsés au MVP).
//! * Nom fr : base = clé TBL du code (`hax` → « [fs]Hachette »), composé
//!   préfixe/suffixe via `affixes::name_fr` (« Hachette irrégulière de
//!   Santé ») quand des affixes magiques sont portés ; rare/unique/set =
//!   `name_resolved` transmis tel quel (déjà composé par les étapes 7b/17c).
//! * Couleur D2 (code retail 0-9) : Normal/Low/Superior 0 (blanc),
//!   Magic 3 (bleu), Rare 9 (jaune), Unique 4 (or), Set 2 (vert),
//!   Crafted 8 (orange) ; un runeword s'affiche en OR (fidèle) avec la
//!   base en dessous. Le JS reçoit `color` (code) + `colorHex`.
//!
//! # Transmutation (mission 18c ①)
//! * [`World::cube_transmute_slots`] : les objets VISÉS (slots d'inventaire
//!   10×4) sont convertis en [`CubeItem`] (miroir `From<&RolledItem>`) et
//!   passés au moteur `cube::HoradricCube` (145 recettes réelles, premier
//!   match gagne, backtracking, no-op propre SANS RNG). Succès ⇒ les slots
//!   consommés sont VIDÉS, le produit est posé à la première place libre
//!   (même règle que le pickup `world.rs`), nom fr composé ; échec ⇒
//!   `Err` en fr, AUCUN état touché (ni inventaire, ni RNG).
//! * Portails (L4/L150 : Cow/Pandemonium) : sortie non-item — `portal`
//!   renseigné, message fr « Un portail s'ouvre… », branchement gameplay
//!   restant (annoncé).
//! * Divergences documentées : l'inventaire moteur (`ItemDef`) ne porte
//!   NI qualité NI affixes (contrat MVP) — le produit complet
//!   (`RolledItem` : qualité/affix_ids/alvéoles/requis) est rendu en JSON à
//!   la page qui le garde pour le tooltip ; une transmutation est
//!   définitive (pas d'undo) comme au drop. Le panneau de la page retient
//!   des RÉFÉRENCES (slots) jusqu'au clic Transmuter — fidèle au moteur,
//!   l'objet ne « part » dans le cube qu'à la transmutation.
//!
//! # Endpoints
//! * [`tooltip_json`] — tooltip complet en JSON (`{name,color,colorHex,
//!   baseName,lines}`) ; PUR et déterministe (aucun RNG, TBL immuables).
//! * [`World::cube_transmute_slots`] — JSON d'issue (`{ok,line,recipe,
//!   consumed,item,portal,message}`) ou `Err(fr)`.
//!
//! # Tests (goldens, données réelles)
//! * Tooltip MAGIC fr non vide (affixes roulés seedés par `decorate_item`,
//!   nom composé fr, une ligne par stat d'affixe, couleur 3).
//! * Tooltip RUNEWORD fr non vide (Enigma réel : socle uui 3 alvéoles,
//!   Jah+Ith+Ber → « Défense », « +2 pour toutes les aptitudes », oskill
//!   « Téléport », « (Selon niv. perso.) », couleur or 4).
//! * Tooltip d'un objet simple d'inventaire (base table : dégâts/défense,
//!   requis, nom fr TBL, blanc).
//! * `cube_transmute_slots` : gemmes 3×gcv → gfv (slots vidés, produit
//!   posé, message fr), recette absente ⇒ `Err` fr SANS mutation, 3
//!   anneaux magiques → amulette, slot hors bornes/vide, inventaire plein.

#![allow(dead_code)] // contrat consommé par d2game/demo (pattern cube.rs)

use std::sync::OnceLock;

use serde::Deserialize;

use crate::affixes::{self, AffixKind, AffixTables};
use crate::cube::{CubeItem, HoradricCube};
use crate::item_props::{self, PropDescriptor, PropEval};
use crate::item_roll::{resolve_affix, RolledItem, RolledQuality};
use crate::rng::Rng;
use crate::skills_tree::SkillTreeData;
use crate::stats::StatCostTable;
use crate::tbl::TblSet;
use crate::tsv::Tsv;
use crate::world::INV_SLOTS;
use crate::{ItemDef, ItemKind, World};

/// MagicPrefix/Suffix + AutoMagic v1.12 (résolution des `affix_ids`) —
/// mêmes copies que `cube.rs`.
const MAGIC_PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
const MAGIC_SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");
/// Armes/armures v1.12 : famille + champs de base des objets simples.
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");

/// Tables STRING fr réelles (embarquées, parse au premier accès).
fn strings() -> &'static TblSet {
    static SET: OnceLock<TblSet> = OnceLock::new();
    SET.get_or_init(TblSet::bundled_fra)
}

/// Tables d'affixes réelles (parse au premier accès ; `None` = tables
/// illisibles — jamais de panique, les tuples restent affichables via les
/// libellés de repli).
fn affix_tables() -> Option<&'static AffixTables> {
    static TABLES: OnceLock<Option<AffixTables>> = OnceLock::new();
    TABLES
        .get_or_init(|| affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT))
        .as_ref()
}

/// Texte fr d'une clé TBL (repli : la clé elle-même — jamais vide).
fn tbl_fr(key: &str) -> String {
    strings().get(key).unwrap_or_else(|| key.to_string())
}

// ---------------------------------------------------------------------------
// Tooltip — vue d'entrée (JSON page) + rendu fr
// ---------------------------------------------------------------------------

/// Une prop supplémentaire (`runeword`/gemme/cube) : descripteur brut v1.12
/// + valeur déjà roulée (contrat `RolledProp` de runewords.rs).
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PropJson {
    /// Code prop (`T1CodeN` des tables : `dmg%`, `str/lvl`, `oskill`…).
    #[serde(default)]
    pub code: String,
    /// Param brut (`T1ParamN` : skill/classe/onglet — `""` à défaut).
    #[serde(default)]
    pub param: String,
    /// `T1MinN` (procs = chance %, charged = charges).
    #[serde(default)]
    pub min: i32,
    /// `T1MaxN` (procs = niveau du sort lancé, charged = niveau).
    #[serde(default)]
    pub max: i32,
    /// Valeur roulée (runewords.rs : formule ilvl pour ac/ac%/dmg%).
    #[serde(default)]
    pub value: i32,
}

/// Vue légère d'un objet pour le tooltip — tout optionnel/default : les
/// objets simples d'inventaire (`ItemDef` : code+nom) s'enrichissent côté
/// moteur (base tables), les produits cube/runewords arrivent complets.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct TooltipItem {
    /// Code table de la base (`hax`, `gcv`, `uui`…).
    #[serde(default)]
    pub code: String,
    /// Nom transmis (`ItemDef.name` ou `name_resolved`) — repli affichage.
    #[serde(default)]
    pub name: String,
    /// Qualité (couleur du nom).
    #[serde(default)]
    pub quality: RolledQuality,
    /// `(id affixe, valeur)` — contrat `RolledItem::affix_ids`.
    #[serde(default)]
    pub affix_ids: Vec<(u32, u32)>,
    /// Alvéoles posées (« Serti (N) »).
    #[serde(default)]
    pub sockets: u8,
    /// Montant d'or (`gld`).
    #[serde(default)]
    pub gold: u32,
    /// Dégâts de la base (arme — sinon enrichis depuis la table).
    #[serde(default)]
    pub min_dmg: u32,
    #[serde(default)]
    pub max_dmg: u32,
    /// Défense (armure — enrichie plage table pour un objet simple).
    #[serde(default)]
    pub ac: u32,
    /// Requis de la base (enrichis depuis les tables si nuls).
    #[serde(default)]
    pub req_str: u32,
    #[serde(default)]
    pub req_dex: u32,
    #[serde(default)]
    pub req_level: u32,
    /// Nom du runeword (l'objet affiche ce nom en OR + la base en dessous).
    #[serde(default)]
    pub runeword: Option<String>,
    /// Props supplémentaires (runeword/gemme/cube) — une ligne chacune.
    #[serde(default)]
    pub props: Vec<PropJson>,
}

/// Construit la vue depuis un objet d'inventaire simple (le moteur enrichit
/// base/requis depuis les tables réelles — cf. [`enrich_from_tables`]).
pub fn tooltip_item_from_def(def: &ItemDef) -> TooltipItem {
    TooltipItem {
        code: def.code.clone(),
        name: def.name.clone(),
        quality: RolledQuality::Normal,
        gold: def.amount,
        ..Default::default()
    }
}

/// Construit la vue depuis un [`RolledItem`] complet (produit du cube).
pub fn tooltip_item_from_rolled(r: &RolledItem) -> TooltipItem {
    TooltipItem {
        code: r.code.clone(),
        name: r.name_resolved.clone(),
        quality: r.quality,
        affix_ids: r.affix_ids.clone(),
        sockets: r.sockets,
        gold: r.gold,
        min_dmg: r.min_dmg,
        max_dmg: r.max_dmg,
        ac: r.ac,
        req_str: r.req_str,
        req_dex: r.req_dex,
        req_level: r.req_level,
        ..Default::default()
    }
}

/// Code couleur D2 retail d'une qualité (0 blanc, 3 bleu, 9 jaune, 4 or,
/// 2 vert set, 8 orange crafted).
pub fn quality_color(q: RolledQuality) -> u8 {
    match q {
        RolledQuality::Low(_) | RolledQuality::Normal | RolledQuality::Superior => 0,
        RolledQuality::Magic => 3,
        RolledQuality::Rare => 9,
        RolledQuality::Set => 2,
        RolledQuality::Unique => 4,
        RolledQuality::Crafted => 8,
    }
}

/// Hex CSS correspondant (rendu page — même palette que D2).
pub fn quality_color_hex(q: RolledQuality) -> &'static str {
    match q {
        RolledQuality::Low(_) => "#b0b0b0",
        RolledQuality::Normal | RolledQuality::Superior => "#e8e8e8",
        RolledQuality::Magic => "#6969ff",
        RolledQuality::Rare => "#ffff64",
        RolledQuality::Set => "#00ff00",
        RolledQuality::Unique => "#a59263",
        RolledQuality::Crafted => "#ff8040",
    }
}

/// Nom fr de la BASE : clé TBL = code (`hax` → « Hachette »), repli nom
/// transmis (anglais table) — jamais vide.
pub fn fr_base_name(code: &str, fallback: &str) -> String {
    let raw = strings().get(code).unwrap_or_default();
    if raw.is_empty() {
        return if fallback.is_empty() { code.to_string() } else { fallback.to_string() };
    }
    let g = affixes::parse_gendered(&raw);
    let n = g.first().trim().to_string();
    if n.is_empty() { fallback.to_string() } else { n }
}

/// Nom fr COMPOSÉ d'un objet : runeword = nom transmis (le runeword EST le
/// nom affiché, en or) ; rare/unique/set = nom transmis (déjà composé par
/// 7b/17c) ; sinon affixes magiques portés ⇒ `affixes::name_fr` (« Base
/// <préfixe accordé> <suffixe> ») ; sinon base seule. `affix_tables()`
/// absent ⇒ repli base.
fn fr_item_name(item: &TooltipItem) -> String {
    if item.runeword.is_some()
        || matches!(item.quality, RolledQuality::Rare | RolledQuality::Unique | RolledQuality::Set)
            && !item.name.is_empty()
    {
        return item.name.clone();
    }
    let (prefixes, suffixes) = match affix_tables() {
        Some(t) if !item.affix_ids.is_empty() => {
            let mut p = Vec::new();
            let mut s = Vec::new();
            for &(id, _) in &item.affix_ids {
                if let Some((rec, kind)) = resolve_affix(t, id) {
                    match kind {
                        AffixKind::Prefix => p.push(rec),
                        AffixKind::Suffix => s.push(rec),
                    }
                }
            }
            (p, s)
        }
        _ => (Vec::new(), Vec::new()),
    };
    if prefixes.is_empty() && suffixes.is_empty() {
        return fr_base_name(&item.code, &item.name);
    }
    affixes::name_fr(strings(), &item.code, &item.name, &prefixes, &suffixes)
}

/// Une ligne « %d » : substitue CHAQUE `%d` par `v` (les 2 emplacements de
/// `ModStre9u` ne sont pas désambiguïsés — divergence documentée).
fn subst_d(text: &str, v: i32) -> String {
    let mut out = String::with_capacity(text.len() + 8);
    let mut rest = text;
    while let Some(i) = rest.find("%d") {
        out.push_str(&rest[..i]);
        out.push_str(&v.to_string());
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// Une ligne « %d %d %s » : proba, niveau, nom du skill (procs).
fn subst_pds(text: &str, prob: i32, level: i32, skill: &str) -> String {
    let mut out = String::with_capacity(text.len() + 16);
    let mut rest = text;
    for v in [prob, level] {
        while let Some(i) = rest.find("%d") {
            out.push_str(&rest[..i]);
            out.push_str(&v.to_string());
            rest = &rest[i + 2..];
            break;
        }
    }
    if let Some(i) = rest.find("%s") {
        out.push_str(&rest[..i]);
        out.push_str(skill);
        rest = &rest[i + 2..];
    }
    out.push_str(rest);
    out
}

/// Nom fr d'un skill : id numérique (saves) ou code (`patch_skills.txt`) →
/// clé `skilldesc` `str name` → TBL fr (« Téléport ») ; repli « sort ».
fn skill_name_fr(id_or_code: i32, raw_param: &str) -> String {
    let tree = SkillTreeData::global();
    let code = if id_or_code >= 0 {
        tree.records().iter().find(|r| r.id as i32 == id_or_code).map(|r| r.code.clone())
    } else {
        None
    }
    .or_else(|| {
        let p = raw_param.trim();
        (!p.is_empty() && p.parse::<i32>().is_err()).then(|| p.to_string())
    });
    let Some(code) = code else { return "sort".to_string() };
    let key = tree.desc_of(&code).map(|d| d.str_name.clone()).unwrap_or_default();
    if key.is_empty() {
        return code;
    }
    let raw = strings().get(&key).unwrap_or_default();
    if raw.is_empty() { code } else { raw.trim().trim_end_matches('\n').trim().to_string() }
}

/// Ligne fr d'une stat émise (ids ItemStatCost) — `descfunc` v1.12.
/// `None` = pas de description (descfunc 0 / stat inconnue).
fn stat_line_fr(stat_name: &str, value: i64, param: i32) -> Option<String> {
    let isc = StatCostTable::global();
    let rec = isc.get(stat_name)?;
    let func = rec.desc_func;
    if func == 0 {
        return None;
    }
    // ValShift : les stats fractionnaires s'affichent ÷256 (donnée).
    let v = value >> rec.val_shift;
    let key = if v < 0 && !rec.desc_neg.is_empty() { &rec.desc_neg } else { &rec.desc_pos };
    if key.is_empty() {
        return None;
    }
    let text = tbl_fr(key);
    let show = rec.desc_val != 0
        || matches!(func, 11 | 13 | 14 | 15 | 16 | 17 | 18 | 24 | 27);
    let abs = v.unsigned_abs() as i64;
    let line = match func {
        // 1 « +N <str> », 2 « N% <str> »
        1 | 2 => {
            if !show {
                text.clone()
            } else if func == 1 {
                if v < 0 { format!("-{abs} {text}") } else { format!("+{v} {text}") }
            } else {
                format!("{v}% {text}")
            }
        }
        // 3 « <str> N », 4 « <str> N% », 5 « N <str> »
        3 | 4 | 5 => {
            if !show {
                text.clone()
            } else {
                let unit = if func == 4 { "%" } else { "" };
                if v < 0 {
                    format!("{text} -{abs}{unit}")
                } else {
                    format!("{text} +{v}{unit}")
                }
            }
        }
        // Par niveau (6/7/8) : la chaîne str2 « (Selon niv. perso.) » suit.
        6 | 7 | 8 => {
            let mut l = if v < 0 {
                match func {
                    8 => format!("{text} -{abs}%"),
                    7 => format!("-{abs}% {text}"),
                    _ => format!("-{abs} {text}"),
                }
            } else {
                match func {
                    8 => format!("{text} +{v}%"),
                    7 => format!("+{v}% {text}"),
                    _ => format!("+{v} {text}"),
                }
            };
            if !rec.desc_str2.is_empty() {
                l.push(' ');
                l.push_str(&tbl_fr(&rec.desc_str2));
            }
            l
        }
        // 9 « <str> Nx » (épines par niveau)
        9 => format!("{text} {v}x"),
        // 11 Réparation : valeur substituée sur chaque %d (divergence ①).
        11 => subst_d(&text, v as i32),
        // 12 « flag x N » : la table fr porte déjà la phrase complète.
        12 => text.clone(),
        // 13 +N aux skills de CLASSE : param → onglet ModStr3a-e/ModStre8a-b.
        13 => {
            let key = if param < 5 {
                format!("ModStr3{}", (b'a' + param.max(0) as u8) as char)
            } else {
                format!("ModStre8{}", (b'a' + (param - 5).max(0) as u8) as char)
            };
            let cls = tbl_fr(&key);
            if v < 0 { format!("-{abs} {cls}") } else { format!("+{v} {cls}") }
        }
        // 14 +N à l'ONGLET : clé StrSklTabItem{param+1}, %d substitué.
        14 => {
            let key = format!("StrSklTabItem{}", param.max(0) + 1);
            subst_d(&tbl_fr(&key), v as i32)
        }
        // 15 procs : la clé porte « %d%% chances de lancer niveau %d %s … ».
        15 => subst_pds(&text, v as i32, param, "sort"),
        // 16/17/18 (+durée) : même gabarit que 1/2 avec la valeur toujours.
        16 | 17 | 18 => {
            let unit = if func == 18 { "%" } else { "" };
            if v < 0 { format!("-{abs}{unit} {text}") } else { format!("+{v}{unit} {text}") }
        }
        // 20 −N%
        20 => format!("-{abs}% {text}"),
        // 22/21 +N% / 23 N% (+str2)
        21 | 22 | 23 => {
            let mut l = format!("+{v}% {text}");
            if func == 23 && !rec.desc_str2.is_empty() {
                l.push(' ');
                l.push_str(&tbl_fr(&rec.desc_str2));
            }
            l
        }
        // 24 charged : « (%d/%d charges) » (niveau, charges) + nom skill.
        24 => format!("{} ({}/{})", "charges", param, v),
        // 27 +N au skill (param = skill) — nom ajouté par l'appelant.
        27 => {
            if v < 0 { format!("-{abs} {text}") } else { format!("+{v} {text}") }
        }
        _ => return None,
    };
    let l = line.trim().to_string();
    (!l.is_empty()).then_some(l)
}

/// Ligne fr d'une accroche skill (les chaînes TBL réelles des procs).
fn skill_line_fr(ev: &PropEval, raw_param: &str) -> Vec<String> {
    let mut out = Vec::new();
    for s in &ev.skills {
        let name = skill_name_fr(s.skill_id, raw_param);
        let line = match s.kind {
            item_props::SkillPropKind::OnAttack => {
                subst_pds(&tbl_fr("ItemExpansiveChanc1"), s.prob, s.level, &name)
            }
            item_props::SkillPropKind::OnGetHit => {
                subst_pds(&tbl_fr("ItemExpansiveChanc2"), s.prob, s.level, &name)
            }
            item_props::SkillPropKind::OnKill => {
                subst_pds(&tbl_fr("ModitemskonKill"), s.prob, s.level, &name)
            }
            item_props::SkillPropKind::OnDeath => {
                subst_pds(&tbl_fr("Moditemskondeath"), s.prob, s.level, &name)
            }
            item_props::SkillPropKind::OnLevelUp => {
                subst_pds(&tbl_fr("ModitemskonLevel"), s.prob, s.level, &name)
            }
            // Charged : « Niveau X : N charges (skill) » — chaîne réelle
            // `ModStre10d` « (%d/%d charges) » (niveau, charges).
            item_props::SkillPropKind::Charged => {
                format!("{} {}", name, subst_d(&tbl_fr("ModStre10d"), s.charges))
                    .replace("((", "(")
            }
            // +N au skill (classe du héros) / oskill (toutes classes).
            item_props::SkillPropKind::Single => format!("+{} {}", s.level, name),
            item_props::SkillPropKind::NonClass => {
                format!("Niveau {} : {} (toutes les classes)", s.level, name)
            }
            item_props::SkillPropKind::RandomSkill | item_props::SkillPropKind::RandomClassSkill => {
                format!("+{} à un sort tiré", s.level)
            }
        };
        let l = line.trim().to_string();
        if !l.is_empty() {
            out.push(l);
        }
    }
    out
}

/// Toutes les lignes fr d'un descripteur + valeur (stats émises + skills).
fn desc_lines_fr(desc: &PropDescriptor, value: i32) -> Vec<String> {
    let ev = item_props::evaluate(desc, value);
    let mut out = Vec::new();
    for e in ev.emits.iter().chain(ev.param_stats.iter()) {
        let Some(name) = StatCostTable::global().name_of_id(e.stat_id) else { continue };
        if let Some(l) = stat_line_fr(name, e.value, e.param) {
            out.push(l);
        }
    }
    let param = desc.param.clone();
    out.extend(skill_line_fr(&ev, &param));
    out
}

/// Champs de base d'un code simple (armes/armures — table réelle) :
/// `(famille, mindam, maxdam, minac, maxac, reqstr, reqdex, levelreq)`.
fn base_of(code: &str) -> Option<(bool, u32, u32, u32, u32, u32, u32, u32)> {
    for (txt, weapon) in [(WEAPONS_TXT, true), (ARMOR_TXT, false)] {
        let Ok(t) = Tsv::parse(txt) else { continue };
        for i in 0..t.len() {
            let Some(r) = t.rec(i) else { continue };
            if r.s("code") == code {
                return Some((
                    weapon,
                    r.u("mindam"),
                    r.u("maxdam"),
                    r.u("minac"),
                    r.u("maxac"),
                    r.u("reqstr"),
                    r.u("reqdex"),
                    r.u("levelreq"),
                ));
            }
        }
    }
    None
}

/// Tooltip COMPLET en JSON : `{name, color, colorHex, baseName, lines[]}`.
/// PUR + déterministe (aucun RNG ; TBL/ISC immuables).
pub fn tooltip_json(item: &TooltipItem) -> String {
    // 1. Nom fr (couleur qualité) + base éventuelle (runeword : or + base).
    let name = fr_item_name(item);
    // Runeword = nom en OR (fidèle), même si le socle est de qualité blanche.
    let color = if item.runeword.is_some() { 4 } else { quality_color(item.quality) };
    let hex = quality_color_hex(item.quality);
    let base_name = item.runeword.as_ref().map(|_| fr_base_name(&item.code, &item.name));

    let mut lines: Vec<String> = Vec::new();
    // 2. Champs de base : enrichis depuis les tables si l'appelant ne les
    //    porte pas (objet simple d'inventaire).
    let (min_dmg, max_dmg, ac, ac_max, req_str, req_dex, req_level) = {
        match base_of(&item.code) {
            Some((weapon, mindam, maxdam, minac, maxac, rs, rd, rl)) => (
                if item.min_dmg > 0 { item.min_dmg } else if weapon { mindam } else { 0 },
                if item.max_dmg > 0 { item.max_dmg } else if weapon { maxdam } else { 0 },
                if item.ac > 0 { item.ac } else if !weapon { minac } else { 0 },
                if !weapon { maxac } else { 0 },
                if item.req_str > 0 { item.req_str } else { rs },
                if item.req_dex > 0 { item.req_dex } else { rd },
                if item.req_level > 0 { item.req_level } else { rl },
            ),
            None => {
                (item.min_dmg, item.max_dmg, item.ac, 0, item.req_str, item.req_dex, item.req_level)
            }
        }
    };
    if min_dmg > 0 || max_dmg > 0 {
        lines.push(format!("Dégâts : {min_dmg}-{max_dmg}"));
    }
    if ac > 0 {
        // Objet simple (non rollé) : plage réelle de la table ; item
        // produit : AC rollé exact.
        if ac_max > ac {
            lines.push(format!("Défense : {ac}-{ac_max}"));
        } else {
            lines.push(format!("Défense : {ac}"));
        }
    }
    if req_str > 0 {
        lines.push(format!("Force requise : {req_str}"));
    }
    if req_dex > 0 {
        lines.push(format!("Dextérité requise : {req_dex}"));
    }
    if req_level > 0 {
        lines.push(format!("Niveau requis : {req_level}"));
    }
    if item.sockets > 0 {
        lines.push(format!("Serti ({})", item.sockets));
    }
    if item.gold > 0 {
        lines.push(format!("Montant : {}", item.gold));
    }
    // 3. Props supplémentaires (runeword/gemme/cube) puis affixes — une
    //    ligne chacune (ordre : socle d'abord, affixes ensuite).
    for p in &item.props {
        let desc = PropDescriptor {
            code: p.code.clone(),
            param: p.param.clone(),
            min: p.min,
            max: p.max,
        };
        lines.extend(desc_lines_fr(&desc, p.value));
    }
    if let Some(t) = affix_tables() {
        for &(id, value) in &item.affix_ids {
            if let Some((rec, _)) = resolve_affix(t, id) {
                for d in &rec.mods {
                    lines.extend(desc_lines_fr(d, value as i32));
                }
            }
        }
    }
    serde_json::json!({
        "name": name,
        "color": color,
        "colorHex": hex,
        "baseName": base_name,
        "lines": lines,
    })
    .to_string()
}

// ---------------------------------------------------------------------------
// Transmutation côté monde — World::cube_transmute_slots (additif)
// ---------------------------------------------------------------------------

/// Code misc → Potion ? (codes potions réels v1.12 — hp1-5/mp1-5/rvs/rvl).
fn is_potion_code(code: &str) -> bool {
    matches!(code, "hp1" | "hp2" | "hp3" | "hp4" | "hp5" | "mp1" | "mp2" | "mp3" | "mp4" | "mp5"
        | "rvs" | "rvl")
}

/// Kind d'inventaire d'un code produit (armes/armures = tables, potions,
/// sinon Misc — miroir `loot::to_def`).
fn kind_of(code: &str) -> ItemKind {
    match base_of(code) {
        Some((true, ..)) => ItemKind::Weapon,
        Some((false, ..)) => ItemKind::Armor,
        None if is_potion_code(code) => ItemKind::Potion,
        None => ItemKind::Misc,
    }
}

/// Nom fr d'un produit : base TBL, composé `affixes::name_fr` si affixes
/// magiques portés ; rare/unique/set/crafted = nom résolu transmis.
fn product_name_fr(item: &RolledItem) -> String {
    let view = TooltipItem {
        code: item.code.clone(),
        name: item.name_resolved.clone(),
        quality: item.quality,
        affix_ids: item.affix_ids.clone(),
        ..Default::default()
    };
    fr_item_name(&view)
}

/// Transmute les objets AUX SLOTS d'inventaire donnés via le moteur
/// `cube.rs` (145 recettes réelles). Succès ⇒ JSON d'issue (`{ok,line,
/// recipe,consumed,item,portal,message}`) ; échec ⇒ `Err` FR, état intact
/// (ni inventaire, ni RNG — no-op propre garanti par le moteur).
///
/// * `slots` : indices 0..40 (grille 10×4, rangée par rangée) ; les objets
///   excédentaires ne bloquent pas (fidèle D2) — `consumed` liste les
///   positions DANS `slots` réellement consommées.
/// * `seed` : graine RNG de la transmutation (la page passe une graine de
///   session — le moteur est déterministe par graine, goldens cube.rs).
pub fn world_cube_transmute_slots(
    w: &mut World,
    slots: &[usize],
    seed: u64,
) -> Result<String, String> {
    if slots.is_empty() {
        return Err("Le cube est vide — placez d'abord des objets.".to_string());
    }
    // 1. Vue CubeItem des slots (aucune mutation avant un match).
    let mut items: Vec<CubeItem> = Vec::with_capacity(slots.len());
    for &s in slots {
        if s >= INV_SLOTS {
            return Err(format!("Slot d'inventaire invalide ({s})."));
        }
        let Some(def) = &w.inventory().get(s).cloned().flatten() else {
            return Err(format!("Le slot {} est vide.", s + 1));
        };
        items.push(CubeItem {
            code: def.code.clone(),
            name: def.name.clone(),
            quality: RolledQuality::Normal,
            sockets: 0,
        });
    }
    // 2. Moteur (LA table fait foi) — no-op propre si aucune recette.
    let cube = HoradricCube::embedded().ok_or_else(|| "Cube indisponible (table illisible).".to_string())?;
    let Some(outcome) = cube.transmute(&items, &mut Rng::new(seed)) else {
        return Err("Aucune recette du Cube Horadrim ne correspond à ces objets.".to_string());
    };
    // 3. Portail : sortie non-item, les inputs sont consommés.
    if outcome.portal.is_some() {
        for &i in &outcome.consumed {
            let slot = slots[i];
            w.players[0].data.inv[slot] = None;
        }
        let portal = outcome.portal.clone().unwrap_or_default();
        return Ok(serde_json::json!({
            "ok": true,
            "line": outcome.line,
            "recipe": recipe_label(&cube, outcome.line),
            "consumed": outcome.consumed,
            "item": serde_json::Value::Null,
            "portal": portal,
            "message": format!("Un portail s'ouvre : {portal} ! (branchement gameplay restant)"),
        })
        .to_string());
    }
    let Some(produced) = &outcome.item else {
        return Err("La recette n'a produit aucun objet.".to_string());
    };
    // 4. Place libre AVANT toute mutation (échec = zéro effet de bord).
    let Some(free) = (0..INV_SLOTS).find(|&s| w.inventory()[s].is_none()) else {
        return Err("Inventaire plein — transmutation annulée.".to_string());
    };
    // 5. Consomme les slots consommés, pose le produit (nom fr).
    for &i in &outcome.consumed {
        let slot = slots[i];
        w.players[0].data.inv[slot] = None;
    }
    let name_fr = product_name_fr(produced);
    let def = ItemDef {
        code: produced.code.clone(),
        name: name_fr.clone(),
        kind: kind_of(&produced.code),
        amount: produced.gold,
    };
    w.players[0].data.inv[free] = Some(def);
    let full = tooltip_item_from_rolled(produced);
    let full_json: serde_json::Value =
        serde_json::from_str(&tooltip_json(&full)).unwrap_or_default();
    let out = serde_json::json!({
        "ok": true,
        "line": outcome.line,
        "recipe": recipe_label(&cube, outcome.line),
        "consumed": outcome.consumed,
        "portal": serde_json::Value::Null,
        "message": format!("Transmutation réussie : {name_fr}."),
        "slot": free,
        // Produit COMPLET (qualité/affixes/requis) — la page le garde pour
        // le tooltip (l'ItemDef d'inventaire MVP ne porte que code/nom/kind).
        "item": {
            "slot": free,
            "code": produced.code,
            "name": name_fr,
            "kind": format!("{:?}", kind_of(&produced.code)),
            "tooltip": full_json,
            "rolled": {
                "quality": format!("{:?}", produced.quality),
                "affixIds": produced.affix_ids,
                "sockets": produced.sockets,
                "minDmg": produced.min_dmg,
                "maxDmg": produced.max_dmg,
                "ac": produced.ac,
                "reqStr": produced.req_str,
                "reqDex": produced.req_dex,
                "reqLevel": produced.req_level,
            },
        },
    });
    Ok(out.to_string())
}

/// Libellé de table de la recette citée (col `description`, anglais v1.12).
fn recipe_label(cube: &HoradricCube, line: usize) -> String {
    cube.recipes()
        .iter()
        .find(|r| r.line == line)
        .map(|r| r.description.clone())
        .unwrap_or_default()
}

impl World {
    /// 18c : transmute les objets aux slots donnés (contrat
    /// [`world_cube_transmute_slots`]) — wrapper méthodique pour la page.
    pub fn cube_transmute_slots(&mut self, slots: &[usize], seed: u64) -> Result<String, String> {
        world_cube_transmute_slots(self, slots, seed)
    }
}

// ---------------------------------------------------------------------------
// Tests — goldens (tables réelles, tout déterministe)
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::affixes::decorate_item;

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    fn world() -> World {
        World::with_seed(
            open_grid(56, 40),
            serde_json::from_str(include_str!("../../../../assets/manifest.json")).unwrap(),
            7,
        )
    }

    fn put(w: &mut World, slot: usize, code: &str) {
        w.players[0].data.inv[slot] = Some(ItemDef {
            code: code.to_string(),
            name: code.to_string(),
            kind: kind_of(code),
            amount: 0,
        });
    }

    fn parse_tooltip(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("tooltip JSON valide")
    }

    // ══ GOLDEN ① — tooltip d'un item MAGIC (fr, non vide) ══

    /// Une Hachette magique décorée par `decorate_item` (7b, seed figée) :
    /// nom composé fr (base TBL + préfixe/suffixe), couleur 3 (bleu), une
    /// ligne par stat d'affixe, aucune ligne vide.
    #[test]
    fn golden_tooltip_magic_item_fr() {
        let tables = affix_tables().expect("tables affixes réelles").clone();
        let strings = strings().clone();
        let mut item = RolledItem {
            code: "hax".into(),
            quality: RolledQuality::Magic,
            name_resolved: String::new(),
            ..Default::default()
        };
        // hax : type réel `axe` (table), arme de mêlée.
        let types = crate::affixes::ItemTypeSet {
            wtype: "axe",
            is_weap: true,
            is_mele: true,
            is_bow: false,
            is_armo: false,
        };
        decorate_item(&mut Rng::new(0x18C), &mut item, &tables, &strings, &types, "Hand Axe", 6, 6, false);
        assert!(!item.affix_ids.is_empty(), "affixes roulés (magic)");
        let view = tooltip_item_from_rolled(&item);
        let tip = parse_tooltip(&tooltip_json(&view));
        // Nom fr composé ≠ nom de table anglais, non vide.
        let name = tip["name"].as_str().unwrap();
        assert!(!name.trim().is_empty());
        assert_ne!(name, "Hand Axe", "le nom passe par la base TBL fr ou le composé affixe");
        assert_eq!(tip["color"].as_u64(), Some(3), "magic = bleu (code retail 3)");
        assert_eq!(tip["colorHex"].as_str(), Some("#6969ff"));
        // Une ligne par stat d'affixe au minimum, toutes non vides et fr.
        let lines: Vec<&str> = tip["lines"].as_array().unwrap().iter().map(|l| l.as_str().unwrap()).collect();
        assert!(!lines.is_empty(), "tooltip magic non vide");
        assert!(lines.len() >= item.affix_ids.len(), "{} lignes pour {} affixes", lines.len(), item.affix_ids.len());
        for l in &lines {
            assert!(!l.trim().is_empty(), "aucune ligne vide");
            assert!(!l.contains("mod"), "pas de code brut : {l}");
        }
        // Dégâts de la base (hax = table réelle : 2-3).
        assert!(lines.iter().any(|l| l.starts_with("Dégâts : ")), "ligne dégâts : {lines:?}");
    }

    // ══ GOLDEN ② — tooltip d'un RUNEWORD (fr, non vide) ══

    /// Enigma réelle (17a : socle uui 3 alvéoles, Jah+Ith+Ber, ilvl 99) :
    /// nom en OR (code 4), base « Linceul crépusculaire » en dessous, et
    /// les mods réels rendus en fr via ItemStatCost+TBL : Défense, +2 pour
    /// toutes les aptitudes, oskill Téléport, str/mag% par niveau « (Selon
    /// niv. perso.) », Vie après chaque mort de Démon, Marche/Course.
    #[test]
    fn golden_tooltip_runeword_fr() {
        let t = crate::runewords::RuneWordTable::global();
        let runes: Vec<String> = ["r31", "r06", "r30"].iter().map(|s| s.to_string()).collect();
        let socle = crate::runewords::RunewordItem {
            base_code: "uui",
            ilvl: 99,
            sockets: 3,
            contents: &runes,
            white_base: true,
        };
        let rec = t.resolve(&socle).expect("Enigma résoluble (17a)");
        let result = t.apply(rec, 99, &mut Rng::new(0x18C));
        // Vue tooltip : nom runeword en or + props roulées.
        let view = TooltipItem {
            code: "uui".into(),
            name: result.name.clone(),
            quality: RolledQuality::Normal,
            runeword: Some(result.name.clone()),
            props: result
                .props
                .iter()
                .map(|p| PropJson {
                    code: p.desc.code.clone(),
                    param: p.desc.param.clone(),
                    min: p.desc.min,
                    max: p.desc.max,
                    value: p.value,
                })
                .collect(),
            ..Default::default()
        };
        let tip = parse_tooltip(&tooltip_json(&view));
        assert_eq!(tip["name"].as_str(), Some("Enigma"));
        assert_eq!(tip["color"].as_u64(), Some(4), "runeword = or (code retail 4)");
        assert_eq!(tip["baseName"].as_str(), Some("Linceul crépusculaire"), "base fr TBL (uui)");
        let lines: Vec<String> = tip["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();
        assert!(!lines.is_empty(), "tooltip runeword non vide");
        let joined = lines.join(" | ");
        // Mods réels d'Enigma rendus en fr (données 17a + TBL réelles).
        assert!(lines.iter().any(|l| l.starts_with("Défense : ")), "{joined}");
        assert!(lines.iter().any(|l| l.contains("pour toutes les aptitudes")), "{joined}");
        assert!(lines.iter().any(|l| l.contains("Téléport")), "oskill fr : {joined}");
        assert!(lines.iter().any(|l| l.contains("(Selon niv. perso.)")), "str/lvl + mag%/lvl : {joined}");
        assert!(lines.iter().any(|l| l.contains("Vie après chaque") || l.contains("victime")), "heal-kill : {joined}");
        assert!(lines.iter().any(|l| l.contains("Marche/Course")), "move2 : {joined}");
        for l in &lines {
            assert!(!l.trim().is_empty());
        }
    }

    // ══ Tooltip d'un objet SIMPLE (base tables, nom fr, blanc) ══

    #[test]
    fn tooltip_plain_inventory_item_fr() {
        // Arme simple : dégâts + requis de la table, nom fr TBL, blanc.
        let view = tooltip_item_from_def(&ItemDef {
            code: "hax".into(),
            name: "Hand Axe".into(),
            kind: ItemKind::Weapon,
            amount: 0,
        });
        let tip = parse_tooltip(&tooltip_json(&view));
        assert_eq!(tip["name"].as_str(), Some("Hachette"), "base fr TBL (hax)");
        assert_eq!(tip["color"].as_u64(), Some(0));
        let lines: Vec<String> = tip["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();
        assert!(lines.iter().any(|l| l.starts_with("Dégâts : ")), "{lines:?}");
        // Armure simple : Défense = plage table (minac-maxac).
        let view = tooltip_item_from_def(&ItemDef {
            code: "uui".into(),
            name: "Ghost Armor".into(),
            kind: ItemKind::Armor,
            amount: 0,
        });
        let tip = parse_tooltip(&tooltip_json(&view));
        assert_eq!(tip["name"].as_str(), Some("Linceul crépusculaire"));
        let lines: Vec<String> = tip["lines"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();
        assert!(lines.iter().any(|l| l.starts_with("Défense : ")), "{lines:?}");
        // Potion : nom fr seul, aucune ligne de stats.
        let view = tooltip_item_from_def(&ItemDef {
            code: "rvl".into(),
            name: "Full Rejuv Potion".into(),
            kind: ItemKind::Potion,
            amount: 0,
        });
        let tip = parse_tooltip(&tooltip_json(&view));
        assert_eq!(tip["name"].as_str(), Some("Potion de rajeunissement total"));
        assert!(tip["lines"].as_array().unwrap().is_empty());
    }

    // ══ Transmutation sur l'inventaire réel (World) ══

    /// 3 gemmes ébréchées (L25) : slots consommés vidés, produit gfv posé
    /// à la première place libre, message fr, JSON complet.
    #[test]
    fn cube_transmute_slots_gems_l25() {
        let mut w = world();
        put(&mut w, 5, "gcv");
        put(&mut w, 7, "gcv");
        put(&mut w, 9, "gcv");
        let out = w.cube_transmute_slots(&[5, 7, 9], 1).expect("L25 doit matcher");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["ok"], true);
        assert_eq!(v["line"], 25);
        assert_eq!(v["consumed"], serde_json::json!([0, 1, 2]));
        let item = &v["item"];
        assert_eq!(item["code"], "gfv");
        assert_eq!(item["name"], "Améthyste fissurée", "nom fr TBL (gfv)");
        assert_eq!(item["slot"], 0, "première place libre");
        assert_eq!(v["message"], "Transmutation réussie : Améthyste fissurée.");
        // État réel : 5/7/9 vidés, slot 0 = produit Misc.
        assert!(w.inventory()[5].is_none() && w.inventory()[7].is_none() && w.inventory()[9].is_none());
        let prod = w.inventory()[0].as_ref().expect("produit dans l'inventaire");
        assert_eq!(prod.code, "gfv");
        assert_eq!(prod.kind, ItemKind::Misc);
        assert_eq!(prod.name, "Améthyste fissurée");
        // Tooltip produit embarqué non vide + fr.
        let tip = &item["tooltip"];
        assert!(!tip["name"].as_str().unwrap_or("").trim().is_empty());
        assert_eq!(tip["name"].as_str(), Some("Améthyste fissurée"));
    }

    /// Recette absente ⇒ Err fr ET AUCUNE mutation (slots intacts).
    #[test]
    fn cube_transmute_slots_no_recipe_is_clean_fr_error() {
        let mut w = world();
        put(&mut w, 0, "gcv");
        put(&mut w, 1, "rvl");
        let err = w.cube_transmute_slots(&[0, 1], 3).unwrap_err();
        assert!(err.contains("Aucune recette du Cube Horadrim"), "{err}");
        assert!(w.inventory()[0].is_some() && w.inventory()[1].is_some(), "état intact");
        // Cube vide / slot vide / slot hors bornes : erreurs fr lisibles.
        assert_eq!(w.cube_transmute_slots(&[], 1).unwrap_err(), "Le cube est vide — placez d'abord des objets.");
        assert!(w.cube_transmute_slots(&[0, 1, 2], 1).unwrap_err().contains("est vide"));
        assert!(w.cube_transmute_slots(&[0, 40], 1).unwrap_err().contains("invalide"));
    }

    /// 3 potions de vie + 3 potions de mana + 1 gemme ébréchée → petite
    /// potion de rajeunissement (L6 réelle) — groupes hpot/mpot/gem0
    /// OUVERTS (objets simples d'inventaire, sans qualité).
    #[test]
    fn cube_transmute_slots_potions_l6() {
        let mut w = world();
        for (i, c) in ["hp1", "hp2", "hp3", "mp1", "mp2", "mp3", "gcr"].iter().enumerate() {
            put(&mut w, 10 + i, c);
        }
        let out = w.cube_transmute_slots(&[10, 11, 12, 13, 14, 15, 16], 5).expect("L6 doit matcher");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["line"], 6);
        assert_eq!(v["consumed"], serde_json::json!([0, 1, 2, 3, 4, 5, 6]));
        let item = &v["item"];
        assert_eq!(item["code"], "rvs");
        assert_eq!(item["name"], "Potion de rajeunissement", "nom fr TBL (rvs)");
        assert_eq!(item["kind"], "Potion");
        assert_eq!(w.inventory()[0].as_ref().unwrap().code, "rvs", "posée à la 1re place libre");
    }

    /// Divergence DOCUMENTÉE (18c) : l'inventaire MVP (`ItemDef`) ne porte
    /// PAS la qualité — les recettes exigeant une qualité en ENTRÉE (L15
    /// `rin,mag` : 3 anneaux magiques → amulette, L131 `uni`…) ne
    /// matchent jamais depuis la page ; l'échec reste PROPRE (fr, aucun
    /// objet consommé). La qualité des PRODUITS est préservée (RolledItem
    /// JSON retourné à la page).
    #[test]
    fn cube_transmute_slots_quality_input_divergence_documented() {
        let mut w = world();
        for s in 0..3 {
            put(&mut w, s, "rin");
        }
        let err = w.cube_transmute_slots(&[0, 1, 2], 5).unwrap_err();
        assert!(err.contains("Aucune recette du Cube Horadrim"), "{err}");
        assert!(w.inventory()[0].is_some(), "aucune mutation");
    }

    /// Sortie ARME (L13 axe+dague → hache de lancer) : kind Weapon, dégâts
    /// de la table portés par le tooltip du produit, posée à la 1re place
    /// libre (slot 0).
    #[test]
    fn cube_transmute_slots_weapon_l13() {
        let mut w = world();
        put(&mut w, 3, "hax");
        put(&mut w, 4, "dgr");
        let out = w.cube_transmute_slots(&[3, 4], 9).expect("L13 doit matcher");
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["line"], 13);
        assert_eq!(v["item"]["code"], "tax");
        assert_eq!(v["item"]["name"], "Hache de lancer");
        assert_eq!(v["item"]["kind"], "Weapon");
        assert_eq!(v["item"]["tooltip"]["lines"][0], "Dégâts : 4-7", "tax mindam/maxdam (table)");
        assert_eq!(w.inventory()[0].as_ref().unwrap().kind, ItemKind::Weapon);
    }

    /// Inventaire plein (produit sans place) ⇒ Err fr, consommés INTACTS.
    #[test]
    fn cube_transmute_slots_full_inventory_rolls_back() {
        let mut w = world();
        put(&mut w, 39, "gcv");
        put(&mut w, 38, "gcv");
        put(&mut w, 37, "gcv");
        // Remplit les 37 autres slots.
        for s in 0..37 {
            put(&mut w, s, "rvl");
        }
        assert_eq!(w.inventory().iter().filter(|s| s.is_some()).count(), INV_SLOTS);
        let err = w.cube_transmute_slots(&[37, 38, 39], 1).unwrap_err();
        assert!(err.contains("Inventaire plein"), "{err}");
        assert!(w.inventory()[37].is_some() && w.inventory()[39].is_some(), "aucune mutation");
    }
}
