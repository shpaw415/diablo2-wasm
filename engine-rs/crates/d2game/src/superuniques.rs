//! Super-uniques data-driven — étape 9a « monde vivant » (campagne
//! diablo-2-wasm). Pour un niveau donné, lit ses planches `umon1..umon10`
//! (`patch_levels.txt`), y résout les super-uniques de `superuniques.txt`
//! (classe ∈ planche) et les instancie via le template `patch_monstats.txt` :
//! stats réelles [`MonLvl`] par difficulté, packs `MinGrp`/`MaxGrp`, TC de
//! drop PAR DIFFICULTÉ (`TC`/`TC(N)`/`TC(H)`), nom fr affiché (TBL v1.12
//! embarquées, [`d2rules::tbl`]) et mods `Mod1-3` exposés en étiquettes de
//! combat (ids monumod + noms) — les boosts numériques (HP/dégâts/toucher
//! des élites) sont le domaine du coéquipier `elite_mods` : ce module expose
//! les ids, il n'invente aucun multiplicateur.
//!
//! # Sources (tables v1.12 réelles, embarquées `include_str!` — zéro IO)
//! * `assets/patch_levels.txt` — `umon1..umon10` (colonnes 96-105) par Id.
//!   VÉRIFIÉ 2026-10-24 : Blood Moor (id 2) = zombie1/fallen1/quillrat1,
//!   Stony Field (id 4) = skeleton1/zombie2/foulcrow1/goatman1/cr_archer1,
//!   Cave Level 1 (id 9) = skeleton1/zombie2/cr_archer1, Den of Evil
//!   (id 8) = zombie1/brute1/fallenshaman1, Dark Wood (id 5) =
//!   fallen2/corruptrogue2/quillrat2/cr_lancer2/sk_archer1.
//! * `assets/superuniques.txt` (67 rec) — `Class`→monstats, `Mod1-3` (ids
//!   monumod), `MinGrp`/`MaxGrp`, `AutoPos`, `Utrans`, `TC`/`TC(N)`/`TC(H)`.
//! * `assets/patch_monstats.txt` / `assets/MonLvl.txt` — template (code
//!   sprite, `Level(N)/(H)`, résists) + facteurs effectifs par difficulté.
//! * `assets/TreasureClassEx.txt` — vérification d'existence des TC
//!   ([`tc_exists`], ex. « Act 1 Super A », « Act 1 (H) Super B »).
//! * `assets/monumod.txt` — noms des mods par id (`8` = resist, `9` = fire…).
//!
//! # Placement (« umon fait foi » vs retail)
//! La résolution data-driven pose le super-unique là où SA CLASSE figure
//! dans la planche `umon1-10` du niveau — approximation assumée des
//! positions retail MonPreset/DS1 (hors périmètre, cf. `monsters.rs`
//! « positions »). Conséquences VÉRIFIÉES dans les tables de CE port :
//! Corpsefire → Blood Moor, Bonebreak → Stony Field + Cave 1, Coldcrow →
//! Cave 1, Bishibosh → Den of Evil (+ Cold Plains), Rakanishu → Dark Wood.
//! (Le brief citait Bishibosh/Blood Moor et Rakanishu/Stony Field « ex. » —
//! les umon réels tranchent autrement, tests à l'appui.)
//!
//! # Positions — `AutoPos`
//! `AutoPos=1` : placement automatique moteur (retail : près de l'entrée) ;
//! `AutoPos=0` : position fixe du preset DS1 (MonPreset — non porté).
//! Sans preset DS1, les DEUX chemins retombent sur un tirage uniforme
//! marchable ([`SuSpawnDef::place`], rejection seedée) ; le flag reste
//! exposé pour l'intégrateur.
//!
//! # Anti-doublon boss scriptés (quests_actes / quests_actes2)
//! Blood Raven (a1q2, cimetière id 17), Duriel (a2q6, repaire id 73) et
//! Mephisto (a3q6, Durance 3 id 102) sont spawnés par les journaux de
//! quêtes — JAMAIS par ce chemin. Garde structurelle : aucun des 67 records
//! `superuniques.txt` n'a pour classe/nom un boss scripté
//! ([`QUEST_SCRIPTED`], filtrée aussi à la résolution + test), et les
//! triggers `Kill` des quêtes ne recoupent aucune classe de super-unique
//! résolue (cimetière id 17 : umon skeleton1/zombie2 → Bonebreak seulement,
//! pas de corruptrogue2 ; le kill de Rakanishu ne déclenche pas a1q2).

use crate::compose::Lcg;
use crate::monsters::{MonLvl, MonStats};
use crate::spawn::pack_size;
use d2rules::tbl::TblSet;
use d2rules::tsv::Tsv;
use d2rules::{Difficulty, MonKind, SpawnRule};
use std::collections::HashMap;
use std::sync::OnceLock;

const PATCH_LEVELS_TXT: &str = include_str!("../../../../assets/patch_levels.txt");
const SUPERUNIQUES_TXT: &str = include_str!("../../../../assets/superuniques.txt");
const PATCH_MONSTATS_TXT: &str = include_str!("../../../../assets/patch_monstats.txt");
const MONUMOD_TXT: &str = include_str!("../../../../assets/monumod.txt");
const MONLVL_TXT: &str = include_str!("../../../../assets/MonLvl.txt");
const TREASURECLASSEX_TXT: &str = include_str!("../../../../assets/TreasureClassEx.txt");

/// Boss spawnés par les journaux de quêtes (garde anti-doublon, cf.
/// en-tête) : noms/classe monstats interdits au chemin data-driven.
pub const QUEST_SCRIPTED: [&str; 3] = ["bloodraven", "duriel", "mephisto"];

/// L'identifiant (nom de super-unique OU classe monstats) est-il la propriété
/// d'un boss scripté de quête ? Insensible à la casse.
pub fn is_quest_scripted(id_or_class: &str) -> bool {
    QUEST_SCRIPTED.iter().any(|b| id_or_class.eq_ignore_ascii_case(b))
}

/// Un mod `Mod1-3` résolu en ÉTIQUETTE de combat : id monumod brut (contrat
/// `elite_mods` — les boosts numériques sont chez lui) + nom officiel
/// (`monumod.txt`, ex. 17 = « lightning ») pour l'affichage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuModLabel {
    /// Id monumod brut de la cellule `Mod1-3`.
    pub id: u32,
    /// Nom officiel (`uniquemod`), vide si l'id est hors table.
    pub name: String,
}

/// Un super-unique résolu pour un niveau : template monstats instancié
/// (stats réelles [`SuSpawnDef::spawn_rule`]), pack, TC par difficulté,
/// noms fr, mods étiquettes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuSpawnDef {
    /// Nom de table (`Superunique`) — clé TBL du nom propre.
    pub name: String,
    /// Nom propre fr affiché (TBL v1.12, repli = nom brut ; ex. Corpsefire
    /// → « Crémation », Bonebreak → « Concasseur d'Os »).
    pub name_fr: String,
    /// Id monstats du template (`Class`, ex. `zombie1`).
    pub mon_id: String,
    /// Code sprite manifest (`Code` monstats, ex. `ZM`).
    pub code: String,
    /// `NameStr` du template (ex. `Zombie`) — clé TBL du nom d'espèce.
    pub class_name: String,
    /// Espèce fr (TBL, repli `class_name`).
    pub class_name_fr: String,
    /// Mods `Mod1-3` en étiquettes (ids exposés, boosts = `elite_mods`).
    pub mods: Vec<SuModLabel>,
    /// Pack officiel (`MinGrp`/`MaxGrp`, 0 ⇒ 1 — convention [`pack_size`]).
    pub min_grp: u32,
    pub max_grp: u32,
    /// `AutoPos` ≠ 0 (placement moteur retail ; cf. en-tête « Positions »).
    pub auto_pos: bool,
    /// Id de teinte palshift (`Utrans` — rendu hors périmètre, exposé).
    pub utrans: u32,
    /// TC de drop PAR DIFFICULTÉ `[Normal, Nightmare, Hell]` (colonnes
    /// `TC`/`TC(N)`/`TC(H)`).
    pub tc: [String; 3],
}

impl SuSpawnDef {
    /// TC de drop pour la difficulté (cellule vide ⇒ repli Normal, lui-même
    /// possiblement vide → `None`, l'appelant garde la TC du template).
    pub fn tc_for(&self, d: Difficulty) -> Option<&str> {
        let c = &self.tc[d.index()];
        if c.is_empty() { None } else { Some(c.as_str()) }
    }

    /// Taille de pack tirée (`MinGrp..MaxGrp`, 0 ⇒ 1) — déterministe au RNG.
    pub fn group_size(&self, rng: &mut Lcg) -> u32 {
        pack_size(self.min_grp, self.max_grp, rng.below(self.max_grp.saturating_sub(self.min_grp) + 1))
    }

    /// Règle de spawn effective ([`SpawnRule`], kind
    /// [`MonKind::SuperUnique`]) : facteurs [`MonLvl`] de la ligne du
    /// TEMPLATE pour la difficulté (`Level(N)/(H)` monstats → colonnes
    /// `(N)/(H)` MonLvl), résists de la difficulté, TC du super-unique.
    /// `None` : template absent des tables fournies.
    pub fn spawn_rule(&self, d: Difficulty, stats: &MonStats, monlvl: &MonLvl) -> Option<SpawnRule> {
        let cls = stats.get(&self.mon_id)?;
        let base = monlvl.row_at(cls.level_for(d), d);
        Some(SpawnRule {
            kind: MonKind::SuperUnique,
            hp_pct: base.hp_pct,
            dmg_pct: base.dm_pct,
            th_pct: base.th,
            xp_pct: base.xp_pct,
            ac: base.ac,
            treasure_class: self.tc_for(d).map(|s| s.to_string()),
            res: cls.res_for(d),
        })
    }

    /// Spot de spawn : tirage uniforme MARCHABLE (rejection seedée, 256
    /// essais) sur la grille sous-tuille `w×h` (`walk[y * w + x]`, même
    /// convention que `compose`). `None` : aucune case marchable trouvée.
    /// Vaut pour `AutoPos` des deux valeurs (cf. en-tête « Positions »).
    pub fn place(
        &self,
        rng: &mut Lcg,
        w: usize,
        h: usize,
        walkable: impl Fn(usize, usize) -> bool,
    ) -> Option<(usize, usize)> {
        if w == 0 || h == 0 {
            return None;
        }
        for _ in 0..256 {
            let (x, y) = (rng.below(w as u32) as usize, rng.below(h as u32) as usize);
            if walkable(x, y) {
                return Some((x, y));
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Tables embarquées — parsées une fois ([`std::sync::OnceLock`], lecture
// ensuite ; même modèle que `spawn.rs`)
// ---------------------------------------------------------------------------

/// `superuniques.txt` : les 67 records dans l'ordre du fichier (déterministe).
fn su_records() -> &'static [SuperUniqueRaw] {
    static TABLE: OnceLock<Vec<SuperUniqueRaw>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let Ok(t) = Tsv::parse(SUPERUNIQUES_TXT) else { return Vec::new() };
        (0..t.len())
            .filter_map(|i| t.rec(i))
            .filter(|r| !r.s("Superunique").is_empty())
            .map(|r| {
                // « 0 » = aucun mod (ligne `none` id 0 de monumod.txt — même
                // convention que le parseur `monsters.rs`).
                let raw_mod = |col: &str| r.s(col).trim().parse::<u32>().ok().filter(|&id| id != 0);
                SuperUniqueRaw {
                    name: r.s("Superunique").to_string(),
                    class: r.s("Class").to_string(),
                    mods: [raw_mod("Mod1"), raw_mod("Mod2"), raw_mod("Mod3")]
                        .into_iter()
                        .flatten()
                        .collect(),
                    min_grp: r.u("MinGrp"),
                    max_grp: r.u("MaxGrp"),
                    auto_pos: r.u("AutoPos") != 0,
                    utrans: r.u("Utrans"),
                    tc: [r.s("TC").to_string(), r.s("TC(N)").to_string(), r.s("TC(H)").to_string()],
                }
            })
            .collect()
    })
}

/// Record brut `superuniques.txt` (résolution [`superuniques_for_level`]).
struct SuperUniqueRaw {
    name: String,
    class: String,
    mods: Vec<u32>,
    min_grp: u32,
    max_grp: u32,
    auto_pos: bool,
    utrans: u32,
    tc: [String; 3],
}

/// Planches `umon1..umon10` par Id levels.txt (cellules vides sautées, ordre
/// des colonnes) — la ligne « Expansion » (Id vide) est sautée.
fn umon_table() -> &'static HashMap<u32, Vec<String>> {
    static TABLE: OnceLock<HashMap<u32, Vec<String>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut map = HashMap::new();
        if let Ok(t) = Tsv::parse(PATCH_LEVELS_TXT) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let Ok(id) = r.s("Id").parse::<u32>() else { continue };
                let plate: Vec<String> = (1..=10)
                    .filter_map(|n| {
                        let tok = r.s(&format!("umon{n}"));
                        (!tok.is_empty()).then(|| tok.to_string())
                    })
                    .collect();
                map.insert(id, plate);
            }
        }
        map
    })
}

/// Templates monstats (une fois).
fn monstats() -> Option<&'static MonStats> {
    static TABLE: OnceLock<Option<MonStats>> = OnceLock::new();
    TABLE.get_or_init(|| MonStats::parse(PATCH_MONSTATS_TXT).ok()).as_ref()
}

/// Facteurs MonLvl (une fois).
fn monlvl() -> Option<&'static MonLvl> {
    static TABLE: OnceLock<Option<MonLvl>> = OnceLock::new();
    TABLE.get_or_init(|| MonLvl::parse(MONLVL_TXT).ok()).as_ref()
}

/// Noms des mods monumod par id (une fois).
fn mod_names() -> &'static HashMap<u32, String> {
    static TABLE: OnceLock<HashMap<u32, String>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        if let Ok(t) = Tsv::parse(MONUMOD_TXT) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let name = r.s("uniquemod");
                if name.is_empty() {
                    continue;
                }
                m.insert(r.u("id"), name.to_string());
            }
        }
        m
    })
}

/// TBL fr v1.12 embarquées (une fois — patch > expansion > string).
fn tbl_fr() -> &'static TblSet {
    static TABLE: OnceLock<TblSet> = OnceLock::new();
    TABLE.get_or_init(TblSet::bundled_fra)
}

/// La TC existe-t-elle dans `TreasureClassEx.txt` ? (validation
/// d'intégration des TC résolues.)
pub fn tc_exists(name: &str) -> bool {
    static NAMES: OnceLock<std::collections::HashSet<String>> = OnceLock::new();
    NAMES
        .get_or_init(|| {
            let mut set = std::collections::HashSet::new();
            if let Ok(t) = Tsv::parse(TREASURECLASSEX_TXT) {
                for i in 0..t.len() {
                    if let Some(r) = t.rec(i) {
                        let n = r.s("Treasure Class");
                        if !n.is_empty() {
                            set.insert(n.to_string());
                        }
                    }
                }
            }
            set
        })
        .contains(name)
}

/// Résolution data-driven d'un niveau : ses super-uniques (`Class` ∈
/// `umon1-10`, ordre du fichier `superuniques.txt`), instanciés via le
/// template monstats. `None` : Id absent de `patch_levels.txt` ; niveau
/// connu sans super-unique résolu → `Some(vec![])`.
pub fn superuniques_for_level(levels_id: u32) -> Option<Vec<SuSpawnDef>> {
    let plate = umon_table().get(&levels_id)?;
    let stats = monstats()?;
    let tbl = tbl_fr();
    let names = mod_names();
    let fr = |key: &str, fallback: &str| tbl.get(key).unwrap_or_else(|| fallback.to_string());
    Some(
        su_records()
            .iter()
            .filter(|su| !is_quest_scripted(&su.name) && !is_quest_scripted(&su.class))
            .filter(|su| plate.iter().any(|m| *m == su.class))
            .filter_map(|su| {
                let cls = stats.get(&su.class)?;
                Some(SuSpawnDef {
                    name: su.name.clone(),
                    name_fr: fr(&su.name, &su.name),
                    mon_id: su.class.clone(),
                    code: cls.code.clone(),
                    class_name: cls.name.clone(),
                    class_name_fr: fr(&cls.name, &cls.name),
                    mods: su
                        .mods
                        .iter()
                        .map(|&id| SuModLabel { id, name: names.get(&id).cloned().unwrap_or_default() })
                        .collect(),
                    min_grp: su.min_grp,
                    max_grp: su.max_grp,
                    auto_pos: su.auto_pos,
                    utrans: su.utrans,
                    tc: su.tc.clone(),
                })
            })
            .collect(),
    )
}

/// Planche `umon1-10` brute d'un niveau (debug/E2E — même extraction que la
/// résolution).
pub fn umon_plate(levels_id: u32) -> Option<Vec<String>> {
    umon_table().get(&levels_id).cloned()
}

// ---------------------------------------------------------------------------
// Tests — tables réelles, résolutions vérifiées cellule par cellule
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 67 records `superuniques.txt` parsés (cohérence `monsters.rs`).
    #[test]
    fn parses_real_superuniques_table() {
        assert_eq!(su_records().len(), 67);
        let bishi = su_records().iter().find(|s| s.name == "Bishibosh").unwrap();
        assert_eq!(bishi.class, "fallenshaman1");
        assert_eq!(bishi.mods, vec![8, 9], "Mod1/Mod2 ids monumod bruts");
        assert_eq!((bishi.min_grp, bishi.max_grp), (2, 2));
        assert!(bishi.auto_pos, "AutoPos=1");
        assert_eq!(bishi.tc, ["Act 1 Super A", "Act 1 (N) Super A", "Act 1 (H) Super A"]);
    }

    /// Planches `umon1-10` réelles — cellules VÉRIFIÉES à la main dans
    /// `patch_levels.txt` (2026-10-24) ; l'Id vide « Expansion » est sauté.
    #[test]
    fn real_umon_plates_match_patch_levels() {
        assert_eq!(
            umon_plate(2).unwrap(),
            ["zombie1", "fallen1", "quillrat1"],
            "Blood Moor : mon=umon ici (3 espèces)"
        );
        assert_eq!(
            umon_plate(4).unwrap(),
            ["skeleton1", "zombie2", "foulcrow1", "goatman1", "cr_archer1"],
            "Stony Field : umon ≠ mon (crownest1 → foulcrow1)"
        );
        assert_eq!(umon_plate(9).unwrap(), ["skeleton1", "zombie2", "cr_archer1"], "Cave Level 1");
        assert_eq!(umon_plate(8).unwrap(), ["zombie1", "brute1", "fallenshaman1"], "Den of Evil");
        assert_eq!(
            umon_plate(5).unwrap(),
            ["fallen2", "corruptrogue2", "quillrat2", "cr_lancer2", "sk_archer1"],
            "Dark Wood"
        );
        assert!(superuniques_for_level(u32::MAX).is_none(), "Id hors table → None");
        assert!(umon_plate(1).unwrap().is_empty(), "Camp : planche umon vide → aucun super");
        assert!(superuniques_for_level(1).unwrap().is_empty());
    }

    /// RESOLUTION RÉELLE (le brief citait « ex. » Bishibosh/Blood Moor et
    /// Rakanishu/Stony Field) : les umon de CE port tranchent — Corpsefire
    /// (zombie1) sur Blood Moor, Bonebreak (skeleton1) sur Stony Field,
    /// Bishibosh sur Den of Evil, Rakanishu (fallen2) sur Dark Wood,
    /// Coldcrow (cr_archer1) sur Cave Level 1.
    #[test]
    fn bloodmoor_spawns_corpsefire_only() {
        let sus = superuniques_for_level(2).unwrap();
        assert_eq!(sus.len(), 1, "Corpsefire seul (zombie1 ∈ planche)");
        let cf = &sus[0];
        assert_eq!(cf.name, "Corpsefire");
        assert_eq!(cf.mon_id, "zombie1");
        assert_eq!(cf.code, "ZM", "code sprite template réel");
        // Nom fr : TBL v1.12 réelle (« Corpsefire » → « Crémation »).
        assert_eq!(cf.name_fr, "Crémation");
        assert_eq!(cf.class_name, "Zombie");
        assert_eq!(cf.class_name_fr, "Zombie");
        // Mods étiquettes : Mod1=27 spectralhit ( boosts = elite_mods).
        assert_eq!(cf.mods, vec![SuModLabel { id: 27, name: "spectralhit".into() }]);
        assert_eq!((cf.min_grp, cf.max_grp), (5, 5));
        assert!(cf.auto_pos, "AutoPos=1");
        assert_eq!(cf.utrans, 25);
        assert_eq!(cf.tc[0], "Act 1 Super A");
        assert!(superuniques_for_level(2).unwrap().iter().all(|s| s.name != "Bishibosh"),
            "Bishibosh (fallenshaman1) n'est PAS dans la planche Blood Moor de ce port");
    }

    #[test]
    fn stony_field_spawns_bonebreak_not_rakanishu() {
        let sus = superuniques_for_level(4).unwrap();
        let names: Vec<&str> = sus.iter().map(|s| s.name.as_str()).collect();
        // skeleton1 ET cr_archer1 ∈ planche (umon réels) → les deux ;
        // fallen2 (Rakanishu) absent de Stony Field dans ce port.
        assert_eq!(names, ["Bonebreak", "Coldcrow"], "skeleton1+cr_archer1 ∈ planche");
        let bb = &sus[0];
        assert_eq!(bb.code, "SK");
        assert_eq!(bb.name_fr, "Concasseur d'Os", "fr réel (TBL)");
        assert_eq!(bb.class_name_fr, "Squelette");
        assert_eq!(
            bb.mods,
            vec![
                SuModLabel { id: 5, name: "strong".into() },
                SuModLabel { id: 8, name: "resist".into() },
            ],
            "Mod1/Mod2 résolus monumod"
        );
        assert!(superuniques_for_level(4).unwrap().iter().all(|s| s.name != "Rakanishu"),
            "Rakanishu (fallen2) : planche Dark Wood dans ce port");
    }

    #[test]
    fn cave1_spawns_coldcrow_and_bonebreak() {
        let sus = superuniques_for_level(9).unwrap();
        let names: Vec<&str> = sus.iter().map(|s| s.name.as_str()).collect();
        // Ordre du FICHIER superuniques.txt (hcIdx 1 < 2) — déterministe.
        assert_eq!(names, ["Bonebreak", "Coldcrow"]);
        let cc = sus.iter().find(|s| s.name == "Coldcrow").unwrap();
        assert_eq!(cc.code, "CR", "cr_archer1 → DarkRanger");
        assert_eq!(cc.class_name_fr, "Guerrier des Ténèbres");
        assert_eq!(cc.mods, vec![SuModLabel { id: 18, name: "cold".into() }], "Coldcrow = froid");
        assert_eq!((cc.min_grp, cc.max_grp), (4, 4));
    }

    #[test]
    fn den_of_evil_spawns_bishibosh_and_corpsefire() {
        let sus = superuniques_for_level(8).unwrap();
        let names: Vec<&str> = sus.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Bishibosh", "Corpsefire"], "fallenshaman1 + zombie1 ∈ planche");
        let b = sus.iter().find(|s| s.name == "Bishibosh").unwrap();
        assert_eq!(b.name_fr, "Bishibosh", "nom propre identique en fr (TBL)");
        assert_eq!(b.code, "FS");
        assert_eq!(b.class_name_fr, "Chaman Déchu", "FallenShaman fr réel");
        assert_eq!(
            b.mods,
            vec![
                SuModLabel { id: 8, name: "resist".into() },
                SuModLabel { id: 9, name: "fire".into() },
            ]
        );
    }

    #[test]
    fn dark_wood_spawns_rakanishu_autopos_false() {
        let sus = superuniques_for_level(5).unwrap();
        let names: Vec<&str> = sus.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Rakanishu"], "fallen2 ∈ planche Dark Wood");
        let r = &sus[0];
        assert_eq!(r.code, "FA", "fallen2 → Carver");
        assert_eq!(r.class_name_fr, "Boucher");
        assert_eq!((r.min_grp, r.max_grp), (8, 8), "bande de 8 officielle");
        assert!(!r.auto_pos, "AutoPos=0 : preset DS1 retail (non porté) — cf. en-tête");
        assert_eq!(r.mods, vec![
            SuModLabel { id: 17, name: "lightning".into() },
            SuModLabel { id: 6, name: "fast".into() },
        ]);
    }

    /// TCs correctes : variantes réelles `TC`/`TC(N)`/`TC(H)` ET existence
    /// dans `TreasureClassEx.txt` (A = Super A, B = Super B chez Rakanishu).
    #[test]
    fn tc_variants_per_difficulty_exist_in_treasureclassex() {
        let r = &superuniques_for_level(5).unwrap()[0]; // Rakanishu
        assert_eq!(r.tc_for(Difficulty::Normal), Some("Act 1 Super B"));
        assert_eq!(r.tc_for(Difficulty::Nightmare), Some("Act 1 (N) Super B"));
        assert_eq!(r.tc_for(Difficulty::Hell), Some("Act 1 (H) Super B"));
        for id in [2, 4, 5, 8, 9] {
            for su in superuniques_for_level(id).unwrap() {
                for d in [Difficulty::Normal, Difficulty::Nightmare, Difficulty::Hell] {
                    let tc = su.tc_for(d).expect("TC renseignée sur les sus A1");
                    assert!(tc_exists(tc), "TC « {tc} » absente de TreasureClassEx.txt");
                }
            }
        }
        assert!(!tc_exists("Act 1 Super Z"), "garde : TC inventée refusée");
    }

    /// Stats réelles : facteurs [`MonLvl`] de la ligne du TEMPLATE
    /// (zombie1 lvl 1 N → 6/8/7/2/30 ; fallenshaman1 lvl 2 N → 12/12/9/3/40),
    /// résists par difficulté (zombie1 ResPo=50 N, immunité froid 120 en H),
    /// kind SuperUnique, TC = celle du super-unique (PAS la TC1 monstats).
    #[test]
    fn spawn_rules_use_real_monlvl_per_difficulty() {
        let (stats, monlvl) = (monstats().unwrap(), monlvl().unwrap());
        let cf = &superuniques_for_level(2).unwrap()[0]; // Corpsefire / zombie1
        let n = cf.spawn_rule(Difficulty::Normal, stats, monlvl).unwrap();
        assert_eq!(n.kind, MonKind::SuperUnique);
        assert_eq!((n.ac, n.th_pct, n.hp_pct, n.dmg_pct, n.xp_pct), (6, 8, 7, 2, 30), "MonLvl ligne 1");
        assert_eq!(n.res, [0, 0, 0, 0, 0, 50], "zombie1 Normal réel (ResPo 50)");
        assert_eq!(n.treasure_class.as_deref(), Some("Act 1 Super A"), "TC su ≠ TC1 zombie1");
        let h = cf.spawn_rule(Difficulty::Hell, stats, monlvl).unwrap();
        let z = stats.get("zombie1").unwrap();
        let base_h = monlvl.row_at(z.level_for(Difficulty::Hell), Difficulty::Hell);
        assert_eq!(h.ac, base_h.ac, "AC ligne (H) du niveau 67");
        assert!(h.hp_pct > n.hp_pct * 50, "facteurs (H) >> Normal");
        assert_eq!(h.res[4], 120, "zombie1 immunité froid Enfer");
        assert_eq!(h.treasure_class.as_deref(), Some("Act 1 (H) Super A"));
        // AUCUN boost élite ici : domaine `elite_mods` (ids exposés seuls).
        assert_eq!(h.hp_pct, base_h.hp_pct);
    }

    /// Déterminisme seedé : même graine ⇒ même pack/spot ; graines
    /// différentes ⇒ spots différents (grille largement ouverte).
    #[test]
    fn seeded_determinism_group_and_spots() {
        let r = &superuniques_for_level(5).unwrap()[0]; // Rakanishu 8/8
        let mut a = Lcg::new(42);
        let mut b = Lcg::new(42);
        assert_eq!(r.group_size(&mut a), 8);
        assert_eq!(r.group_size(&mut b), 8, "même graine ⇒ même pack");
        // grp 0/0 ⇒ 1 (convention pack_size) : The Summoner-like (0/0).
        let mut c = Lcg::new(7);
        assert_eq!(pack_size(0, 0, c.below(1)), 1);
        // Spots : grille 64×64 tout marchable.
        let open = |x: usize, y: usize| x < 64 && y < 64;
        let sa = r.place(&mut a, 64, 64, open).unwrap();
        let sb = r.place(&mut b, 64, 64, open).unwrap();
        assert_eq!(sa, sb, "même graine ⇒ même spot");
        let mut seen: Vec<(usize, usize)> = vec![sa];
        for seed in 0..20u64 {
            let mut g = Lcg::new(seed ^ 0xFEED);
            if let Some(s) = r.place(&mut g, 64, 64, open) {
                if !seen.contains(&s) {
                    seen.push(s);
                    break;
                }
            }
        }
        assert_eq!(seen.len(), 2, "graine ≠ ⇒ spot ≠ (grille ouverte 64×64)");
    }

    /// Placement : respecte la grille (jamais de mur, None si rien de
    /// marchable), vaut pour AutoPos=0 (preset DS1 non porté — cf. en-tête).
    #[test]
    fn place_respects_walkable_grid() {
        let r = &superuniques_for_level(5).unwrap()[0];
        // Couloir vertical x=3 : tout spot tiré est dedans.
        let corridor = |x: usize, y: usize| x == 3 && y < 16;
        for seed in 0..30u64 {
            let mut g = Lcg::new(seed);
            let (x, y) = r.place(&mut g, 16, 16, corridor).expect("couloir → spot");
            assert_eq!(x, 3);
            assert!(y < 16);
        }
        // Grille pleine de murs : None (pas de panic, pas de boucle).
        let walls = |_: usize, _: usize| false;
        let mut g = Lcg::new(1);
        assert!(r.place(&mut g, 8, 8, walls).is_none());
        assert!(r.place(&mut g, 0, 0, walls).is_none(), "grille vide");
    }

    /// Anti-doublon boss scriptés (en-tête) : Blood Raven / Duriel /
    /// Mephisto ne sortent JAMAIS du chemin data-driven — aucun record
    /// superuniques.txt ne les porte, la garde filtre à la résolution, et
    /// leurs niveaux (cimetière 17, repaire 73, Durance 3 102) n'instancient
    /// aucun conflit de trigger Kill.
    #[test]
    fn quest_scripted_bosses_never_data_driven() {
        assert!(is_quest_scripted("bloodraven") && is_quest_scripted("DURIEL"));
        assert!(!is_quest_scripted("Rakanishu"));
        for su in su_records() {
            assert!(!is_quest_scripted(&su.name), "{} est un boss scripté ?", su.name);
            assert!(!is_quest_scripted(&su.class), "classe {} scriptée ?", su.class);
        }
        // Cimetière (id 17, Blood Raven) : Bonebreak seulement — pas de
        // corruptrogue2 (classe alias a1q2) → kill data-driven ne complète
        // pas la quête.
        let sus17 = superuniques_for_level(17).unwrap();
        let names: Vec<&str> = sus17.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Bonebreak"]);
        // Repaire de Duriel (73) / Durance 3 (102) : aucun su ne porte
        // duriel/mephisto (résolution filtrée + faits de table).
        for id in [73, 102] {
            for su in superuniques_for_level(id).unwrap() {
                assert!(!is_quest_scripted(&su.name) && !is_quest_scripted(&su.mon_id));
            }
        }
    }
}
