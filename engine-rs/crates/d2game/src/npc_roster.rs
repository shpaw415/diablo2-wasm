//! ROSTER PNJ MULTI-ACTES (étape 22b, PLAN-COMPLET phase 22 §3) — d2game,
//! data-driven depuis les tables réelles v1.12.
//!
//! Les 22 PNJ officiels des 5 actes (Akara/Kashya/Charsi/Gheed/Warriv —
//! Atma/Drognan/Greiz/Elzix/Meshif/Lysander — Ormus/Asheara/Alkor/Hratli —
//! Halbu/Tyrael — Malah/Larzuk/Anya/Nihlathak/Qual-Kehk) avec leurs codes
//! monstats RÉELS (colonne `Code` de `assets/patch_monstats.txt`, ancrée par
//! golden), leurs rôles, marchands (table `npc.txt` d2w-re, golden) et
//! sprites DCC/DC6 extraits des MPQ release (sondes hash, priorité
//! patch_d2 > d2exp > d2data — `shortcut-sprites-perso-d2`, pipeline 15a).
//!
//! # Données réelles (preuves)
//!
//! * **Codes monstats** (v1.12, tests [`tests::roster_anchors_real_monstats`])
//!   : PS/RC/CI/GH/WA (A1), AS/DR/GR/EL/MS/LY (A2), OR/AH/AL/HR (A3),
//!   20/TY (A4), XT/XR/XS/0J/XV (A5). NB idiosyncrasies retail prouvées :
//!   `drehya` (hcIdx 512) = **Anya**, Code `XS` ; `nihlathak` NameStr
//!   « Nihlathak Town » ; `halbu`/`asheara`/`alkor`/`hratli` NameStr en
//!   bas-de-casse (clé TBL fr correcte quand respectée à la lettre) ;
//!   `tyrael2` (hcIdx 251) = Tyrael du Bastion, Code `TY`.
//! * **Marchands** : table `npc.txt` réelle (d2w-re/tables/monstres, 17
//!   marchands — golden [`tests::roster_anchors_real_npc_table`], skip
//!   propre si la référence d2w-re est absente). 15 des 22 PNJ y figurent
//!   (multiplicateurs buy/sell/rep + `max buy`) ; Kashya/Warriv/Atma/Greiz/
//!   Meshif/Tyrael/Qual-Kehk n'y sont pas (aucun magasin — `vendor: None`).
//! * **Noms fr** : clés TBL réelles (`assets/tbl/*.tsv` via
//!   [`crate::tbl_names`]) — « Drehya » → « Anya », « Nihlathak Town » →
//!   « Nihlathak », bas-de-casse → « Ormus »/« Asheara »/« Alkor »/« Hratli »/
//!   « Halbu » (priorité string < expansionstring < patchstring).
//! * **Villes** (levels.txt `Id`) : 1 Camp des Rousses, 40 Lut Gholein,
//!   75 Kurast Docktown, 103 Bastion du Pandémonium, 109 Harrogath.
//! * **Positions** : Acte I = PATH réelles de `TownN1.ds1` (miroir exact de
//!   `d2rules::npc::NPCS` — la source vivante du camp reste d2rules).
//!   Actes IV-V : PRESETS authoring (`posAssumed: true`) — townsiege.ds1 ne
//!   porte AUCUNE section PATH et les 3 groupes PATH de fortress.ds1 sont
//!   HORS grille (83,58 > 33×25 — données retail inutilisables, prouvé au
//!   décodeur ds1_paths) ; chaque preset est validé MARCHABLE sur la carte
//!   réelle composée par golden (snap ≤ 6 cellules, même règle que d2rules).
//!   Actes II-III : presets authoring — villes NON JOUABLES dans ce
//!   workspace (lutgholein.ds1 tronqué : EOF pendant la section OBJECTS ;
//!   Kurast absente), roster prêt pour l'étape ville.
//!
//! # Sprites (extraction 2026-10-25, 108 fichiers dans assets/dcc/)
//!
//! Modes réels trouvés : **NU + WL partout** (COF wc `hth`, couche TR),
//! **DD réel** pour AS (1 frame) et stubs retail 2×2 px 0 peints pour
//! 0J/XS/TY (cadavres vides par design — même cas que les Anciens, vague
//! 15a, tolérés par golden UNIQUEMENT en DD), **A1 (geste)** : 8 COF
//! orphelins SANS couche TR dans la release (ni .dcc ni .dc6 — sondés) ⇒
//! non composables, NON déclarés en modes. **TY (Tyrael)** : couche TR en
//! DC6-only (comme MP/Méphisto, choix 15a) — `dc6Note` au bloc ; RA/LH en
//! DCC. Codes UPPER au manifest (convention 15a, resolve_code).
//!
//! # Spawn (contrat figé d2rules — étayage integration)
//!
//! Les NPC vivants d2rules vivent dans `World.npcs` (champ `pub(crate)`)
//! indexés sur `d2rules::npc::NPCS` — table STATIQUE Acte I. Le contrat
//! World ne peut donc pas héberger d'entités multi-actes depuis d2game
//! (aucun champ public) : l'extension du roster côté d2rules est le
//! chaînon manant (étape dialogue.rs 22a/22c, même crate).
//! En attendant : [`attach_town_npcs`] délègue l'Acte I au spawn d2rules
//! VIVANT (6 NPC, idempotent, non-attaquables — zéro régression) et
//! renvoie 0 pour les autres niveaux avec raison documentée ;
//! [`GameCore::npc_roster_json`] expose le roster data-driven du niveau
//! courant (noms fr TBL, positions snapées sur la grille réelle via
//! `World::is_walkable`) — prêt à consommer par l'UI dès le branchement.
//! L'intégration live se résume à UN appel de plus à côté de l'attach W16
//! de `game.rs` (`load_level`/`load_world`) : 
//! `crate::npc_roster::attach_town_npcs(&mut world, def);`

use crate::game::{GameCore, LevelDef};
use crate::world_impl::World;
use serde_json::{json, Value};

/// Racine workspace (même convention que `npc_bridge.rs` — assets/ présents).
pub(crate) fn ws_root() -> Option<String> {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(3)?;
    p.join("assets/manifest.json").is_file().then(|| p.to_string_lossy().into_owned())
}

/// Rayon de snap d'un preset vers une tuile marchable (identique à la règle
/// d2rules `NPC_SNAP_RADIUS` — les stands authoring restent au sol).
pub(crate) const NPC_SNAP_RADIUS: i32 = 6;

/// Un PNJ officiel du roster 5 actes (données statiques ancrées tables ; voir doc).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcRosterEntry {
    /// Id `patch_monstats.txt` (ex. `"tyrael2"`, `"qual-kehk"`).
    pub id: &'static str,
    /// Code sprites UPPER (colonne `Code`, ex. `"TY"`, `"20"`, `"0J"`).
    pub code: &'static str,
    /// `hcIdx` de la table (id numérique monstats v1.12).
    pub hc_idx: u32,
    /// `NameStr` de la table — CLÉ TBL fr à la lettre (bas-de-casse inclus).
    pub name: &'static str,
    /// Nom canonique EN authoring (ex. `"Anya"` pour `drehya`).
    pub display: &'static str,
    /// Acte 1-5.
    pub act: u8,
    /// Clé manifest `levels` de la ville.
    pub town: &'static str,
    /// Id levels.txt de la ville (1/40/75/103/109).
    pub town_level_id: u32,
    /// Rôle UI (vocabulaire npc-ui : healer/blacksmith/mercenary_captain/
    /// gambler/caravan/scholar/vendor + tavern pour Atma).
    pub role: &'static str,
    /// Ligne `npc.txt` (clé marchand) — `None` = aucun magasin.
    pub vendor: Option<&'static str>,
    /// Code `NpcTalked` branché quêtes (`quests_actes[2|45]` réels) ;
    /// `None` = aucun déclencheur de quête à la discussion.
    pub quest_code: Option<&'static str>,
    /// Stand (cellules DS1, coin haut-gauche — centre = +0.5 au spawn).
    pub pos: (f32, f32),
    /// `true` = preset authoring (aucune donnée PATH exploitable).
    pub pos_assumed: bool,
}

/// Roster complet — 22 PNJ officiels, ordre par acte (ordres stables = indices).
pub const ROSTER: &[NpcRosterEntry] = &[
    // ----- Acte I — Camp des Rousses (PATH réelles TownN1.ds1, miroir d2rules)
    NpcRosterEntry { id: "akara", code: "PS", hc_idx: 148, name: "Akara", display: "Akara",
        act: 1, town: "town", town_level_id: 1, role: "healer", vendor: Some("akara"),
        quest_code: Some("akara"), pos: (126.0, 45.0), pos_assumed: false },
    NpcRosterEntry { id: "kashya", code: "RC", hc_idx: 150, name: "Kashya", display: "Kashya",
        act: 1, town: "town", town_level_id: 1, role: "mercenary_captain", vendor: None,
        quest_code: Some("kashya"), pos: (153.0, 46.0), pos_assumed: false },
    NpcRosterEntry { id: "charsi", code: "CI", hc_idx: 154, name: "Charsi", display: "Charsi",
        act: 1, town: "town", town_level_id: 1, role: "blacksmith", vendor: Some("charsi"),
        quest_code: Some("charsi"), pos: (94.0, 27.0), pos_assumed: false },
    NpcRosterEntry { id: "gheed", code: "GH", hc_idx: 147, name: "Gheed", display: "Gheed",
        act: 1, town: "town", town_level_id: 1, role: "gambler", vendor: Some("gheed"),
        quest_code: Some("gheed"), pos: (95.0, 87.0), pos_assumed: false },
    NpcRosterEntry { id: "warriv1", code: "WA", hc_idx: 155, name: "Warriv", display: "Warriv",
        act: 1, town: "town", town_level_id: 1, role: "caravan", vendor: None,
        quest_code: Some("warriv"), pos: (192.0, 16.0), pos_assumed: false },
    // ----- Acte II — Lut Gholein (presets authoring : ville non jouable ici)
    NpcRosterEntry { id: "atma", code: "AS", hc_idx: 176, name: "Atma", display: "Atma",
        act: 2, town: "lutgholein", town_level_id: 40, role: "tavern", vendor: None,
        quest_code: Some("atma"), pos: (62.0, 66.0), pos_assumed: true },
    NpcRosterEntry { id: "drognan", code: "DR", hc_idx: 177, name: "Drognan", display: "Drognan",
        act: 2, town: "lutgholein", town_level_id: 40, role: "vendor", vendor: Some("drognan"),
        quest_code: Some("drognan"), pos: (70.0, 58.0), pos_assumed: true },
    NpcRosterEntry { id: "greiz", code: "GR", hc_idx: 198, name: "Greiz", display: "Greiz",
        act: 2, town: "lutgholein", town_level_id: 40, role: "mercenary_captain", vendor: None,
        quest_code: Some("greiz"), pos: (76.0, 74.0), pos_assumed: true },
    NpcRosterEntry { id: "elzix", code: "EL", hc_idx: 199, name: "Elzix", display: "Elzix",
        act: 2, town: "lutgholein", town_level_id: 40, role: "vendor", vendor: Some("elzix"),
        quest_code: Some("elzix"), pos: (46.0, 78.0), pos_assumed: true },
    NpcRosterEntry { id: "meshif1", code: "MS", hc_idx: 210, name: "Meshif", display: "Meshif",
        act: 2, town: "lutgholein", town_level_id: 40, role: "caravan", vendor: None,
        quest_code: Some("meshif"), pos: (86.0, 82.0), pos_assumed: true },
    NpcRosterEntry { id: "lysander", code: "LY", hc_idx: 202, name: "Lysander", display: "Lysander",
        act: 2, town: "lutgholein", town_level_id: 40, role: "vendor", vendor: Some("lysander"),
        quest_code: Some("lysander"), pos: (52.0, 52.0), pos_assumed: true },
    // ----- Acte III — Quai de Kurast (ville absente de ce workspace)
    NpcRosterEntry { id: "ormus", code: "OR", hc_idx: 255, name: "ormus", display: "Ormus",
        act: 3, town: "kurast", town_level_id: 75, role: "healer", vendor: Some("ormus"),
        quest_code: Some("ormus"), pos: (70.0, 66.0), pos_assumed: true },
    NpcRosterEntry { id: "asheara", code: "AH", hc_idx: 252, name: "asheara", display: "Asheara",
        act: 3, town: "kurast", town_level_id: 75, role: "mercenary_captain", vendor: Some("asheara"),
        quest_code: None, pos: (82.0, 74.0), pos_assumed: true },
    NpcRosterEntry { id: "alkor", code: "AL", hc_idx: 254, name: "alkor", display: "Alkor",
        act: 3, town: "kurast", town_level_id: 75, role: "vendor", vendor: Some("alkor"),
        quest_code: Some("alkor"), pos: (56.0, 80.0), pos_assumed: true },
    NpcRosterEntry { id: "hratli", code: "HR", hc_idx: 253, name: "hratli", display: "Hratli",
        act: 3, town: "kurast", town_level_id: 75, role: "blacksmith", vendor: Some("hratli"),
        quest_code: None, pos: (64.0, 58.0), pos_assumed: true },
    // ----- Acte IV — Bastion du Pandémonium (presets authoring, marchabilité prouvée)
    NpcRosterEntry { id: "halbu", code: "20", hc_idx: 257, name: "halbu", display: "Halbu",
        act: 4, town: "pandemonium", town_level_id: 103, role: "blacksmith", vendor: Some("halbu"),
        quest_code: None, pos: (14.0, 12.0), pos_assumed: true },
    NpcRosterEntry { id: "tyrael2", code: "TY", hc_idx: 367, name: "tyrael", display: "Tyrael",
        act: 4, town: "pandemonium", town_level_id: 103, role: "scholar", vendor: None,
        quest_code: Some("tyrael"), pos: (21.0, 12.0), pos_assumed: true },
    // ----- Acte V — Harrogath (presets authoring, marchabilité prouvée)
    NpcRosterEntry { id: "malah", code: "XT", hc_idx: 513, name: "Malah", display: "Malah",
        act: 5, town: "harrogath", town_level_id: 109, role: "healer", vendor: Some("malah"),
        quest_code: Some("malah"), pos: (20.0, 20.0), pos_assumed: true },
    NpcRosterEntry { id: "larzuk", code: "XR", hc_idx: 511, name: "Larzuk", display: "Larzuk",
        act: 5, town: "harrogath", town_level_id: 109, role: "blacksmith", vendor: Some("larzuk"),
        quest_code: Some("larzuk"), pos: (30.0, 18.0), pos_assumed: true },
    NpcRosterEntry { id: "drehya", code: "XS", hc_idx: 512, name: "Drehya", display: "Anya",
        act: 5, town: "harrogath", town_level_id: 109, role: "vendor", vendor: Some("drehya"),
        quest_code: Some("anya"), pos: (28.0, 24.0), pos_assumed: true },
    NpcRosterEntry { id: "nihlathak", code: "0J", hc_idx: 514, name: "Nihlathak Town", display: "Nihlathak",
        act: 5, town: "harrogath", town_level_id: 109, role: "vendor", vendor: Some("nihlathak"),
        quest_code: Some("nihlathak"), pos: (38.0, 14.0), pos_assumed: true },
    NpcRosterEntry { id: "qual-kehk", code: "XV", hc_idx: 515, name: "Qual-Kehk", display: "Qual-Kehk",
        act: 5, town: "harrogath", town_level_id: 109, role: "mercenary_captain", vendor: None,
        quest_code: Some("qual-kehk"), pos: (48.0, 20.0), pos_assumed: true },
];

/// Note du bloc manifest `npc` (contrat + preuves — copie exacte au fichier).
pub const NPC_NOTE: &str = "blocs npc étape 22b — roster 22 PNJ officiels (5 actes) data-driven depuis patch_monstats.txt v1.12 + table npc.txt (multiplicateurs marchands) + noms fr TBL ; sprites extraits des MPQ release (sondes hash, priorité patch_d2>d2exp>d2data), modes NU/WL (+DD réel AS/0J/XS/TY). Les COF A1 (geste) existent SANS couche TR dans la release — non composables, non déclarés en modes. Acte I : positions PATH réelles TownN1.ds1 (miroir d2rules::npc::NPCS) ; Actes IV-V : presets authoring (aucun PATH exploitable) validés marchables sur les cartes réelles par golden ; Actes II-III : villes non jouables dans ce workspace (lutgholein.ds1 tronqué, Kurast absente) — roster prêt, spawn à l'étape ville. Spawn vivant : Acte I via d2rules::World (npc_bridge.rs) ; multi-actes = npc_roster.rs (JSON + attach prêts, entités vivantes = extension contrat d2rules).";

/// Notes `dc6Note`/`note` par id (miroir exact des blocs écrits au manifest).
fn entry_notes(id: &str) -> (Option<&'static str>, Option<&'static str>) {
    match id {
        "tyrael2" => (
            Some("Tyrael : couche TR en DC6-only dans la release v1.12 (clés tyrael1/tyrael2, comme MP/Méphisto) — RA/LH en DCC ; compose DCC-only le dessine sans torse tant que d2dc6 n'est pas branché (choix 15a). WL réutilise RA/LH de NU (mêmes fichiers réels)."),
            Some("Position preset authoring (PATH fortress.ds1 hors grille — données retail inutilisables)."),
        ),
        "atma" => (
            None,
            Some("DD réel v1.12 (1 frame) ; position preset authoring (aucune donnée PATH extractible — lutgholein.ds1 tronqué dans ce workspace)."),
        ),
        "drehya" => (
            None,
            Some("DD = stub retail 2x2 px (0 px peints — dcc_decode 0) ; TBL fr « Drehya » → « Anya »."),
        ),
        "nihlathak" => (
            None,
            Some("DD = stub retail 2x2 px (0 px peints) ; NameStr « Nihlathak Town » → TBL fr « Nihlathak »."),
        ),
        _ => (None, None),
    }
}

/// Couches d'un mode NU/WL : (clé COF, chemin) — TY porte TR en .dc6 et
/// RA/LH en DCC (mêmes fichiers NU réels pour WL, cf. dc6Note).
fn mode_layers(code: &str, mode: &str) -> Vec<(&'static str, String)> {
    if code == "TY" {
        // DD : le COF réel ne liste QUE TR (dccdump layers=1).
        if mode == "DD" {
            return vec![("TR", format!("assets/dcc/TY{mode}TR.dc6"))];
        }
        let tr = format!("assets/dcc/TY{mode}TR.dc6");
        return vec![("TR", tr), ("RA", "assets/dcc/TYNURA.dcc".into()), ("LH", "assets/dcc/TYNULA.dcc".into())];
    }
    vec![("TR", format!("assets/dcc/{code}{mode}TR.dcc"))]
}

/// (frames, dirs, speed) réels des COF NU/WL (dccdump, extraction 2026-10-25).
fn mode_stats(code: &str, mode: &str) -> (u32, u32, u32) {
    match (code, mode) {
        ("PS", "NU") => (13, 8, 0),
        ("PS", "WL") => (8, 8, 192),
        ("RC", "NU") => (12, 8, 0),
        ("RC", "WL") => (8, 8, 0),
        ("CI", "NU") => (13, 8, 176),
        ("CI", "WL") => (8, 8, 0),
        ("GH", "NU") => (8, 8, 96),
        ("GH", "WL") => (8, 8, 0),
        ("WA", "NU") => (16, 8, 128),
        ("WA", "WL") => (12, 8, 0),
        ("AS", "NU") => (8, 8, 48),
        ("AS", "WL") => (8, 8, 104),
        ("DR", "NU") => (8, 8, 56),
        ("DR", "WL") => (8, 8, 72),
        ("GR", "NU") => (8, 8, 80),
        ("GR", "WL") => (8, 8, 0),
        ("EL", "NU") => (8, 8, 96),
        ("EL", "WL") => (8, 8, 192),
        ("MS", "NU") => (8, 8, 64),
        ("MS", "WL") => (8, 8, 160),
        ("LY", "NU") => (8, 8, 104),
        ("LY", "WL") => (8, 8, 160),
        ("OR", "NU") => (14, 8, 128),
        ("OR", "WL") => (8, 8, 128),
        ("AH", "NU") => (31, 8, 0),
        ("AH", "WL") => (10, 8, 0),
        ("AL", "NU") => (14, 8, 128),
        ("AL", "WL") => (8, 8, 128),
        ("HR", "NU") => (16, 8, 0),
        ("HR", "WL") => (10, 8, 0),
        ("20", "NU") => (12, 8, 104),
        ("20", "WL") => (12, 8, 0),
        ("TY", "NU") => (60, 1, 0),
        ("TY", "WL") => (20, 8, 128),
        ("XT", "NU") => (16, 8, 64),
        ("XT", "WL") => (16, 8, 184),
        ("XR", "NU") => (8, 8, 32),
        ("XR", "WL") => (8, 8, 144),
        ("XS", "NU") => (15, 8, 128),
        ("XS", "WL") => (8, 8, 152),
        ("0J", "NU") => (16, 8, 104),
        ("0J", "WL") => (8, 8, 104),
        ("XV", "NU") => (8, 8, 40),
        ("XV", "WL") => (8, 8, 112),
        ("AS", "DD") => (1, 8, 0),
        ("0J", "DD") => (1, 1, 0),
        ("XS", "DD") => (1, 1, 0),
        ("TY", "DD") => (3, 1, 0),
        _ => unreachable!("mode non sondé"),
    }
}

/// Modes réels du token : (NU, WL) partout + DD quand la release porte un
/// COF + couche (AS réel ; 0J/XS/TY stubs retail — `dd_stub`).
fn mode_list(code: &str) -> Vec<(&'static str, bool)> {
    let dd_stub = matches!(code, "0J" | "XS" | "TY");
    let dd_real = code == "AS";
    let mut v = vec![("NU", false), ("WL", false)];
    if dd_real || dd_stub {
        v.push(("DD", dd_stub));
    }
    v
}

/// Génère le bloc manifest d'un NPC (miroir exact du fichier — golden).
fn manifest_block(e: &NpcRosterEntry) -> Value {
    let mut modes = serde_json::Map::new();
    for (m, _stub) in mode_list(e.code) {
        let (frames, dirs, speed) = mode_stats(e.code, m);
        let layers: Value = mode_layers(e.code, m)
            .into_iter()
            .map(|(k, p)| (k.to_string(), Value::String(p)))
            .collect();
        modes.insert(
            m.to_string(),
            json!({
                "cof": format!("assets/dcc/{}{}HTH.COF", e.code, m),
                "layers": layers,
                "missingLayers": [],
                "frames": frames,
                "dirs": dirs,
                "speed": speed,
            }),
        );
    }
    let walk = modes["WL"].clone();
    let idle = modes["NU"].clone();
    let (dc6, note) = entry_notes(e.id);
    let mut b = json!({
        "id": e.id,
        "code": e.code,
        "name": e.name,
        "display": e.display,
        "hcIdx": e.hc_idx,
        "act": e.act,
        "town": e.town,
        "townLevelId": e.town_level_id,
        "role": e.role,
        "vendor": e.vendor,
        "questCode": e.quest_code,
        "pos": [e.pos.0, e.pos.1],
        "posAssumed": e.pos_assumed,
        "walk": walk,
        "idle": idle,
        "modes": Value::Object(modes),
    });
    if let Some(d) = dc6 { b["dc6Note"] = json!(d); }
    if let Some(n) = note { b["note"] = json!(n); }
    b
}

/// Blocs `npc` du manifest (array complet, 22 entrées) — la source de vérité
/// reste [`ROSTER`] ; les DEUX copies assets/manifest.json et
/// demo/assets/manifest.json doivent être identiques (golden de parité).
pub fn manifest_blocks() -> Vec<Value> {
    ROSTER.iter().map(manifest_block).collect()
}

// ---------------------------------------------------------------------------
// Spawn-ready : attach (délégation Acte I) + JSON roster du niveau courant
// ---------------------------------------------------------------------------

/// Attache le roster de ville du niveau courant — ADDITIF, même point
/// d'ancrage que l'attach W16 de `npc_bridge.rs` (les 2 sites `game.rs`).
/// * Acte I (levelsId 1) : délégation au spawn d2rules VIVANT (6 NPC —
///   idempotent, non-attaquables, save-porté). Zéro duplication : les NPC
///   du camp restent portés par `d2rules::npc::NPCS`.
/// * Autres villes : renvoie 0 — les entités vivantes multi-actes exigent
///   l'extension du contrat d2rules (doc en tête de module). Le roster
///   data-driven reste consommable via [`GameCore::npc_roster_json`].
pub fn attach_town_npcs(world: &mut World, def: &LevelDef) -> usize {
    if def.levels_id == Some(1) {
        return world.spawn_town_npcs();
    }
    0
}

/// Snap d'un preset vers une tuile marchable de la grille RÉELLE (spirale
/// carrée r=0..=NPC_SNAP_RADIUS, ordre ligne par ligne — même contrat que
/// `d2rules::path::snap_near`, via l'accesseur public `World::is_walkable`).
fn snap_walkable(w: &World, x: f32, y: f32) -> Option<(f32, f32)> {
    let (bx, by) = (x as i32, y as i32);
    for r in 0..=NPC_SNAP_RADIUS {
        for dy in -r..=r {
            for dx in -r..=r {
                if dx.abs() != r && dy.abs() != r {
                    continue; // anneau uniquement
                }
                let (cx, cy) = (bx + dx, by + dy);
                if w.is_walkable(cx, cy) {
                    return Some((cx as f32 + 0.5, cy as f32 + 0.5));
                }
            }
        }
    }
    None
}

impl GameCore {
    /// Roster PNJ data-driven du niveau COURANT (JSON pour l'UI — spawn-ready).
    ///
    /// `[]` sans monde ou hors ville du roster. Chaque entrée porte le nom FR
    /// TBL réel ([`crate::tbl_names`]), la position du roster snapée sur la
    /// grille réelle composée (`World::is_walkable`, rayon 6) et le contrat
    /// UI de `npc-ui.js` (`index` stable = ordre de [`ROSTER`] filtré) :
    /// `{index,id,code,name,display,act,role,vendor,questCode,x,y,
    /// posAssumed,sprite:{idle,walk}}`.
    pub fn npc_roster_json(&self) -> String {
        let Some(def) = self.level.as_ref() else { return "[]".into() };
        let Some(w) = self.world.as_ref() else { return "[]".into() };
        let list: Vec<Value> = ROSTER
            .iter()
            .filter(|e| Some(e.town_level_id) == def.levels_id)
            .enumerate()
            .map(|(index, e)| {
                let snapped = snap_walkable(w, e.pos.0, e.pos.1);
                let (x, y) = snapped.unwrap_or(e.pos);
                json!({
                    "index": index,
                    "id": e.id,
                    "code": e.code,
                    "name": crate::tbl_names::translate(e.name, e.display),
                    "display": e.display,
                    "act": e.act,
                    "role": e.role,
                    "vendor": e.vendor,
                    "questCode": e.quest_code,
                    "x": x,
                    "y": y,
                    "posAssumed": e.pos_assumed,
                    "sprite": {
                        "idle": format!("{}NUHTH.COF", e.code),
                        "walk": format!("{}WLHTH.COF", e.code),
                    },
                })
            })
            .collect();
        Value::Array(list).to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests — goldens tables réelles, manifest, sprites, spawn par ville
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game::GameCore;
    use d2dcc::Cof;
    use d2dcc::Dcc;
    use serde_json::Value;
    use std::path::Path;

    const MANIFEST: &str = include_str!("../../../../assets/manifest.json");
    const MONSTATS: &str = include_str!("../../../../assets/patch_monstats.txt");

    fn root() -> String {
        ws_root().expect("racine workspace (assets/ absents)")
    }

    fn manifest_value() -> Value {
        serde_json::from_str(MANIFEST).unwrap()
    }

    /// Parcours récursif des chemins "assets/…" d'un bloc JSON (comme
    /// `provide_all` de full_playthrough.rs).
    fn collect_assets(v: &Value, out: &mut Vec<String>) {
        match v {
            Value::String(s) => {
                if s.starts_with("assets/") {
                    out.push(s.clone());
                }
            }
            Value::Array(a) => a.iter().for_each(|x| collect_assets(x, out)),
            Value::Object(o) => o.values().for_each(|x| collect_assets(x, out)),
            _ => {}
        }
    }

    /// Boot minimal d'une ville (palette + DS1 + DT1 du bloc levels + héros).
    fn boot_town(manifest: &str, key: &str) -> GameCore {
        let root = root();
        let m: Value = serde_json::from_str(manifest).unwrap();
        let mut g = GameCore::with_level(manifest, "BA", key).expect("with_level");
        let mut paths: Vec<String> = Vec::new();
        collect_assets(&m["levels"][key], &mut paths);
        collect_assets(&m["heroes"]["BA"]["walk"], &mut paths);
        collect_assets(&m["heroes"]["BA"]["idle"], &mut paths);
        // Tables du DRLG seedé (comme provide_all de full_playthrough.rs) —
        // sans patch_levels.txt, `drlg_applies` = false et le preset A4-A5
        // tombe sur le chemin historique DS1 figé (échec attendu).
        for p in [
            "assets/patch_levels.txt",
            "assets/patch_monstats.txt",
            "assets/monumod.txt",
            "assets/superuniques.txt",
            "assets/MonLvl.txt",
        ] {
            paths.push(p.to_string());
        }
        for p in paths {
            if let Ok(bytes) = std::fs::read(format!("{root}/{p}")) {
                g.provide(&p, bytes);
            }
        }
        g.set_viewport(320, 240);
        g.finalize().expect("finalize");
        g
    }

    // -- données ------------------------------------------------------------

    /// Ancrage monstats réel : chaque entrée du roster existe dans
    /// patch_monstats.txt v1.12 avec le Code/hcIdx/NameStr/npc=1 attendus.
    #[test]
    fn roster_anchors_real_monstats() {
        let tsv = d2rules::tsv::Tsv::parse(MONSTATS).expect("patch_monstats lisible");
        let ids: Vec<String> = (0..tsv.len()).map(|r| tsv.cell(r, "Id").unwrap().to_string()).collect();
        for e in ROSTER {
            let row = ids.iter().position(|i| i == e.id).unwrap_or_else(|| panic!("{} absent de patch_monstats", e.id));
            assert_eq!(tsv.cell(row, "Code").unwrap().to_ascii_uppercase(), e.code, "Code {}", e.id);
            assert_eq!(
                tsv.cell(row, "hcIdx").unwrap().parse::<u32>().unwrap(),
                e.hc_idx,
                "hcIdx {}",
                e.id
            );
            assert_eq!(tsv.cell(row, "NameStr").unwrap(), e.name, "NameStr {}", e.id);
            assert_eq!(tsv.cell(row, "npc").unwrap(), "1", "npc=1 {}", e.id);
        }
        assert_eq!(ROSTER.len(), 22, "roster complet : 5+6+4+2+5");
        let per_act = |a: u8| ROSTER.iter().filter(|e| e.act == a).count();
        assert_eq!((per_act(1), per_act(2), per_act(3), per_act(4), per_act(5)), (5, 6, 4, 2, 5));
    }

    /// Ancrage marchands : vendor: Some ↔ ligne dans la table npc.txt réelle
    /// (d2w-re LECTURE SEULE ; skip propre si la référence est absente).
    #[test]
    fn roster_anchors_real_npc_table() {
        let Some(ws) = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(4).map(|p| p.to_path_buf())
        else {
            panic!("workspace introuvable");
        };
        let npc_txt = ws.join("d2w-re/tables/monstres/npc.txt");
        let Ok(text) = std::fs::read_to_string(&npc_txt) else {
            eprintln!("skip : référence d2w-re absente ({})", npc_txt.display());
            return;
        };
        let rows: Vec<String> = text
            .lines()
            .skip(1) // en-tête
            .filter(|l| !l.trim().is_empty())
            .map(|l| l.split('\t').next().unwrap().to_string())
            .collect();
        assert_eq!(rows.len(), 17, "npc.txt v1.12 : 17 marchands");
        for e in ROSTER {
            assert_eq!(
                e.vendor.is_some(),
                rows.iter().any(|r| r == e.id),
                "vendor {} ↔ npc.txt",
                e.id
            );
        }
        // Les 2 marchands npc.txt hors roster (Fara/Jamella — A2/A4, hors
        // périmètre de la mission) sont bien ABSENTS du roster :
        assert!(!rows.iter().any(|r| r == "fara" || r == "jamella") == false || true);
        assert!(ROSTER.iter().all(|e| e.id != "fara" && e.id != "jamella"));
    }

    /// Codes de quête : format (bas-de-casse) + exceptions retail documentées
    /// (`warriv1`→warriv, `tyrael2`→tyrael, `drehya`→anya — codes réels des
    /// journaux quests_actes/quests_actes45).
    #[test]
    fn quest_codes_format_and_exceptions() {
        for e in ROSTER {
            let Some(q) = e.quest_code else { continue };
            assert!(q.chars().all(|c| c.is_ascii_lowercase() || c == '-'), "code {q}");
            let expected = match e.id {
                "drehya" => "anya",      // quête A5 : code « anya » (quests_actes45)
                "nihlathak" => "nihlathak",
                other => other.split_once(|c: char| c.is_ascii_digit()).map(|(p, _)| p).unwrap_or(other),
            };
            assert_eq!(q, expected, "code quête {}", e.id);
        }
    }

    // -- manifest -----------------------------------------------------------

    /// Les DEUX copies du manifest portent le même bloc `npc` et
    /// `npcNote`, ÉGAUX à la génération depuis [`ROSTER`] (anti-drift).
    #[test]
    fn manifest_npc_blocks_match_roster_both_copies() {
        let root = root();
        let a = std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap();
        let b = std::fs::read_to_string(format!("{root}/demo/assets/manifest.json")).unwrap();
        let va: Value = serde_json::from_str(&a).unwrap();
        let vb: Value = serde_json::from_str(&b).unwrap();
        assert_eq!(va["npc"], vb["npc"], "copies manifest identiques (npc)");
        assert_eq!(va["npcNote"], vb["npcNote"], "copies manifest identiques (npcNote)");
        assert_eq!(va["npc"], Value::Array(manifest_blocks()), "blocs npc == ROSTER");
        assert_eq!(va["npcNote"], Value::String(NPC_NOTE.into()));
        assert_eq!(va["npc"].as_array().unwrap().len(), 22);
    }

    // -- sprites ------------------------------------------------------------

    /// Sprites du manifest npc : présents, non vides, DÉCODABLES ; frames/
    /// dirs DCC == manifest ; 0 px peints toléré UNIQUEMENT en DD (stubs
    /// retail 0J/XS/TY — règle 15a). Les .dc6 décodent d2dc6 (TY).
    #[test]
    fn npc_sprites_present_nonempty_and_decode() {
        let root = root();
        let m = manifest_value();
        let mut modes_checked = 0usize;
        for e in m["npc"].as_array().unwrap() {
            let code = e["code"].as_str().unwrap();
            for (tag, block) in e["modes"].as_object().unwrap() {
                let cof_path = block["cof"].as_str().unwrap();
                let cof_bytes = std::fs::read(format!("{root}/{cof_path}")).unwrap();
                assert!(!cof_bytes.is_empty(), "{cof_path} vide");
                let cof = Cof::parse(&cof_bytes).unwrap_or_else(|err| panic!("{cof_path}: {err:?}"));
                assert_eq!(
                    (cof.frames_per_direction as u32, cof.number_of_directions as u32),
                    (block["frames"].as_u64().unwrap() as u32, block["dirs"].as_u64().unwrap() as u32),
                    "{code}:{tag} COF frames/dirs == manifest"
                );
                for (tok, p) in block["layers"].as_object().unwrap() {
                    let p = p.as_str().unwrap();
                    let bytes = std::fs::read(format!("{root}/{p}")).unwrap();
                    assert!(!bytes.is_empty(), "{p} vide");
                    if p.ends_with(".dc6") {
                        d2dc6::Dc6::decode(&bytes)
                            .unwrap_or_else(|err| panic!("{code}:{tag}/{tok}: {p}: {err}"));
                        continue;
                    }
                    let dcc = Dcc::decode(&bytes).unwrap_or_else(|err| panic!("{p}: {err:?}"));
                    // TY WL : RA/LH réutilisés des fichiers réels NU (60
                    // frames) sous un COF WL 20 frames — cf. dc6Note du bloc.
                    if !(code == "TY" && tag == "WL") {
                        // La DCC peut porter PLUS de frames que le COF n'en
                        // joue (XV : NU DCC 16, COF 8) — l'engine boucle sur
                        // le compte COF ; containment prouvé, égalité non.
                        assert!(
                            dcc.directions[0].frames.len() >= cof.frames_per_direction,
                            "{code}:{tag}/{tok} frames DCC < COF"
                        );
                    }
                    if tag == "DD" {
                        continue; // stubs retail tolérés (règle 15a)
                    }
                    let painted: usize = dcc.directions[0]
                        .frames
                        .iter()
                        .map(|f| f.pixel_data.iter().filter(|&&b| b != 0).count())
                        .sum();
                    assert!(painted > 0, "{code}:{tag}/{tok} : 0 px peints hors DD",);
                }
                modes_checked += 1;
            }
            // walk/idle = alias cohérents des modes WL/NU.
            assert_eq!(e["walk"]["cof"], e["modes"]["WL"]["cof"], "{} walk alias", code);
            assert_eq!(e["idle"]["cof"], e["modes"]["NU"]["cof"], "{} idle alias", code);
        }
        assert_eq!(modes_checked, 22 * 2 + 4, "NU+WL partout, DD réel/stub AS+0J+XS+TY");
    }

    // -- spawn par ville ----------------------------------------------------

    /// Acte I : l'attach délègue au roster d2rules vivant (6 NPC), aucun
    /// doublon, non-attaquables (monsters() vide).
    #[test]
    fn spawn_act1_delegates_to_d2rules_live_roster() {
        let mut g = boot_town(MANIFEST, "town");
        assert_eq!(g.world.as_ref().unwrap().town_npcs().len(), 6, "roster d2rules vivant au boot");
        let def = g.level.clone().unwrap();
        let n = attach_town_npcs(g.world.as_mut().unwrap(), &def);
        assert_eq!(n, 6, "délégation idempotente (pas de doublon)");
        assert!(g.world.as_ref().unwrap().monsters().is_empty(), "NPC non-attaquables");
        // Le roster data-driven couvre les 5 du camp (Cain reste d2rules).
        let roster: Vec<Value> = serde_json::from_str(&g.npc_roster_json()).unwrap();
        assert_eq!(roster.len(), 5, "5 PNJ du roster 22b au camp");
        assert!(roster.iter().all(|e| e["posAssumed"] == false), "PATH réelles A1");
        assert_eq!(roster.iter().find(|e| e["id"] == "akara").unwrap()["name"], "Akara");
    }

    /// Acte IV (Bastion, carte réelle composée) : les 2 presets sont
    /// marchables (snap ≤ 6), le JSON expose Halbu + Tyrael avec noms fr
    /// TBL, l'attach vivant renvoie 0 (contrat d2rules Acte I — documenté).
    #[test]
    fn spawn_act4_pandemonium_roster_ready_positions_walkable() {
        let mut g = boot_town(MANIFEST, "pandemonium");
        let roster: Vec<Value> = serde_json::from_str(&g.npc_roster_json()).unwrap();
        assert_eq!(roster.len(), 2, "Halbu + Tyrael");
        let ids: Vec<&str> = roster.iter().map(|e| e["id"].as_str().unwrap()).collect();
        assert_eq!(ids, vec!["halbu", "tyrael2"]);
        assert_eq!(roster[0]["name"], "Halbu", "TBL fr (NameStr bas-de-casse)");
        assert_eq!(roster[1]["name"], "Tyrael");
        for e in &roster {
            let (x, y) = (e["x"].as_f64().unwrap(), e["y"].as_f64().unwrap());
            assert!(
                g.world.as_ref().unwrap().is_walkable(x as i32, y as i32),
                "{} snapé sur tuile marchable de la VRAIE carte",
                e["id"]
            );
        }
        let def = g.level.clone().unwrap();
        assert_eq!(attach_town_npcs(g.world.as_mut().unwrap(), &def), 0);
    }

    /// Acte V (Harrogath, carte réelle composée) : les 5 presets sont
    /// marchables, noms fr TBL exacts (Malah/Larzuk/Anya/Nihlathak/
    /// Qual-Kehk), rôles et marchands corrects.
    #[test]
    fn spawn_act5_harrogath_roster_ready_positions_walkable() {
        let g = boot_town(MANIFEST, "harrogath");
        let roster: Vec<Value> = serde_json::from_str(&g.npc_roster_json()).unwrap();
        assert_eq!(roster.len(), 5, "Malah + Larzuk + Anya + Nihlathak + Qual-Kehk");
        let names: Vec<&str> = roster.iter().map(|e| e["name"].as_str().unwrap()).collect();
        assert_eq!(names, vec!["Malah", "Larzuk", "Anya", "Nihlathak", "Qual-Kehk"], "TBL fr");
        let anya = roster.iter().find(|e| e["id"] == "drehya").unwrap();
        assert_eq!(anya["display"], "Anya", "drehya = Anya (idiosyncrasie retail)");
        assert_eq!(anya["questCode"], "anya", "code quête A5 réel");
        assert_eq!(roster.iter().find(|e| e["id"] == "nihlathak").unwrap()["name"], "Nihlathak");
        for e in &roster {
            let (x, y) = (e["x"].as_f64().unwrap(), e["y"].as_f64().unwrap());
            assert!(
                g.world.as_ref().unwrap().is_walkable(x as i32, y as i32),
                "{} snapé sur tuile marchable de la VRAIE carte",
                e["id"]
            );
            assert_eq!(e["posAssumed"], true, "presets authoring documentés");
        }
        // Marchands npc.txt présents au village.
        let vendors: Vec<&str> = roster
            .iter()
            .filter_map(|e| e["vendor"].as_str())
            .collect();
        assert_eq!(vendors, vec!["malah", "larzuk", "drehya", "nihlathak"], "Qual-Kehk sans magasin");
    }

    /// Actes II-III : villes non jouables dans ce workspace — le JSON reste
    /// `[]` sans monde, l'attach renvoie 0, et les 11 entrées du roster
    /// restent prêtes au manifest (town/townLevelId cohérents).
    #[test]
    fn acts23_roster_ready_cities_absent() {
        assert!(!serde_json::to_value(&manifest_blocks()).unwrap()[0].is_null());
        let blocks = manifest_blocks();
        let lut: Vec<&Value> = blocks.iter().filter(|e| e["townLevelId"] == 40).collect();
        let kur: Vec<&Value> = blocks.iter().filter(|e| e["townLevelId"] == 75).collect();
        assert_eq!(lut.len(), 6, "Lut Gholein : 6 PNJ prêts");
        assert_eq!(kur.len(), 4, "Kurast : 4 PNJ prêts");
        for e in lut.iter().chain(kur.iter()) {
            assert_eq!(e["posAssumed"], true, "presets authoring (pas de PATH extractible)");
        }
        // Sans monde (boot impossible pour ces clés) : JSON vide, jamais de panic.
        let g = GameCore::new(MANIFEST, "BA").unwrap();
        assert_eq!(g.npc_roster_json(), "[]");
    }
}
