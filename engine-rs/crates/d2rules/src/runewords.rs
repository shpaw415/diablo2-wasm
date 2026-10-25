//! `runewords` — RUNEWORDS v1.12 (PLAN-FIDÉLITÉ phase 17a, étape 17a
//! « items fin de chaîne », 2026-10-24).
//!
//! Parse la table réelle `assets/patch_runes.txt` (169 lignes v1.12 — LA
//! source fait foi, zéro copie : `include_str!`) en recettes typées, puis
//! résout la formation d'un runeword : socle sertie + runes insérées
//! DANS L'ORDRE → nom du runeword + mods (ids ItemStatCost via
//! [`crate::item_props`]) + niveau requis. Pur, déterministe, zéro IO.
//!
//! # Règle retail de formation — CE QUE CE MODULE IMPLÉMENTE (exact)
//!
//! Sources : table `patch_runes.txt` (données) + Amazon Basin wiki « Rune
//! Words » (comportement retail communautaire documenté, cité faute de
//! source binaire) + **preuve réelle** : le save Javazon
//! (`crates/d2d2s/tests/real_javazon.rs`) décode un Enigma Jah+Ith+Ber
//! authentique (RWid 59).
//!
//! 1. **Socle blanc** : seul un item Normal/Superior (`white_base`) devient
//!    un runeword — Magic/Rare/Unique/Set/Crafted ne le peuvent pas
//!    (Basin « Rune Words », §rules ; test `socle_magique_refuse`).
//! 2. **Nombre EXACT d'alvéoles** : le socle doit porter EXACTEMENT le
//!    nombre de runes de la recette (`sockets == contents.len() ==
//!    rec.runes.len()`) — une armure 4 alvéoles ne fait PAS un Enigma de 3
//!    runes (règle retail bien connue, Basin §rules ; test
//!    `runes_en_desordre_pas_de_runeword`, cas 4 sockets).
//! 3. **Ordre d'insertion OBLIGATOIRE** : la séquence complète des runes,
//!    insérée consécutivement depuis la première alvéole, doit être
//!    ÉGALE à la séquence `Rune1..RuneN` de la recette (Jah+Ith+Ber dans
//!    CET ordre). Le contenu des alvéoles (`contents`) est la liste des
//!    runes dans L'ORDRE D'INSERTION — c'est l'ordre dans lequel le moteur
//!    retail lit les enfants sertis d'un item (preuve : le save réel stocke
//!    les enfants de l'Enigma dans l'ordre r31, r06, r30 juste après
//!    l'armure, `real_javazon.rs::real_javazon_runewords`). Retirer une
//!    rune, en ajouter, ou les réordonner désactive le runeword (aucune
//!    recette ne matche ; test `runes_en_desordre_pas_de_runeword`).
//! 4. **Type du socle** : le type de base (résolu de `patch_weapons` /
//!    `patch_armor` / `patch_misc`, colonne `type`) doit être dans les
//!    `itype1-6` de la recette et dans AUCUN `etype1-3` (exclusions — la
//!    table v1.12 n'en utilise aucune : golden test). Familles supportées :
//!    `weap` (toute arme), `mele` (arme non-distance, miroir
//!    `item_roll::NON_MELEE`), `miss` (arme distance), `armo`, et `pala`
//!    (écussons de paladin : codes pa1..paf, tous type `ashd` — vérifié
//!    record par record sur `patch_armor.txt`).
//! 5. **Premier match gagne** : parcours en ordre du fichier — sans
//!    ambiguïté réelle : les 169 séquences de runes sont deux-à-deux
//!    distinctes (golden test `sequences_uniques`).
//!
//! # Valeurs des mods (props T1Code1-7)
//!
//! * **Fixes** (`min == max`, ex. `allskills 2`) : valeur posée.
//! * **`/lvl`** (`str/lvl 6`, `mag%/lvl 8` : min/max vides, valeur dans
//!   `parN`) : quirk v1.12 géré par [`crate::item_props::evaluate`]
//!   (substitution `param`). Valeurs validées contre le save réel (str/lvl
//!   6, mag%/lvl 8 sur l'Enigma décodé).
//! * **Defense/ED variables** (`ac`, `ac%`, `dmg%` avec `max > min`) :
//!   formule retail ilvl-dépendante, `valeur = min + ((max−min) × (ilvl−1))
//!   / 98` (division entière, plafonnée à `max`, ilvl planché à 1) — Basin
//!   « Rune Words », §Variable values. **Preuves réelles** : Enigma
//!   `ac 750-775` → 775 à ilvl 99 (save : stat 31 = 775) ; Fortitude
//!   `ac%` → 200 à ilvl 99 (save : stat 16 = 200).
//! * **Autres variables** (niveaux de skills `oskill`/`aura`, `res-all`,
//!   `hp/lvl`…) : tirage UNIFORME ALÉATOIRE `min..=max` ([`crate::rng::Rng`]
//!   seedé — déterministe à graine fixe). **Preuve réelle** : le Call to
//!   Arms du save (socle ilvl 99) porte Battle Command +3 / Battle Orders
//!   +3 / Battle Cry +4 sur des ranges 2-6 / 1-6 / 1-4 — un socle ilvl 99
//!   NE donne PAS le max : ces props ne suivent PAS la formule ilvl.
//! * Accroches skill (`charged`, procs, `oskill`…) : structures minimales
//!   [`crate::item_props::SkillProp`], aucun comportement runtime
//!   (convention gelée `item_props`, étape 12b).
//!
//! # Ids save .d2s
//!
//! Le save binaire identifie un runeword par `n° de la colonne `Name`
//! ("Runeword33") + 26` ([`SAVE_ID_OFFSET`]) — convention vérifiée sur les
//! TROIS runewords du save réel : Enigma 33+26 = 59, Call to Arms
//! 13+26 = 39, Fortitude 41+26 = 67 (`real_javazon.rs`).
//!
//! # Divergences documentées (assumées)
//!
//! ① **Niveau requis** : dérivé des runes (`levelreq` de `patch_misc.txt`,
//!    max des runes de la recette — Hel : quirk `levelreq 0` ⇒ repli
//!    `level`). La règle retail exacte dérive du niveau des MODS via
//!    `properties.txt` (non embarquée) — ex. Obedience : 43 ici vs 41
//!    retail ; Enigma : 65 = 65 retail ( Jah ), golden test.
//! ② **Paires élémentaires** (`dmg-fire`/`dmg-ltng`/`dmg-cold`/`dmg-pois`/
//!    `dmg-mag`, `lifesteal`, `manasteal`) : [`crate::item_props`] (contrat
//!    gelé) émet la MÊME valeur aux deux stats — le retail stocke la paire
//!    (min, max). Insensible sur les valeurs fixes (Black `dmg% 120 120`) ;
//!    documenté, non contourné.
//! ③ **Colonne `server`** (23 lignes v1.12, marqueur ladder) : exposée sur
//!    [`RuneWordRec`] mais le matching ne filtre PAS dessus (SP/offline =
//!    contenu ladder accessible, choix campagne).
//! ④ Une recette s'applique TELLE QUELLE à toute base valide (Fortitude
//!    armure porte aussi le `dmg% 300` de sa ligne — inapplicable/ non
//!    affiché, le retail procède pareil).

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::item_props::{self, PropDescriptor, RolledProp};
use crate::rng::Rng;
use crate::statlist::ItemStats;
use crate::tsv::Tsv;

/// `patch_runes.txt` v1.12 — LA source fait foi : 169 lignes = 83 recettes
/// typées (runes présentes) + 86 lignes stubs vides (Name seul, jamais
/// matchables — une recette sans runes ne peut égaler aucun contenu).
const RUNES_TXT: &str = include_str!("../../../../assets/patch_runes.txt");
/// Bases v1.12 : code → type (colonne `type`) pour le matching `itype`.
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");

/// Types d'armes HORS famille `mele` (distance/jet — miroir item_roll.rs).
const NON_MELEE: &[&str] = &["bow", "xbow", "abow", "jave", "ajav", "tkni", "tpot"];

/// Décalage des ids de runeword dans les saves `.d2s` : id stocké =
/// n° `Name` ("Runeword33") + 26. Vérifié sur les 3 runewords du save réel
/// (`crates/d2d2s/tests/real_javazon.rs` : 59, 39, 67).
pub const SAVE_ID_OFFSET: u32 = 26;

/// Famille d'une base (résolue de la table d'origine du code).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BaseKind {
    /// Trouvé dans `patch_weapons.txt`.
    Weapon,
    /// Trouvé dans `patch_armor.txt`.
    Armor,
    /// Trouvé dans `patch_misc.txt` (aucun runeword ne s'applique).
    Misc,
}

/// Une recette typée de `patch_runes.txt` (ordre du fichier = priorité).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuneWordRec {
    /// Ligne 1-based du fichier (`patch_runes.txt`) — golden tests.
    pub line: usize,
    /// N° de la colonne `Name` ("Runeword33" → 33) ; id save = ceci + 26.
    pub id: u32,
    /// Nom du runeword (colonne `Rune Name`, ex. "Enigma").
    pub name: String,
    /// Colonne `complete` (marqueur de complétude de la table v1.12).
    pub complete: bool,
    /// Colonne `server` (marqueur ladder v1.12 — non filtrée au matching,
    /// divergence ③).
    pub server: bool,
    /// Types de socle valides (`itype1-6`, familles `weap`/`mele`/`miss`…).
    pub itypes: Vec<String>,
    /// Types exclus (`etype1-3` — aucune ligne réelle ne les utilise).
    pub etypes: Vec<String>,
    /// Runes ORDONNÉES (`Rune1..Rune6`, ex. ["r31", "r06", "r30"]).
    pub runes: Vec<String>,
    /// Mods T1 (`T1Code1-7` + param/min/max) — descripteurs bruts.
    pub props: Vec<PropDescriptor>,
}

impl RuneWordRec {
    /// Id tel que stocké dans les saves `.d2s` (n° `Name` + 26).
    pub fn save_id(&self) -> u32 {
        self.id + SAVE_ID_OFFSET
    }
}

/// Résultat appliqué d'un runeword : mods roulés + stats évaluées.
#[derive(Debug, Clone, PartialEq)]
pub struct RunewordResult {
    /// N° de recette (colonne `Name`).
    pub id: u32,
    /// Nom du runeword (ex. "Enigma").
    pub name: String,
    /// Ligne 1-based de la recette dans `patch_runes.txt` (audit).
    pub line: usize,
    /// Niveau requis (divergence ① : dérivé des runes, pas des mods).
    pub min_level: u32,
    /// Props roulées (valeur ilvl-formule ou tirage seedé).
    pub props: Vec<RolledProp>,
    /// Stats évaluées (ids ItemStatCost — pont `statlist`).
    pub stats: ItemStats,
}

/// Un socle sertie + son contenu, TEL QUE LU du moteur (ou d'un save).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunewordItem<'a> {
    /// Code table de la BASE (ex. "uui" Ghost Armor, "clb" Club).
    pub base_code: &'a str,
    /// Niveau d'item du socle (roule les defense/ED variables).
    pub ilvl: u32,
    /// Nb d'alvéoles du socle — doit être EXACT (règle 2).
    pub sockets: u8,
    /// Contenu des alvéoles DANS L'ORDRE D'INSERTION (codes runes).
    pub contents: &'a [String],
    /// Socle « blanc » (Normal/Superior) ? — règle 1.
    pub white_base: bool,
}

/// Tables des runewords : recettes typées + types de bases + niveau des runes.
#[derive(Debug, Clone, Default)]
pub struct RuneWordTable {
    recs: Vec<RuneWordRec>,
    bases: HashMap<String, (String, BaseKind)>,
    rune_req: HashMap<String, u32>,
}

impl RuneWordTable {
    /// Parse `patch_runes.txt` + les 3 tables de bases. `None` si une table
    /// est illisible (jamais paniquer).
    pub fn parse(runes: &str, weapons: &str, armor: &str, misc: &str) -> Option<RuneWordTable> {
        let mut table = RuneWordTable::default();
        let tsv = Tsv::parse(runes).ok()?;
        for row in 0..tsv.len() {
            let r = tsv.rec(row)?;
            let mut rec = RuneWordRec {
                line: row + 2, // en-tête ligne 1 ; patch_runes.txt n'a pas de ligne vide
                id: r.s("Name").trim_start_matches("Runeword").parse().ok()?,
                name: r.s("Rune Name").to_string(),
                complete: r.s("complete") == "1",
                server: r.s("server") == "1",
                itypes: Vec::new(),
                etypes: Vec::new(),
                runes: Vec::new(),
                props: Vec::new(),
            };
            for c in ["itype1", "itype2", "itype3", "itype4", "itype5", "itype6"] {
                if !r.s(c).is_empty() {
                    rec.itypes.push(r.s(c).to_string());
                }
            }
            for c in ["etype1", "etype2", "etype3"] {
                if !r.s(c).is_empty() {
                    rec.etypes.push(r.s(c).to_string());
                }
            }
            for c in ["Rune1", "Rune2", "Rune3", "Rune4", "Rune5", "Rune6"] {
                if !r.s(c).is_empty() {
                    rec.runes.push(r.s(c).to_string());
                }
            }
            for n in 1..=7 {
                let code = r.s(&format!("T1Code{n}"));
                if code.is_empty() {
                    continue;
                }
                rec.props.push(PropDescriptor {
                    code: code.to_string(),
                    param: r.s(&format!("T1Param{n}")).to_string(),
                    min: r.i(&format!("T1Min{n}")),
                    max: r.i(&format!("T1Max{n}")),
                });
            }
            table.recs.push(rec);
        }
        table.bases = parse_bases(weapons, BaseKind::Weapon);
        for (k, v) in parse_bases(armor, BaseKind::Armor) {
            table.bases.insert(k, v);
        }
        for (k, v) in parse_bases(misc, BaseKind::Misc) {
            table.bases.entry(k).or_insert(v);
        }
        // Niveau requis des runes (`patch_misc.txt` r01..r33) — divergence ①.
        if let Ok(m) = Tsv::parse(misc) {
            for row in 0..m.len() {
                let r = m.rec(row)?;
                let code = r.s("code");
                if code.len() == 3 && code.starts_with('r') && code[1..].bytes().all(|b| b.is_ascii_digit()) {
                    let req = r.u("levelreq");
                    let req = if req > 0 { req } else { r.u("level") };
                    table.rune_req.insert(code.to_string(), req);
                }
            }
        }
        Some(table)
    }

    /// Table globale embarquée (parse au premier accès).
    pub fn global() -> &'static RuneWordTable {
        static TABLE: OnceLock<RuneWordTable> = OnceLock::new();
        TABLE.get_or_init(|| {
            RuneWordTable::parse(RUNES_TXT, WEAPONS_TXT, ARMOR_TXT, MISC_TXT)
                .expect("patch_runes.txt + tables de bases réelles doivent parser")
        })
    }

    /// Les recettes, en ordre du fichier (premier match gagne — règle 5).
    /// Les lignes stubs sans runes y figurent mais ne matchent jamais.
    pub fn recs(&self) -> &[RuneWordRec] {
        &self.recs
    }

    /// Recette par nom (`Rune Name`, ex. "Enigma") ; None si inconnue.
    pub fn find(&self, name: &str) -> Option<&RuneWordRec> {
        self.recs.iter().find(|r| r.name == name)
    }

    /// Type de base d'un code (`None` = code absent des 3 tables).
    pub fn type_of(&self, code: &str) -> Option<&str> {
        self.bases.get(code).map(|(t, _)| t.as_str())
    }

    /// Le code `itype`/`etype` d'une recette matche-t-il la base ?
    /// Types directs (`tors`, `club`, `h2h`…) + familles `weap`/`armo`/
    /// `mele`/`miss`/`pala` (doc module, règle 4).
    pub fn type_matches(&self, itype: &str, base_code: &str) -> bool {
        let Some((t, kind)) = self.bases.get(base_code) else {
            return false;
        };
        if itype == t {
            return true;
        }
        match itype {
            "weap" => *kind == BaseKind::Weapon,
            "armo" => *kind == BaseKind::Armor,
            "mele" => *kind == BaseKind::Weapon && !NON_MELEE.contains(&t.as_str()),
            "miss" => *kind == BaseKind::Weapon && NON_MELEE.contains(&t.as_str()),
            // Écussons de paladin : les 15 bases pa1..paf de patch_armor.txt
            // (toutes type `ashd`) — aucun autre socle ne porte ce préfixe.
            "pala" => *kind == BaseKind::Armor && t == "ashd" && base_code.starts_with("pa"),
            _ => false,
        }
    }

    /// Résout la formation d'un runeword (règles 1-5, doc module) —
    /// `Some(rec)` = le runeword EST actif sur ce socle.
    pub fn resolve<'a>(&'a self, item: &RunewordItem<'_>) -> Option<&'a RuneWordRec> {
        // ① socle blanc ; ② nb EXACT d'alvéoles, toutes remplies.
        if !item.white_base {
            return None;
        }
        if item.sockets == 0 || item.contents.len() != item.sockets as usize {
            return None;
        }
        let n = item.contents.len();
        for rec in &self.recs {
            // ③ séquence complète égale, ordre d'insertion respecté.
            if rec.runes.len() != n {
                continue;
            }
            if !rec
                .runes
                .iter()
                .zip(item.contents.iter())
                .all(|(r, c)| r == &c.as_str())
            {
                continue;
            }
            // ④ type du socle valide (itype + pas d'etype).
            if rec
                .itypes
                .iter()
                .any(|t| self.type_matches(t, item.base_code))
                && !rec
                    .etypes
                    .iter()
                    .any(|t| self.type_matches(t, item.base_code))
            {
                return Some(rec);
            }
        }
        None
    }

    /// Niveau requis (divergence ①) : max des `levelreq` des runes
    /// (`patch_misc.txt` v1.12 ; Hel : `levelreq 0` ⇒ repli `level`).
    pub fn min_level(&self, rec: &RuneWordRec) -> u32 {
        rec.runes
            .iter()
            .filter_map(|r| self.rune_req.get(r))
            .copied()
            .max()
            .unwrap_or(0)
    }

    /// Applique une recette résolue : props roulées (ilvl-formule pour
    /// defense/ED, tirage seedé pour le reste — doc module) puis stats
    /// évaluées via [`crate::item_props`] → [`ItemStats`]. Déterministe.
    pub fn apply(&self, rec: &RuneWordRec, ilvl: u32, rng: &mut Rng) -> RunewordResult {
        let props: Vec<RolledProp> = rec
            .props
            .iter()
            .map(|d| {
                let value = prop_value(d, ilvl, rng);
                RolledProp { desc: d.clone(), value, typed: item_props::interpret(d, value) }
            })
            .collect();
        let stats = ItemStats::from_props(&props);
        RunewordResult {
            id: rec.id,
            name: rec.name.clone(),
            line: rec.line,
            min_level: self.min_level(rec),
            props,
            stats,
        }
    }
}

/// Valeur d'une prop de runeword : ilvl-formule retail pour `ac`/`ac%`/
/// `dmg%` variables (preuves save : Enigma 775, Fortitude 200 à ilvl 99),
/// tirage seedé pour les autres variables, fixe sinon (doc module).
fn prop_value(desc: &PropDescriptor, ilvl: u32, rng: &mut Rng) -> i32 {
    if desc.max > desc.min
        && matches!(
            desc.code.trim().to_ascii_lowercase().as_str(),
            "ac" | "ac%" | "dmg%"
        )
    {
        let lo = desc.min as i64;
        let span = (desc.max - desc.min) as i64;
        let il = ilvl.max(1) as i64;
        (lo + span * (il - 1) / 98).min(desc.max as i64) as i32
    } else {
        item_props::roll(desc, rng).value
    }
}

/// Map `code → (type, famille)` d'une table de bases (colonnes `code`/`type`).
fn parse_bases(txt: &str, kind: BaseKind) -> HashMap<String, (String, BaseKind)> {
    let mut out = HashMap::new();
    if let Ok(t) = Tsv::parse(txt) {
        for row in 0..t.len() {
            let Some(r) = t.rec(row) else { continue };
            let (code, ty) = (r.s("code"), r.s("type"));
            if !code.is_empty() && !ty.is_empty() {
                out.entry(code.to_string()).or_insert((ty.to_string(), kind));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Tests — goldens contre les tables réelles embarquées + le save réel.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::StatCostTable;

    /// Id numérique d'une stat ItemStatCost par nom (table réelle embarquée).
    fn sid(name: &str) -> i32 {
        StatCostTable::global().get(name).unwrap().id
    }

    const ENIGMA_RUNES: [&str; 3] = ["r31", "r06", "r30"]; // Jah+Ith+Ber

    /// Socle Enigma du save réel : Ghost Armor (uui) sup 3 alvéoles, ilvl 99.
    fn ghost(contents: &[&str], ilvl: u32, sockets: u8) -> RunewordItem<'static> {
        let owned: Vec<String> = contents.iter().map(|c| c.to_string()).collect();
        RunewordItem {
            base_code: "uui",
            ilvl,
            sockets,
            contents: Box::leak(owned.into_boxed_slice()),
            white_base: true,
        }
    }

    // ══ Parse de la table réelle ══

    #[test]
    fn real_table_golden() {
        let t = RuneWordTable::global();
        assert_eq!(t.recs.len(), 169, "patch_runes.txt v1.12 : 169 lignes");
        // 83 recettes typées (runes présentes) + 86 lignes STUBS vides
        // (Name seul — placeholders réels de la table, jamais matchables :
        // une recette sans runes ne peut pas égaler un contenu non vide).
        assert_eq!(t.recs.iter().filter(|r| !r.runes.is_empty()).count(), 83);
        assert_eq!(t.recs.iter().filter(|r| r.runes.is_empty()).count(), 86);
        assert!(t.recs.iter().all(|r| r.runes.is_empty() && r.props.is_empty()
            || !r.runes.is_empty()));
        // Enigma — ligne 33, citée partout (save réel RWid 59).
        let enigma = t.find("Enigma").unwrap();
        assert_eq!(enigma.line, 33);
        assert_eq!(enigma.id, 33);
        assert_eq!(enigma.save_id(), 59, "RWid du save réel (real_javazon)");
        assert!(enigma.complete && !enigma.server);
        assert_eq!(enigma.itypes, ["tors"]);
        assert!(enigma.etypes.is_empty());
        assert_eq!(enigma.runes, ENIGMA_RUNES);
        assert_eq!(enigma.props.len(), 7);
        assert_eq!(enigma.props[0].code, "ac");
        assert_eq!((enigma.props[0].min, enigma.props[0].max), (750, 775));
        // Black — ligne 3 (RW d'arme du golden).
        let black = t.find("Black").unwrap();
        assert_eq!(black.line, 3);
        assert_eq!(black.id, 6);
        assert_eq!(black.itypes, ["club", "hamm", "mace"]);
        assert_eq!(black.runes, ["r10", "r16", "r04"]);
        // Call to Arms + Fortitude — les 2 autres runewords du save réel.
        let cta = t.find("Call to Arms").unwrap();
        assert_eq!((cta.line, cta.id), (29, 13));
        assert_eq!(cta.save_id(), 39, "RWid du save réel (real_javazon)");
        assert_eq!(cta.runes, ["r11", "r08", "r23", "r24", "r27"]);
        let fort = t.find("Fortitude").unwrap();
        assert_eq!((fort.line, fort.id), (68, 41));
        assert_eq!(fort.save_id(), 67, "RWid du save réel (real_javazon)");
        assert_eq!(fort.itypes, ["weap", "tors"]);
        assert!(fort.server, "colonne server posée sur cette ligne v1.12");
        // Marqueurs de table : 78 complete, 23 server, 0 etype utilisés.
        assert_eq!(t.recs.iter().filter(|r| r.complete).count(), 78);
        assert_eq!(t.recs.iter().filter(|r| r.server).count(), 23);
        assert!(
            t.recs.iter().all(|r| r.etypes.is_empty()),
            "aucune exclusion etype dans les données v1.12 (règle vacuelle)"
        );
    }

    /// « Premier match gagne » est sans ambiguïté : séquences des recettes
    /// réelles 2-à-2 uniques (les 86 stubs vides sont exclus d'office).
    #[test]
    fn sequences_uniques() {
        let t = RuneWordTable::global();
        let real: Vec<&RuneWordRec> =
            t.recs.iter().filter(|r| !r.runes.is_empty()).collect();
        for (i, a) in real.iter().enumerate() {
            for b in real.iter().skip(i + 1) {
                assert_ne!(a.runes, b.runes, "{} vs {}", a.name, b.name);
            }
        }
    }

    /// Niveau requis des runes (golden `patch_misc.txt` r01..r33) + exemples.
    #[test]
    fn rune_levelreq_golden() {
        let t = RuneWordTable::global();
        assert_eq!(t.rune_req.len(), 33, "33 runes v1.12");
        assert_eq!(t.rune_req["r01"], 11, "El 11");
        assert_eq!(t.rune_req["r31"], 65, "Jah 65");
        assert_eq!(t.rune_req["r33"], 69, "Zod 69");
        let mut prev = 0;
        for n in 1..=33 {
            let req = t.rune_req[&format!("r{n:02}")];
            assert!(req >= prev, "monotone : r{n:02}");
            prev = req;
        }
        // Enigma rlvl 65 = retail (Jah) ; Black : Thul 23 / Io 35 / Nef 13.
        assert_eq!(t.min_level(t.find("Enigma").unwrap()), 65);
        assert_eq!(t.min_level(t.find("Black").unwrap()), 35);
    }

    // ══ Golden Enigma — VALIDATION CONTRE LE SAVE RÉEL ══
    // Vérité terrain : crates/d2d2s/tests/real_javazon.rs décode, du fichier
    // Javazon.d2s AUTHENTIQUE, l'Enigma Jah+Ith+Ber (Ghost Armor sup, 3
    // alvéoles, RWid 59) avec ses 7 mods : 31=775, 86=14, 96=45, 97
    // (param 54)=1, 127=2, 220=6, 240=8.

    #[test]
    fn golden_enigma_vs_real_save() {
        let t = RuneWordTable::global();
        let item = ghost(&ENIGMA_RUNES, 99, 3);
        let rec = t.resolve(&item).expect("Enigma doit se former");
        assert_eq!(rec.name, "Enigma");
        let mut rng = Rng::new(1);
        let rw = t.apply(rec, 99, &mut rng);
        // Props ac 750-775 → 775 à ilvl 99 (formule ilvl, save : stat 31).
        assert_eq!(rw.stats.stats.get(sid("armorclass")), 775);
        assert_eq!(rw.stats.stats.get(sid("item_healafterkill")), 14); // save 86
        assert_eq!(rw.stats.stats.get(sid("item_fastermovevelocity")), 45); // save 96
        assert_eq!(rw.stats.stats.get(sid("item_strength_perlevel")), 6); // save 220
        assert_eq!(rw.stats.stats.get(sid("item_allskills")), 2); // save 127
        assert_eq!(rw.stats.stats.get(sid("item_find_magic_perlevel")), 8); // save 240
        // +1 Téléport : id 97 param 54 dans le save — accroche oskill ici.
        let skills = &rw.stats.skills;
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].kind, item_props::SkillPropKind::NonClass);
        assert_eq!(skills[0].skill_id, 54, "Teleport (save : param 54)");
        assert_eq!(skills[0].level, 1);
        assert_eq!(rw.stats.param_stats.len(), 0);
        // L'ensemble des stats émises = l'ensemble des mods rw du save
        // (ids triés ; le save porte en plus l'accroche 97 param 54).
        let mut ids: Vec<i32> = rw.stats.stats.iter().map(|(id, _)| id).collect();
        ids.sort_unstable();
        assert_eq!(ids, vec![31, 86, 96, 127, 220, 240]);
        assert_eq!(rw.min_level, 65, "rlvl retail Enigma (Jah)");
    }

    /// Formule ilvl des defense/ED variables (Enigma `ac 750-775`).
    #[test]
    fn variables_defense_suivent_l_ilvl() {
        let t = RuneWordTable::global();
        let enigma = t.find("Enigma").unwrap();
        for (ilvl, expected) in [(1u32, 750i64), (50, 762), (99, 775), (200, 775)] {
            let item = ghost(&ENIGMA_RUNES, ilvl, 3);
            let rw = t.apply(t.resolve(&item).unwrap(), ilvl, &mut Rng::new(7));
            assert_eq!(
                rw.stats.stats.get(sid("armorclass")),
                expected,
                "ilvl {ilvl}"
            );
        }
        assert_eq!(enigma.props[0].min, 750, "planche de la recette");
    }

    // ══ Golden Fortitude (2e runeword du save réel) ══

    #[test]
    fn golden_fortitude_armure() {
        let t = RuneWordTable::global();
        // utp = Archon Plate (type tors), 4 alvéoles, El+Sol+Dol+Lo.
        let runes = ["r01", "r12", "r14", "r28"].map(String::from);
        let item = RunewordItem {
            base_code: "utp",
            ilvl: 99,
            sockets: 4,
            contents: &runes,
            white_base: true,
        };
        let rw = t.apply(t.resolve(&item).unwrap(), 99, &mut Rng::new(3));
        // ac% 200 fixe (save : stat 16 = 200) ; dmg% 300 fixe (paire 17/18).
        assert_eq!(rw.stats.stats.get(sid("item_armor_percent")), 200);
        assert_eq!(rw.stats.stats.get(sid("item_maxdamage_percent")), 300);
        assert_eq!(rw.stats.stats.get(sid("item_mindamage_percent")), 300);
        assert_eq!(rw.stats.stats.get(sid("item_fastercastrate")), 25); // save 105
        assert_eq!(rw.stats.stats.get(sid("item_damagetomana")), 12); // save 114
        // res-all 25-30 : tiré UNE fois, réparti égal sur les 4 résistances.
        let res = rw.stats.stats.get(sid("fireresist"));
        assert!((25..=30).contains(&res), "range recette 25-30 : {res}");
        for n in ["lightresist", "coldresist", "poisonresist"] {
            assert_eq!(rw.stats.stats.get(sid(n)), res, "res-all égal");
        }
        // hp/lvl 8-12 : variable non-defense → tirage (PAS la formule ilvl).
        let hp = rw.stats.stats.get(sid("item_hp_perlevel"));
        assert!((8..=12).contains(&hp), "hp/lvl dans le range : {hp}");
        // gethit-skill Chilling Armor (20 % chance, niveau 15).
        let sk = &rw.stats.skills;
        assert_eq!(sk.len(), 1);
        assert_eq!(sk[0].kind, item_props::SkillPropKind::OnGetHit);
        assert_eq!(sk[0].prob, 20);
        assert_eq!(sk[0].level, 15);
        assert!(sk[0].skill_id > 0, "Chilling Armor résolu de patch_skills");
    }

    // ══ Golden Black — RW d'ARME (patch_runes.txt LIGNE 3) ══

    #[test]
    fn golden_weapon_black_ligne_3() {
        let t = RuneWordTable::global();
        // Club (clb, type club) — itype1 de la ligne 3, Thul+Io+Nef.
        let runes = ["r10", "r16", "r04"].map(String::from);
        let item = RunewordItem {
            base_code: "clb",
            ilvl: 1,
            sockets: 3,
            contents: &runes,
            white_base: true,
        };
        let rec = t.resolve(&item).expect("Black doit se former");
        assert_eq!(rec.line, 3, "ligne citée : patch_runes.txt ligne 3");
        let rw = t.apply(rec, 1, &mut Rng::new(9));
        assert_eq!(rw.name, "Black");
        assert_eq!(rw.min_level, 35, "Io 35");
        assert_eq!(rw.stats.stats.get(sid("item_crushingblow")), 40);
        assert_eq!(rw.stats.stats.get(sid("tohit")), 200);
        assert_eq!(rw.stats.stats.get(sid("item_fasterattackrate")), 15);
        assert_eq!(rw.stats.stats.get(sid("magic_damage_reduction")), 2);
        assert_eq!(rw.stats.stats.get(sid("item_mindamage_percent")), 120);
        assert_eq!(rw.stats.stats.get(sid("item_maxdamage_percent")), 120);
        // charged : Level 4 Corpse Explosion (skill 74), 12 charges.
        let sk = &rw.stats.skills;
        assert_eq!(sk.len(), 1);
        assert_eq!(sk[0].kind, item_props::SkillPropKind::Charged);
        assert_eq!(sk[0].skill_id, 74, "Corpse Explosion");
        assert_eq!(sk[0].charges, 12);
        assert_eq!(sk[0].level, 4);
    }

    // ══ Ordre d'insertion = règle retail : mutations ⇒ PAS de runeword ══

    #[test]
    fn runes_en_desordre_pas_de_runeword() {
        let t = RuneWordTable::global();
        // Réordonné (Ith+Jah+Ber, Ber+Ith+Jah) : non.
        for seq in [["r06", "r31", "r30"], ["r30", "r06", "r31"]] {
            assert!(t.resolve(&ghost(&seq, 99, 3)).is_none(), "{seq:?}");
        }
        // Séquence incomplète (rune retirée) : non.
        assert!(t.resolve(&ghost(&["r31", "r06"], 99, 2)).is_none());
        // 4e alvéole (socle 4 sockets — nb EXACT, règle 2) : non.
        assert!(t
            .resolve(&ghost(&["r31", "r06", "r30", "r07"], 99, 4))
            .is_none());
        // Alvéoles déclarées ≠ contenu rempli : non.
        assert!(t.resolve(&ghost(&ENIGMA_RUNES, 99, 5)).is_none());
        // Rune inconnue dans la séquence : non.
        assert!(t.resolve(&ghost(&["r31", "r06", "buc"], 99, 3)).is_none());
    }

    // ══ Types : socle valide / invalide / familles ══

    #[test]
    fn socle_magique_refuse() {
        let t = RuneWordTable::global();
        let mut item = ghost(&ENIGMA_RUNES, 99, 3);
        item.white_base = false; // Magic/Rare/Unique : jamais de runeword.
        assert!(t.resolve(&item).is_none());
    }

    #[test]
    fn mauvais_type_pas_de_runeword() {
        let t = RuneWordTable::global();
        // Enigma (tors) sur une épée (crs) ou un casque (cap) : non.
        let seq = ENIGMA_RUNES.map(String::from);
        for base in ["crs", "cap"] {
            let item = RunewordItem {
                base_code: base,
                ilvl: 99,
                sockets: 3,
                contents: &seq,
                white_base: true,
            };
            assert!(t.resolve(&item).is_none(), "{base}");
        }
        // Black (club/hamm/mace) sur une épée : non ; sur un club : oui.
        let black = ["r10", "r16", "r04"].map(String::from);
        let clb = RunewordItem {
            base_code: "crs",
            ilvl: 1,
            sockets: 3,
            contents: &black,
            white_base: true,
        };
        assert!(t.resolve(&clb).is_none());
        assert!(t.type_of("clb") == Some("club"));
    }

    /// Familles `mele`/`miss`/`pala` (règle 4) — Fury / Exile's Path.
    #[test]
    fn familles_de_types() {
        let t = RuneWordTable::global();
        // Fury (Jah+Gul+Eth, itype `mele`) : épée oui, casque non.
        let fury = ["r31", "r25", "r05"].map(String::from);
        let crs = RunewordItem {
            base_code: "crs",
            ilvl: 99,
            sockets: 3,
            contents: &fury,
            white_base: true,
        };
        assert_eq!(t.resolve(&crs).unwrap().name, "Fury");
        let cap = RunewordItem {
            base_code: "cap",
            ilvl: 99,
            sockets: 3,
            contents: &fury,
            white_base: true,
        };
        assert!(t.resolve(&cap).is_none());
        // Exile's Path (itype `pala`) : Targe (pa1) oui, Buckler (buc) non.
        let exile = ["r26", "r27", "r24", "r14"].map(String::from);
        let pa1 = RunewordItem {
            base_code: "pa1",
            ilvl: 99,
            sockets: 4,
            contents: &exile,
            white_base: true,
        };
        assert_eq!(t.resolve(&pa1).unwrap().name, "Exile's Path");
        let buc = RunewordItem {
            base_code: "buc",
            ilvl: 99,
            sockets: 4,
            contents: &exile,
            white_base: true,
        };
        assert!(t.resolve(&buc).is_none(), "buckler n'est pas un écusson pala");
    }

    /// Règle `etype` (aucune ligne réelle) : mini-table synthétique.
    #[test]
    fn etype_exclut_le_socle() {
        // Recette : rune r99, valide sur les épées SAUF "swor" (etype1).
        let runes = "Name\tRune Name\tcomplete\tserver\titype1\tetype1\tRune1\tT1Code1\tT1Param1\tT1Min1\tT1Max1\r\nRuneword99\tTest\t1\t\tswor\tswor\tr99\tac\t\t1\t1\r\n";
        let weapons = "code\ttype\r\ncrs\tswor\r\nclb\tclub\r\n";
        let armor = "code\ttype\r\ncap\thelm\r\n";
        let misc = "code\ttype\tlevel\tlevelreq\r\nr99\trune\t42\t42\r\n";
        let t = RuneWordTable::parse(runes, weapons, armor, misc).unwrap();
        let seq = ["r99"].map(String::from);
        let crs = RunewordItem {
            base_code: "crs",
            ilvl: 1,
            sockets: 1,
            contents: &seq,
            white_base: true,
        };
        assert!(t.resolve(&crs).is_none(), "etype swor exclut l'épée");
        // min_level dérivé du misc synthétique.
        let rw = t.apply(t.find("Test").unwrap(), 1, &mut Rng::new(1));
        assert_eq!(rw.min_level, 42);
    }

    /// Déterminisme : même graine ⇒ résultat identique (Bit à bit, PartialEq).
    #[test]
    fn determinisme() {
        let t = RuneWordTable::global();
        let item = ghost(&ENIGMA_RUNES, 99, 3);
        let rec = t.resolve(&item).unwrap();
        let a = t.apply(rec, 99, &mut Rng::new(42));
        let b = t.apply(rec, 99, &mut Rng::new(42));
        assert_eq!(a, b);
        // Et une graine différente ne touche PAS les props fixes.
        let c = t.apply(rec, 99, &mut Rng::new(43));
        assert_eq!(a.stats.stats.get(sid("armorclass")), c.stats.stats.get(sid("armorclass")));
        assert_eq!(a.stats.stats.get(sid("item_allskills")), c.stats.stats.get(sid("item_allskills")));
    }
}
