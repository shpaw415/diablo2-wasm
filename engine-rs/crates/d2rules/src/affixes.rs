//! `affixes` — affixes magiques `MagicPrefix.txt` / `MagicSuffix.txt` v1.12.
//!
//! Étape 7b « loot D2 réel » : le module couvre TOUTE la sélection d'affixes
//! (les drops/intégration restent chez le coéquipier Qualites, `item_roll.rs`).
//!
//! Port Rust des filtres D2 réels (réf. od2 `item_factory.go:324
//! FindMatchingAffixes` + `item_affix_loader.go`, complétés par les règles
//! v1.12 documentées — od2 oublie `spawnable`, `maxlevel` et le tirage
//! pondéré) :
//!
//! * **Chargement** : via le parseur TSV runtime [`crate::tsv::Tsv`] (CRLF,
//!   quotes Excel, en-têtes nommés). Tables embarquées : magicprefix (638
//!   records nommés), magicsuffix (742), automagic (36), rareprefix (46),
//!   raresuffix (155 — copies `assets/`, sources `d2w-re/tables/items-ui/`
//!   en lecture seule ; le FICHIER fait foi sur les comptes du brief).
//! * **Filtre d'un affixe** pour une base de niveau effectif `alvl` :
//!   `spawnable = 1`, `version` acceptée (v1.12 : `0` = classic et `100` =
//!   patch VIVANTS dans le même fichier — « toBeSpawned » de fait, accepté
//!   tel quel, champ [`AffixRec::version`] informatif), `level ≤ alvl ≤
//!   maxlevel` (0 = pas de plafond), base couverte par un `itype1-7` (type
//!   propre ou famille `weap`/`mele`/`bow`/`armo`), aucun `etype1-3` ne la
//!   couvre. `alvl = max(qlvl, ilvl)` (règle D2 magic/rare).
//! * **Choix pondéré** par `frequency` (0 ⇒ poids 1), groupes `group`
//!   EXCLUSIFS (jamais deux affixes du même groupe sur un item).
//! * **Magic** : 1-3 prefixes + 1-3 suffixes MAX ([`roll_magic`], tirage
//!   uniforme 0-3 par côté, plancher 1 affixe total — un item magique porte
//!   toujours un affixe ; le NOM affiché = premier prefix + premier suffixe,
//!   règle retail). Le tirage historique [`pick_magic`] (≤ 1 + 1, port
//!   Diablerie `rand(4)`) reste branché sur la chaîne `loot`/`item_roll`.
//! * **Rare** : 3-6 affixes ([`roll_rare`], marqués `rare`, groupes
//!   exclusifs TOUS types confondus) + NOM RARE (`rareprefix` +
//!   `raresuffix` fr, agreement de genre via les clés TBL `<mot>UM`).
//! * **AutoMagic** : 36 records v1.12 ([`automagic_pool`]/[`roll_automagic`])
//!   — mods automatiques des bases à colonne `automagic` (orbes/baguettes,
//!   gamble/craft). Encodage d'id ADDITIF : `id ≥ 200_000` = rang dans
//!   AutoMagic ([`AUTOMAGIC_ID_OFFSET`]) — le contrat gelé 7a
//!   (`id < 200_000` = prefix/suffix, résolution [`resolve_affix_ext`])
//!   reste inchangé ; branchement drop = décision du coéquipier.
//! * **Sortie** : tuples contractuels `(id affixe, valeur)` (encodage
//!   `item_roll::RolledItem::affix_ids`, rangs 1-based / offset 100_000) +
//!   expansion `id ItemStatCost` v1.12 ([`stat_ids_of`] : id numérique du
//!   dictionnaire `patch_itemstatcost.txt`, valeur roulée, param) + nom FR
//!   composé ([`name_fr`]/[`rare_name_fr`] — TBL STRING fr réelles :
//!   « Hachette irrégulière de Santé », « Morsure mortelle »).
//! * **Coloration** : `transform`/`transformcolor` lus sur la rec
//!   ([`transform_of`] — contrat coéquipier).
//!
//! Zéro IO : tables embarquées (`include_str!`), déterminisme via [`Rng`].

use crate::item_props::{self, descriptor, PropDescriptor};
use crate::rng::Rng;
use crate::stats::StatCostTable;
use crate::tbl::TblSet;

/// Prefix ou suffix (tables séparées, groupes distincts chez od2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AffixKind {
    /// `MagicPrefix.txt`.
    Prefix,
    /// `MagicSuffix.txt`.
    Suffix,
}

/// Un affixe (ligne de `magicprefix`/`magicsuffix`, port
/// `item_affix_record.go::ItemAffixCommonRecord` — champs utiles au drop).
#[derive(Debug, Clone)]
pub struct AffixRec {
    /// Nom affichable anglais (`Sturdy`, `of Health`…) — CLE TBL : le texte
    /// fr se résout par ce nom dans les tables STRING (`Sturdy` → « solide »,
    /// variantes `[fs][ms][fp][mp]`).
    pub name: String,
    /// Prefix ou suffix.
    pub kind: AffixKind,
    /// Colonne `version` v1.12 (`0` = classic, `100` = patch — les deux
    /// vivants en 1.12 ; informatif, le filtre accepte tout).
    pub version: u32,
    /// `spawnable = 1` (sinon jamais tiré).
    pub spawnable: bool,
    /// Peut apparaître sur un rare (`rare`).
    pub rare: bool,
    /// Niveau minimal (`level ≤ alvl`).
    pub level: u32,
    /// Niveau maximal (0/vide = aucun).
    pub maxlevel: u32,
    /// Niveau requis porté sur l'item (`levelreq`).
    pub levelreq: u32,
    /// Poids de tirage (`frequency`, 0 ⇒ 1).
    pub frequency: u32,
    /// Groupe d'exclusion mutuelle (`group`).
    pub group: i32,
    /// Jusqu'à 3 modificateurs (`modNcode/param/min/max`).
    pub mods: Vec<PropDescriptor>,
    /// Coloration (`transform` + `transformcolor`).
    pub transform: bool,
    pub transformcolor: String,
    /// Types inclus (`itype1-7`).
    pub itypes: Vec<String>,
    /// Types exclus (`etype1-3`).
    pub etypes: Vec<String>,
}

/// Un mot de nom RARE (ligne de `rareprefix.txt`/`raresuffix.txt` v1.12).
/// Ces tables n'ont ni niveau ni fréquence : filtres de TYPE seulement,
/// tirage uniforme. Le texte fr = clé TBL (`<name>UM` si présent — formes
/// accordées — sinon `<name>`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RareWord {
    /// Mot table (`Doom`, `bite`… — clé TBL fr directe).
    pub name: String,
    /// Types inclus (`itype1-7`).
    pub itypes: Vec<String>,
    /// Types exclus (`etype1-4`).
    pub etypes: Vec<String>,
}

impl RareWord {
    /// Le mot peut-il être tiré pour cette base ? (filtre `itype`/`etype`.)
    pub fn covers(&self, types: &ItemTypeSet) -> bool {
        (!self.itypes.is_empty() && !self.itypes.iter().any(|t| types.covers(t)))
            || self.etypes.iter().any(|t| types.covers(t))
    }
}

/// Tables d'affixes v1.12 embarquées.
#[derive(Debug, Clone, Default)]
pub struct AffixTables {
    /// `MagicPrefix.txt` (spawnables et non-spawnables, filtrés au tirage).
    pub prefixes: Vec<AffixRec>,
    /// `MagicSuffix.txt`.
    pub suffixes: Vec<AffixRec>,
    /// `AutoMagic.txt` (mods automatiques — sélection ici, branchement drop
    /// chez le coéquipier).
    pub automagics: Vec<AffixRec>,
    /// `rareprefix.txt` (46 records — premier mot du nom rare, adjectif fr).
    pub rareprefixes: Vec<RareWord>,
    /// `raresuffix.txt` (155 records — nom du rare, substantif fr).
    pub raresuffixes: Vec<RareWord>,
}

/// Offset des ids AUTOMAGIC dans l'encodage d'affixes (extension additive
/// étape 7b : le contrat gelé `item_roll` réserve `id < 200_000` ; une id
/// `200_000 + rang 1-based` résout via [`resolve_affix_ext`] seulement —
/// `item_roll::resolve_affix` la laisse à `None`, sans ambiguïté).
pub const AUTOMAGIC_ID_OFFSET: u32 = 200_000;

/// Ensemble des types couverts par une base (pour `itype`/`etype`) :
/// type propre (`swor`, `tors`…) + familles d'équivalence connues de
/// [`crate::loot`].
#[derive(Debug, Clone, Copy, Default)]
pub struct ItemTypeSet {
    /// Type propre de la base (`axe`, `shie`, `amul`…).
    pub wtype: &'static str,
    /// Dans la famille `weap` (toute arme).
    pub is_weap: bool,
    /// Dans la famille `mele` (arme de mêlée).
    pub is_mele: bool,
    /// Dans la famille `bow` (arc/arbalète).
    pub is_bow: bool,
    /// Dans la famille `armo` (toute armure).
    pub is_armo: bool,
}

impl ItemTypeSet {
    /// La base couvre-t-elle le type `t` d'un `itype`/`etype` ?
    pub fn covers(&self, t: &str) -> bool {
        t == self.wtype
            || (self.is_weap && t == "weap")
            || (self.is_mele && t == "mele")
            || (self.is_bow && t == "bow")
            || (self.is_armo && t == "armo")
    }
}

/// rareprefix.txt / raresuffix.txt v1.12 (copies `assets/`, sources
/// `d2w-re/tables/items-ui/` — lecture seule, fichier fait foi).
const RARE_PREFIX_TXT: &str = include_str!("../../../../assets/rareprefix.txt");
const RARE_SUFFIX_TXT: &str = include_str!("../../../../assets/raresuffix.txt");

/// Parse les deux tables d'affixes (+ AutoMagic + mots rares embarqués) ;
/// `None` si aucune ligne exploitable (les drops retombent alors sans
/// affixe). Signature inchangée (consommateurs : `loot`, `vendor`,
/// `item_roll`) — chargement passé sur [`crate::tsv::Tsv`] (étape 7b).
pub fn parse(prefix_txt: &str, suffix_txt: &str, automagic_txt: &str) -> Option<AffixTables> {
    let prefixes = parse_affix_file(prefix_txt, AffixKind::Prefix);
    let suffixes = parse_affix_file(suffix_txt, AffixKind::Suffix);
    let automagics = parse_affix_file(automagic_txt, AffixKind::Prefix);
    (prefixes.len() + suffixes.len() > 0).then_some(AffixTables {
        prefixes,
        suffixes,
        automagics,
        rareprefixes: parse_rare_words(RARE_PREFIX_TXT),
        raresuffixes: parse_rare_words(RARE_SUFFIX_TXT),
    })
}

/// Parse une table d'affixes via [`crate::tsv::Tsv`] (colonnes v1.12, paires
/// `modN*` et `itypeN`/`etypeN` découvertes depuis l'en-tête ; les lignes
/// drapeaux « expansion » sans nom sont exclues — ordre fichier conservé,
/// contrat d'encodage des ids `item_roll`).
fn parse_affix_file(txt: &str, kind: AffixKind) -> Vec<AffixRec> {
    let Ok(t) = crate::tsv::Tsv::parse(txt) else { return Vec::new() };
    if t.column("Name").is_none() || t.column("level").is_none() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for i in 0..t.len() {
        let r = t.rec(i).unwrap();
        let name = r.s("Name");
        if name.is_empty() {
            continue; // ligne drapeau « expansion » des MPQ
        }
        let mut mods = Vec::new();
        for n in 1..=3 {
            let cells = [
                r.s(&format!("mod{n}code")),
                r.s(&format!("mod{n}param")),
                r.s(&format!("mod{n}min")),
                r.s(&format!("mod{n}max")),
            ];
            if let Some(d) = descriptor(cells) {
                mods.push(d);
            }
        }
        out.push(AffixRec {
            name: name.to_string(),
            kind,
            version: r.u("version"),
            spawnable: r.u("spawnable") == 1,
            rare: r.u("rare") == 1,
            level: r.u("level"),
            maxlevel: r.u("maxlevel"),
            levelreq: r.u("levelreq"),
            frequency: r.u("frequency"),
            group: r.i("group"),
            mods,
            transform: r.u("transform") == 1,
            transformcolor: r.s("transformcolor").to_string(),
            itypes: (1..=7)
                .map(|n| r.s(&format!("itype{n}")).to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            etypes: (1..=3)
                .map(|n| r.s(&format!("etype{n}")).to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        });
    }
    out
}

/// Parse une table de mots rares (`name/version/itype1-7/etype1-4`).
fn parse_rare_words(txt: &str) -> Vec<RareWord> {
    let Ok(t) = crate::tsv::Tsv::parse(txt) else { return Vec::new() };
    let mut out = Vec::new();
    for i in 0..t.len() {
        let r = t.rec(i).unwrap();
        let name = r.s("name");
        if name.is_empty() {
            continue;
        }
        out.push(RareWord {
            name: name.to_string(),
            itypes: (1..=7)
                .map(|n| r.s(&format!("itype{n}")).to_string())
                .filter(|s| !s.is_empty())
                .collect(),
            etypes: (1..=4)
                .map(|n| r.s(&format!("etype{n}")).to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        });
    }
    out
}

/// Niveau effectif d'affixe d'un drop : `alvl = max(qlvl, ilvl)` (règle D2
/// pour magic/rare ; crafted = cube, restant).
pub fn alvl(qlvl: u32, ilvl: u32) -> u32 {
    qlvl.max(ilvl)
}

/// L'affixe peut-il apparaître sur la base ? (`spawnable` déjà testé par le
/// filtrage ; port od2 `FindMatchingAffixes` + fenêtre `level/maxlevel`.)
pub fn matches(affix: &AffixRec, types: &ItemTypeSet, level: u32) -> bool {
    if !affix.spawnable || level < affix.level {
        return false;
    }
    if affix.maxlevel > 0 && level > affix.maxlevel {
        return false;
    }
    if !affix.itypes.is_empty() && !affix.itypes.iter().any(|t| types.covers(t)) {
        return false;
    }
    if affix.etypes.iter().any(|t| types.covers(t)) {
        return false;
    }
    true
}

/// Pools filtrés (prefixes, suffixes) pour une base — matière première des
/// tirages étape 7b ([`roll_magic`]/[`roll_rare`]/[`roll_automagic`]).
pub fn pools<'a>(tables: &'a AffixTables, types: &ItemTypeSet, level: u32) -> AffixPools<'a> {
    AffixPools {
        prefixes: tables.prefixes.iter().filter(|a| matches(a, types, level)).collect(),
        suffixes: tables.suffixes.iter().filter(|a| matches(a, types, level)).collect(),
    }
}

/// Pools filtrés d'un niveau (voir [`pools`]).
pub struct AffixPools<'a> {
    /// Prefixes spawnables pour la base à ce niveau.
    pub prefixes: Vec<&'a AffixRec>,
    /// Suffixes spawnables pour la base à ce niveau.
    pub suffixes: Vec<&'a AffixRec>,
}

/// Tirage pondéré par `frequency` parmi des candidats déjà filtrés.
/// Retourne l'index dans `cands` (None si vide). Un tirage [`Rng`].
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

/// Item MAGIQUE : au plus 1 prefix ET 1 suffix (port du tirage Diablerie
/// `GenerateItemProperties` : `rand(4)` — `<2` ⇒ prefix, `!= 1` ⇒ suffix).
/// Filtrage `rare = 0` imposé ? Non : v1.12, les affixes `rare` restent
/// tirables sur magic (Diablerie `affix.rare || quality != Rare`).
/// Déterministe : 1 à 2 tirages [`Rng`]. TIRAGE HISTORIQUE — branché sur
/// `loot`/`vendor`/`item_roll` ; la version « 1-3 + 1-3 » est [`roll_magic`].
pub(crate) fn pick_magic<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
    level: u32,
) -> (Option<&'a AffixRec>, Option<&'a AffixRec>) {
    let roll = rng.below(4);
    let prefixes: Vec<&AffixRec> = tables.prefixes.iter().filter(|a| matches(a, types, level)).collect();
    let suffixes: Vec<&AffixRec> = tables.suffixes.iter().filter(|a| matches(a, types, level)).collect();
    let prefix = (roll < 2).then(|| weighted(rng, &prefixes)).unwrap_or(None);
    let suffix = (roll != 1).then(|| weighted(rng, &suffixes)).unwrap_or(None);
    (prefix, suffix)
}

/// Item RARE : 4 affixes (tous types), exclusion de groupe mutuelle,
/// affixes marqués `rare` uniquement (règle v1.12). 1 à 4 tirages [`Rng`].
/// TIRAGE HISTORIQUE — branché sur `loot`/`vendor`/`item_roll` ; la version
/// « 3-6 » du brief 7b est [`roll_rare`].
pub(crate) fn pick_rare<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
    level: u32,
) -> Vec<&'a AffixRec> {
    let mut used_groups: Vec<i32> = Vec::new();
    let mut out: Vec<&'a AffixRec> = Vec::new();
    let candidates: Vec<&AffixRec> = tables
        .prefixes
        .iter()
        .chain(tables.suffixes.iter())
        .filter(|a| matches(a, types, level) && a.rare)
        .collect();
    // Un tirage par slot ; groupe déjà pris ⇒ ce slot reste libre (D2
    // re-tire, mais garde déterministe équivalente en flux ici, écart mineur).
    for _ in 0..4 {
        let pool: Vec<&AffixRec> =
            candidates.iter().copied().filter(|a| !used_groups.contains(&a.group)).collect();
        let Some(picked) = weighted(rng, &pool) else { continue };
        if !used_groups.contains(&picked.group) {
            used_groups.push(picked.group);
            out.push(picked);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Étape 7b — sélection complète (1-3/1-3, rare 3-6, AutoMagic, noms)
// ---------------------------------------------------------------------------

/// Tirage d'un item MAGIQUE (règle D2 complète, brief 7b) : **1-3 prefixes +
/// 1-3 suffixes max**, pondérés `frequency`, groupes exclusifs par côté.
/// Flux [`Rng`] gelé : ① nb prefixes `0-3` ② nb suffixes `0-3` (plancher
/// « ≥ 1 affixe total » : les deux à 0 ⇒ 1 prefix) ③ picks prefix ④ picks
/// suffix. Le nom affiché = PREMIER prefix + PREMIER suffix (règle retail).
pub fn roll_magic<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
    level: u32,
) -> MagicRoll<'a> {
    let pool = pools(tables, types, level);
    let n_prefix = rng.below(4) as usize;
    let mut n_suffix = rng.below(4) as usize;
    let mut prefixes = Vec::new();
    let mut used_p: Vec<i32> = Vec::new();
    for _ in 0..n_prefix {
        let cands: Vec<&AffixRec> =
            pool.prefixes.iter().copied().filter(|a| !used_p.contains(&a.group)).collect();
        let Some(a) = weighted(rng, &cands) else { break };
        used_p.push(a.group);
        prefixes.push(a);
    }
    if prefixes.is_empty() {
        n_suffix = n_suffix.max(1); // un item magique porte au moins 1 affixe
    }
    let mut suffixes = Vec::new();
    let mut used_s: Vec<i32> = Vec::new();
    for _ in 0..n_suffix {
        let cands: Vec<&AffixRec> =
            pool.suffixes.iter().copied().filter(|a| !used_s.contains(&a.group)).collect();
        let Some(a) = weighted(rng, &cands) else { break };
        used_s.push(a.group);
        suffixes.push(a);
    }
    MagicRoll { prefixes, suffixes }
}

/// Résultat d'un tirage MAGIQUE ([`roll_magic`]).
#[derive(Debug, Clone)]
pub struct MagicRoll<'a> {
    /// 0-3 prefixes tirés (groupes exclusifs, ordre de tirage).
    pub prefixes: Vec<&'a AffixRec>,
    /// 0-3 suffixes tirés (groupes exclusifs, ordre de tirage).
    pub suffixes: Vec<&'a AffixRec>,
}

/// Tirage d'un item RARE (règle D2 complète, brief 7b) : **3-6 affixes**,
/// marqués `rare`, groupes exclusifs TOUS types confondus, pondérés
/// `frequency`. Flux [`Rng`] gelé : ① nb `3 + 0..4` ② picks (un par slot,
/// slot à pool épuisé = vide).
pub fn roll_rare<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
    level: u32,
) -> Vec<&'a AffixRec> {
    let n = 3 + rng.below(4) as usize;
    let mut used_groups: Vec<i32> = Vec::new();
    let candidates: Vec<&AffixRec> = tables
        .prefixes
        .iter()
        .chain(tables.suffixes.iter())
        .filter(|a| matches(a, types, level) && a.rare)
        .collect();
    let mut out = Vec::new();
    for _ in 0..n {
        let pool: Vec<&AffixRec> =
            candidates.iter().copied().filter(|a| !used_groups.contains(&a.group)).collect();
        let Some(a) = weighted(rng, &pool) else { break };
        used_groups.push(a.group);
        out.push(a);
    }
    out
}

/// Pool AUTOMAGIC applicable à une base ([`AutoMagic.txt`] v1.12 : filtres
/// `itype`/`etype` + fenêtre `level/maxlevel`, 36 records). L'application
/// réelle au drop (orbes/baguettes, colonne `automagic` des bases) est le
/// branchement du coéquipier — le contrat `affix_ids` gelé ne porte pas
/// d'automod (encodage additif [`AUTOMAGIC_ID_OFFSET`] prêt).
pub fn automagic_pool<'a>(tables: &'a AffixTables, types: &ItemTypeSet, level: u32) -> Vec<&'a AffixRec> {
    tables.automagics.iter().filter(|a| matches(a, types, level)).collect()
}

/// Tire UN automod (pondéré `frequency`, uniforme à défaut). Un tirage
/// [`Rng`]. Orbes/baguettes : l'appelant décide de l'application.
pub fn roll_automagic<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
    level: u32,
) -> Option<&'a AffixRec> {
    weighted(rng, &automagic_pool(tables, types, level))
}

/// Id d'une rec dans son encodage contractuel (identité de pointeur) :
/// prefix `1..N`, suffix `100_000 + rang`, automagic `200_000 + rang`
/// ([`AUTOMAGIC_ID_OFFSET`]). `None` si la rec n'est pas des tables.
pub fn row_id(tables: &AffixTables, rec: &AffixRec) -> Option<u32> {
    if let Some(i) = tables.prefixes.iter().position(|a| std::ptr::eq(a, rec)) {
        return Some(i as u32 + 1);
    }
    if let Some(i) = tables.suffixes.iter().position(|a| std::ptr::eq(a, rec)) {
        return Some(i as u32 + 1 + crate::item_roll::SUFFIX_ID_OFFSET);
    }
    tables
        .automagics
        .iter()
        .position(|a| std::ptr::eq(a, rec))
        .map(|i| i as u32 + 1 + AUTOMAGIC_ID_OFFSET)
}

/// Résolution ÉTENDUE d'une id d'`affix_ids` : prefix / suffix (encodage
/// `item_roll::resolve_affix`) + automagic (`id ≥ 200_000`). `(rec, kind)`.
pub fn resolve_affix_ext<'a>(tables: &'a AffixTables, id: u32) -> Option<(&'a AffixRec, AffixKind)> {
    if let Some(i) = id.checked_sub(AUTOMAGIC_ID_OFFSET) {
        return tables
            .automagics
            .get(i.checked_sub(1)? as usize)
            .map(|a| (a, AffixKind::Prefix));
    }
    crate::item_roll::resolve_affix(tables, id)
}

// ---------------------------------------------------------------------------
// Ids ItemStatCost (brief 7b : « affix_ids (id ItemStatCost + valeur,
// + dur param) »)
// ---------------------------------------------------------------------------

/// Un mod roulé déplié en stat D2 : `stat_id` = id numérique ItemStatCost
/// v1.12 (colonne `ID` — encodage des saves binaires), `value` = valeur
/// roulée, `param` = colonne `modNparam` (skill/state, 0 à défaut).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AffixStatId {
    /// Id ItemStatCost (`patch_itemstatcost.txt` col `ID`).
    pub stat_id: i32,
    /// Valeur roulée (telle quelle — un même tuple contractuel peut
    /// déplier plusieurs stats, ex. `dmg%` ⇒ min% + max%).
    pub value: i32,
    /// Param brut (`modNparam`, 0 si vide).
    pub param: i32,
}

/// Déplie des tuples contractuels `(id affixe, valeur)` en ids
/// ItemStatCost. Ordre : par tuple, mods dans l'ordre de la ligne, stats
/// dans l'ordre du mapping. Codes v1.12 non mappés ⇒ `stat_id = -1`
/// (jamais perdu ; le golden test garantit que TOUT le vocabulaire réel
/// des tables est mappé, donc `-1` n'arrive pas sur les données v1.12).
pub fn stat_ids_of(tables: &AffixTables, affix_ids: &[(u32, u32)]) -> Vec<AffixStatId> {
    let isc = StatCostTable::global();
    let mut out = Vec::new();
    for &(id, value) in affix_ids {
        let Some((rec, _)) = resolve_affix_ext(tables, id) else { continue };
        for d in &rec.mods {
            let param = d.param.trim().parse::<i32>().unwrap_or(0);
            for name in prop_stat_names(&d.code) {
                let stat_id = isc.get(name).map(|r| r.id).unwrap_or(-1);
                out.push(AffixStatId { stat_id, value: value as i32, param });
            }
        }
    }
    out
}

/// Mapping code propriété (`properties.txt` v1.12 — colonnes `modNcode`
/// des tables d'affixes) → nom(s) de stat `ItemStatCost.txt`. Port du
/// vocabulaire RÉEL des tables embarquées (105 codes distincts, golden
/// testé exhaustivement) ; les codes à double stat (ex. `dmg%`) déplient
/// les deux bornes D2, `lifesteal`/`manasteal` portent les drain min+max,
/// `res-all` les 4 résistances, les classes (`sor`…`ass`) et `skilltab`
/// les stats « +skills » paramétrées, `charged`/`att-skill` les stats
/// skill paramétrées (param = id de skill), et les codes `/lvl`+`/time`
/// leurs stats dédiées `_perlevel`/`_bytime` de l'ISC v1.12.
fn prop_stat_names(code: &str) -> &'static [&'static str] {
    match code {
        "ac" => &["armorclass"],
        "ac%" => &["item_armor_percent"],
        "ac%/lvl" => &["item_armorpercent_perlevel"],
        "ac/lvl" => &["item_armor_perlevel"],
        "ac/time" => &["item_armor_bytime"],
        // Absorptions des suffixes d'anneaux (« of Amianthus »…) — variante
        // pour cent de l'ISC v1.12 (les tables ne portent pas de codes
        // `abs-*%` ; l'affichage % vs plat est un choix UI documenté ici).
        "abs-cold" => &["item_absorbcold_percent"],
        "abs-fire" => &["item_absorbfire_percent"],
        "abs-ltng" => &["item_absorblight_percent"],
        "ama" | "ass" | "bar" | "dru" | "nec" | "pal" | "sor" => &["item_addclassskills"],
        "att" => &["tohit"],
        "att%" => &["item_tohit_percent"],
        "att%/lvl" => &["item_tohitpercent_perlevel"],
        "att/lvl" => &["item_tohit_perlevel"],
        "att-demon" => &["item_demon_tohit"],
        "att-skill" => &["item_singleskill"],
        "att-undead" => &["item_undead_tohit"],
        "balance1" | "balance2" | "balance3" => &["item_fastergethitrate"],
        "block" | "block2" => &["toblock"],
        "cast1" | "cast3" => &["item_fastercastrate"],
        "charged" => &["item_charged_skill"],
        "cold-len" => &["coldlength"],
        "cold-max" => &["coldmaxdam"],
        "cold-min" => &["coldmindam"],
        "dex" => &["dexterity"],
        "dex/lvl" => &["item_dexterity_perlevel"],
        "dmg%" => &["item_mindamage_percent", "item_maxdamage_percent"],
        "dmg%/lvl" => &["item_maxdamage_percent_perlevel"],
        "dmg-ac" => &["item_fractionaltargetac"],
        "dmg-cold" => &["coldmindam", "coldmaxdam"],
        "dmg-demon" => &["item_demondamage_percent"],
        "dmg-fire" => &["firemindam", "firemaxdam"],
        "dmg-ltng" => &["lightmindam", "lightmaxdam"],
        "dmg-min" => &["mindamage"],
        "dmg-max" => &["maxdamage"],
        "dmg-pois" => &["poisonmindam", "poisonmaxdam"],
        "dmg-to-mana" => &["item_damagetomana"],
        "dmg-undead" => &["item_undeaddamage_percent"],
        "dmg/lvl" => &["item_maxdamage_perlevel"],
        "ease" => &["item_req_percent"],
        "enr" => &["energy"],
        "fire-max" => &["firemaxdam"],
        "fire-min" => &["firemindam"],
        "gethit-skill" => &["item_skillongethit"],
        "gold%" => &["item_goldbonus"],
        "half-freeze" => &["item_halffreezeduration"],
        "hit-skill" => &["item_skillonhit"],
        "howl" => &["item_howl"],
        "hp" => &["maxhp"],
        "hp/lvl" => &["item_hp_perlevel"],
        "ignore-ac" => &["item_ignoretargetac"],
        "indestruct" => &["item_indesctructible"],
        "knock" => &["item_knockback"],
        "lifesteal" => &["lifedrainmindam", "lifedrainmaxdam"],
        "light" => &["item_lightradius"],
        "ltng-max" => &["lightmaxdam"],
        "ltng-min" => &["lightmindam"],
        "mag%" => &["item_magicbonus"],
        "mana" => &["maxmana"],
        "mana-kill" => &["item_manaafterkill"],
        "mana/lvl" => &["item_mana_perlevel"],
        "manasteal" => &["manadrainmindam", "manadrainmaxdam"],
        "move1" | "move2" | "move3" => &["item_fastermovevelocity"],
        "noheal" => &["item_preventheal"],
        "pois-len" => &["poisonlength"],
        "pois-max" => &["poisonmaxdam"],
        "pois-min" => &["poisonmindam"],
        "red-dmg" => &["normal_damage_reduction"],
        "red-mag" => &["magic_damage_reduction"],
        "regen" => &["hpregen"],
        "regen-stam" => &["staminarecoverybonus"],
        "rep-dur" => &["item_replenish_durability"],
        "rep-quant" => &["item_replenish_quantity"],
        "res-all" => &["fireresist", "lightresist", "coldresist", "poisonresist"],
        "res-cold" => &["coldresist"],
        "res-cold/lvl" => &["item_resist_cold_perlevel"],
        "res-fire" => &["fireresist"],
        "res-fire/lvl" => &["item_resist_fire_perlevel"],
        "res-ltng" => &["lightresist"],
        "res-ltng/lvl" => &["item_resist_ltng_perlevel"],
        "res-pois" => &["poisonresist"],
        "res-pois-len" => &["item_poisonlengthresist"],
        "res-pois/lvl" => &["item_resist_pois_perlevel"],
        "skilltab" => &["item_addskill_tab"],
        "sock" => &["item_numsockets"],
        "stack" => &["item_extra_stack"],
        "stam" => &["maxstamina"],
        "stamdrain" => &["item_staminadrainpct"],
        "str" => &["strength"],
        "str/lvl" => &["item_strength_perlevel"],
        "swing1" | "swing2" | "swing3" => &["item_fasterattackrate"],
        "thorns" => &["item_attackertakesdamage"],
        _ => &[],
    }
}

// ---------------------------------------------------------------------------
// Noms FR — TBL STRING fr réelles (`tbl.rs`, patch > expansion > string).
// ---------------------------------------------------------------------------

/// Une chaîne TBL fr à variantes de genre/nombre `[fs]x[ms]y[fp]z[mp]w`
/// (les affixes/bases/mots rares fr utilisent ce format) ; les chaînes
/// sans balise sont invariables (`plain`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Gendered {
    /// Forme féminin singulier.
    pub fs: Option<String>,
    /// Forme masculin singulier.
    pub ms: Option<String>,
    /// Forme féminin pluriel.
    pub fp: Option<String>,
    /// Forme masculin pluriel.
    pub mp: Option<String>,
    /// Texte sans balise (invariable).
    pub plain: String,
    /// Ordre d'apparition des balises (0=fs 1=ms 2=fp 3=mp) — la PREMIÈRE
    /// variante d'une chaîne portant une seule forme singulière donne le
    /// genre du substantif (« [fs]Hachette » = féminin).
    pub order: Vec<u8>,
}

const TAGS: [(&str, u8); 4] = [("[fs]", 0), ("[ms]", 1), ("[fp]", 2), ("[mp]", 3)];

impl Gendered {
    /// Chaîne invariable.
    fn plain(s: &str) -> Gendered {
        Gendered { plain: s.to_string(), ..Default::default() }
    }

    /// Première variante présente (ordre d'apparition), sinon le texte brut.
    pub fn first(&self) -> &str {
        for &w in &self.order {
            let f = [&self.fs, &self.ms, &self.fp, &self.mp][w as usize];
            if let Some(s) = f {
                return s;
            }
        }
        &self.plain
    }

    /// Forme accordée (féminin, singulier au MVP), sinon [`Self::first`].
    pub fn agree(&self, fem: bool) -> &str {
        let want = if fem { 0 } else { 1 };
        if self.order.contains(&want) {
            let f = [&self.fs, &self.ms, &self.fp, &self.mp][want as usize];
            if let Some(s) = f {
                return s;
            }
        }
        self.first()
    }

    /// Genre par défaut du substantif : féminin si sa première forme est
    /// une `[fs]` seule (« [fs]Morsure », « [fs]Hachette »), sinon masculin.
    pub fn fem(&self) -> bool {
        self.order.first() == Some(&0) && !self.order.contains(&1)
    }
}

/// Parse les balises `[fs][ms][fp][mp]` d'une chaîne TBL fr.
pub fn parse_gendered(raw: &str) -> Gendered {
    let mut g = Gendered::default();
    let mut rest = raw;
    let mut head = String::new();
    loop {
        let hit = TAGS.iter().filter_map(|(t, w)| rest.find(t).map(|i| (i, *w, t))).min_by_key(|(i, _, _)| *i);
        let Some((i, w, tag)) = hit else {
            head.push_str(rest);
            break;
        };
        head.push_str(&rest[..i]);
        if g.order.is_empty() {
            g.plain = head.clone(); // texte avant la 1re balise (souvent vide)
        }
        let after = &rest[i + tag.len()..];
        let end = TAGS
            .iter()
            .filter_map(|(t, _)| after.find(t))
            .min()
            .unwrap_or(after.len());
        let slot = match w {
            0 => &mut g.fs,
            1 => &mut g.ms,
            2 => &mut g.fp,
            _ => &mut g.mp,
        };
        if slot.is_none() {
            *slot = Some(after[..end].to_string());
            g.order.push(w);
        }
        rest = &after[end..];
    }
    if g.order.is_empty() {
        g.plain = raw.to_string();
    }
    g
}

/// Texte fr d'une clé (patch > expansion > string), `None` si absente.
fn tbl_fr(strings: &TblSet, key: &str) -> Option<String> {
    strings.get(key)
}

/// Libellé fr d'un affixe (clé = nom anglais de la ligne ; repli : nom
/// anglais brut — jamais vide).
pub fn affix_fr(strings: &TblSet, rec: &AffixRec) -> String {
    tbl_fr(strings, &rec.name).unwrap_or_else(|| rec.name.clone())
}

/// Nom MAGIQUE fr composé : `« <Base> <prefix accordé> <suffixe> »` —
/// ordre fr correct : l'adjectif-préfixe se place APRÈS le substantif de
/// la base (« Hachette irrégulière de Santé »), le suffixe s'ajoute en
/// queue (ses textes fr sont des compléments « de Santé », « du Renard »).
/// Base : clé TBL = code de la base (`hax` → « [fs]Hachette ») ; absent ⇒
/// `base_fallback` (nom anglais) invariable. Premier prefix + premier
/// suffixe tirés (règle retail d'affichage). Repli : nom de base seul.
pub fn name_fr(
    strings: &TblSet,
    base_code: &str,
    base_fallback: &str,
    prefixes: &[&AffixRec],
    suffixes: &[&AffixRec],
) -> String {
    let base_raw = tbl_fr(strings, base_code).unwrap_or_default();
    let base_g = if base_raw.is_empty() { Gendered::plain(base_fallback) } else { parse_gendered(&base_raw) };
    let mut name = base_g.first().to_string();
    if let Some(p) = prefixes.first() {
        let raw = affix_fr(strings, p);
        let pg = parse_gendered(&raw);
        let adj = if raw.starts_with("de ") || raw.starts_with("d'") { raw.clone() } else { pg.agree(base_g.fem()).to_string() };
        name = format!("{name} {adj}");
    }
    if let Some(s) = suffixes.first() {
        name = format!("{name} {}", affix_fr(strings, s).trim());
    }
    name.trim().to_string()
}

/// Tire les DEUX mots d'un nom rare (prefix adjectif + suffix substantif),
/// filtrés par type, tirage UNIFORME (ces tables n'ont pas de colonne
/// `frequency`). Flux [`Rng`] gelé : ① mot prefix ② mot suffix.
pub fn roll_rare_name<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
) -> Option<(&'a RareWord, &'a RareWord)> {
    let rp: Vec<&RareWord> = tables.rareprefixes.iter().filter(|w| w.covers(types)).collect();
    let rs: Vec<&RareWord> = tables.raresuffixes.iter().filter(|w| w.covers(types)).collect();
    if rp.is_empty() || rs.is_empty() {
        return None;
    }
    let p = &rp[rng.below(rp.len() as u32) as usize];
    let s = &rs[rng.below(rs.len() as u32) as usize];
    Some((p, s))
}

/// Nom RARE fr composé : `« <substantif> <adjectif accordé> »` — le mot du
/// `raresuffix` est le NOM (genre = sa première forme singulière, «
/// [fs]Morsure » = féminin), le mot du `rareprefix` l'ADJECTIF, lu sur la
/// clé TBL `<mot>UM` (formes accordées : `DoomUM` → « mortelle/mortel »)
/// sinon sur la clé simple (compléments « de la Bête » ajoutés tels
/// quels). Ex. `Doom` + `bite` → « Morsure mortelle ».
pub fn rare_name_fr(strings: &TblSet, prefix_word: &RareWord, suffix_word: &RareWord) -> String {
    let noun_raw = tbl_fr(strings, &suffix_word.name).unwrap_or_default();
    let noun = parse_gendered(&noun_raw);
    let fem = noun.fem();
    let adj_raw = tbl_fr(strings, &format!("{}UM", prefix_word.name))
        .or_else(|| tbl_fr(strings, &prefix_word.name))
        .unwrap_or_else(|| prefix_word.name.clone());
    let adj = if adj_raw.starts_with("de ") || adj_raw.starts_with("d'") {
        adj_raw.clone()
    } else {
        parse_gendered(&adj_raw).agree(fem).to_string()
    };
    format!("{} {adj}", noun.first()).trim().to_string()
}

/// Coloration du premier affixe transformant d'une liste d'ids
/// (`transform`/`transformcolor` v1.12 — contrat coéquipier « la
/// coloration se lit sur la ligne d'affixe résolue »). `Some(couleur)`.
pub fn transform_of(tables: &AffixTables, affix_ids: &[(u32, u32)]) -> Option<String> {
    affix_ids.iter().find_map(|&(id, _)| {
        let (rec, _) = resolve_affix_ext(tables, id)?;
        rec.transform.then(|| rec.transformcolor.clone()).filter(|c| !c.is_empty())
    })
}

/// Complète un [`RolledItem`] (contrat gelé coéquipier Qualites) pour un
/// drop MAGIC ou RARE : picks étape 7b, valeurs roulées `min..=max` par
/// mod, `affix_ids`/`sockets` (règle `sock` : `param > 0` = fixe) et
/// `name_resolved` FR. HORS chaîne de drop (branchement = `item_roll`) ;
/// `with_automagic` ajoute un automod ([`AUTOMAGIC_ID_OFFSET`]). Flux
/// [`Rng`] gelé : picks → valeurs (prefixes puis suffixes) → automod →
/// (rare) mots du nom. Qualités autres : no-op.
#[allow(clippy::too_many_arguments)]
pub fn decorate_item(
    rng: &mut Rng,
    item: &mut crate::item_roll::RolledItem,
    tables: &AffixTables,
    strings: &TblSet,
    types: &ItemTypeSet,
    base_fallback: &str,
    qlvl: u32,
    ilvl: u32,
    with_automagic: bool,
) {
    use crate::item_roll::RolledQuality;
    let level = alvl(qlvl, ilvl);
    let apply = |rng: &mut Rng, item: &mut crate::item_roll::RolledItem, recs: &[&AffixRec]| {
        for rec in recs {
            let Some(id) = row_id(tables, rec) else { continue };
            for d in &rec.mods {
                let value = item_props::roll(d, rng).value.max(0) as u32;
                if d.code == "sock" {
                    let param: u32 = d.param.trim().parse().unwrap_or(0);
                    item.sockets =
                        if param > 0 { param.min(u8::MAX as u32) as u8 } else { value.min(u8::MAX as u32) as u8 };
                }
                item.affix_ids.push((id, value));
            }
        }
    };
    match item.quality {
        RolledQuality::Magic => {
            let roll = roll_magic(rng, tables, types, level);
            let recs: Vec<&AffixRec> = roll.prefixes.iter().copied().chain(roll.suffixes.iter().copied()).collect();
            apply(rng, item, &recs);
            item.name_resolved =
                name_fr(strings, &item.code, base_fallback, &roll.prefixes, &roll.suffixes);
        }
        RolledQuality::Rare => {
            let picks = roll_rare(rng, tables, types, level);
            apply(rng, item, &picks);
            item.name_resolved = roll_rare_name(rng, tables, types)
                .map(|(p, s)| rare_name_fr(strings, p, s))
                .unwrap_or_else(|| name_fr(strings, &item.code, base_fallback, &[], &[]));
        }
        _ => return,
    }
    if with_automagic {
        if let Some(a) = roll_automagic(rng, tables, types, level) {
            apply(rng, item, &[a]);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests — tables réelles embarquées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_roll::{RolledItem, RolledQuality};

    const PREFIX: &str = include_str!("../../../../assets/patch_magicprefix.txt");
    const SUFFIX: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
    const AUTOMAGIC: &str = include_str!("../../../../assets/patch_automagic.txt");

    fn tables() -> AffixTables {
        parse(PREFIX, SUFFIX, AUTOMAGIC).expect("tables d'affixes valides")
    }

    fn strings() -> TblSet {
        TblSet::bundled_fra()
    }

    fn axe_types() -> ItemTypeSet {
        ItemTypeSet { wtype: "axe", is_weap: true, is_mele: true, is_bow: false, is_armo: false }
    }

    fn armor_types() -> ItemTypeSet {
        ItemTypeSet { wtype: "tors", is_weap: false, is_mele: false, is_bow: false, is_armo: true }
    }

    fn wand_types() -> ItemTypeSet {
        ItemTypeSet { wtype: "wand", is_weap: true, is_mele: true, is_bow: false, is_armo: false }
    }

    /// Golden : les records réels v1.12 sont parsés (patch_d2.mpq) via
    /// [`crate::tsv::Tsv`] — 638 prefixes / 742 suffixes / 36 automagics
    /// nommés + 46/155 mots rares (comptes brief « 669/747/45/154 » = lignes
    /// brutes ; les drapeaux « expansion » sans nom sont exclus — LE
    /// FICHIER FAIT FOI, mêmes comptes que l'audit `tables.rs`).
    #[test]
    fn parse_real_tables_and_sturdy_golden() {
        let t = tables();
        assert_eq!(t.prefixes.len(), 638, "prefixes (MagicPrefix.txt v1.12)");
        assert_eq!(t.suffixes.len(), 742, "suffixes (MagicSuffix.txt v1.12)");
        assert_eq!(t.automagics.len(), 36, "AutoMagic 36 records");
        assert_eq!(t.rareprefixes.len(), 46, "rareprefix.txt 46 records");
        assert_eq!(t.raresuffixes.len(), 155, "raresuffix.txt 155 records");
        let sturdy = t
            .prefixes
            .iter()
            .find(|a| a.name == "Sturdy")
            .expect("Sturdy présent");
        assert_eq!(sturdy.kind, AffixKind::Prefix);
        assert_eq!(sturdy.version, 0, "Sturdy = ligne classic (0), vivante en 1.12");
        assert_eq!(sturdy.level, 4);
        assert_eq!(sturdy.group, 101);
        assert!(sturdy.spawnable);
        assert_eq!(sturdy.mods.len(), 1);
        assert_eq!(sturdy.mods[0].code, "ac%");
        assert_eq!((sturdy.mods[0].min, sturdy.mods[0].max), (20, 30));
        assert!(sturdy.itypes.contains(&"armo".to_string()));
        // version 100 (patch) et version 0 (classic) coexistent et vivent :
        // le filtre accepte tout (« version/toBeSpawned » — champ informatif).
        assert!(t.prefixes.iter().any(|a| a.version == 100));
        assert!(t.suffixes.iter().any(|a| a.version == 100));
        // Rare words : les clés TBL fr existent pour les deux premiers mots.
        let s = strings();
        for w in t.rareprefixes.iter().take(3) {
            assert!(
                s.get(&w.name).is_some() || s.get(&format!("{}UM", w.name)).is_some(),
                "clé fr manquante pour le mot rare {}",
                w.name
            );
        }
    }

    /// GOLDEN 7b — la TBL fr couvre 100 % des affixes réels : chaque nom
    /// magicprefix/magicsuffix (638 + 742) résout en une chaîne fr.
    #[test]
    fn tbl_fr_covers_every_real_affix_name() {
        let t = tables();
        let s = strings();
        let missing: Vec<&str> = t
            .prefixes
            .iter()
            .chain(t.suffixes.iter())
            .filter(|a| a.spawnable)
            .map(|a| a.name.as_str())
            .filter(|n| s.get(n).is_none())
            .collect();
        assert!(missing.is_empty(), "noms sans clé fr : {missing:?}");
        // (le record drapeau « Expansion », non spawnable, n'a pas de clé
        // fr — jamais affiché, et le seul du genre dans les données réelles.)
    }

    /// Suffix « of Health » — golden exact depuis la table réelle v1.12 :
    /// level 7, `red-dmg` 1-1 (« Damage Reduced by 1 », la table fait foi).
    #[test]
    fn of_health_suffix_golden() {
        let t = tables();
        let h = t.suffixes.iter().find(|a| a.name == "of Health").expect("of Health");
        assert_eq!(h.level, 7);
        assert_eq!(h.mods[0].code, "red-dmg");
        assert_eq!((h.mods[0].min, h.mods[0].max), (1, 1));
    }

    /// Filtres : fenêtre `level`/`maxlevel`, `spawnable`, itype/etype.
    #[test]
    fn filters_level_and_types() {
        let t = tables();
        let sturdy = t.prefixes.iter().find(|a| a.name == "Sturdy").unwrap();
        // Sturdy (level 4, armo) : visible dès alvl 4 sur armure, pas à 1/3.
        assert!(!matches(sturdy, &armor_types(), 3));
        assert!(matches(sturdy, &armor_types(), 4));
        assert!(!matches(sturdy, &axe_types(), 30), "itype armo ≠ arme");
        // Un affixe non spawnable n'est jamais matché.
        let dead = t.prefixes.iter().find(|a| !a.spawnable);
        if let Some(d) = dead {
            assert!(!matches(d, &armor_types(), 99));
        }
    }

    /// GOLDEN brief — prefix ilvl 1 connu sur une arme de départ : « Jagged »
    /// (level 1, spawnable, `dmg%` 10-20, itype weap) matche la Hachette du
    /// barbare (code `hax`, qlvl 3, ilvl 1 ⇒ alvl 3).
    #[test]
    fn known_ilvl1_prefix_on_starting_weapon() {
        let t = tables();
        let jagged = t.prefixes.iter().find(|a| a.name == "Jagged").expect("Jagged présent");
        assert_eq!(jagged.level, 1);
        assert!(jagged.spawnable);
        assert_eq!(jagged.mods[0].code, "dmg%");
        assert_eq!((jagged.mods[0].min, jagged.mods[0].max), (10, 20));
        assert!(matches(jagged, &axe_types(), alvl(3, 1)), "Jagged sur hax à ilvl 1");
        // …et le pool arme à alvl 1 n'est PAS vide (Jagged, Bronze, ...).
        let pool = pools(&t, &axe_types(), 1);
        assert!(pool.prefixes.iter().any(|a| a.name == "Jagged"));
        assert!(pool.suffixes.iter().any(|a| a.name == "of Readiness"), "suffix level 1 connu");
    }

    /// BRIEF : ilvl 1 vs 30 sur la même base ⇒ ensembles d'affixes différents.
    #[test]
    fn affix_pools_differ_between_ilvl_1_and_30() {
        let t = tables();
        let types = axe_types();
        let at = |ilvl: u32| pools(&t, &types, alvl(6, ilvl)).prefixes.len();
        let low = at(1);
        let high = at(30);
        assert!(high > low, "alvl 1 : {low} affixes, alvl 30 : {high}");
        let gated = t
            .prefixes
            .iter()
            .find(|a| a.spawnable && a.level > 7 && a.itypes.iter().any(|ty| types.covers(ty)))
            .expect("affixe à niveau seuil couvrant les armes existe");
        assert!(!matches(gated, &types, 1));
        assert!(matches(gated, &types, 30));
    }

    /// Magic 7b : 0-3 prefixes + 0-3 suffixes, ≥ 1 affixe total, groupes
    /// exclusifs par côté, déterminisme au seed.
    #[test]
    fn roll_magic_bounds_groups_and_determinism() {
        let t = tables();
        for seed in 0..300u64 {
            let mut a = Rng::new(seed | 1);
            let mut b = Rng::new(seed | 1);
            let ra = roll_magic(&mut a, &t, &axe_types(), 30);
            let rb = roll_magic(&mut b, &t, &axe_types(), 30);
            assert_eq!(ra.prefixes.len(), rb.prefixes.len(), "seed {seed}");
            assert_eq!(
                ra.prefixes.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
                rb.prefixes.iter().map(|x| x.name.as_str()).collect::<Vec<_>>()
            );
            assert_eq!(
                ra.suffixes.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
                rb.suffixes.iter().map(|x| x.name.as_str()).collect::<Vec<_>>()
            );
            assert!(ra.prefixes.len() <= 3 && ra.suffixes.len() <= 3, "bornes 1-3/1-3");
            assert!(!ra.prefixes.is_empty() || !ra.suffixes.is_empty(), "≥ 1 affixe total");
            let mut groups: Vec<i32> = Vec::new();
            for x in ra.prefixes.iter().chain(ra.suffixes.iter()) {
                assert!(!groups.contains(&x.group), "groupe dupliqué {}", x.group);
                groups.push(x.group);
            }
        }
    }

    /// Rare 7b : 3-6 affixes, tous `rare`, groupes exclusifs GLOBAUX,
    /// déterminisme au seed.
    #[test]
    fn roll_rare_three_to_six_exclusive_groups() {
        let t = tables();
        for seed in 0..300u64 {
            let mut rng = Rng::new(seed.wrapping_mul(7) | 1);
            let picks = roll_rare(&mut rng, &t, &armor_types(), 40);
            assert!((3..=6).contains(&picks.len()), "3-6 affixes : {}", picks.len());
            let mut groups: Vec<i32> = Vec::new();
            for p in &picks {
                assert!(p.rare, "affixe non-rare sur rare : {}", p.name);
                assert!(!groups.contains(&p.group), "groupe dupliqué {}", p.group);
                groups.push(p.group);
            }
            // déterminisme
            let mut rng2 = Rng::new(seed.wrapping_mul(7) | 1);
            let again = roll_rare(&mut rng2, &t, &armor_types(), 40);
            assert_eq!(
                picks.iter().map(|x| x.name.as_str()).collect::<Vec<_>>(),
                again.iter().map(|x| x.name.as_str()).collect::<Vec<_>>()
            );
        }
    }

    /// AutoMagic 7b : pool filtré par type + niveau (Fletcher's level 1 dès
    /// alvl 1, Archer's 60 ensuite), tirage déterministe, ids offset 200_000.
    #[test]
    fn automagic_pool_pick_and_ids() {
        let t = tables();
        let p1: Vec<&str> =
            automagic_pool(&t, &axe_types(), 1).iter().map(|a| a.name.as_str()).collect();
        let p60: Vec<&str> =
            automagic_pool(&t, &axe_types(), 60).iter().map(|a| a.name.as_str()).collect();
        assert!(p1.contains(&"Fletcher's"), "pool alvl 1 : {p1:?}");
        assert!(p1.contains(&"of the Jackal"));
        assert!(p60.contains(&"Archer's"), "pool alvl 60 : {p60:?}");
        assert!(p60.len() > p1.len(), "le pool croît avec le niveau");
        // déterminisme
        let mut a = Rng::new(11);
        let mut b = Rng::new(11);
        assert_eq!(
            roll_automagic(&mut a, &t, &axe_types(), 60).map(|x| x.name.as_str()),
            roll_automagic(&mut b, &t, &axe_types(), 60).map(|x| x.name.as_str())
        );
        // ids automagic = offset 200_000 + rang ; résolution étendue only.
        let fletcher = t.automagics.iter().find(|a| a.name == "Fletcher's").unwrap();
        let id = row_id(&t, fletcher).expect("id automagic");
        assert_eq!(id, AUTOMAGIC_ID_OFFSET + 1);
        assert!(crate::item_roll::resolve_affix(&t, id).is_none(), "résolveur gelé 7a : None");
        assert!(resolve_affix_ext(&t, id).is_some(), "résolveur étendu 7b : Some");
        // orbes/baguettes : pool présent (itype weap couvre wand).
        assert!(!automagic_pool(&t, &wand_types(), 10).is_empty());
    }

    /// GOLDEN ids ItemStatCost : `Sturdy` → `item_armor_percent` (id 16),
    /// `Jagged` (`dmg%`) → min% (18) + max% (17), `charged` →
    /// `item_charged_skill` (204) avec param conservé.
    #[test]
    fn stat_ids_golden() {
        let t = tables();
        let s = strings();
        let sturdy = t.prefixes.iter().find(|a| a.name == "Sturdy").unwrap();
        let jagged = t.prefixes.iter().find(|a| a.name == "Jagged").unwrap();
        let ids = stat_ids_of(&t, &[(row_id(&t, sturdy).unwrap(), 25), (row_id(&t, jagged).unwrap(), 15)]);
        assert_eq!(ids.len(), 3, "ac% + dmg%(min,max) : {ids:?}");
        assert_eq!(ids[0].stat_id, 16, "item_armor_percent");
        assert_eq!(ids[0].value, 25);
        assert_eq!(ids[0].param, 0);
        assert_eq!(ids[1].stat_id, 18, "item_mindamage_percent");
        assert_eq!(ids[2].stat_id, 17, "item_maxdamage_percent");
        // aller-retour dictionnaire : id 16 ↔ item_armor_percent.
        let isc = StatCostTable::global();
        assert_eq!(isc.name_of_id(16), Some("item_armor_percent"));
        // charged (suffix « of Lightning » version 100) : stat 204 + param.
        let charged = t
            .suffixes
            .iter()
            .find(|a| a.mods.iter().any(|m| m.code == "charged"))
            .expect("suffix charged (of Lightning)");
        let cid = row_id(&t, charged).unwrap();
        let exp = stat_ids_of(&t, &[(cid, 1)]);
        assert!(exp.iter().all(|x| x.stat_id == 204), "item_charged_skill : {exp:?}");
        assert!(exp.iter().all(|x| x.param != 0), "param skill conservé : {exp:?}");
        let _ = s;
    }

    /// Le vocabulaire COMPLET des tables réelles (105 codes) est mappé vers
    /// des stats EXISTANTES de l'ItemStatCost v1.12 (jamais de -1).
    #[test]
    fn every_real_mod_code_maps_to_itemstatcost() {
        let t = tables();
        let isc = StatCostTable::global();
        let mut codes: Vec<&str> = t
            .prefixes
            .iter()
            .chain(t.suffixes.iter())
            .chain(t.automagics.iter())
            .flat_map(|a| a.mods.iter().map(|m| m.code.as_str()))
            .collect();
        codes.sort_unstable();
        codes.dedup();
        assert!(codes.len() >= 100, "vocabulaire réel : {} codes", codes.len());
        for code in &codes {
            let names = prop_stat_names(code);
            assert!(!names.is_empty(), "code {code} non mappé");
            for n in names {
                assert!(isc.get(n).is_some(), "stat {n} (code {code}) absente de l'ISC");
            }
        }
    }

    /// Noms FR — golden exacts : « Hachette irrégulière de Santé » (base
    /// `hax` féminine + prefix `Jagged` accordé + suffix `of Health`) et
    /// « Ecu robuste » (base `buc` masculine + `Strong`).
    #[test]
    fn magic_name_fr_golden() {
        let t = tables();
        let s = strings();
        let jagged = t.prefixes.iter().find(|a| a.name == "Jagged").unwrap();
        let health = t.suffixes.iter().find(|a| a.name == "of Health").unwrap();
        let strong = t.prefixes.iter().find(|a| a.name == "Strong").unwrap();
        assert_eq!(name_fr(&s, "hax", "Hand Axe", &[jagged], &[health]), "Hachette irrégulière de Santé");
        assert_eq!(name_fr(&s, "buc", "Buckler", &[strong], &[]), "Ecu robuste");
        // Base absente de la TBL ⇒ repli anglais invariable.
        assert_eq!(name_fr(&s, "zzz", "Sword", &[strong], &[health]), "Sword robuste de Santé");
        // Sans affixe : nom de base seul (fr si dispo).
        assert_eq!(name_fr(&s, "hax", "Hand Axe", &[], &[]), "Hachette");
    }

    /// Noms RARES fr — golden : `Doom` + `bite` → « Morsure mortelle »
    /// (accord féminin via la clé `DoomUM`) ; mot « de la … » ajouté tel
    /// quel (« Griffe de la Bête »).
    #[test]
    fn rare_name_fr_golden() {
        let s = strings();
        let doom = RareWord {
            name: "Doom".into(),
            itypes: vec!["armo".into(), "weap".into(), "misc".into()],
            etypes: vec![],
        };
        let bite = RareWord {
            name: "bite".into(),
            itypes: vec!["swor".into(), "knif".into(), "spea".into(), "pole".into(), "axe".into(), "h2h".into()],
            etypes: vec![],
        };
        assert_eq!(rare_name_fr(&s, &doom, &bite), "Morsure mortelle");
        let beast = RareWord { name: "Beast".into(), ..doom.clone() };
        assert_eq!(rare_name_fr(&s, &beast, &bite), "Morsure de la Bête");
        // roll_rare_name : mots filtrés par type + déterminisme.
        let t = tables();
        let mut a = Rng::new(3);
        let mut b = Rng::new(3);
        let pa = roll_rare_name(&mut a, &t, &axe_types()).expect("mots rares pour une hache");
        let pb = roll_rare_name(&mut b, &t, &axe_types()).expect("déterminisme");
        assert_eq!(pa.0.name, pb.0.name);
        assert_eq!(pa.1.name, pb.1.name);
        assert!(pa.0.covers(&axe_types()) && pa.1.covers(&axe_types()));
        assert!(!rare_name_fr(&s, pa.0, pa.1).trim().is_empty());
    }

    /// Parseur de variantes fr : balises, ordre, genre du substantif,
    /// chaînes invariables.
    #[test]
    fn gendered_parsing() {
        let g = parse_gendered("[fs]solide[ms]solide[fp]solides[mp]solides");
        assert_eq!(g.first(), "solide");
        assert_eq!(g.agree(true), "solide");
        assert!(!g.fem(), "adjectif : les deux singuliers présents");
        let g = parse_gendered("[fs]irrégulière[ms]irrégulier[fp]irrégulières[mp]irréguliers");
        assert_eq!(g.agree(true), "irrégulière");
        assert_eq!(g.agree(false), "irrégulier");
        let g = parse_gendered("[fs]Hachette");
        assert_eq!(g.first(), "Hachette");
        assert!(g.fem(), "fs seule = féminin");
        assert!(!parse_gendered("[ms]Ecu").fem());
        assert_eq!(parse_gendered("de Santé").first(), "de Santé");
        assert_eq!(parse_gendered("de Santé").agree(true), "de Santé", "invariable");
    }

    /// Coloration : `Saintly` (transform dgld) lue via [`transform_of`].
    #[test]
    fn transform_color_read_from_resolved_rec() {
        let t = tables();
        let saintly = t
            .prefixes
            .iter()
            .find(|a| a.name == "Saintly" && a.transform)
            .expect("Saintly transformant");
        assert_eq!(saintly.transformcolor, "dgld");
        let id = row_id(&t, saintly).unwrap();
        assert_eq!(transform_of(&t, &[(id, 66)]), Some("dgld".to_string()));
        assert_eq!(transform_of(&t, &[]), None);
    }

    /// [`decorate_item`] : magic sur la hache de départ — affix_ids
    /// résolubles, sockets (`sock`), nom FR posé, déterminisme au seed,
    /// automagic en option (id ≥ 200_000).
    #[test]
    fn decorate_item_magic_golden_and_determinism() {
        let t = tables();
        let s = strings();
        let mk = |seed: u64, auto: bool| {
            let mut rng = Rng::new(seed);
            let mut item = RolledItem {
                code: "hax".into(),
                quality: RolledQuality::Magic,
                ..Default::default()
            };
            decorate_item(&mut rng, &mut item, &t, &s, &axe_types(), "Hand Axe", 3, 1, auto);
            item
        };
        let a = mk(0xD2, false);
        let b = mk(0xD2, false);
        assert_eq!(a, b, "même seed ⇒ même RolledItem");
        assert!(!a.affix_ids.is_empty(), "magic porte au moins 1 affixe");
        assert!(a.name_resolved.starts_with("Hachette"), "nom FR composé : {}", a.name_resolved);
        assert_ne!(a.name_resolved, "Hachette", "un affixe figure dans le nom");
        for &(id, v) in &a.affix_ids {
            let (rec, _) = resolve_affix_ext(&t, id).expect("id résoluble");
            assert!(id < crate::item_roll::SUFFIX_ID_OFFSET || id < AUTOMAGIC_ID_OFFSET);
            let ok = rec
                .mods
                .iter()
                .any(|d| v as i32 >= d.min.min(d.max) && v as i32 <= d.max.max(d.min));
            assert!(ok, "valeur {v} hors bornes de {}", rec.name);
        }
        // Automagic : ids automod ≥ 200_000, dénombrables via le pool.
        let with_auto = mk(0xD2 | 1 << 40, true);
        assert!(with_auto.affix_ids.iter().any(|&(id, _)| id >= AUTOMAGIC_ID_OFFSET));
        // Le nom magic reste composé même avec automod.
        assert!(with_auto.name_resolved.starts_with("Hachette"));
    }

    /// [`decorate_item`] rare : 3-6 affixes `rare`, NOM RARE fr posé
    /// (≠ nom de base), déterministe.
    #[test]
    fn decorate_item_rare_carries_rare_name() {
        let t = tables();
        let s = strings();
        let mut a = Rng::new(0xD2A);
        let mut b = Rng::new(0xD2A);
        let mk = |rng: &mut Rng| {
            let mut item = RolledItem {
                code: "hax".into(),
                quality: RolledQuality::Rare,
                ..Default::default()
            };
            decorate_item(rng, &mut item, &t, &s, &axe_types(), "Hand Axe", 30, 30, false);
            item
        };
        let (ra, rb) = (mk(&mut a), mk(&mut b));
        assert_eq!(ra, rb);
        // ≤ 6 affixes × 3 mods ; ids distincts 3-6 (le contrat laisse la
        // granularité fine au mod — on borne les ids distincts).
        let distinct: std::collections::HashSet<u32> = ra.affix_ids.iter().map(|a| a.0).collect();
        assert!((3..=6).contains(&distinct.len()), "3-6 affixes : {distinct:?}");
        for &id in &distinct {
            let (rec, _) = resolve_affix_ext(&t, id).unwrap();
            assert!(rec.rare, "affixe non-rare sur rare : {}", rec.name);
        }
        assert!(!ra.name_resolved.is_empty());
        assert_ne!(ra.name_resolved, "Hand Axe", "nom rare composé : {}", ra.name_resolved);
    }

    /// Tirage historique (compat loot/vendor/item_roll) : ≤ 1 prefix et
    /// ≤ 1 suffix, déterministe au seed.
    #[test]
    fn magic_picks_respect_max_and_determinism() {
        let t = tables();
        for seed in 0..200u64 {
            let mut a = Rng::new(seed | 1);
            let mut b = Rng::new(seed | 1);
            let (pa, sa) = pick_magic(&mut a, &t, &axe_types(), 30);
            let (pb, sb) = pick_magic(&mut b, &t, &axe_types(), 30);
            assert!(pa.is_some() || sa.is_some(), "seed {seed} : aucun affixe");
            assert_eq!(pa.map(|x| x.name.as_str()), pb.map(|x| x.name.as_str()));
            assert_eq!(sa.map(|x| x.name.as_str()), sb.map(|x| x.name.as_str()));
        }
    }

    /// Tirage historique rare : 4 affixes max, uniquement `rare`, groupes
    /// exclusifs.
    #[test]
    fn rare_picks_exclusive_groups_and_rare_flag() {
        let t = tables();
        let mut rng = Rng::new(5);
        for _ in 0..50 {
            let picks = pick_rare(&mut rng, &t, &armor_types(), 40);
            assert!(picks.len() <= 4);
            let mut groups: Vec<i32> = Vec::new();
            for p in &picks {
                assert!(p.rare, "affixe non-rare sur rare : {}", p.name);
                assert!(!groups.contains(&p.group), "groupe dupliqué {}", p.group);
                groups.push(p.group);
            }
        }
    }

    /// Tirage pondéré par frequency : un affixe fréquence 1000 sort ~10×
    /// plus souvent qu'un fréquence 100 (même pool).
    #[test]
    fn weighting_follows_frequency() {
        let hi = AffixRec {
            name: "Hi".into(),
            kind: AffixKind::Prefix,
            version: 0,
            spawnable: true,
            rare: false,
            level: 1,
            maxlevel: 0,
            levelreq: 0,
            frequency: 1000,
            group: 1,
            mods: Vec::new(),
            transform: false,
            transformcolor: String::new(),
            itypes: vec![],
            etypes: vec![],
        };
        let lo = AffixRec { name: "Lo".into(), frequency: 100, group: 2, ..hi.clone() };
        let pool = vec![&hi, &lo];
        let mut hi_count = 0;
        let mut rng = Rng::new(3);
        for _ in 0..20_000 {
            if weighted(&mut rng, &pool).unwrap().name == "Hi" {
                hi_count += 1;
            }
        }
        let share = hi_count as f64 / 20_000.0;
        assert!((0.88..=0.98).contains(&share), "part de Hi : {share}");
    }

    /// `alvl = max(qlvl, ilvl)` : un drop de monstre faible d'une base de
    /// haut niveau garde le qlvl de la base.
    #[test]
    fn alvl_is_max_of_qlvl_and_ilvl() {
        assert_eq!(alvl(6, 1), 6);
        assert_eq!(alvl(6, 30), 30);
    }
}

