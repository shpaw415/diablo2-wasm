//! `affixes` — affixes magiques `MagicPrefix.txt` / `MagicSuffix.txt` v1.12.
//!
//! Port Rust des filtres D2 réels (réf. od2 `item_factory.go:324
//! FindMatchingAffixes` + `item_affix_loader.go`, complétés par les règles
//! v1.12 documentées — od2 oublie `spawnable`, `maxlevel` et le tirage
//! pondéré) :
//!
//! * **Filtre d'un affixe** pour une base de niveau effectif `alvl` :
//!   `spawnable = 1`, `version` acceptée (port 1.12 = LoD ⇒ tout),
//!   `level ≤ alvl ≤ maxlevel` (0 = pas de plafond), base couverte par un
//!   `itype1-7` (type propre ou famille `weap`/`mele`/`bow`/`armo`), aucun
//!   `etype1-3` ne la couvre. `alvl = max(qlvl, ilvl)` (règle D2 magic/rare).
//! * **Choix** pondéré par `frequency` (0 ⇒ poids 1). Item magique :
//!   1 prefix ET/OU 1 suffix max (port du tirage Diablerie
//!   `rand(4)` : `<2` prefix, `!= 1` suffix ⇒ 50 %/75 %, moyenne 1,25).
//!   Rare : 4 affixes tous types confondus, groupes `group` exclusifs.
//! * **exclusion de groupe** : jamais deux affixes du même `group`.
//! * **AutoMagic** : parse v1.12 (36 records, mods automatiques type orbes) —
//!   exposé mais non branché sur les drops (usage vendeurs/cube, restant).
//!
//! Zéro IO : tables embarquées (`include_str!`), déterminisme via [`Rng`].

use crate::item_props::{descriptor, PropDescriptor};
use crate::rng::Rng;

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
    /// Nom affichable anglais (`Sturdy`, `of Health`… ; fr = TBL, restant).
    pub name: String,
    /// Prefix ou suffix.
    pub kind: AffixKind,
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

/// Tables d'affixes v1.12 embarquées.
#[derive(Debug, Clone, Default)]
pub struct AffixTables {
    /// `MagicPrefix.txt` (spawnables et non-spawnables, filtrés au tirage).
    pub prefixes: Vec<AffixRec>,
    /// `MagicSuffix.txt`.
    pub suffixes: Vec<AffixRec>,
    /// `AutoMagic.txt` (mods automatiques — non branché, restant).
    pub automagics: Vec<AffixRec>,
}

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

/// Parse les deux tables d'affixes (+ AutoMagic) embarquées ; `None` si
/// aucune ligne exploitable (les drops retombent alors sans affixe).
pub fn parse(prefix_txt: &str, suffix_txt: &str, automagic_txt: &str) -> Option<AffixTables> {
    let prefixes = parse_affix_file(prefix_txt, AffixKind::Prefix);
    let suffixes = parse_affix_file(suffix_txt, AffixKind::Suffix);
    let automagics = parse_affix_file(automagic_txt, AffixKind::Prefix);
    (prefixes.len() + suffixes.len() > 0).then_some(AffixTables { prefixes, suffixes, automagics })
}

/// Parse une table d'affixes (colonnes v1.12, paires `modN*` et
/// `itypeN`/`etypeN` découvertes depuis l'en-tête).
fn parse_affix_file(txt: &str, kind: AffixKind) -> Vec<AffixRec> {
    let rows = crate::loot::tsv_rows(txt);
    let Some(hdr) = rows.first() else { return Vec::new() };
    let col = |name: &str| hdr.iter().position(|h| h.eq_ignore_ascii_case(name));
    let (Some(name_c), Some(level_c)) = (col("Name"), col("level")) else {
        return Vec::new();
    };
    let group_c = col("group");
    let mods: Vec<(usize, usize, usize, usize)> = (1..=3)
        .filter_map(|n| {
            Some((
                col(&format!("mod{n}code"))?,
                col(&format!("mod{n}param"))?,
                col(&format!("mod{n}min"))?,
                col(&format!("mod{n}max"))?,
            ))
        })
        .collect();
    let types = |prefix: &str, max: usize| -> Vec<usize> {
        (1..=max).filter_map(|n| col(&format!("{prefix}{n}"))).collect()
    };
    let itypes = types("itype", 7);
    let etypes = types("etype", 3);

    let mut out = Vec::new();
    for r in rows.iter().skip(1) {
        let cell = |i: Option<usize>| i.and_then(|i| r.get(i)).map(|s| s.trim()).unwrap_or("");
        let name = cell(Some(name_c));
        if name.is_empty() {
            continue; // ligne drapeau « expansion » des MPQ
        }
        let num = |i: &str| cell(col(i)).parse::<u32>().unwrap_or(0);
        let level = cell(Some(level_c)).parse::<u32>().unwrap_or(0);
        let mods_parsed = mods
            .iter()
            .filter_map(|&(c, p, mn, mx)| descriptor([cell(Some(c)), cell(Some(p)), cell(Some(mn)), cell(Some(mx))]))
            .collect();
        out.push(AffixRec {
            name: name.to_string(),
            kind,
            spawnable: num("spawnable") == 1,
            rare: num("rare") == 1,
            level,
            maxlevel: num("maxlevel"),
            levelreq: num("levelreq"),
            frequency: num("frequency"),
            group: group_c.and_then(|g| r.get(g)).and_then(|s| s.trim().parse().ok()).unwrap_or(0),
            mods: mods_parsed,
            transform: num("transform") == 1,
            transformcolor: cell(col("transformcolor")).to_string(),
            itypes: itypes.iter().map(|&i| cell(Some(i)).to_string()).filter(|s| !s.is_empty()).collect(),
            etypes: etypes.iter().map(|&i| cell(Some(i)).to_string()).filter(|s| !s.is_empty()).collect(),
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
/// Déterministe : 1 à 2 tirages [`Rng`].
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
pub(crate) fn pick_rare<'a>(
    rng: &mut Rng,
    tables: &'a AffixTables,
    types: &ItemTypeSet,
    level: u32,
) -> Vec<&'a AffixRec> {
    let mut used_groups: Vec<i32> = Vec::new();
    let mut out: Vec<&AffixRec> = Vec::new();
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
// Tests — tables réelles embarquées, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;

    const PREFIX: &str = include_str!("../../../../assets/patch_magicprefix.txt");
    const SUFFIX: &str = include_str!("../../../../assets/patch_magicsuffix.txt");
    const AUTOMAGIC: &str = include_str!("../../../../assets/patch_automagic.txt");

    fn tables() -> AffixTables {
        parse(PREFIX, SUFFIX, AUTOMAGIC).expect("tables d'affixes valides")
    }

    fn axe_types() -> ItemTypeSet {
        ItemTypeSet { wtype: "axe", is_weap: true, is_mele: true, is_bow: false, is_armo: false }
    }

    fn armor_types() -> ItemTypeSet {
        ItemTypeSet { wtype: "tors", is_weap: false, is_mele: false, is_bow: false, is_armo: true }
    }

    /// Golden : les 638/742 records réels v1.12 sont parsés (patch_d2.mpq),
    /// « Sturdy » porte ses valeurs exactes (level 4, group 101, ac% 20-30,
    /// itype armo).
    #[test]
    fn parse_real_tables_and_sturdy_golden() {
        let t = tables();
        assert_eq!(t.prefixes.len(), 638, "prefixes (MagicPrefix.txt v1.12)");
        assert_eq!(t.suffixes.len(), 742, "suffixes (MagicSuffix.txt v1.12)");
        assert_eq!(t.automagics.len(), 36, "AutoMagic 36 records");
        let sturdy = t
            .prefixes
            .iter()
            .find(|a| a.name == "Sturdy")
            .expect("Sturdy présent");
        assert_eq!(sturdy.kind, AffixKind::Prefix);
        assert_eq!(sturdy.level, 4);
        assert_eq!(sturdy.group, 101);
        assert!(sturdy.spawnable);
        assert_eq!(sturdy.mods.len(), 1);
        assert_eq!(sturdy.mods[0].code, "ac%");
        assert_eq!((sturdy.mods[0].min, sturdy.mods[0].max), (20, 30));
        assert!(sturdy.itypes.contains(&"armo".to_string()));
    }

    /// Suffix « of Health » — golden exact depuis la table réelle v1.12 :
    /// level 7, `red-dmg` 1-1 (« Damage Reduced by 1 », la table fait foi —
    /// le +vie est porté par d'autres suffixes).
    #[test]
    fn of_health_suffix_golden() {
        let t = tables();
        let h = t.suffixes.iter().find(|a| a.name == "of Health").expect("of Health");
        assert_eq!(h.level, 7);
        assert_eq!(h.mods[0].code, "red-dmg");
        assert_eq!((h.mods[0].min, h.mods[0].max), (1, 1));
        assert!(!h.mods.is_empty());
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

    /// BRIEF : ilvl 1 vs 30 sur la même base ⇒ ensembles d'affixes différents
    /// (le haut niveau débloque des affixes de niveau supérieur). Le palier
    /// est vérifié sur un affixe COUVRANT la base (itype arme) — un affixe
    /// restreint aux armures ne matcherait jamais une hache, palier ou non.
    #[test]
    fn affix_pools_differ_between_ilvl_1_and_30() {
        let t = tables();
        let types = axe_types();
        let at = |ilvl: u32| {
            let lvl = alvl(6, ilvl); // hax : qlvl 6
            t.prefixes.iter().filter(|a| matches(a, &types, lvl)).count()
        };
        let low = at(1);
        let high = at(30);
        assert!(high > low, "alvl 1 : {low} affixes, alvl 30 : {high}");
        // Un affixe de niveau > 7 tirable sur les armes (itype couvrant axe)
        // absent à ilvl 1 mais présent à 30.
        let gated = t
            .prefixes
            .iter()
            .find(|a| a.spawnable && a.level > 7 && a.itypes.iter().any(|ty| types.covers(ty)))
            .expect("affixe à niveau seuil couvrant les armes existe");
        assert!(!matches(gated, &types, 1));
        assert!(matches(gated, &types, 30));
    }

    /// Magic : ≤ 1 prefix et ≤ 1 suffix, déterministe au seed.
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

    /// Rare : 4 affixes max, uniquement `rare`, groupes exclusifs.
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
