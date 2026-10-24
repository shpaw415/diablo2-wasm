//! `cube` — Étape 11a « cube horadrim » : parseur `cubemain.txt` v1.12 + moteur
//! de transmutation data-driven (campagne diablo-2-wasm, 2026-10-24).
//!
//! # Sources — LA TABLE FAIT FOI
//! * `assets/patch_cubemain.txt` (150 recettes réelles v1.12, embarquée) :
//!   colonnes `numinputs`, `input 1..7` (patterns), `output` (+ `output b/c`,
//!   ABSENTS du fichier réel — vérifié par golden), `lvl`/`plvl`/`ilvl`,
//!   `mod 1..5` (+ chance/param/min/max). LA TABLE FAIT FOI ; les guides
//!   publics (Nefarius « Cube_main.txt Guide », Phrozen Keep, Amazon Basin)
//!   sont cités pour la SÉMANTIQUE des patterns mais NON FIABLES — tout est
//!   arbitré par les données réelles embarquées.
//! * Groupes d'inputs dérivés des tables réelles (aucun hard-code) :
//!   - **code exact** : `gcv`, `r01`, `rvs`, `msf`, `fhl`… (cols `code` de
//!     `patch_misc.txt` / `patch_weapons.txt` / `patch_armor.txt`) ;
//!   - **type / type2** (famille de l'item) : `hpot` (hp1..hp5), `mpot`,
//!     `gem0`..`gem4` (grades des gemmes, type2 des gemmes réelles — gcv
//!     `type=gema type2=gem0`, skc `type=gemz type2=gem0` : les crânes SONT
//!     des `gem0`), `axe`, `knif`, `swor`, `shld`… (cols `type` de
//!     weapons/armor), `ring`/`amul` (types de `rin`/`amu`) ;
//!   - **familles `weap`/`armo`** : appartenance à `patch_weapons.txt` /
//!     `patch_armor.txt` (miroir `item_roll.rs`) ;
//!   - **`any`** : joker (recettes L62-65 réelles) ;
//!   - **nom d'unique** : token avec espaces (L64/L65 « The Stone of
//!     Jordan ») = correspondance exacte sur le nom résolu.
//! * Sémantique des flags d'input (docs NON FIABLES, arbitrés par les
//!   recettes réelles du fichier) : `qty=N` = N OBJETS distincts (une pile =
//!   1 objet), `mag`/`rar`/`uni`/`nor`/`low`/`hiq` = qualité, `bas`/`exc` =
//!   palier (normal/exceptionnel — dérivé des colonnes `ubercode`/`ultracode`
//!   de weapons/armor), `sock`/`nos` = sockets posés/absents, `noe` = non
//!   éthéré (éthéré non modélisé au MVP ⇒ toujours vrai), `upg` stocké non
//!   discriminant (crafts).
//!
//! # Moteur ([`HoradricCube::transmute`])
//! * **Ordre du fichier = priorité D2** : la PREMIÈRE recette compatible
//!   gagne (`patch_cubemain.txt` ligne 17 avant 149 — golden test).
//! * Appariement par slots avec retour-arrière (backtracking) : chaque slot
//!   consomme `qty` objets DISTINCTS ; les objets excédentaires dans le cube
//!   ne bloquent pas (fidèle D2) ; déterministe (candidats en ordre de liste).
//! * `numinputs` = total d'objets requis (cohérence Σ qty vérifiée sur les
//!   150 lignes réelles en golden).
//! * Sorties : code exact (`rvl`, `gfv`, `r02`… — champs de base tirés des
//!   tables), `usetype` (= code du 1er objet consommé du slot 1 — c'est
//!   l'item-template dans toutes les lignes `usetype`/`useitem` du fichier
//!   réel), `useitem` (l'objet lui-même : qualité/nom conservés),
//!   remap de palier `exc`/`eli` (code → `ubercode`/`ultracode` de sa ligne,
//!   L131-138), `pre=N`/`suf=N` (affixe magique posé par rang parsé — même
//!   encodage que le contrat `RolledItem::affix_ids`, L8-12/L19-20), portails
//!   (« Cow Portal » L4, « Pandemonium Portal » L150 — non-item, UI restant).
//! * Mods de sortie appliqués : `sock` (alvéoles roulées [min,max], L17),
//!   `levelreq` (+N, L131-138) ; les AUTRES codes de mods (`thorns`,
//!   `lifesteal`… crafts L66-101) sont parsés, stockés, NON appliqués
//!   (restant). Colonnes `lvl`/`plvl`/`ilvl` stockées, non appliquées
//!   (`RolledItem` sans ilvl — restant).
//! * Non-recette ⇒ `None`, AUCUN tirage RNG consommé (no-op propre).
//!
//! # Divergences documentées (annoncées, additives)
//! * `qty` compte des OBJETS, pas des tailles de pile (2 faisceaux `aqv` = 2
//!   objets — L23).
//! * Groupes `blun` (L74) et `rod` (L92) : absents des tables embarquées
//!   (types réels des masses = club/mace/hamm ; `itemtypes.txt` non embarqué)
//!   ⇒ ces 2 recettes ne matchent jamais au MVP (restant).
//! * Recettes à input `tmp` (L148) et sorties `tmp` (L144-147) : non
//!   supportées, jamais appariées (restant) — 145/150 recettes supportées
//!   (golden).
//! * Qualité magic des sorties `usetype,mag` : affixes NON roulés (nom =
//!   nom de base ; seuls les `pre=`/`suf=` explicites posent des affixes) —
//!   restant, comme les crafts.

#![allow(dead_code)] // contrat consommé par d2game à l'intégration (pattern item_roll)

use std::collections::HashMap;

use crate::affixes::{self, AffixTables};
use crate::item_props::{roll as roll_prop, PropDescriptor};
use crate::item_roll::{resolve_affix, RolledItem, RolledQuality, SUFFIX_ID_OFFSET};
use crate::rng::Rng;
use crate::tsv::Tsv;

/// cubemain.txt v1.12 — LA source fait foi (golden test ligne à ligne).
const CUBEMAIN_TXT: &str = include_str!("../../../../assets/patch_cubemain.txt");
/// Armes/armures/misc v1.12 : codes, types, champs de base, uber/ultra.
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");
/// MagicPrefix/Suffix + AutoMagic (résolution des `pre=`/`suf=` par rang).
const MAGIC_PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
const MAGIC_SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");

// ---------------------------------------------------------------------------
// Item du cube (vue légère, découplée de World/inventaire)
// ---------------------------------------------------------------------------

/// Un objet posé dans le cube (1 objet = 1 [`CubeItem`], pile comprise).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CubeItem {
    /// Code table D2 (`hax`, `gcv`, `rvs`, `rin`…).
    pub code: String,
    /// Nom résolu (clé exacte d'unique pour « The Stone of Jordan », L64).
    pub name: String,
    /// Qualité (flags `mag`/`rar`/`uni`/`nor`/`low`/`hiq` des patterns).
    pub quality: RolledQuality,
    /// Alvéoles posées (flag `sock`/`nos`).
    pub sockets: u8,
}

impl CubeItem {
    /// Item simple de qualité Normal (gemmes, runes, potions…).
    pub fn base(code: &str, name: &str) -> CubeItem {
        CubeItem {
            code: code.to_string(),
            name: name.to_string(),
            quality: RolledQuality::Normal,
            sockets: 0,
        }
    }
}

impl From<&RolledItem> for CubeItem {
    fn from(r: &RolledItem) -> CubeItem {
        CubeItem {
            code: r.code.clone(),
            name: r.name_resolved.clone(),
            quality: r.quality,
            sockets: r.sockets,
        }
    }
}

// ---------------------------------------------------------------------------
// Patterns d'inputs typés
// ---------------------------------------------------------------------------

/// Qualité exigée par un pattern d'input (docs cubemain NON FIABLES, arbitrés
/// par les recettes réelles — cf. doc module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputQuality {
    /// `mag` (ex. L15 `rin,mag`).
    Mag,
    /// `rar` (L63 `any,rar`).
    Rar,
    /// `uni` (L131 `weap,bas,uni`).
    Uni,
    /// `nor` (L125 `tors,nor,nos`).
    Nor,
    /// `low` (L129 `weap,low`).
    Low,
    /// `hiq` = supérieur (L144 `armo,hiq,nos`).
    Hiq,
}

/// Palier exigé (`bas` = normal, `exc` = exceptionnel — via
/// `ubercode`/`ultracode` des tables réelles ; l'élite n'est jamais exigée
/// en input dans le fichier réel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Palier normal (`bas`).
    Normal,
    /// Palier exceptionnel (`exc`).
    Exceptional,
    /// Palier élite (sorties `eli` uniquement).
    Elite,
}

/// Exigence de sockets d'un pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SockReq {
    /// Sans exigence (défaut).
    #[default]
    Any,
    /// `sock` : au moins une alvéole (L17 `weap,sock`).
    Socked,
    /// `nos` : aucune alvéole (L125 `tors,nor,nos`).
    Unsocked,
}

/// Un slot d'input typé (`patch_cubemain.txt` cols `input 1..7`).
/// Ex. réel L25 : `"gcv,qty=3"` → code `gcv`, qty 3 ; L17 : `"weap,sock"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubeInput {
    /// Code exact, groupe (type/type2/weap/armo), `any`, ou nom d'unique.
    pub code: String,
    /// Nombre d'objets DISTINCTS consommés (`qty=N`, défaut 1).
    pub qty: u32,
    /// Qualité exigée.
    pub quality: Option<InputQuality>,
    /// Palier exigé (`bas`/`exc`).
    pub tier: Option<Tier>,
    /// Exigence de sockets (`sock`/`nos`).
    pub sock: SockReq,
    /// `noe` : non éthéré — éthéré non modélisé au MVP ⇒ toujours vrai.
    pub noe: bool,
    /// `upg` stocké non discriminant (crafts L66-101).
    pub upg: bool,
    /// `false` si un token du pattern n'est pas supporté au MVP (ex. `tmp`,
    /// L148) — la recette n'est JAMAIS appariée (jamais de faux positif).
    pub supported: bool,
}

// ---------------------------------------------------------------------------
// Sorties typées
// ---------------------------------------------------------------------------

/// Base d'une sortie (`output` col).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutKind {
    /// Code exact (`rvl`, `gfv`, `hst`…).
    Code,
    /// `usetype` : même code que le 1er objet consommé (slot 1 = template).
    UseType,
    /// `useitem` : l'objet lui-même (qualité/nom conservés).
    UseItem,
    /// Portail (« Cow Portal », « Pandemonium Portal ») — non-item.
    Portal,
}

/// Un mod de sortie typé (cols `mod N` + chance/param/min/max). Seuls
/// `sock` et `levelreq` sont appliqués au MVP (L17, L131-138) ; les autres
/// codes (crafts `thorns`/`lifesteal`…) sont stockés, restant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubeMod {
    /// Code du mod (`sock`, `levelreq`, `thorns`…).
    pub code: String,
    /// Colonne `mod N chance` — `None` = s'applique toujours (cas réel).
    pub chance: Option<i32>,
    /// Colonne `mod N param`.
    pub param: String,
    /// Colonne `mod N min`.
    pub min: i32,
    /// Colonne `mod N max`.
    pub max: i32,
}

/// Sortie typée d'une recette (col `output` + mods des colonnes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CubeOutput {
    /// Base de la sortie.
    pub kind: OutKind,
    /// Code (kind Code) ou texte du portail (kind Portal).
    pub code: String,
    /// Qualité imposée (`mag`/`rar`/`nor`/`crf`) ; `None` = Normal, sauf
    /// `useitem` qui conserve la qualité de l'objet.
    pub quality: Option<RolledQuality>,
    /// Remap de palier (`exc`/`eli` — L131-138 : code → uber/ultracode).
    pub tier_up: Option<Tier>,
    /// `pre=N` : id du prefix MagicPrefix (rang parsé, L8-12).
    pub pre: Option<u32>,
    /// `suf=N` : id du suffix MagicSuffix (rang parsé — L19/20).
    pub suf: Option<u32>,
    /// Mods de sortie (voir [`CubeMod`]).
    pub mods: Vec<CubeMod>,
    /// `false` = sortie non supportée au MVP (ex. `tmp`, L144-147).
    pub supported: bool,
}

/// Une recette typée de `patch_cubemain.txt` (ordre du fichier = priorité).
#[derive(Debug, Clone, PartialEq)]
pub struct CubeRecipe {
    /// Ligne 1-based dans le fichier réel (l'en-tête = ligne 1) — pour
    /// citer chaque recette en revue.
    pub line: usize,
    /// Colonne `description` (libellé de table, anglais v1.12).
    pub description: String,
    /// Colonne `enabled` (1 partout dans le fichier réel — vérifié).
    pub enabled: bool,
    /// Colonne `version` (0 partout).
    pub version: u32,
    /// Colonne `numinputs` = total d'objets requis (2..=7 réels).
    pub numinputs: u32,
    /// Slots d'inputs parsés (`input 1..7` non vides, ordre du fichier).
    pub inputs: Vec<CubeInput>,
    /// Sortie principale (`output` + mods `mod 1..5`).
    pub output: CubeOutput,
    /// Sorties alternatives `output b`/`output c` — ABSENTES du fichier
    /// réel (vérifié) ; parsées par complétude, jamais roulées.
    pub output_b: Option<CubeOutput>,
    pub output_c: Option<CubeOutput>,
    /// Cols `lvl`/`plvl`/`ilvl` (stockées, non appliquées — restant ;
    /// ex. L15 : plvl 75).
    pub lvl: u32,
    pub plvl: u32,
    pub ilvl: u32,
    /// `false` = recette jamais appariée au MVP (input ou output non
    /// supporté — cf. doc module).
    pub supported: bool,
}

// ---------------------------------------------------------------------------
// Tables du cube (codes/types/paliers/affixes — le tout dérivé des données)
// ---------------------------------------------------------------------------

/// Famille d'une base (appartenance table réelle).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Family {
    /// Ligne de `patch_weapons.txt`.
    Weapon,
    /// Ligne de `patch_armor.txt`.
    Armor,
    /// Ligne de `patch_misc.txt`.
    Misc,
}

/// Champs de base d'un code (weapons/armor/misc réels).
#[derive(Debug, Clone)]
pub(crate) struct BaseRow {
    /// Col `name`.
    pub name: String,
    /// Table d'origine.
    pub family: Family,
    /// Col `type` (famille : axe, knif, hpot, gema, rpot…).
    pub itype: String,
    /// Col `type2` (grades de gemmes gem0..gem4).
    pub itype2: String,
    /// Cols `mindam`/`maxdam` (armes).
    pub min_dmg: u32,
    pub max_dmg: u32,
    /// Cols `minac`/`maxac` (armures).
    pub min_ac: u32,
    pub max_ac: u32,
    /// Cols `reqstr`/`reqdex`/`levelreq`.
    pub req_str: u32,
    pub req_dex: u32,
    pub req_level: u32,
    /// Cols `ubercode`/`ultracode` (remap exc/eli, L131-138).
    pub uber: String,
    pub ultra: String,
}

/// Tables dérivées des données réelles (jamais paniquer — accès lenient).
pub(crate) struct CubeTables {
    /// Code → ligne de base (weapons/armor/misc fusionnés).
    pub(crate) bases: HashMap<String, BaseRow>,
    /// Code → palier (membre d'un `ultracode` = élite, d'un `ubercode` =
    /// exceptionnel, sinon normal — dérivé des 2 tables).
    pub(crate) tiers: HashMap<String, Tier>,
    /// MagicPrefix/Suffix parsés (`pre=`/`suf=` par rang) — `None` ⇒ id posé
    /// sans affixe (jamais de panique).
    pub(crate) affixes: Option<AffixTables>,
}

impl CubeTables {
    /// Parse les 4 tables embarquées (lenient : une table illisible ⇒
    /// entrées absentes, jamais de panique).
    pub(crate) fn embedded() -> CubeTables {
        let mut bases = HashMap::new();
        for (txt, family) in
            [(WEAPONS_TXT, Family::Weapon), (ARMOR_TXT, Family::Armor), (MISC_TXT, Family::Misc)]
        {
            let Ok(t) = Tsv::parse(txt) else { continue };
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                if r.s("code").is_empty() {
                    continue;
                }
                bases.entry(r.s("code").to_string()).or_insert_with(|| BaseRow {
                    name: r.s("name").to_string(),
                    family,
                    itype: r.s("type").to_string(),
                    itype2: r.s("type2").to_string(),
                    min_dmg: r.u("mindam"),
                    max_dmg: r.u("maxdam"),
                    min_ac: r.u("minac"),
                    max_ac: r.u("maxac"),
                    req_str: r.u("reqstr"),
                    req_dex: r.u("reqdex"),
                    req_level: r.u("levelreq"),
                    uber: r.s("ubercode").to_string(),
                    ultra: r.s("ultracode").to_string(),
                });
            }
        }
        // Paliers : un code qui apparaît comme `ubercode`/`ultracode` d'une
        // ligne existante est exceptionnel/élite (hax uber=9ha ultra=7ha,
        // données réelles). L'élite prime (and_modify).
        let mut tiers: HashMap<String, Tier> = HashMap::new();
        for row in bases.values() {
            for (code, tier) in [(&row.uber, Tier::Exceptional), (&row.ultra, Tier::Elite)] {
                if !code.is_empty() && bases.contains_key(code.as_str()) {
                    tiers
                        .entry(code.clone())
                        .and_modify(|t| *t = Tier::Elite)
                        .or_insert(tier);
                }
            }
        }
        CubeTables {
            bases,
            tiers,
            affixes: affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT),
        }
    }

    /// Palier d'un code (dérivé des tables ; inconnu ⇒ normal).
    pub(crate) fn tier_of(&self, code: &str) -> Tier {
        self.tiers.get(code).copied().unwrap_or(Tier::Normal)
    }

    /// Un token de pattern matche-t-il le code d'un objet ? Code exact,
    /// type/type2 de sa base, familles weap/armo, joker `any`.
    pub(crate) fn code_matches(&self, token: &str, item_code: &str) -> bool {
        if token == item_code || token == "any" {
            return true;
        }
        let Some(b) = self.bases.get(item_code) else { return false };
        b.itype == token
            || b.itype2 == token
            || (token == "weap" && b.family == Family::Weapon)
            || (token == "armo" && b.family == Family::Armor)
    }
}

// ---------------------------------------------------------------------------
// Cube — parse + moteur
// ---------------------------------------------------------------------------

/// Le cube horadrim : 150 recettes réelles v1.12 + tables dérivées.
/// Ordre du fichier = priorité D2 (premier match gagne).
pub struct HoradricCube {
    recipes: Vec<CubeRecipe>,
    tables: CubeTables,
}

/// Résultat d'une transmutation : recette citée, objets consommés, produit.
#[derive(Debug, Clone, PartialEq)]
pub struct TransmuteOutcome {
    /// Ligne 1-based de la recette dans `patch_cubemain.txt`.
    pub line: usize,
    /// Indices (dans la liste passée à [`HoradricCube::transmute`]) des
    /// objets consommés, slot par slot (slot 1 d'abord).
    pub consumed: Vec<usize>,
    /// Objet produit (`None` pour les sorties portail).
    pub item: Option<RolledItem>,
    /// Portail ouvert (L4/L150/L151) — non-item, UI restant.
    pub portal: Option<String>,
}

impl HoradricCube {
    /// Parse `patch_cubemain.txt` embarqué + tables dérivées ; `None` si la
    /// table est illisible (jamais paniquer — règle mission).
    pub fn embedded() -> Option<HoradricCube> {
        let recipes = parse(CUBEMAIN_TXT).ok()?;
        Some(HoradricCube { recipes, tables: CubeTables::embedded() })
    }

    /// Les recettes parsées (ordre du fichier = priorité).
    pub fn recipes(&self) -> &[CubeRecipe] {
        &self.recipes
    }

    /// Tente UNE transmutation sur la liste d'objets : première recette
    /// compatible (ordre du fichier), objets consommés, produit roulé.
    /// Non-recette ⇒ `None` SANS consommer de RNG (no-op propre).
    pub fn transmute(&self, items: &[CubeItem], rng: &mut Rng) -> Option<TransmuteOutcome> {
        for rec in &self.recipes {
            if let Some(consumed) = self.try_match(rec, items) {
                return Some(self.produce(rec, items, &consumed, rng));
            }
        }
        None
    }

    /// Appariement d'une recette : slots en ordre, backtracking
    /// déterministe (candidats en ordre croissant d'indices).
    fn try_match(&self, rec: &CubeRecipe, items: &[CubeItem]) -> Option<Vec<usize>> {
        if !rec.enabled || !rec.supported {
            return None;
        }
        let total: u32 = rec.inputs.iter().map(|i| i.qty.max(1)).sum();
        if rec.numinputs > 0 && (total != rec.numinputs || (items.len() as u32) < total) {
            return None; // Σ qty ≠ numinputs (jamais vrai sur la table) ou pas assez d'objets
        }
        let mut used = vec![false; items.len()];
        self.match_slots(rec, 0, items, &mut used, &mut Vec::new())
    }

    fn match_slots(
        &self,
        rec: &CubeRecipe,
        k: usize,
        items: &[CubeItem],
        used: &mut [bool],
        acc: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        let Some(pat) = rec.inputs.get(k) else {
            return Some(acc.clone()); // tous les slots remplis
        };
        let need = pat.qty.max(1) as usize;
        let cand: Vec<usize> = (0..items.len())
            .filter(|&i| !used[i] && self.item_matches(pat, &items[i]))
            .collect();
        let mut cur = Vec::with_capacity(need);
        self.choose_combo(rec, k, &cand, 0, need, items, used, acc, &mut cur)
    }

    /// Énumère les combinaisons de `need` candidats (ordre lexicographique)
    /// et récursion sur le slot suivant.
    #[allow(clippy::too_many_arguments)]
    fn choose_combo(
        &self,
        rec: &CubeRecipe,
        k: usize,
        cand: &[usize],
        start: usize,
        need: usize,
        items: &[CubeItem],
        used: &mut [bool],
        acc: &mut Vec<usize>,
        cur: &mut Vec<usize>,
    ) -> Option<Vec<usize>> {
        if cur.len() == need {
            for &i in cur.iter() {
                used[i] = true;
                acc.push(i);
            }
            if let Some(done) = self.match_slots(rec, k + 1, items, used, acc) {
                return Some(done);
            }
            for _ in 0..need {
                let i = acc.pop().expect("pushé ci-dessus");
                used[i] = false;
            }
            return None;
        }
        for j in start..cand.len() {
            cur.push(cand[j]);
            if let Some(done) = self.choose_combo(rec, k, cand, j + 1, need, items, used, acc, cur)
            {
                return Some(done);
            }
            cur.pop();
        }
        None
    }

    /// Un objet matche-t-il un pattern ? (code/groupe, qualité, palier,
    /// sockets — cf. doc module pour la sémantique arbitée par les données)
    fn item_matches(&self, pat: &CubeInput, it: &CubeItem) -> bool {
        if !pat.supported {
            return false;
        }
        // Code exact / groupe / joker / nom d'unique (token avec espaces).
        if pat.code.contains(' ') {
            if it.name != pat.code {
                return false;
            }
        } else if !self.tables.code_matches(&pat.code, &it.code) {
            return false;
        }
        // Qualité.
        let ok_quality = match pat.quality {
            None => true,
            Some(InputQuality::Mag) => it.quality == RolledQuality::Magic,
            Some(InputQuality::Rar) => it.quality == RolledQuality::Rare,
            Some(InputQuality::Uni) => it.quality == RolledQuality::Unique,
            Some(InputQuality::Nor) => it.quality == RolledQuality::Normal,
            Some(InputQuality::Low) => matches!(it.quality, RolledQuality::Low(_)),
            Some(InputQuality::Hiq) => it.quality == RolledQuality::Superior,
        };
        if !ok_quality {
            return false;
        }
        // Palier (bas/exc — dérivé des colonnes uber/ultracode réelles).
        if let Some(tier) = pat.tier {
            if self.tables.tier_of(&it.code) != tier {
                return false;
            }
        }
        // Sockets.
        match pat.sock {
            SockReq::Socked if it.sockets == 0 => return false,
            SockReq::Unsocked if it.sockets > 0 => return false,
            _ => {}
        }
        // `noe` : éthéré non modélisé ⇒ toujours vrai ; `upg` : non
        // discriminant (crafts). Cf. doc module.
        true
    }

    /// Produit la sortie d'une recette appariée (base via les tables
    /// réelles, qualité par flags/mods, affixes `pre`/`suf` roulés).
    fn produce(
        &self,
        rec: &CubeRecipe,
        items: &[CubeItem],
        consumed: &[usize],
        rng: &mut Rng,
    ) -> TransmuteOutcome {
        let out = &rec.output;
        // Portails (L4/L150/L151) : non-item, l'UI d2game les branche.
        if out.kind == OutKind::Portal {
            return TransmuteOutcome {
                line: rec.line,
                consumed: consumed.to_vec(),
                item: None,
                portal: Some(out.code.clone()),
            };
        }
        let template = &items[consumed[0]]; // slot 1 = item-template (données réelles)
        // Base/qualité/nom/nom selon le kind (cf. doc module).
        let (mut code, quality, mut name) = match out.kind {
            OutKind::UseItem => {
                // L'objet lui-même : code/qualité/nom conservés (le remap
                // de palier et les mods s'appliquent ensuite).
                (template.code.clone(), template.quality, template.name.clone())
            }
            OutKind::UseType => {
                // Même base que le template, fraîche (qualité par flags).
                (template.code.clone(), out.quality.unwrap_or(RolledQuality::Normal), String::new())
            }
            _ => {
                // Code exact (rvl, gfv, r02, tax…).
                (out.code.clone(), out.quality.unwrap_or(RolledQuality::Normal), String::new())
            }
        };
        let mut sockets = if out.kind == OutKind::UseItem { template.sockets } else { 0 };
        // Remap de palier exc/eli (L131-138) : code → uber/ultracode.
        if let Some(tier) = out.tier_up {
            if let Some(b) = self.tables.bases.get(&code) {
                let target = match tier {
                    Tier::Exceptional => &b.uber,
                    _ => &b.ultra,
                };
                if !target.is_empty() {
                    code = target.clone();
                }
            }
        }
        // Champs de base depuis les tables réelles : armes = dégâts,
        // armures = AC roulé, misc = zéros ; requis de la ligne. Pour
        // `useitem`, le nom résolu est conservé (nom rare restant en D2),
        // les champs de base suivent le NOUVEAU code (fidèle D2 : la base
        // exceptionnelle remplace les dégâts/requis).
        let mut min_dmg = 0;
        let mut max_dmg = 0;
        let mut ac = 0;
        let mut req_str = 0;
        let mut req_dex = 0;
        let mut req_level = 0;
        if let Some(b) = self.tables.bases.get(&code) {
            if out.kind != OutKind::UseItem {
                name = b.name.clone();
            }
            req_str = b.req_str;
            req_dex = b.req_dex;
            req_level = b.req_level;
            match b.family {
                Family::Weapon => {
                    min_dmg = b.min_dmg;
                    max_dmg = b.max_dmg;
                }
                Family::Armor => {
                    ac = if b.max_ac > 0 { rng.range_u32(b.min_ac, b.max_ac) } else { 0 };
                }
                Family::Misc => {}
            }
        }
        // Mods de sortie appliqués : sock (alvéoles) + levelreq (+N) ;
        // les autres codes sont stockés dans la recette (restant).
        for m in &out.mods {
            if let Some(chance) = m.chance {
                if chance < 100 && rng.below(100) >= chance.max(0) as u32 {
                    continue;
                }
            }
            match m.code.as_str() {
                "sock" => {
                    let lo = m.min.max(0) as u32;
                    let hi = m.max.max(0) as u32;
                    sockets =
                        sockets.saturating_add(rng.range_u32(lo, hi).min(u8::MAX as u32) as u8);
                }
                "uns" => sockets = 0,
                "levelreq" => {
                    let v = rng.range_i32(m.min.min(m.max), m.max.max(m.min)).max(0) as u32;
                    req_level = req_level.saturating_add(v);
                }
                _ => {} // thorns, lifesteal… (crafts L66-101) — restant
            }
        }
        // Affixes posés pre=/suf= (L8-12 / L19-20) : id par rang parsé
        // (encodage du contrat `RolledItem::affix_ids`), valeur roulée dans
        // les bornes de la ligne ; nom composé `<Prefix> <Base> <Suffix>`.
        let mut affix_ids: Vec<(u32, u32)> = Vec::new();
        if let (Some(pre), Some(t)) = (out.pre, &self.tables.affixes) {
            if let Some((rec_a, _)) = resolve_affix(t, pre) {
                for d in &rec_a.mods {
                    let v = roll_prop(d, rng).value.max(0) as u32;
                    let param: u32 = d.param.trim().parse().unwrap_or(0);
                    if d.code == "sock" {
                        let add = if param > 0 { param } else { v };
                        sockets = sockets.saturating_add(add.min(u8::MAX as u32) as u8);
                    }
                    affix_ids.push((pre, v));
                }
                name = format!("{} {}", rec_a.name, name);
            } else {
                affix_ids.push((pre, 0)); // id introuvable : posé brut (donnée > panique)
            }
        }
        if let (Some(suf), Some(t)) = (out.suf, &self.tables.affixes) {
            let id = suf + SUFFIX_ID_OFFSET;
            if let Some((rec_a, _)) = resolve_affix(t, id) {
                for d in &rec_a.mods {
                    let v = roll_prop(d, rng).value.max(0) as u32;
                    affix_ids.push((id, v));
                }
                name = format!("{} {}", name, rec_a.name);
            } else {
                affix_ids.push((id, 0));
            }
        }
        TransmuteOutcome {
            line: rec.line,
            consumed: consumed.to_vec(),
            item: Some(RolledItem {
                code,
                quality,
                name_resolved: name,
                affix_ids,
                sockets,
                gold: 0,
                min_dmg,
                max_dmg,
                ac,
                req_str,
                req_dex,
                req_level,
            }),
            portal: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Parseur cubemain (via tsv.rs — recettes typées)
// ---------------------------------------------------------------------------

/// Parse le texte cubemain complet en recettes typées. L'ordre du fichier
/// est conservé (priorité D2). Lenient : ligne illisible ⇒ ignorée (jamais
/// paniquer) ; le golden verrouille 150 recettes réelles parsées.
pub fn parse(input: &str) -> Result<Vec<CubeRecipe>, crate::tsv::TsvError> {
    let t = Tsv::parse(input)?;
    let mut recipes = Vec::new();
    for i in 0..t.len() {
        let Some(r) = t.rec(i) else { continue };
        let inputs: Vec<CubeInput> = (1..=7)
            .filter_map(|k| {
                let cell = r.s(&format!("input {k}"));
                (!cell.is_empty()).then(|| parse_input(cell))
            })
            .collect();
        let mut output = parse_output(r.s("output"));
        output.mods.extend(parse_mods(&t, i));
        let output_b = (!r.s("output b").is_empty()).then(|| parse_output(r.s("output b")));
        let output_c = (!r.s("output c").is_empty()).then(|| parse_output(r.s("output c")));
        let supported = inputs.iter().all(|p| p.supported) && output.supported;
        recipes.push(CubeRecipe {
            line: i + 2, // en-tête = ligne 1 ; recettes dès la ligne 2
            description: r.s("description").to_string(),
            enabled: r.u("enabled") != 0,
            version: r.u("version"),
            numinputs: r.u("numinputs"),
            inputs,
            output,
            output_b,
            output_c,
            lvl: r.u("lvl"),
            plvl: r.u("plvl"),
            ilvl: r.u("ilvl"),
            supported,
        });
    }
    Ok(recipes)
}

/// Parse un pattern d'input (`"gcv,qty=3"`, `"weap,sock"`, `gem0`,
/// `"amu,mag,pre=331"` — cells déjà déquotées par [`Tsv`]).
fn parse_input(cell: &str) -> CubeInput {
    let mut p = CubeInput {
        code: String::new(),
        qty: 1,
        quality: None,
        tier: None,
        sock: SockReq::Any,
        noe: false,
        upg: false,
        supported: true,
    };
    for (k, tok) in cell.split(',').enumerate() {
        let tok = tok.trim();
        if k == 0 {
            p.code = tok.to_string();
            continue;
        }
        if let Some(q) = tok.strip_prefix("qty=") {
            p.qty = q.trim().parse().unwrap_or(1).max(1);
        } else {
            match tok {
                "mag" => p.quality = Some(InputQuality::Mag),
                "rar" => p.quality = Some(InputQuality::Rar),
                "uni" => p.quality = Some(InputQuality::Uni),
                "nor" => p.quality = Some(InputQuality::Nor),
                "low" => p.quality = Some(InputQuality::Low),
                "hiq" => p.quality = Some(InputQuality::Hiq),
                "bas" => p.tier = Some(Tier::Normal),
                "exc" => p.tier = Some(Tier::Exceptional),
                "sock" => p.sock = SockReq::Socked,
                "nos" => p.sock = SockReq::Unsocked,
                "noe" => p.noe = true,
                "upg" => p.upg = true,
                _ => p.supported = false, // ex. tmp (L148) — jamais apparié
            }
        }
    }
    p
}

/// Parse une sortie (`rvl`, `"amu,mag,pre=331"`, `"usetype,mag"`,
/// `"useitem,mod,exc"`, « Cow Portal »…). Les mods des colonnes `mod 1..5`
/// sont ajoutés ensuite par [`parse`].
fn parse_output(cell: &str) -> CubeOutput {
    let mut o = CubeOutput {
        kind: OutKind::Code,
        code: String::new(),
        quality: None,
        tier_up: None,
        pre: None,
        suf: None,
        mods: Vec::new(),
        supported: true,
    };
    let mut toks = cell.split(',').map(|t| t.trim());
    let first = toks.next().unwrap_or("");
    if first == "usetype" {
        o.kind = OutKind::UseType;
    } else if first == "useitem" {
        o.kind = OutKind::UseItem;
    } else if first.contains(' ') {
        // Portail (« Cow Portal »…) : non-item, texte conservé tel quel.
        o.kind = OutKind::Portal;
        o.code = first.to_string();
    } else {
        o.code = first.to_string();
    }
    for tok in toks {
        match tok {
            "mag" => o.quality = Some(RolledQuality::Magic),
            "rar" => o.quality = Some(RolledQuality::Rare),
            "nor" => o.quality = Some(RolledQuality::Normal),
            "crf" => o.quality = Some(RolledQuality::Crafted),
            "exc" => o.tier_up = Some(Tier::Exceptional),
            "eli" => o.tier_up = Some(Tier::Elite),
            "uns" => o.mods.push(CubeMod {
                code: "uns".into(),
                chance: None,
                param: String::new(),
                min: 0,
                max: 0,
            }),
            // `mod` = marqueur « ajoute les mods à l'item » (L131-138) :
            // transparent, les mods viennent des colonnes.
            "mod" => {}
            // `rep`/`rch`/`qty=255` : réparation/recharge — non modélisé.
            "rep" | "rch" => {}
            t if t.starts_with("qty=") => {}
            "tmp" => o.supported = false, // L144-147 — restant
            t if t.starts_with("pre=") => o.pre = t[4..].parse().ok(),
            t if t.starts_with("suf=") => o.suf = t[4..].parse().ok(),
            t if t.starts_with("sock=") => {
                let n = t[5..].parse().unwrap_or(1);
                o.mods.push(CubeMod {
                    code: "sock".into(),
                    chance: None,
                    param: String::new(),
                    min: n,
                    max: n,
                });
            }
            _ => o.supported = false,
        }
    }
    o
}

/// Parse les mods de sortie d'une ligne (cols `mod N` ×5 + chance/param/min/max).
fn parse_mods(t: &Tsv, i: usize) -> Vec<CubeMod> {
    let Some(r) = t.rec(i) else { return Vec::new() };
    (1..=5)
        .filter_map(|m| {
            let code = r.s(&format!("mod {m}"));
            (!code.is_empty()).then(|| CubeMod {
                code: code.to_string(),
                chance: r.s(&format!("mod {m} chance")).parse().ok(),
                param: r.s(&format!("mod {m} param")).to_string(),
                min: r.i(&format!("mod {m} min")),
                max: r.i(&format!("mod {m} max")),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Golden tests — recettes réelles citées ligne à ligne, bout en bout
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> HoradricCube {
        HoradricCube::embedded().expect("cubemain embarqué parsable")
    }

    // ══ Parseur ══

    /// GOLDEN — les 150 recettes réelles : ordre fichier, enabled/version,
    /// cohérence numinputs = Σ qty, 145/150 supportées (5 `tmp` : L144-148),
    /// lignes citées (L5 potions, L15 anneaux, L102 rune+gemme, L135
    /// upgrade, L150 portail pandémonium).
    #[test]
    fn parser_real_cubemain_150_recipes_golden() {
        let c = cube();
        assert_eq!(c.recipes().len(), 150, "150 recettes réelles v1.12");
        // Cellule `enabled` réelle : '1' partout SAUF L144-148 (recettes
        // `tmp` — cellule VIDE dans le fichier = désactivée ; parseur
        // lenient "" ⇒ 0 ⇒ off). Zéro enabled=0 explicite dans la table.
        let disabled: Vec<usize> =
            c.recipes().iter().filter(|r| !r.enabled).map(|r| r.line).collect();
        assert_eq!(disabled, vec![144, 145, 146, 147, 148], "seules les tmp sont désactivées");
        for r in c.recipes() {
            assert_eq!(r.enabled, !matches!(r.line, 144..=148), "L{} : enabled", r.line);
            // Colonne version réelle : 0 (55 recettes) ou 100 LoD-only
            // (95 recettes : gemmes/runes/crafts — L53/113/135/150 = 100).
            assert!(r.version == 0 || r.version == 100, "L{} : version", r.line);
            let total: u32 = r.inputs.iter().map(|i| i.qty.max(1)).sum();
            assert_eq!(total, r.numinputs, "L{} : Σ qty = numinputs", r.line);
            assert!(r.inputs.len() >= 1 && r.inputs.len() <= 7);
        }
        assert_eq!(c.recipes()[3].version, 0, "L5 classic");
        assert_eq!(c.recipes()[51].version, 100, "L53 runes = LoD-only");
        let supported = c.recipes().iter().filter(|r| r.supported).count();
        assert_eq!(supported, 145, "5 recettes tmp non supportées (L144-148)");
        let unsupported: Vec<usize> =
            c.recipes().iter().filter(|r| !r.supported).map(|r| r.line).collect();
        assert_eq!(unsupported, vec![144, 145, 146, 147, 148]);
        // L5 : 3 hpot + 3 mpot + gem2 (gemme standard) → rvl (full rejuv).
        let l5 = &c.recipes()[3];
        assert_eq!(l5.line, 5);
        assert_eq!(l5.inputs[0].code, "hpot");
        assert_eq!(l5.inputs[0].qty, 3);
        assert_eq!(l5.inputs[2].code, "gem2");
        assert_eq!(l5.output.code, "rvl");
        assert_eq!(l5.numinputs, 7);
        // L15 : 3 anneaux magiques → 1 amulette magique (plvl 75).
        let l15 = &c.recipes()[13];
        assert_eq!(l15.line, 15);
        assert_eq!(l15.inputs[0].code, "rin");
        assert_eq!(l15.inputs[0].quality, Some(InputQuality::Mag));
        assert_eq!(l15.inputs[0].qty, 3);
        assert_eq!(l15.output.kind, OutKind::Code);
        assert_eq!(l15.output.code, "amu");
        assert_eq!(l15.output.quality, Some(RolledQuality::Magic));
        assert_eq!(l15.plvl, 75, "col plvl stockée (non appliquée)");
        // L102 : 3×Thul (r10) + 1 topaz ébréchée (gcy) → Amn (r11).
        let l102 = &c.recipes()[100];
        assert_eq!(l102.line, 102);
        assert_eq!(l102.inputs[0].code, "r10");
        assert_eq!(l102.inputs[0].qty, 3);
        assert_eq!(l102.inputs[1].code, "gcy");
        assert_eq!(l102.output.code, "r11");
        // L135 : arme rare de palier normal + Ort + Amn + saphir parfait
        // → useitem,mod,exc (upgrade exceptionnel, levelreq +5).
        let l135 = &c.recipes()[133];
        assert_eq!(l135.line, 135);
        assert_eq!(l135.inputs[0].code, "weap");
        assert_eq!(l135.inputs[0].tier, Some(Tier::Normal));
        assert_eq!(l135.inputs[0].quality, Some(InputQuality::Rar));
        assert_eq!(l135.output.kind, OutKind::UseItem);
        assert_eq!(l135.output.tier_up, Some(Tier::Exceptional));
        assert_eq!(l135.output.mods.len(), 1);
        assert_eq!(l135.output.mods[0].code, "levelreq");
        assert_eq!((l135.output.mods[0].min, l135.output.mods[0].max), (5, 5));
        // L17 : mods de colonnes parsés (sock 1..2 sur usetype,mag).
        let l17 = &c.recipes()[15];
        assert_eq!(l17.line, 17);
        assert_eq!(l17.output.mods.len(), 1);
        assert_eq!(l17.output.mods[0].code, "sock");
        assert_eq!((l17.output.mods[0].min, l17.output.mods[0].max), (1, 2));
        // L150 : pk1+pk2+pk3 → Pandemonium Portal (sortie non-item).
        let l150 = &c.recipes()[148];
        assert_eq!(l150.line, 150);
        assert_eq!(l150.output.kind, OutKind::Portal);
        assert_eq!(l150.output.code, "Pandemonium Portal");
        // output b/c absents du fichier réel (vérifié).
        assert!(c.recipes().iter().all(|r| r.output_b.is_none() && r.output_c.is_none()));
    }

    // ══ Potions (priorité mission — vérifiées contre la table réelle) ══

    /// La recette « 3 potions de vie → 1 supérieure » N'EXISTE PAS dans
    /// cubemain v1.12 (vérifié) : les vraies recettes de potions sont les
    /// réjuv (L5/L6/L7) et gaz+vie → antidote (L22). GOLDEN L7 :
    /// `rvs,qty=3 → rvl` (« 3 small rejuvs -> one large »).
    #[test]
    fn three_small_rejuvs_to_large_l7() {
        let c = cube();
        // Aucune recette ne produit de potion de vie hp* (donnée réelle).
        assert!(
            !c.recipes()
                .iter()
                .any(|r| r.output.kind == OutKind::Code && r.output.code.starts_with("hp")),
            "3 hp → 1 hp supérieur : absent de cubemain v1.12"
        );
        let items: [CubeItem; 3] = std::array::from_fn(|_| CubeItem::base("rvs", "Rejuv Potion"));
        let out = c.transmute(&items, &mut Rng::new(1)).expect("L7 doit matcher");
        assert_eq!(out.line, 7, "3 small rejuvs -> one large (ligne 7)");
        assert_eq!(out.consumed, vec![0, 1, 2]);
        let it = out.item.expect("item");
        assert_eq!(it.code, "rvl");
        assert_eq!(it.name_resolved, "Full Rejuv Potion", "nom patch_misc.txt");
        assert_eq!(it.quality, RolledQuality::Normal);
        // 2 seules → no-op (ni L7, ni une autre).
        assert!(c.transmute(&items[..2], &mut Rng::new(1)).is_none());
    }

    /// GOLDEN L6 : `hpot,qty=3 + mpot,qty=3 + gem0 → rvs` (« 3 health
    /// potions + 3 mana potions + 1 chipped gem -> rejuvenate potion »).
    /// Groupes hpot/mpot ouverts (hp1..hp5/mp1..mp5 réels, mixés ici) et
    /// `gem0` = TOUTE gemme ébréchée (gcr `type2=gem0` — les crânes skc
    /// aussi). L5 (gem2 → rvl) ne matche pas : gcr est `gem0` pas `gem2`.
    #[test]
    fn potions_and_chipped_gem_to_rejuv_l6() {
        let c = cube();
        let items = [
            CubeItem::base("hp1", "Lesser Healing Potion"),
            CubeItem::base("hp2", "Light Healing Potion"),
            CubeItem::base("hp3", "Healing Potion"),
            CubeItem::base("mp1", "Lesser Mana Potion"),
            CubeItem::base("mp2", "Light Mana Potion"),
            CubeItem::base("mp3", "Mana Potion"),
            CubeItem::base("gcr", "Chipped Ruby"),
        ];
        let out = c.transmute(&items, &mut Rng::new(2)).expect("L6 doit matcher");
        assert_eq!(out.line, 6);
        assert_eq!(out.consumed, vec![0, 1, 2, 3, 4, 5, 6]);
        let it = out.item.expect("item");
        assert_eq!(it.code, "rvs");
        assert_eq!(it.name_resolved, "Rejuv Potion", "nom patch_misc.txt");
        // Gemme standard au lieu d'ébréchée → L5 (rvl) et non L6.
        let mut items = items;
        items[6] = CubeItem::base("gsv", "Amethyst");
        let out = c.transmute(&items, &mut Rng::new(2)).expect("L5 doit matcher");
        assert_eq!(out.line, 5, "gem2 → full rejuv (ligne 5)");
        assert_eq!(out.item.expect("item").code, "rvl");
    }

    // ══ Gemmes (3 chippies → 1 flawed, mission) ══

    /// GOLDEN L25 : `gcv,qty=3 → gfv` (« 3 chipped amethysts -> flawed
    /// amethyst »). Mélange de familles → no-op (un slot = un seul code).
    /// Chaîne complète présente pour les 7 familles (L25-52, données).
    #[test]
    fn three_chipped_amethysts_to_flawed_l25() {
        let c = cube();
        let items: [CubeItem; 3] = std::array::from_fn(|_| CubeItem::base("gcv", "Chipped Amethyst"));
        let out = c.transmute(&items, &mut Rng::new(3)).expect("L25 doit matcher");
        assert_eq!(out.line, 25);
        let it = out.item.expect("item");
        assert_eq!(it.code, "gfv");
        assert_eq!(it.name_resolved, "Flawed Amethyst", "nom patch_misc.txt");
        // [gcv, gcv, gcr] : le 3e n'est pas une améthyste → aucune recette.
        let mixed = [
            CubeItem::base("gcv", "Chipped Amethyst"),
            CubeItem::base("gcv", "Chipped Amethyst"),
            CubeItem::base("gcr", "Chipped Ruby"),
        ];
        assert!(c.transmute(&mixed, &mut Rng::new(3)).is_none(), "mélange de familles = no-op");
        // La chaîne est un ESCALIER de 4 recettes par famille (données
        // réelles L25-52) : chip→flawed→standard→flawless→perfect.
        let familles: [(&str, [&str; 5]); 7] = [
            ("améthyste L25-28", ["gcv", "gfv", "gsv", "gzv", "gpv"]),
            ("rubis L29-32", ["gcr", "gfr", "gsr", "glr", "gpr"]),
            ("diamant L33-36", ["gcb", "gfb", "gsb", "glb", "gpb"]),
            ("émeraude L37-40", ["gcy", "gfy", "gsy", "gly", "gpy"]),
            ("saphir L41-44", ["gcg", "gfg", "gsg", "glg", "gpg"]),
            ("topaze L45-48", ["gcw", "gfw", "gsw", "glw", "gpw"]),
            ("crâne L49-52", ["skc", "skf", "sku", "skl", "skz"]),
        ];
        for (nom, f) in familles {
            for paire in f.windows(2) {
                assert!(
                    c.recipes().iter().any(|r| r.output.kind == OutKind::Code
                        && r.output.code == paire[1]
                        && r.inputs.iter().any(|i| i.code == paire[0])),
                    "chaîne {} : étape {}→{} absente",
                    nom, paire[0], paire[1]
                );
            }
        }
    }

    // ══ Runes (3×El → 1×Eld, mission ; quirk 3+gemme et 2+gemme) ══

    /// GOLDEN L53 : `r01,qty=3 → r02` (« 3 El runes -> 1 Eld rune », noms
    /// `patch_misc.txt`). GOLDEN L102 : `r10,qty=3 + gcy → r11` — au-delà
    /// de Thul, une gemme accompagne les 3 runes (donnée réelle).
    /// GOLDEN L113 : `r21,qty=2 + gfw → r22` — paliers 21+ : 2 runes.
    #[test]
    fn rune_upgrades_l53_l102_l113() {
        let c = cube();
        // 3×El → Eld.
        let els: [CubeItem; 3] = std::array::from_fn(|_| CubeItem::base("r01", "El Rune"));
        let out = c.transmute(&els, &mut Rng::new(4)).expect("L53 doit matcher");
        assert_eq!(out.line, 53);
        let it = out.item.expect("item");
        assert_eq!(it.code, "r02");
        assert_eq!(it.name_resolved, "Eld Rune");
        // 3×Thul + 1 topaz ébréchée → Amn (L102, quirk gemme).
        let thuls = [
            CubeItem::base("r10", "Thul Rune"),
            CubeItem::base("r10", "Thul Rune"),
            CubeItem::base("r10", "Thul Rune"),
            CubeItem::base("gcy", "Chipped Topaz"),
        ];
        let out = c.transmute(&thuls, &mut Rng::new(4)).expect("L102 doit matcher");
        assert_eq!(out.line, 102);
        assert_eq!(out.item.expect("item").name_resolved, "Amn Rune");
        // 2×Pul + 1 topaz imparfaite → Um (L113, palier 2+gemme).
        let puls = [
            CubeItem::base("r21", "Pul Rune"),
            CubeItem::base("r21", "Pul Rune"),
            CubeItem::base("gfw", "Flawed Topaz"),
        ];
        let out = c.transmute(&puls, &mut Rng::new(4)).expect("L113 doit matcher");
        assert_eq!(out.line, 113);
        assert_eq!(out.item.expect("item").code, "r22");
        // Sans la gemme, 3 r10 ne matchent QUE L53-61 style (r10→? non :
        // r10,qty=3 seul n'existe pas — L102 exige la gemme) → no-op.
        let thuls_sans_gemme = &thuls[..3];
        assert!(c.transmute(thuls_sans_gemme, &mut Rng::new(4)).is_none());
    }

    // ══ Anneaux / amulettes (3 anneaux → 1 amulette — mission) ══

    /// GOLDEN L15/L16 : `rin,mag,qty=3 → amu,mag` et `amu,mag,qty=3 →
    /// rin,mag` (plvl 75 stockées). La recette « 1 anneau → 1 amulette »
    /// N'EXISTE PAS (vérifié) : c'est 3 pour 1, magiques des deux côtés.
    /// Une seule bague, ou des bagues blanches → no-op.
    #[test]
    fn three_magic_rings_to_amulet_l15_l16() {
        let c = cube();
        let rings: [CubeItem; 3] =
            std::array::from_fn(|_| CubeItem { code: "rin".into(), name: "ring".into(), quality: RolledQuality::Magic, sockets: 0 });
        let out = c.transmute(&rings, &mut Rng::new(5)).expect("L15 doit matcher");
        assert_eq!(out.line, 15);
        let it = out.item.expect("item");
        assert_eq!(it.code, "amu");
        assert_eq!(it.quality, RolledQuality::Magic);
        assert_eq!(it.name_resolved, "amulet", "nom patch_misc.txt (table fait foi)");
        // Le sens inverse existe : 3 amulettes magiques → 1 anneau (L16).
        let amus: [CubeItem; 3] =
            std::array::from_fn(|_| CubeItem { code: "amu".into(), name: "amulet".into(), quality: RolledQuality::Magic, sockets: 0 });
        let out = c.transmute(&amus, &mut Rng::new(5)).expect("L16 doit matcher");
        assert_eq!(out.line, 16);
        assert_eq!(out.item.expect("item").code, "rin");
        // Ni 1 seule, ni des blanches.
        assert!(c.transmute(&rings[..1], &mut Rng::new(5)).is_none());
        let white: [CubeItem; 3] = std::array::from_fn(|_| CubeItem::base("rin", "ring"));
        assert!(c.transmute(&white, &mut Rng::new(5)).is_none(), "rin blanches : ni L15 ni L16");
    }

    // ══ Armes (types axe/knif, champs de base table) ══

    /// GOLDEN L13 : `axe + knif → tax,nor` (« 1 axe + 1 dagger -> throwing
    /// axe »). `axe`/`knif` = TYPES réels (hax type=axe, dgr type=knif) ;
    /// sortie avec champs de base de `patch_weapons.txt` : 4-7, reqdex 40.
    #[test]
    fn axe_plus_dagger_to_throwing_axe_l13() {
        let c = cube();
        let items = [CubeItem::base("hax", "Hand Axe"), CubeItem::base("dgr", "Dagger")];
        let out = c.transmute(&items, &mut Rng::new(6)).expect("L13 doit matcher");
        assert_eq!(out.line, 13);
        let it = out.item.expect("item");
        assert_eq!(it.code, "tax");
        assert_eq!(it.name_resolved, "Throwing Axe", "nom patch_weapons.txt");
        assert_eq!(it.quality, RolledQuality::Normal, "flag nor");
        assert_eq!((it.min_dmg, it.max_dmg), (4, 7), "tax mindam/maxdam (table)");
        assert_eq!(it.req_dex, 40, "tax reqdex (table)");
        assert_eq!(it.req_str, 0);
    }

    // ══ Flèches/bolts (qty sur faisceaux) ══

    /// GOLDEN L23 : `aqv,qty=2 → cqv` (« 2 arrows -> bolts ») et L24 en
    /// sens inverse. Une seule faisceau → no-op.
    #[test]
    fn arrows_to_bolts_l23() {
        let c = cube();
        let items: [CubeItem; 2] = std::array::from_fn(|_| CubeItem::base("aqv", "Arrows"));
        let out = c.transmute(&items, &mut Rng::new(7)).expect("L23 doit matcher");
        assert_eq!(out.line, 23);
        assert_eq!(out.item.expect("item").name_resolved, "Bolts");
        let bolts: [CubeItem; 2] = std::array::from_fn(|_| CubeItem::base("cqv", "Bolts"));
        let out = c.transmute(&bolts, &mut Rng::new(7)).expect("L24 doit matcher");
        assert_eq!(out.line, 24);
        assert_eq!(out.item.expect("item").code, "aqv");
        assert!(c.transmute(&items[..1], &mut Rng::new(7)).is_none());
    }

    // ══ Upgrade rare bas → exceptionnel (mission) ══

    /// GOLDEN L135 : `weap,bas,rar + r09 + r11 + gpb → useitem,mod,exc`.
    /// Une Hache (hax, rare, palier normal) devient son exceptionnel
    /// `9ha` (Hatchet — ubercode de la ligne hax, table réelle) : dégâts
    /// 10-21, requis 25/25, levelreq de ligne 19 + mod `levelreq +5` = 24.
    /// Qualité rare et nom conservés (nom rare = restant en D2).
    #[test]
    fn rare_weapon_upgrade_base_to_exceptional_l135() {
        let c = cube();
        let items = [
            CubeItem {
                code: "hax".into(),
                name: "Hand Axe".into(), // nom rare résolu (rare names restant)
                quality: RolledQuality::Rare,
                sockets: 0,
            },
            CubeItem::base("r09", "Ort Rune"),
            CubeItem::base("r11", "Amn Rune"),
            CubeItem::base("gpb", "Perfect Saphire"),
        ];
        let out = c.transmute(&items, &mut Rng::new(8)).expect("L135 doit matcher");
        assert_eq!(out.line, 135);
        assert_eq!(out.consumed.len(), 4);
        let it = out.item.expect("item");
        assert_eq!(it.code, "9ha", "hax ubercode = 9ha (Hatchet, table réelle)");
        assert_eq!(it.quality, RolledQuality::Rare, "qualité conservée");
        assert_eq!(it.name_resolved, "Hand Axe", "useitem conserve le nom");
        assert_eq!((it.min_dmg, it.max_dmg), (10, 21), "9ha mindam/maxdam (table)");
        assert_eq!((it.req_str, it.req_dex), (25, 25), "9ha requis (table)");
        assert_eq!(it.req_level, 24, "9ha levelreq 19 + mod levelreq +5");
    }

    // ══ usetype + mods (socket roll) + priorité du fichier ══

    /// GOLDEN L17 : `weap,sock + gem2,qty=3 → usetype,mag` + mod `sock
    /// 1..2` (« 3 standard gems + 1 socketed weapon -> magic, socketable
    /// weapon »). Déterminisme : même graine ⇒ même sortie ; alvéoles dans
    /// [1,2] ; PRIORITÉ : une arme magique socketée + gemmes standard
    /// matche L17 (première) et non L18/L149.
    #[test]
    fn socketed_weapon_plus_standard_gems_usetype_l17_priority_and_determinism() {
        let c = cube();
        let items = [
            CubeItem { code: "hax".into(), name: "Hand Axe".into(), quality: RolledQuality::Normal, sockets: 1 },
            CubeItem::base("gsv", "Amethyst"),
            CubeItem::base("gsg", "Saphire"),
            CubeItem::base("gsb", "Diamond"),
        ];
        let mut rng_a = Rng::new(0xD2C);
        let mut rng_b = Rng::new(0xD2C);
        let a = c.transmute(&items, &mut rng_a).expect("L17 doit matcher");
        let b = c.transmute(&items, &mut rng_b).expect("L17 doit matcher");
        assert_eq!(a, b, "déterminisme : même graine ⇒ même outcome");
        assert_eq!(a.line, 17);
        assert_eq!(a.consumed, vec![0, 1, 2, 3]);
        let it = a.item.expect("item");
        assert_eq!(it.code, "hax", "usetype : même base que le slot 1");
        assert_eq!(it.quality, RolledQuality::Magic, "flag mag");
        assert_eq!(it.name_resolved, "Hand Axe", "affixes magic non roulés (restant)");
        assert!((1..=2).contains(&it.sockets), "mod sock 1..2 : {}", it.sockets);
        // PRIORITÉ fichier : arme magique socketée → toujours L17 (jamais
        // L18 `weap,mag`+gem3 ni L149 `weap,mag`+gem0).
        let magic_socked = [
            CubeItem { code: "hax".into(), name: "Hand Axe".into(), quality: RolledQuality::Magic, sockets: 2 },
            CubeItem::base("gsv", "Amethyst"),
            CubeItem::base("gsg", "Saphire"),
            CubeItem::base("gsb", "Diamond"),
        ];
        let out = c.transmute(&magic_socked, &mut Rng::new(0xD2C)).expect("matche");
        assert_eq!(out.line, 17, "premier match gagne (ordre du fichier = D2)");
        // Arme magique NON socketée + 3 gemmes ébréchées → L149 (pas L17).
        let l149 = [
            CubeItem { code: "hax".into(), name: "Hand Axe".into(), quality: RolledQuality::Magic, sockets: 0 },
            CubeItem::base("gcv", "Chipped Amethyst"),
            CubeItem::base("gcr", "Chipped Ruby"),
            CubeItem::base("gcb", "Chipped Diamond"),
        ];
        let out = c.transmute(&l149, &mut Rng::new(0xD2C)).expect("L149 doit matcher");
        assert_eq!(out.line, 149, "weap,mag + gem0,qty=3 (ligne 149)");
    }

    // ══ pre= affixe posé (prismatic amulet) ══

    /// GOLDEN L8 : `amu,mag + 6 gemmes parfaites → amu,mag,pre=331`
    /// (« 1 perfect gem of each type + 1 amulet -> prismatic amulet »).
    /// L'affixe prefix id 331 (rang parsé MagicPrefix — encodage du contrat
    /// `RolledItem::affix_ids`) est résolu dans la table réelle, sa valeur
    /// roulée dans ses bornes, et le nom composé porte le prefix.
    #[test]
    fn prismatic_amulet_l8_prefix_affix() {
        let c = cube();
        let items = [
            CubeItem { code: "amu".into(), name: "amulet".into(), quality: RolledQuality::Magic, sockets: 0 },
            CubeItem::base("gpv", "Perfect Amethyst"),
            CubeItem::base("gpy", "Perfect Topaz"),
            CubeItem::base("gpb", "Perfect Saphire"),
            CubeItem::base("gpg", "Perfect Emerald"),
            CubeItem::base("gpr", "Perfect Ruby"),
            CubeItem::base("gpw", "Perfect Diamond"),
        ];
        let out = c.transmute(&items, &mut Rng::new(9)).expect("L8 doit matcher");
        assert_eq!(out.line, 8);
        let it = out.item.expect("item");
        assert_eq!(it.code, "amu");
        assert_eq!(it.quality, RolledQuality::Magic);
        assert_eq!(it.affix_ids.len(), 1, "un tuple (id 331, valeur roulée)");
        assert_eq!(it.affix_ids[0].0, 331);
        // L'id résout dans MagicPrefix réel et la valeur est dans ses bornes.
        let t = affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT)
            .expect("tables affixes réelles");
        let (rec, kind) = resolve_affix(&t, 331).expect("prefix 331 résoluble");
        assert_eq!(kind, crate::affixes::AffixKind::Prefix);
        assert!(it.name_resolved.starts_with(&rec.name), "{} vs {}", it.name_resolved, rec.name);
        assert!(
            rec.mods.iter().any(|d: &PropDescriptor| {
                let v = it.affix_ids[0].1 as i32;
                v >= d.min.min(d.max) && v <= d.max.max(d.min)
            }),
            "valeur {} dans les bornes de {}", it.affix_ids[0].1, rec.name
        );
    }

    // ══ Portail (sortie non-item) ══

    /// GOLDEN L4 : `leg + tbk → Cow Portal` — sortie non-item : `item`
    /// vide, `portal` renseigné, objets consommés listés.
    #[test]
    fn cow_portal_l4_is_non_item_output() {
        let c = cube();
        let items = [CubeItem::base("leg", "Wirt's Leg"), CubeItem::base("tbk", "Town Portal Book")];
        let out = c.transmute(&items, &mut Rng::new(10)).expect("L4 doit matcher");
        assert_eq!(out.line, 4);
        assert_eq!(out.consumed, vec![0, 1]);
        assert!(out.item.is_none(), "portail : pas d'item");
        assert_eq!(out.portal.as_deref(), Some("Cow Portal"));
    }

    // ══ No-op propre (mission) ══

    /// Non-recette ⇒ `None` ET aucun tirage RNG (état du générateur intact)
    /// : gemmes mélangées, objet seul, cube vide, groupe non dérivable.
    #[test]
    fn non_recipe_is_clean_noop_without_rng() {
        let c = cube();
        let mut rng = Rng::new(0xBEEF);
        let before = rng.s;
        assert!(c.transmute(&[], &mut rng).is_none(), "cube vide");
        assert_eq!(rng.s, before, "aucun tirage consommé");
        let jewel = [CubeItem::base("jew", "Jewel")];
        assert!(c.transmute(&jewel, &mut rng).is_none(), "objet seul");
        assert_eq!(rng.s, before);
        let junk = [
            CubeItem::base("gcv", "Chipped Amethyst"),
            CubeItem::base("rvl", "Full Rejuv Potion"),
            CubeItem::base("tbk", "Town Portal Book"),
        ];
        assert!(c.transmute(&junk, &mut rng).is_none(), "bricolage sans recette");
        assert_eq!(rng.s, before, "no-op = zéro RNG (déterminisme préservé)");
        // blun (L74) : groupe absent des tables embarquées ⇒ jamais apparié.
        let mac = [CubeItem::base("mac", "Mace")];
        assert!(c.transmute(&mac, &mut Rng::new(1)).is_none(), "groupe blun non dérivable");
    }

    /// Déterminisme global : 200 graines × la recette L8 (roule un affixe)
    /// — même graine ⇒ outcomes identiques champ par champ.
    #[test]
    fn determinism_over_many_seeds() {
        let c = cube();
        let items = || -> [CubeItem; 7] {
            [
                CubeItem { code: "amu".into(), name: "amulet".into(), quality: RolledQuality::Magic, sockets: 0 },
                CubeItem::base("gpv", "Perfect Amethyst"),
                CubeItem::base("gpy", "Perfect Topaz"),
                CubeItem::base("gpb", "Perfect Saphire"),
                CubeItem::base("gpg", "Perfect Emerald"),
                CubeItem::base("gpr", "Perfect Ruby"),
                CubeItem::base("gpw", "Perfect Diamond"),
            ]
        };
        for seed in 0..200u64 {
            let mut a = Rng::new(seed.wrapping_mul(6364136223846793005) | 1);
            let mut b = Rng::new(seed.wrapping_mul(6364136223846793005) | 1);
            assert_eq!(c.transmute(&items(), &mut a), c.transmute(&items(), &mut b), "seed {seed}");
        }
    }
}
