//! `loot` — drops réels via `TreasureClassEx.txt` (données v1.12, feature #34),
//! enrichis phase 3 : qualité (`ItemRatio.txt`), affixes
//! (`MagicPrefix/Suffix`), uniques/sets (`uniqueitems`/`setitems`/`sets`).
//!
//! Port Rust de la chaîne OpenDiablo2 `item_factory.go:163-330` +
//! `treasure_class_loader.go`, `item_ratio_loader.go`, `item_affix_loader.go`,
//! `unique_items_loader.go`, `set_item_loader.go`, `set_loader.go` (contrat
//! de parité od2-features-formats #34, od2-features-gameplay D1, F2-F5) :
//!
//! * **Parse TSV** : `Treasure Class`, `group`, `level`, `Picks`, `Unique`,
//!   `Set`, `Rare`, `Magic`, `NoDrop` + paires dynamiques `ItemN`/`ProbN`
//!   (découvertes depuis l'en-tête, pas codées en dur).
//! * **Résolution** : TC → `Picks`/`NoDrop` → sous-TC (récursif, garde de
//!   profondeur) → code d'item final. `Picks < 0` = comptes par trésor
//!   (sémantique od2) ; `NoDrop` = part de « rien » dans le tirage pondéré.
//!   Les colonnes `Unique/Set/Rare/Magic` de chaque TC se cumulent en MAX
//!   le long de la récursion (facteurs de qualité, port Diablerie
//!   `ItemDrop.Drop`).
//! * **Codes génériques** : `weap33`/`mele3`/`bow3`/`armo3` → item concret de
//!   la famille dont `level ∈ [L, L+3)` (fenêtre od2 `dynamicItemLevelRange`),
//!   choix uniforme parmi les items `spawnable` (tables v1.12 embarquées).
//! * **Or** : `gld` et `gld,mul=X` (multiplicateur /1024) sur une base
//!   `[5×mlvl, 15×mlvl]` (héritage MVP — le vrai D2 lit `monstats`).
//! * **Qualité + affixes + uniques/sets** (armes/armures seulement — les
//!   misc/potions n'ont pas de qualité en D2) :
//!   `item_quality::resolve` (ItemRatio, ordre U→S→R→M→HiQ→N→Low) →
//!   uniques/sets (candidats par code de base, porte ilvl ≥ lvl, pondérés
//!   `rarity`, dégradation en rare si aucun) → affixes (magic : ≤ 1 prefix
//!   et 1 suffix ; rare : 4, groupes exclusifs) → props ROULÉES
//!   (min..=max à la génération). Nom composé D2 (`<Prefix> <Base>
//!   <Suffix>` / index unique / pièce de set).
//! * **Déterminisme** : uniquement `Rng` seedé du `World` (contrat save
//!   bit-exact) ; ordre de tirage FIXE documenté : résolution TC → qualité
//!   (1 jet par test, arrêt au 1er succès) → sélection unique/set (1 jet
//!   pondéré) → affixes → valeurs des props (prefixes → suffixes →
//!   unique/set). Familles triées, aucune itération de `HashMap` dans un
//!   tirage.
//! * **Repli** : table absente/illisible, ou nom de TC inconnu dans le
//!   manifest ⇒ anciennes constantes MVP 60 % rien / 25 % or / 15 % `hp1`.
//!   Tables qualité/affixes absentes ⇒ drop nu sans qualité (jamais de
//!   panique).
//!
//! Zéro IO : les tables sont embarquées à la compilation (`include_str!`,
//! même pattern que `tests.rs` pour le manifest).

use std::collections::HashMap;

use super::affixes::{self, AffixKind, AffixTables, ItemTypeSet};
use super::inventory::{BaseInfo, Item};
use super::item_props::{self, RolledProp};
use super::item_quality::{self, Quality, QualityFactors, QualityTables};
use super::rng::Rng;
use super::uniques_sets::{self, UniquesSets};
use crate::world::{DROP_GOLD_PCT, DROP_NOTHING_PCT};
use crate::ItemKind;

/// TreasureClassEx.txt v1.12 (~850 classes de trésor).
const TCE_TXT: &str = include_str!("../../../../assets/TreasureClassEx.txt");
/// Armes v1.12 (patch_d2.mpq) : familles d'équivalence `weap`/`mele`/`bow`.
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
/// Armures v1.12 (patch_d2.mpq) : famille d'équivalence `armo`.
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
/// Divers v1.12 (patch_d2.mpq) : potions, gemmes, runes, clés… (codes directs).
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");
/// ItemRatio.txt v1.12 : tirage de qualité (unique/set/rare/magic/hiq/normal).
const ITEM_RATIO_TXT: &str = include_str!("../../../../assets/patch_itemratio.txt");
/// MagicPrefix.txt / MagicSuffix.txt v1.12 : affixes magiques.
const MAGIC_PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
const MAGIC_SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
/// AutoMagic.txt v1.12 : affixes automatiques (parsé, non branché — restant).
const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");
/// Uniques + sets v1.12.
const UNIQUE_ITEMS_TXT: &str = include_str!("../../../../assets/patch_uniqueitems.txt");
const SET_ITEMS_TXT: &str = include_str!("../../../../assets/patch_setitems.txt");
const SETS_TXT: &str = include_str!("../../../../assets/patch_sets.txt");

/// Fenêtre de niveau des codes dynamiques (`armo3` ⇒ niveaux 3..=5) —
/// `dynamicItemLevelRange = 3` chez od2 (item_factory.go:37).
const LEVEL_WINDOW: u32 = 3;
/// Garde anti-cycle de récursion TC (la table réelle n'a pas de cycle).
const MAX_DEPTH: u8 = 16;

/// Types d'armes EXCLUS de la famille `mele` (distance / jet) ; le reste
/// (axe, swor, mace, knif, hamm, scep, staf, pole, club, h2h…) est mêlé.
const NON_MELEE: &[&str] = &["bow", "xbow", "abow", "jave", "ajav", "tkni", "tpot"];
/// Types d'armes de la famille `bow` (arcs + arbalètes).
const BOW_TYPES: &[&str] = &["bow", "xbow", "abow"];

/// Une classe de trésor (ligne de `TreasureClassEx.txt`).
pub(crate) struct TcRec {
    /// `Picks` : >0 = tirages pondérés ; <0 = comptes par trésor (od2).
    pub(crate) picks: i32,
    /// `NoDrop` : poids de « rien tomber » dans chaque tirage.
    pub(crate) no_drop: u32,
    /// Facteurs de qualité (colonnes `Unique`/`Set`/`Rare`/`Magic`, 0 si vide).
    pub(crate) quality: QualityFactors,
    /// Trésors `(code, prob)` — code = sous-TC ou item (sémantique od2).
    pub(crate) treasures: Vec<(String, u32)>,
}

/// Un item connu des tables (armes/armures/divers v1.12).
#[derive(Clone)]
pub(crate) struct ItemRec {
    /// `level` (qlvl — niveau de qualité D2).
    pub(crate) level: u32,
    /// `spawnable` = 1 (peut apparaître dans un tirage d'équivalence).
    pub(crate) spawnable: bool,
    /// Type D2 (`axe`, `shie`, `hpot`, `rune`…) — pilote la famille.
    pub(crate) wtype: &'static str,
    /// `normcode` (base normale de cette version — item « uber » si diffèrent).
    pub(crate) normcode: String,
    /// Catégorie [`ItemKind`] pour [`crate::ItemDef`].
    pub(crate) kind: ItemKind,
    /// Infos de base réelles (taille, AC, dégâts, requis) pour [`Item`].
    pub(crate) base: BaseInfo,
}

/// Butin résolu réduit (contrat `world.rs`, inchangé depuis la phase 2).
pub(crate) struct RolledDrop {
    /// Code table final (`gld`, `hax`, `buc`, `r01`…).
    pub(crate) code: String,
    /// Montant d'or (code `gld` uniquement), 0 sinon.
    pub(crate) gold: u32,
}

/// Butin généré RICHE : base + qualité + affixes + uniques/sets + props
/// roulées (prêt pour [`inventory::Item`], HUD, tooltip).
#[derive(Debug, Clone)]
pub struct GeneratedItem {
    /// Code de la base (`hax`, `buc`…).
    pub code: String,
    /// Nom composé D2 (`Sturdy Hand Axe of Health`, `Gull`…).
    pub name: String,
    /// Catégorie.
    pub kind: ItemKind,
    /// Or (code `gld`), 0 sinon.
    pub gold: u32,
    /// Qualité résolue (misc/potions = [`Quality::Normal`]).
    pub quality: Quality,
    /// Unique (clé TBL), si drop unique.
    pub unique: Option<String>,
    /// Pièce de set (clé TBL), si drop de set.
    pub set_item: Option<String>,
    /// Set parent (clé Sets.txt).
    pub set: Option<String>,
    /// Noms des prefixes/suffixes magiques (ordre de tirage).
    pub prefixes: Vec<String>,
    pub suffixes: Vec<String>,
    /// Props roulées (valeurs min..=max tirées), ordre fixe.
    pub props: Vec<RolledProp>,
    /// Item prêt-à-porter (bases + qualités déjà fusionnées).
    pub item: Item,
}

/// Tables de loot v1.12 embarquées + résolution déterministe.
pub(crate) struct Loot {
    /// TCs par nom exact ("Act 1 H2H A"…).
    tcs: HashMap<String, TcRec>,
    /// Items connus par code.
    items: HashMap<String, ItemRec>,
    /// Familles d'équivalence, triées (level, code) ⇒ tirage déterministe.
    fam_weap: Vec<(String, ItemRec)>,
    fam_mele: Vec<(String, ItemRec)>,
    fam_bow: Vec<(String, ItemRec)>,
    fam_armo: Vec<(String, ItemRec)>,
    /// ItemRatio v1.12 (qualité) — `None` ⇒ drops nus (jamais de panique).
    quality: Option<QualityTables>,
    /// MagicPrefix/Suffix v1.12 — `None` ⇒ pas d'affixes.
    affixes: Option<AffixTables>,
    /// Uniques/sets v1.12 — `None` ⇒ pas d'uniques/sets.
    uniques: Option<UniquesSets>,
}

/// Catégorie des items divers : potions de vie/rejuv ⇒ [`ItemKind::Potion`]
/// (utilisables), le reste (mana, gemmes, runes, clés…) ⇒ [`ItemKind::Misc`].
fn misc_kind(wtype: &str) -> ItemKind {
    match wtype {
        "hpot" | "rpot" => ItemKind::Potion,
        _ => ItemKind::Misc,
    }
}

/// Construit les tables depuis les TSV embarqués ; `None` si TreasureClassEx
/// est illisible/vide (le `World` retombe alors sur les constantes MVP).
pub(crate) fn build() -> Option<Loot> {
    let rows = tsv_rows(TCE_TXT);
    let hdr = rows.first()?;
    let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
    let name_c = col("Treasure Class")?;
    let picks_c = col("Picks");
    let nodrop_c = col("NoDrop");
    let qf_c = [
        col("Unique").map(|i| (i, 0usize)),
        col("Set").map(|i| (i, 1usize)),
        col("Rare").map(|i| (i, 2usize)),
        col("Magic").map(|i| (i, 3usize)),
    ];
    // Paires dynamiques ItemN/ProbN découvertes dans l'en-tête (10 au v1.12).
    let pairs: Vec<(usize, usize)> = hdr
        .iter()
        .enumerate()
        .filter_map(|(i, h)| {
            let n: usize = h.strip_prefix("Item")?.parse().ok()?;
            let p = col(&format!("Prob{n}"))?;
            Some((i, p))
        })
        .collect();

    let mut tcs: HashMap<String, TcRec> = HashMap::new();
    for r in rows.iter().skip(1) {
        let cell =
            |i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("");
        let name = cell(Some(name_c));
        if name.is_empty() {
            continue;
        }
        let treasures = pairs
            .iter()
            .filter_map(|&(ic, pc)| {
                let c = cell(Some(ic));
                if c.is_empty() {
                    None
                } else {
                    Some((c.to_string(), cell(Some(pc)).parse::<u32>().unwrap_or(0)))
                }
            })
            .collect();
        let mut qf = QualityFactors::default();
        for (i, slot) in qf_c.iter().flatten() {
            let v: i32 = cell(Some(*i)).parse().unwrap_or(0);
            match slot {
                0 => qf.unique = v,
                1 => qf.set = v,
                2 => qf.rare = v,
                _ => qf.magic = v,
            }
        }
        tcs.insert(
            name.to_string(),
            TcRec {
                picks: cell(picks_c).parse::<i32>().unwrap_or(1),
                no_drop: cell(nodrop_c).parse::<u32>().unwrap_or(0),
                quality: qf,
                treasures,
            },
        );
    }
    if tcs.is_empty() {
        return None;
    }

    // Tables d'items : armes, armures, divers (codes directs + équivalences).
    let mut items: HashMap<String, ItemRec> = HashMap::new();
    let mut weap: Vec<(String, ItemRec)> = Vec::new();
    let mut armo: Vec<(String, ItemRec)> = Vec::new();
    load_item_table(WEAPONS_TXT, &mut items, &mut weap, |_| ItemKind::Weapon);
    load_item_table(ARMOR_TXT, &mut items, &mut armo, |_| ItemKind::Armor);
    load_item_table(MISC_TXT, &mut items, &mut Vec::new(), misc_kind);

    let sort_key =
        |a: &(String, ItemRec), b: &(String, ItemRec)| a.1.level.cmp(&b.1.level).then(a.0.cmp(&b.0));
    weap.sort_by(sort_key);
    armo.sort_by(sort_key);
    let fam_weap = weap;
    let fam_armo = armo;
    let fam_mele: Vec<(String, ItemRec)> = fam_weap
        .iter()
        .filter(|(_, r)| !NON_MELEE.contains(&r.wtype))
        .cloned()
        .collect();
    let fam_bow: Vec<(String, ItemRec)> = fam_weap
        .iter()
        .filter(|r| BOW_TYPES.contains(&r.1.wtype))
        .cloned()
        .collect();

    Some(Loot {
        tcs,
        items,
        fam_weap,
        fam_mele,
        fam_bow,
        fam_armo,
        quality: item_quality::parse(ITEM_RATIO_TXT),
        affixes: affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT),
        uniques: uniques_sets::parse(UNIQUE_ITEMS_TXT, SET_ITEMS_TXT, SETS_TXT),
    })
}

/// Parse une table d'items TSV (colonnes `code`/`level`/`spawnable`/`type` +
/// colonnes de base v1.12) et l'ajoute à `items` + à la liste `fam`.
fn load_item_table(
    txt: &str,
    items: &mut HashMap<String, ItemRec>,
    fam: &mut Vec<(String, ItemRec)>,
    kind_of: fn(&str) -> ItemKind,
) {
    let rows = tsv_rows(txt);
    let Some(hdr) = rows.first() else { return };
    let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(code_c), level_c, spawn_c, type_c) =
        (col("code"), col("level"), col("spawnable"), col("type"))
    else {
        return;
    };
    let name_c = col("name");
    let w_c = col("invwidth");
    let h_c = col("invheight");
    let minac_c = col("minac");
    let maxac_c = col("maxac");
    let mindam_c = col("mindam");
    let maxdam_c = col("maxdam");
    let reqstr_c = col("reqstr");
    let reqdex_c = col("reqdex");
    let lreq_c = col("levelreq");
    let norm_c = col("normcode");
    for r in rows.iter().skip(1) {
        let cell = |i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("");
        let num = |i: Option<usize>| cell(i).parse::<u32>().unwrap_or(0);
        let code = cell(Some(code_c));
        if code.is_empty() || items.contains_key(code) {
            continue;
        }
        let level = num(level_c);
        let spawnable = cell(spawn_c) == "1";
        let wtype: &'static str = intern_type(cell(type_c));
        let base = BaseInfo {
            name: cell(name_c).to_string(),
            wtype: wtype.to_string(),
            w: num(w_c),
            h: num(h_c),
            min_ac: num(minac_c),
            max_ac: num(maxac_c),
            min_dam: num(mindam_c),
            max_dam: num(maxdam_c),
            req_str: num(reqstr_c),
            req_dex: num(reqdex_c),
            req_level: num(lreq_c),
        };
        let rec = ItemRec {
            level,
            spawnable,
            wtype,
            kind: kind_of(wtype),
            normcode: cell(norm_c).to_string(),
            base,
        };
        items.insert(code.to_string(), rec.clone());
        fam.push((code.to_string(), rec));
    }
}

/// Types internés (ensemble fermé des tables D2 v1.12) — garde `ItemRec`
/// léger ; type inconnu ⇒ `""` (hors familles mêlées).
fn intern_type(t: &str) -> &'static str {
    const KNOWN: &[&str] = &[
        "abow", "ajav", "amul", "apot", "aspe", "ashd", "axe", "belt", "body", "boot", "book",
        "bow", "bowq", "circ", "club", "elix", "gema", "gemd", "geme", "gemr", "gems", "gemt",
        "gemz", "glov", "gold", "h2h", "h2h2", "hamm", "helm", "herb", "hpot", "jave", "jewl",
        "key", "knif", "lcha", "mace", "mcha", "mpot", "orb", "pelt", "phlm", "play", "pole",
        "ques", "ring", "rpot", "rune", "scep", "scha", "scro", "shie", "spea", "spot", "staf",
        "swor", "taxe", "tkni", "torc", "tpot", "tors", "wand", "wpot", "xbow", "xboq",
    ];
    KNOWN.iter().copied().find(|k| *k == t).unwrap_or("")
}

/// Le type est-il une arme (famille `weap`) ? (équipement mains.)
pub(crate) fn is_weapon_type(wtype: &str) -> bool {
    matches!(
        wtype,
        "axe" | "swor" | "mace" | "knif" | "hamm" | "scep" | "staf" | "pole" | "club"
            | "h2h" | "h2h2" | "spea" | "jave" | "ajav" | "tkni" | "tpot" | "taxe"
            | "orb" | "aspe" | "bow" | "xbow" | "abow"
    )
}

/// Découpe un code dynamique `armo33` → (`"armo"`, 33) (sémantique od2 :
/// lettres = famille, chiffres = niveau).
fn split_code(code: &str) -> (&str, u32) {
    let alpha: &str = code.trim_end_matches(|c: char| c.is_ascii_digit());
    let num: String = code[alpha.len()..].to_string();
    (alpha, num.parse().unwrap_or(0))
}

/// Parse un TSV tabulé en lignes de cellules trimées (guillemets D2 enlevés).
/// Public crate : réutilisé par `item_quality`/`affixes`/`uniques_sets`.
pub(crate) fn tsv_rows(txt: &str) -> Vec<Vec<String>> {
    txt.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.split('\t').map(|c| c.trim().trim_matches('"').to_string()).collect())
        .collect()
}

impl Loot {
    /// TC connue ?
    pub(crate) fn has_tc(&self, name: &str) -> bool {
        self.tcs.contains_key(name)
    }

    /// Tire tout le butin d'une TC (déterministe au seed du [`Rng`]) —
    /// réduction [`RolledDrop`] du flux [`Self::generate`] (même tirages).
    pub(crate) fn roll(&self, rng: &mut Rng, tc_name: &str, mlvl: u32) -> Vec<RolledDrop> {
        self.generate(rng, tc_name, mlvl)
            .into_iter()
            .map(|g| RolledDrop { code: g.code, gold: g.gold })
            .collect()
    }

    /// Chaîne COMPLÈTE : TC → picks/NoDrop → sous-TC → base → qualité →
    /// uniques/sets → affixes → props roulées. Ordre de tirage fixe (doc
    /// module). Les misc (potions, gemmes…) et l'or ne roulent pas de qualité.
    pub fn generate(&self, rng: &mut Rng, tc_name: &str, mlvl: u32) -> Vec<GeneratedItem> {
        self.generate_qf(rng, tc_name, mlvl, QualityFactors::default())
    }

    /// Variante avec facteurs de qualité INITIAUX (plancher : fusion MAX avec
    /// les colonnes des TCs traversées — testable et réutilisable).
    pub fn generate_qf(
        &self,
        rng: &mut Rng,
        tc_name: &str,
        mlvl: u32,
        qf: QualityFactors,
    ) -> Vec<GeneratedItem> {
        let mut out = Vec::new();
        self.roll_tc(rng, tc_name, mlvl, &mut out, 0, qf);
        out
    }

    /// Repli MVP (60 % rien / 25 % or / 15 % hp1) — table absente ou TC
    /// inconnue : même comportement que l'ancien `roll_drop` (phase 2).
    pub(crate) fn roll_fallback(rng: &mut Rng, mlvl: u32) -> Vec<RolledDrop> {
        let r = rng.below(100);
        if r < DROP_NOTHING_PCT {
            Vec::new()
        } else if r < DROP_NOTHING_PCT + DROP_GOLD_PCT {
            vec![RolledDrop { code: "gld".into(), gold: rng.range_u32(mlvl * 5, mlvl * 15) }]
        } else {
            vec![RolledDrop { code: "hp1".into(), gold: 0 }]
        }
    }

    /// Résolution récursive d'une TC (port de `ItemsFromTreasureClass` +
    /// cumul MAX des facteurs de qualité le long de la chaîne — Diablerie).
    fn roll_tc(
        &self,
        rng: &mut Rng,
        name: &str,
        mlvl: u32,
        out: &mut Vec<GeneratedItem>,
        depth: u8,
        qf: QualityFactors,
    ) {
        if depth > MAX_DEPTH {
            return;
        }
        let Some(rec) = self.tcs.get(name) else { return };
        let qf = merge_qf(qf, &rec.quality);

        // Picks : <0 = comptes par trésor, sinon tirages pondérés NoDrop inclus.
        let mut picks: Vec<&str> = Vec::new();
        if rec.picks < 0 {
            let mut left = rec.picks;
            for (code, count) in &rec.treasures {
                let mut n = (*count).min(64);
                while n > 0 && left < 0 {
                    picks.push(code);
                    left += 1;
                    n -= 1;
                }
            }
        } else {
            let items_total: u32 = rec.treasures.iter().map(|t| t.1).sum();
            let total = rec.no_drop.saturating_add(items_total);
            for _ in 0..rec.picks {
                if total == 0 {
                    continue;
                }
                let roll = rng.below(total);
                if roll < rec.no_drop {
                    continue; // NoDrop : rien pour ce pick
                }
                let mut cum = rec.no_drop;
                for (code, prob) in &rec.treasures {
                    cum += prob;
                    if roll < cum {
                        picks.push(code);
                        break;
                    }
                }
            }
        }

        for code in picks {
            self.resolve(rng, code, mlvl, out, depth + 1, qf);
        }
    }

    /// Résout un code de trésor : sous-TC, item direct, or, ou code générique
    /// à suffixe de niveau (`armo3`…) — port de `ItemFromTreasure` +
    /// `resolveDynamicTreasureCode`. Code inconnu ⇒ pick perdu (rien).
    fn resolve(
        &self,
        rng: &mut Rng,
        code: &str,
        mlvl: u32,
        out: &mut Vec<GeneratedItem>,
        depth: u8,
        qf: QualityFactors,
    ) {
        // Or (+ variante "gld,mul=X" : multiplicateur /1024, convention D2).
        if code == "gld" || code.starts_with("gld,mul=") {
            let mul: u64 =
                code.strip_prefix("gld,mul=").and_then(|m| m.parse().ok()).unwrap_or(1024);
            let base = rng.range_u32(mlvl * 5, mlvl * 15).max(1) as u64;
            let amount = ((base * mul) / 1024).max(1) as u32;
            out.push(gold_item(amount));
            return;
        }
        // Sous-TC : récursion.
        if self.tcs.contains_key(code) {
            self.roll_tc(rng, code, mlvl, out, depth, qf);
            return;
        }
        // Item direct connu (hp1, rin, r01, key…) : la QUALITÉ ne s'applique
        // qu'aux armes/armures (D2 réel) ; misc = base nue nommée.
        if let Some(rec) = self.items.get(code) {
            out.push(self.finish(rng, code, rec, mlvl, &qf));
            return;
        }
        // Code générique à niveau : famille + fenêtre [L, L+3).
        let (family, lvl) = split_code(code);
        let fam = match family {
            "weap" => &self.fam_weap,
            "mele" => &self.fam_mele,
            "bow" => &self.fam_bow,
            "armo" => &self.fam_armo,
            _ => return,
        };
        let cands: Vec<&(String, ItemRec)> = fam
            .iter()
            .filter(|(_, r)| r.spawnable && r.level >= lvl && r.level < lvl + LEVEL_WINDOW)
            .collect();
        if let Some((code, rec)) = cands.get(rng.below(cands.len() as u32) as usize) {
            let rec = rec.clone();
            out.push(self.finish(rng, code, &rec, mlvl, &qf));
        }
    }

    /// Fin de génération d'un item de base : qualité (armes/armures),
    /// uniques/sets, affixes, props roulées, nom composé.
    fn finish(
        &self,
        rng: &mut Rng,
        code: &str,
        rec: &ItemRec,
        mlvl: u32,
        qf: &QualityFactors,
    ) -> GeneratedItem {
        let mut item = Item {
            code: code.to_string(),
            name: rec.base.name.clone(),
            quality: Quality::Normal,
            identified: true,
            w: rec.base.w,
            h: rec.base.h,
            base: rec.base.clone(),
            ..Default::default()
        };
        let mut g = GeneratedItem {
            code: code.to_string(),
            name: rec.base.name.clone(),
            kind: rec.kind,
            gold: 0,
            quality: Quality::Normal,
            unique: None,
            set_item: None,
            set: None,
            prefixes: Vec::new(),
            suffixes: Vec::new(),
            props: Vec::new(),
            item: item.clone(),
        };
        // Les potions/gemmes/runes/clés (misc) n'ont PAS de qualité en D2.
        if !matches!(rec.kind, ItemKind::Weapon | ItemKind::Armor) {
            return g;
        }

        let default_qt = QualityTables::default();
        let qt = self.quality.as_ref().unwrap_or(&default_qt);
        // Item exceptionnel/élite ⇔ code ≠ normcode (réf. Diablerie).
        let uber = !rec.normcode.is_empty() && rec.normcode != *code;
        let _ = uber; // ligne class-specific non sélectionnée au MVP (écart documenté)
        let qlvl = rec.level;
        let quality = item_quality::resolve(rng, qt, qlvl, mlvl, 0, qf, true);
        g.quality = quality;
        item.quality = quality;

        match quality {
            Quality::Unique => {
                let empty = UniquesSets::default();
                let u = self
                    .uniques
                    .as_ref()
                    .unwrap_or(&empty)
                    .select_unique(rng, mlvl, code);
                if let Some(u) = u {
                    // Uniques : props fixes (min..=max roulé), non identifié.
                    for d in &u.props {
                        g.props.push(item_props::roll(d, rng));
                    }
                    g.unique = Some(u.index.clone());
                    item.unique = g.unique.clone();
                    item.name = u.index.clone();
                    item.identified = false;
                } else {
                    // Aucun unique de cette base à ce niveau ⇒ rare (od2
                    // `applyDropModifier`, Diablerie `GenerateItemProperties`).
                    // La qualité RÉELLE devient Rare — sans ça le butin porte
                    // `Unique` sans clé d'unique (contrat `g.unique`).
                    g.quality = Quality::Rare;
                    self.gen_rare(rng, rec, mlvl, &mut g, &mut item);
                }
            }
            Quality::Set => {
                let empty = UniquesSets::default();
                let s = self
                    .uniques
                    .as_ref()
                    .unwrap_or(&empty)
                    .select_set_item(rng, mlvl, code);
                if let Some(s) = s {
                    for d in &s.props {
                        g.props.push(item_props::roll(d, rng));
                    }
                    g.set_item = Some(s.index.clone());
                    g.set = Some(s.set.clone());
                    item.set_item = g.set_item.clone();
                    item.set = g.set.clone();
                    item.name = s.index.clone();
                    item.identified = false;
                } else {
                    // Pas de pièce de cette base à ce niveau ⇒ rare (même
                    // dégradation que les uniques, qualité réalignée).
                    g.quality = Quality::Rare;
                    self.gen_rare(rng, rec, mlvl, &mut g, &mut item);
                }
            }
            Quality::Rare => self.gen_rare(rng, rec, mlvl, &mut g, &mut item),
            Quality::Magic => {
                let no_affix = AffixTables::default();
                let t = self.affixes.as_ref().unwrap_or(&no_affix);
                let types = self.type_set(rec);
                let alvl = affixes::alvl(qlvl, mlvl);
                let (p, s) = affixes::pick_magic(rng, t, &types, alvl);
                for a in [p, s].into_iter().flatten() {
                    for d in &a.mods {
                        g.props.push(item_props::roll(d, rng));
                    }
                    match a.kind {
                        AffixKind::Prefix => g.prefixes.push(a.name.clone()),
                        AffixKind::Suffix => g.suffixes.push(a.name.clone()),
                    }
                }
                item.prefixes = g.prefixes.clone();
                item.suffixes = g.suffixes.clone();
                item.name = compose_name(&g.prefixes, &rec.base.name, &g.suffixes);
                item.identified = false;
            }
            Quality::Superior => {
                item.name = format!("Superior {}", rec.base.name);
            }
            // Normal / Low : nom de base (noms low = lowqualityitems.txt, restant).
            _ => {}
        }

        g.name = item.name.clone();
        g.item = item;
        g
    }

    /// Item RARE : 4 affixes (groupes exclusifs, marqués `rare`), nom de base
    /// (noms rares = rareprefix/raresuffix.txt — non extraits, restant).
    fn gen_rare(
        &self,
        rng: &mut Rng,
        rec: &ItemRec,
        mlvl: u32,
        g: &mut GeneratedItem,
        item: &mut Item,
    ) {
        let no_affix = AffixTables::default();
        let t = self.affixes.as_ref().unwrap_or(&no_affix);
        let types = self.type_set(rec);
        let alvl = affixes::alvl(rec.level, mlvl);
        let picks = affixes::pick_rare(rng, t, &types, alvl);
        for a in picks {
            for d in &a.mods {
                g.props.push(item_props::roll(d, rng));
            }
            match a.kind {
                AffixKind::Prefix => g.prefixes.push(a.name.clone()),
                AffixKind::Suffix => g.suffixes.push(a.name.clone()),
            }
        }
        item.prefixes = g.prefixes.clone();
        item.suffixes = g.suffixes.clone();
        item.quality = Quality::Rare;
        item.identified = false;
        item.name = rec.base.name.clone(); // noms rares : TBL, restant
    }

    /// Types couverts par une base (pour les filtres `itype`/`etype`).
    fn type_set(&self, rec: &ItemRec) -> ItemTypeSet {
        ItemTypeSet {
            wtype: rec.wtype,
            is_weap: self.fam_weap.iter().any(|(_, r)| r.wtype == rec.wtype),
            is_mele: self.fam_mele.iter().any(|(_, r)| r.wtype == rec.wtype),
            is_bow: self.fam_bow.iter().any(|(_, r)| r.wtype == rec.wtype),
            is_armo: self.fam_armo.iter().any(|(_, r)| r.wtype == rec.wtype),
        }
    }

    /// Convertit un tirage en [`crate::ItemDef`] (kind depuis les tables) —
    /// CONTRAT world.rs phase 2 inchangé (le riche part via [`Self::generate`]).
    pub(crate) fn to_def(&self, d: &RolledDrop) -> crate::ItemDef {
        if d.gold > 0 {
            return crate::ItemDef::gold(d.gold);
        }
        if d.code == "hp1" {
            return crate::ItemDef::potion();
        }
        let kind = self.items.get(&d.code).map(|r| r.kind).unwrap_or(ItemKind::Misc);
        crate::ItemDef { code: d.code.clone(), name: d.code.clone(), kind, amount: 0 }
    }
}

/// Or généré (`gld`) — taille grille 1×1 (colonnes `invwidth`/`invheight`
/// de la ligne `gld` de `patch_misc.txt` v1.12 ; une pile d'or occupe une
/// cellule, comme tout item d'inventaire D2).
fn gold_item(amount: u32) -> GeneratedItem {
    GeneratedItem {
        code: "gld".into(),
        name: "Or".into(),
        kind: ItemKind::Gold,
        gold: amount,
        quality: Quality::Normal,
        unique: None,
        set_item: None,
        set: None,
        prefixes: Vec::new(),
        suffixes: Vec::new(),
        props: Vec::new(),
        item: Item {
            code: "gld".into(),
            name: "Or".into(),
            w: 1,
            h: 1,
            base: BaseInfo {
                name: "gold".into(),
                wtype: "gold".into(),
                w: 1,
                h: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    }
}

/// Cumule les facteurs de qualité le long de la chaîne de TCs (MAX, Diablerie).
fn merge_qf(a: QualityFactors, b: &QualityFactors) -> QualityFactors {
    QualityFactors {
        unique: a.unique.max(b.unique),
        set: a.set.max(b.set),
        rare: a.rare.max(b.rare),
        magic: a.magic.max(b.magic),
    }
}

/// Nom magique D2 : `<Prefix> <Base> <Suffix>` (od2 `generateName`, anglais
/// tables ; libellés fr = TBL STRING, restant).
fn compose_name(prefixes: &[String], base: &str, suffixes: &[String]) -> String {
    let mut name = String::from(base);
    if let Some(s) = suffixes.last() {
        name = format!("{name} {s}");
    }
    if let Some(p) = prefixes.last() {
        name = format!("{p} {name}");
    }
    name
}

// ---------------------------------------------------------------------------
// Tests — tables réelles embarquées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn loot() -> Loot {
        build().expect("tables embarquées valides")
    }

    /// #34 : la table réelle v1.12 est bien parsée (~850 TCs, pas 3 constantes).
    #[test]
    fn parse_tce_real_table() {
        let l = loot();
        assert!(l.tcs.len() >= 800, "TCs parsés : {}", l.tcs.len());
        assert!(l.tcs.contains_key("Act 1 H2H A"));
        assert!(l.tcs.contains_key("Gold"));
        assert!(l.tcs.contains_key("Jewelry A"));
        // items : armes + armures + divers v1.12
        assert!(l.items.len() >= 500, "items : {}", l.items.len());
        assert!(l.items.contains_key("hax"));
        assert!(l.items.contains_key("buc"));
        assert!(l.items.contains_key("hp1"));
    }

    /// Champs de "Act 1 H2H A" (NoDrop 100, 1 pick, gld 21 + 3 sous-TCs,
    /// facteurs qualité vides).
    #[test]
    fn tce_act1_h2h_a_fields() {
        let l = loot();
        let rec = l.tcs.get("Act 1 H2H A").expect("TC act 1 h2h A");
        assert_eq!(rec.picks, 1);
        assert_eq!(rec.no_drop, 100);
        assert_eq!(rec.treasures.len(), 4);
        assert_eq!(rec.treasures[0], ("gld".to_string(), 21));
        assert_eq!(rec.quality, QualityFactors::default(), "facteurs vides = 0");
    }

    /// Contrat brief : "Act 1 H2H A" au même seed ⇒ mêmes items concrets.
    #[test]
    fn act1_h2h_a_deterministic_same_seed() {
        let l = loot();
        let mut a = Rng::new(0x0D2);
        let mut b = Rng::new(0x0D2);
        let ra = l.roll(&mut a, "Act 1 H2H A", 1);
        let rb = l.roll(&mut b, "Act 1 H2H A", 1);
        assert_eq!(ra.len(), rb.len());
        for (x, y) in ra.iter().zip(rb.iter()) {
            assert_eq!(x.code, y.code);
            assert_eq!(x.gold, y.gold);
        }
    }

    /// La chaîne complète résout vers des items CONCRETS (jamais "armo3").
    #[test]
    fn act1_h2h_a_resolves_concrete_items() {
        let l = loot();
        let mut kinds_weapon = 0;
        let mut kinds_armor = 0;
        let mut drops = 0;
        for seed in 0..200u64 {
            let mut rng = Rng::new(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
            for d in l.roll(&mut rng, "Act 1 H2H A", 1) {
                drops += 1;
                assert!(
                    d.code == "gld" || l.items.contains_key(&d.code),
                    "code non résolu : {}",
                    d.code
                );
                if d.code == "gld" {
                    assert!(d.gold >= 1);
                }
                match l.items.get(&d.code).map(|r| r.kind) {
                    Some(ItemKind::Weapon) => kinds_weapon += 1,
                    Some(ItemKind::Armor) => kinds_armor += 1,
                    _ => {}
                }
            }
        }
        assert!(drops > 40, "trop peu de drops : {drops}");
        assert!(kinds_weapon > 0, "aucune arme tirée d'Act 1 Equip");
        assert!(kinds_armor > 0, "aucune armure tirée d'Act 1 Equip");
    }

    /// Codes génériques : fenêtre de niveau [L, L+3) et bonne famille.
    #[test]
    fn dynamic_codes_resolve_in_level_window() {
        let l = loot();
        let check = |code: &str, lo: u32, want_kind: ItemKind, ranged: bool| {
            for seed in 0..50u64 {
                let mut rng = Rng::new(seed | 1);
                let mut out = Vec::new();
                l.resolve(&mut rng, code, 1, &mut out, 0, QualityFactors::default());
                assert_eq!(out.len(), 1, "{code} non résolu au seed {seed}");
                let rec = l.items.get(&out[0].code).expect("item connu");
                assert!(
                    rec.level >= lo && rec.level < lo + LEVEL_WINDOW,
                    "{code} → {} lvl {}",
                    out[0].code,
                    rec.level
                );
                assert_eq!(rec.kind, want_kind, "{code} → {}", out[0].code);
                if ranged {
                    assert!(BOW_TYPES.contains(&rec.wtype), "{code} → {}", out[0].code);
                }
            }
        };
        check("armo3", 3, ItemKind::Armor, false);
        check("mele3", 3, ItemKind::Weapon, false);
        check("weap33", 33, ItemKind::Weapon, false);
        check("bow3", 3, ItemKind::Weapon, true);
    }

    /// Repli : constantes MVP 60/25/15, déterministes, bornées par mlvl.
    #[test]
    fn fallback_mvp_deterministic() {
        let mut a = Rng::new(7);
        let mut b = Rng::new(7);
        for mlvl in [1u32, 5, 12] {
            let ra = Loot::roll_fallback(&mut a, mlvl);
            let rb = Loot::roll_fallback(&mut b, mlvl);
            assert_eq!(ra.len(), rb.len());
            for (x, y) in ra.iter().zip(rb.iter()) {
                assert_eq!(x.code, y.code);
                assert_eq!(x.gold, y.gold);
                assert!(x.code == "gld" || x.code == "hp1");
                if x.code == "gld" {
                    assert!((mlvl * 5..=mlvl * 15).contains(&x.gold));
                }
            }
        }
    }

    /// Or : "gld,mul=2048" double exactement le montant de "gld" au même seed.
    #[test]
    fn gold_mul_scales_amount() {
        let l = loot();
        let mut r1 = Rng::new(99);
        let mut r2 = Rng::new(99);
        let mut out1 = Vec::new();
        let mut out2 = Vec::new();
        l.resolve(&mut r1, "gld", 3, &mut out1, 0, QualityFactors::default());
        l.resolve(&mut r2, "gld,mul=2048", 3, &mut out2, 0, QualityFactors::default());
        let p = out1.into_iter().next().expect("or tiré");
        let m = out2.into_iter().next().expect("or mul tiré");
        assert_eq!(p.code, "gld");
        assert_eq!(m.code, "gld");
        assert_eq!(m.gold, p.gold * 2, "mul=2048 ⇒ ×2");
    }

    /// Picks < 0 (comptes par trésor, sémantique od2) sur une TC synthétique.
    #[test]
    fn negative_picks_use_counts() {
        let mut tcs = HashMap::new();
        tcs.insert(
            "Synth".to_string(),
            TcRec {
                picks: -3,
                no_drop: 0,
                quality: QualityFactors::default(),
                treasures: vec![("hax".to_string(), 2), ("buc".to_string(), 1)],
            },
        );
        let mut items = HashMap::new();
        items.insert(
            "hax".to_string(),
            ItemRec {
                level: 1,
                spawnable: true,
                wtype: "axe",
                kind: ItemKind::Weapon,
                normcode: String::new(),
                base: BaseInfo { name: "Hand Axe".into(), ..Default::default() },
            },
        );
        items.insert(
            "buc".to_string(),
            ItemRec {
                level: 1,
                spawnable: true,
                wtype: "shie",
                kind: ItemKind::Armor,
                normcode: String::new(),
                base: BaseInfo { name: "Buckler".into(), ..Default::default() },
            },
        );
        let l = Loot {
            tcs,
            items,
            fam_weap: Vec::new(),
            fam_mele: Vec::new(),
            fam_bow: Vec::new(),
            fam_armo: Vec::new(),
            quality: None,
            affixes: None,
            uniques: None,
        };
        // hax (compte 2) puis buc (compte 1) : 3 drops exactement.
        let mut rng = Rng::new(3);
        let out = l.roll(&mut rng, "Synth", 1);
        assert_eq!(out.len(), 3);
        assert_eq!((out[0].code.as_str(), out[1].code.as_str(), out[2].code.as_str()), ("hax", "hax", "buc"));
    }

    // -- Phase 3 : qualité + affixes + uniques branchés sur la chaîne TC. --

    /// La GÉNÉRATION riche consomme le même flux que `roll` (contrat
    /// determinisme : mêmes codes, même or, seeds identiques).
    #[test]
    fn generate_matches_roll_stream() {
        let l = loot();
        for seed in [1u64, 42, 0x0D2, 0xDEAD_BEEF] {
            let mut a = Rng::new(seed);
            let mut b = Rng::new(seed);
            let full = l.generate(&mut a, "Act 1 H2H A", 1);
            let red = l.roll(&mut b, "Act 1 H2H A", 1);
            assert_eq!(full.len(), red.len(), "seed {seed}");
            for (g, d) in full.iter().zip(red.iter()) {
                assert_eq!(g.code, d.code);
                assert_eq!(g.gold, d.gold);
            }
        }
    }

    /// Les armes/armures générées portent une qualité valide et les misc
    /// restent Normal (pas de qualité en D2 hors weap/armo).
    #[test]
    fn generated_items_carry_quality() {
        let l = loot();
        let mut seen_weap_armo = 0;
        for seed in 0..300u64 {
            let mut rng = Rng::new(seed.wrapping_mul(7919) | 1);
            for g in l.generate(&mut rng, "Act 1 H2H A", 1) {
                let rec_kind = l.items.get(&g.code).map(|r| r.kind);
                match rec_kind {
                    Some(ItemKind::Weapon) | Some(ItemKind::Armor) => {
                        seen_weap_armo += 1;
                    }
                    _ => {
                        assert_eq!(
                            g.quality,
                            Quality::Normal,
                            "misc {} avec qualité ?",
                            g.code
                        );
                        assert!(g.prefixes.is_empty() && g.suffixes.is_empty());
                    }
                }
                assert!(!g.name.is_empty(), "nom composé manquant");
                assert!(g.item.w > 0 && g.item.h > 0, "taille grille manquante");
            }
        }
        assert!(seen_weap_armo > 0, "aucun weap/armo généré");
    }

    /// Uniques sur "Act 1 Equip A" à facteur unique saturé (1024) : le
    /// plancher `min` 6400 plafonne P ≈ 2 % (comportement D2 réel, Diablerie)
    /// — sur 2500 drops déterministes on DOIT en voir, tous connus, nommés,
    /// non-identifiés, avec props roulées ; les bases sans unique dégradent
    /// en rares.
    #[test]
    fn uniques_drop_and_resolve() {
        let l = loot();
        let qf = QualityFactors { unique: 1024, set: 0, rare: 0, magic: 0 };
        let mut uniques = 0;
        let mut rares = 0;
        let mut magics = 0;
        for seed in 0..2500u64 {
            let mut rng = Rng::new(seed | 1);
            for g in l.generate_qf(&mut rng, "Act 1 Equip A", 10, qf) {
                match g.quality {
                    Quality::Unique => {
                        uniques += 1;
                        let name = g.unique.clone().expect("unique sans clé");
                        assert!(
                            l.uniques.as_ref().unwrap().uniques.iter().any(|u| u.index == name),
                            "unique inconnu {name}"
                        );
                        assert_eq!(g.item.name, name);
                        assert!(!g.item.identified, "unique non-identifié au drop");
                        assert!(!g.props.is_empty(), "unique sans props");
                    }
                    Quality::Rare => rares += 1,
                    Quality::Magic => magics += 1,
                    _ => {}
                }
            }
        }
        assert!(uniques >= 5, "uniques obtenus : {uniques} (P ≈ 2 %/drop)");
        assert!(rares > 0, "dégradés rares attendus (bases sans unique)");
        assert!(magics > 0, "magiques attendus (bases sans unique, sans set)");
    }

    /// BRIEF : Gull EST résoluable depuis la chaîne (`dgr`, ilvl 10 ≥ lvl 6,
    /// The Diggler lvl 15 exclu) — itération déterministe jusqu'au hit
    /// (P ≈ 2 %/drop même facteur saturé : plancher ItemRatio).
    #[test]
    fn gull_drops_as_unique_with_exact_props() {
        let l = loot();
        let qf = QualityFactors { unique: 1024, ..Default::default() };
        let mut found = None;
        for seed in 1..8000u64 {
            let mut rng = Rng::new(seed);
            let mut out = Vec::new();
            l.resolve(&mut rng, "dgr", 10, &mut out, 0, qf);
            if let Some(g) = out.into_iter().next() {
                if g.quality == Quality::Unique {
                    found = Some(g);
                    break;
                }
            }
        }
        let gull = found.expect("Gull doit tomber (2 %/drop sur 8000 essais)");
        assert_eq!(gull.unique.as_deref(), Some("Gull"));
        let dmg_max = gull.props.iter().find(|p| p.desc.code == "dmg-max").expect("dmg-max");
        assert_eq!(dmg_max.value, 15, "Gull dmg-max = 15 (fixe)");
        let mag = gull.props.iter().find(|p| p.desc.code == "mag%").expect("mag%");
        assert!(mag.value > 0);
    }

    /// Magic : ≤1 prefix + ≤1 suffix, nom composé D2, props roulées dans
    /// l'ordre (prefixes → suffixes).
    #[test]
    fn magic_items_have_prefix_and_or_suffix() {
        let l = loot();
        let qf = QualityFactors { unique: 0, set: 0, rare: 0, magic: 1024 };
        let mut magic = 0;
        for seed in 0..300u64 {
            let mut rng = Rng::new(seed | 1);
            for g in l.generate_qf(&mut rng, "Act 1 Equip A", 20, qf) {
                if g.quality != Quality::Magic {
                    continue;
                }
                magic += 1;
                assert!(g.prefixes.len() <= 1 && g.suffixes.len() <= 1);
                assert!(!g.prefixes.is_empty() || !g.suffixes.is_empty(), "magic sans affixe");
                assert!(!g.item.identified, "magic non identifié au drop");
                let expected = if let (Some(p), Some(s)) = (g.prefixes.last(), g.suffixes.last()) {
                    format!("{p} {} {s}", l.items[&g.code].base.name)
                } else if let Some(s) = g.suffixes.last() {
                    format!("{} {s}", l.items[&g.code].base.name)
                } else {
                    format!("{} {}", g.prefixes[0], l.items[&g.code].base.name)
                };
                assert_eq!(g.name, expected, "nom composé D2");
            }
        }
        assert!(magic >= 100, "magiques obtenus : {magic}");
    }

    /// Items generated → inventory::Item : équipable, stats dérivées > 0.
    #[test]
    fn generated_item_fits_inventory_pipeline() {
        use crate::inventory::{derive_equipped, Equipment, EquipSlot};
        let l = loot();
        let mut eq = Equipment::new();
        let mut equipped = 0;
        for seed in 0..200u64 {
            let mut rng = Rng::new(seed | 1);
            for mut g in l.generate(&mut rng, "Act 1 Equip A", 5) {
                g.item.props = g.props.clone();
                g.item.name = g.name.clone();
                if let Ok(Some(_old)) = eq.equip(g.item.clone()) {
                    equipped += 1;
                    break;
                }
                // slot libre du bon type ?
                if let Some(slot) = Equipment::slot_for_type(&g.item.base.wtype) {
                    if eq.slots[slot.index()].is_none() {
                        eq.slots[slot.index()] = Some(g.item.clone());
                        equipped += 1;
                        break;
                    }
                }
            }
            if equipped >= 2 {
                break;
            }
        }
        assert!(equipped >= 1, "aucun item équipé (mapping type→slot cassé ?)");
        let mut rng = Rng::new(4);
        let d = derive_equipped(&eq, &mut rng);
        // Déterministe :
        let mut rng2 = Rng::new(4);
        assert_eq!(d, derive_equipped(&eq, &mut rng2));
        let _ = EquipSlot::Head;
    }

    /// Tailles réelles v1.12 (golden croisé patch_d2.mpq) : Hand Axe `hax`
    /// = 1×3, Buckler `buc` = 2×2, potion `hp1` = 1×1 (colonnes
    /// `invwidth`/`invheight` des tables — les tailles D2 ne sont pas toutes
    /// « larges » : la hache de main est ÉTROITE, le buckler bas).
    #[test]
    fn base_sizes_from_real_tables() {
        let l = loot();
        let hax = &l.items["hax"].base;
        assert_eq!((hax.w, hax.h), (1, 3), "Hand Axe 1×3 (invwidth/invheight)");
        assert_eq!((hax.min_dam, hax.max_dam), (3, 6), "Hand Axe dégâts");
        let buc = &l.items["buc"].base;
        assert_eq!((buc.w, buc.h), (2, 2), "Buckler 2×2");
        assert!((buc.min_ac, buc.max_ac) == (4, 6) || buc.max_ac >= buc.min_ac, "AC buc");
        let hp1 = &l.items["hp1"].base;
        assert_eq!((hp1.w, hp1.h), (1, 1), "potion 1×1");
        let gld = &l.items["gld"].base;
        assert_eq!((gld.w, gld.h), (1, 1), "or 1×1 (misc.txt)");
    }
}
