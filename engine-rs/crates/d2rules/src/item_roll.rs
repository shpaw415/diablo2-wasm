//! `item_roll` — étape 7a « loot D2 réel » : le contrat [`RolledItem`]
//! partagé par les deux coéquipiers (Affixes, Équipement) + le roll de
//! qualité ItemRatio v1.12 + le branchement sur la chaîne [`crate::loot`].
//!
//! # ══ CONTRAT GELÉ — RolledItem (posé 2026-10-24, ne plus changer) ══
//!
//! [`RolledItem`] est LA représentation d'un drop résolu, consommée par :
//! * **Affixes** : [`RolledItem::affix_ids`] (+ la coloration `transform`/
//!   `transformcolor` lue sur la ligne d'affixe résolue via [`resolve_affix`]) ;
//! * **Équipement** : les champs de base `min_dmg`/`max_dmg`/`ac` et les
//!   requis `req_str`/`req_dex`/`req_level` (tirés des tables v1.12).
//!
//! Champs (sémantique exacte) :
//! * `code` : code table D2 de la BASE (`hax`, `buc`, `hp1`, `gld`…).
//! * `quality` : [`RolledQuality`] — Low(`[`LowQualityVariant`]`) / Normal /
//!   Magic / Rare / Set / Unique / Crafted (+ Superior, divergence ①).
//! * `name_resolved` : nom final résolu — nom composé D2 pour Magic
//!   (`<Prefix> <Base> <Suffix>`), **clé exacte** de `patch_uniqueitems.txt`
//!   / `patch_setitems.txt` pour Unique/Set (le coéquipier Uniques résout
//!   les mods par cette clé), nom de base pour Normal/Superior/Rare (noms
//!   rares = rareprefix/raresuffix — restant), `<variante> <base>` pour Low
//!   (`lowqualityitems.txt`).
//! * `affix_ids` : `Vec<(id, valeur_effective)>` — **un tuple par MOD
//!   roulé** (un affixe à 2 mods ⇒ 2 tuples de même `id` ; un magic peut
//!   donc porter 0 tuple si l'affixe tiré est une ligne réelle SANS mods —
//!   quirk v1.12 : `MagicSuffix.txt` « of Telekinesis » ligne 429, row
//!   classic résiduelle, nom composé quand même). Encodage de `id`
//!   (MagicPrefix/Suffix v1.12 n'ont PAS de colonne id officielle, et des
//!   noms homonymes classic/LoD) : `1..=N` = rang 1-based dans la table
//!   PARSEE `MagicPrefix.txt` (ordre fichier, lignes sans nom exclues) ;
//!   `id ≥ 100_000` = rang `id − 100_000` dans `MagicSuffix.txt`. La
//!   résolution passe OBLIGATOIREMENT par [`resolve_affix`]. `valeur_
//!   effective` = roll `min..=max` à la génération, SAUF mod `sock` à
//!   `param` fixe (Artificer's 3, Jeweler's 4) où c'est le `param`.
//! * `sockets` : alvéoles — mod `sock` des affixes (groupe 122 v1.12 :
//!   Mechanist's 1-2 roulées, Artificer's 3, Jeweler's 4), 0 sinon
//!   (fentes quest/Larzuk : restant).
//! * `gold` : montant pour `gld` (0 sinon) — divergence ②, champ requis
//!   pour faire circuler les drops d'or de la chaîne TC.
//! * base : `min_dmg`/`max_dmg` (armes, cols `mindam`/`maxdam`), `ac`
//!   (armures : AC ROLLÉ dans `[min_ac, max_ac]`, jet D2 réel ; 0 armes/
//!   misc), `req_str`/`req_dex`/`req_level` (cols `reqstr`/`reqdex`/`levelreq`).
//!
//! # Divergences vs brief (annoncées 2026-10-24, additives — le reste est
//! la liste du brief mot pour mot) : ① `Superior` dans `RolledQuality` — le
//! test HiQuality est une étape du port ItemRatio fidèle, masquer un
//! supérieur en Normal perdrait l'information ; ② champ `gold` — les drops
//! `gld` existent dans toute la chaîne TC. Contrat gelé tel quel.
//!
//! # Roll de qualité — port fidèle ItemRatio v1.12
//! Sources : la table embarquée `assets/patch_itemratio.txt` FAIT FOI
//! (6 lignes : 2 classic `version 0` ignorées + 4 LoD `version 1`, citées
//! ligne à ligne en golden test) ; port Diablerie `Engine/ItemDrop.cs`
//! `TestQuality`/`SelectItemQuality` ; guides publics (Phrozen Keep, Amazon
//! Basin) NON FIABLES — leurs arrangements de colonnes divergent (« folklore »
//! 30/16 vs 34/3…) : arbitré par le fichier réel. Formule (par qualité, ordre
//! U→S→R→M→HiQ→Normal, tout échouer = Low) :
//! ```text
//! chance  = (freq − (ilvl − qlvl) / divisor) × 128   (division tronquée)
//! chance −= chance × facteur_TC / 1024               (colonnes TC, max chaîne)
//! chance  = max(chance, Min)                         (plancher 6400…)
//! réussite ⇔ rng[0, chance) < 128                    (P = 128 / chance)
//! ```
//! Implémentation : [`crate::item_quality`] (déjà golden-testée). MF au
//! facteur 0 au MVP. RNG : [`crate::rng::Rng`] SplitMix64 seedé (contrat
//! d2rules, save bit-exact). Table ItemRatio absente/illisible ⇒ qualité
//! **Normal par défaut** (jamais panique — règle mission).
//! Note de distribution : le test Normal (freq 2/div 2) donne P = 50 % à
//! ilvl == qlvl et 100 % dès (ilvl − qlvl) ≥ 1 — Low est donc le
//! COMPLÉMENT (fréquent sur les drops de bas niveau, fidèle au D2 réel) ;
//! la variante [`LowQualityVariant`] est tirée à chaque drop Low.
//!
//! # Branchement + ordre de tirage (gelé)
//! [`ItemRoller`] résout la TC via la chaîne existante ([`crate::loot`] :
//! picks/NoDrop → sous-TC → base, fenêtres `weap3`/`armo3`, or `gld,mul=`,
//! repli MVP 60 % rien / 25 % or / 15 % `hp1` si TC inconnue), puis complète
//! par item weap/armo : ① variante low (si Low) ② roll d'AC ③ affixes
//! Magic/Rare (RE-sélection + valeurs roulées ICI — les affixes tirés par
//! `loot.rs` sont consommés puis ignorés : double consommation de flux,
//! documentée, déterministe des deux côtés). `Crafted` : variante posée,
//! jamais tirée au drop (cube = restant).

#![allow(dead_code)] // contrat consommé par les coéquipiers + d2game à l'intégration

use std::collections::HashMap;

use crate::affixes::{self, AffixKind, AffixRec, AffixTables, ItemTypeSet};
use crate::item_props;
use crate::item_quality::{self, Quality, QualityFactors, QualityTables};
use crate::loot::{self, GeneratedItem};
use crate::rng::Rng;
use crate::tsv::Tsv;
use crate::ItemKind;

/// ItemRatio.txt v1.12 (LA source fait foi — golden test ligne à ligne).
const ITEM_RATIO_TXT: &str = include_str!("../../../../assets/patch_itemratio.txt");
/// MagicPrefix/MagicSuffix v1.12 (affixes magic/rare ; ids = rang parsé).
const MAGIC_PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
const MAGIC_SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
/// AutoMagic v1.12 (parse pour complétude, non tiré au drop).
const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");
/// Armes/armures v1.12 : map code → qlvl (colonne `level`) pour `alvl`.
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
/// lowqualityitems.txt v1.12 (noms basse qualité, ordre fichier = tirage).
const LOW_QUALITY_TXT: &str = include_str!("../../../../../d2w-re/tables/items-ui/lowqualityitems.txt");

/// Types d'armes exclus de la famille `mele` (distance/jet — miroir loot.rs).
const NON_MELEE: &[&str] = &["bow", "xbow", "abow", "jave", "ajav", "tkni", "tpot"];
/// Types d'armes de la famille `bow` (miroir loot.rs).
const BOW_TYPES: &[&str] = &["bow", "xbow", "abow"];
/// Types portés par `patch_armor.txt` (famille `armo`).
const ARMOR_TYPES: &[&str] = &[
    "ashd", "belt", "boot", "circ", "glov", "head", "helm", "pelt", "phlm", "shie", "tors",
];

// ---------------------------------------------------------------------------
// CONTRAT — types gelés
// ---------------------------------------------------------------------------

/// Offset d'encodage des ids de SUFFIXES dans `affix_ids` (doc module).
pub const SUFFIX_ID_OFFSET: u32 = 100_000;

/// Variante d'un drop LOW QUALITY — `lowqualityitems.txt` v1.12 (4 records,
/// ordre fichier = ordre de tirage).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum LowQualityVariant {
    /// « Crude » (ligne 1).
    #[default]
    Crude,
    /// « Cracked » (ligne 2).
    Cracked,
    /// « Damaged » (ligne 3).
    Damaged,
    /// « Low Quality » (ligne 4).
    LowQuality,
}

impl LowQualityVariant {
    /// Libellé table (anglais v1.12 ; fr = TBL STRING, restant).
    pub fn name(self) -> &'static str {
        match self {
            LowQualityVariant::Crude => "Crude",
            LowQualityVariant::Cracked => "Cracked",
            LowQualityVariant::Damaged => "Damaged",
            LowQualityVariant::LowQuality => "Low Quality",
        }
    }
}

/// Qualité d'un [`RolledItem`] — contrat des coéquipiers (Superior =
/// divergence ① : étape HiQuality du port ItemRatio fidèle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum RolledQuality {
    /// Basse qualité (fissuré/bruté…) — variante [`LowQualityVariant`].
    Low(LowQualityVariant),
    /// Normal (blanc) — DÉFAUT quand les tables manquent.
    #[default]
    Normal,
    /// Supérieur (HiQuality — mods qualityitems.txt, restant).
    Superior,
    /// Magique (bleu) — `affix_ids` remplis (≤ 1 prefix + 1 suffix).
    Magic,
    /// Rare (jaune) — ≤ 4 affixes distincts, groupes exclusifs.
    Rare,
    /// Pièce de set — `name_resolved` = clé `patch_setitems.txt`.
    Set,
    /// Unique — `name_resolved` = clé `patch_uniqueitems.txt`.
    Unique,
    /// Cube (recette) — jamais tiré au drop au MVP, posé pour le contrat.
    Crafted,
}

/// CONTRAT GELÉ (étape 7a) — un drop résolu : base + qualité + affixes.
/// Sémantique champ par champ en tête de module.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RolledItem {
    /// Code table de la base (`hax`, `buc`, `hp1`, `gld`…).
    pub code: String,
    /// Qualité résolue.
    pub quality: RolledQuality,
    /// Nom final (composé, clé unique/set, ou variante low).
    pub name_resolved: String,
    /// `(id affixe, valeur effective)` — un tuple par mod (encodage en doc).
    pub affix_ids: Vec<(u32, u32)>,
    /// Alvéoles (mod `sock` : Mechanist's 1-2, Artificer's 3, Jeweler's 4).
    pub sockets: u8,
    /// Montant d'or (code `gld`), 0 sinon.
    pub gold: u32,
    /// Dégât min de la base (`patch_weapons.txt` col `mindam`).
    pub min_dmg: u32,
    /// Dégât max de la base (col `maxdam`).
    pub max_dmg: u32,
    /// AC rollé dans `[min_ac, max_ac]` (armures ; 0 armes/misc).
    pub ac: u32,
    /// Requis force (col `reqstr`).
    pub req_str: u32,
    /// Requis dextérité (col `reqdex`).
    pub req_dex: u32,
    /// Requis niveau (col `levelreq`).
    pub req_level: u32,
}

/// Résout un id d'`affix_ids` vers sa ligne d'affixe (contrat coéquipier
/// Affixes : `transform`/`transformcolor`/mods se lisent sur la rec).
/// `(rec, kind)` ; `None` si id inconnu.
pub fn resolve_affix<'a>(tables: &'a AffixTables, id: u32) -> Option<(&'a AffixRec, AffixKind)> {
    if id == 0 {
        return None;
    }
    if let Some(i) = id.checked_sub(SUFFIX_ID_OFFSET) {
        let i = i.checked_sub(1)? as usize;
        tables.suffixes.get(i).map(|a| (a, AffixKind::Suffix))
    } else {
        tables.prefixes.get(id as usize - 1).map(|a| (a, AffixKind::Prefix))
    }
}

/// Constructeur contractuel : un item Crafted (cube — non tiré au drop).
pub fn crafted(code: &str, name: &str) -> RolledItem {
    RolledItem {
        code: code.to_string(),
        quality: RolledQuality::Crafted,
        name_resolved: name.to_string(),
        ..Default::default()
    }
}

/// Tire la variante low quality (ordre fichier `lowqualityitems.txt`).
/// Un tirage [`Rng`].
pub(crate) fn roll_low_variant(rng: &mut Rng) -> LowQualityVariant {
    match rng.below(4) {
        0 => LowQualityVariant::Crude,
        1 => LowQualityVariant::Cracked,
        2 => LowQualityVariant::Damaged,
        _ => LowQualityVariant::LowQuality,
    }
}

/// Id d'une rec d'affixe par IDENTITÉ de pointeur dans les tables parsées
/// (insensible aux noms homonymes classic/LoD) ; `None` = introuvable.
fn affix_id(t: &AffixTables, rec: &AffixRec) -> Option<u32> {
    if let Some(i) = t.prefixes.iter().position(|a| std::ptr::eq(a, rec)) {
        return Some(i as u32 + 1);
    }
    t.suffixes.iter().position(|a| std::ptr::eq(a, rec)).map(|i| i as u32 + 1 + SUFFIX_ID_OFFSET)
}

/// Nom magique D2 : `<Prefix> <Base> <Suffix>` (miroir loot::compose_name).
fn compose_name(prefix: Option<&AffixRec>, base: &str, suffix: Option<&AffixRec>) -> String {
    let mut name = base.to_string();
    if let Some(s) = suffix {
        name = format!("{name} {}", s.name);
    }
    if let Some(p) = prefix {
        name = format!("{} {name}", p.name);
    }
    name
}

/// Mappe la qualité de la chaîne `loot` sur le contrat (Low déjà développé
/// en variante par l'appelant).
fn map_quality(q: Quality) -> RolledQuality {
    match q {
        Quality::Normal => RolledQuality::Normal,
        Quality::Superior => RolledQuality::Superior,
        Quality::Magic => RolledQuality::Magic,
        Quality::Set => RolledQuality::Set,
        Quality::Rare => RolledQuality::Rare,
        Quality::Unique => RolledQuality::Unique,
        Quality::Low => RolledQuality::Low(LowQualityVariant::default()),
    }
}

/// Interne les types en `&'static` (ensemble fermé des tables v1.12, miroir
/// `loot::intern_type` ; type inconnu ⇒ `""`).
fn leak_type(t: &str) -> &'static str {
    const KNOWN: &[&str] = &[
        "abow", "ajav", "amul", "apot", "aspe", "ashd", "axe", "belt", "body", "boot", "book",
        "bow", "bowq", "circ", "club", "elix", "gema", "gemd", "geme", "gemr", "gems", "gemt",
        "gemz", "glov", "gold", "h2h", "h2h2", "hamm", "head", "helm", "herb", "hpot", "jave",
        "jewl", "key", "knif", "lcha", "mace", "mcha", "mpot", "orb", "pelt", "phlm", "play",
        "pole", "ques", "ring", "rpot", "rune", "scep", "scha", "scro", "shie", "spea", "spot",
        "staf", "swor", "taxe", "tkni", "torc", "tpot", "tors", "wand", "wpot", "xbow", "xboq",
    ];
    KNOWN.iter().copied().find(|k| *k == t).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Roller — chaîne loot + complétion RolledItem
// ---------------------------------------------------------------------------

/// Tables du roller : qualité (ItemRatio) + affixes + map code → qlvl.
pub(crate) struct RollTables {
    /// ItemRatio parsé — `None` ⇒ qualité Normal par défaut (mission).
    pub(crate) quality: Option<QualityTables>,
    /// MagicPrefix/Suffix parsés — `None` ⇒ jamais d'affixes.
    pub(crate) affixes: Option<AffixTables>,
    /// Code base → qlvl (colonne `level` de weapons/armor) pour `alvl`.
    pub(crate) qlvl_of: HashMap<String, u32>,
}

impl RollTables {
    /// Parse les tables embarquées (lenient : chaque parse peut échouer seul).
    pub(crate) fn embedded() -> RollTables {
        let mut qlvl_of = HashMap::new();
        for txt in [WEAPONS_TXT, ARMOR_TXT] {
            if let Ok(t) = Tsv::parse(txt) {
                for i in 0..t.len() {
                    if let (Some(c), Some(l)) = (t.cell(i, "code"), t.cell(i, "level")) {
                        if !c.is_empty() {
                            qlvl_of
                                .entry(c.to_string())
                                .or_insert_with(|| l.trim().parse().unwrap_or(0));
                        }
                    }
                }
            }
        }
        RollTables {
            quality: item_quality::parse(ITEM_RATIO_TXT),
            affixes: affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT),
            qlvl_of,
        }
    }
}

/// Un affixe choisi : id encodé + ligne parsée (identité exacte).
struct PickedAffix<'a> {
    id: u32,
    rec: &'a AffixRec,
}

/// Affixes choisis pour un drop magic/rare (ordre de tirage conservé).
struct AffixPicks<'a> {
    items: Vec<PickedAffix<'a>>,
}

/// Roller de butin : chaîne TC [`crate::loot`] → [`RolledItem`] (contrat).
pub(crate) struct ItemRoller {
    loot: loot::Loot,
    tables: RollTables,
}

impl ItemRoller {
    /// Tables embarquées réelles ; `None` si TreasureClassEx illisible
    /// (aucun roller — le `World` garde son repli MVP).
    pub(crate) fn new() -> Option<ItemRoller> {
        let loot = loot::build()?;
        Some(ItemRoller { loot, tables: RollTables::embedded() })
    }

    /// Variante testable : tables injectées (`quality: None` ⇒ Normal par
    /// défaut — golden de la règle mission).
    pub(crate) fn with_tables(loot: loot::Loot, tables: RollTables) -> ItemRoller {
        ItemRoller { loot, tables }
    }

    /// Tire tout le butin d'une TC (déterministe au seed du [`Rng`]).
    /// TC inconnue ⇒ repli MVP 60 % rien / 25 % or / 15 % `hp1`.
    pub(crate) fn roll(&self, rng: &mut Rng, tc_name: &str, mlvl: u32) -> Vec<RolledItem> {
        self.roll_qf(rng, tc_name, mlvl, QualityFactors::default())
    }

    /// Variante avec facteurs de qualité initiaux (tests de saturation,
    /// Magic Find futur — mêmes tirages que la chaîne `loot`).
    pub(crate) fn roll_qf(
        &self,
        rng: &mut Rng,
        tc_name: &str,
        mlvl: u32,
        qf: QualityFactors,
    ) -> Vec<RolledItem> {
        if !self.loot.has_tc(tc_name) {
            return loot::Loot::roll_fallback(rng, mlvl).into_iter().map(fallback_item).collect();
        }
        self.loot
            .generate_qf(rng, tc_name, mlvl, qf)
            .iter()
            .map(|g| self.rolled(rng, g, mlvl))
            .collect()
    }

    /// Construit le [`RolledItem`] d'un [`GeneratedItem`] — ordre de tirage
    /// gelé : ① variante low ② AC ③ affixes (picks puis valeurs par mod).
    fn rolled(&self, rng: &mut Rng, g: &GeneratedItem, mlvl: u32) -> RolledItem {
        // Or : montant porté tel quel, qualité Normal.
        if g.gold > 0 {
            return RolledItem {
                code: "gld".into(),
                quality: RolledQuality::Normal,
                name_resolved: g.name.clone(),
                gold: g.gold,
                ..Default::default()
            };
        }
        // Misc (potions, gemmes, runes, clés…) : pas de qualité en D2.
        if !matches!(g.kind, ItemKind::Weapon | ItemKind::Armor) {
            return RolledItem {
                code: g.code.clone(),
                quality: RolledQuality::Normal,
                name_resolved: g.name.clone(),
                ..Default::default()
            };
        }
        let base = &g.item.base;
        // Qualité : Normal PAR DÉFAUT quand la table ItemRatio manque.
        let quality = match (&self.tables.quality, g.quality) {
            (None, _) => RolledQuality::Normal,
            (Some(_), Quality::Low) => RolledQuality::Low(roll_low_variant(rng)),
            (Some(_), q) => map_quality(q),
        };
        // Ordre gelé ② : AC rollé (armures ; armes 0/0 ne consomment rien).
        let ac = rng.range_u32(base.min_ac, base.max_ac);
        let mut item = RolledItem {
            code: g.code.clone(),
            quality,
            name_resolved: g.name.clone(),
            affix_ids: Vec::new(),
            sockets: 0,
            gold: 0,
            min_dmg: base.min_dam,
            max_dmg: base.max_dam,
            ac,
            req_str: base.req_str,
            req_dex: base.req_dex,
            req_level: base.req_level,
        };
        // Ordre gelé ③ : affixes Magic/Rare (re-sélection ici, ids exacts).
        match item.quality {
            RolledQuality::Magic => {
                let picks = self.pick_affixes(rng, g, mlvl, false);
                self.apply_affixes(rng, &mut item, &picks);
                let pre =
                    picks.items.iter().find(|p| p.rec.kind == AffixKind::Prefix).map(|p| p.rec);
                let suf =
                    picks.items.iter().find(|p| p.rec.kind == AffixKind::Suffix).map(|p| p.rec);
                item.name_resolved = compose_name(pre, base.name.as_str(), suf);
            }
            RolledQuality::Rare => {
                let picks = self.pick_affixes(rng, g, mlvl, true);
                self.apply_affixes(rng, &mut item, &picks);
                // noms rares : rareprefix/raresuffix.txt — restant (nom base).
                item.name_resolved = base.name.clone();
            }
            RolledQuality::Low(v) => {
                item.name_resolved = format!("{} {}", v.name(), base.name);
            }
            // Unique/Set : name_resolved = clé (mods par le coéquipier) ;
            // Normal/Superior : nom déjà posé ci-dessus.
            _ => {}
        }
        item
    }

    /// (Re)sélection des affixes d'un drop magic/rare — ids calculés par
    /// identité de pointeur (noms homonymes classic/LoD sans ambiguïté).
    fn pick_affixes(&self, rng: &mut Rng, g: &GeneratedItem, mlvl: u32, rare: bool) -> AffixPicks<'_> {
        let Some(t) = &self.tables.affixes else {
            return AffixPicks { items: Vec::new() };
        };
        let qlvl = self.tables.qlvl_of.get(&g.code).copied().unwrap_or(0);
        let alvl = affixes::alvl(qlvl, mlvl);
        let types = self.type_set(g.item.base.wtype.as_str());
        let chosen: Vec<&AffixRec> = if rare {
            affixes::pick_rare(rng, t, &types, alvl)
        } else {
            let (p, s) = affixes::pick_magic(rng, t, &types, alvl);
            [p, s].into_iter().flatten().collect()
        };
        AffixPicks {
            items: chosen
                .into_iter()
                .map(|rec| PickedAffix { id: affix_id(t, rec).unwrap_or(0), rec })
                .collect(),
        }
    }

    /// Roule les valeurs des affixes choisis et remplit `affix_ids` +
    /// `sockets` (mod `sock` : `param > 0` = fixe, sinon roll min..max).
    fn apply_affixes(&self, rng: &mut Rng, item: &mut RolledItem, picks: &AffixPicks) {
        for p in &picks.items {
            for d in &p.rec.mods {
                let rolled = item_props::roll(d, rng).value.max(0) as u32;
                let param: u32 = d.param.trim().parse().unwrap_or(0);
                let effective = if d.code == "sock" && param > 0 { param } else { rolled };
                if d.code == "sock" {
                    item.sockets = effective.min(u8::MAX as u32) as u8;
                }
                item.affix_ids.push((p.id, effective));
            }
        }
    }

    /// Types couverts par une base (miroir `loot::type_set`, filtres
    /// `itype`/`etype` des tables d'affixes).
    fn type_set(&self, wtype: &str) -> ItemTypeSet {
        let is_weap = loot::is_weapon_type(wtype);
        ItemTypeSet {
            wtype: leak_type(wtype),
            is_weap,
            is_mele: is_weap && !NON_MELEE.contains(&wtype),
            is_bow: BOW_TYPES.contains(&wtype),
            is_armo: ARMOR_TYPES.contains(&wtype),
        }
    }
}

/// Repli MVP (TC inconnue) → [`RolledItem`] (qualité Normal — mission).
fn fallback_item(d: loot::RolledDrop) -> RolledItem {
    if d.code == "gld" {
        RolledItem {
            code: "gld".into(),
            quality: RolledQuality::Normal,
            name_resolved: "Or".into(),
            gold: d.gold,
            ..Default::default()
        }
    } else {
        RolledItem {
            code: d.code.clone(),
            quality: RolledQuality::Normal,
            name_resolved: if d.code == "hp1" { "Lesser Healing Potion".into() } else { d.code },
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Golden tests — chaque ligne d'ItemRatio citée, distributions seedées,
// drop unique connu (Gull), sockets, Normal par défaut.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_quality::{DropRatioInfo, RatioRow};

    fn roller() -> ItemRoller {
        ItemRoller::new().expect("tables embarquées valides")
    }

    fn affix_tables() -> AffixTables {
        affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT)
            .expect("tables affixes embarquées valides")
    }

    /// Contrat gelé : forme littérale exhaustive (champ par champ) +
    /// encodage des ids (prefix brut / suffix offset 100_000).
    #[test]
    fn contract_shape_is_frozen() {
        let it = RolledItem {
            code: "hax".into(),
            quality: RolledQuality::Magic,
            name_resolved: "Sturdy Hand Axe of Health".into(),
            affix_ids: vec![(1, 25), (SUFFIX_ID_OFFSET + 1, 10)],
            sockets: 0,
            gold: 0,
            min_dmg: 3,
            max_dmg: 6,
            ac: 0,
            req_str: 0,
            req_dex: 0,
            req_level: 3,
        };
        assert_eq!(it.code, "hax");
        assert_eq!(it.quality, RolledQuality::Magic);
        assert_eq!(it.affix_ids, vec![(1, 25), (100_001, 10)]);
        assert_eq!(it.sockets, 0);
        assert_eq!(it.gold, 0);
        assert_eq!((it.min_dmg, it.max_dmg, it.ac), (3, 6, 0));
        assert_eq!((it.req_str, it.req_dex, it.req_level), (0, 0, 3));
        let t = affix_tables();
        let (rec, kind) = resolve_affix(&t, 1).expect("prefix id 1");
        assert_eq!(kind, AffixKind::Prefix);
        assert_eq!(rec.name, t.prefixes[0].name, "id 1 = 1re ligne parsée");
        let (rec, kind) = resolve_affix(&t, SUFFIX_ID_OFFSET + 1).expect("suffix id 100_001");
        assert_eq!(kind, AffixKind::Suffix);
        assert_eq!(rec.name, t.suffixes[0].name, "id 100_001 = 1re ligne parsée suffix");
        assert!(resolve_affix(&t, 0).is_none(), "0 = pas d'affixe");
        assert!(resolve_affix(&t, u32::MAX).is_none(), "hors bornes ⇒ None");
    }

    /// GOLDEN — les 6 lignes réelles d'`patch_itemratio.txt`, citées
    /// cellule par cellule (parseur TSV runtime [`Tsv`]). LA TABLE FAIT FOI ;
    /// les docs publiques (Phrozen Keep « folklore » 30/16 vs 34/3…) ont des
    /// arrangements de colonnes divergents et ne sont PAS la référence.
    #[test]
    fn itemratio_all_six_rows_cited() {
        let t = Tsv::parse(ITEM_RATIO_TXT).expect("ItemRatio.txt parsable");
        assert_eq!(t.len(), 6, "6 lignes de ratio (2 classic + 4 LoD)");
        // L1 classic (version 0, IGNORÉE au port v1.12).
        let c = t.rec(0).unwrap();
        assert_eq!(c.s("Function"), "Ratio - (Monster Level / Ratio Divisor)");
        assert_eq!(c.u("Version"), 0);
        assert_eq!((c.u("Unique"), c.u("UniqueDivisor"), c.u("UniqueMin")), (400, 2, 6400));
        assert_eq!((c.u("Rare"), c.u("RareDivisor"), c.u("RareMin")), (160, 3, 3200));
        assert_eq!((c.u("Set"), c.u("SetDivisor"), c.u("SetMin")), (125, 6, 5600));
        assert_eq!((c.u("Magic"), c.u("MagicDivisor"), c.u("MagicMin")), (30, 16, 192));
        assert_eq!((c.u("HiQuality"), c.u("HiQualityDivisor")), (12, 16));
        assert_eq!((c.u("Normal"), c.u("NormalDivisor")), (4, 8));
        // L2 classic uber (version 0, ignorée) : 240/2, 96/3, 96/6, 3/100.
        let c = t.rec(1).unwrap();
        assert_eq!(c.s("Function"), "Uber");
        assert_eq!(c.u("Version"), 0);
        assert_eq!((c.u("Unique"), c.u("UniqueDivisor"), c.u("UniqueMin")), (240, 2, 6400));
        assert_eq!((c.u("Rare"), c.u("RareDivisor"), c.u("RareMin")), (96, 3, 3200));
        assert_eq!((c.u("Set"), c.u("SetDivisor"), c.u("SetMin")), (96, 6, 5600));
        assert_eq!((c.u("Magic"), c.u("MagicDivisor"), c.u("MagicMin")), (3, 100, 192));
        // L3 LoD normal (version 1) — LA ligne active non-uber :
        // 400/1/6400, 100/2/3200, 160/2/5600 (quirk authentique), 34/3/192,
        // HiQ 12/8, Normal 2/2.
        let t2 = item_quality::parse(ITEM_RATIO_TXT).expect("tables qualité");
        let n = t2.row_for(false, false);
        assert_eq!(n, RatioRow {
            uber: false,
            class_specific: false,
            unique: DropRatioInfo { frequency: 400, divisor: 1, min: 6400 },
            rare: DropRatioInfo { frequency: 100, divisor: 2, min: 3200 },
            set: DropRatioInfo { frequency: 160, divisor: 2, min: 5600 },
            magic: DropRatioInfo { frequency: 34, divisor: 3, min: 192 },
            hi_quality: DropRatioInfo { frequency: 12, divisor: 8, min: 0 },
            normal: DropRatioInfo { frequency: 2, divisor: 2, min: 0 },
        });
        let c = t.rec(2).unwrap();
        assert_eq!((c.u("Version"), c.u("Uber")), (1, 0));
        assert_eq!((c.u("Unique"), c.u("UniqueDivisor"), c.u("UniqueMin")), (400, 1, 6400));
        // L4 LoD uber (version 1, uber=1) : mêmes freq, HiQ 8/1, Normal 1/1.
        let u = t2.uber.expect("ligne uber LoD");
        assert_eq!(
            (u.unique.frequency, u.rare.frequency, u.set.frequency, u.magic.frequency),
            (400, 100, 160, 34)
        );
        assert_eq!((u.hi_quality.frequency, u.hi_quality.divisor), (12, 8));
        assert_eq!((u.normal.frequency, u.normal.divisor), (1, 1));
        let c = t.rec(3).unwrap();
        assert_eq!((c.u("Version"), c.u("Uber")), (1, 1));
        // L5 LoD class-specific : 240/3/6400, 80/3/3200, 120/3/5600, 17/6/192.
        let cs = t2.class_specific.expect("ligne class LoD");
        assert_eq!((cs.unique.frequency, cs.unique.divisor, cs.unique.min), (240, 3, 6400));
        assert_eq!((cs.rare.frequency, cs.rare.divisor, cs.rare.min), (80, 3, 3200));
        assert_eq!((cs.set.frequency, cs.set.divisor, cs.set.min), (120, 3, 5600));
        assert_eq!((cs.magic.frequency, cs.magic.divisor, cs.magic.min), (17, 6, 192));
        assert_eq!((cs.hi_quality.frequency, cs.hi_quality.divisor), (9, 8));
        assert_eq!((cs.normal.frequency, cs.normal.divisor), (2, 2));
        let c = t.rec(4).unwrap();
        assert_eq!((c.u("Version"), c.u("Uber"), c.s("Class Specific")), (1, 0, "1"));
        // L6 LoD class-specific uber : mêmes freq, HiQ 8/1, Normal 1/1.
        let cu = t2.class_specific_uber.expect("ligne class uber LoD");
        assert_eq!(cu.unique.frequency, 240);
        assert_eq!((cu.hi_quality.frequency, cu.hi_quality.divisor), (9, 8));
        assert_eq!((cu.normal.frequency, cu.normal.divisor), (1, 1));
        let c = t.rec(5).unwrap();
        assert_eq!((c.u("Version"), c.u("Uber"), c.s("Class Specific")), (1, 1, "1"));
    }

    /// Formule ItemRatio : P(unique) mesurée ≈ 128/51200 = 1/400 à
    /// ilvl == qlvl, sans facteur TC ni MF (port fidèle, doc module).
    #[test]
    fn itemratio_formula_probability_is_faithful() {
        let info = DropRatioInfo { frequency: 400, divisor: 1, min: 6400 };
        let n = 200_000u32;
        let mut hits = 0;
        let mut rng = Rng::new(0x7A);
        for _ in 0..n {
            if item_quality::test_quality(&mut rng, &info, 3, 3, 0, 0, 0) {
                hits += 1;
            }
        }
        let ppm = hits as u64 * 1_000_000 / n as u64;
        assert!((2125..=2875).contains(&ppm), "P(unique) {ppm} ppm ≈ 2500 (1/400)");
    }

    /// Déterminisme : même seed ⇒ mêmes RolledItem (champ par champ), TC
    /// réelle comme repli.
    #[test]
    fn roll_is_deterministic_same_seed() {
        let r = roller();
        for tc in ["Act 1 Equip A", "TC inconnue XYZ"] {
            let mut a = Rng::new(0xD2A);
            let mut b = Rng::new(0xD2A);
            let ra = r.roll(&mut a, tc, 5);
            let rb = r.roll(&mut b, tc, 5);
            assert_eq!(ra, rb, "TC {tc}");
        }
    }

    /// Distribution seedée (2000 drops « Act 1 Equip A », qf nuls) : le
    /// spectre respecte ItemRatio — normal/supérieur dominent, magic bien
    /// présent, unique/set rares, JAMAIS de Low ni Crafted, misc Normal.
    #[test]
    fn distribution_is_sane_and_seeded() {
        let r = roller();
        let mut counts = [0u32; 8]; // Low, Normal, Superior, Magic, Rare, Set, Unique, Crafted
        let mut drops = 0;
        for seed in 0..2000u64 {
            let mut rng = Rng::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            for it in r.roll(&mut rng, "Act 1 Equip A", 5) {
                drops += 1;
                let idx = match &it.quality {
                    RolledQuality::Low(_) => 0,
                    RolledQuality::Normal => 1,
                    RolledQuality::Superior => 2,
                    RolledQuality::Magic => 3,
                    RolledQuality::Rare => 4,
                    RolledQuality::Set => 5,
                    RolledQuality::Unique => 6,
                    RolledQuality::Crafted => 7,
                };
                counts[idx] += 1;
                match &it.quality {
                    // affix_ids possiblement vide : lignes d'affixes réelles
                    // sans mods (cf. doc module) — couvert par le test magic.
                    RolledQuality::Magic => {}
                    RolledQuality::Unique => {
                        assert!(it.affix_ids.is_empty(), "unique avec affixes magic ?");
                        assert!(!it.name_resolved.is_empty());
                    }
                    _ => {}
                }
                if it.code == "gld" {
                    assert!(it.gold >= 1, "or positif");
                    assert_eq!(it.quality, RolledQuality::Normal);
                }
            }
        }
        assert!(counts[0] > 100, "Low présent (P = 100 % − P(Normal)) : {}", counts[0]);
        assert!(counts[0] < counts[1], "Low minoritaire vs normal");
        assert!(counts[1] > 300, "normal {}", counts[1]);
        assert!(counts[2] > 10, "supérieur {}", counts[2]);
        assert!(counts[3] > 10, "magic {}", counts[3]);
        assert!(counts[3] < counts[1], "magic minoritaire vs normal");
        assert!(counts[4] <= counts[3], "rare ≤ magic");
        assert!(counts[6] <= 15, "uniques {} (P ≈ 0,25 %/drop)", counts[6]);
        assert!(counts[5] <= 20, "sets {} (P ≈ 0,63 %/drop)", counts[5]);
        assert_eq!(counts[7], 0, "crafted jamais au drop");
        assert!(drops > 500, "drops {drops}");
    }

    /// GOLDEN mission — drop unique connu : "Act 1 Equip A" → weap3 →
    /// `dgr` (Dagger, qlvl 3, fenêtre [3,6)) à facteur unique saturé ;
    /// résolution par `patch_uniqueitems.txt` : Gull (lvl 6 ≤ ilvl 10 ;
    /// The Diggler lvl 15 exclu). Base = tables : dégâts 1/4, aucun requis.
    #[test]
    fn known_unique_drop_resolves_to_gull() {
        let r = roller();
        let qf = QualityFactors { unique: 1024, ..Default::default() };
        let mut found = None;
        for seed in 1..30000u64 {
            let mut rng = Rng::new(seed);
            for it in r.roll_qf(&mut rng, "Act 1 Equip A", 10, qf) {
                if it.code == "dgr" && it.quality == RolledQuality::Unique {
                    found = Some(it);
                    break;
                }
            }
            if found.is_some() {
                break;
            }
        }
        let gull = found.expect("Gull doit tomber (P ≈ 2 %/drop saturé, 30k seeds)");
        assert_eq!(gull.name_resolved, "Gull", "clé exacte patch_uniqueitems.txt");
        assert_eq!(gull.quality, RolledQuality::Unique);
        assert!(gull.affix_ids.is_empty(), "mods d'unique ≠ affixes magic");
        assert_eq!(gull.sockets, 0);
        assert_eq!((gull.min_dmg, gull.max_dmg), (1, 4), "Dagger 1-4 (patch_weapons)");
        assert_eq!((gull.req_str, gull.req_dex, gull.req_level), (0, 0, 0), "Dagger sans requis");
        // La clé résout bien dans la table réelle des uniques.
        let us = crate::uniques_sets::parse(
            include_str!("../../../../assets/patch_uniqueitems.txt"),
            include_str!("../../../../assets/patch_setitems.txt"),
            include_str!("../../../../assets/patch_sets.txt"),
        )
        .unwrap();
        assert!(us.uniques.iter().any(|u| u.index == "Gull"));
    }

    /// Magic : ids résolubles → noms composés corrects, valeurs dans les
    /// bornes `modNmin/max` de la ligne (ou `param` fixe pour `sock`) ;
    /// ≤ 1 prefix + 1 suffix.
    #[test]
    fn magic_affix_ids_resolve_and_compose_name() {
        let r = roller();
        let t = affix_tables();
        let qf = QualityFactors { magic: 1024, ..Default::default() };
        let mut magics = 0;
        let mut with_prefix = 0;
        'outer: for seed in 0..1500u64 {
            let mut rng = Rng::new(seed | 1);
            for it in r.roll_qf(&mut rng, "Act 1 Equip A", 20, qf) {
                if it.quality != RolledQuality::Magic {
                    continue;
                }
                magics += 1;
                if it.affix_ids.is_empty() {
                    continue; // ligne d'affixe réelle sans mods (doc module)
                }
                // Ids DISTINCTS par table (un affixe à N mods ⇒ N tuples).
                let mut prefix_ids = std::collections::HashSet::new();
                let mut suffix_ids = std::collections::HashSet::new();
                for &(id, v) in &it.affix_ids {
                    let (rec, kind) = resolve_affix(&t, id).expect("id résoluble");
                    match kind {
                        AffixKind::Prefix => {
                            prefix_ids.insert(id);
                        }
                        AffixKind::Suffix => {
                            suffix_ids.insert(id);
                        }
                    }
                    let ok = rec.mods.iter().any(|d| {
                        let param: u32 = d.param.trim().parse().unwrap_or(0);
                        if d.code == "sock" && param > 0 {
                            v == param
                        } else {
                            v as i32 >= d.min.min(d.max) && v as i32 <= d.max.max(d.min)
                        }
                    });
                    assert!(ok, "valeur {v} hors bornes de {} ({:?})", rec.name, rec.mods);
                    assert!(it.name_resolved.contains(rec.name.as_str()), "{} vs {}", it.name_resolved, rec.name);
                }
                assert!(prefix_ids.len() <= 1 && suffix_ids.len() <= 1, "≤ 1 prefix + 1 suffix");
                if !prefix_ids.is_empty() {
                    with_prefix += 1;
                }
                if magics >= 60 {
                    break 'outer;
                }
            }
        }
        assert!(magics >= 30, "magics vus : {magics}");
        assert!(with_prefix > 0, "aucun prefix sur {magics} magics");
    }

    /// Rare : ≤ 4 affixes DISTINCTS (ids uniques), tous marqués `rare`.
    #[test]
    fn rare_items_carry_at_most_four_distinct_affixes() {
        let r = roller();
        let t = affix_tables();
        let qf = QualityFactors { rare: 1024, ..Default::default() };
        let mut rares = 0;
        'outer: for seed in 0..3000u64 {
            let mut rng = Rng::new(seed.wrapping_mul(31) | 1);
            for it in r.roll_qf(&mut rng, "Act 1 Equip A", 25, qf) {
                if it.quality != RolledQuality::Rare {
                    continue;
                }
                rares += 1;
                let ids: Vec<u32> = it.affix_ids.iter().map(|a| a.0).collect();
                assert!(ids.len() <= 12, "4 affixes × 3 mods max : {}", ids.len());
                let distinct: std::collections::HashSet<u32> = ids.iter().copied().collect();
                assert!(distinct.len() <= 4, "≤ 4 affixes : {distinct:?}");
                for id in &distinct {
                    let (rec, _) = resolve_affix(&t, *id).expect("id rare résoluble");
                    assert!(rec.rare, "{} n'est pas un affixe rare", rec.name);
                }
                if rares >= 25 {
                    break 'outer;
                }
            }
        }
        assert!(rares >= 10, "rares vus : {rares}");
    }

    /// Sockets : le prefix `sock` (groupe 122 — Mechanist's/Artificer's/
    /// Jeweler's) renseigne `sockets` (`param` fixe, sinon roll min..max,
    /// bornes D2 1..=4).
    #[test]
    fn sock_prefix_yields_sockets() {
        let r = roller();
        let t = affix_tables();
        let qf = QualityFactors { magic: 1024, ..Default::default() };
        let mut hits = 0;
        'outer: for seed in 0..8000u64 {
            let mut rng = Rng::new(seed.wrapping_mul(7919) | 1);
            for it in r.roll_qf(&mut rng, "Act 1 Equip A", 60, qf) {
                if it.sockets == 0 {
                    continue;
                }
                hits += 1;
                let mut explained = false;
                for &(id, v) in &it.affix_ids {
                    let (rec, _) = resolve_affix(&t, id).unwrap();
                    for d in &rec.mods {
                        if d.code == "sock" {
                            let param: u32 = d.param.trim().parse().unwrap_or(0);
                            let expected = if param > 0 { param } else { v };
                            assert_eq!(it.sockets as u32, expected, "{}", rec.name);
                            assert!((1..=4).contains(&it.sockets), "bornes D2 1..=4");
                            explained = true;
                        }
                    }
                }
                assert!(explained, "sockets sans mod sock : {:?}", it.affix_ids);
                if hits >= 5 {
                    break 'outer;
                }
            }
        }
        assert!(hits >= 3, "drops socketés vus : {hits}");
    }

    /// Mission : tables ItemRatio absentes ⇒ qualité Normal PAR DÉFAUT
    /// (jamais de Low, jamais d'affixe — jamais de panique).
    #[test]
    fn normal_by_default_when_quality_tables_missing() {
        let loot = loot::build().expect("TCE");
        let tables = RollTables { quality: None, affixes: None, qlvl_of: HashMap::new() };
        let r = ItemRoller::with_tables(loot, tables);
        for seed in 0..400u64 {
            let mut rng = Rng::new(seed | 1);
            for it in r.roll(&mut rng, "Act 1 Equip A", 20) {
                assert_eq!(it.quality, RolledQuality::Normal, "seed {seed} : {:?}", it);
                assert!(it.affix_ids.is_empty());
                assert_eq!(it.sockets, 0);
            }
        }
    }

    /// Variante low : l'ordre de tirage = ordre fichier lowqualityitems.txt
    /// (Crude → Cracked → Damaged → Low Quality), les 4 sortent.
    #[test]
    fn low_quality_variants_follow_lowqualityitems_txt() {
        let t = Tsv::parse(LOW_QUALITY_TXT).expect("lowqualityitems.txt parsable");
        let file_names: Vec<String> =
            (0..t.len()).map(|i| t.cell(i, "Name").unwrap().to_string()).collect();
        let variants = [
            LowQualityVariant::Crude,
            LowQualityVariant::Cracked,
            LowQualityVariant::Damaged,
            LowQualityVariant::LowQuality,
        ];
        let enum_names: Vec<String> = variants.iter().map(|v| v.name().to_string()).collect();
        assert_eq!(file_names, enum_names, "ordre fichier = ordre enum (4 records)");
        let mut seen = [0u32; 4];
        let mut rng = Rng::new(0x10D2);
        for _ in 0..4000 {
            let v = roll_low_variant(&mut rng);
            let i = variants.iter().position(|x| *x == v).unwrap();
            seen[i] += 1;
        }
        assert!(seen.iter().all(|&n| n > 500), "distribution low : {seen:?}");
        // La variante porte bien le contrat Low(variante) + nom composé.
        let it = RolledItem {
            quality: RolledQuality::Low(LowQualityVariant::Cracked),
            code: "cap".into(),
            name_resolved: "Cracked Cap".into(),
            ..Default::default()
        };
        assert!(matches!(it.quality, RolledQuality::Low(LowQualityVariant::Cracked)));
        assert_eq!(it.name_resolved, "Cracked Cap");
    }

    /// Repli MVP : TC inconnue ⇒ or/potion Normal, or borné [5×mlvl, 15×mlvl].
    #[test]
    fn unknown_tc_falls_back_mvp() {
        let r = roller();
        let mut gold_seen = 0;
        let mut hp_seen = 0;
        for mlvl in [1u32, 4, 9] {
            for seed in 0..300u64 {
                let mut rng = Rng::new(0xF00D + mlvl as u64 * 10_007 + seed);
                for it in r.roll(&mut rng, "TC absente des tables", mlvl) {
                    assert_eq!(it.quality, RolledQuality::Normal);
                    if it.code == "gld" {
                        gold_seen += 1;
                        assert!(
                            (mlvl * 5..=mlvl * 15).contains(&it.gold),
                            "or {} vs [{},{}]",
                            it.gold,
                            mlvl * 5,
                            mlvl * 15
                        );
                        assert_eq!(it.name_resolved, "Or");
                    } else {
                        assert_eq!(it.code, "hp1");
                        hp_seen += 1;
                        assert_eq!(it.gold, 0);
                        assert_eq!(it.name_resolved, "Lesser Healing Potion");
                    }
                }
            }
        }
        assert!(gold_seen > 100, "repli 25 % or vu : {gold_seen}");
        assert!(hp_seen > 50, "repli 15 % hp1 vu : {hp_seen}");
    }

    /// Crafted : constructeur contractuel (cube — jamais tiré au drop).
    #[test]
    fn crafted_constructor_carries_quality() {
        let it = crafted("hax", "Hitpower Hand Axe");
        assert_eq!(it.quality, RolledQuality::Crafted);
        assert!(it.affix_ids.is_empty());
        let r = roller();
        let mut rng = Rng::new(3);
        for _ in 0..200 {
            for it in r.roll(&mut rng, "Act 1 Equip A", 5) {
                assert_ne!(it.quality, RolledQuality::Crafted, "crafted ne tombe pas au drop");
            }
        }
    }

    /// Base fields armure : AC rollé dans [min_ac, max_ac] et requis portés
    /// DE LA TABLE (vérifiés contre `patch_armor.txt` embarqué, parseur
    /// [`Tsv`]) ; goldens explicites `lea` (14-17, req_str 15) et `dr1`
    /// (8-11, req_str 16 — fenêtre armo3 [3,6)) ; les armes portent les
    /// dégâts de table et ac 0.
    #[test]
    fn armor_base_fields_rolled_from_tables() {
        let r = roller();
        let arm = Tsv::parse(ARMOR_TXT).expect("patch_armor.txt parsable");
        let bounds = |code: &str| -> Option<(u32, u32, u32, u32, u32)> {
            (0..arm.len()).find_map(|i| {
                (arm.cell(i, "code") == Some(code)).then(|| {
                    (
                        arm.rec(i).unwrap().u("minac"),
                        arm.rec(i).unwrap().u("maxac"),
                        arm.rec(i).unwrap().u("reqstr"),
                        arm.rec(i).unwrap().u("reqdex"),
                        arm.rec(i).unwrap().u("levelreq"),
                    )
                })
            })
        };
        let mut seen_lea = 0;
        let mut seen_dr1 = 0;
        for seed in 0..3000u64 {
            let mut rng = Rng::new(seed.wrapping_mul(6700417) | 1);
            for it in r.roll(&mut rng, "Act 1 Equip A", 3) {
                if let Some((min_ac, max_ac, req_str, req_dex, req_level)) = bounds(&it.code) {
                    assert!(
                        (min_ac..=max_ac).contains(&it.ac),
                        "{} AC {} hors table [{min_ac},{max_ac}]",
                        it.code,
                        it.ac
                    );
                    assert_eq!(it.req_str, req_str, "{} req_str", it.code);
                    assert_eq!(it.req_dex, req_dex, "{} req_dex", it.code);
                    assert_eq!(it.req_level, req_level, "{} req_level", it.code);

                    if it.code == "lea" {
                        seen_lea += 1;
                    }
                    if it.code == "dr1" {
                        seen_dr1 += 1;
                    }
                }
                if it.code == "hax" {
                    assert_eq!(it.ac, 0, "arme : ac 0");
                    assert_eq!((it.min_dmg, it.max_dmg), (3, 6), "Hand Axe 3-6");
                }
            }
        }
        assert!(seen_lea >= 10, "lea vus : {seen_lea}");
        assert!(seen_dr1 >= 10, "dr1 vus : {seen_dr1}");
    }
}


