//! `mercenary` — MERCENAIRES v1.12 (domaine H, `finish-systemes.md` §H1,
//! chantier 17 de `finish-jeu.md`).
//!
//! Port des règles D2 réelles, 100 % data-driven depuis la table RÉELLE
//! `assets/patch_hireling.txt` (120 records v1.12, extraite du MPQ
//! `patch_d2.mpq` le 2026-10-23 — `data\global\excel\hireling.txt`,
//! 26 415 octets, sha256 `c743a6489456bd2c…8f54a555afa78`). Chaque champ
//! cite sa colonne (`hireling.txt:colonne`) ; aucune valeur inventée.
//!
//! # Les 4 types (colonne `Hireling`, acte col `Act`)
//! * **Rogue Scout** (acte 1) — arc (`WType1=bow`), sous-types Fire/Ice :
//!   **distance**, flèche porteuse d'arme (`firearrow`/`icearrow` de la
//!   table d2missiles, carriers `Weapon` — dégâts de l'arme transportés).
//! * **Desert Mercenary** (acte 2) — perche (`WType1=pole`), sous-types
//!   Combat/Défense/Offense : **mêlée**.
//! * **Eastern Sorceror** (acte 3) — épée+bouclier (`WType1=swor`,
//!   `WType2=shie`), sous-types Fire/Cold/Ltng : **distance magique** —
//!   MVP : le missile de MÊME ÉLÉMENT de la table d2missiles embarquée
//!   (`firebolt` 3-8 feu / `icebolt` 3-7 froid / `chargedbolt` 2-6 foudre,
//!   carriers `Skill` = dégâts officiels niveau 1). ÉCART documenté : les
//!   sorts réels du mercenaire (colonne `Skill1/Skill2` : Inferno/Fire Ball,
//!   Glacial Spike, Charged Bolt/Lightning) ne sont pas dans la table
//!   d2missiles MVP (10 définitions) — restant.
//! * **Barbarian** (acte 5) — épée 1 main (`WType1=swor`) : **mêlée**.
//!
//! # Règles portées (chaque valeur = `fichier:colonne`)
//! * **Embauche** : coût = colonne `Gold` du record (ex. Rogue Normal lvl 3 =
//!   100 or, Desert Normal = 350, Sorceror = 1 000-1 500, Barbarian = 9 000) ;
//!   niveau offert = record `Level` le plus haut ≤ niveau du héros (paliers
//!   réels de la table : Rogue Normal 3/25/36/49/67…) ; sous-type par défaut
//!   = `DefaultChance` maximal (ex. Rogue Fire 75 > Ice 25 ; égalité ⇒
//!   premier record du fichier). Débité de l'or du héros.
//! * **NPC vendeur** : colonne `Seller` = `Id` de `patch_monstats.txt` —
//!   150 = `kashya` (acte 1), 198 = `greiz` (acte 2), 252 = `asheara`
//!   (acte 3), 515 = `qual-kehk` (acte 5) : exactement les 4 vendeurs de
//!   mercenaires du jeu.
//! * **Scaling par niveau** : au niveau `lvl` ≥ `Level` du record,
//!   HP = `HP` + (`lvl`−`Level`)×`HP/Lvl`, idem `Def/Lvl`, `AR/Lvl`,
//!   `Dmg/Lvl` (ajouté aux min/max), `Str/Lvl`, `Dex/Lvl`, `Resist/Lvl`
//!   (toutes résistances, colonne `Resist`).
//! * **XP / level-up** : palier = colonne `Exp/Lvl` du record (MVP : palier
//!   constant ; le scaling exact client v1.12 — restant) ; plafond hireling
//!   **98** (règle D2 v1.10+ connue, non portée par hireling.txt). Le
//!   mercenaire gagne l'XP des monstres qu'IL tue (le joueur reçoit la
//!   sienne via `MonsterDied`, flux inchangé).
//! * **Un seul mercenaire à la fois** (règle officielle D2 — « You may only
//!   have one hireling at a time », Arreat Summit) : embauche REFUSÉE tant
//!   que le mercenaire courant est vivant ; embaucher pendant qu'il est KO
//!   **remplace** l'ancien, définitivement.
//! * **KO / résurrection** : hp 0 ⇒ KO (cadavre sur place, anim DD) ;
//!   résurrection = pleine vie, auprès du NPC de son type. COÛT : la table
//!   v1.12 ne porte PAS de colonne de résurrection (73 colonnes auditées) ⇒
//!   MVP gratuit (0) — la formule client par niveau est un restant.
//! * **Combat** : suit le héros (rayon 3,5 tuiles), attaque LA CIBLE DU
//!   HÉROS en priorité (sinon le monstre vivant le plus proche, rayon 8) ;
//!   mêlée = jet `200×AR/(AR+AC)` [5,95] (formule §3 GAME-DATA, dégâts
//!   `Dmg-Min..Dmg-Max`) ; distance = missile d2missiles (rogue : dégâts
//!   d'arme du mercenaire ; sorceror : dégâts `Skill` de la table). Les
//!   monstres frappent la cible la plus PROCHE (héros ou mercenaire vivant) —
//!   additif : sans mercenaire, le flux historique est inchangé.
//!
//! # Persistance
//! Champ save ADDITIF `mercenary` (`#[serde(default)]`, save v3 inchangée
//! ailleurs — les autres chantiers évoluent en parallèle) : type/sous-type/
//! niveau/xp/hp/entité/missiles en vol. Les stats effectives sont
//! recalculées au chargement depuis la table (pas de double source).
//!
//! # 3c-a — MERCENAIRES PAR JOUEUR (coop)
//! Chaque SLOT embauche SON mercenaire (type indépendant, bourse et niveau
//! du joueur propriétaire), suit SON joueur, et meurt/ressuscite chez le NPC
//! de sa ville ([`World::hire_merc_slot`]/[`World::resurrect_merc_slot`]).
//! Le merc du SLOT 0 reste l'objet RACINE `World.merc` (bit-exact solo) et
//! porte en `MercData::coop` l'état des slots ≥ 1 (`players_coop.rs` — aucun
//! champ World disponible : world.rs gelé 3c) ; quand le slot 0 n'a pas de
//! merc, la racine est une COQUILLE invisible (`slot == usize::MAX`).
//! Les mercenaires frappent les monstres et ENCAISSENT via le hook documenté
//! `monster_strikes_nearest_merc` (le merc le plus PROCHE de tous les slots).
//! ÉCARTS documentés : (1) la cible du joueur n'existe que pour le slot 0
//! (`merc_target` historique) — les mercs des slots ≥ 1 visent le monstre
//! vivant le plus proche ; (2) tous les mercs consomment le flux RNG MONDE
//! (comme le merc slot 0 historique) — déterministe lockstep, mais sans
//! l'indépendance stricte par joueur des héros (3b) ; (3) `state_hash` ne
//! couvre que la racine (world.rs gelé) — les mercs coop sont couverts par
//! les tests double-exécution et le round-trip save bit-exact.
//!
//! Zéro IO, zéro rendu : pareil que `vendor.rs` (tables embarquées
//! compile-time), état runtime porté par `World` (blocs `impl World`
//! additifs ici, comme `waypoints.rs`).

use std::sync::OnceLock;

use crate::difficulty::Difficulty;
use crate::{AnimState, Entity, GameEvent};
use d2missiles::MissileSystem;
use serde::{Deserialize, Serialize};

/// Table réelle v1.12 `hireling.txt` (120 records, patch_d2.mpq du
/// 2026-10-23, sha256 c743a6489456bd2c…8f54a555afa78) — embarquée
/// compile-time, zéro IO runtime (même pattern que `vendor.rs`).
pub(crate) const HIRELING_TXT: &str = include_str!("../../../../assets/patch_hireling.txt");

// ---------------------------------------------------------------------------
// Constantes gameplay (décisions MVP documentées — cf. module docs)
// ---------------------------------------------------------------------------

/// Rayon de suivi du héros (tuiles) — au-delà, le mercenaire re-path.
pub(crate) const MERC_FOLLOW_RADIUS: f32 = 3.5;
/// Rayon d'aggro propre du mercenaire (tuiles) : cible de repli = monstre
/// vivant le plus proche de LUI.
pub(crate) const MERC_AGGRO_RADIUS: f32 = 8.0;
/// Distance d'engagement du tir (tuiles) — au-delà, il s'approche.
pub(crate) const MERC_SHOOT_RANGE: f32 = 10.0;
/// Cooldown de swing mêlée/tir (ms) — aligné sur `HERO_SWING_MS`.
pub(crate) const MERC_SWING_MS: u32 = 800;
/// Re-path périodique (ms) — aligné sur `REPATH_MS`.
pub(crate) const MERC_REPATH_MS: u32 = 400;
/// Plafond de niveau d'un hireling (règle D2 v1.10+ ; absent de la table).
pub(crate) const MERC_MAX_LEVEL: u32 = 98;
/// Graine XOR du système de missiles du mercenaire (indépendant du héros —
/// aucune interference avec `World.missiles`, pas de champ ajouté à
/// d2missiles).
pub(crate) const MERC_MISSILE_SEED_XOR: u64 = 0x4D45_5243_0000_0001;

// ---------------------------------------------------------------------------
// Types (colonne `Hireling`)
// ---------------------------------------------------------------------------

/// Les 4 types de mercenaires v1.12 (`hireling.txt` col 1 `Hireling`,
/// acte col 6 `Act` : 1/2/3/5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MercKind {
    /// « Rogue Scout » — acte 1 (Kashya, col `Seller`=150). Arc.
    RogueScout,
    /// « Desert Mercenary » — acte 2 (Greiz, col `Seller`=198). Perche.
    DesertMercenary,
    /// « Eastern Sorceror » — acte 3 (Asheara, col `Seller`=252). Sorts.
    EasternSorceror,
    /// « Barbarian » — acte 5 (Qual-Kehk, col `Seller`=515). Épée 1 main.
    Barbarian,
}

impl MercKind {
    /// Les 4, dans l'ordre des actes (1/2/3/5).
    pub const ALL: [MercKind; 4] = [
        MercKind::RogueScout,
        MercKind::DesertMercenary,
        MercKind::EasternSorceror,
        MercKind::Barbarian,
    ];

    /// Libellé exact de la colonne `Hireling` (table = foi).
    pub fn as_str(self) -> &'static str {
        match self {
            MercKind::RogueScout => "Rogue Scout",
            MercKind::DesertMercenary => "Desert Mercenary",
            MercKind::EasternSorceror => "Eastern Sorceror",
            MercKind::Barbarian => "Barbarian",
        }
    }

    /// Acte de recrutement (colonne `Act` — constante par type dans la table).
    pub fn act(self) -> u8 {
        match self {
            MercKind::RogueScout => 1,
            MercKind::DesertMercenary => 2,
            MercKind::EasternSorceror => 3,
            MercKind::Barbarian => 5,
        }
    }

    /// NPC vendeur (colonne `Seller` = `Id` de `patch_monstats.txt` :
    /// kashya/greiz/asheara/qual-kehk).
    pub fn seller_monstats_id(self) -> u32 {
        match self {
            MercKind::RogueScout => 150,
            MercKind::DesertMercenary => 198,
            MercKind::EasternSorceror => 252,
            MercKind::Barbarian => 515,
        }
    }

    /// Parse depuis la chaîne de table (insensible à la casse) ou un alias
    /// court ("rogue", "a1", "desert", "a2", "sorceror", "a3", "barb", "a5").
    pub fn parse(s: &str) -> Option<MercKind> {
        let up = s.trim().to_ascii_lowercase();
        MercKind::ALL.into_iter().find(|k| {
            k.as_str().eq_ignore_ascii_case(&up)
                || match k {
                    MercKind::RogueScout => matches!(up.as_str(), "rogue" | "a1" | "1"),
                    MercKind::DesertMercenary => matches!(up.as_str(), "desert" | "a2" | "2"),
                    MercKind::EasternSorceror => {
                        matches!(up.as_str(), "sorceror" | "sorcerer" | "a3" | "3")
                    }
                    MercKind::Barbarian => matches!(up.as_str(), "barb" | "barbarian" | "a5" | "5"),
                }
        })
    }
}

/// Mode d'attaque effectif du type (dérivé des colonnes `WType1`/`SubType` —
/// cf. docs du module).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MercAttack {
    /// Mêlée (Desert Mercenary `WType1=pole`, Barbarian `WType1=swor`).
    Melee,
    /// Distance — flèche porteuse d'ARME du mercenaire
    /// (Rogue Scout : `firearrow`/`icearrow`, carriers `Weapon`).
    Arrow,
    /// Distance — missile de SORT de la table d2missiles (Eastern Sorceror :
    /// `firebolt`/`icebolt`/`chargedbolt`, carriers `Skill`, dégâts de table).
    Spell,
}

// ---------------------------------------------------------------------------
// Record de table
// ---------------------------------------------------------------------------

/// Un record `hireling.txt` (une ligne = un type × sous-type × difficulté ×
/// palier de niveau). Les indices de colonnes sont résolus par le NOM
/// d'en-tête (la table fait foi, pas la position).
#[derive(Debug, Clone)]
pub struct MercRec {
    /// Col 1 `Hireling` — le 4 types.
    pub kind: MercKind,
    /// Col 2 `SubType` — « Fire - Normal », « Comb-Normal », « 1hs-Hell »…
    pub subtype: String,
    /// Col 7 `Difficulty` — 1/2/3 = Normal/Nightmare/Hell (index
    /// [`Difficulty::index`]).
    pub diff: usize,
    /// Col 8 `Level` — niveau de base du record (palier d'embauche).
    pub level: u32,
    /// Col 9 `Seller` — `Id` monstats du NPC vendeur.
    pub seller: u32,
    /// Col 12 `Gold` — coût d'embauche (or).
    pub gold: u32,
    /// Col 13 `Exp/Lvl` — palier d'XP par niveau (MVP constant).
    pub exp_per_lvl: u32,
    /// Col 14 `HP` — PV au niveau `level`.
    pub hp: u32,
    /// Col 15 `HP/Lvl`.
    pub hp_per_lvl: u32,
    /// Col 16 `Defense`.
    pub defense: u32,
    /// Col 17 `Def/Lvl`.
    pub def_per_lvl: u32,
    /// Col 18 `Str`.
    pub strength: u32,
    /// Col 19 `Str/Lvl`.
    pub str_per_lvl: u32,
    /// Col 20 `Dex`.
    pub dexterity: u32,
    /// Col 21 `Dex/Lvl`.
    pub dex_per_lvl: u32,
    /// Col 22 `AR` (attack rating).
    pub ar: u32,
    /// Col 23 `AR/Lvl`.
    pub ar_per_lvl: u32,
    /// Col 25 `Dmg-Min`.
    pub dmg_min: u32,
    /// Col 26 `Dmg-Max`.
    pub dmg_max: u32,
    /// Col 27 `Dmg/Lvl` — ajouté au min ET au max par niveau.
    pub dmg_per_lvl: u32,
    /// Col 28 `Resist` — toutes résistances (MVP : exposé ; aucune source
    /// élémentaire ennemie au port — les monstres ne tirent pas).
    pub resist: u32,
    /// Col 29 `Resist/Lvl`.
    pub resist_per_lvl: u32,
    /// Col 30 `WType1` — type d'arme (`bow`/`pole`/`swor`).
    pub wtype: String,
    /// Col 33 `DefaultChance` — poids du sous-type par défaut.
    pub default_chance: u32,
    /// Colonnes 34/40/46/52/58/64 `Skill1..Skill6` non vides (libellés).
    pub skills: Vec<String>,
}

impl MercRec {
    /// HP effectifs au niveau `lvl` (≥ `Level`) : `HP + (lvl-Level)×HP/Lvl`.
    pub fn hp_at(&self, lvl: u32) -> u32 {
        self.hp.saturating_add(lvl.saturating_sub(self.level).saturating_mul(self.hp_per_lvl))
    }
    /// Défense effective au niveau `lvl` (`Defense + (lvl-Level)×Def/Lvl`).
    pub fn defense_at(&self, lvl: u32) -> u32 {
        self.defense
            .saturating_add(lvl.saturating_sub(self.level).saturating_mul(self.def_per_lvl))
    }
    /// Toucher effectif au niveau `lvl` (`AR + (lvl-Level)×AR/Lvl`).
    pub fn ar_at(&self, lvl: u32) -> u32 {
        self.ar.saturating_add(lvl.saturating_sub(self.level).saturating_mul(self.ar_per_lvl))
    }
    /// Dégâts (min, max) au niveau `lvl` (`Dmg-Min/Max + (lvl-Level)×Dmg/Lvl`).
    pub fn dmg_at(&self, lvl: u32) -> (u32, u32) {
        let n = lvl.saturating_sub(self.level).saturating_mul(self.dmg_per_lvl);
        (self.dmg_min.saturating_add(n), self.dmg_max.saturating_add(n))
    }
    /// Résistances (toutes) au niveau `lvl` (`Resist + (lvl-Level)×Resist/Lvl`).
    pub fn resist_at(&self, lvl: u32) -> u32 {
        self.resist
            .saturating_add(lvl.saturating_sub(self.level).saturating_mul(self.resist_per_lvl))
    }
    /// Force au niveau `lvl` (col `Str` + `Str/Lvl`).
    pub fn strength_at(&self, lvl: u32) -> u32 {
        self.strength
            .saturating_add(lvl.saturating_sub(self.level).saturating_mul(self.str_per_lvl))
    }
    /// Dextérité au niveau `lvl` (col `Dex` + `Dex/Lvl`).
    pub fn dexterity_at(&self, lvl: u32) -> u32 {
        self.dexterity
            .saturating_add(lvl.saturating_sub(self.level).saturating_mul(self.dex_per_lvl))
    }

    /// Mode d'attaque + code missile éventuel, dérivés de `WType1`/`SubType`
    /// (cf. docs du module pour les correspondances et écarts MVP).
    pub fn attack(&self) -> (MercAttack, Option<&'static str>) {
        match self.kind {
            MercKind::DesertMercenary | MercKind::Barbarian => (MercAttack::Melee, None),
            MercKind::RogueScout => {
                if self.subtype.starts_with("Fire") {
                    (MercAttack::Arrow, Some("firearrow"))
                } else if self.subtype.starts_with("Ice") {
                    (MercAttack::Arrow, Some("icearrow"))
                } else {
                    (MercAttack::Arrow, Some("arrow"))
                }
            }
            MercKind::EasternSorceror => {
                if self.subtype.starts_with("Fire") {
                    (MercAttack::Spell, Some("firebolt"))
                } else if self.subtype.starts_with("Cold") {
                    (MercAttack::Spell, Some("icebolt"))
                } else {
                    (MercAttack::Spell, Some("chargedbolt"))
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Table embarquée (parse une fois)
// ---------------------------------------------------------------------------

/// Table parsée (une fois par processus — `OnceLock`, zéro coût au tick).
pub(crate) fn table() -> &'static [MercRec] {
    static TABLE: OnceLock<Vec<MercRec>> = OnceLock::new();
    TABLE.get_or_init(|| parse_table(HIRELING_TXT))
}

/// Parseur `hireling.txt` : en-tête → index de colonnes par NOM, puis un
/// [`MercRec`] par ligne. Les lignes d'en-tête vides/tronquées sont ignorées
/// (robustesse tables D2) ; une ligne sans type connu est ignorée.
pub(crate) fn parse_table(txt: &str) -> Vec<MercRec> {
    let mut rows = txt.lines().filter(|l| !l.trim().is_empty());
    let Some(head) = rows.next() else { return Vec::new() };
    let cols: Vec<String> = head
        .split('\t')
        .map(|c| c.trim().trim_matches('"').to_string())
        .collect();
    let at = |row: &[&str], name: &str| -> String {
        cols.iter()
            .position(|c| c == name)
            .and_then(|i| row.get(i))
            .map(|v| v.trim().trim_matches('"').to_string())
            .unwrap_or_default()
    };
    let num = |row: &[&str], name: &str| -> u32 {
        at(row, name).parse::<u32>().unwrap_or(0)
    };
    rows.map(|l| l.split('\t').map(|c| c.trim().trim_matches('"')).collect::<Vec<&str>>())
        .filter_map(|row| {
            let kind = MercKind::parse(&at(&row, "Hireling"))?;
            let skills = ["Skill1", "Skill2", "Skill3", "Skill4", "Skill5", "Skill6"]
                .iter()
                .map(|s| at(&row, s))
                .filter(|s| !s.is_empty())
                .collect();
            Some(MercRec {
                kind,
                subtype: at(&row, "SubType"),
                diff: num(&row, "Difficulty").clamp(1, 3) as usize - 1,
                level: num(&row, "Level").max(1),
                seller: num(&row, "Seller"),
                gold: num(&row, "Gold"),
                exp_per_lvl: num(&row, "Exp/Lvl"),
                hp: num(&row, "HP"),
                hp_per_lvl: num(&row, "HP/Lvl"),
                defense: num(&row, "Defense"),
                def_per_lvl: num(&row, "Def/Lvl"),
                strength: num(&row, "Str"),
                str_per_lvl: num(&row, "Str/Lvl"),
                dexterity: num(&row, "Dex"),
                dex_per_lvl: num(&row, "Dex/Lvl"),
                ar: num(&row, "AR"),
                ar_per_lvl: num(&row, "AR/Lvl"),
                dmg_min: num(&row, "Dmg-Min"),
                dmg_max: num(&row, "Dmg-Max"),
                dmg_per_lvl: num(&row, "Dmg/Lvl"),
                resist: num(&row, "Resist"),
                resist_per_lvl: num(&row, "Resist/Lvl"),
                wtype: at(&row, "WType1"),
                default_chance: num(&row, "DefaultChance"),
                skills,
            })
        })
        .collect()
}

/// Records d'un (type, difficulté) — ordre du fichier (déterministe).
/// Les références pointent la table `OnceLock` (vraiment `'static`,
/// aucune allocation conservée).
pub fn records_for(kind: MercKind, diff: Difficulty) -> Vec<&'static MercRec> {
    let d = diff.index();
    table().iter().filter(|r| r.kind == kind && r.diff == d).collect()
}

/// Record par (type, sous-type, difficulté) — lookup save/load.
pub fn record_of(kind: MercKind, subtype: &str, diff: Difficulty) -> Option<&'static MercRec> {
    table().iter().find(|r| {
        r.kind == kind && r.diff == diff.index() && r.subtype.eq_ignore_ascii_case(subtype)
    })
}

/// Sous-type PAR DÉFAUT du type à la difficulté donnée : `DefaultChance`
/// maximal, égalité ⇒ premier record du fichier (déterminisme).
pub fn default_subtype(kind: MercKind, diff: Difficulty) -> Option<&'static MercRec> {
    // Départage déterministe : PREMIER du fichier en cas d'égalité
    // (v1.12 réel : tous les sous-types Rogue partagent DefaultCh=75 —
    // la même convention que l'offre Desert Comb/Off/Def à 30).
    records_for(kind, diff).into_iter().fold(None, |acc, r| match acc {
        Some(a) if a.default_chance >= r.default_chance => Some(a),
        _ => Some(r),
    })
}

/// Record OFFERT au niveau du héros : palier `Level` le plus haut
/// ≤ `hero_level`, sinon le palier le plus bas (D2 : on embauche au palier
/// atteint). `subtype` vide/None = sous-type par défaut.
pub fn offer_record(
    kind: MercKind,
    subtype: Option<&str>,
    diff: Difficulty,
    hero_level: u32,
) -> Option<&'static MercRec> {
    let mut rs = records_for(kind, diff);
    if let Some(s) = subtype {
        if !s.trim().is_empty() {
            rs.retain(|r| r.subtype.eq_ignore_ascii_case(s));
        }
    }
    let best = rs.iter().copied().filter(|r| r.level <= hero_level).max_by_key(|r| r.level);
    best.or_else(|| rs.iter().copied().min_by_key(|r| r.level))
}

// ---------------------------------------------------------------------------
// État runtime (porté par World)
// ---------------------------------------------------------------------------

/// Échec d'embauche (API [`World::hire_merc`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HireError {
    /// Un mercenaire VIVANT existe déjà — règle D2 : un seul hireling à la
    /// fois (embauche possible seulement sans mercenaire ou KO).
    AlreadyAlive,
    /// Or insuffisant (coût = colonne `Gold` du record offert).
    InsufficientGold { cost: u32, have: u32 },
    /// Aucun record de table pour ce type (impossible avec la table embarquée
    /// — garde pour une future table personnalisée).
    NoHireling,
    /// 3c-a : slot hors monde (≥ `hero_count`) — pas de joueur à qui
    /// adosser le mercenaire.
    NoSlot,
}

/// Échec de résurrection ([`World::resurrect_merc`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResurrectError {
    /// Aucun mercenaire KO à ressusciter (aucun, ou déjà vivant).
    NoCorpse,
}

/// Mercenaire runtime : type/sous-type fixés à l'embauche, stats recalculées
/// depuis la table à chaque niveau (pas de double source), état de combat
/// (entité + FSM légère + missiles propres).
pub struct MercData {
    /// Type (colonne `Hireling`).
    pub kind: MercKind,
    /// Sous-type (colonne `SubType`, tel qu'embouché).
    pub subtype: String,
    /// Niveau de base du record d'embauche (référence du scaling /Lvl).
    pub(crate) base_level: u32,
    /// Niveau courant (démarre au palier offert).
    pub level: u32,
    /// XP cumulés depuis l'embauche.
    pub xp: u32,
    /// XP restants avant le prochain niveau (palier = colonne `Exp/Lvl`).
    pub xp_to_next: u32,
    /// PV courants.
    pub hp: u32,
    /// PV max effectifs (scaling `HP/Lvl`).
    pub max_hp: u32,
    /// Défense effective (`Def/Lvl`).
    pub defense: u32,
    /// Toucher effectif (`AR/Lvl`).
    pub ar: u32,
    /// Dégâts effectifs min/max (`Dmg/Lvl`) — mêlée ET flèche (arme).
    pub dmg_min: u32,
    pub dmg_max: u32,
    /// Résistances (toutes, col `Resist`+`Resist/Lvl`) — exposées (MVP :
    /// aucune source élémentaire ennemie au port).
    pub resist: u32,
    /// Coût d'embauche payé (colonne `Gold` — audit/affichage).
    pub hire_cost: u32,
    /// Entité (position, facing, anim).
    pub ent: Entity,
    /// Vivant ? `false` = KO (cadavre, résurrectible).
    pub alive: bool,
    /// Chemin courant (follow/approach).
    pub(crate) path: Vec<(f32, f32)>,
    /// Compteur re-path.
    pub(crate) repath_ms: u32,
    /// Cooldown de swing (ms).
    pub(crate) swing_ms: u32,
    /// Horloge d'animation.
    pub(crate) anim_ms: u32,
    /// Missiles PROPRES au mercenaire (flèches rogue / sorts sorceror) —
    /// système séparé de celui du héros : zéro changement dans d2missiles,
    /// collisions et jets résolus ici avec l'AR du mercenaire.
    pub(crate) missiles: MissileSystem,
    /// 3c-a : slot propriétaire (index dans `World.players`). 0 = mercenaire
    /// du héros historique (racine) ; ≥ 1 = mercenaire coop (porté par la
    /// racine en `coop`) ; `usize::MAX` = COQUILLE porte-état coop (aucun
    /// mercenaire slot 0 — invisible : `mercenary()` → None, jamais tickée).
    pub(crate) slot: usize,
    /// 3c-a : état coop embarqué (mercs des slots ≥ 1 + corps par slot) —
    /// significatif UNIQUEMENT sur la racine ; vide sur un merc coop.
    /// Cf. `players_coop.rs` (world.rs gelé : aucun champ World disponible).
    pub(crate) coop: crate::players_coop::CoopState,
}

impl MercData {
    /// Construit depuis un record offert (embauche) — position posée par
    /// l'appelant ([`World::hire_merc`] la snap près du héros).
    pub(crate) fn from_rec(rec: &MercRec, rng_seed: u64, slot: usize) -> MercData {
        let mut m = MercData {
            kind: rec.kind,
            subtype: rec.subtype.clone(),
            base_level: rec.level,
            level: rec.level,
            xp: 0,
            xp_to_next: rec.exp_per_lvl,
            hp: 0,
            max_hp: 0,
            defense: 0,
            ar: 0,
            dmg_min: 0,
            dmg_max: 0,
            resist: 0,
            hire_cost: rec.gold,
            ent: Entity { x: 0.0, y: 0.0, facing: 0, anim: AnimState::NU, anim_frame: 0 },
            alive: true,
            path: Vec::new(),
            repath_ms: 0,
            swing_ms: 0,
            anim_ms: 0,
            missiles: MissileSystem::with_seed(rng_seed ^ MERC_MISSILE_SEED_XOR),
            slot,
            coop: crate::players_coop::CoopState::default(),
        };
        m.recompute(rec);
        m.hp = m.max_hp;
        m
    }

    /// 3c-a : COQUILLE porte-état coop — un « mercenaire » factice (jamais
    /// tickée, jamais visible, jamais résolue comme cible) qui porte l'état
    /// des slots ≥ 1 quand le slot 0 n'a pas de mercenaire. Reconstruite au
    /// rechargement SANS lookup de table (stable quelle que soit la
    /// difficulté sauvegardée).
    pub(crate) fn shell() -> MercData {
        MercData::shell_with(crate::players_coop::CoopState::default())
    }

    /// Coquille avec état coop pré-rempli (rechargement de save).
    pub(crate) fn shell_with(coop: crate::players_coop::CoopState) -> MercData {
        let rec = offer_record(MercKind::RogueScout, None, Difficulty::Normal, 1)
            .expect("record rogue (coquille)");
        let mut m = MercData::from_rec(rec, 0, usize::MAX);
        m.coop = coop;
        m.alive = false;
        m.hp = 0;
        m.path.clear();
        m
    }

    /// Record de table courant (par type/sous-type/difficulté).
    pub(crate) fn rec(&self, diff: Difficulty) -> Option<&'static MercRec> {
        record_of(self.kind, &self.subtype, diff)
    }

    /// Recalcule les stats effectives depuis la table au niveau courant.
    pub(crate) fn recompute(&mut self, rec: &MercRec) {
        self.max_hp = rec.hp_at(self.level).max(1);
        self.defense = rec.defense_at(self.level);
        self.ar = rec.ar_at(self.level);
        let (dmin, dmax) = rec.dmg_at(self.level);
        self.dmg_min = dmin.max(1);
        self.dmg_max = dmax.max(dmin.max(1));
        self.resist = rec.resist_at(self.level);
        self.xp_to_next = rec.exp_per_lvl;
    }

    /// Mode d'attaque + code missile éventuel (dérivé du record).
    pub(crate) fn attack(&self, diff: Difficulty) -> (MercAttack, Option<&'static str>) {
        self.rec(diff).map(|r| r.attack()).unwrap_or((MercAttack::Melee, None))
    }

    /// XP d'un kill porté par le mercenaire + level-ups (plafond 98).
    /// Renvoie true si au moins un niveau gagné.
    pub(crate) fn gain_xp(&mut self, rec: &MercRec, xp: u32, ev: &mut Vec<GameEvent>) -> bool {
        self.xp = self.xp.saturating_add(xp);
        let mut leveled = false;
        while self.level < MERC_MAX_LEVEL && self.xp >= self.xp_to_next && self.xp_to_next > 0 {
            self.xp = self.xp.saturating_sub(self.xp_to_next);
            self.level += 1;
            let old_max = self.max_hp;
            self.recompute(rec);
            // level-up : soigne le gain de PV max (comme le héros)
            self.hp += self.max_hp - old_max;
            ev.push(GameEvent::MercLeveled { level: self.level });
            leveled = true;
        }
        if self.level >= MERC_MAX_LEVEL {
            self.xp_to_next = 0;
        }
        leveled
    }
}

// ---------------------------------------------------------------------------
// Sauvegarde (champ ADDITIF `mercenary` — save v3 inchangée ailleurs)
// ---------------------------------------------------------------------------

/// État sérialisable du mercenaire (save additive `#[serde(default)]`).
/// Les stats effectives NE sont PAS sauvegardées : recalculées au chargement
/// depuis la table (type/sous-type/niveau = source unique).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MercSave {
    /// Type (colonne `Hireling`).
    pub kind: MercKind,
    /// Sous-type (colonne `SubType`).
    pub subtype: String,
    /// Niveau de base du record d'embauche (scaling /Lvl).
    pub base_level: u32,
    /// Niveau courant.
    pub level: u32,
    /// XP cumulés.
    pub xp: u32,
    /// PV courants (clampés au max recalculé).
    pub hp: u32,
    /// Vivant ? (`false` = KO résurrectible).
    pub alive: bool,
    /// Entité.
    pub ent: Entity,
    /// Chemin en vol.
    pub path: Vec<(f32, f32)>,
    /// Cooldown de swing.
    pub swing_ms: u32,
    /// Horloge d'animation.
    pub anim_ms: u32,
    /// Missiles du mercenaire en vol.
    pub missiles: MissileSystem,
    /// 3c-a : slot propriétaire (0 = héros historique ; `usize::MAX` =
    /// coquille porte-état coop). `#[serde(default)]` : absent des saves
    /// antérieures à la 3c ⇒ 0 (rétro-compat totale).
    #[serde(default)]
    pub slot: usize,
    /// 3c-a : état coop embarqué (mercs slots ≥ 1 + corps par slot, cf.
    /// `players_coop.rs`). `#[serde(default)]` : absent des saves
    /// antérieures ⇒ vide.
    #[serde(default)]
    pub coop: crate::players_coop::CoopSave,
}

impl MercSave {
    /// Capture l'état runtime.
    pub fn of(m: &MercData) -> MercSave {
        MercSave {
            kind: m.kind,
            subtype: m.subtype.clone(),
            base_level: m.base_level,
            level: m.level,
            xp: m.xp,
            hp: m.hp,
            alive: m.alive,
            ent: m.ent,
            path: m.path.clone(),
            swing_ms: m.swing_ms,
            anim_ms: m.anim_ms,
            missiles: m.missiles.clone(),
            slot: m.slot,
            coop: crate::players_coop::CoopSave::of(&m.coop),
        }
    }

    /// Reconstruit l'état runtime depuis la save + la table (stats
    /// recalculées au niveau courant ; hp clampé). `Err(())` = record
    /// introuvable dans la table (save d'une autre version de données) —
    /// l'appelant IGNORE alors le mercenaire (jamais un save invalide).
    /// La COQUILLE (`slot == usize::MAX`) se reconstruit SANS lookup de
    /// table (stable quelle que soit la difficulté — cf. `MercData::shell`).
    pub(crate) fn rebuild(self, diff: Difficulty) -> Result<MercData, ()> {
        if self.slot == usize::MAX {
            return Ok(MercData::shell_with(self.coop.rebuild(diff)));
        }
        let rec = record_of(self.kind, &self.subtype, diff).ok_or(())?;
        let mut m = MercData {
            kind: self.kind,
            subtype: self.subtype,
            base_level: self.base_level.max(rec.level),
            level: self.level.max(rec.level).min(MERC_MAX_LEVEL),
            xp: self.xp,
            xp_to_next: rec.exp_per_lvl,
            hp: self.hp,
            max_hp: 0,
            defense: 0,
            ar: 0,
            dmg_min: 0,
            dmg_max: 0,
            resist: 0,
            hire_cost: rec.gold,
            ent: self.ent,
            alive: self.alive,
            path: self.path,
            repath_ms: 0,
            swing_ms: self.swing_ms,
            anim_ms: self.anim_ms,
            missiles: self.missiles,
            slot: self.slot,
            coop: self.coop.rebuild(diff),
        };
        m.recompute(rec);
        m.hp = m.hp.min(m.max_hp).max(if m.alive { 1 } else { 0 });
        Ok(m)
    }
}

// ---------------------------------------------------------------------------
// Blocs `impl World` additifs (même pattern que waypoints.rs)
// ---------------------------------------------------------------------------

use crate::world::{dist, hit_chance, World, ATTACK_RANGE, MAX_DT_MS};

impl World {
    /// Embauche (`H`) : `hire_merc(type)` — le sous-type par défaut
    /// (`DefaultChance` max) au palier offert par le niveau du héros, à la
    /// difficulté courante. Coût = colonne `Gold` (débité de l'or du héros).
    ///
    /// RÈGLE D2 : un seul mercenaire à la fois — `Err(AlreadyAlive)` tant que
    /// le courant est vivant ; s'il est KO, le NOUVEAU LE REMPLACE
    /// (l'ancien disparaît définitivement, règle officielle D2).
    /// Le mercenaire apparaît sur une tuile marchable près du héros.
    /// L'événement `MercHired` est livré au TICK SUIVANT (file différée —
    /// l'API renvoie déjà le niveau embauché). Renvoie `Ok(niveau embauché)`
    /// ou l'erreur (état inchangé).
    /// 3c-a : mercenaire du SLOT 0 — délègue à [`World::hire_merc_slot`].
    pub fn hire_merc(&mut self, kind: MercKind) -> Result<u32, HireError> {
        self.hire_merc_slot(0, kind)
    }

    /// 3c-a : embauche PAR JOUEUR — le slot `slot` embauche SON mercenaire
    /// (type indépendant, palier offert par SON niveau, or débité de SA
    /// bourse) ; le merc suit SON joueur, meurt et ressuscite chez le NPC de
    /// sa ville ([`World::resurrect_merc_slot`]). Règle D2 par slot : un
    /// seul hireling vivant par joueur ; KO ⇒ le nouveau remplace.
    /// Graine missiles : slot 0 = flux MONDE (bit-exact) ; slots ≥ 1 = flux
    /// RNG du joueur (aucun tirage consommé dans les deux cas). L'événement
    /// `MercHired` est commun (pas de slot — file différée, cf. docs module).
    pub fn hire_merc_slot(&mut self, slot: usize, kind: MercKind) -> Result<u32, HireError> {
        if slot >= self.players.len() {
            return Err(HireError::NoSlot);
        }
        if self.mercenary_slot(slot).map(|m| m.alive).unwrap_or(false) {
            return Err(HireError::AlreadyAlive);
        }
        let rec = match offer_record(kind, None, self.difficulty, self.players[slot].data.stats.level)
        {
            Some(r) => r,
            None => return Err(HireError::NoHireling),
        };
        let cost = rec.gold;
        if self.players[slot].data.stats.gold < cost {
            return Err(HireError::InsufficientGold {
                cost,
                have: self.players[slot].data.stats.gold,
            });
        }
        self.players[slot].data.stats.gold -= cost;
        let seed = if slot == 0 {
            self.rng.s
        } else {
            self.players[slot].rng.s ^ (MERC_MISSILE_SEED_XOR.wrapping_mul(slot as u64 + 1))
        };
        let mut m = MercData::from_rec(rec, seed, slot);
        // apparition : tuile marchable près de SON joueur (spirale 2, sinon
        // sur lui)
        let (hx, hy) = (self.players[slot].ent.x, self.players[slot].ent.y);
        let (tx, ty) = (hx.floor() as i32, hy.floor() as i32);
        let g = crate::path::Grid { w: self.w, h: self.h, blocked: &self.grid };
        let spot = crate::path::snap_near(&g, (tx, ty), 2).unwrap_or((tx, ty));
        m.ent.x = spot.0 as f32 + 0.5;
        m.ent.y = spot.1 as f32 + 0.5;
        m.ent.facing = crate::world::facing_from(hx - m.ent.x, hy - m.ent.y);
        let level = m.level;
        let ev = GameEvent::MercHired {
            kind: m.kind.as_str().to_string(),
            subtype: m.subtype.clone(),
            level,
            cost,
        };
        if slot == 0 {
            // la racine DEVIENT le merc du slot 0 — l'état coop embarqué suit
            let mut old = self.merc.take();
            if let Some(o) = old.as_mut() {
                m.coop = std::mem::take(&mut o.coop);
            }
            self.merc = Some(m);
            self.merc_target = None;
        } else {
            let root = crate::players_coop::ensure_root(self);
            root.coop.mercs.retain(|mm| mm.slot != slot); // KO remplacé
            root.coop.mercs.push(m);
        }
        self.deferred.push(ev);
        Ok(level)
    }

    /// Résurrection du mercenaire KO chez le NPC de son type (H). PLEINE VIE,
    /// à côté du héros. COÛT MVP 0 : la table v1.12 ne porte pas de colonne
    /// de résurrection (formule client par niveau — restant) ; la valeur
    /// renvoyée est ce coût (0 aujourd'hui). L'événement `MercResurrected`
    /// est livré au tick suivant (file différée).
    /// 3c-a : mercenaire du SLOT 0 — délègue à [`World::resurrect_merc_slot`].
    pub fn resurrect_merc(&mut self) -> Result<u32, ResurrectError> {
        self.resurrect_merc_slot(0)
    }

    /// 3c-a : résurrection PAR JOUEUR — le merc KO du slot `slot` revient
    /// pleine vie auprès de SON joueur (la résurrection se fait en ville,
    /// chez le NPC vendeur de SON type : kashya/greiz/asheara/qual-kehk,
    /// col `Seller` — l'UI d2game branche le clic NPC sur cette API).
    pub fn resurrect_merc_slot(&mut self, slot: usize) -> Result<u32, ResurrectError> {
        if self.mercenary_slot(slot).map(|m| m.alive).unwrap_or(true) {
            return Err(ResurrectError::NoCorpse);
        }
        // ramène le cadavre près de SON joueur (la résurrection se fait en
        // ville) — position calculée AVANT l'emprunt mutable du mercenaire
        let (hx, hy) = (self.players[slot].ent.x, self.players[slot].ent.y);
        let g = crate::path::Grid { w: self.w, h: self.h, blocked: &self.grid };
        let spot = crate::path::snap_near(
            &g,
            (hx.floor() as i32, hy.floor() as i32),
            2,
        )
        .unwrap_or((hx.floor() as i32, hy.floor() as i32));
        let m = self.mercenary_slot_mut(slot).expect("merc KO connu");
        m.alive = true;
        m.hp = m.max_hp;
        m.path.clear();
        m.swing_ms = 0;
        m.repath_ms = 0;
        m.ent.anim = AnimState::NU;
        m.ent.anim_frame = 0;
        m.anim_ms = 0;
        m.ent.x = spot.0 as f32 + 0.5;
        m.ent.y = spot.1 as f32 + 0.5;
        let cost = 0; // colonne absente de hireling.txt v1.12 (MVP gratuit)
        self.deferred.push(GameEvent::MercResurrected { cost });
        Ok(cost)
    }

    /// Mercenaire courant (lecture) — état complet. 3c-a : vue du SLOT 0
    /// (la coquille porte-état n'est jamais visible : `None`).
    pub fn mercenary(&self) -> Option<&MercData> {
        self.mercenary_slot(0)
    }

    /// 3c-a : mercenaire du slot `slot` (lecture) — 0 = racine historique,
    /// ≥ 1 = merc coop. `None` = pas de merc pour ce slot (ou coquille).
    pub fn mercenary_slot(&self, slot: usize) -> Option<&MercData> {
        match slot {
            0 => self.merc.as_ref().filter(|m| m.slot == 0),
            s => self.merc.as_ref().and_then(|r| r.coop.merc(s)),
        }
    }

    /// Mercenaire courant (écriture — tests/chantiers parallèles, même crate).
    /// 3c-a : vue du SLOT 0 (coquille → `None`).
    pub(crate) fn mercenary_mut(&mut self) -> Option<&mut MercData> {
        self.mercenary_slot_mut(0)
    }

    /// 3c-a : mercenaire du slot (écriture).
    pub(crate) fn mercenary_slot_mut(&mut self, slot: usize) -> Option<&mut MercData> {
        match slot {
            0 => self.merc.as_mut().filter(|m| m.slot == 0),
            s => self.merc.as_mut().and_then(|r| r.coop.merc_mut(s)),
        }
    }

    /// 3c-a : slots ayant UN mercenaire (lecture UI) — 0 en tête s'il
    /// existe, puis coop triés (déterministe). La coquille n'y figure jamais.
    pub fn mercenary_slots(&self) -> Vec<usize> {
        let mut v = Vec::new();
        if let Some(r) = &self.merc {
            if r.slot == 0 {
                v.push(0);
            }
            let mut coop: Vec<usize> = r.coop.mercs.iter().map(|m| m.slot).collect();
            coop.sort_unstable();
            v.extend(coop);
        }
        v
    }

    /// Stats du héros (écriture — tests : doter l'or d'embauche).
    pub(crate) fn hero_stats_mut(&mut self) -> &mut crate::HeroStats {
        &mut self.players[0].data.stats
    }

    /// Entité du mercenaire (lecture — rendu d2game). 3c-a : vue du SLOT 0.
    pub fn merc_entity(&self) -> Option<&Entity> {
        self.mercenary_slot(0).map(|m| &m.ent)
    }

    /// 3c-a : entité du mercenaire du slot (rendu des mercs coop).
    pub fn merc_entity_slot(&self, slot: usize) -> Option<&Entity> {
        self.mercenary_slot(slot).map(|m| &m.ent)
    }

    /// Missiles du mercenaire en vol (lecture — rendu d2game, flèches/sorts).
    /// 3c-a : vue du SLOT 0.
    pub fn merc_missiles(&self) -> &[d2missiles::Missile] {
        self.mercenary_slot(0).map(|m| m.missiles.missiles()).unwrap_or(&[])
    }

    /// 3c-a : missiles du mercenaire du slot (rendu coop).
    pub fn merc_missiles_slot(&self, slot: usize) -> &[d2missiles::Missile] {
        self.mercenary_slot(slot).map(|m| m.missiles.missiles()).unwrap_or(&[])
    }

    /// Offre d'embauche visible (lecture — UI NPC du membre VILLES) :
    /// `(sous-type, niveau offert, coût)` pour le type demandé, à la
    /// difficulté et au niveau de héros courants. 3c-a : vue du SLOT 0.
    pub fn merc_offer(&self, kind: MercKind) -> Option<(String, u32, u32)> {
        self.merc_offer_slot(0, kind)
    }

    /// 3c-a : offre d'embauche POUR LE SLOT (palier offert par SON niveau).
    pub fn merc_offer_slot(&self, slot: usize, kind: MercKind) -> Option<(String, u32, u32)> {
        offer_record(kind, None, self.difficulty, self.players.get(slot)?.data.stats.level)
            .map(|r| (r.subtype.clone(), r.level, r.gold))
    }
}

// ---------------------------------------------------------------------------
// Tick (appelé par `World::tick`, après `update_missiles`)
// ---------------------------------------------------------------------------

/// 3c-a : FSM de TOUS les mercenaires (par joueur) — le slot 0 d'abord (flux
/// historique bit-exact), puis les mercs coop triés par slot ; et le tick
/// coop « corps par joueur » (players_coop.rs) AVANT, chaque tick, même sans
/// mercenaire (zéro coût solo).
pub(crate) fn tick_mercenary(w: &mut World, dt: u32, ev: &mut Vec<GameEvent>) {
    crate::players_coop::tick_corpses(w, dt, ev); // 3c-a : corps par slot
    let dt = dt.min(MAX_DT_MS);
    for slot in w.mercenary_slots() {
        tick_one_merc(w, slot, dt, ev);
    }
}

/// FSM d'UN mercenaire (celui du slot `slot`) : follow de SON joueur →
/// attaque de la cible de son joueur (slot 0 seulement, cf. écart 3c-a) →
/// KO. Même ordre d'opérations que le flux historique slot 0 (bit-exact).
pub(crate) fn tick_one_merc(w: &mut World, slot: usize, dt: u32, ev: &mut Vec<GameEvent>) {
    {
        let Some(m) = w.mercenary_slot_mut(slot) else { return };
        if !m.alive {
            // cadavre : joue l'anim DD une fois puis reste au sol (résurrectible)
            crate::world::set_anim(&mut m.ent, AnimState::DD, &mut m.anim_ms);
            crate::world::advance_anim(&mut m.ent, &mut m.anim_ms, dt, false);
            return;
        }
        // cooldowns
        m.swing_ms = m.swing_ms.saturating_sub(dt);
        m.repath_ms = m.repath_ms.saturating_sub(dt);
    }

    let hero_dead = w.hero_slot_dead(slot).unwrap_or(true);
    let Some((hx, hy)) = w.hero_slot_pos(slot) else { return };

    // --- décision (lectures immuables) ---
    enum Act {
        Melee(usize),
        Shoot(usize),
        Walk(f32, f32),
        Idle,
    }
    let act = {
        let Some(m) = w.mercenary_slot(slot) else { return };
        // cible : LA CIBLE DU JOUEUR propriétaire d'abord (brief H ; slot 0 =
        // `merc_target` historique — slots ≥ 1 : pas de cible par slot au
        // moteur, ÉCART documenté 3c-a : repli sur la proximité), sinon le
        // monstre vivant le plus proche DU MERCENAIRE (rayon d'aggro propre)
        let mut target: Option<usize> = if slot == 0 {
            w.merc_target
                .filter(|&i| i < w.mon_ent.len() && w.mon_dat[i].state != crate::world::MonState::Dying)
        } else {
            None
        };
        if target.is_none() && !hero_dead {
            let mut bd = MERC_AGGRO_RADIUS;
            for (i, (e, d)) in w.mon_ent.iter().zip(w.mon_dat.iter()).enumerate() {
                if d.state == crate::world::MonState::Dying {
                    continue;
                }
                let dd = dist(e.x - m.ent.x, e.y - m.ent.y);
                if dd <= bd {
                    bd = dd;
                    target = Some(i);
                }
            }
        }
        match target {
            Some(t) if !hero_dead => {
                let (tx, ty) = (w.mon_ent[t].x, w.mon_ent[t].y);
                let d = dist(tx - m.ent.x, ty - m.ent.y);
                match m.attack(w.difficulty) {
                    (MercAttack::Melee, _) => {
                        if d <= ATTACK_RANGE {
                            Act::Melee(t)
                        } else {
                            Act::Walk(tx, ty)
                        }
                    }
                    (MercAttack::Arrow, _) | (MercAttack::Spell, _) => {
                        if d <= MERC_SHOOT_RANGE {
                            Act::Shoot(t)
                        } else {
                            Act::Walk(tx, ty)
                        }
                    }
                }
            }
            _ => {
                // pas de cible : suit SON joueur (rayon de suivi)
                if dist(hx - m.ent.x, hy - m.ent.y) > MERC_FOLLOW_RADIUS {
                    Act::Walk(hx, hy)
                } else {
                    Act::Idle
                }
            }
        }
    };

    // --- application ---
    match act {
        Act::Melee(t) => merc_melee_attack_slot(w, slot, t, ev),
        Act::Shoot(t) => merc_shoot_slot(w, slot, t, ev),
        Act::Walk(wx, wy) => {
            let (mt, needs) = {
                let m = w.mercenary_slot(slot).expect("merc connu");
                (
                    (m.ent.x.floor() as i32, m.ent.y.floor() as i32),
                    m.path.is_empty() || m.repath_ms == 0,
                )
            };
            if needs {
                // re-path périodique (ou chemin vide) — A* partagé du monde
                if let Some(p) = w.path_between(mt, (wx.floor() as i32, wy.floor() as i32)) {
                    w.mercenary_slot_mut(slot).expect("merc connu").path = p;
                }
                w.mercenary_slot_mut(slot).expect("merc connu").repath_ms = MERC_REPATH_MS;
            }
        }
        Act::Idle => {
            w.mercenary_slot_mut(slot).expect("merc connu").path.clear();
        }
    }

    // --- déplacement + anim ---
    // vitesse : celle de SON joueur (le mercenaire garde le pas — MVP ; le
    // walkSpeed hireling réel viendrait de monstats, restant)
    let speed = w.hero_stats_slot(slot).map(|s| s.walk_speed).unwrap_or(1.0).max(1.0);
    let m = w.mercenary_slot_mut(slot).expect("merc connu");
    if !m.path.is_empty() {
        crate::world::advance_along_path(&mut m.ent, &mut m.path, speed, dt);
    }
    let st = if m.swing_ms > 0 {
        AnimState::A1
    } else if !m.path.is_empty() {
        AnimState::WL
    } else {
        AnimState::NU
    };
    crate::world::set_anim(&mut m.ent, st, &mut m.anim_ms);
    crate::world::advance_anim(
        &mut m.ent,
        &mut m.anim_ms,
        dt,
        matches!(st, AnimState::NU | AnimState::WL),
    );
}

/// Mêlée mercenaire : jet de toucher `200×AR/(AR+AC)` [5,95] (§3 GAME-DATA),
/// dégâts `Dmg-Min..Dmg-Max` scalés, résist physique du monstre (ResDm, W08).
fn merc_melee_attack_slot(w: &mut World, slot: usize, t: usize, ev: &mut Vec<GameEvent>) {
    let (mx, my) = (w.mon_ent[t].x, w.mon_ent[t].y);
    {
        let m = w.mercenary_slot_mut(slot).expect("merc connu");
        if m.swing_ms > 0 {
            return; // cooldown
        }
        m.ent.facing = crate::world::facing_from(mx - m.ent.x, my - m.ent.y);
        m.swing_ms = MERC_SWING_MS;
        m.anim_ms = 0;
    }
    let (ar, dmin, dmax) = {
        let m = w.mercenary_slot(slot).expect("merc connu");
        (m.ar, m.dmg_min, m.dmg_max)
    };
    let ac = w.mon_dat[t].ac;
    if (w.rng.below(100)) < hit_chance(ar, ac) {
        let dmg = w.rng.range_u32(dmin, dmax).max(1);
        let dmg = crate::difficulty::apply_resist(dmg, w.mon_dat[t].res[0]);
        w.mon_dat[t].hp = w.mon_dat[t].hp.saturating_sub(dmg);
        ev.push(GameEvent::MercAttacked { target: t, dmg });
        if w.mon_dat[t].hp == 0 {
            start_mon_death(w, t);
            merc_kill_credit_slot(w, slot, t, ev);
        }
    } else {
        ev.push(GameEvent::MercMissed { target: t });
    }
}

/// Tir mercenaire : flèche porteuse d'ARME (rogue — dégâts du mercenaire
/// résolus au tir, règle D2 « la flèche sait ») ou missile de SORT (sorceror
/// — dégâts officiels de la table d2missiles embarquée).
fn merc_shoot_slot(w: &mut World, slot: usize, t: usize, ev: &mut Vec<GameEvent>) {
    let (tx, ty) = (w.mon_ent[t].x, w.mon_ent[t].y);
    let (kind, code) = {
        let m = w.mercenary_slot(slot).expect("merc connu");
        if m.swing_ms > 0 {
            return;
        }
        m.attack(w.difficulty)
    };
    let Some(code) = code else { return };
    let m = w.mercenary_slot_mut(slot).expect("merc connu");
    m.ent.facing = crate::world::facing_from(tx - m.ent.x, ty - m.ent.y);
    m.swing_ms = MERC_SWING_MS;
    m.anim_ms = 0;
    let from = (m.ent.x, m.ent.y);
    let spawned = match kind {
        // carrier Weapon : l'appelant résout les dégâts d'arme du mercenaire
        MercAttack::Arrow => m.missiles.spawn_weapon(code, from, (tx, ty), m.dmg_min, m.dmg_max),
        // carrier Skill : la table porte les dégâts officiels niveau 1
        _ => m.missiles.spawn(code, from, (tx, ty)),
    };
    if spawned.is_some() {
        ev.push(GameEvent::MercMissileSpawned);
    }
}

/// Missiles du mercenaire : collision monstres (rayon partagé), jet de
/// toucher avec l'AR du mercenaire, résist élémentaire du monstre (W08),
/// crédit XP mercenaire sur kill. Séparé de `update_missiles` (héros) :
/// aucun changement dans d2missiles ni dans le flux héros.
/// 3c-a : chaque mercenaire (par slot) résout SES missiles, slot 0 d'abord.
pub(crate) fn tick_merc_missiles(w: &mut World, dt: u32, ev: &mut Vec<GameEvent>) {
    for slot in w.mercenary_slots() {
        tick_merc_missiles_slot(w, slot, dt, ev);
    }
}

/// Missiles du mercenaire du slot `slot` (flux historique slot 0 bit-exact).
fn tick_merc_missiles_slot(w: &mut World, slot: usize, dt: u32, ev: &mut Vec<GameEvent>) {
    // snapshot des cibles (comme update_missiles : monstres figés au tick)
    let targets: Vec<(f32, f32, bool)> = w
        .mon_ent
        .iter()
        .zip(w.mon_dat.iter())
        .map(|(e, d)| (e.x, e.y, d.state == crate::world::MonState::Dying))
        .collect();
    let (grid, gw, gh) = (&w.grid, w.w, w.h);
    let events = {
        // accès CHAMP direct (emprunts disjoints grid/merc — une fonction
        // intermédiaire re-emprunterait tout `w` et casserait le split)
        let m = match slot {
            0 => w.merc.as_mut().filter(|m| m.slot == 0),
            _ => w.merc.as_mut().and_then(|r| r.coop.merc_mut(slot)),
        };
        let Some(m) = m else { return };
        if m.missiles.is_empty() {
            return;
        }
        m.missiles.tick_walls(
            dt.min(MAX_DT_MS),
            &mut |x, y| {
                for (i, (ex, ey, dying)) in targets.iter().enumerate() {
                    if *dying {
                        continue;
                    }
                    if dist(*ex - x, *ey - y) <= crate::world::MISSILE_HIT_RADIUS {
                        return Some(i);
                    }
                }
                None
            },
            &mut |x, y| {
                let (tx, ty) = (x.floor() as i32, y.floor() as i32);
                !(tx >= 0 && ty >= 0 && tx < gw && ty < gh && !grid[ty as usize][tx as usize])
            },
        )
    };
    for me in events {
        match me {
            d2missiles::MissileEvent::Hit { id, target, dmg, .. } => {
                let (ar, elem) = {
                    let m = w.mercenary_slot(slot).expect("merc connu");
                    let elem = m
                        .missiles
                        .missiles()
                        .iter()
                        .find(|mm| mm.id == id)
                        .map(|mm| mm.elem)
                        .unwrap_or_default();
                    (m.ar, elem)
                };
                let ac = w.mon_dat[target].ac;
                if (w.rng.below(100)) < hit_chance(ar, ac) {
                    let dmg =
                        crate::difficulty::apply_resist(dmg, w.mon_dat[target].res[crate::Difficulty::resist_index(elem)]);
                    if dmg == 0 {
                        // immunité : missile consumé, rien ne passe (W08/K)
                        ev.push(GameEvent::MercMissed { target });
                        continue;
                    }
                    w.mon_dat[target].hp = w.mon_dat[target].hp.saturating_sub(dmg);
                    ev.push(GameEvent::MercAttacked { target, dmg });
                    if w.mon_dat[target].hp == 0 {
                        start_mon_death(w, target);
                        merc_kill_credit_slot(w, slot, target, ev);
                    }
                } else {
                    ev.push(GameEvent::MercMissed { target });
                }
            }
            _ => {} // Spawned/Expired/Blocked : silencieux (comme côté héros)
        }
    }
}

/// Passe le monstre `t` en anim DD (mort) — même bloc que les branches héros.
fn start_mon_death(w: &mut World, t: usize) {
    let ent = &mut w.mon_ent[t];
    let dat = &mut w.mon_dat[t];
    dat.state = crate::world::MonState::Dying;
    dat.dying_ms = 0;
    dat.anim_ms = 0;
    dat.path.clear();
    ent.anim = AnimState::DD;
    ent.anim_frame = 0;
}

/// Crédit XP du mercenaire du slot `slot` pour le kill du monstre `t` (les
/// JOUEURS reçoivent toujours leur XP partagée via `MonsterDied` →
/// `apply_xp_slot`, flux 3b inchangé).
fn merc_kill_credit_slot(w: &mut World, slot: usize, t: usize, ev: &mut Vec<GameEvent>) {
    let xp = w.mon_dat[t].xp;
    if xp == 0 {
        return;
    }
    let diff = w.difficulty;
    let rec = w.mercenary_slot(slot).and_then(|m| m.rec(diff));
    if let Some(rec) = rec {
        let m = w.mercenary_slot_mut(slot).expect("merc connu");
        m.gain_xp(rec, xp, ev);
    }
}

/// 3c-a (HOOK documenté — remplace, à sémantique identique, le bloc merc du
/// slot 0 que `world.rs::monster_strike` exécutait en ligne) : le monstre `i`
/// frappe le mercenaire le plus PROCHE parmi TOUS les slots (0 = racine,
/// ≥ 1 = coop) s'il est plus proche que le héros vivant le plus proche
/// (`hero_dist`, calculé par l'appelant). Renvoie true si le mercenaire
/// encaisse (alors AUCUN héros n'est frappé ce tick). SANS mercenaire vivant
/// plus proche : false et AUCUN tirage RNG (flux historique inchangé).
/// Égalité de distance → mercenaire : le merc spawn collé au héros,
/// l'arbitrer sur un strict < ferait du héros la cible permanente (le merc
/// ne servirait jamais d'encaisseur, son rôle D2). Égalité entre mercs →
/// slot le plus bas (déterminisme).
pub(crate) fn monster_strikes_nearest_merc(
    w: &mut World,
    i: usize,
    hero_dist: f32,
    ev: &mut Vec<GameEvent>,
) -> bool {
    let (mx, my) = (w.mon_ent[i].x, w.mon_ent[i].y);
    let Some((slot, dm)) = nearest_alive_merc(w, mx, my) else { return false };
    if dm > hero_dist {
        return false;
    }
    monster_strike_merc_slot(w, slot, i, ev);
    true
}

/// Mercenaire vivant le plus proche de `(x, y)` — `(slot, distance)` ;
/// égalité ⇒ slot le plus bas (déterministe). `None` sans merc vivant.
fn nearest_alive_merc(w: &World, x: f32, y: f32) -> Option<(usize, f32)> {
    let root = w.merc.as_ref()?;
    let mut cand: Vec<(usize, f32, f32)> = Vec::new();
    if root.slot == 0 && root.alive {
        cand.push((0, root.ent.x, root.ent.y));
    }
    let mut coop: Vec<&MercData> = root.coop.mercs.iter().collect();
    coop.sort_by_key(|m| m.slot);
    for m in coop {
        if m.alive {
            cand.push((m.slot, m.ent.x, m.ent.y));
        }
    }
    cand
        .into_iter()
        .map(|(s, mx, my)| (s, dist(mx - x, my - y)))
        .min_by(|a, b| {
            a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0))
        })
}

/// Frappe d'un monstre SUR le mercenaire du slot `slot` (jet de toucher vs
/// défense du mercenaire, KO à hp 0 — cadavre résurrectible chez le NPC de
/// sa ville). Flux historique du merc slot 0, généralisé par slot.
pub(crate) fn monster_strike_merc_slot(w: &mut World, slot: usize, i: usize, ev: &mut Vec<GameEvent>) {
    let (dmin, dmax, th) = {
        let d = &w.mon_dat[i];
        (d.dmg_min, d.dmg_max, d.th)
    };
    let def = w.mercenary_slot(slot).map(|m| m.defense).unwrap_or(0);
    let chance = hit_chance(th, def);
    if (w.rng.below(100)) < chance {
        let dmg = w.rng.range_u32(dmin, dmax).max(1);
        let m = w.mercenary_slot_mut(slot).expect("merc connu");
        m.hp = m.hp.saturating_sub(dmg);
        ev.push(GameEvent::MercHit { dmg });
        if m.hp == 0 {
            m.alive = false;
            m.path.clear();
            m.swing_ms = 0;
            m.anim_ms = 0;
            m.missiles.clear();
            ev.push(GameEvent::MercDied);
        }
    }
    // raté : pas d'événement (même convention que la frappe sur le héros)
}

// ---------------------------------------------------------------------------
// Tests — stats de table, follow/attack/die, save/load, résurrection
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PlayerInput, World};
    use serde_json::json;

    /// Manifest réel du projet (compile-time, comme tests.rs).
    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn manifest_no_monsters() -> serde_json::Value {
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = json!([]);
        m
    }

    /// Manifest avec UN SEUL template monstre (zombie `ZM` du manifest réel)
    /// — les monstres auto-spawnés sont vidés : les tests posent leurs cibles
    /// via `spawn_monster` (déterminisme total).
    fn world_zombie_template(seed: u64) -> World {
        let full: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        let zm = full["monsters"]
            .as_array()
            .and_then(|arr| {
                arr.iter().find(|mm| mm.get("code").and_then(|c| c.as_str()) == Some("ZM")).cloned()
            })
            .expect("ZM dans le manifest réel");
        let mut m: serde_json::Value = full;
        m["monsters"] = json!([zm]);
        let mut w = World::with_seed(open_grid(40, 40), m, seed);
        w.clear_monsters();
        w
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    fn run(w: &mut World, ms: u32, input: Option<PlayerInput>) -> Vec<GameEvent> {
        let mut ev = Vec::new();
        let mut left = ms;
        while left > 0 {
            let dt = left.min(50);
            ev.extend(w.tick(dt, input));
            left -= dt;
        }
        ev
    }

    fn give_gold(w: &mut World, amount: u32) {
        w.hero_stats_mut().gold = amount;
    }

    fn hire_default(w: &mut World, kind: MercKind) -> u32 {
        give_gold(w, 1_000_000);
        w.hire_merc(kind).expect("embauche test")
    }

    // --- (1) table réelle : structure + valeurs golden ----------------------

    #[test]
    fn table_has_120_records_and_4_types() {
        let t = table();
        assert_eq!(t.len(), 120, "hireling.txt v1.12 = 120 records");
        for k in MercKind::ALL {
            let n = t.iter().filter(|r| r.kind == k).count();
            assert!(n > 0, "{k:?} absent de la table");
        }
        // actes réels (colonne Act)
        assert_eq!(MercKind::RogueScout.act(), 1);
        assert_eq!(MercKind::DesertMercenary.act(), 2);
        assert_eq!(MercKind::EasternSorceror.act(), 3);
        assert_eq!(MercKind::Barbarian.act(), 5);
        // vendeurs réels (colonne Seller = monstats Id)
        assert_eq!(MercKind::RogueScout.seller_monstats_id(), 150); // kashya
        assert_eq!(MercKind::DesertMercenary.seller_monstats_id(), 198); // greiz
        assert_eq!(MercKind::EasternSorceror.seller_monstats_id(), 252); // asheara
        assert_eq!(MercKind::Barbarian.seller_monstats_id(), 515); // qual-kehk
    }

    #[test]
    fn golden_rogue_scout_fire_normal_lvl3() {
        // hireling.txt ligne 2 : Rogue Scout | Fire - Normal | … | Level 3 |
        // Gold 100 | Exp/Lvl 100 | HP 45 | HP/Lvl 8 | Defense 15 | Def/Lvl 6 |
        // Str 35/10 | Dex 45/16 | AR 10/10 | Dmg-Min 1 | Dmg-Max 3 | Dmg/Lvl 2 |
        // Resist 0 | Resist/Lvl 8 | WType1 bow | DefaultChance 75
        let r = record_of(MercKind::RogueScout, "Fire - Normal", Difficulty::Normal)
            .expect("record rogue fire normal");
        assert_eq!(r.level, 3);
        assert_eq!(r.gold, 100);
        assert_eq!(r.exp_per_lvl, 100);
        assert_eq!(r.hp, 45);
        assert_eq!(r.hp_per_lvl, 8);
        assert_eq!(r.defense, 15);
        assert_eq!(r.ar, 10);
        assert_eq!(r.dmg_min, 1);
        assert_eq!(r.dmg_max, 3);
        assert_eq!(r.dmg_per_lvl, 2);
        assert_eq!(r.resist, 0);
        assert_eq!(r.resist_per_lvl, 8);
        assert_eq!(r.wtype, "bow");
        assert_eq!(r.default_chance, 75);
        assert_eq!(r.skills, vec!["Inner Sight", "Fire Arrow"]);
        // scaling niveau 4 : 45 + 8 = 53
        assert_eq!(r.hp_at(4), 53);
        assert_eq!(r.dmg_at(4), (3, 5));
    }

    #[test]
    fn golden_barbarian_normal_lvl28() {
        // hireling.txt : Barbarian | 1hs-Normal | Level 28 | Gold 9000 |
        // Exp/Lvl 120 | HP 288 | HP/Lvl 14 | … | Dmg-Min 16 | Dmg-Max 20 |
        // Dmg/Lvl 6 | Resist 56 | Resist/Lvl 7 | WType1 swor
        let r = record_of(MercKind::Barbarian, "1hs-Normal", Difficulty::Normal)
            .expect("record barb normal");
        assert_eq!(r.level, 28);
        assert_eq!(r.gold, 9000);
        assert_eq!(r.exp_per_lvl, 120);
        assert_eq!(r.hp, 288);
        assert_eq!(r.hp_per_lvl, 14);
        assert_eq!(r.dmg_min, 16);
        assert_eq!(r.dmg_max, 20);
        assert_eq!(r.resist, 56);
        assert_eq!(r.wtype, "swor");
        assert_eq!(r.skills.first().map(|s| s.as_str()), Some("Bash"));
        // mêlée
        assert_eq!(r.attack(), (MercAttack::Melee, None));
    }

    #[test]
    fn subtypes_and_default_chance() {
        // Rogue Normal : 2 sous-types, Fire = défaut (75 > 25)
        let rogue = records_for(MercKind::RogueScout, Difficulty::Normal);
        assert_eq!(rogue.len(), 12, "table réelle : 6 records/sous-type (palier lvl 3 dupliqué dans hireling.txt v1.12)");
        let def = default_subtype(MercKind::RogueScout, Difficulty::Normal).unwrap();
        assert_eq!(def.subtype, "Fire - Normal");
        // Desert Normal : Comb/Off/Def tous 30 → premier du fichier
        let dm = default_subtype(MercKind::DesertMercenary, Difficulty::Normal).unwrap();
        assert_eq!(dm.default_chance, 30);
        // offre : palier le plus haut ≤ niveau héros
        let r3 = offer_record(MercKind::RogueScout, None, Difficulty::Normal, 10).unwrap();
        assert_eq!(r3.level, 3);
        let r25 = offer_record(MercKind::RogueScout, Some("Ice - Normal"), Difficulty::Normal, 30)
            .unwrap();
        assert_eq!(r25.subtype, "Ice - Normal");
        assert_eq!(r25.level, 25);
        // sous le palier le plus bas → palier minimal
        let low = offer_record(MercKind::Barbarian, None, Difficulty::Normal, 1).unwrap();
        assert_eq!(low.level, 28);
    }

    #[test]
    fn attack_kinds_by_type() {
        let rogue = record_of(MercKind::RogueScout, "Fire - Normal", Difficulty::Normal).unwrap();
        assert_eq!(rogue.attack(), (MercAttack::Arrow, Some("firearrow")));
        let rogue_ice =
            record_of(MercKind::RogueScout, "Ice - Normal", Difficulty::Normal).unwrap();
        assert_eq!(rogue_ice.attack(), (MercAttack::Arrow, Some("icearrow")));
        let sorc =
            record_of(MercKind::EasternSorceror, "Cold-Normal", Difficulty::Normal).unwrap();
        assert_eq!(sorc.attack(), (MercAttack::Spell, Some("icebolt")));
        let sorc_l =
            record_of(MercKind::EasternSorceror, "Ltng-Normal", Difficulty::Normal).unwrap();
        assert_eq!(sorc_l.attack(), (MercAttack::Spell, Some("chargedbolt")));
        let desert =
            record_of(MercKind::DesertMercenary, "Off-Normal", Difficulty::Normal).unwrap();
        assert_eq!(desert.attack(), (MercAttack::Melee, None));
    }

    // --- (2/3) embauche, follow/attack/die, un seul merc ---------------------

    #[test]
    fn hire_deducts_gold_and_spawns_near_hero() {
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 42);
        give_gold(&mut w, 500);
        let hx = w.hero().x;
        let hy = w.hero().y;
        // rogue normal palier 3 = 100 or (colonne Gold)
        let lvl = w.hire_merc(MercKind::RogueScout).expect("embauche");
        assert_eq!(lvl, 3);
        assert_eq!(w.hero_stats().gold, 400, "coût 100 (colonne Gold) débité");
        let m = w.mercenary().expect("merc présent");
        assert_eq!(m.kind, MercKind::RogueScout);
        assert_eq!(m.subtype, "Fire - Normal", "sous-type par défaut (75)");
        assert_eq!(m.level, 3);
        assert_eq!(m.hp, m.max_hp);
        assert_eq!(m.max_hp, 45, "HP record ligne 2");
        assert_eq!(m.dmg_min, 1);
        assert_eq!(m.dmg_max, 3);
        // apparaît près du héros (≤ 3 tuiles)
        let md = (m.ent.x - hx).abs().max((m.ent.y - hy).abs());
        assert!(md <= 3.5, "spawn trop loin : {md}");
        // vivant, debout
        assert!(m.alive);
    }

    #[test]
    fn hire_refuses_when_alive_or_broke_replaces_when_ko() {
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 7);
        give_gold(&mut w, 1_000_000);
        w.hire_merc(MercKind::RogueScout).expect("1re embauche");
        // un seul mercenaire à la fois (règle D2) : refus tant qu'il vit
        assert_eq!(
            w.hire_merc(MercKind::Barbarian),
            Err(HireError::AlreadyAlive),
            "un seul hireling vivant"
        );
        // or insuffisant après KO → refus, merc KO conservé
        w.mercenary_mut().unwrap().hp = 0;
        w.mercenary_mut().unwrap().alive = false;
        w.hero_stats_mut().gold = 10;
        assert_eq!(
            w.hire_merc(MercKind::Barbarian),
            Err(HireError::InsufficientGold { cost: 9000, have: 10 }),
        );
        assert!(w.mercenary().unwrap().alive == false);
        // avec l'or : le NOUVEAU remplace l'ancien KO
        w.hero_stats_mut().gold = 1_000_000;
        let lvl = w.hire_merc(MercKind::Barbarian).expect("remplacement");
        assert_eq!(lvl, 28, "palier offert au niveau héros 1 : palier min 28");
        let m = w.mercenary().unwrap();
        assert_eq!(m.kind, MercKind::Barbarian);
        assert_eq!(w.hero_stats().gold, 1_000_000 - 9000);
    }

    #[test]
    fn merc_follows_the_hero() {
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 99);
        hire_default(&mut w, MercKind::RogueScout);
        // déplace le héros de 10 tuiles : le merc doit le suivre
        let hx = w.hero().x.floor() as i32;
        let hy = w.hero().y.floor() as i32;
        w.tick(0, Some(PlayerInput::MoveTo { x: hx + 10, y: hy }));
        run(&mut w, 3000, None);
        let m = w.mercenary().unwrap();
        let d = ((m.ent.x - w.hero().x).powi(2) + (m.ent.y - w.hero().y).powi(2)).sqrt();
        assert!(d <= MERC_FOLLOW_RADIUS + 1.0, "merc ne suit pas : {d}");
    }

    #[test]
    fn merc_attacks_hero_target_melee_and_dies() {
        // Desert Mercenary (mêlée) + un zombie posé : la CIBLE DU HÉROS est
        // désignée (un seul input ranged, le héros reste à distance), le merc
        // engage, tue, et encaisse l'XP du kill.
        let mut w = world_zombie_template(1234);
        hire_default(&mut w, MercKind::DesertMercenary);
        // zombie1 à 4 tuiles du merc (le héros reste loin du combat)
        let mx = w.mercenary().unwrap().ent.x.floor() as i32;
        let my = w.mercenary().unwrap().ent.y.floor() as i32;
        assert!(w.spawn_monster("ZM", mx + 4, my), "spawn zombie");
        // la cible du héros = le zombie (1 input, pas de poursuite héros)
        w.tick(50, Some(PlayerInput::RangedAttack { x: mx + 4, y: my }));
        let ev = run(&mut w, 20_000, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MercAttacked { .. })),
            "aucun coup de merc : {ev:?}"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })),
            "le monstre n'est pas mort"
        );
        assert!(
            w.mercenary().unwrap().xp > 0,
            "kill sans crédit XP mercenaire"
        );
    }

    #[test]
    fn rogue_shoots_arrows() {
        let mut w = world_zombie_template(555);
        hire_default(&mut w, MercKind::RogueScout);
        let mx = w.mercenary().unwrap().ent.x.floor() as i32;
        let my = w.mercenary().unwrap().ent.y.floor() as i32;
        assert!(w.spawn_monster("ZM", mx + 6, my));
        // PV gonflés : la flèche du HÉROS ne doit pas tuer la cible avant
        // que la rogue (flèches 2-3) n'ait décoché au moins une fois.
        w.mon_dat[0].hp = 5_000;
        w.mon_dat[0].max_hp = 5_000;
        w.tick(50, Some(PlayerInput::RangedAttack { x: mx + 6, y: my }));
        let ev = run(&mut w, 15_000, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MercMissileSpawned)),
            "la rogue n'a pas décoché : {ev:?}"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MercAttacked { .. })),
            "flèche sans coup résolu"
        );
    }

    #[test]
    fn merc_takes_damage_and_dies_ko() {
        let mut w = world_zombie_template(31415);
        hire_default(&mut w, MercKind::RogueScout);
        let mx = w.mercenary().unwrap().ent.x.floor() as i32;
        let my = w.mercenary().unwrap().ent.y.floor() as i32;
        // monstre collé au merc (plus proche de lui que du héros → le cible) ;
        // PV gonflés (la rogue tue à distance sinon) + pv merc prédrainés (3)
        // pour un KO dans la fenêtre de test déterministe
        assert!(w.spawn_monster("ZM", mx + 1, my));
        w.mon_dat[0].hp = 5_000;
        w.mon_dat[0].max_hp = 5_000;
        w.mercenary_mut().unwrap().hp = 3;
        let ev = run(&mut w, 30_000, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::MercHit { .. })),
            "le merc n'a rien encaissé : {ev:?}"
        );
        assert!(ev.iter().any(|e| matches!(e, GameEvent::MercDied)));
        let m = w.mercenary().unwrap();
        assert!(!m.alive, "merc devrait être KO");
        assert_eq!(m.hp, 0);
        assert_eq!(m.ent.anim, AnimState::DD);
    }

    // --- (3) résurrection ----------------------------------------------------

    #[test]
    fn resurrect_ko_merc_full_life() {
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 2718);
        hire_default(&mut w, MercKind::RogueScout);
        // vivant : résurrection refusée
        assert_eq!(w.resurrect_merc(), Err(ResurrectError::NoCorpse));
        // KO → résurrection pleine vie, coût MVP 0 (colonne absente de la table)
        w.mercenary_mut().unwrap().hp = 0;
        w.mercenary_mut().unwrap().alive = false;
        let cost = w.resurrect_merc().expect("résurrection");
        assert_eq!(cost, 0, "coût : pas de colonne resurrection dans hireling.txt");
        let m = w.mercenary().unwrap();
        assert!(m.alive);
        assert_eq!(m.hp, m.max_hp);
        assert_eq!(m.ent.anim, AnimState::NU);
    }

    // --- (4) save/load additif ----------------------------------------------

    #[test]
    fn save_load_roundtrip_merc() {
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 1618);
        hire_default(&mut w, MercKind::RogueScout);
        // merc blessé + XP partielle + niveau hors record (test du scaling)
        {
            let diff = w.difficulty();
            let m = w.mercenary_mut().unwrap();
            m.level = 4;
            let rec = record_of(m.kind, &m.subtype, diff).unwrap();
            m.recompute(rec);
            m.hp = m.max_hp - 7;
            m.xp = 42;
        }
        let json = w.save();
        let w2 = World::load(&json).expect("reload");
        let a = w.mercenary().unwrap();
        let b = w2.mercenary().expect("merc perdu à la sauvegarde");
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.subtype, b.subtype);
        assert_eq!(a.level, b.level);
        assert_eq!(a.xp, b.xp);
        assert_eq!(a.hp, b.hp);
        assert_eq!(a.max_hp, b.max_hp, "scaling recalculé depuis la table");
        assert_eq!(a.alive, b.alive);
        assert_eq!(a.ent.x, b.ent.x);
        assert_eq!(a.ent.y, b.ent.y);
        // rétro-compat : une save SANS mercenaire charge (merc = None)
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut old = v.clone();
        old.as_object_mut().unwrap().remove("mercenary");
        let w3 = World::load(&old.to_string()).expect("save sans mercenaire");
        assert!(w3.mercenary().is_none());
    }

    // --- intégration : flux héros inchangé sans mercenaire -------------------

    #[test]
    fn no_merc_by_default_and_zero_interference() {
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 8);
        assert!(w.mercenary().is_none());
        assert!(w.merc_entity().is_none());
        assert!(w.merc_missiles().is_empty());
        // tick sans mercenaire : aucun event merc
        let ev = run(&mut w, 2000, Some(PlayerInput::Attack { x: 5, y: 5 }));
        assert!(ev.iter().all(|e| !matches!(
            e,
            GameEvent::MercHired { .. }
                | GameEvent::MercAttacked { .. }
                | GameEvent::MercHit { .. }
                | GameEvent::MercDied { .. }
        )));
    }

    #[test]
    fn merc_levels_up_from_exp_per_lvl_column() {
        // palier = colonne Exp/Lvl (rogue fire normal = 100) ; plafond 98.
        let mut w = World::with_seed(open_grid(40, 40), manifest_no_monsters(), 404);
        hire_default(&mut w, MercKind::RogueScout);
        let diff = w.difficulty();
        let m = w.mercenary_mut().unwrap();
        assert_eq!(m.xp_to_next, 100, "Exp/Lvl record ligne 2");
        let rec = record_of(m.kind, &m.subtype, diff).unwrap();
        let mut evs = Vec::new();
        m.gain_xp(rec, 99, &mut evs);
        assert_eq!(m.level, 3, "99 < 100 : pas de niveau");
        m.gain_xp(rec, 1, &mut evs);
        assert_eq!(m.level, 4, "100 atteint → niveau 4");
        assert_eq!(m.max_hp, 53, "HP 45 + 8/lvl (colonnes HP, HP/Lvl)");
        assert!(evs.iter().any(|e| matches!(e, GameEvent::MercLeveled { level: 4 })));
    }

    // =======================================================================
    // 3c-a — MERCENAIRES PAR JOUEUR (coop) : indépendance par slot, suivi du
    // bon joueur, frappe du merc le plus proche, résurrection par slot,
    // déterminisme (double exécution ⇒ mêmes hash global et par slot).
    // =======================================================================

    /// Monde coop 2 joueurs sans monstres (slot 0 ouest, slot 1 est).
    fn coop_world_mercs(seed: u64) -> World {
        let m = manifest_no_monsters();
        let mut w = World::with_seed(open_grid(48, 48), m, seed);
        w.place_hero_at(8.5, 24.5);
        w.add_cohero_near("SOR", 40.0, 24.0).expect("cohéros");
        assert!(w.place_hero_slot_at(1, 40.5, 24.5), "slot 1 posé");
        give_gold(&mut w, 1_000_000);
        w.hero_stats_slot_mut(1).unwrap().gold = 1_000_000;
        w
    }

    #[test]
    fn coop_deux_slots_embauchent_des_mercs_independants() {
        let mut w = coop_world_mercs(0x3C_A1);
        // types INDÉPENDANTS : slot 0 = rogue (palier 3 = 100 or, bourse slot 0),
        // slot 1 = desert (bourse SLOT 1)
        let lvl0 = w.hire_merc_slot(0, MercKind::RogueScout).expect("merc slot 0");
        assert_eq!(lvl0, 3);
        let lvl1 = w.hire_merc_slot(1, MercKind::DesertMercenary).expect("merc slot 1");
        assert!(lvl1 >= 3, "palier offert au niveau du slot 1");
        assert_eq!(w.mercenary().unwrap().kind, MercKind::RogueScout, "mercenary() = vue slot 0");
        assert_eq!(w.mercenary_slot(0).unwrap().kind, MercKind::RogueScout);
        assert_eq!(w.mercenary_slot(1).unwrap().kind, MercKind::DesertMercenary);
        assert_eq!(w.mercenary_slots(), vec![0, 1]);
        // bourses débitées DU BON JOUEUR (rogue 100 ; desert = coût réel ≥ 100)
        let c1 = w.mercenary_slot(1).unwrap().hire_cost;
        assert_eq!(w.hero_stats().gold, 1_000_000 - 100, "or du slot 0 débité (100)");
        assert_eq!(w.hero_stats_slot(1).unwrap().gold, 1_000_000 - c1, "or du SLOT 1 débité");
        // chacun apparaît près de SON joueur
        let m0 = w.mercenary_slot(0).unwrap().ent;
        let m1 = w.mercenary_slot(1).unwrap().ent;
        let d0 = dist(m0.x - 8.5, m0.y - 24.5);
        let d1 = dist(m1.x - 40.5, m1.y - 24.5);
        assert!(d0 <= MERC_FOLLOW_RADIUS + 1.0, "merc slot 0 loin de SON joueur : {d0}");
        assert!(d1 <= MERC_FOLLOW_RADIUS + 1.0, "merc slot 1 loin de SON joueur : {d1}");
        // règle D2 PAR SLOT : un seul hireling vivant par joueur
        assert_eq!(w.hire_merc_slot(0, MercKind::Barbarian), Err(HireError::AlreadyAlive));
        assert_eq!(w.hire_merc_slot(1, MercKind::Barbarian), Err(HireError::AlreadyAlive));
        // slot hors monde refusé
        assert_eq!(w.hire_merc_slot(9, MercKind::Barbarian), Err(HireError::NoSlot));
    }

    #[test]
    fn coop_merc_suit_son_joueur_pas_celui_des_autres() {
        let mut w = coop_world_mercs(0x3C_A2);
        w.hire_merc_slot(0, MercKind::RogueScout).expect("merc slot 0");
        w.hire_merc_slot(1, MercKind::DesertMercenary).expect("merc slot 1");
        // le SLOT 1 part vers l'est ; le slot 0 reste à l'ouest
        w.tick_inputs(50, &[(1usize, PlayerInput::MoveTo { x: 46, y: 24 })]);
        run(&mut w, 4000, None);
        let (p0, p1) = (w.hero_slot_pos(0).unwrap(), w.hero_slot_pos(1).unwrap());
        let m0 = w.mercenary_slot(0).unwrap().ent;
        let m1 = w.mercenary_slot(1).unwrap().ent;
        let d0 = dist(m0.x - p0.0, m0.y - p0.1);
        let d1 = dist(m1.x - p1.0, m1.y - p1.1);
        assert!(d0 <= MERC_FOLLOW_RADIUS + 1.0, "merc slot 0 suit SON joueur : {d0}");
        assert!(d1 <= MERC_FOLLOW_RADIUS + 1.0, "merc slot 1 suit SON joueur : {d1}");
        assert!(dist(m1.x - p0.0, m1.y - p0.1) > 10.0, "le merc du slot 1 est resté à l'est");
    }

    #[test]
    fn coop_ko_et_resurrection_par_slot_independants() {
        let mut w = coop_world_mercs(0x3C_A3);
        w.hire_merc_slot(0, MercKind::RogueScout).expect("merc slot 0");
        w.hire_merc_slot(1, MercKind::DesertMercenary).expect("merc slot 1");
        // KO des DEUX mercs
        for s in 0..2 {
            let m = w.mercenary_slot_mut(s).unwrap();
            m.hp = 0;
            m.alive = false;
        }
        // résurrection du SEUL slot 1 : le slot 0 reste KO
        let cost = w.resurrect_merc_slot(1).expect("résurrection slot 1");
        assert_eq!(cost, 0, "coût MVP (colonne absente de hireling.txt)");
        let m1 = w.mercenary_slot(1).unwrap();
        assert!(m1.alive && m1.hp == m1.max_hp);
        assert_eq!(m1.ent.anim, AnimState::NU);
        assert!(!w.mercenary_slot(0).unwrap().alive, "merc slot 0 toujours KO");
        // ... puis celui du slot 0 ; vivant ⇒ refus
        w.resurrect_merc_slot(0).expect("résurrection slot 0");
        assert!(w.mercenary_slot(0).unwrap().alive);
        assert_eq!(w.resurrect_merc_slot(0), Err(ResurrectError::NoCorpse));
        assert_eq!(w.resurrect_merc_slot(1), Err(ResurrectError::NoCorpse));
        assert_eq!(w.resurrect_merc(), Err(ResurrectError::NoCorpse), "vue slot 0");
        // et l'embauche reste bloquée par slot tant que SON merc vit
        assert_eq!(w.hire_merc_slot(0, MercKind::Barbarian), Err(HireError::AlreadyAlive));
        assert_eq!(w.hire_merc_slot(1, MercKind::Barbarian), Err(HireError::AlreadyAlive));
    }

    #[test]
    fn coop_monstre_frappe_le_merc_le_plus_proche() {
        // FA collé au merc du SLOT 1 (est), slot 0 + son merc loin à l'ouest :
        // le monstre encaisse/rend les coups avec le merc COOP le plus proche
        // (hook `monster_strikes_nearest_merc` — le merc slot 0 n'est jamais
        // touché).
        let full: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        let fa = full["monsters"]
            .as_array()
            .and_then(|arr| arr.iter().find(|mm| mm.get("code").and_then(|c| c.as_str()) == Some("FA")).cloned())
            .expect("FA dans le manifest réel");
        let mut m: serde_json::Value = full;
        m["monsters"] = serde_json::json!([fa]);
        let mut w = World::with_seed(open_grid(48, 48), m, 0x3C_A4);
        w.clear_monsters();
        w.place_hero_at(5.5, 5.5);
        w.add_cohero_near("SOR", 40.0, 24.0).expect("cohéros");
        assert!(w.place_hero_slot_at(1, 40.5, 24.5), "slot 1 posé");
        give_gold(&mut w, 1_000_000);
        w.hero_stats_slot_mut(1).unwrap().gold = 1_000_000;
        w.hire_merc_slot(0, MercKind::RogueScout).expect("merc slot 0");
        w.hire_merc_slot(1, MercKind::DesertMercenary).expect("merc slot 1");
        // scénario déterministe : le merc coop ENCAISSE entre le FA et son
        // joueur (rôle D2 du tank) — FA PV gonflés (la mêlée du merc ne le
        // tue pas pendant la fenêtre), merc prédrainé pour un coup visible
        let m1x = w.mercenary_slot(1).unwrap().ent.x.floor() as i32;
        let m1y = w.mercenary_slot(1).unwrap().ent.y.floor() as i32;
        assert!(w.spawn_monster("FA", m1x - 3, m1y), "spawn FA à l'ouest du merc coop");
        w.mon_dat[0].hp = 5_000;
        w.mon_dat[0].max_hp = 5_000;
        w.mercenary_slot_mut(1).unwrap().hp = 3;
        // le merc se place SUR le segment FA→joueur (entre les deux, à 1 tuile
        // de son joueur) — rayon de suivi respecté, il n'en bouge plus
        {
            let (p1x, p1y) = w.hero_slot_pos(1).unwrap();
            let m = w.mercenary_slot_mut(1).unwrap();
            m.ent.x = p1x - 1.0;
            m.ent.y = p1y;
            m.path.clear();
        }
        let hp0 = w.mercenary_slot(0).unwrap().hp;
        let mut struck = false;
        for _ in 0..400 {
            let ev = w.tick(50, None);
            if ev.iter().any(|e| matches!(e, GameEvent::MercHit { .. })) {
                struck = true;
            }
            if struck && w.mercenary_slot(1).map(|mm| mm.hp < 3).unwrap_or(false) {
                break;
            }
        }
        assert!(struck, "le merc coop n'a rien encaissé (hook de frappe inactif ?)");
        let m1 = w.mercenary_slot(1).unwrap();
        assert!(m1.hp < m1.max_hp, "PV du merc SLOT 1 entamés");
        assert_eq!(w.mercenary_slot(0).unwrap().hp, hp0, "merc SLOT 0 (loin) jamais touché");
    }

    #[test]
    fn coop_determinisme_mercs_par_slot_double_execution() {
        // DEUX mondes, MÊMES entrées (script lockstep), mercs embauchés dans
        // les deux : hash GLOBAL et PAR SLOT identiques à chaque contrôle —
        // les mercenaires coop ne désynchronisent pas et ne déstabilisent pas
        // le hash par slot (3b).
        let build = || {
            let mut w = coop_world_mercs(0x3C_A5);
            w.hire_merc_slot(0, MercKind::RogueScout).expect("merc slot 0");
            w.hire_merc_slot(1, MercKind::DesertMercenary).expect("merc slot 1");
            w
        };
        let mut a = build();
        let mut b = build();
        for i in 0..90u32 {
            let batch: Vec<(usize, PlayerInput)> = match i % 3 {
                0 => vec![
                    (0usize, PlayerInput::MoveTo { x: 10 + (i as i32) % 6, y: 24 }),
                    (1usize, PlayerInput::MoveTo { x: 44 - (i as i32) % 6, y: 26 }),
                ],
                1 => vec![
                    (0usize, PlayerInput::RangedAttack { x: 20, y: 24 }),
                    (1usize, PlayerInput::Attack { x: 30, y: 24 }),
                ],
                _ => vec![(0usize, PlayerInput::SetRun(true)), (1usize, PlayerInput::SetRun(false))],
            };
            a.tick_inputs(50, &batch);
            b.tick_inputs(50, &batch);
            if i % 17 == 0 {
                assert_eq!(a.state_hash(), b.state_hash(), "hash global divergent au tick {i}");
                for s in 0..2 {
                    assert_eq!(
                        a.state_hash_slot(s),
                        b.state_hash_slot(s),
                        "hash par slot {s} divergent au tick {i}"
                    );
                }
            }
        }
        assert_eq!(a.state_hash(), b.state_hash(), "hash final identique");
        for s in 0..2 {
            assert_eq!(a.state_hash_slot(s), b.state_hash_slot(s), "hash slot {s} final");
        }
    }

    #[test]
    fn coop_save_roundtrip_mercs_par_slot() {
        let mut w = coop_world_mercs(0x3C_A6);
        w.hire_merc_slot(0, MercKind::RogueScout).expect("merc slot 0");
        w.hire_merc_slot(1, MercKind::EasternSorceror).expect("merc slot 1");
        // merc slot 1 blessé + XP partielle (niveau > palier record : le
        // sorcier normal embauché au palier 15, niveau courant 16)
        {
            let diff = w.difficulty();
            let m = w.mercenary_slot_mut(1).unwrap();
            m.level = 16;
            let rec = record_of(m.kind, &m.subtype, diff).unwrap();
            m.recompute(rec);
            m.hp = m.max_hp - 5;
            m.xp = 33;
        }
        let json = w.save();
        let w2 = World::load(&json).expect("reload");
        let save2 = w2.save();
        assert_eq!(json, save2, "round-trip bit-exact (mercs par slot)");
        let a = w.mercenary_slot(1).unwrap();
        let b = w2.mercenary_slot(1).expect("merc slot 1 perdu à la save");
        assert_eq!(a.kind, b.kind);
        assert_eq!(a.subtype, b.subtype);
        assert_eq!(a.level, b.level);
        assert_eq!(a.xp, b.xp);
        assert_eq!(a.hp, b.hp, "PV clampés/recalculés identiques");
        assert_eq!(a.ent.x, b.ent.x);
        // et la vue slot 0 traverse aussi
        assert_eq!(
            w.mercenary().unwrap().kind,
            w2.mercenary().expect("merc slot 0").kind
        );
        // rétro-compat : une save SANS mercenaire charge (aucun merc, pas de
        // coquille orpheline)
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let mut old = v.clone();
        old.as_object_mut().unwrap().remove("mercenary");
        let w3 = World::load(&old.to_string()).expect("save sans mercenaire");
        assert!(w3.mercenary().is_none());
        assert!(w3.mercenary_slot(1).is_none());
        assert!(w3.mercenary_slots().is_empty());
    }
}
