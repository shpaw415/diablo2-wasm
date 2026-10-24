//! `audio_env` — étape 14c (audio, campagne diablo-2-wasm) : résolution PURE
//! niveau → ambiance (`SoundEnviron.txt` v1.12 → boucle extraite).
//!
//! Sources réelles (lecture seule, embarquées en l'état) :
//! * `d2w-re/tables/monde/SoundEnviron.txt` — 50 records : `Index` (l'id
//!   `SoundEnv` de `levels.txt`, colonne 119 réelle de
//!   `assets/patch_levels.txt`), `Song` (id `music.txt`), `Day/Night
//!   Ambience` (ids `sounds.txt`), `Day/Night Event` + `Event Delay`
//!   (one-shots — ids portés, fichiers NON extraits au 14c), `Indoors`.
//! * `data\global\excel\sounds.txt` (patch_d2.mpq, v1.12) — la table des ids
//!   d'ambiance : les ids 52-71 sont les boucles « scene »
//!   (`data\global\sfx\ambient\scene\*.wav`), `Loop=1` : conçues pour boucler.
//!
//! Les 15 boucles extraites au 14c (`demo/assets/sfx/ambiance/`, 2-8 s,
//! 22 kHz stéréo — voir `tools/extract_ambiance.py`) couvrent TOUTES les
//! valeurs Day/Night Ambience des 50 records. Trois variantes nuit ne sont
//! pas extraites (57 désert, 60 jungle, 69 ville A3) : la table les résout
//! sur la boucle jour correspondante — fallback documenté, jamais de
//! changement de silence. Test de TOTALITÉ : tout id référencé résout.
//!
//! Côté démo, `demo/music.js` porte une table JS miroir (mêmes stems,
//! mêmes fallbacks) — ce module est le contrat testé par golden (12 niveaux
//! réels A1-A5 de `patch_levels.txt`). Zéro IO, zéro panique.

use crate::tsv::Tsv;
use std::sync::OnceLock;

/// `SoundEnviron.txt` v1.12 (51 lignes = en-tête + 50 records), embarqué
/// EN L'ÉTAT depuis `d2w-re/tables/monde/SoundEnviron.txt` (2026-10-24,
/// miroir `data\global\excel\SoundEnviron.txt` de patch_d2.mpq).
const SOUND_ENVIRON_TSV: &str = r#"Handle	Index	Song	Day Ambience	Night Ambience	Day Event	Night Event	Event Delay	Indoors	Material 1	Material 2	EAX Environ	EAX Env Size	EAX Env Diff	EAX Room Vol	EAX Room HF	EAX Decay Time	EAX Decay HF	EAX Reflect	EAX Reflect Delay	EAX Reverb	EAX Rev Delay	EAX Room Roll	EAX Air Absorb
ESOUNDENVIRON_NONE	0	0	0	0	0	0	250	0	0	0	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_TOWN_1	1	4673	70	71	0	0	250	0	1	5	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_WILDERNESS	2	4679	70	71	192	197	250	0	1	5	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_TRISTRAM	3	4677	70	71	192	197	250	0	1	5	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_CAVE	4	4657	54	54	87	87	200	1	1	0	5	11600	1000	-1000	-300	2700	640	-711	12	83	17	0	-5000
ESOUNDENVIRON_CRYPT	5	4658	55	55	95	95	500	1	2	0	5	11600	1000	-1240	-300	1700	640	-711	12	83	17	0	-5000
ESOUNDENVIRON_MONASTERY	6	4667	63	63	157	157	250	0	1	3	7	19600	1000	-1650	-500	3300	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_COURTYARD	7	4667	63	63	157	157	250	0	1	3	7	19600	1000	-1650	-500	3300	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_BARRACKS	8	4667	52	52	82	82	800	1	1	0	6	21600	1000	-1540	-476	2600	590	-789	20	-289	30	0	-5000
ESOUNDENVIRON_CATHEDRAL	9	4667	53	53	77	77	500	1	2	0	7	19600	1000	-1240	-500	4750	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_CATACOMBS	10	4667	52	52	82	82	800	1	2	0	5	11600	1000	-1600	-300	2310	640	-711	12	83	17	0	-5000
ESOUNDENVIRON_ANDARIEL_LAIR	11	4667	52	52	82	82	250	1	2	0	6	11600	1000	-1540	-476	2600	590	-789	20	-289	30	0	-5000
ESOUNDENVIRON_TOWN_2	12	4674	56	57	172	177	250	0	3	4	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_DESERT	13	4659	56	57	100	100	250	0	4	3	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_VALLEY_OF_KINGS	14	4678	56	57	100	100	250	0	4	3	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_SEWERS	15	4670	65	65	87	87	150	1	2	0	13	13500	1000	-1650	-237	1400	790	-1214	13	395	20	0	-5000
ESOUNDENVIRON_HAREM	16	4661	58	58	110	110	750	1	2	0	9	36200	1000	-1700	-698	4000	330	-1166	20	16	30	0	-5000
ESOUNDENVIRON_CORRUPT_HAREM	17	4661	58	58	110	110	750	1	2	0	9	36200	1000	-1700	-698	4000	330	-1166	20	16	30	0	-5000
ESOUNDENVIRON_BASEMENT	18	4661	52	52	110	110	750	1	2	0	10	50300	1000	-1592	-1000	3300	230	-602	20	198	30	0	-5000
ESOUNDENVIRON_TOMBS	19	4672	54	54	138	138	500	1	2	4	9	36200	1000	-1356	-698	3400	330	-1166	20	16	56	0	-5000
ESOUNDENVIRON_MAGGOT_LAIR	20	4665	67	67	77	77	500	1	1	0	13	13500	1000	-1500	-237	1800	790	-1214	13	395	20	0	-5000
ESOUNDENVIRON_DURIEL_LAIR	21	4672	67	67	77	77	500	1	2	0	13	13500	1000	-1500	-237	2000	790	-1214	13	395	20	0	-5000
ESOUNDENVIRON_ARCANE	22	4669	62	62	0	0	250	0	2	0	7	19600	1000	-2091	-500	4000	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_TOWN_3	23	4675	68	69	182	187	500	0	1	0	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_JUNGLE	24	4662	59	60	118	123	300	0	1	0	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_KURAST	25	4663	68	60	128	133	325	0	1	0	17	100000	270	-2175	-2500	1800	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_KURAST_SEWER	26	4664	54	54	95	95	500	1	2	0	5	11600	1000	-1200	-300	2700	640	-711	12	83	17	0	-5000
ESOUNDENVIRON_DUNGEON	27	4671	65	65	138	138	250	1	2	0	5	11600	1000	-1240	-300	1700	640	-711	12	83	17	0	-5000
ESOUNDENVIRON_TEMPLE	28	4664	53	53	162	162	250	0	2	0	9	36200	1000	-1700	-698	4100	1167	-1166	20	16	30	0	-5000
ESOUNDENVIRON_SPIDER_LAIR	29	4671	54	54	72	72	250	1	1	0	5	11600	1000	-1600	-300	2310	330	-711	12	83	17	0	-5000
ESOUNDENVIRON_MEPHISTO_LAIR	30	4664	52	52	162	162	250	1	2	0	7	19600	1000	-1400	-500	4750	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_TOWN_4	31	4676	62	62	0	0	500	0	1	0	5	11600	1000	-1600	-300	2310	640	-711	12	83	17	0	-5000
ESOUNDENVIRON_MESA	32	4666	62	62	82	82	300	0	1	0	12	1800	1000	-2300	-300	4700	590	-1217	7	441	11	0	-5000
ESOUNDENVIRON_LAVA	33	4666	61	61	143	143	300	0	1	0	12	1800	1000	-2300	-300	4700	590	-1217	7	441	11	0	-5000
ESOUNDENVIRON_DIABLO_LAIR	34	4660	61	61	143	143	300	0	1	0	12	1800	1000	-2100	-300	4700	590	-1217	7	441	11	0	-5000
ESOUNDENVIRON_GUILD	35	4679	70	71	0	0	250	0	1	5	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_XTOWN	36	4684	70	71	0	0	250	0	1	3	0	0	0	-10000	-10000	0	0	0	0	0	0	0	0
ESOUNDENVIRON_BARRICADE	37	4682	70	71	72	77	250	0	1	5	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_BARRICADESNOW	38	4682	70	71	72	77	250	0	1	6	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_ICECAVE1	39	4681	54	54	87	138	250	1	2	5	5	11600	1000	-1600	-300	2310	330	-711	12	83	17	0	-5000
ESOUNDENVIRON_ICECAVE2	40	4681	54	54	138	87	250	1	2	5	5	11600	1000	-1600	-300	2310	330	-711	12	83	17	0	-5000
ESOUNDENVIRON_ICECAVE3	41	4681	54	54	87	138	250	1	2	5	5	11600	1000	-1600	-300	2310	330	-711	12	83	17	0	-5000
ESOUNDENVIRON_MOUNTAINTOP	42	4682	63	63	72	77	250	0	3	6	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_XTEMPLEENTRANCE	43	4682	63	63	77	72	500	0	1	6	17	100000	270	-2100	-2500	1490	210	-2780	226	-1434	100	0	-5000
ESOUNDENVIRON_XTEMPLE	44	4683	52	52	162	162	250	1	1	2	7	19600	1000	-1400	-500	4750	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_XTEMPLEBOSS	45	4683	52	52	72	72	250	1	1	2	7	19600	1000	-1400	-500	4750	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_XHELL	46	4666	62	62	143	143	300	1	1	0	12	1800	1000	-2300	-300	4700	590	-1217	7	441	11	0	-5000
ESOUNDENVIRON_BAALTEMPLE	47	4680	52	52	82	82	250	1	2	5	7	19600	1000	-1400	-500	4750	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_BAALTHRONE	48	4680	52	52	82	82	250	1	2	0	7	19600	1000	-1400	-500	4750	700	-1230	20	-2	29	0	-5000
ESOUNDENVIRON_WORLDSTONE	49	4680	53	53	82	82	250	1	2	0	7	19600	1000	-1240	-500	4750	700	-1230	20	-2	29	0	-5000"#;

/// Un record `SoundEnviron.txt` (champs portés au 14c ; les colonnes EAX
/// acoustiques sont hors périmètre — rien ne les consomme).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundEnviron {
    /// `Index` — l'id porté par `levels.txt` colonne `SoundEnv` (0..=49).
    pub index: u8,
    /// `Handle` — nom symbolique (ex. `ESOUNDENVIRON_WILDERNESS`).
    pub handle: &'static str,
    /// `Song` — id `music.txt` (piste musique officielle du record).
    pub song: u32,
    /// `Day Ambience` — id `sounds.txt` de la boucle jour (0 = aucune).
    pub day_ambience: u16,
    /// `Night Ambience` — id `sounds.txt` de la boucle nuit (0 = aucune).
    pub night_ambience: u16,
    /// `Day Event` — id `sounds.txt` du one-shot jour (0 = aucun, NON extrait).
    pub day_event: u16,
    /// `Night Event` — id `sounds.txt` du one-shot nuit (0 = aucun, NON extrait).
    pub night_event: u16,
    /// `Event Delay` — délai de tirage des one-shots (ms, convention
    /// Diablerie ; porté brut, non consommé au 14c).
    pub event_delay: u32,
    /// `Indoors` — 1 = intérieur (pas de cycle jour/nuit visible en D2).
    pub indoors: bool,
}

/// Table des ids d'ambiance référencés par les 50 records (aucun autre id
/// n'apparaît dans `Day/Night Ambience`) → STEM du WAV extrait dans
/// `demo/assets/sfx/ambiance/` (`AMBIENCE_DIR` + stem + `.wav`).
///
/// Fallbacks DOCUMENTÉS (variantes nuit non extraites — fourchette 8-15
/// boucles de la mission 14c) : 57 (désert nuit) → 56, 60 (jungle nuit) → 59,
/// 69 (ville A3 nuit) → 68. Les stems sont ceux de `sounds.txt` colonne
/// `Sound` (ex. id 62 `scene_mesa` joue le fichier MPQ `hell1.wav`).
const AMBIENCE_LOOPS: &[(u16, &str)] = &[
    (52, "scene_catacombs"),
    (53, "scene_cathedral"),
    (54, "scene_cave"),
    (55, "scene_crypt"),
    (56, "scene_desert_day"),
    (57, "scene_desert_day"), // 57 désert nuit → boucle jour (non extraite)
    (58, "scene_harem"),
    (59, "scene_jungle_day"),
    (60, "scene_jungle_day"), // 60 jungle nuit → boucle jour (non extraite)
    (61, "scene_lava"),
    (62, "scene_mesa"),
    (63, "scene_monastery"),
    (65, "scene_sewer"),
    (67, "scene_tomb"),
    (68, "scene_town_3_day"),
    (69, "scene_town_3_day"), // 69 ville A3 nuit → boucle jour (non extraite)
    (70, "scene_wilderness_day"),
    (71, "scene_wilderness_night"),
];

/// Répertoire démo des boucles (relatif à `demo/`, comme `assets/music/`).
pub const AMBIENCE_DIR: &str = "assets/sfx/ambiance/";

fn sound_environ_table() -> &'static Tsv {
    static TABLE: OnceLock<Tsv> = OnceLock::new();
    TABLE.get_or_init(|| Tsv::parse(SOUND_ENVIRON_TSV).expect("SoundEnviron.txt embarqué illisible"))
}

/// Record `SoundEnviron` par id `SoundEnv` (`levels.txt`, colonne 119) —
/// `None` si l'id est hors table (levels.txt n'en a que 0..=49).
pub fn record(sound_env: u8) -> Option<SoundEnviron> {
    let tsv = sound_environ_table();
    for row in 0..tsv.len() {
        let rec = tsv.rec(row)?;
        if rec.u("Index") == sound_env as u32 {
            let handle: &'static str = rec.s("Handle");
            return Some(SoundEnviron {
                index: sound_env,
                handle,
                song: rec.u("Song"),
                day_ambience: rec.u("Day Ambience") as u16,
                night_ambience: rec.u("Night Ambience") as u16,
                day_event: rec.u("Day Event") as u16,
                night_event: rec.u("Night Event") as u16,
                event_delay: rec.u("Event Delay"),
                indoors: rec.u("Indoors") != 0,
            });
        }
    }
    None
}

/// Id `sounds.txt` de la boucle d'ambiance du record (jour ou nuit),
/// 0 = aucune (ex. id 0 `NONE`, `Town 4` en événements seuls).
pub fn ambience_id(sound_env: u8, night: bool) -> Option<u16> {
    let r = record(sound_env)?;
    Some(if night { r.night_ambience } else { r.day_ambience })
}

/// Résolution région → AMBIANCE : stem du WAV extrait à jouer en boucle
/// (`AMBIENCE_DIR` + stem + `.wav`). `None` = aucune ambiance (id 0 ou
/// id non table) — l'appelant garde sa boucle courante, jamais de coupure.
pub fn ambience(sound_env: u8, night: bool) -> Option<&'static str> {
    let id = ambience_id(sound_env, night)?;
    if id == 0 {
        return None;
    }
    AMBIENCE_LOOPS.iter().find(|(i, _)| *i == id).map(|(_, stem)| *stem)
}

/// Id `music.txt` officiel du record (croisement futur musique ↔ ambiance).
pub fn song(sound_env: u8) -> Option<u32> {
    Some(record(sound_env)?.song)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 12 niveaux RÉELS de `assets/patch_levels.txt` (A1-A5, colonnes 1/119)
    /// → golden (se, stem jour, stem nuit). Nuits A2/A3 = fallback jour
    /// documenté (57→56, 69→68, 60→59).
    const GOLDEN_LEVELS: &[(u8, &str, &str, &str)] = &[
        (1, "Act 1 - Town", "scene_wilderness_day", "scene_wilderness_night"),
        (2, "Act 1 - Wilderness 1 (Blood Moor)", "scene_wilderness_day", "scene_wilderness_night"),
        (10, "Act 1 - Catacombs 4", "scene_catacombs", "scene_catacombs"),
        (12, "Act 2 - Town (Lut Gholein)", "scene_desert_day", "scene_desert_day"),
        (19, "Act 2 - Tomb Tal 1", "scene_cave", "scene_cave"),
        (21, "Act 2 - Duriel's Lair", "scene_tomb", "scene_tomb"),
        (23, "Act 3 - Town (Kurast Docks)", "scene_town_3_day", "scene_town_3_day"),
        (24, "Act 3 - Jungle 1", "scene_jungle_day", "scene_jungle_day"),
        (33, "Act 4 - Lava 1", "scene_lava", "scene_lava"),
        (39, "Act 5 - Ice Cave 1", "scene_cave", "scene_cave"),
        (46, "Act 5 - Hell 1", "scene_mesa", "scene_mesa"),
        (49, "Act 5 - World Stone", "scene_cathedral", "scene_cathedral"),
    ];

    #[test]
    fn golden_region_to_ambiance_a1_a5() {
        for (se, name, day, night) in GOLDEN_LEVELS {
            assert_eq!(ambience(*se, false), Some(*day), "{name} (jour)");
            assert_eq!(ambience(*se, true), Some(*night), "{name} (nuit)");
        }
    }

    #[test]
    fn table_parse_50_records() {
        let tsv = sound_environ_table();
        assert_eq!(tsv.len(), 50, "SoundEnviron.txt v1.12 = 50 records");
        for i in 0..50u8 {
            let r = record(i).unwrap_or_else(|| panic!("record {i} absent"));
            assert_eq!(r.index, i);
            assert!(!r.handle.is_empty(), "handle {i} vide");
        }
        assert!(record(50).is_none(), "id hors table");
    }

    #[test]
    fn totalite_tous_les_ids_resolvent() {
        for i in 0..50u8 {
            for night in [false, true] {
                match ambience_id(i, night) {
                    None => panic!("record {i} hors table ?"),
                    Some(0) if i == 0 => {} // record NONE : silence voulu
                    Some(0) => panic!("record {i} sans ambience ?"),
                    Some(id) => {
                        assert!(
                            AMBIENCE_LOOPS.iter().any(|(j, _)| *j == id),
                            "id {id} (record {i}) non résolu"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn champs_reels_de_quelques_records() {
        let t = record(1).unwrap();
        assert_eq!(t.handle, "ESOUNDENVIRON_TOWN_1");
        assert_eq!(t.song, 4673);
        assert!(!t.indoors);
        let m = record(6).unwrap();
        assert_eq!(m.day_ambience, 63);
        assert_eq!(m.event_delay, 250);
        let x = record(46).unwrap(); // XHELL
        assert!(x.indoors);
        assert_eq!(x.day_ambience, 62);
    }

    #[test]
    fn none_et_fallbacks_documentes() {
        assert_eq!(ambience(0, false), None, "record NONE = silence");
        // fallbacks nuit → boucle jour
        assert_eq!(ambience(12, true), ambience(12, false)); // 57 → 56
        assert_eq!(ambience(12, true), Some("scene_desert_day"));
        assert_eq!(ambience(23, true), Some("scene_town_3_day"));
        assert_eq!(ambience(24, true), Some("scene_jungle_day"));
    }
}
