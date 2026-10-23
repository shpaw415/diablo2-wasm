//! `vendor` — MARCHANDS & GAMBLE v1.12 (domaine G1/G2, `finish-systemes.md` §G).
//!
//! Port des règles D2 réelles, data-driven depuis les tables v1.12 embarquées
//! (réf. brief : Diablerie + Nefarius' Vendor Guide, `storepage.txt` ; la
//! STRUCTURE des données = colonnes vendeurs de `weapons/armor/misc.txt`
//! réels — la table fait foi) :
//!
//! * **Stock par vendeur** : les tables portent pour CHAQUE vendeur des
//!   colonnes `XxxMin/XxxMax` (nombre d'exemplaires de la base normale) et
//!   `XxxMagicMin/XxxMagicMax/XxxMagicLvl` (exemplaires MAGIQUES + « magic
//!   level » du vendeur). Sentinelle v1.12 : vide/0/255 = non vendu. Ex.
//!   réels : Akara `hp1` 1-1 (permanent), `tsc` 11-17, baguettes `wnd` 4-6
//!   normales + 7-9 magiques (MagicLvl 1) ; Charsi `cap` 1-1 + 1 magique
//!   (MagicLvl 5). Permanents (`PermStoreItem = 1`, potions/scrolls/clés/
//!   tomes) : toujours en rayon, pas de tirage. Les autres : 80 % de chance
//!   PAR exemplaire (brief, guide vendeurs Diablerie/Nefarius).
//! * **Niveaux** : ilvl des items stockés = niveau du vendeur (= niveau du
//!   héros au restock, G1 « restock au changement de niveau de perso ») ;
//!   affixes des versions magiques tirés à `alvl = max(qlvl, ilvl + MagicLvl)`
//!   (le MagicLvl du vendeur joue le rôle du « magic level » d'item).
//! * **Économie** : achat = `cost` (colonne réelle weapons/armor/misc) ;
//!   vente = `cost/4` (plancher) ; réserve d'or du vendeur — Gheed 350-750
//!   (guide v1.12), Akara/Charsi = constante MVP 10 000 (source table non
//!   trouvée — restant) ; un vendeur ne paie pas au-delà de sa réserve (règle
//!   D2 réelle). Gamble : prix = `gamble cost` × (100 + 2·clvl) / 100.
//! * **Gamble (Gheed, MVP)** : liste tirée de `gamble.txt` (125 codes réels,
//!   extraits patch_d2.mpq), filtrée `spawnable && qlvl ≤ niveau vendeur` ;
//!   items NON identifiés, les affixes sont DÉJÀ roulés à la génération
//!   (révélation = lecture). Odds `DifficultyLevels.txt` v1.12 Normal :
//!   Rare 10000 / Set 100 / Unique 50 / Uber 90 / Ultra 33 — MVP : poids
//!   relatifs réels, magic = poids de Rare, Uber/Ultra pliés dans unique
//!   (formule exacte scalée clvl : restant, écart documenté).
//! * **Déterminisme** : un [`Rng`] seedé par appel (aucun état global) ;
//!   ordre de tirage fixe : or (flux séparé) → bases (ordre fichiers
//!   weap/armo/misc) → compte normal → exemplaires (80 %) → compte magique →
//!   exemplaires (80 % + affixes). Aucune itération de HashMap dans un tirage.
//!
//! Zéro IO, zéro `world.rs` : l'or du héros vit dans [`TradeSession`]
//! (paramètre), l'intégration moteur est un contrat documenté (rapport
//! `impl-vendors.md`).

use std::collections::HashMap;

use crate::affixes::{self, AffixKind, AffixTables, ItemTypeSet};
use crate::inventory::{BaseInfo, Item};
use crate::item_props::{self, RolledProp};
use crate::item_quality::Quality;
use crate::loot::tsv_rows;
use crate::rng::Rng;
use crate::uniques_sets::{self, UniquesSets};
use crate::ItemKind;

/// Armes v1.12 (patch_d2.mpq) — colonnes vendeurs + `cost`/`gamble cost`.
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
/// Armures v1.12 (casques/corps/bottes/gants/boucliers).
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
/// Divers v1.12 — potions (hp1-5/mp1-5/rvs/rvl), scrolls, clés, tomes, bijoux
/// (`rin`/`amu`).
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");
/// gamble.txt v1.12 (125 codes gambleables — extrait patch_d2.mpq le
/// 2026-10-23, sha256 46a41a1d55deffd6…8ae34833c694).
const GAMBLE_TXT: &str = include_str!("../../../../assets/patch_gamble.txt");
/// DifficultyLevels.txt v1.12 (odds gamble GambleRare/Set/Unique/Uber/Ultra).
const DIFFICULTY_TXT: &str = include_str!("../../../../assets/patch_difficultylevels.txt");
/// StorePage.txt v1.12 (4 pages : armo/weap/mag/misc).
const STOREPAGE_TXT: &str = include_str!("../../../../assets/patch_storepage.txt");
/// Affixes (stock magique des vendeurs + gamble).
const MAGIC_PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
const MAGIC_SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");
/// Uniques/sets (le gamble peut sortir un unique/set réel, dégradé rare sinon).
const UNIQUE_ITEMS_TXT: &str = include_str!("../../../../assets/patch_uniqueitems.txt");
const SET_ITEMS_TXT: &str = include_str!("../../../../assets/patch_setitems.txt");
const SETS_TXT: &str = include_str!("../../../../assets/patch_sets.txt");

/// Chance d'apparition PAR exemplaire non-permanent (brief : guide vendeurs
/// Diablerie/Nefarius ; formule exacte du guide : restant).
const SPAWN_PCT: u32 = 80;
/// Réserve d'or de Gheed (Normal, guide v1.12) — roulée à la génération.
const GHEED_GOLD: (u32, u32) = (350, 750);
/// Réserve MVP Akara/Charsi (source table non trouvée — restant).
const DEFAULT_VENDOR_GOLD: u32 = 10_000;
/// Taille de la page de gamble (écran StorePage MVP).
const GAMBLE_SLOTS: usize = 12;
/// Plafond du facteur clvl du prix de gamble (garde anti-overflow).
const GAMBLE_CLVL_CAP: u32 = 50;
/// Types misc POUVANT être magiques (bijoux/charmes — potions/gemmes/clés
/// n'ont pas de qualité en D2, même convention que `loot.rs`).
const MAGIC_MISC_TYPES: &[&str] = &["ring", "amul", "jewl", "ques"];
/// Types d'armes hors mêlée / arcs (même découpage familles que `loot.rs`).
const NON_MELEE: &[&str] = &["bow", "xbow", "abow", "jave", "ajav", "tkni", "tpot"];
const BOW_TYPES: &[&str] = &["bow", "xbow", "abow"];

/// Les vendeurs implantés (acte I — les colonnes des 20 vendeurs des tables
/// sont parsées de la même façon ; ajouter un vendeur = une variante ici).
/// Sérialisable : porté par les définitions NPC de ville (`npc.rs`, W16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum VendorId {
    /// Akara — potions/scrolls/clés/tomes + baguettes/bâtons.
    Akara,
    /// Charsi — armes/armures de forgeron + carquois.
    Charsi,
    /// Gheed — armes/armures variées + clés, et LE gamble.
    Gheed,
}

impl VendorId {
    /// Préfixe exact des colonnes de table (`AkaraMin`…).
    fn column_prefix(self) -> &'static str {
        match self {
            VendorId::Akara => "Akara",
            VendorId::Charsi => "Charsi",
            VendorId::Gheed => "Gheed",
        }
    }

    /// Nom affichable.
    pub fn name(self) -> &'static str {
        match self {
            VendorId::Akara => "Akara",
            VendorId::Charsi => "Charsi",
            VendorId::Gheed => "Gheed",
        }
    }

    /// Réserve d'or (min, max) — Gheed 350-750 (guide), autres MVP fixe.
    fn gold_range(self) -> (u32, u32) {
        match self {
            VendorId::Gheed => GHEED_GOLD,
            _ => (DEFAULT_VENDOR_GOLD, DEFAULT_VENDOR_GOLD),
        }
    }

    /// Parse depuis un nom ("Akara"…) — intégration data-driven.
    pub fn parse(name: &str) -> Option<VendorId> {
        const ALL: [VendorId; 3] = [VendorId::Akara, VendorId::Charsi, VendorId::Gheed];
        ALL.into_iter().find(|v| v.column_prefix().eq_ignore_ascii_case(name))
    }
}

/// Comptes de stock d'une base pour UN vendeur (colonnes réelles v1.12).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct VendorCounts {
    /// Exemplaires normaux min (`XxxMin`, vide/0/255 ⇒ 0).
    pub min: u32,
    /// Exemplaires normaux max (`XxxMax`).
    pub max: u32,
    /// Exemplaires magiques min (`XxxMagicMin`).
    pub magic_min: u32,
    /// Exemplaires magiques max (`XxxMagicMax`).
    pub magic_max: u32,
    /// « Magic level » du vendeur pour cette base (`XxxMagicLvl`, 255 ⇒ 0) —
    /// ajouté à l'ilvl avant le tirage d'affixes.
    pub magic_lvl: u32,
}

/// Une base vendable (ligne de weapons/armor/misc v1.12).
#[derive(Debug, Clone)]
pub struct VendorBase {
    /// Code table (`hp1`, `hax`, `cap`, `rin`…).
    pub code: String,
    /// Nom anglais de la base (colonne `name`).
    pub name: String,
    /// Niveau de la base (`level` — qlvl, filtre gamble).
    pub level: u32,
    /// `spawnable = 1`.
    pub spawnable: bool,
    /// Valeur de base (`cost`) — PRIX D'ACHAT (marge D2 : achat = valeur).
    pub cost: u32,
    /// Prix de gamble (`gamble cost`) — hax 4510, rin 50 000, amu 63 000…
    pub gamble_cost: u32,
    /// Catégorie.
    pub kind: ItemKind,
    /// Peut rouler des affixes (armes/armures + bijoux/charmes misc).
    pub magic_capable: bool,
    /// Permanent (`PermStoreItem = 1`) : toujours en rayon.
    pub permanent: bool,
    /// Infos de base réelles (taille/AC/dégâts/requis) pour [`Item`].
    pub base: BaseInfo,
    /// Couverture de types pour le filtrage d'affixes.
    pub types: ItemTypeSet,
    /// Comptes par vendeur (seuls les vendeurs qui la vendent sont présents).
    pub stock: HashMap<VendorId, VendorCounts>,
}

/// Odds de gamble (`DifficultyLevels.txt` — colonnes Gamble*, valeurs réelles).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GambleOdds {
    /// `GambleRare` (Normal : 10000).
    pub rare: u32,
    /// `GambleSet` (100).
    pub set: u32,
    /// `GambleUnique` (50).
    pub unique: u32,
    /// `GambleUber` (90) — uniques de codes « uber » (plié dans unique, MVP).
    pub uber: u32,
    /// `GambleUltra` (33) — idem « ultra » (plié dans unique, MVP).
    pub ultra: u32,
}

impl GambleOdds {
    /// Ligne d'une difficulté (`Normal`/`Nightmare`/`Hell`) de la table réelle.
    fn from_table(txt: &str, difficulty: &str) -> Option<GambleOdds> {
        let rows = tsv_rows(txt);
        let hdr = rows.first()?;
        let col = |n: &str| -> Option<usize> { hdr.iter().position(|h| h.eq_ignore_ascii_case(n)) };
        let name_c = col("Name")?;
        let get = |r: &[String], n: &str| col(n).and_then(|i| r.get(i)).and_then(|s| s.trim().parse().ok());
        for r in rows.iter().skip(1) {
            if r.get(name_c).map(|s| s.trim().eq_ignore_ascii_case(difficulty)) == Some(true) {
                return Some(GambleOdds {
                    rare: get(r, "GambleRare")?,
                    set: get(r, "GambleSet")?,
                    unique: get(r, "GambleUnique")?,
                    uber: get(r, "GambleUber")?,
                    ultra: get(r, "GambleUltra")?,
                });
            }
        }
        None
    }

    /// Poids unique total (Uber/Ultra pliés — MVP documenté).
    fn unique_weight(&self) -> u32 {
        self.unique + self.uber + self.ultra
    }
}

/// Catalogue marchand v1.12 embarqué (bases + gamble + odds + affixes).
pub struct VendorCatalog {
    /// Bases vendables, ordre fixe (fichiers weap → armo → misc, ordre lignes).
    bases: Vec<VendorBase>,
    /// Index code → position dans `bases`.
    by_code: HashMap<String, usize>,
    /// Codes gambleables (`gamble.txt`, ordre du fichier — 125 réels).
    gamble_codes: Vec<String>,
    /// Odds gamble (difficulté Normal v1.12).
    pub odds: GambleOdds,
    /// Pages de store (`StorePage.txt` réel : (nom, code)).
    pub store_pages: Vec<(String, String)>,
    /// Tables d'affixes (stock magique + gamble).
    affixes: Option<AffixTables>,
    /// Uniques/sets (gamble).
    uniques: Option<UniquesSets>,
}

impl VendorCatalog {
    /// Parse les tables embarquées ; `None` si une table d'items est
    /// illisible (l'appelant retombe sur un magasin vide, jamais de panique).
    pub fn build() -> Option<VendorCatalog> {
        let mut bases = Vec::new();
        load_table(WEAPONS_TXT, ItemKind::Weapon, false, &mut bases);
        load_table(ARMOR_TXT, ItemKind::Armor, true, &mut bases);
        load_table(MISC_TXT, ItemKind::Misc, false, &mut bases);
        if bases.is_empty() {
            return None;
        }
        let by_code: HashMap<String, usize> =
            bases.iter().enumerate().map(|(i, b)| (b.code.clone(), i)).collect();
        // gamble.txt : colonnes (name, code) — codes retenus (nom = TBL restant).
        let gamble_codes: Vec<String> = tsv_rows(GAMBLE_TXT)
            .into_iter()
            .skip(1)
            .filter_map(|r| {
                let c = r.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
                (!c.is_empty()).then_some(c)
            })
            .collect();
        // Pages de store réelles (armo/weap/mag/misc).
        let store_pages: Vec<(String, String)> = tsv_rows(STOREPAGE_TXT)
            .into_iter()
            .skip(1)
            .filter_map(|r| {
                let n = r.first()?.trim().to_string();
                let c = r.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
                (!n.is_empty() && !c.is_empty()).then_some((n, c))
            })
            .collect();
        Some(VendorCatalog {
            bases,
            by_code,
            gamble_codes,
            odds: GambleOdds::from_table(DIFFICULTY_TXT, "Normal")?,
            store_pages,
            affixes: affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT),
            uniques: uniques_sets::parse(UNIQUE_ITEMS_TXT, SET_ITEMS_TXT, SETS_TXT),
        })
    }

    /// Base par code.
    pub fn base(&self, code: &str) -> Option<&VendorBase> {
        self.by_code.get(code).map(|&i| &self.bases[i])
    }

    /// Valeur de base d'un code (prix de référence pour la vente).
    pub fn cost_of(&self, code: &str) -> Option<u32> {
        self.base(code).map(|b| b.cost)
    }

    /// Nombre de bases parsées (diagnostic/tests).
    pub fn len(&self) -> usize {
        self.bases.len()
    }

    /// Catalogue non vide (test `is_empty` complémentaire).
    pub fn is_empty(&self) -> bool {
        self.bases.is_empty()
    }

    /// Prix de vente D2 : valeur / 4 (plancher) — marge du marchand.
    pub fn sell_price(&self, code: &str) -> Option<u32> {
        self.cost_of(code).map(|v| v / 4)
    }

    /// Prix de gamble D2 : `gamble cost × (100 + 2·clvl) / 100` (plancher).
    pub fn gamble_price(&self, code: &str, clvl: u32) -> Option<u32> {
        let gc = self.base(code)?.gamble_cost;
        Some(gc.saturating_mul(100 + 2 * clvl.clamp(0, GAMBLE_CLVL_CAP)) / 100)
    }

    /// Exemplaires d'une base pour un vendeur (colonnes réelles).
    pub fn counts(&self, code: &str, id: VendorId) -> Option<&VendorCounts> {
        self.base(code).and_then(|b| b.stock.get(&id))
    }
}

/// Parse une table d'items vers les bases vendables (colonnes vendeurs
/// repérées dans l'en-tête — les 20 vendeurs v1.12, 3 retenus ici).
fn load_table(txt: &str, fallback_kind: ItemKind, is_armo: bool, out: &mut Vec<VendorBase>) {
    let rows = tsv_rows(txt);
    let Some(hdr) = rows.first() else { return };
    let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(code_c), Some(level_c), Some(spawn_c), Some(cost_c), Some(gamb_c)) =
        (col("code"), col("level"), col("spawnable"), col("cost"), col("gamble cost"))
    else {
        return;
    };
    let name_c = col("name");
    let type_c = col("type");
    let perm_c = col("PermStoreItem");
    let w_c = col("invwidth");
    let h_c = col("invheight");
    let minac_c = col("minac");
    let maxac_c = col("maxac");
    let mindam_c = col("mindam");
    let maxdam_c = col("maxdam");
    let reqstr_c = col("reqstr");
    let reqdex_c = col("reqdex");
    let lreq_c = col("levelreq");
    // Colonnes vendeurs (positions une fois pour toutes) :
    // [Min, Max, MagicMin, MagicMax, MagicLvl].
    let vcols: Vec<(VendorId, [Option<usize>; 5])> =
        [VendorId::Akara, VendorId::Charsi, VendorId::Gheed]
            .into_iter()
            .map(|id| {
                let p = id.column_prefix();
                (
                    id,
                    [
                        col(&format!("{p}Min")),
                        col(&format!("{p}Max")),
                        col(&format!("{p}MagicMin")),
                        col(&format!("{p}MagicMax")),
                        col(&format!("{p}MagicLvl")),
                    ],
                )
            })
            .collect();

    for r in rows.iter().skip(1) {
        let cell = |i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("");
        let num = |i: Option<usize>| cell(i).parse::<u32>().unwrap_or(0);
        // Sentinelle v1.12 : vide/0/255 = non vendu / inutilisé.
        let count = |i: Option<usize>| match cell(i) {
            "" | "0" | "255" => 0,
            s => s.parse().unwrap_or(0),
        };
        let magic_lvl = |i: Option<usize>| match cell(i) {
            "" | "255" => 0,
            s => s.parse().unwrap_or(0),
        };
        let code = cell(Some(code_c)).to_string();
        if code.is_empty() {
            continue;
        }
        let wtype = cell(type_c).to_string();
        let kind = match fallback_kind {
            ItemKind::Misc => misc_kind(&wtype),
            k => k,
        };
        let magic_capable = matches!(kind, ItemKind::Weapon | ItemKind::Armor)
            || MAGIC_MISC_TYPES.contains(&wtype.as_str());
        // Type interné (ItemTypeSet porte `&'static str`) — fuite bornée à la
        // construction du catalogue (une fois par process, quelques Ko).
        let wtype_static: &'static str = Box::leak(wtype.clone().into_boxed_str());
        let is_weap = crate::loot::is_weapon_type(wtype_static);
        let types = ItemTypeSet {
            wtype: wtype_static,
            is_weap,
            is_mele: is_weap && !NON_MELEE.contains(&wtype_static),
            is_bow: BOW_TYPES.contains(&wtype_static),
            is_armo,
        };
        let mut stock = HashMap::new();
        for (id, [cmin, cmax, cmmin, cmmax, cmlvl]) in &vcols {
            let (min, max) = (count(*cmin), count(*cmax));
            let (magic_min, magic_max) = (count(*cmmin), count(*cmmax));
            if max > 0 || magic_max > 0 {
                stock.insert(
                    *id,
                    VendorCounts { min, max, magic_min, magic_max, magic_lvl: magic_lvl(*cmlvl) },
                );
            }
        }
        out.push(VendorBase {
            code,
            name: cell(name_c).to_string(),
            level: num(Some(level_c)),
            spawnable: cell(Some(spawn_c)) == "1",
            cost: num(Some(cost_c)),
            gamble_cost: num(Some(gamb_c)),
            kind,
            magic_capable,
            permanent: perm_c.map(|i| cell(Some(i)) == "1").unwrap_or(false),
            base: BaseInfo {
                name: cell(name_c).to_string(),
                wtype,
                w: num(w_c),
                h: num(h_c),
                min_ac: num(minac_c),
                max_ac: num(maxac_c),
                min_dam: num(mindam_c),
                max_dam: num(maxdam_c),
                req_str: num(reqstr_c),
                req_dex: num(reqdex_c),
                req_level: num(lreq_c),
            },
            types,
            stock,
        });
    }
}

/// Catégorie misc : potions ⇒ [`ItemKind::Potion`], reste ⇒ [`ItemKind::Misc`]
/// (même convention que `loot.rs`).
fn misc_kind(wtype: &str) -> ItemKind {
    match wtype {
        "hpot" | "rpot" => ItemKind::Potion,
        _ => ItemKind::Misc,
    }
}

/// Une entrée de rayon (ou de page de gamble) : l'item riche + ses prix.
#[derive(Debug, Clone, PartialEq)]
pub struct StockEntry {
    /// Item prêt pour l'inventaire (`inventory::Item`).
    pub item: Item,
    /// Catégorie (potion/arme/armure/misc).
    pub kind: ItemKind,
    /// Prix d'ACHAT = valeur de base (`cost` ; magic MVP : base seule —
    /// restant formule ItemStatCost des affixes).
    pub buy: u32,
    /// Prix de VENTE = valeur / 4.
    pub sell: u32,
    /// Permanent (`PermStoreItem = 1`) — ne disparaît jamais du rayon.
    pub permanent: bool,
}

/// Un vendeur : stock + réserve d'or + niveau (API pure, brief §3).
#[derive(Debug, Clone, PartialEq)]
pub struct Vendor {
    /// Identité (colonnes de table utilisées).
    pub id: VendorId,
    /// Nom affichable.
    pub name: String,
    /// Niveau du vendeur = niveau du héros au restock (ilvl des items).
    pub level: u32,
    /// Réserve d'or (Gheed 350-750, guide v1.12).
    pub gold: u32,
    /// Rayon.
    pub stock: Vec<StockEntry>,
}

impl Vendor {
    /// Génère le stock d'un vendeur — DÉTERMINISTE au seed (contrat RNG du
    /// projet). `vendor_level` = niveau du héros au restock (G1).
    pub fn generate(cat: &VendorCatalog, id: VendorId, vendor_level: u32, seed: u64) -> Vendor {
        let mut rng = Rng::new(seed);
        // Réserve d'or : flux séparé (l'or ne décale pas le tirage du stock).
        let (gmin, gmax) = id.gold_range();
        let gold = Rng::new(seed ^ 0xD26_0000_0000_0001).range_u32(gmin, gmax);
        let mut stock = Vec::new();
        for b in &cat.bases {
            let Some(cnt) = b.stock.get(&id) else { continue };
            // Exemplaires normaux.
            if cnt.max > 0 {
                let n = rng.range_u32(cnt.min.min(cnt.max), cnt.max);
                for _ in 0..n {
                    if !b.permanent && rng.below(100) >= SPAWN_PCT {
                        continue;
                    }
                    stock.push(base_entry(b));
                }
            }
            // Exemplaires magiques (affixes roulés à l'ilvl vendeur + MagicLvl).
            if cnt.magic_max > 0 && b.magic_capable {
                let n = rng.range_u32(cnt.magic_min.min(cnt.magic_max), cnt.magic_max);
                for _ in 0..n {
                    if !b.permanent && rng.below(100) >= SPAWN_PCT {
                        continue;
                    }
                    let alvl = affixes::alvl(b.level, vendor_level + cnt.magic_lvl);
                    stock.push(magic_entry(cat, b, alvl, &mut rng));
                }
            }
        }
        Vendor { id, name: id.name().to_string(), level: vendor_level, gold, stock }
    }

    /// Rafraîchissement (restock) : MÊME vendeur/niveau, NOUVEAU seed dérivé
    /// (les permanents restent, le reste re-tire — G1).
    pub fn refresh(&self, cat: &VendorCatalog, seed: u64) -> Vendor {
        let derived = seed.rotate_left(17) ^ 0x0D2B_0000_9E37_79B9;
        Vendor::generate(cat, self.id, self.level, derived)
    }
}

/// Entrée de rayon d'une base NORMALE (identifiée, prix = valeur).
fn base_entry(b: &VendorBase) -> StockEntry {
    StockEntry {
        item: Item {
            code: b.code.clone(),
            name: b.name.clone(),
            quality: Quality::Normal,
            identified: true,
            w: b.base.w,
            h: b.base.h,
            base: b.base.clone(),
            ..Default::default()
        },
        kind: b.kind,
        buy: b.cost,
        sell: b.cost / 4,
        permanent: b.permanent,
    }
}

/// Entrée MAGIQUE : affixes roulés maintenant (révélation = lecture, gamble
/// compris), non identifiée (règle D2 : à identifier après achat).
fn magic_entry(cat: &VendorCatalog, b: &VendorBase, alvl: u32, rng: &mut Rng) -> StockEntry {
    let mut item = Item {
        code: b.code.clone(),
        name: b.name.clone(),
        quality: Quality::Magic,
        identified: false,
        w: b.base.w,
        h: b.base.h,
        base: b.base.clone(),
        ..Default::default()
    };
    let mut prefixes = Vec::new();
    let mut suffixes = Vec::new();
    let mut props: Vec<RolledProp> = Vec::new();
    roll_magic(cat, b, alvl, &mut prefixes, &mut suffixes, &mut props, rng);
    item.prefixes = prefixes.clone();
    item.suffixes = suffixes.clone();
    item.props = props;
    item.name = compose_name(&prefixes, &b.name, &suffixes);
    StockEntry {
        item,
        kind: b.kind,
        buy: b.cost,
        sell: b.cost / 4,
        permanent: b.permanent,
    }
}

/// Nom magique D2 : `<Prefix> <Base> <Suffix>` (même convention que `loot.rs`).
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

/// Liste de GAMBLE (Gheed) : items non identifiés à prix majoré, affixes DÉJÀ
/// roulés à l'ilvl vendeur. Déterministe au seed.
pub fn gamble_list(
    cat: &VendorCatalog,
    vendor_level: u32,
    clvl: u32,
    seed: u64,
) -> Vec<StockEntry> {
    let mut rng = Rng::new(seed ^ 0x6A1B_0000_0000_0001);
    // Pool : codes de gamble.txt présents dans les tables, spawnables, sous
    // le niveau du vendeur (brief G2 : « item de base spawnable ≤ mlvl »).
    let pool: Vec<&VendorBase> = cat
        .gamble_codes
        .iter()
        .filter_map(|c| cat.base(c))
        .filter(|b| b.spawnable && b.level <= vendor_level)
        .collect();
    if pool.is_empty() {
        return Vec::new();
    }
    let odds = cat.odds;
    // Poids réels DifficultyLevels v1.12 ; magic = poids de GambleRare (MVP,
    // écart documenté : formule exacte scalée clvl restant).
    let w_unique = odds.unique_weight();
    let w_set = odds.set;
    let w_rare = odds.rare;
    let w_magic = odds.rare;
    let total = w_unique + w_set + w_rare + w_magic;
    let mut out = Vec::with_capacity(GAMBLE_SLOTS);
    for _ in 0..GAMBLE_SLOTS {
        let b = pool[rng.below(pool.len() as u32) as usize];
        let roll = rng.below(total);
        let quality = if roll < w_unique {
            Quality::Unique
        } else if roll < w_unique + w_set {
            Quality::Set
        } else if roll < w_unique + w_set + w_rare {
            Quality::Rare
        } else {
            Quality::Magic
        };
        out.push(gamble_entry(cat, b, quality, vendor_level, clvl, &mut rng));
    }
    out
}

/// Un item de gamble : qualité roulée MAINTENANT (affixes/uniques ilvl
/// vendeur), non identifié, prix = `gamble cost × (100 + 2·clvl)/100`.
fn gamble_entry(
    cat: &VendorCatalog,
    b: &VendorBase,
    quality: Quality,
    vendor_level: u32,
    clvl: u32,
    rng: &mut Rng,
) -> StockEntry {
    let alvl = affixes::alvl(b.level, vendor_level);
    let mut item = Item {
        code: b.code.clone(),
        name: b.name.clone(),
        quality,
        identified: false,
        w: b.base.w,
        h: b.base.h,
        base: b.base.clone(),
        ..Default::default()
    };
    let mut prefixes = Vec::new();
    let mut suffixes = Vec::new();
    let mut props: Vec<RolledProp> = Vec::new();
    match quality {
        Quality::Unique => {
            let empty = UniquesSets::default();
            let u = cat
                .uniques
                .as_ref()
                .unwrap_or(&empty)
                .select_unique(rng, vendor_level, &b.code);
            if let Some(u) = u {
                for d in &u.props {
                    props.push(item_props::roll(d, rng));
                }
                item.unique = Some(u.index.clone());
                item.name = u.index.clone();
            } else {
                // Aucun unique de cette base à ce niveau ⇒ rare (dégradation
                // D2 réelle, même convention que `loot.rs`).
                item.quality = Quality::Rare;
                roll_rare(cat, b, alvl, &mut prefixes, &mut suffixes, &mut props, rng);
                item.name = b.name.clone(); // noms rares : TBL restant
            }
        }
        Quality::Set => {
            let empty = UniquesSets::default();
            let s = cat
                .uniques
                .as_ref()
                .unwrap_or(&empty)
                .select_set_item(rng, vendor_level, &b.code);
            if let Some(s) = s {
                for d in &s.props {
                    props.push(item_props::roll(d, rng));
                }
                item.set_item = Some(s.index.clone());
                item.set = Some(s.set.clone());
                item.name = s.index.clone();
            } else {
                item.quality = Quality::Rare;
                roll_rare(cat, b, alvl, &mut prefixes, &mut suffixes, &mut props, rng);
                item.name = b.name.clone();
            }
        }
        Quality::Rare => {
            roll_rare(cat, b, alvl, &mut prefixes, &mut suffixes, &mut props, rng);
            item.name = b.name.clone();
        }
        // Magic (et repli Normal si la base n'est pas magic-capable).
        _ => {
            if b.magic_capable {
                item.quality = Quality::Magic;
                roll_magic(cat, b, alvl, &mut prefixes, &mut suffixes, &mut props, rng);
                item.name = compose_name(&prefixes, &b.name, &suffixes);
            } else {
                item.quality = Quality::Normal;
                item.name = b.name.clone();
            }
        }
    }
    item.prefixes = prefixes;
    item.suffixes = suffixes;
    item.props = props;
    let buy = cat.gamble_price(&b.code, clvl).unwrap_or(b.gamble_cost);
    StockEntry { item, kind: b.kind, buy, sell: b.cost / 4, permanent: false }
}

/// 4 affixes rares (groupes exclusifs — même tirage que `loot.rs`).
fn roll_rare(
    cat: &VendorCatalog,
    b: &VendorBase,
    alvl: u32,
    prefixes: &mut Vec<String>,
    suffixes: &mut Vec<String>,
    props: &mut Vec<RolledProp>,
    rng: &mut Rng,
) {
    let empty = AffixTables::default();
    let t = cat.affixes.as_ref().unwrap_or(&empty);
    for a in affixes::pick_rare(rng, t, &b.types, alvl) {
        for d in &a.mods {
            props.push(item_props::roll(d, rng));
        }
        match a.kind {
            AffixKind::Prefix => prefixes.push(a.name.clone()),
            AffixKind::Suffix => suffixes.push(a.name.clone()),
        }
    }
}

/// 0-1 prefix + 0-1 suffix magiques (même tirage que `loot.rs`).
fn roll_magic(
    cat: &VendorCatalog,
    b: &VendorBase,
    alvl: u32,
    prefixes: &mut Vec<String>,
    suffixes: &mut Vec<String>,
    props: &mut Vec<RolledProp>,
    rng: &mut Rng,
) {
    let empty = AffixTables::default();
    let t = cat.affixes.as_ref().unwrap_or(&empty);
    let (p, s) = affixes::pick_magic(rng, t, &b.types, alvl);
    for a in [p, s].into_iter().flatten() {
        for d in &a.mods {
            props.push(item_props::roll(d, rng));
        }
        match a.kind {
            AffixKind::Prefix => prefixes.push(a.name.clone()),
            AffixKind::Suffix => suffixes.push(a.name.clone()),
        }
    }
}

/// Erreurs de commerce (API pure — pas d'effet de bord partiel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TradeError {
    /// Or du héros insuffisant (prix demandé, or porté).
    NotEnoughGold { price: u32, gold: u32 },
    /// Le vendeur ne peut pas payer (réserve dépassée — règle D2 réelle).
    VendorCantAfford { price: u32, gold: u32 },
    /// Index de rayon invalide.
    BadIndex(usize),
    /// Code inconnu des tables (vente d'un item hors catalogue).
    UnknownItem(String),
}

/// Session de commerce : or du héros EN PARAMÈTRE (jamais `world.rs`).
#[derive(Debug, Clone, PartialEq)]
pub struct TradeSession {
    /// Le vendeur et son rayon.
    pub vendor: Vendor,
    /// Or porté par le héros.
    pub hero_gold: u32,
}

impl TradeSession {
    /// Ouvre la session (or du héros passé par l'appelant).
    pub fn new(vendor: Vendor, hero_gold: u32) -> TradeSession {
        TradeSession { vendor, hero_gold }
    }

    /// ACHète l'entrée `index` du rayon : paie le prix, retire du rayon,
    /// rend l'item + prix payé (ok/prix — brief §3). Le marchand encaisse.
    pub fn buy(&mut self, index: usize) -> Result<(Item, u32), TradeError> {
        let price = self.vendor.stock.get(index).ok_or(TradeError::BadIndex(index))?.buy;
        if self.hero_gold < price {
            return Err(TradeError::NotEnoughGold { price, gold: self.hero_gold });
        }
        let entry = self.vendor.stock.remove(index);
        self.hero_gold -= price;
        self.vendor.gold = self.vendor.gold.saturating_add(price);
        Ok((entry.item, price))
    }

    /// VEND un item du héros : prix = valeur/4, payé par la RÉSERVE du
    /// vendeur (règle D2 : au-delà de sa réserve, il refuse). L'item rejoint
    /// son rayon (revendable, comme en D2). Le catalogue donne la valeur.
    pub fn sell(&mut self, cat: &VendorCatalog, item: &Item) -> Result<u32, TradeError> {
        let price = cat
            .sell_price(&item.code)
            .ok_or_else(|| TradeError::UnknownItem(item.code.clone()))?;
        if price > self.vendor.gold {
            return Err(TradeError::VendorCantAfford { price, gold: self.vendor.gold });
        }
        self.vendor.gold -= price;
        self.hero_gold = self.hero_gold.saturating_add(price);
        self.vendor.stock.push(StockEntry {
            item: item.clone(),
            kind: item_kind_of(item),
            buy: cat.cost_of(&item.code).unwrap_or(0),
            sell: price,
            permanent: false,
        });
        Ok(price)
    }
}

/// Catégorie d'un item vendu (dérivée de sa base connue des tables).
fn item_kind_of(item: &Item) -> ItemKind {
    const ARMOR_TYPES: &[&str] = &[
        "tors", "body", "helm", "circ", "pelt", "phlm", "boot", "glov", "shie", "ashd", "belt",
    ];
    match item.base.wtype.as_str() {
        "hpot" | "rpot" => ItemKind::Potion,
        w if crate::loot::is_weapon_type(w) => ItemKind::Weapon,
        w if ARMOR_TYPES.contains(&w) => ItemKind::Armor,
        _ => ItemKind::Misc,
    }
}

// ---------------------------------------------------------------------------
// Tests — tables réelles v1.12, tout déterministe au seed.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn cat() -> VendorCatalog {
        VendorCatalog::build().expect("tables v1.12 valides")
    }

    fn stock_of<'a>(v: &'a Vendor, code: &str) -> Vec<&'a StockEntry> {
        v.stock.iter().filter(|e| e.item.code == code).collect()
    }

    // -- Catalogue : goldens des tables réelles ------------------------------

    /// Golden tables : hp1 = 30 or, Akara 1-1 permanent ; tsc 11-17 ; wnd
    /// Akara 4-6 + 7-9 magiques (MagicLvl 1) ; hax cost 170, gamble 4510,
    /// Charsi 0-1 + 1 magique (MagicLvl 1), Gheed 1-1 ; cap Charsi 1-1
    /// MagicLvl 5 ; rin gamble 50000, amu 63000 ; hp5 = 500 mais PAS chez
    /// Akara au v1.12 (la table fait foi sur le brief « hp1-hp5 chez Akara »).
    #[test]
    fn catalog_goldens_real_tables() {
        let c = cat();
        assert!(c.len() >= 600, "bases parsées : {}", c.len());

        let hp1 = c.base("hp1").expect("hp1");
        assert_eq!(hp1.cost, 30, "hp1 : valeur réelle misc.txt");
        assert!(hp1.permanent, "hp1 : PermStoreItem = 1");
        assert_eq!(hp1.kind, ItemKind::Potion);
        let ak = c.counts("hp1", VendorId::Akara).expect("Akara vend hp1");
        assert_eq!((ak.min, ak.max), (1, 1), "Akara hp1 1-1");
        assert!(c.counts("hp1", VendorId::Charsi).is_none(), "Charsi ne vend pas hp1");

        let tsc = c.counts("tsc", VendorId::Akara).expect("tsc Akara");
        assert_eq!((tsc.min, tsc.max), (11, 17), "Akara tsc 11-17");
        assert_eq!(tsc.magic_max, 0, "pas de scrolls « magiques »");

        let wnd = c.counts("wnd", VendorId::Akara).expect("wnd Akara");
        assert_eq!((wnd.min, wnd.max), (4, 6), "Akara baguettes normales 4-6");
        assert_eq!((wnd.magic_min, wnd.magic_max), (7, 9), "Akara baguettes magiques 7-9");
        assert_eq!(wnd.magic_lvl, 1);

        let hax = c.base("hax").expect("hax");
        assert_eq!(hax.cost, 170);
        assert_eq!(hax.gamble_cost, 4510);
        let ch = c.counts("hax", VendorId::Charsi).expect("Charsi hax");
        assert_eq!((ch.min, ch.max), (0, 1), "Charsi hax 0-1 (Min vide)");
        assert_eq!((ch.magic_min, ch.magic_max, ch.magic_lvl), (0, 1, 1));
        assert_eq!(c.counts("hax", VendorId::Gheed).unwrap().max, 1);

        let cap = c.counts("cap", VendorId::Charsi).expect("Charsi cap");
        assert_eq!((cap.min, cap.max), (1, 1));
        assert_eq!(cap.magic_lvl, 5, "Charsi MagicLvl casque 5");

        assert_eq!(c.base("rin").unwrap().gamble_cost, 50000);
        assert_eq!(c.base("amu").unwrap().gamble_cost, 63000);
        assert_eq!(c.base("hp5").unwrap().cost, 500, "hp5 : valeur réelle");
        assert!(c.counts("hp5", VendorId::Akara).is_none(), "hp5 hors Akara v1.12");
    }

    /// Odds gamble réelles DifficultyLevels.txt Normal (10000/100/50/90/33)
    /// + pages StorePage (4) + 125 codes gamble tous résolus + prix scalé
    /// clvl : rin 50000 × (100+2·8)/100 = 58 000, × (100+2)/100 = 51 000.
    #[test]
    fn odds_storepage_gamble_goldens() {
        let c = cat();
        assert_eq!(
            c.odds,
            GambleOdds { rare: 10000, set: 100, unique: 50, uber: 90, ultra: 33 }
        );
        assert_eq!(c.odds.unique_weight(), 173, "Uber+Ultra pliés dans unique");
        assert_eq!(c.store_pages.len(), 4, "StorePage.txt : 4 pages");
        assert!(c.store_pages.contains(&("Weapons Page".to_string(), "weap".to_string())));
        assert_eq!(c.gamble_codes.len(), 125, "gamble.txt : 125 codes");
        assert!(c.gamble_codes.iter().all(|cd| c.base(cd).is_some()));
        assert_eq!(c.gamble_price("rin", 8), Some(58000));
        assert_eq!(c.gamble_price("rin", 1), Some(51000));
        assert_eq!(c.sell_price("cap"), Some(16), "vente = 64/4");
    }

    // -- Stock : déterminisme + données réelles ------------------------------

    /// Même seed ⇒ stock identique (codes, prix, qualité) ; seed ≠ ⇒ stock
    /// différent ; vente = valeur/4 partout, tout prix > 0.
    #[test]
    fn akara_stock_deterministic_by_seed() {
        let c = cat();
        let a = Vendor::generate(&c, VendorId::Akara, 3, 0x0D2);
        let b = Vendor::generate(&c, VendorId::Akara, 3, 0x0D2);
        assert_eq!(a, b, "même seed ⇒ même stock");
        let d = Vendor::generate(&c, VendorId::Akara, 3, 0x0D3);
        assert_ne!(a.stock, d.stock, "seed différent ⇒ stock différent");
        for seed in 0..50u64 {
            let v = Vendor::generate(&c, VendorId::Charsi, seed as u32 % 10 + 1, seed);
            assert!(!v.stock.is_empty());
            for e in &v.stock {
                assert_eq!(e.sell, e.buy / 4, "vente = valeur/4");
                assert!(e.buy > 0, "prix nul sur {}", e.item.code);
            }
        }
    }

    /// Akara au seed fixé : hp1 ×1 EXACT (permanent 1-1, pas de tirage 80 %),
    /// mp1 ×1, scrolls 8..=17 (11-17 roulé × 80 %), baguettes présentes dont
    /// des magiques à ≤1 prefix + ≤1 suffix roulés, non identifiées ; prix
    /// réels hp1 : achat 30, vente 7 (30/4 plancher).
    #[test]
    fn akara_stock_matches_real_vendor_columns() {
        let c = cat();
        let v = Vendor::generate(&c, VendorId::Akara, 5, 42);
        assert_eq!(stock_of(&v, "hp1").len(), 1, "hp1 permanent 1-1");
        assert_eq!(stock_of(&v, "mp1").len(), 1, "mp1 permanent 1-1");
        let tsc = stock_of(&v, "tsc").len();
        assert!((8..=17).contains(&tsc), "tsc : 11-17 × 80 % ⇒ 8..17, got {tsc}");
        let wands = stock_of(&v, "wnd");
        assert!(!wands.is_empty(), "Akara vend des baguettes");
        let magic: Vec<&&StockEntry> =
            wands.iter().filter(|e| e.item.quality == Quality::Magic).collect();
        assert!(!magic.is_empty(), "baguettes magiques attendues (7-9 × 80 %)");
        for e in &magic {
            assert!(e.item.prefixes.len() <= 1 && e.item.suffixes.len() <= 1);
            // ≥1 affixe NOMINATIF (props = mods parsés ; certains mods v1.12
            // restent hors couverture descripteurs, ex. `charged` — écart
            // partagé avec loot.rs, restant item_props).
            assert!(
                !e.item.prefixes.is_empty() || !e.item.suffixes.is_empty(),
                "magic sans affixe"
            );
            assert!(!e.item.identified, "à identifier après achat (D2)");
            assert!(e.item.name != e.item.base.name, "nom composé prefix/suffix");
        }
        // Les bases normales restent identifiées (seul le magique attend ID).
        assert!(wands.iter().filter(|e| e.item.quality == Quality::Normal)
            .all(|e| e.item.identified));
        assert_eq!(stock_of(&v, "hp1")[0].buy, 30);
        assert_eq!(stock_of(&v, "hp1")[0].sell, 7);
        assert_eq!(stock_of(&v, "hp1")[0].permanent, true);
    }

    /// Charsi : armes ET armures en rayon, tailles de grille réelles portées
    /// (hax 1×3) ; Gheed : clés 4..9 (5-9 × 80 %), hax présent, les « clés
    /// magiques » de la table (données mortes) restent normales.
    #[test]
    fn charsi_gheed_stocks_match_tables() {
        let c = cat();
        let ch = Vendor::generate(&c, VendorId::Charsi, 8, 7);
        assert!(ch.stock.iter().any(|e| e.kind == ItemKind::Weapon));
        assert!(ch.stock.iter().any(|e| e.kind == ItemKind::Armor));
        for e in stock_of(&ch, "hax") {
            assert_eq!((e.item.w, e.item.h), (1, 3), "Hand Axe 1×3");
        }
        let g = Vendor::generate(&c, VendorId::Gheed, 8, 7);
        let keys = stock_of(&g, "key").len();
        assert!((4..=9).contains(&keys), "clés Gheed 5-9 × 80 % ⇒ 4..9, got {keys}");
        assert!(!stock_of(&g, "hax").is_empty(), "Gheed hax 1-1");
        assert!(stock_of(&g, "key").iter().all(|e| e.item.quality == Quality::Normal));
    }

    /// Réserve d'or : Gheed 350..=750 (guide v1.12) sur 200 seeds ; Akara =
    /// constante MVP 10 000 ; l'or ne décale PAS le tirage du stock (flux
    /// séparé : même seed ⇒ même stock quel que soit le vendeur/or).
    #[test]
    fn vendor_gold_reserve_gheed_350_750() {
        let c = cat();
        for seed in 0..200u64 {
            let g = Vendor::generate(&c, VendorId::Gheed, 1, seed);
            assert!((350..=750).contains(&g.gold), "Gheed gold {}", g.gold);
        }
        let a = Vendor::generate(&c, VendorId::Akara, 1, 1);
        assert_eq!(a.gold, 10_000, "Akara : réserve MVP");
        // Flux séparé : le stock de Charsi au seed 99 est reproductible.
        assert_eq!(
            Vendor::generate(&c, VendorId::Charsi, 1, 99).stock,
            Vendor::generate(&c, VendorId::Charsi, 1, 99).stock
        );
    }

    /// Restock : `refresh` dérive un NOUVEAU seed (le stock change) mais
    /// reste valide, même vendeur/niveau, et les permanents ne disparaissent
    /// jamais (hp1/mp1 ×1).
    #[test]
    fn refresh_keeps_permanents_and_rerolls() {
        let c = cat();
        let v1 = Vendor::generate(&c, VendorId::Akara, 4, 1000);
        let v2 = v1.refresh(&c, 1000);
        assert_ne!(v1.stock, v2.stock, "seed dérivé ⇒ nouveau tirage");
        assert_eq!(v2.id, v1.id, "même vendeur");
        assert_eq!(v2.level, v1.level, "même niveau");
        assert_eq!(stock_of(&v2, "hp1").len(), 1, "hp1 permanent toujours là");
        assert_eq!(stock_of(&v2, "mp1").len(), 1);
        for e in &v2.stock {
            assert_eq!(e.sell, e.buy / 4);
        }
    }

    // -- Économie : TradeSession ---------------------------------------------

    /// Achat : or du héros en paramètre, prix = valeur réelle (hp1 = 30),
    /// vente = valeur/4 (cap 64 ⇒ 16), l'entrée quitte le rayon, le marchand
    /// encaisse, l'item vendu redevient achetable.
    #[test]
    fn trade_buy_and_sell_margins() {
        let c = cat();
        let vendor = Vendor::generate(&c, VendorId::Akara, 1, 5);
        let hp_idx =
            vendor.stock.iter().position(|e| e.item.code == "hp1").expect("hp1 en rayon");
        let gold_vendor0 = vendor.gold;
        let mut s = TradeSession::new(vendor, 100);
        let (item, paid) = s.buy(hp_idx).expect("achat hp1");
        assert_eq!(paid, 30, "prix réel hp1");
        assert_eq!(item.code, "hp1");
        assert_eq!(s.hero_gold, 70);
        assert!(s.vendor.stock.iter().all(|e| e.item.code != "hp1"), "retiré du rayon");
        assert!(s.vendor.gold > gold_vendor0, "le marchand encaisse");
        // Vente : valeur/4 — un Cap vaut 64 ⇒ 16.
        let mut cap = Item { code: "cap".into(), ..Default::default() };
        cap.base.name = "Cap".into();
        cap.base.wtype = "helm".into();
        let got = s.sell(&c, &cap).expect("vente cap");
        assert_eq!(got, 16, "cap 64 ⇒ vente 16");
        assert_eq!(s.hero_gold, 70 + 16);
        assert!(s.vendor.stock.iter().any(|e| e.item.code == "cap"), "revendable");
    }

    /// Garde-fous : or insuffisant ⇒ NotEnoughGold (rien ne bouge) ; réserve
    /// du vendeur dépassée ⇒ VendorCantAfford (Gheed 350-750 refuse un gros
    /// lot : stu 2385 ⇒ vente 598) ; index invalide ⇒ BadIndex ; code
    /// inconnu ⇒ UnknownItem.
    #[test]
    fn trade_guards() {
        let c = cat();
        let gheed = Vendor::generate(&c, VendorId::Gheed, 1, 9);
        assert!((350..=750).contains(&gheed.gold));
        let mut s = TradeSession::new(gheed, 0);
        let n0 = s.vendor.stock.len();
        match s.buy(0) {
            Err(TradeError::NotEnoughGold { price, gold }) => {
                assert!(price > 0 && gold == 0);
            }
            other => panic!("achat sans or : {other:?}"),
        }
        assert_eq!(s.vendor.stock.len(), n0, "rayon intact après refus");
        // Réserve : stu (2385) se vend 598 > réserve Gheed ≤ 750 ⇒ refus.
        let mut stu = Item { code: "stu".into(), ..Default::default() };
        stu.base.name = "Studded Leather".into();
        stu.base.wtype = "tors".into();
        assert_eq!(c.cost_of("stu"), Some(2385), "stu : valeur réelle");
        match s.sell(&c, &stu) {
            Err(TradeError::VendorCantAfford { price, gold }) => {
                assert_eq!(price, 2385 / 4);
                assert!((350..=750).contains(&gold));
            }
            other => panic!("réserve Gheed dépassée : {other:?}"),
        }
        assert!(matches!(s.buy(999), Err(TradeError::BadIndex(999))));
        let ghost = Item { code: "zzz".into(), ..Default::default() };
        assert!(matches!(s.sell(&c, &ghost), Err(TradeError::UnknownItem(_))));
    }

    // -- Gamble ---------------------------------------------------------------

    /// Pool respecte « spawnable && qlvl ≤ niveau vendeur » : au niveau 1
    /// aucune base de qlvl > 1 (hax lvl 3 absent) ; au niveau 8, hax entre
    /// dans le pool. Prix = gamble cost × (100 + 2·clvl)/100, non identifié.
    #[test]
    fn gamble_pool_respects_vendor_level() {
        let c = cat();
        let low = gamble_list(&c, 1, 1, 7);
        let high = gamble_list(&c, 8, 8, 7);
        assert!(!low.is_empty() && !high.is_empty());
        assert_eq!(low.len(), 12, "page de gamble MVP : 12 slots");
        for e in &low {
            let b = c.base(&e.item.code).unwrap();
            assert!(b.level <= 1, "{} qlvl {} > 1", e.item.code, b.level);
            assert!(!e.item.identified, "gamble = non identifié");
            assert_eq!(e.buy, b.gamble_cost * 102 / 100, "clvl 1");
        }
        assert_eq!(c.base("hax").unwrap().level, 3);
        assert!(low.iter().all(|e| e.item.code != "hax"), "hax hors pool niveau 1");
        assert!(high.iter().any(|e| e.item.code == "hax"), "hax dans le pool niveau 8");
        for e in &high {
            let b = c.base(&e.item.code).unwrap();
            assert_eq!(e.buy, b.gamble_cost * 116 / 100, "clvl 8");
        }
    }

    /// Les affixes de gamble sont ROULÉS À L'ILVL VENDEUR : alvl =
    /// max(qlvl, ilvl) (unité) ; sur 300 listes au niveau 20, magiques et
    /// rares apparaissent, chaque affixe roulé a un `level` de table ≤ alvl.
    #[test]
    fn gamble_rolls_affixes_at_vendor_ilvl() {
        let c = cat();
        assert_eq!(affixes::alvl(1, 20), 20, "alvl = max(qlvl, ilvl)");
        assert_eq!(affixes::alvl(25, 20), 25);
        let aff = c.affixes.as_ref().expect("tables d'affixes");
        let mut seen_magic = 0;
        let mut seen_rare = 0;
        let mut checked = 0;
        for seed in 0..300u64 {
            for e in gamble_list(&c, 20, 20, seed) {
                match e.item.quality {
                    Quality::Magic => {
                        seen_magic += 1;
                        assert!(e.item.prefixes.len() <= 1 && e.item.suffixes.len() <= 1);
                        // ≥1 affixe nominatif (certains mods restent hors
                        // couverture descripteurs — écart item_props, restant).
                        assert!(!e.item.prefixes.is_empty() || !e.item.suffixes.is_empty());
                        let alvl = affixes::alvl(c.base(&e.item.code).unwrap().level, 20);
                        // Chaque affixe roulé existe dans les tables à un
                        // niveau ≤ alvl (les tables v1.12 portent des
                        // HOMONYMES à niveaux différents, ex. « Sounding »
                        // 20/50 — on cherche un record atteignable).
                        for name in e.item.prefixes.iter().chain(e.item.suffixes.iter()) {
                            let ok = aff
                                .prefixes
                                .iter()
                                .chain(aff.suffixes.iter())
                                .filter(|a| &a.name == name)
                                .any(|a| a.level <= alvl);
                            assert!(ok, "{name} introuvable à alvl {alvl}");
                            checked += 1;
                        }
                    }
                    Quality::Rare => {
                        seen_rare += 1;
                        assert!(e.item.props.len() >= 2, "rare : ≥2 affixes");
                    }
                    Quality::Unique => {
                        assert!(e.item.unique.is_some(), "unique gamble résolu");
                        assert!(!e.item.props.is_empty());
                    }
                    Quality::Set => {
                        assert!(e.item.set_item.is_some(), "set gamble résolu");
                    }
                    _ => {}
                }
            }
        }
        assert!(seen_magic > 200, "magiques dominent : {seen_magic}/3600");
        assert!(seen_rare > 200, "rares attendus : {seen_rare}");
        assert!(checked > 100, "affixes validés : {checked}");
    }

    /// Déterminisme gamble : même seed ⇒ même liste (codes, prix, qualité).
    #[test]
    fn gamble_deterministic() {
        let c = cat();
        let a = gamble_list(&c, 8, 8, 0x0D2);
        let b = gamble_list(&c, 8, 8, 0x0D2);
        assert_eq!(a, b);
    }

    /// Invariant qualité : une entrée `Unique` porte toujours sa clé, une
    /// `Set` sa pièce (dégradation rare sinon) — 2500 tirages niveau 1.
    #[test]
    fn gamble_quality_invariants() {
        let c = cat();
        let mut magics = 0;
        for seed in 0..500u64 {
            for e in gamble_list(&c, 1, 1, seed) {
                match e.item.quality {
                    Quality::Unique => assert!(e.item.unique.is_some(), "unique sans clé"),
                    Quality::Set => assert!(e.item.set_item.is_some(), "set sans pièce"),
                    Quality::Magic => magics += 1,
                    _ => {}
                }
            }
        }
        assert!(magics > 0, "magiques attendus");
    }
}
