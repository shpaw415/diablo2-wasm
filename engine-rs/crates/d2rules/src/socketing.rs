//! `socketing` — Étape 17b « items fin de chaîne » (PLAN-FIDÉLITÉ phase 17,
//! 2026-10-24) : insertion de gemmes / jewels / runes dans les alvéoles
//! (sockets) d'un item.
//!
//! # Sources — LA TABLE FAIT FOI
//! * `assets/patch_gems.txt` (69 records nommés réels v1.12 : 35 gemmes —
//!   7 types × 5 grades chipped/flawed/normal/flawless/perfect — + 33 runes
//!   El..Zod de la section Expansion + 1 ligne drapeau « Expansion » SANS
//!   code, exclue comme les lignes drapeaux d'affixes) : blocs
//!   `weaponMod1-3*` / `helmMod1-3*` / `shieldMod1-3*` (Code/Param/Min/Max).
//!   Les valeurs des gemmes/runes sont FIXES (min == max) SAUF les 4 codes
//!   élémentaires d'arme `dmg-fire` / `dmg-ltng` / `dmg-cold` / `dmg-pois`
//!   (Ral 5-30, Ort 1-50, Thul 3-14, Tal 154/154) où (min,max) EST
//!   l'intervalle affiché « Adds 5-30 Fire Damage » — émis sur la PAIRE de
//!   stats mindam/maxdam, pas un roll.
//! * `patch_weapons.txt` (col `gemsockets` 53), `patch_armor.txt` (col 32),
//!   `patch_misc.txt` (col 21) : le NOMBRE MAX d'alvéoles RÉEL par base
//!   (cap retail Larzuk/quest). Empreintes réelles golden-testées : Hand Axe
//!   (`hax`) 2, Crystal Sword (`crs`) 6, Great Helm (`ghm`) 3, Quilted
//!   Armor (`qui`) 2, Archon Plate (`utp`) 4, Monarch (`uit`) 4, Jewel
//!   (`jew`) 0.
//! * Guides publics (Arreat Summit, Amazon Basin) NON FIABLES — cités pour
//!   la SÉMANTIQUE des blocs seulement, tout est arbitré par le fichier.
//!
//! # Bloc de bonus par TYPE de socket (règle retail documentée)
//! La table ne porte que TROIS blocs ; la classe d'une base :
//! * **weapon** — arme (famille `weap`, [`crate::loot::is_weapon_type`]) ;
//! * **shield** — boucliers (`shie`) + boucliers de paladin (`ashd`) ;
//! * **helm** — tout le reste d'alvéole possible : casques (`helm`, `pelt`,
//!   `head`, `circ`) ET ARMURE DE CORPS (`tors`) — l'armure partage le bloc
//!   « Helms/Armor » des guides : Perfect Ruby = +38 Life en casque ET en
//!   armure de corps, la table ne porte pas de 4e bloc.
//!
//! # Règles d'insertion (règle retail, documentée)
//! * Une gemme/rune/jewel va dans la PREMIÈRE alvéole LIBRE : la `position`
//!   d'insertion DOIT être `contents.len()` (pas de trou — l'UI retail ne
//!   choisit pas l'alvéole). La SÉQUENCE D'INSERTION = ordre retail, c'est
//!   le contrat consommé par le coéquipier Runewords (étape 17a) : « la
//!   séquence complète insérée consécutivement dans le bon ordre active le
//!   runeword » — [`ordered_runes`] restitue cette séquence.
//! * **Jewels** : props ROULÉES AU MOMENT DU DROP ([`roll_jewel`] via
//!   [`crate::item_props`] — affixes `itype jewl` réels de
//!   MagicPrefix/Suffix : 39 prefix / 36 suffix), congelées dans le joyau,
//!   appliquées à l'hôte à l'insertion.
//! * **RETRAIT** : impossible en UI retail — UNIQUEMENT via une transmutation
//!   du cube horadrim ([`remove_socket`] est ce que le cube appelle) et il
//!   DÉTRUIT la gemme (jamais rendue à l'inventaire) ET l'alvéole (`sockets`
//!   diminue de 1 : ré-insérer ne la recrée pas). Toute rupture de séquence
//!   désactive le runeword (règle du coéquipier 17a).
//!
//! # Déterminisme
//! L'insertion et le retrait sont PURES (zéro tirage) ; le bonus d'une
//! gemme/rune est FIXE (table) ; seul [`roll_jewel`] consomme le [`Rng`]
//! (un coup `pick_magic` + 1 tirage par mod d'affixe, ordre prefix →
//! suffix) — déterministe au seed, comme tout le contrat `d2rules`.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::affixes::{self, AffixTables, ItemTypeSet};
use crate::item_props::{self, PropDescriptor, RolledProp};
use crate::item_roll::RolledItem;
use crate::rng::Rng;
use crate::statlist::StatLine;
use crate::stats::StatCostTable;
use crate::tsv::Tsv;

/// patch_gems.txt v1.12 (35 gemmes + 33 runes + ligne drapeau « Expansion »).
const GEMS_TXT: &str = include_str!("../../../../assets/patch_gems.txt");
/// Bases armes (col `gemsockets` = cap d'alvéoles réel + col `type`).
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
/// Bases armures (idem).
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
/// Bases misc (jewel `jew`… gemsockets 0 — jamais socketable).
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");
/// Affixes réels (props des jewels : itype `jewl` — 39 prefix / 36 suffix).
#[cfg(test)]
const MAGIC_PREFIX_TXT: &str = include_str!("../../../../assets/patch_magicprefix.txt");
#[cfg(test)]
const MAGIC_SUFFIX_TXT: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
#[cfg(test)]
const AUTOMAGIC_TXT: &str = include_str!("../../../../assets/patch_automagic.txt");

// ---------------------------------------------------------------------------
// Types — contenu d'une alvéole
// ---------------------------------------------------------------------------

/// Classe de socket : quel bloc de mods (`weaponMod*` / `shieldMod*` /
/// `helmMod*` de `patch_gems.txt`) s'applique à une base — doc module.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SocketClass {
    /// Bloc `weaponMod*` (armes).
    Weapon,
    /// Bloc `shieldMod*` (`shie` + `ashd`).
    Shield,
    /// Bloc `helmMod*` (casques ET armure de corps `tors` — règle retail).
    Helm,
}

/// Grade d'une gemme (ordre retail de fusion cube 3→1 grade suivant).
/// Dérivé du préfixe du NOM table (`Chipped `/`Flawed `/`Flwless `/`Flawless `
/// /`Perfect ` — oui, « Flwless Saphire » ligne 10 : coquille v1.12 réelle).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GemGrade {
    /// « Chipped » (ébréchée).
    Chipped,
    /// « Flawed » (défectueuse).
    Flawed,
    /// Grade médian (pas de préfixe : « Amethyst », « Skull »…).
    Normal,
    /// « Flawless »/« Flwless » (immaculée).
    Flawless,
    /// « Perfect » (parfaite).
    Perfect,
}

/// Un joyau : props roulées AU MOMENT DU DROP (règle retail — les affixes
/// d'un jewel sont tirés à la génération de l'item, pas à l'insertion).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Jewel {
    /// Nom composé retail « `<Prefix> Jewel <Suffix>` » (angles vides si
    /// pas d'affixe).
    pub name: String,
    /// Props roulées ([`crate::item_props::RolledProp`]) — évaluées vers
    /// ids ItemStatCost à l'agrégation ([`socket_stat_lines`]).
    pub props: Vec<RolledProp>,
}

/// Contenu d'une alvéole — une gemme OU une rune (toutes deux des records
/// `patch_gems.txt` désignés par leur code : `gpv`, `r31`…) ou un jewel.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum SocketContent {
    /// Gemme ou rune — code table (`gcv`..`skz`, `r01`..`r33`).
    Gem(String),
    /// Joyau avec ses props roulées au drop.
    Jewel(Jewel),
}

impl SocketContent {
    /// Une gemme/rune par code.
    pub fn gem(code: impl Into<String>) -> SocketContent {
        SocketContent::Gem(code.into())
    }

    /// Code de gemme/rune (`Gem`), `None` pour un jewel.
    pub fn gem_code(&self) -> Option<&str> {
        match self {
            SocketContent::Gem(c) => Some(c),
            SocketContent::Jewel(_) => None,
        }
    }

    /// C'est une RUNE (`r01`..`r33` de la section Expansion) ?
    pub fn is_rune(&self) -> bool {
        self.gem_code().is_some_and(is_rune_code)
    }
}

/// Une rune = code `r` + 2 chiffres (`r01`..`r33` — section Expansion de
/// `patch_gems.txt`, rows 38-70 du fichier réel).
fn is_rune_code(code: &str) -> bool {
    code.len() == 3
        && code.starts_with('r')
        && code[1..].bytes().all(|b| b.is_ascii_digit())
}

/// Erreurs d'insertion/retrait — négligeables en données réelles (l'UI
/// retail ne peut pas les produire), exposées pour l'intégrateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SocketError {
    /// Code gemme/rune absent de `patch_gems.txt` (68 codes réels).
    UnknownGem,
    /// Plus d'alvéole libre (sockets 0 ou toutes occupées/détruites).
    Full,
    /// `position` invalide — insertion : DOIT être la première alvéole
    /// libre (`contents.len()`, règle retail « pas de trou ») ; retrait :
    /// hors de la zone remplie.
    BadPosition,
}

// ---------------------------------------------------------------------------
// Item socketé — hôte + alvéoles
// ---------------------------------------------------------------------------

/// Un item porteur d'alvéoles (l'HÔTE) et de leur contenu, en ordre
/// d'insertion retail. Pur — aucun tirage : construit depuis un drop
/// ([`SocketedItem::from_rolled`]) ou à la main (tests/intégrateur).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SocketedItem {
    /// Code table de la base hôte (`crs`, `uit`, `ghm`…).
    pub code: String,
    /// Type D2 de la base (`swor`, `shie`, `tors`, `helm`…) — pilote la
    /// [`SocketClass`] (bloc de bonus des gemmes).
    pub wtype: String,
    /// Alvéoles ACTUELLES (≤ `max_sockets`) — diminue à chaque retrait cube
    /// (l'alvéole est détruite, cf. doc module).
    pub sockets: u8,
    /// Alvéoles MAX de la base (colonne `gemsockets` des tables — cap retail
    /// Larzuk/quest ; `0` si la base n'est jamais socketable).
    pub max_sockets: u8,
    /// Contenu des alvéoles dans L'ORDRE D'INSERTION (règle retail —
    /// contrat du coéquipier Runewords : [`ordered_runes`]).
    pub contents: Vec<SocketContent>,
}

impl SocketedItem {
    /// Hôte socketé à la main (tests/intégrateur) : `sockets` = alvéoles
    /// actuelles (≤ `max_sockets`), contenu vide.
    pub fn new(
        code: impl Into<String>,
        wtype: impl Into<String>,
        sockets: u8,
        max_sockets: u8,
    ) -> SocketedItem {
        SocketedItem {
            code: code.into(),
            wtype: wtype.into(),
            sockets,
            max_sockets,
            contents: Vec::new(),
        }
    }

    /// Hôte depuis un drop : `sockets` = le mod `sock` roulé du drop
    /// ([`RolledItem::sockets`] — Mechanist's/Artificer's/Jeweler's),
    /// `max_sockets` = colonne `gemsockets` de la base, `wtype` résolu par
    /// code dans les tables. `None` si le code n'est pas une base
    /// weap/armo/misc connue (jamais socketable).
    pub fn from_rolled(rolled: &RolledItem) -> Option<SocketedItem> {
        let t = embedded();
        let base = t.bases.get(&rolled.code)?;
        Some(SocketedItem {
            code: rolled.code.clone(),
            wtype: base.wtype.clone(),
            sockets: rolled.sockets,
            max_sockets: base.max_sockets,
            contents: Vec::new(),
        })
    }

    /// Flag « socketed » retail : l'item PORTE des alvéoles (pleines ou
    /// vides) — un item détruit-alvéole-par-alvéole redevient non-socketé.
    pub fn is_socketed(&self) -> bool {
        self.sockets > 0
    }

    /// Alvéoles libres (`sockets − remplies` ; ≥ 0).
    pub fn free(&self) -> usize {
        self.sockets as usize - self.contents.len().min(self.sockets as usize)
    }

    /// Classe de socket de CETTE base (via son `wtype`).
    pub fn class(&self) -> SocketClass {
        socket_class_of(&self.wtype)
    }
}

// ---------------------------------------------------------------------------
// Tables parsées — patch_gems + gemsockets des 3 tables de bases
// ---------------------------------------------------------------------------

/// Un record gemme/rune de `patch_gems.txt` : les trois blocs de mods en
/// descripteurs bruts (Code/Param/Min/Max — mêmes cellules que les tables
/// d'affixes, consommés par [`item_props`]).
#[derive(Debug, Clone, PartialEq)]
pub struct GemRec {
    /// Nom table exact (« Perfect Amethyst », « Flwless Saphire »,
    /// « Jah Rune »…).
    pub name: String,
    /// Code item (`gpv`, `glb`, `r31`… — c'est le code `patch_misc.txt`).
    pub code: String,
    /// Bloc arme (`weaponMod1-3*`).
    pub weapon: Vec<PropDescriptor>,
    /// Bloc casque/armure (`helmMod1-3*`).
    pub helm: Vec<PropDescriptor>,
    /// Bloc bouclier (`shieldMod1-3*`).
    pub shield: Vec<PropDescriptor>,
}

impl GemRec {
    /// Grade de la gemme (`None` pour les 33 runes — pas de grade).
    pub fn grade(&self) -> Option<GemGrade> {
        if is_rune_code(&self.code) {
            return None;
        }
        let g = if self.name.starts_with("Chipped ") {
            GemGrade::Chipped
        } else if self.name.starts_with("Flawed ") {
            GemGrade::Flawed
        } else if self.name.starts_with("Flwless ") || self.name.starts_with("Flawless ") {
            GemGrade::Flawless
        } else if self.name.starts_with("Perfect ") {
            GemGrade::Perfect
        } else {
            GemGrade::Normal
        };
        Some(g)
    }

    /// Nom de TYPE (« Amethyst », « Saphire »…) — le nom table sans le
    /// préfixe de grade ; le nom complet pour les runes (« Jah Rune »).
    pub fn gem_type_name(&self) -> &str {
        for p in ["Chipped ", "Flawed ", "Flwless ", "Flawless ", "Perfect "] {
            if let Some(rest) = self.name.strip_prefix(p) {
                return rest;
            }
        }
        &self.name
    }

    /// Le bloc de mods pour une classe de socket (règle retail, doc module).
    pub fn mods_for(&self, class: SocketClass) -> &[PropDescriptor] {
        match class {
            SocketClass::Weapon => &self.weapon,
            SocketClass::Shield => &self.shield,
            SocketClass::Helm => &self.helm,
        }
    }
}

/// Alvéoles max + type d'une base (colonnes `gemsockets`/`type` des tables
/// weap/armo/misc).
#[derive(Debug, Clone, PartialEq)]
struct BaseSockets {
    max_sockets: u8,
    wtype: String,
}

/// Tables du socketing : gemmes/runes + map base → (gemsockets, type).
/// Parse LENIENT (table absente/illisible = entrées manquantes, jamais de
/// panique — règle mission) ; [`embedded`] garantit les tables réelles.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SocketingTables {
    gems: HashMap<String, GemRec>,
    bases: HashMap<String, BaseSockets>,
}

impl SocketingTables {
    /// Parse les 4 tables (patch_gems + weapons/armor/misc).
    pub fn parse(gems_txt: &str, weapons_txt: &str, armor_txt: &str, misc_txt: &str) -> SocketingTables {
        let mut out = SocketingTables::default();
        if let Ok(t) = Tsv::parse(gems_txt) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let name = r.s("name").trim().to_string();
                let code = r.s("code").trim().to_string();
                if code.is_empty() {
                    continue; // ligne drapeau « Expansion » (sans code)
                }
                let block = |prefix: &str| {
                    (1..=3)
                        .map(|n| {
                            item_props::descriptor([
                                r.s(&format!("{prefix}{n}Code")),
                                r.s(&format!("{prefix}{n}Param")),
                                r.s(&format!("{prefix}{n}Min")),
                                r.s(&format!("{prefix}{n}Max")),
                            ])
                        })
                        .flatten()
                        .collect::<Vec<_>>()
                };
                out.gems.insert(
                    code.clone(),
                    GemRec {
                        name,
                        code,
                        weapon: block("weaponMod"),
                        helm: block("helmMod"),
                        shield: block("shieldMod"),
                    },
                );
            }
        }
        for txt in [weapons_txt, armor_txt, misc_txt] {
            let Ok(t) = Tsv::parse(txt) else { continue };
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let code = r.s("code").trim().to_string();
                if code.is_empty() {
                    continue;
                }
                let gs = r.u("gemsockets").min(u8::MAX as u32) as u8;
                let wtype = r.s("type").trim().to_string();
                out.bases
                    .entry(code)
                    .or_insert(BaseSockets { max_sockets: gs, wtype });
            }
        }
        out
    }

    /// Record gemme/rune par code (`gpv`, `r31`…) — `None` si inconnu.
    pub fn gem(&self, code: &str) -> Option<&GemRec> {
        self.gems.get(code)
    }

    /// Nombre de records gemmes+runes parsés (68 réels — golden test).
    pub fn gem_count(&self) -> usize {
        self.gems.len()
    }

    /// Alvéoles max réelles d'une base (colonne `gemsockets`) — `None` si
    /// base inconnue des trois tables.
    pub fn max_sockets(&self, code: &str) -> Option<u8> {
        self.bases.get(code).map(|b| b.max_sockets)
    }

    /// Type D2 d'une base (`swor`, `shie`, `tors`…) — `None` si inconnue.
    pub fn wtype_of(&self, code: &str) -> Option<&str> {
        self.bases.get(code).map(|b| b.wtype.as_str())
    }

    /// Bonus FIXES d'une gemme/rune pour une classe de socket, en lignes
    /// `(id ItemStatCost, valeur)` :
    /// * codes généraux (`att`, `str`, `ac`, `hp`, `res-*`, `regen`,
    ///   `cold-len`…) → [`item_props::evaluate`] (valeur FIXE `min` —
    ///   gemmes/runes : min == max) ;
    /// * les 4 codes élémentaires d'arme en PAIRE (`dmg-fire`/`dmg-ltng`/
    ///   `dmg-cold`/`dmg-pois`) → min sur `*mindam`, max sur `*maxdam`
    ///   (l'intervalle affiché retail « Adds 5-30 Fire Damage » de Ral) +
    ///   la durée `par` sur `coldlength`/`poisonlength` si présente.
    /// `None` si le code est inconnu.
    pub fn gem_bonus(&self, code: &str, class: SocketClass) -> Option<Vec<StatLine>> {
        let rec = self.gem(code)?;
        Some(rec.mods_for(class).iter().flat_map(mod_stat_lines).collect())
    }
}

static EMBEDDED: OnceLock<SocketingTables> = OnceLock::new();

/// Tables embarquées réelles v1.12 (singleton — mêmes tables que le crate).
pub fn embedded() -> &'static SocketingTables {
    EMBEDDED.get_or_init(|| SocketingTables::parse(GEMS_TXT, WEAPONS_TXT, ARMOR_TXT, MISC_TXT))
}

/// Tables d'affixes embarquées (props des jewels — [`roll_jewel`]).
#[cfg(test)]
fn jewel_affixes() -> Option<AffixTables> {
    affixes::parse(MAGIC_PREFIX_TXT, MAGIC_SUFFIX_TXT, AUTOMAGIC_TXT)
}

// ---------------------------------------------------------------------------
// Émission des bonus → ids ItemStatCost (pont statlist)
// ---------------------------------------------------------------------------

/// Id numérique d'une stat par nom (`ItemStatCost.txt` — `-1` si absente,
/// comme [`item_props::evaluate`]).
fn isc(name: &str) -> i32 {
    StatCostTable::global().get(name).map(|r| r.id).unwrap_or(-1)
}

/// Émet les lignes d'UN descripteur de gemme/rune (valeur FIXE — pas de
/// roll ; les 4 codes élémentaires portent l'intervalle (min,max) sur la
/// paire mindam/maxdam, cf. [`SocketingTables::gem_bonus`]).
fn mod_stat_lines(desc: &PropDescriptor) -> Vec<StatLine> {
    let min = desc.min;
    let max = if desc.max == 0 { desc.min } else { desc.max };
    match desc.code.trim().to_ascii_lowercase().as_str() {
        "dmg-fire" => pair_lines("firemindam", min, "firemaxdam", max),
        "dmg-ltng" => pair_lines("lightmindam", min, "lightmaxdam", max),
        "dmg-cold" => {
            let mut v = pair_lines("coldmindam", min, "coldmaxdam", max);
            push_length(&mut v, "coldlength", &desc.param);
            v
        }
        "dmg-pois" => {
            let mut v = pair_lines("poisonmindam", min, "poisonmaxdam", max);
            push_length(&mut v, "poisonlength", &desc.param);
            v
        }
        // Codes généraux : la valeur FIXE est `min` (gemmes/runes : min ==
        // max) — même dispatch que les affixes/uniques ([`item_props]).
        _ => item_props::evaluate(desc, min)
            .emits
            .iter()
            .filter(|e| e.stat_id >= 0)
            .map(|e| StatLine::new(e.stat_id, e.value))
            .collect(),
    }
}

fn pair_lines(min_stat: &str, min: i32, max_stat: &str, max: i32) -> Vec<StatLine> {
    vec![StatLine::new(isc(min_stat), min as i64), StatLine::new(isc(max_stat), max as i64)]
}

fn push_length(out: &mut Vec<StatLine>, stat: &str, param: &str) {
    if let Ok(frames) = param.trim().parse::<i32>() {
        if frames > 0 {
            out.push(StatLine::new(isc(stat), frames as i64));
        }
    }
}

/// Classe de socket d'un TYPE de base (règle retail — doc module) :
/// armes → weapon ; `shie`/`ashd` → shield ; le reste (casques, `tors`…) →
/// helm (l'armure de corps partage le bloc casque).
pub fn socket_class_of(wtype: &str) -> SocketClass {
    if crate::loot::is_weapon_type(wtype) {
        SocketClass::Weapon
    } else if wtype == "shie" || wtype == "ashd" {
        SocketClass::Shield
    } else {
        SocketClass::Helm
    }
}

/// Bonus d'une gemme/rune pour le type de socket d'un WTYPE de base
/// (confort intégrateur — [`SocketingTables::gem_bonus`] sur les tables
/// embarquées). `None` si code inconnu.
pub fn gem_bonus_for_wtype(code: &str, wtype: &str) -> Option<Vec<StatLine>> {
    embedded().gem_bonus(code, socket_class_of(wtype))
}

/// Agrège TOUT le contenu des alvéoles d'un hôte en lignes `(id, valeur)`
/// — la source « insertions » que [`crate::statlist::StatList`] somme avec
/// les autres (base héros + affixes + uniques). Gemmes/runes : bonus FIXES
/// du bloc de leur classe ; jewels : les émissions PURES de leurs props
/// roulées (les émissions paramétrées/skills restent portées par
/// [`Jewel::props`] pour l'intégrateur).
pub fn socket_stat_lines(item: &SocketedItem) -> Vec<StatLine> {
    let t = embedded();
    let class = item.class();
    let mut out = Vec::new();
    for c in &item.contents {
        match c {
            SocketContent::Gem(code) => {
                if let Some(bonus) = t.gem_bonus(code, class) {
                    out.extend(bonus);
                }
            }
            SocketContent::Jewel(j) => {
                for p in &j.props {
                    for e in &item_props::evaluate(&p.desc, p.value).emits {
                        if e.stat_id >= 0 {
                            out.push(StatLine::new(e.stat_id, e.value));
                        }
                    }
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// API CONTRAT coéquipier Runewords (étape 17a)
// ---------------------------------------------------------------------------

/// Insère une gemme/rune/jewel dans une alvéole (RÈGLE RETAIL, doc module) :
/// * `gem` doit exister (`Gem` résolu dans `patch_gems.txt` — 68 codes) ;
/// * `position` DOIT être `item.contents.len()` — la PREMIÈRE alvéole
///   libre (l'UI retail ne choisit pas l'alvéole : remplissage consécutif) ;
/// * l'hôte doit avoir une alvéole libre (`contents.len() < sockets`).
/// La séquence d'insertion ainsi formée EST l'ordre retail consommé par
/// [`ordered_runes`] (coéquipier Runewords : « la séquence complète
/// insérée consécutivement dans le bon ordre active le runeword »).
pub fn insert_socket(
    item: &mut SocketedItem,
    position: usize,
    gem: SocketContent,
) -> Result<(), SocketError> {
    if let Some(code) = gem.gem_code() {
        if embedded().gem(code).is_none() {
            return Err(SocketError::UnknownGem);
        }
    }
    if position != item.contents.len() {
        return Err(SocketError::BadPosition);
    }
    if item.contents.len() >= item.sockets as usize {
        return Err(SocketError::Full);
    }
    item.contents.push(gem);
    Ok(())
}

/// La liste ORDONNÉE des runes insérées (`r01`..`r33` — codes table), dans
/// l'ordre d'insertion retail — LE contrat du coéquipier Runewords (étape
/// 17a : Enigma = `r31`+`r06`+`r30` insérés dans cet ordre, cf.
/// `crates/d2d2s/tests/real_javazon.rs`). Gemmes et jewels exclus ; une
/// séquence rompue (retrait cube) reste dans l'ordre relatif restant — la
/// désactivation du runeword est LA règle du coéquipier.
pub fn ordered_runes(item: &SocketedItem) -> Vec<String> {
    item.contents
        .iter()
        .filter(|c| c.is_rune())
        .filter_map(|c| c.gem_code().map(str::to_string))
        .collect()
}

/// Retire le contenu d'une alvéole — UNIQUE VIA LE CUBE HORADRIM (règle
/// retail : pas de retrait en UI). DÉTRUIT la gemme (rien n'est rendue à
/// l'inventaire — jamais de gemme récupérée) ET l'ALVÉOLE elle-même
/// (`sockets` diminue de 1 : l'alvéole ne peut plus être remplie). Le
/// contenu restant garde son ordre d'insertion relatif (compacté) — toute
/// rupture de séquence désactive le runeword (règle coéquipier 17a).
pub fn remove_socket(item: &mut SocketedItem, position: usize) -> Result<(), SocketError> {
    if position >= item.contents.len() {
        return Err(SocketError::BadPosition);
    }
    item.contents.remove(position);
    item.sockets = item.sockets.saturating_sub(1); // l'alvéole est détruite
    Ok(())
}

/// Alvéoles MAX réelles d'une base par code (colonne `gemsockets` des
/// tables weap/armo/misc, tables embarquées) — `None` si base inconnue.
pub fn max_sockets_of(code: &str) -> Option<u8> {
    embedded().max_sockets(code)
}

/// Roule un JOYAU AU MOMENT DU DROP (règle retail : les affixes d'un jewel
/// sont tirés à la génération). Pool RÉEL : MagicPrefix/Suffix v1.12
/// filtrés sur `itype jewl` (39 prefix / 36 suffix), fenêtre `level..=max`
/// à `ilvl` (qlvl jewel 0 → alvl = ilvl, cf. [`affixes::alvl`]) ; tirage
/// [`affixes::pick_magic`] (1 coup : prefix <2/5, suffix ≠1/5 — port
/// Diablerie) puis UN tirage [`item_props::roll`] par mod, ordre prefix →
/// suffix. Déterministe au seed du [`Rng`].
#[allow(private_interfaces)] // Rng crate-priv, même pattern que statlist::ItemStats
pub fn roll_jewel(rng: &mut Rng, affixes: &AffixTables, ilvl: u32) -> SocketContent {
    let types = ItemTypeSet {
        wtype: "jewl",
        is_weap: false,
        is_mele: false,
        is_bow: false,
        is_armo: false,
    };
    let (prefix, suffix) = affixes::pick_magic(rng, affixes, &types, ilvl);
    let mut props = Vec::new();
    if let Some(p) = prefix {
        for d in &p.mods {
            props.push(item_props::roll(d, rng));
        }
    }
    if let Some(s) = suffix {
        for d in &s.mods {
            props.push(item_props::roll(d, rng));
        }
    }
    let mut name = "Jewel".to_string();
    if let Some(s) = suffix {
        name = format!("{name} {}", s.name);
    }
    if let Some(p) = prefix {
        name = format!("{} {name}", p.name);
    }
    SocketContent::Jewel(Jewel { name, props })
}

// ---------------------------------------------------------------------------
// Golden tests — tables réelles v1.12 citées ligne à ligne, déterminisme.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::statlist::{StatList, ARMORCLASS, FIRERESIST, MAXHP, STRENGTH, TOHIT};

    fn affix_tables() -> AffixTables {
        jewel_affixes().expect("tables affixes embarquées valides")
    }

    // --- Parse patch_gems.txt : 35 gemmes + 33 runes, drapeau exclu ---

    /// `patch_gems.txt` : 69 records nommés = 35 gemmes (7 types × 5
    /// grades) + 33 runes ; la ligne drapeau « Expansion » (sans code) est
    /// exclue comme les drapeaux d'affixes. Quirks de noms réels cités.
    #[test]
    fn gems_table_parse_v112() {
        let t = embedded();
        assert_eq!(t.gem_count(), 68, "35 gemmes + 33 runes ; drapeau « Expansion » exclu");
        let gems: Vec<&GemRec> = t.gems.values().filter(|g| g.grade().is_some()).collect();
        let runes: Vec<&GemRec> = t.gems.values().filter(|g| g.grade().is_none()).collect();
        assert_eq!(gems.len(), 35, "7 types × 5 grades");
        assert_eq!(runes.len(), 33, "El..Zod (section Expansion)");
        // Amethyst : lignes 2-6 (gcv/gfv/gsv/gzv/gpv).
        let amethyst = [
            ("gcv", GemGrade::Chipped),
            ("gfv", GemGrade::Flawed),
            ("gsv", GemGrade::Normal),
            ("gzv", GemGrade::Flawless),
            ("gpv", GemGrade::Perfect),
        ];
        for (code, grade) in amethyst {
            let g = t.gem(code).unwrap_or_else(|| panic!("{code}"));
            assert_eq!(g.grade(), Some(grade), "{code}");
            assert_eq!(g.gem_type_name(), "Amethyst");
        }
        // Quirk v1.12 : « Flwless Saphire » (ligne 10, glb) — coquille
        // réelle du fichier, le grade reste Flawless.
        let glb = t.gem("glb").unwrap();
        assert_eq!(glb.name, "Flwless Saphire");
        assert_eq!(glb.grade(), Some(GemGrade::Flawless));
        assert_eq!(glb.gem_type_name(), "Saphire");
        // Skulls : skc..skz (lignes 32-36), grade médian sans préfixe.
        assert_eq!(t.gem("sku").unwrap().grade(), Some(GemGrade::Normal));
        assert_eq!(t.gem("sku").unwrap().gem_type_name(), "Skull");
        // Runes : nom complet, pas de grade (r01..r33).
        let jah = t.gem("r31").unwrap();
        assert_eq!(jah.name, "Jah Rune");
        assert_eq!(jah.grade(), None);
        assert!(SocketContent::gem("r31").is_rune());
        assert!(!SocketContent::gem("gpv").is_rune());
        assert!(!SocketContent::gem("skz").is_rune());
    }

    // --- 3 GOLDEN gemmes par type de socket (valeurs patch_gems citées) ---

    /// GOLDEN ① — Perfect Amethyst (`gpv`, patch_gems.txt ligne 6) :
    /// `weaponMod1 att 150` / `helmMod1 str 10` / `shieldMod1 ac 30` →
    /// ids ItemStatCost `tohit` 19 / `strength` 0 / `armorclass` 31.
    #[test]
    fn golden_perfect_amethyst_three_socket_types() {
        let t = embedded();
        assert_eq!(
            t.gem_bonus("gpv", SocketClass::Weapon).unwrap(),
            vec![StatLine::new(TOHIT, 150)],
            "gpv weaponMod1 « att 150 » (ligne 6)"
        );
        assert_eq!(
            t.gem_bonus("gpv", SocketClass::Helm).unwrap(),
            vec![StatLine::new(STRENGTH, 10)],
            "gpv helmMod1 « str 10 » (ligne 6)"
        );
        assert_eq!(
            t.gem_bonus("gpv", SocketClass::Shield).unwrap(),
            vec![StatLine::new(ARMORCLASS, 30)],
            "gpv shieldMod1 « ac 30 » (ligne 6)"
        );
    }

    /// GOLDEN ② — Chipped Ruby (`gcr`, ligne 17) : arme en PAIRE de mods
    /// `fire-min 3` / `fire-max 4` (adds 3-4 fire) ; helm `hp 10` ;
    /// shield `res-fire 12`. Classe par base réelle : `crs` (swor) →
    /// Weapon, `uit` (Monarch, shie) → Shield, `utp` (Archon Plate, tors)
    /// → Helm (l'armure partage le bloc casque).
    #[test]
    fn golden_chipped_ruby_by_base_class() {
        let fire_min = isc("firemindam");
        let fire_max = isc("firemaxdam");
        assert!(fire_min >= 0 && fire_max >= 0);
        assert_eq!(
            gem_bonus_for_wtype("gcr", "swor").unwrap(),
            vec![StatLine::new(fire_min, 3), StatLine::new(fire_max, 4)],
            "gcr weaponMod1/2 « fire-min 3 / fire-max 4 » (ligne 17)"
        );
        assert_eq!(
            gem_bonus_for_wtype("gcr", "helm").unwrap(),
            vec![StatLine::new(MAXHP, 10)],
            "gcr helmMod1 « hp 10 » (ligne 17)"
        );
        assert_eq!(
            gem_bonus_for_wtype("gcr", "shie").unwrap(),
            vec![StatLine::new(FIRERESIST, 12)],
            "gcr shieldMod1 « res-fire 12 » (ligne 17)"
        );
        // L'ARMURE DE CORPS (tors) prend le bloc casque (règle retail) —
        // vérifié sur la base réelle utp (Archon Plate).
        assert_eq!(socket_class_of("tors"), SocketClass::Helm);
        assert_eq!(
            gem_bonus_for_wtype("gcr", "tors").unwrap(),
            vec![StatLine::new(MAXHP, 10)],
            "Perfect-style bloc helm partagé avec l'armure de corps"
        );
    }

    /// GOLDEN ③ — Ral (`r08`, ligne 44) : le code d'arme en INTERVALLE
    /// `dmg-fire 5 30` (min ≠ max — « Adds 5-30 Fire Damage » retail) est
    /// émis sur la PAIRE `firemindam` 5 / `firemaxdam` 30 (PAS un roll) ;
    /// helm `res-fire 30` / shield `res-fire 35`. Thul (`r10`) ajoute la
    /// durée `par` 75 sur `coldlength`.
    #[test]
    fn golden_ral_and_thul_range_pairs() {
        let t = embedded();
        let fire_min = isc("firemindam");
        let fire_max = isc("firemaxdam");
        assert_eq!(
            t.gem_bonus("r08", SocketClass::Weapon).unwrap(),
            vec![StatLine::new(fire_min, 5), StatLine::new(fire_max, 30)],
            "r08 weaponMod1 « dmg-fire 5-30 » (ligne 44)"
        );
        assert_eq!(
            t.gem_bonus("r08", SocketClass::Helm).unwrap(),
            vec![StatLine::new(FIRERESIST, 30)],
            "r08 helmMod1 « res-fire 30 » (ligne 44)"
        );
        assert_eq!(
            t.gem_bonus("r08", SocketClass::Shield).unwrap(),
            vec![StatLine::new(FIRERESIST, 35)],
            "r08 shieldMod1 « res-fire 35 » (ligne 44)"
        );
        let cold_min = isc("coldmindam");
        let cold_max = isc("coldmaxdam");
        let cold_len = isc("coldlength");
        assert_eq!(
            t.gem_bonus("r10", SocketClass::Weapon).unwrap(),
            vec![
                StatLine::new(cold_min, 3),
                StatLine::new(cold_max, 14),
                StatLine::new(cold_len, 75),
            ],
            "r10 weaponMod1 « dmg-cold par 75 min 3 max 14 » (ligne 49)"
        );
    }

    // --- Sockets : cap réel gemsockets + flag socketed + from_rolled ---

    /// Colonne `gemsockets` réelle : Hand Axe 2 / Crystal Sword 6
    /// (patch_weapons col 53), Great Helm 3 / Quilted 2 / Archon Plate 4 /
    /// Monarch 4 (patch_armor col 32), Jewel 0 (patch_misc col 21).
    /// `from_rolled` résout wtype + cap depuis un drop.
    #[test]
    fn gemsockets_caps_real_columns() {
        let t = embedded();
        for (code, gs) in [
            ("hax", 2u8),
            ("crs", 6),
            ("ghm", 3),
            ("qui", 2),
            ("utp", 4),
            ("uit", 4),
            ("jew", 0),
        ] {
            assert_eq!(t.max_sockets(code), Some(gs), "{code} gemsockets");
        }
        assert_eq!(max_sockets_of("crs"), Some(6));
        assert_eq!(max_sockets_of("zzz"), None, "base inconnue");
        // from_rolled : le drop porte ses alvéoles (mod sock roulé 7a).
        let rolled = RolledItem { code: "crs".into(), sockets: 4, ..Default::default() };
        let host = SocketedItem::from_rolled(&rolled).unwrap();
        assert_eq!(host.wtype, "swor");
        assert_eq!(host.sockets, 4);
        assert_eq!(host.max_sockets, 6);
        assert_eq!(host.class(), SocketClass::Weapon);
        assert!(host.is_socketed());
        assert!(host.contents.is_empty());
        // Base jamais socketable (jewel gs=0) : flag socketed false.
        let jewel = RolledItem { code: "jew".into(), ..Default::default() };
        let host = SocketedItem::from_rolled(&jewel).unwrap();
        assert_eq!(host.max_sockets, 0);
        assert!(!host.is_socketed());
        // Code hors des 3 tables → pas d'hôte socketable.
        assert!(SocketedItem::from_rolled(&RolledItem {
            code: "zzz".into(),
            ..Default::default()
        })
        .is_none());
    }

    // --- Cycle insert / ordered_runes / remove (contrat Runewords) ---

    /// Séquence ordonnée retail : Jah+Ith+Ber (Enigma, cf.
    /// `crates/d2d2s/tests/real_javazon.rs`) insérés DANS L'ORDRE sur une
    /// base réelle 4 alvéoles (Archon Plate) — `ordered_runes` restitue la
    /// séquence ; position arbitraire refusée (pas de trou) ; cap refusé ;
    /// retrait CUBE détruit gemme + alvéole.
    #[test]
    fn enigma_sequence_insert_order_and_cube_removal() {
        let mut host = SocketedItem::new("utp", "tors", 4, 4);
        assert_eq!(host.class(), SocketClass::Helm, "tors → bloc helm");
        // Code inconnu refusé AVANT toute position.
        assert_eq!(
            insert_socket(&mut host, 0, SocketContent::gem("zzz")),
            Err(SocketError::UnknownGem)
        );
        // Enigma = r31 (Jah) + r06 (Ith) + r30 (Ber) DANS CET ORDRE.
        insert_socket(&mut host, 0, SocketContent::gem("r31")).unwrap();
        insert_socket(&mut host, 1, SocketContent::gem("r06")).unwrap();
        insert_socket(&mut host, 2, SocketContent::gem("r30")).unwrap();
        assert_eq!(ordered_runes(&host), vec!["r31", "r06", "r30"]);
        // Pas de trou : l'UI retail ne choisit pas l'alvéole.
        assert_eq!(
            insert_socket(&mut host, 1, SocketContent::gem("r04")),
            Err(SocketError::BadPosition)
        );
        // 4e alvéole (jewel) puis cap : Full.
        insert_socket(&mut host, 3, SocketContent::Jewel(Jewel::default())).unwrap();
        assert_eq!(host.free(), 0);
        assert_eq!(
            insert_socket(&mut host, 4, SocketContent::gem("gpv")),
            Err(SocketError::Full)
        );
        // Retrait CUBE @1 (Ith) : gemme détruite + ALVÉOLE détruite
        // (4 → 3 alvéoles), ordre relatif compacté — runeword rompu
        // (désactivation = règle du coéquipier 17a, il consomme la liste).
        remove_socket(&mut host, 1).unwrap();
        assert_eq!(host.sockets, 3, "l'alvéole est détruite (4 → 3)");
        assert_eq!(host.max_sockets, 4, "le cap table reste l'historique");
        assert_eq!(host.contents.len(), 3);
        assert_eq!(ordered_runes(&host), vec!["r31", "r30"]);
        // L'alvéole détruite ne revient pas : 3/3 remplies → Full.
        assert_eq!(
            insert_socket(&mut host, 3, SocketContent::gem("r04")),
            Err(SocketError::Full)
        );
        // Gemme détruite = jamais rendue (remove ne retourne rien).
        // Retraits suivants jusqu'à zéro alvéole : plus « socketed ».
        remove_socket(&mut host, 0).unwrap();
        remove_socket(&mut host, 0).unwrap();
        remove_socket(&mut host, 0).unwrap();
        assert_eq!(host.sockets, 0);
        assert!(!host.is_socketed());
        assert!(ordered_runes(&host).is_empty());
        assert_eq!(remove_socket(&mut host, 0), Err(SocketError::BadPosition));
    }

    // --- Jewel : props roulées au drop, appliquées à l'hôte ---

    /// Jewel : le roll est DÉTERMINISTE au seed, les props roulées restent
    /// dans les bornes de leur descripteur, et l'insertion applique les
    /// props au HOST (et non pas un re-roll) — un jewel inséré dans deux
    /// hôtes différents porte EXACTEMENT les mêmes valeurs.
    #[test]
    fn jewel_rolled_at_drop_and_applied_as_is() {
        let affixes = affix_tables();
        // Déterminisme (mission ④) : même seed ⇒ jewel identique.
        let a = roll_jewel(&mut Rng::new(11), &affixes, 99);
        let b = roll_jewel(&mut Rng::new(11), &affixes, 99);
        assert_eq!(a, b);
        // Les pools jewels réels (39 prefix / 36 suffix itype jewl)
        // produisent des joyaux AFFIXÉS sur quelques seeds.
        let affixed: Vec<Jewel> = (0..10u64)
            .map(|s| roll_jewel(&mut Rng::new(s), &affixes, 99))
            .filter_map(|c| match c {
                SocketContent::Jewel(j) if !j.props.is_empty() => Some(j),
                _ => None,
            })
            .collect();
        assert!(!affixed.is_empty(), "39/36 affixes jewl réels : des rolls affixés vus");
        // Valeurs dans les bornes du descripteur (max vide = valeur fixe min).
        for j in &affixed {
            for p in &j.props {
                let hi = if p.desc.max == 0 { p.desc.min } else { p.desc.max };
                assert!(
                    p.value >= p.desc.min.min(hi) && p.value <= p.desc.min.max(hi),
                    "{:?} hors bornes [{}, {}]",
                    p.desc,
                    p.desc.min,
                    hi
                );
            }
        }
        // Un jewel (prefix Blanched niveau 1 : ac 5-8) inséré dans DEUX
        // hôtes porte les mêmes valeurs — zéro re-roll à l'insertion.
        let jewel = affixed.into_iter().next().unwrap();
        let expected: Vec<StatLine> = jewel
            .props
            .iter()
            .flat_map(|p| {
                item_props::evaluate(&p.desc, p.value)
                    .emits
                    .iter()
                    .filter(|e| e.stat_id >= 0)
                    .map(|e| StatLine::new(e.stat_id, e.value))
                    .collect::<Vec<_>>()
            })
            .collect();
        let mut host1 = SocketedItem::new("crs", "swor", 2, 6);
        let mut host2 = SocketedItem::new("uit", "shie", 1, 4);
        insert_socket(&mut host1, 0, SocketContent::Jewel(jewel.clone())).unwrap();
        insert_socket(&mut host2, 0, SocketContent::Jewel(jewel)).unwrap();
        assert_eq!(socket_stat_lines(&host1), expected);
        assert_eq!(socket_stat_lines(&host2), expected);
        // L'ordre d'insertion est conservé et les jewels n'entrent pas
        // dans ordered_runes (contrat runes uniquement).
        insert_socket(&mut host1, 1, SocketContent::gem("r08")).unwrap();
        assert_eq!(ordered_runes(&host1), vec!["r08"]);
    }

    // --- Agrégation vers statlist (base + affixes + insertions) ---

    /// Agrégation : les bonus d'insertions se SOMMENT avec le reste
    /// (héros + items) dans UNE [`StatList`] — Monarch (`uit`, shie)
    /// + Perfect Amethyst (shield : ac 30) + jewel str 5 + helm Great
    /// Helm (`ghm`) + gpv (helm : str 10) ; Ral dans une arme : paire
    /// fire 5-30. Déterminisme champ par champ (mission ④).
    #[test]
    fn socket_stat_lines_aggregate_into_statlist() {
        let mut shield = SocketedItem::new("uit", "shie", 2, 4);
        insert_socket(&mut shield, 0, SocketContent::gem("gpv")).unwrap();
        let mut jewel = Jewel { name: "Fine Jewel".into(), props: vec![] };
        jewel.props.push(RolledProp {
            desc: PropDescriptor { code: "str".into(), param: String::new(), min: 5, max: 5 },
            value: 5,
            typed: item_props::interpret(
                &PropDescriptor { code: "str".into(), param: String::new(), min: 5, max: 5 },
                5,
            ),
        });
        insert_socket(&mut shield, 1, SocketContent::Jewel(jewel)).unwrap();
        let mut helm = SocketedItem::new("ghm", "helm", 1, 3);
        insert_socket(&mut helm, 0, SocketContent::gem("gpv")).unwrap();
        let mut sword = SocketedItem::new("crs", "swor", 1, 6);
        insert_socket(&mut sword, 0, SocketContent::gem("r08")).unwrap();

        // Héros fictif : str 25 (BA niv 1 ≈ charstats) + 3 insertions.
        let hero = vec![StatLine::new(STRENGTH, 25)];
        let total = StatList::from_sources([
            hero,
            socket_stat_lines(&shield),
            socket_stat_lines(&helm),
            socket_stat_lines(&sword),
        ]);
        assert_eq!(total.strength(), 25 + 5 + 10, "jewel 5 + gpv helm 10 + base 25");
        assert_eq!(total.armor_class(), 30, "gpv shieldMod1 ac 30 (ligne 6)");
        let fire_min = isc("firemindam");
        let fire_max = isc("firemaxdam");
        assert_eq!(total.get(fire_min), 5, "Ral weapon mindam (ligne 44)");
        assert_eq!(total.get(fire_max), 30, "Ral weapon maxdam (ligne 44)");
    }

    /// Déterminisme global (mission ④) : la même séquence PURE
    /// (insertion/retrait/agrégation — aucun tirage) donne EXACTEMENT le
    /// même état et les mêmes lignes de stats.
    #[test]
    fn socketing_is_deterministic() {
        let run = || {
            let mut host = SocketedItem::new("utp", "tors", 3, 4);
            insert_socket(&mut host, 0, SocketContent::gem("r31")).unwrap();
            insert_socket(&mut host, 1, SocketContent::gem("r06")).unwrap();
            insert_socket(&mut host, 2, SocketContent::gem("r30")).unwrap();
            let runes = ordered_runes(&host);
            remove_socket(&mut host, 2).unwrap();
            let lines = socket_stat_lines(&host);
            (host, runes, lines)
        };
        let a = run();
        let b = run();
        assert_eq!(a.0, b.0, "état identique");
        assert_eq!(a.1, b.1, "séquence de runes identique");
        assert_eq!(a.2, b.2, "lignes de stats identiques");
    }
}
