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
}

impl MercData {
    /// Construit depuis un record offert (embauche) — position posée par
    /// l'appelant ([`World::hire_merc`] la snap près du héros).
    pub(crate) fn from_rec(rec: &MercRec, rng_seed: u64) -> MercData {
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
        };
        m.recompute(rec);
        m.hp = m.max_hp;
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
        }
    }

    /// Reconstruit l'état runtime depuis la save + la table (stats
    /// recalculées au niveau courant ; hp clampé). `Err(())` = record
    /// introuvable dans la table (save d'une autre version de données) —
    /// l'appelant IGNORE alors le mercenaire (jamais un save invalide).
    pub(crate) fn rebuild(self, diff: Difficulty) -> Result<MercData, ()> {
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
    pub fn hire_merc(&mut self, kind: MercKind) -> Result<u32, HireError> {
        if self.merc.as_ref().map(|m| m.alive).unwrap_or(false) {
            return Err(HireError::AlreadyAlive);
        }
        let rec = match offer_record(kind, None, self.difficulty, self.hero.stats.level) {
            Some(r) => r,
            None => return Err(HireError::NoHireling),
        };
        let cost = rec.gold;
        if self.hero.stats.gold < cost {
            return Err(HireError::InsufficientGold { cost, have: self.hero.stats.gold });
        }
        self.hero.stats.gold -= cost;
        let seed = self.rng.s;
        let mut m = MercData::from_rec(rec, seed);
        // apparition : tuile marchable près du héros (spirale 2, sinon sur lui)
        let (hx, hy) = (self.hero_ent.x, self.hero_ent.y);
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
        self.merc = Some(m);
        self.merc_target = None;
        self.deferred.push(ev);
        Ok(level)
    }

    /// Résurrection du mercenaire KO chez le NPC de son type (H). PLEINE VIE,
    /// à côté du héros. COÛT MVP 0 : la table v1.12 ne porte pas de colonne
    /// de résurrection (formule client par niveau — restant) ; la valeur
    /// renvoyée est ce coût (0 aujourd'hui). L'événement `MercResurrected`
    /// est livré au tick suivant (file différée).
    pub fn resurrect_merc(&mut self) -> Result<u32, ResurrectError> {
        let alive = self.merc.as_ref().map(|m| m.alive).unwrap_or(false);
        if self.merc.is_none() || alive {
            return Err(ResurrectError::NoCorpse);
        }
        let m = self.merc.as_mut().expect("merc connu");
        m.alive = true;
        m.hp = m.max_hp;
        m.path.clear();
        m.swing_ms = 0;
        m.repath_ms = 0;
        m.ent.anim = AnimState::NU;
        m.ent.anim_frame = 0;
        m.anim_ms = 0;
        // ramène le cadavre près du héros (la résurrection se fait en ville)
        let (hx, hy) = (self.hero_ent.x, self.hero_ent.y);
        let g = crate::path::Grid { w: self.w, h: self.h, blocked: &self.grid };
        let spot = crate::path::snap_near(
            &g,
            (hx.floor() as i32, hy.floor() as i32),
            2,
        )
        .unwrap_or((hx.floor() as i32, hy.floor() as i32));
        m.ent.x = spot.0 as f32 + 0.5;
        m.ent.y = spot.1 as f32 + 0.5;
        let cost = 0; // colonne absente de hireling.txt v1.12 (MVP gratuit)
        self.deferred.push(GameEvent::MercResurrected { cost });
        Ok(cost)
    }

    /// Mercenaire courant (lecture) — état complet.
    pub fn mercenary(&self) -> Option<&MercData> {
        self.merc.as_ref()
    }

    /// Mercenaire courant (écriture — tests/chantiers parallèles, même crate).
    pub(crate) fn mercenary_mut(&mut self) -> Option<&mut MercData> {
        self.merc.as_mut()
    }

    /// Stats du héros (écriture — tests : doter l'or d'embauche).
    pub(crate) fn hero_stats_mut(&mut self) -> &mut crate::HeroStats {
        &mut self.hero.stats
    }

    /// Entité du mercenaire (lecture — rendu d2game).
    pub fn merc_entity(&self) -> Option<&Entity> {
        self.merc.as_ref().map(|m| &m.ent)
    }

    /// Missiles du mercenaire en vol (lecture — rendu d2game, flèches/sorts).
    pub fn merc_missiles(&self) -> &[d2missiles::Missile] {
        self.merc.as_ref().map(|m| m.missiles.missiles()).unwrap_or(&[])
    }

    /// Offre d'embauche visible (lecture — UI NPC du membre VILLES) :
    /// `(sous-type, niveau offert, coût)` pour le type demandé, à la
    /// difficulté et au niveau de héros courants.
    pub fn merc_offer(&self, kind: MercKind) -> Option<(String, u32, u32)> {
        offer_record(kind, None, self.difficulty, self.hero.stats.level)
            .map(|r| (r.subtype.clone(), r.level, r.gold))
    }
}

// ---------------------------------------------------------------------------
// Tick (appelé par `World::tick`, après `update_missiles`)
// ---------------------------------------------------------------------------

/// FSM du mercenaire : follow → attaque de la cible du héros → KO.
pub(crate) fn tick_mercenary(w: &mut World, dt: u32, ev: &mut Vec<GameEvent>) {
    let dt = dt.min(MAX_DT_MS);
    if w.merc.is_none() {
        return;
    }
    let alive = w.merc.as_ref().map(|m| m.alive).unwrap_or(false);
    if !alive {
        // cadavre : joue l'anim DD une fois puis reste au sol (résurrectible)
        let m = w.merc.as_mut().expect("merc connu");
        crate::world::set_anim(&mut m.ent, AnimState::DD, &mut m.anim_ms);
        crate::world::advance_anim(&mut m.ent, &mut m.anim_ms, dt, false);
        return;
    }
    // cooldowns
    {
        let m = w.merc.as_mut().expect("merc connu");
        m.swing_ms = m.swing_ms.saturating_sub(dt);
        m.repath_ms = m.repath_ms.saturating_sub(dt);
    }

    let hero_dead = w.hero.dead_ms > 0;
    let (hx, hy) = (w.hero_ent.x, w.hero_ent.y);

    // --- décision (lectures immuables) ---
    enum Act {
        Melee(usize),
        Shoot(usize),
        Walk(f32, f32),
        Idle,
    }
    let act = {
        let m = w.merc.as_ref().expect("merc connu");
        // cible : LA CIBLE DU HÉROS d'abord (brief H), sinon le monstre vivant
        // le plus proche DU MERCENAIRE (rayon d'aggro propre)
        let mut target: Option<usize> = w
            .merc_target
            .filter(|&i| i < w.mon_ent.len() && w.mon_dat[i].state != crate::world::MonState::Dying);
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
                // pas de cible : suit le héros (rayon de suivi)
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
        Act::Melee(t) => merc_melee_attack(w, t, ev),
        Act::Shoot(t) => merc_shoot(w, t, ev),
        Act::Walk(wx, wy) => {
            let (mt, needs) = {
                let m = w.merc.as_ref().expect("merc connu");
                (
                    (m.ent.x.floor() as i32, m.ent.y.floor() as i32),
                    m.path.is_empty() || m.repath_ms == 0,
                )
            };
            if needs {
                // re-path périodique (ou chemin vide) — A* partagé du monde
                if let Some(p) = w.path_between(mt, (wx.floor() as i32, wy.floor() as i32)) {
                    let m = w.merc.as_mut().expect("merc connu");
                    m.path = p;
                }
                let m = w.merc.as_mut().expect("merc connu");
                m.repath_ms = MERC_REPATH_MS;
            }
        }
        Act::Idle => {
            let m = w.merc.as_mut().expect("merc connu");
            m.path.clear();
        }
    }

    // --- déplacement + anim ---
    // vitesse : celle du HÉROS (le mercenaire garde le pas — MVP ; le walkSpeed
    // hireling réel viendrait de monstats, restant)
    let speed = w.hero.stats.walk_speed.max(1.0);
    let m = w.merc.as_mut().expect("merc connu");
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
fn merc_melee_attack(w: &mut World, t: usize, ev: &mut Vec<GameEvent>) {
    let (mx, my) = (w.mon_ent[t].x, w.mon_ent[t].y);
    {
        let m = w.merc.as_mut().expect("merc connu");
        if m.swing_ms > 0 {
            return; // cooldown
        }
        m.ent.facing = crate::world::facing_from(mx - m.ent.x, my - m.ent.y);
        m.swing_ms = MERC_SWING_MS;
        m.anim_ms = 0;
    }
    let (ar, dmin, dmax) = {
        let m = w.merc.as_ref().expect("merc connu");
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
            merc_kill_credit(w, t, ev);
        }
    } else {
        ev.push(GameEvent::MercMissed { target: t });
    }
}

/// Tir mercenaire : flèche porteuse d'ARME (rogue — dégâts du mercenaire
/// résolus au tir, règle D2 « la flèche sait ») ou missile de SORT (sorceror
/// — dégâts officiels de la table d2missiles embarquée).
fn merc_shoot(w: &mut World, t: usize, ev: &mut Vec<GameEvent>) {
    let (tx, ty) = (w.mon_ent[t].x, w.mon_ent[t].y);
    let (kind, code) = {
        let m = w.merc.as_ref().expect("merc connu");
        if m.swing_ms > 0 {
            return;
        }
        m.attack(w.difficulty)
    };
    let Some(code) = code else { return };
    let m = w.merc.as_mut().expect("merc connu");
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
pub(crate) fn tick_merc_missiles(w: &mut World, dt: u32, ev: &mut Vec<GameEvent>) {
    if w.merc.is_none() {
        return;
    }
    // snapshot des cibles (comme update_missiles : monstres figés au tick)
    let targets: Vec<(f32, f32, bool)> = w
        .mon_ent
        .iter()
        .zip(w.mon_dat.iter())
        .map(|(e, d)| (e.x, e.y, d.state == crate::world::MonState::Dying))
        .collect();
    let (grid, gw, gh) = (&w.grid, w.w, w.h);
    let events = {
        let m = w.merc.as_mut().expect("merc connu");
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
                    let m = w.merc.as_ref().expect("merc connu");
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
                        merc_kill_credit(w, target, ev);
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

/// Crédit XP du mercenaire pour le kill du monstre `t` (le JOUEUR reçoit
/// toujours son XP via `MonsterDied` → `apply_xp`, flux inchangé).
fn merc_kill_credit(w: &mut World, t: usize, ev: &mut Vec<GameEvent>) {
    let xp = w.mon_dat[t].xp;
    if xp == 0 {
        return;
    }
    let diff = w.difficulty;
    let rec = w.merc.as_ref().and_then(|m| m.rec(diff));
    if let Some(rec) = rec {
        let m = w.merc.as_mut().expect("merc connu");
        m.gain_xp(rec, xp, ev);
    }
}

/// Frappe d'un monstre SUR le mercenaire (retarget additif : le monstre
/// frappe la cible la plus proche — cf. `monster_strike` dans world.rs).
/// Jet de toucher vs défense du mercenaire, KO à hp 0 (cadavre résurrectible).
pub(crate) fn monster_strike_merc(w: &mut World, i: usize, ev: &mut Vec<GameEvent>) {
    let (dmin, dmax, th) = {
        let d = &w.mon_dat[i];
        (d.dmg_min, d.dmg_max, d.th)
    };
    let def = w.merc.as_ref().map(|m| m.defense).unwrap_or(0);
    let chance = hit_chance(th, def);
    if (w.rng.below(100)) < chance {
        let dmg = w.rng.range_u32(dmin, dmax).max(1);
        let m = w.merc.as_mut().expect("merc connu");
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
}
