//! Sons monstres data-driven — étape 14a « audio » (campagne diablo-2-wasm,
//! PLAN-FIDÉLITÉ phase 14, 2026-10-24).
//!
//! Parse la table officielle `assets/monsounds.txt` (141 records v1.12 :
//! famille sonore → événements attaque/touché/mort/annonce) via le parseur
//! TSV générique [`crate::tsv`], résout un JETON monstats (`zombie2`,
//! `fallenshaman1`, `ancientbarb1`…) vers sa famille et expose les fichiers
//! de la table. Zéro IO, tables embarquées `include_str!` (modèle `spawn.rs`).
//!
//! # Deux couches, une seule source de vérité par couche
//! * TABLE ([`MONSOUNDS_TXT`]) — les noms de sons v1.12 réels (cellules
//!   `Attack1`/`HitSound`/`DeathSound`/`Taunt`), consommés par [`event_file`].
//! * FICHIERS LOCAUX ([`INDEX_JSON`]) — ce qui est réellement embarqué dans le
//!   port : `demo/assets/sfx/monsounds/` + `index.json`, produits par
//!   `tools/extract_mon_sfx.py` depuis les MPQ LECTURE SEULE (sons :
//!   `data\global\sfx\monster\…` de d2sfx.mpq ; annonces :
//!   `data\local\sfx\act*\…` de d2speech.mpq ; inventaire des noms via
//!   `data\global\excel\sounds.txt`, col `FileName`). NOTE : les chemins
//!   retail `data\global\monsounds\…` sont ABSENTS de cette release digitale
//!   (sweep 568 sondes = 0 hit) ; les dossiers taillés (imp, overseer, baal…)
//!   n'ont pas de fichier → repli générique côté JS, sans invention.
//!
//! Les golden tests vérifient : valeurs TABLE record-à-record, présence sur
//! disque + en-tête RIFF des wav indexés, et PARITÉ avec le lecteur JS
//! `demo/mon-sfx.js` (qui embarque le même mapping, incl. les codes sprites
//! [`PORT_CODES`]).
//!
//! # Résolution jeton → famille ([`family_of`])
//! Chiffres finaux retirés (`zombie2` → `zombie`), puis alias documentés
//! ([`ALIAS`] : `ancientbarb→ancient`, `griswold→zombieunique`,
//! `nihlathakboss→nihlathak`, `hephasto→smithdemon`, `skmage→sk_mage`), en
//! remontant les `_` (`skmage_pois` → `skmage`). Familles SANS ligne
//! (`venomlord`, `baalhighpriest`, `snowyeti`, `succubuswitch`) → `None` :
//! la table v1.12 embarquée ne les connaît pas, le JS reste en générique.

use crate::tsv::Tsv;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;

const MONSOUNDS_TXT: &str = include_str!("../../../../assets/monsounds.txt");
const INDEX_JSON: &str = include_str!("../../../../demo/assets/sfx/monsounds/index.json");
const MON_SFX_JS: &str = include_str!("../../../../demo/mon-sfx.js");

/// Alias tige → ligne monsounds, décidés sur les données du port (cf. en-tête
/// et rapport d'étape) — jamais une invention de sons.
const ALIAS: [(&str, &str); 5] = [
    ("ancientbarb", "ancient"),     // Anciens d'Arreat = sons barbare (ligne « ancient »)
    ("griswold", "zombieunique"),   // Griswold = zombie unique (ligne dédiée v1.12)
    ("nihlathakboss", "nihlathak"), // boss de Nihlathak → ligne du PNJ/boss
    ("hephasto", "smithdemon"),     // col MonSound de superuniques.txt = smithdemon
    ("skmage", "sk_mage"),          // mage squelette (lignes sk_mage/necromage identiques)
];

/// Événement sonore d'un monstre — les 4 colonnes câblées de `monsounds.txt`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MonSfxEvent {
    /// Attaque du monstre (col `Attack1`).
    Attack,
    /// Le monstre encaisse un coup (col `HitSound`).
    Hit,
    /// Mort du monstre (col `DeathSound`).
    Death,
    /// Annonce de super-unique/boss (col `Taunt`).
    Taunt,
}

impl MonSfxEvent {
    /// Colonne `monsounds.txt` portant l'événement.
    pub fn column(self) -> &'static str {
        match self {
            MonSfxEvent::Attack => "Attack1",
            MonSfxEvent::Hit => "HitSound",
            MonSfxEvent::Death => "DeathSound",
            MonSfxEvent::Taunt => "Taunt",
        }
    }

    /// Clé `index.json` (extraction, minuscules) du même événement.
    pub fn key(self) -> &'static str {
        match self {
            MonSfxEvent::Attack => "attack",
            MonSfxEvent::Hit => "hit",
            MonSfxEvent::Death => "death",
            MonSfxEvent::Taunt => "taunt",
        }
    }
}

/// Un record `monsounds.txt` : famille (col `Id`) et ses 4 événements
/// (cellule brute, `None` si vide — fidèle : certains monstres n'ont pas de
/// voix d'attaque, ex. `skeleton`). Champs POSSÉDÉS : le TSV est parse dans
/// l'initialisation de la table statique puis libéré.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonSoundSet {
    /// Famille sonore (col `Id`, ex. `fallen`, `zombie`, `andariel`).
    pub family: String,
    /// Voix d'attaque (col `Attack1`).
    pub attack1: Option<String>,
    /// Son d'encaissement (col `HitSound`).
    pub hit: Option<String>,
    /// Son de mort (col `DeathSound`).
    pub death: Option<String>,
    /// Annonce (col `Taunt`).
    pub taunt: Option<String>,
}

/// Table parsée UNE FOIS, indexée par famille.
fn table() -> &'static HashMap<&'static str, MonSoundSet> {
    static TABLE: OnceLock<HashMap<&'static str, MonSoundSet>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let t = Tsv::parse(MONSOUNDS_TXT).expect("monsounds.txt embarqué doit parser");
        let mut map = HashMap::new();
        for i in 0..t.len() {
            let Some(r) = t.rec(i) else { continue };
            let fam = r.s("Id").to_string();
            if fam.is_empty() {
                continue;
            }
            let cell = |col: &str| -> Option<String> {
                let c = r.s(col);
                (!c.is_empty()).then(|| c.to_string())
            };
            let key: &'static str = Box::leak(fam.clone().into_boxed_str());
            map.insert(
                key,
                MonSoundSet {
                    family: fam,
                    attack1: cell("Attack1"),
                    hit: cell("HitSound"),
                    death: cell("DeathSound"),
                    taunt: cell("Taunt"),
                },
            );
        }
        map
    })
}

/// Toutes les familles de la table (triées, pour tests/rapports).
pub fn families() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = table().keys().copied().collect();
    v.sort_unstable();
    v
}

/// Nombre de records réellement indexés (141 attendu).
pub fn family_count() -> usize {
    table().len()
}

/// Résout un JETON monstats (`zombie2`, `ancientbarb1`, `skmage_pois3`…) en
/// famille sonore de la table : chiffres finaux retirés, alias [`ALIAS`],
/// remontée des `_`. `None` si aucune ligne ne porte la famille.
pub fn family_of(token: &str) -> Option<&'static str> {
    let mut stem = token.trim().to_ascii_lowercase();
    // jetons monstats = tige + niveau (`zombie2`, `skmage_pois3`) : retire les
    // chiffres finaux avant la résolution.
    let keep = stem.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    stem.truncate(keep);
    loop {
        if let Some(a) = ALIAS.iter().find(|a| a.0 == stem) {
            return table().contains_key(a.1).then_some(a.1);
        }
        if let Some((key, _)) = table().get_key_value(stem.as_str()) {
            return Some(key);
        }
        match stem.rfind('_') {
            Some(pos) => stem.truncate(pos),
            None => return None,
        }
    }
}

/// Le jeu de sons TABLE d'un jeton monstats ([`family_of`] + lookup).
pub fn sounds_of(token: &str) -> Option<&'static MonSoundSet> {
    let fam = family_of(token)?;
    table().get(fam)
}

/// Nom de son TABLE (cellule `monsounds.txt`) d'un événement pour un jeton —
/// ex. `("zombie2", Hit)` → `zombie_hit_1`. `None` : famille inconnue ou
/// cellule vide (pas de son en retail).
pub fn event_file(token: &str, ev: MonSfxEvent) -> Option<&'static str> {
    sounds_of(token)?.event(ev)
}

impl MonSoundSet {
    /// Cellule d'un événement ([`MonSfxEvent`]).
    pub fn event(&self, ev: MonSfxEvent) -> Option<&str> {
        match ev {
            MonSfxEvent::Attack => self.attack1.as_deref(),
            MonSfxEvent::Hit => self.hit.as_deref(),
            MonSfxEvent::Death => self.death.as_deref(),
            MonSfxEvent::Taunt => self.taunt.as_deref(),
        }
    }
}

// --- couche FICHIERS LOCAUX (index.json) -------------------------------------

/// `index.json` parsé une fois ({families: {<famille>: {attack,hit,death,taunt}}}.
fn index() -> &'static Value {
    static IDX: OnceLock<Value> = OnceLock::new();
    IDX.get_or_init(|| serde_json::from_str(INDEX_JSON).expect("index.json embarqué doit parser"))
}

/// Fichier local réellement embarqué (`demo/assets/sfx/monsounds/<nom>`) pour
/// une famille + événement — ex. `("zombie", Hit)` → `zombie_gethit1.wav`.
/// `None` = non extrait (release taillée / cellule vide) → le lecteur JS
/// retombe sur son générique sans erreur.
pub fn local_file(family: &str, ev: MonSfxEvent) -> Option<&'static str> {
    index()["families"]
        .get(family)?
        .get(ev.key())
        .and_then(Value::as_str)
}

/// Nombre d'entrées fichier de l'index (124 attendu au 2026-10-24).
pub fn local_entry_count() -> usize {
    index()["families"]
        .as_object()
        .map(|o| {
            o.values()
                .filter_map(Value::as_object)
                .map(|f| f.values().filter(|v| v.is_string()).count())
                .sum()
        })
        .unwrap_or(0)
}

/// Familles de l'index ayant AU MOINS un fichier réel extrait.
pub fn families_with_files() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = index()["families"]
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(_, f)| f.as_object().map(|f| f.values().any(Value::is_string)).unwrap_or(false))
                .map(|(k, _)| k.as_str())
                .collect()
        })
        .unwrap_or_default();
    v.sort_unstable();
    v
}

// --- codes sprites du port → famille -----------------------------------------

/// Codes sprites (`patch_monstats.txt` col `Code`, `patch_levels.txt`/spawn)
/// que le port peut réellement afficher : 6 monstres du manifest démo, planches
/// extérieures (goatman/foulcrow), tokens de super-uniques et boss scriptés/
/// vagues (`quests_finale::FINALE_RECORDS`, `quests_actes2::BOSSES_ACTS23`).
/// Consommé par le lecteur JS (pan/volume par code) et vérifié par test.
pub const PORT_CODES: [(&str, &str); 23] = [
    // manifest démo (bloodmoor : FA/ZM/SI)
    ("FA", "fallen"),
    ("ZM", "zombie"),
    ("SI", "quillrat"),
    ("SK", "skeleton"),
    ("FS", "fallenshaman"),
    ("YE", "brute"),
    // super-uniques / planches A1-A3 (patch_monstats)
    ("CR", "corruptrogue"),
    ("GM", "goatman"),
    ("RD", "radament"),
    ("SU", "summoner"),
    ("GZ", "zombieunique"),
    ("AN", "andariel"),
    ("DU", "duriel"),
    ("MP", "mephisto"),
    // boss scriptés + vagues du trône (quests_finale FINALE_RECORDS)
    ("DI", "diablo"),
    ("22", "izual"),
    ("5P", "smithdemon"),
    ("0D", "ancient"),
    ("0E", "ancient"),
    ("0F", "ancient"),
    ("GY", "unraveler"),
    ("42", "baalcrab"),
    ("BK", "foulcrow"),
];

/// Famille sonore d'un CODE sprite du port ([`PORT_CODES`]).
pub fn code_family(code: &str) -> Option<&'static str> {
    PORT_CODES
        .iter()
        .find(|(c, _)| c.eq_ignore_ascii_case(code))
        .map(|(_, f)| *f)
}

// --- tests -------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_counts_140_real_families() {
        // 141 lignes de données dont 1 séparateur à Id vide (ligne 2, format
        // monsounds v1.12) -> 140 familles indexées.
        assert_eq!(family_count(), 140, "monsounds.txt = 141 rec - 1 séparateur");
    }

    #[test]
    fn golden_fallen_zombie_quillrat_tokens() {
        // Jetons réellement spawnés par le démo (bloodmoor : FA/ZM/SI) —
        // valeurs lues à la main dans assets/monsounds.txt (2026-10-24).
        assert_eq!(family_of("fallen1"), Some("fallen"));
        let f = sounds_of("fallen1").unwrap();
        assert_eq!(f.attack1.as_deref(), Some("fallen_attack_1"));
        assert_eq!(f.hit.as_deref(), Some("fallen_hit_1"));
        assert_eq!(f.death.as_deref(), Some("fallen_death_1"));
        assert_eq!(f.taunt, None, "les fallen n'ont pas d'annonce");
        assert_eq!(event_file("fallen2", MonSfxEvent::Hit), Some("fallen_hit_1"));

        assert_eq!(family_of("zombie2"), Some("zombie"));
        let z = sounds_of("zombie1").unwrap();
        assert_eq!(z.attack1.as_deref(), Some("zombie_attack_1"));
        assert_eq!(event_file("zombie5", MonSfxEvent::Death), Some("zombie_death_1"));

        // quillrat = spikefiend dans la table v1.12
        assert_eq!(family_of("quillrat1"), Some("quillrat"));
        assert_eq!(event_file("quillrat4", MonSfxEvent::Attack), Some("spikefiend_attack_1"));
    }

    #[test]
    fn golden_bosses_and_taunts() {
        assert_eq!(event_file("andariel", MonSfxEvent::Taunt), Some("monster_andariel_taunt_1"));
        assert_eq!(event_file("andariel", MonSfxEvent::Death), Some("andariel_death"));
        assert_eq!(event_file("duriel", MonSfxEvent::Attack), Some("duriel_attack_1"));
        assert_eq!(event_file("baalcrab", MonSfxEvent::Death), Some("baal_death"));
        assert_eq!(event_file("diablo", MonSfxEvent::Hit), Some("diablo_hit_1"));
        assert_eq!(event_file("mephisto", MonSfxEvent::Taunt), Some("monster_mephisto_taunt_1"));
        assert_eq!(event_file("izual", MonSfxEvent::Taunt), Some("monster_izual_taunt_1"));
        assert_eq!(event_file("bloodraven", MonSfxEvent::Taunt), Some("monster_bloodraven_taunt_1"));
        assert_eq!(event_file("countess", MonSfxEvent::Taunt), Some("monster_countess_taunt_1"));
        assert_eq!(event_file("radament", MonSfxEvent::Attack), Some("greatermummy_attack_1"));
    }

    #[test]
    fn aliases_match_documented_decisions() {
        assert_eq!(family_of("ancientbarb1"), Some("ancient"));
        assert_eq!(event_file("ancientbarb2", MonSfxEvent::Hit), Some("barbarian_hit_1"));
        assert_eq!(family_of("griswold"), Some("zombieunique"));
        assert_eq!(event_file("griswold", MonSfxEvent::Attack), Some("zombie_attack_1"));
        assert_eq!(family_of("nihlathakboss"), Some("nihlathak"));
        assert_eq!(event_file("nihlathakboss", MonSfxEvent::Taunt), Some("monster_nihlathak_taunt_1"));
        assert_eq!(family_of("hephasto"), Some("smithdemon"));
        assert_eq!(event_file("hephasto", MonSfxEvent::Attack), Some("smith_attack_1"));
        assert_eq!(family_of("skmage_pois3"), Some("sk_mage"));
        assert_eq!(event_file("skmage_pois3", MonSfxEvent::Hit), Some("skeleton_hit_1"));
    }

    #[test]
    fn unknown_families_are_none() {
        // Lignes absentes de la table v1.12 embarquée → pas de son, repli
        // générique côté JS (jamais d'invention).
        assert_eq!(family_of("venomlord"), None);
        assert_eq!(family_of("baalhighpriest"), None);
        assert_eq!(family_of("snowyeti4"), None);
        assert_eq!(family_of("succubuswitch2"), None);
        assert_eq!(family_of("absent_de_table"), None);
        // cellule vide = pas de son (squelettes sans voix d'attaque)
        assert_eq!(event_file("skeleton1", MonSfxEvent::Attack), None);
        assert_eq!(event_file("skeleton1", MonSfxEvent::Hit), Some("skeleton_hit_1"));
    }

    #[test]
    fn all_port_spawned_tokens_resolve() {
        // Chaque classe de super-unique + boss/vague du port doit se résoudre
        // (famille connue ou None assumé) sans paniquer — couverture large.
        let t = crate::tsv::Tsv::parse(include_str!("../../../../assets/superuniques.txt")).unwrap();
        let mut resolved = 0;
        for i in 0..t.len() {
            let Some(r) = t.rec(i) else { continue };
            let cls = r.s("Class");
            if cls.is_empty() {
                continue;
            }
            if family_of(cls).is_some() {
                resolved += 1;
            }
        }
        assert!(resolved >= 55, "{resolved} classes de super-uniques résolues (≥55 attendu)");
    }

    #[test]
    fn port_codes_cover_manifest_and_finale() {
        // 6 codes du manifest démo…
        for c in ["FA", "ZM", "SI", "SK", "FS", "YE"] {
            assert!(code_family(c).is_some(), "code {c} mappé");
        }
        // …et codes des records FINALE (quests_finale.rs) + andariel.
        assert_eq!(code_family("22"), Some("izual"));
        assert_eq!(code_family("5P"), Some("smithdemon"));
        assert_eq!(code_family("DI"), Some("diablo"));
        assert_eq!(code_family("0D"), Some("ancient"));
        assert_eq!(code_family("0F"), Some("ancient"));
        assert_eq!(code_family("0E"), Some("ancient"));
        assert_eq!(code_family("GY"), Some("unraveler"));
        assert_eq!(code_family("42"), Some("baalcrab"));
        assert_eq!(code_family("AN"), Some("andariel"));
        assert_eq!(code_family("DU"), Some("duriel"));
        assert_eq!(code_family("MP"), Some("mephisto"));
        assert_eq!(code_family("GM"), Some("goatman"));
        assert_eq!(code_family("BK"), Some("foulcrow"));
        assert_eq!(code_family("inconnu"), None);
    }

    #[test]
    fn local_index_counts_and_goldens() {
        // 56 familles indexées, 124 entrées fichier réelles (2026-10-24).
        assert_eq!(index()["families"].as_object().unwrap().len(), 56);
        assert_eq!(local_entry_count(), 124);
        assert_eq!(families_with_files().len(), 43);
        // goldens : ce qui est réellement embarqué
        assert_eq!(local_file("zombie", MonSfxEvent::Attack), Some("zombie_attack1.wav"));
        assert_eq!(local_file("zombie", MonSfxEvent::Hit), Some("zombie_gethit1.wav"));
        assert_eq!(local_file("zombie", MonSfxEvent::Death), Some("zombie_death1.wav"));
        assert_eq!(local_file("fallen", MonSfxEvent::Death), Some("fallen_death1.wav"));
        // attaque fallen taillée de la release → repli JS, pas de fichier inventé
        assert_eq!(local_file("fallen", MonSfxEvent::Attack), None);
        assert_eq!(local_file("andariel", MonSfxEvent::Taunt), Some("andariel_taunt1.wav"));
        assert_eq!(local_file("duriel", MonSfxEvent::Taunt), Some("duriel_taunt1.wav"));
        assert_eq!(local_file("mephisto", MonSfxEvent::Taunt), Some("mephisto_taunt1.wav"));
        // planches A1 : goatman/foulcrow réels, attaque cr_archer = arme (hors table)
        assert_eq!(local_file("goatman", MonSfxEvent::Attack), Some("goatman_attack1.wav"));
        assert_eq!(local_file("foulcrow", MonSfxEvent::Death), Some("hawk_death1.wav"));
        assert_eq!(local_file("cr_archer", MonSfxEvent::Attack), None);
        // famille sans fichier du tout
        assert_eq!(local_file("imp", MonSfxEvent::Attack), None);
    }

    #[test]
    fn indexed_wav_files_exist_on_disk_with_riff_header() {
        // Chaque fichier de l'index doit exister et être un WAV RIFF réel.
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../demo/assets/sfx/monsounds");
        let mut checked = 0;
        let mut unique = std::collections::HashSet::new();
        for fam in families_with_files() {
            for ev in [MonSfxEvent::Attack, MonSfxEvent::Hit, MonSfxEvent::Death, MonSfxEvent::Taunt] {
                if let Some(f) = local_file(fam, ev) {
                    let bytes = std::fs::read(format!("{dir}/{f}"))
                        .unwrap_or_else(|e| panic!("{f}: {e}"));
                    assert_eq!(&bytes[..4], b"RIFF", "{f} pas un RIFF");
                    assert_eq!(&bytes[8..12], b"WAVE", "{f} pas un WAVE");
                    checked += 1;
                    unique.insert(f);
                }
            }
        }
        // 124 entrées (famille×événement), 110 fichiers UNIQUES (familles
        // aliasées partageant le même extrait : smith/smithdemon,
        // zombie/zombieunique, unraveler/radament -> greatermummy…).
        assert_eq!(checked, 124, "entrées indexées avec fichier");
        assert_eq!(unique.len(), 110, "fichiers wav uniques extraits (2026-10-24)");
    }

    #[test]
    fn mon_sfx_js_parity_with_index_and_codes() {
        // Le lecteur JS embarque le MÊME mapping : chaque fichier réel indexé
        // apparaît dans demo/mon-sfx.js, et les codes du port sont mappés.
        for fam in families_with_files() {
            for ev in [MonSfxEvent::Attack, MonSfxEvent::Hit, MonSfxEvent::Death, MonSfxEvent::Taunt] {
                if let Some(f) = local_file(fam, ev) {
                    assert!(MON_SFX_JS.contains(f), "mon-sfx.js ne référence pas {f} ({fam}/{})", ev.column());
                }
            }
        }
        for (code, fam) in PORT_CODES {
            assert!(
                MON_SFX_JS.contains(&format!("\"{code}\"")) || MON_SFX_JS.contains(&format!("'{code}'")),
                "mon-sfx.js ne mappe pas le code {code}"
            );
            assert!(MON_SFX_JS.contains(fam), "mon-sfx.js ne connaît pas la famille {fam}");
        }
    }
}
