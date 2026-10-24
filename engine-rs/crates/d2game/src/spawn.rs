//! Spawn extérieur data-driven — socle W01/A3 (audits d2w-re), consommateur
//! réel n°1 du parseur TSV générique [`d2rules::tsv`].
//!
//! Remplace la palette codée en dur `DEFAULT_OUTDOOR_MONSTERS` de `game.rs`
//! (6 codes constants `FA/FA/FA/ZM/ZM/SI`) par les planches réelles
//! `mon1`..`mon10` du record levels.txt :
//!
//! * `assets/patch_levels.txt` — ex. Blood Moor (`Id=2`, colonne `Name` =
//!   « Act 1 - Wilderness 1 », colonne `LevelName` = « Blood Moor ») :
//!   `mon1..mon3` = zombie1/fallen1/quillrat1 (mon4..mon10 vides),
//!   `MonDen` = 520, `NumMon` = 3 (ligne 4 du fichier, vérifiée).
//! * `assets/patch_monstats.txt` — packs officiels `MinGrp`/`MaxGrp` par
//!   token (zombie1 1-2, fallen1 2-3, quillrat1 1-2) + colonne `Code` =
//!   code sprite (`ZM`/`FA`/`SI`) consommé par `LevelDef.monsters`.
//!
//! Les tables sont embarquées (`include_str!`, même modèle que d2rules) :
//! zéro IO, disponibles même quand l'hôte WASM ne fournit pas les assets
//! runtime. Parse une fois ([`std::sync::OnceLock`]), lecture ensuite.

use d2rules::tsv::Tsv;
use std::collections::HashMap;
use std::sync::OnceLock;

const PATCH_LEVELS_TXT: &str = include_str!("../../../../assets/patch_levels.txt");
const PATCH_MONSTATS_TXT: &str = include_str!("../../../../assets/patch_monstats.txt");

/// Une entrée de planche `mon1..mon10` résolue : token table (`zombie1`),
/// code sprite (`ZM`, colonne `Code` de patch_monstats.txt) et taille de
/// pack officielle (`MinGrp`/`MaxGrp` — cellule vide ⇒ 0, normalisée dans
/// [`pack_size`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonPlate {
    pub token: String,
    pub code: String,
    pub min_grp: u32,
    pub max_grp: u32,
}

/// Plan de spawn extérieur d'un record levels.txt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutdoorSpawn {
    /// Id levels.txt (`Id`).
    pub id: u32,
    /// Nom affichable EN (colonne `LevelName`, ex. « Blood Moor ») — la
    /// colonne `Name` porte le nom technique (« Act 1 - Wilderness 1 »).
    pub level_name: String,
    /// Planches `mon1..mon10` dans l'ordre des colonnes (cellules vides
    /// sautées) avec packs résolus.
    pub plates: Vec<MonPlate>,
    /// Densité de population (`MonDen`, 0 si cellule vide).
    pub mon_den: u32,
    /// Monstres par groupe cible (`NumMon`, 0 si cellule vide).
    pub num_mon: u32,
}

impl OutdoorSpawn {
    /// Codes sprites de la planche, dans l'ordre — consommé par
    /// `LevelDef::from_manifest` pour `LevelDef.monsters`.
    pub fn plate_codes(&self) -> Vec<String> {
        self.plates.iter().map(|p| p.code.clone()).collect()
    }

    /// Packs `(MinGrp, MaxGrp)` parallèles à la planche.
    pub fn plate_packs(&self) -> Vec<(u32, u32)> {
        self.plates.iter().map(|p| (p.min_grp, p.max_grp)).collect()
    }
}

/// Taille de pack tirée pour `(min_grp, max_grp)` et un rouleau `roll ∈
/// 0..range` : `min + roll`, bornée ≥ 1 (cellules vides/0 ⇒ 1, cf.
/// monstres.rs « MinGrp..MaxGrp, 0 ⇒ 1 »).
pub fn pack_size(min_grp: u32, max_grp: u32, roll: u32) -> u32 {
    if max_grp <= min_grp {
        return min_grp.max(1);
    }
    (min_grp + roll % (max_grp - min_grp + 1)).max(1)
}

/// Tables embarquées parsées une fois, indexées par Id levels.txt.
fn tables() -> &'static HashMap<u32, OutdoorSpawn> {
    static TABLES: OnceLock<HashMap<u32, OutdoorSpawn>> = OnceLock::new();
    TABLES.get_or_init(|| {
        // Packs par token table (patch_monstats.txt, clé = colonne `Id`).
        // Un même code sprite peut servir plusieurs tokens (quillrat1 ET
        // quillrat2 → `SI`) : l'index par token garde le pack propre à
        // chacun ; l'affichage/rendu passe par `Code`.
        let mut packs: HashMap<String, (String, u32, u32)> = HashMap::new();
        if let Ok(t) = Tsv::parse(PATCH_MONSTATS_TXT) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let id = r.s("Id");
                if id.is_empty() {
                    continue;
                }
                packs.insert(
                    id.to_string(),
                    (r.s("Code").to_ascii_uppercase(), r.u("MinGrp"), r.u("MaxGrp")),
                );
            }
        }
        let mut map = HashMap::new();
        if let Ok(t) = Tsv::parse(PATCH_LEVELS_TXT) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let Some(id) = r.s("Id").parse::<u32>().ok() else { continue };
                let plates = ["mon1", "mon2", "mon3", "mon4", "mon5", "mon6", "mon7", "mon8", "mon9", "mon10"]
                    .iter()
                    .filter_map(|col| {
                        let token = r.s(col);
                        if token.is_empty() {
                            return None; // mon4..mon10 vides pour Blood Moor
                        }
                        let (code, min_grp, max_grp) = packs
                            .get(token)
                            .cloned()
                            .unwrap_or_else(|| (token.to_ascii_uppercase(), 1, 1));
                        Some(MonPlate { token: token.to_string(), code, min_grp, max_grp })
                    })
                    .collect();
                map.insert(
                    id,
                    OutdoorSpawn {
                        id,
                        level_name: r.s("LevelName").to_string(),
                        plates,
                        mon_den: r.u("MonDen"),
                        num_mon: r.u("NumMon"),
                    },
                );
            }
        }
        map
    })
}

/// Plan de spawn data-driven d'un Id levels.txt — `None` si le record est
/// absent de la table (l'appelant garde son repli historique).
pub fn outdoor_spawn(levels_id: u32) -> Option<OutdoorSpawn> {
    tables().get(&levels_id).cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bloodmoor_plates_match_patch_levels_line_4() {
        // assets/patch_levels.txt ligne 4 (`Id=2`, « Act 1 - Wilderness 1 »,
        // `LevelName` « Blood Moor ») : mon1..mon3 = zombie1/fallen1/
        // quillrat1, mon4..mon10 vides, MonDen=520, NumMon=3 — vérifié à la
        // main dans le fichier (2026-10-24).
        let s = outdoor_spawn(2).expect("Id=2 présent de patch_levels.txt");
        assert_eq!(s.id, 2);
        assert_eq!(s.level_name, "Blood Moor");
        let tokens: Vec<&str> = s.plates.iter().map(|p| p.token.as_str()).collect();
        assert_eq!(tokens, ["zombie1", "fallen1", "quillrat1"], "ordre des colonnes mon1-10");
        assert_eq!(s.mon_den, 520, "MonDen réel = 520");
        assert_eq!(s.num_mon, 3, "NumMon réel = 3");
        // Codes sprites résolus via patch_monstats.txt colonne `Code`.
        assert_eq!(s.plate_codes(), ["ZM", "FA", "SI"]);
    }

    #[test]
    fn bloodmoor_packs_match_patch_monstats() {
        // assets/patch_monstats.txt : zombie1 MinGrp=1/MaxGrp=2, fallen1
        // 2/3, quillrat1 1/2 (lignes Id=zombie1/fallen1/quillrat1, vérifiées
        // à la main — 2026-10-24). NB : quillrat2 porte aussi le code `SI`
        // mais son pack propre (1-3) n'affecte pas quillrat1.
        let s = outdoor_spawn(2).unwrap();
        assert_eq!(s.plate_packs(), [(1, 2), (2, 3), (1, 2)]);
    }

    #[test]
    fn pack_size_stays_within_min_max() {
        // Cellules vides/0 ⇒ 1 (convention monstres.rs).
        assert_eq!(pack_size(0, 0, 99), 1);
        assert_eq!(pack_size(1, 1, 7), 1);
        // zombie1 (1-2) : rouleau 0..2 → 1 ou 2.
        assert_eq!(pack_size(1, 2, 0), 1);
        assert_eq!(pack_size(1, 2, 1), 2);
        assert_eq!(pack_size(1, 2, 2), 1, "modulo range");
        // fallen1 (2-3).
        assert_eq!(pack_size(2, 3, 0), 2);
        assert_eq!(pack_size(2, 3, 1), 3);
        // max < min : borné au min.
        assert_eq!(pack_size(3, 1, 5), 3);
    }

    #[test]
    fn unknown_level_id_is_none() {
        assert!(outdoor_spawn(u32::MAX).is_none());
        assert!(outdoor_spawn(9999).is_none(), "Id hors table → repli appelant");
    }

    #[test]
    fn level_rows_without_integer_id_are_skipped() {
        // DÉCOUVERTE FORMAT : patch_levels.txt ligne 111 (« Expansion »,
        // séparateur de section Nightmare/Hell, Id vide) — le parseur
        // lenient la saute (record sans Id entier ⇒ pas indexé), les 137
        // vrais niveaux restent adressables.
        let t = Tsv::parse(PATCH_LEVELS_TXT).unwrap();
        let ids: Vec<u32> = (0..t.len())
            .filter_map(|i| t.rec(i).and_then(|r| r.s("Id").parse::<u32>().ok()))
            .collect();
        assert_eq!(ids.len(), t.len() - 1, "une ligne à Id vide (ligne 111), 137 niveaux");
        for id in &ids {
            assert!(outdoor_spawn(*id).is_some(), "Id={id} indexé");
        }
        // Camp (Id=1) : planche vide → l'appelant le traite en zone sûre.
        let camp = outdoor_spawn(1).unwrap();
        assert!(camp.plates.is_empty(), "town sans planche de monstres");
        assert_eq!(camp.level_name, "Rogue Encampment");
    }
}
