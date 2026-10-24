//! Étape 9b — monde vivant (W09, `finish-monde.md`) : élites data-driven —
//! champions bleus & uniques dorés (`elite_mods.rs`).
//!
//! Couche ADDITIVE au plan existant (`monsters.rs::plan_level_spawns_diff`,
//! `spawn.rs::OutdoorSpawn`) : ce module lit `assets/monumod.txt` via le
//! parseur TSV générique [`d2rules::tsv`] et produit, pour un niveau +
//! difficulté, les [`EliteInfo`] (JSON) des leaders élites — sans toucher au
//! code de spawn existant (contrainte étape 9b : ce fichier + 1 ligne
//! `lib.rs`).
//!
//! Sources réelles v1.12 (aucune invention — chaque champ cite son fichier) :
//!
//! * `assets/monumod.txt` (43 records) — colonnes `cpick`/`cpick (N)`/
//!   `cpick (H)` (poids CHAMPION par difficulté) et `upick`/`upick (N)`/
//!   `upick (H)` (poids UNIQUE) : **le pool champion réel est
//!   {champion, ghostly, fanatic, possessed, berserk}** (cpick=1) et le pool
//!   unique est {strong, fast, curse, fire, lightning, cold, manahit,
//!   teleport, spectralhit, stoneskin, multishot, aura} (upick=6) +
//!   `resist` en NM/Enfer SEULEMENT (cellule Normal vide — `upick`, ligne
//!   « resist »). `champion` (id 16) = marque du rang, tirée comme les
//!   autres. `lightning` porte `exclude1=sandleaper` (exclusion de classe
//!   réelle). `ghostly`/`fanatic`/`possessed`/`berserk` ont `version=100`
//!   (introduits en 1.10, donc présents en 1.12).
//! * Colonne `constants` de `monumod.txt`, indexée par `*constant desc` —
//!   **les constantes officielles du moteur**, par difficulté via les
//!   suffixes `(N)`/`(H)` : champion +hp% 200/150/100 (⇒ ×3 / ×2,5 / ×2),
//!   unique +hp% 300/200/100 (⇒ ×4 / ×3 / ×2), champion +dmg% 100 (×2),
//!   unique +dmg% 150 (×2,5), minion +hp% 100/75/50, +tohit% champion 75 /
//!   unique 100 / minion 50, minion +dmg% 75, et les dégâts élémentaires
//!   ajoutés par les mods Enchanted : champion +elem min/max % = 33/50,
//!   unique = 66/100, minion = 0-50-50.
//! * `assets/patch_difficultylevels.txt` (via [`DifficultyLevels`],
//!   `d2rules::difficulty`) — `UniqueDamageBonus`/`ChampionDamageBonus`
//!   90/75/66 : multiplicateur final des dégâts d'élite
//!   `(100 + bonus_monumod) × bonus_difficulté / 100`
//!   (guide DifficultyLevels.txt d2mods.info/Nefarius — communauté, non
//!   fiable, cohérent avec le sens 90→75→66 = amortissement des one-shots
//!   NM/Enfer introduit en 1.10).
//! * `assets/superuniques.txt` (colonne `Utrans`) — pool de TEINTES réelles
//!   D2 (31 valeurs distinctes 4..38, celles que Blizzard pose sur ses
//!   nommés) : l'unique aléatoire y est tiré seedé ; `monstats2.txt`
//!   (610 records, `d2w-re/tables/monstres`) confirme la convention :
//!   Utrans des boss 2/3/4, 255 = « pas de transformation » (summons
//!   nécro), 595 classes sans teinte. Champion : 0 = pas de palette (réel :
//!   seuls les uniques dorés changent de teinte — côté UI le bleu vient du
//!   rang).
//!
//! # Multiplicateurs retenus (golden, testés)
//!
//! | rang      | HP (N/NM/H)   | dégâts (N/NM/H)      |
//! |-----------|---------------|----------------------|
//! | champion  | ×3 / ×2,5 / ×2 | ×1,80 / ×1,50 / ×1,32 |
//! | unique    | ×4 / ×3 / ×2   | ×2,25 / ×1,875 / ×1,65 |
//!
//! HP = `100 + monumod["champion|unique +hp%"]` ; dégâts =
//! `(100 + monumod["champion|unique +dmg%"]) × DifficultyLevels[
//! Champion|Unique]DamageBonus / 100` (division entière, plancher :
//! 2,5 × 0,75 = 187,5 → 187). Les docs publiques (Arreat Summit, Amazon
//! Basin wiki — NON FIABLES) confirment ×3/×4 HP et ×2 champion en Normal ;
//! elles ne publient NI le palier HP par difficulté NI la pénalité
//! `DamageBonus` — la table embarquée fait foi.
//!
//! Le RNG est le contrat d2rules : [`crate::drlg::SplitMix64`], la copie
//! publique exacte de `d2rules::rng::Rng` (même graine ⇒ même partie).

use crate::drlg::SplitMix64;
use crate::levels::LevelRecord;
use crate::monsters::LevelSpawnPlan;
use d2rules::difficulty::DifficultyLevels;
use d2rules::tsv::Tsv;
use d2rules::Difficulty;
use serde::Serialize;

/// Tables embarquées (même modèle que `spawn.rs`/`monsters.rs`) : zéro IO.
const MONUMOD_TXT: &str = include_str!("../../../../assets/monumod.txt");
const SUPERUNIQUES_TXT: &str = include_str!("../../../../assets/superuniques.txt");

// ---------------------------------------------------------------------------
// Catalogue des mods (monumod.txt via d2rules::tsv)
// ---------------------------------------------------------------------------

/// Effet moteur d'un mod — lecture **qualitative** communauté (Arreat Summit /
/// Amazon Basin wiki, NON FFIABLE : le moteur 1.12 les code en dur, sans
/// table) ; les seuls pourcentages exposés viennent des colonnes `constants`
/// réelles de monumod.txt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EliteEffect {
    /// Aucun effet moteur consommé (mod interne/marque).
    Passive,
    /// Bonus de dégâts physiques (Extra Strong).
    PhysicalDamage,
    /// Bonus de vitesse (Extra Fast, Fanatic).
    Speed,
    /// Frappes maudites (Cursed).
    CurseOnHit,
    /// Résists élémentaires (Magic Resistant — NM/Enfer seulement).
    ElementalResist,
    /// Dégâts élémentaires sur frappe (pourcentages RÉELS : constants
    /// monumod par rang) ; `death_burst` = relâche à la mort (explosion
    /// feu / Frost Nova / Charged Bolts), `chills` = ralentit la cible.
    Elemental { elem: Elem, death_burst: bool, chills: bool },
    /// Brûlure de mana à la frappe (Mana Burn).
    ManaBurn,
    /// Se téléporte quand frappé (Teleport).
    Teleport,
    /// +défense et résist physique (Stone Skin).
    StoneSkin,
    /// Projectiles multiples pour les attaquants à distance (Multiple Shot).
    MultiShot,
    /// Émet une aura tirée dans [`AURA_POOL`] (Might, Holy Fire, Thorns…).
    Aura,
    /// Variante de champion 1.10+ (Ghostly/Fanatic/Possessed/Berserk) —
    /// descriptif communauté, aucun chiffre in-engine exposé.
    ChampionVariant,
}

/// Élément d'un mod Enchanted (`spectralhit` = élément aléatoire par frappe).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elem {
    Fire,
    Cold,
    Lightning,
    Random,
}

/// Code JSON court d'un effet (stable pour l'UI).
pub fn effect_code(effect: EliteEffect) -> &'static str {
    match effect {
        EliteEffect::Passive => "none",
        EliteEffect::PhysicalDamage => "dmg_physical",
        EliteEffect::Speed => "speed",
        EliteEffect::CurseOnHit => "curse_on_hit",
        EliteEffect::ElementalResist => "resist_all",
        EliteEffect::Elemental { elem: Elem::Fire, .. } => "elem_fire",
        EliteEffect::Elemental { elem: Elem::Cold, .. } => "elem_cold",
        EliteEffect::Elemental { elem: Elem::Lightning, .. } => "elem_lightning",
        EliteEffect::Elemental { elem: Elem::Random, .. } => "elem_random",
        EliteEffect::ManaBurn => "mana_burn",
        EliteEffect::Teleport => "teleport",
        EliteEffect::StoneSkin => "stone_skin",
        EliteEffect::MultiShot => "multishot",
        EliteEffect::Aura => "aura",
        EliteEffect::ChampionVariant => "champion_variant",
    }
}

/// Auras possibles d'« Envoutement aura » — liste communauté (non fiable) ;
/// la valeur tirée est purement décorative côté port (affichage UI à venir).
pub const AURA_POOL: &[&str] =
    &["Might", "Holy Fire", "Thorns", "Holy Freeze", "Holy Shock", "Conviction", "Fanaticism"];

/// Métadonnées d'affichage d'un token monumod (nom EN + clé TBL fr-FR réelle).
struct ModMeta {
    en: &'static str,
    /// Clé `assets/tbl/string.tsv` / `expansionstring.tsv` ("" = aucune).
    tbl: &'static str,
    effect: EliteEffect,
}

/// Métadonnées par token — clés TBL vérifiées dans les tables fr v1.12
/// (string.tsv lignes 3205-3221, expansionstring.tsv lignes 2472-2475 ; la
/// coquille Blizzard « uniquextrastrong » est la clé RÉELLE). Token absent
/// de la table ⇒ EN vide : l'appelant retombe sur le token brut.
fn meta(token: &str) -> ModMeta {
    let (en, tbl, effect) = match token {
        "strong" => ("Extra Strong", "uniquextrastrong", EliteEffect::PhysicalDamage),
        "fast" => ("Extra Fast", "uniqueextrafast", EliteEffect::Speed),
        "curse" => ("Cursed", "uniquecursed", EliteEffect::CurseOnHit),
        "resist" => ("Magic Resistant", "uniquemagicresistance", EliteEffect::ElementalResist),
        "fire" => ("Fire Enchanted", "uniquefireenchanted", EliteEffect::Elemental {
            elem: Elem::Fire,
            death_burst: true,
            chills: false,
        }),
        "cold" => ("Cold Enchanted", "monsteruniqueprop1", EliteEffect::Elemental {
            elem: Elem::Cold,
            death_burst: true,
            chills: true,
        }),
        "lightning" => ("Lightning Enchanted", "monsteruniqueprop2", EliteEffect::Elemental {
            elem: Elem::Lightning,
            death_burst: true,
            chills: false,
        }),
        "spectralhit" => ("Spectral Hit", "monsteruniqueprop4", EliteEffect::Elemental {
            elem: Elem::Random,
            death_burst: false,
            chills: false,
        }),
        "manahit" => ("Mana Burn", "monsteruniqueprop3", EliteEffect::ManaBurn),
        "teleport" => ("Teleport", "monsteruniqueprop5", EliteEffect::Teleport),
        "stoneskin" => ("Stone Skin", "monsteruniqueprop6", EliteEffect::StoneSkin),
        "multishot" => ("Multiple Shot", "monsteruniqueprop7", EliteEffect::MultiShot),
        "aura" => ("Aura Enchanted", "monsteruniqueprop9", EliteEffect::Aura),
        "champion" => ("Champion", "Champion", EliteEffect::Passive),
        "ghostly" => ("Ghostly", "Ghostly", EliteEffect::ChampionVariant),
        "fanatic" => ("Fanatic", "Fanatic", EliteEffect::ChampionVariant),
        "possessed" => ("Possessed", "Possessed", EliteEffect::ChampionVariant),
        "berserk" => ("Berserk", "Berserk", EliteEffect::ChampionVariant),
        _ => ("", "", EliteEffect::Passive),
    };
    ModMeta { en, tbl, effect }
}

/// Un modificateur d'élite (`monumod.txt`, une ligne).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EliteModDef {
    /// Id numérique de la table (référencé par `SuperUniques.Mod1-3`).
    pub id: u32,
    /// Token de la colonne `uniquemod` (`strong`, `cold`…).
    pub token: String,
    /// Nom EN d'affichage (table `meta`, repli = token).
    pub display_en: String,
    /// Clé TBL fr-FR ("" = pas de libellé retail).
    pub tbl_key: String,
    /// 0 = classique, 100 = introduit en 1.10 (ghostly & co — présent 1.12).
    pub version: u32,
    /// Colonne `enabled`.
    pub enabled: bool,
    /// Poids champion par difficulté [N, NM, H] (`cpick`/`cpick (N)/(H)`).
    pub champion_pool: [u32; 3],
    /// Poids unique par difficulté [N, NM, H] (`upick`/`upick (N)/(H)`).
    pub unique_pool: [u32; 3],
    /// Colonne `fPick` brute (sémantique « forced pick » non consommée —
    /// fast=3, multishot=2, teleport=1 ; valeur conservée, data fait foi).
    pub f_pick: u32,
    /// Exclusions de classe réelles (`exclude1`/`exclude2` — ex. lightning
    /// exclut `sandleaper`).
    pub exclude: [String; 2],
    /// Effet moteur (lecture qualitative communauté, cf. [`EliteEffect`]).
    pub effect: EliteEffect,
}

/// Constantes moteur de monumod.txt (colonne `constants` × `*constant desc`),
/// par difficulté. Défauts = valeurs réelles du fichier v1.12.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EliteConstants {
    /// « champion chance » = 20 — probabilité par pack qu'un leader soit
    /// champion. Le port la consomme en % (roue /100, convention établie
    /// `monsters.rs`) ; l'hypothèse /1024 (≈2 %, plus proche du retail) est
    /// non vérifiée in-engine et PAS appliquée ici.
    pub champion_chance: u32,
    /// « champion +hp% » [N, NM, H] = 200/150/100 ⇒ ×3 / ×2,5 / ×2.
    pub champion_hp_pct: [u32; 3],
    /// « unique +hp% » [N, NM, H] = 300/200/100 ⇒ ×4 / ×3 / ×2.
    pub unique_hp_pct: [u32; 3],
    /// « minion +hp% » [N, NM, H] = 100/75/50.
    pub minion_hp_pct: [u32; 3],
    /// « champion +tohit% » = 75.
    pub champion_th_pct: u32,
    /// « unique +tohit% » = 100.
    pub unique_th_pct: u32,
    /// « minion +tohit% » = 50.
    pub minion_th_pct: u32,
    /// « champion +dmg% » = 100 ⇒ ×2.
    pub champion_dmg_pct: u32,
    /// « unique +dmg% (strong) » = 150 ⇒ ×2,5.
    pub unique_dmg_pct: u32,
    /// « minion +dmg% (strong) » = 75.
    pub minion_dmg_pct: u32,
    /// « champion +elem min/max dmg% » = 33/50.
    pub champion_elem_dmg_pct: [u32; 2],
    /// « unique +elem min/max dmg% » = 66/100.
    pub unique_elem_dmg_pct: [u32; 2],
    /// « minion +elem min/max dmg% » [N, NM, H] = 0/50/50 (min comme max).
    pub minion_elem_dmg_pct: [u32; 3],
}

impl Default for EliteConstants {
    fn default() -> EliteConstants {
        EliteConstants {
            champion_chance: 20,
            champion_hp_pct: [200, 150, 100],
            unique_hp_pct: [300, 200, 100],
            minion_hp_pct: [100, 75, 50],
            champion_th_pct: 75,
            unique_th_pct: 100,
            minion_th_pct: 50,
            champion_dmg_pct: 100,
            unique_dmg_pct: 150,
            minion_dmg_pct: 75,
            champion_elem_dmg_pct: [33, 50],
            unique_elem_dmg_pct: [66, 100],
            minion_elem_dmg_pct: [0, 50, 50],
        }
    }
}

/// Le catalogue monumod + constantes + pool de teintes.
#[derive(Debug, Clone, Default)]
pub struct EliteMods {
    pub mods: Vec<EliteModDef>,
    pub constants: EliteConstants,
    /// Teintes D2 réelles (colonne `Utrans` de superuniques.txt, distinctes
    /// triées) — l'unique aléatoire tire dedans ; vide ⇒ teinte 0.
    pub utrans_pool: Vec<u32>,
}

/// Rang d'élite (leader de pack).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EliteRank {
    /// Champion bleu — 1 mod (pool `cpick`).
    Champion,
    /// Unique doré — 3 mods distincts (pool `upick`).
    Unique,
}

impl EliteRank {
    /// Libellé JSON stable (l'UI « doré/bleu » en découle).
    pub fn as_str(self) -> &'static str {
        match self {
            EliteRank::Champion => "champion",
            EliteRank::Unique => "unique",
        }
    }
}

impl EliteMods {
    /// Parse `monumod.txt` via le parseur TSV générique [`d2rules::tsv`]
    /// (43 records v1.12). `Err` si la table est vide ou trop courte.
    pub fn parse(tsv: &str) -> Result<EliteMods, String> {
        let table = Tsv::parse(tsv).map_err(|e| format!("monumod.txt : {e}"))?;
        let mut m = EliteMods::default();
        for i in 0..table.len() {
            let Some(r) = table.rec(i) else { continue };
            let token = r.s("uniquemod");
            if token.is_empty() {
                continue;
            }
            let meta = meta(token);
            m.mods.push(EliteModDef {
                id: r.u("id"),
                token: token.to_string(),
                display_en: if meta.en.is_empty() { token.to_string() } else { meta.en.to_string() },
                tbl_key: meta.tbl.to_string(),
                version: r.u("version"),
                enabled: r.u("enabled") == 1,
                champion_pool: [r.u("cpick"), r.u("cpick (N)"), r.u("cpick (H)")],
                unique_pool: [r.u("upick"), r.u("upick (N)"), r.u("upick (H)")],
                f_pick: r.u("fPick"),
                exclude: [r.s("exclude1").to_string(), r.s("exclude2").to_string()],
                effect: meta.effect,
            });
            // Constantes moteur : (desc → valeur) sur la même ligne, suffixe
            // de difficulté « (N) »/« (H) » → slot 1/2, sans suffixe → slot 0.
            let (v, desc) = (r.u("constants"), r.s("*constant desc"));
            let (base, slot) = match desc.strip_suffix(" (N)") {
                Some(b) => (b, 1usize),
                None => match desc.strip_suffix(" (H)") {
                    Some(b) => (b, 2usize),
                    None => (desc, 0usize),
                },
            };
            let c = &mut m.constants;
            match base {
                "champion chance" => c.champion_chance = v,
                "champion +hp%" => c.champion_hp_pct[slot] = v,
                "unique +hp%" => c.unique_hp_pct[slot] = v,
                "minion +hp%" => c.minion_hp_pct[slot] = v,
                "champion +tohit%" => c.champion_th_pct = v,
                "unique +tohit%" => c.unique_th_pct = v,
                "minion +tohit%" => c.minion_th_pct = v,
                "champion +dmg%" => c.champion_dmg_pct = v,
                "unique +dmg% (strong)" => c.unique_dmg_pct = v,
                "minion +dmg% (strong)" => c.minion_dmg_pct = v,
                "champion +elem min dmg%" => c.champion_elem_dmg_pct[0] = v,
                "champion +elem max dmg%" => c.champion_elem_dmg_pct[1] = v,
                "unique +elem min dmg%" => c.unique_elem_dmg_pct[0] = v,
                "unique +elem max dmg%" => c.unique_elem_dmg_pct[1] = v,
                "minion +elem min dmg%" | "minion +elem max dmg%" => c.minion_elem_dmg_pct[slot] = v,
                _ => {}
            }
        }
        if m.mods.len() < 40 {
            return Err(format!("monumod.txt : {} records seulement", m.mods.len()));
        }
        Ok(m)
    }

    /// Charge le pool de teintes réelles depuis `superuniques.txt`
    /// (colonne `Utrans`, valeurs distinctes triées — 31 valeurs 4..38).
    /// Appel lenient : table absente ⇒ pool inchangé.
    pub fn load_utrans_pool(&mut self, superuniques_tsv: &str) {
        let Ok(t) = Tsv::parse(superuniques_tsv) else { return };
        let mut vals: Vec<u32> = (0..t.len())
            .filter_map(|i| t.rec(i))
            .map(|r| r.u("Utrans"))
            .filter(|v| *v > 0 && *v < 255)
            .collect();
        vals.sort_unstable();
        vals.dedup();
        if !vals.is_empty() {
            self.utrans_pool = vals;
        }
    }

    /// Instance embarquée parsée une fois (runtime WASM, tests).
    pub fn embedded() -> &'static EliteMods {
        static E: std::sync::OnceLock<EliteMods> = std::sync::OnceLock::new();
        E.get_or_init(|| {
            let mut m = EliteMods::parse(MONUMOD_TXT)
                .expect("monumod.txt embarqué valide");
            m.load_utrans_pool(SUPERUNIQUES_TXT);
            m
        })
    }

    fn pool_weight(self_rank: EliteRank, m: &EliteModDef, d: Difficulty) -> u32 {
        let slot = d.index();
        match self_rank {
            EliteRank::Champion => m.champion_pool[slot],
            EliteRank::Unique => m.unique_pool[slot],
        }
    }

    /// Tirage de `n` mods DISTINCTS aux poids réels de la difficulté, en
    /// excluant les classes `exclude1`/`exclude2` (ex. lightning/sandleaper).
    /// Re-tirage borné (8 essais) si l'exclusion tombe ; `class = None`
    /// (classe inconnue) n'exclut rien.
    pub fn pick_mods(
        &self,
        rank: EliteRank,
        diff: Difficulty,
        class: Option<&str>,
        rng: &mut SplitMix64,
        n: usize,
    ) -> Vec<&EliteModDef> {
        let mut pool: Vec<&EliteModDef> = self
            .mods
            .iter()
            .filter(|m| m.enabled && Self::pool_weight(rank, m, diff) > 0)
            .collect();
        let mut picked = Vec::new();
        for _ in 0..n {
            if pool.is_empty() {
                break;
            }
            let total: u64 = pool
                .iter()
                .map(|m| Self::pool_weight(rank, m, diff) as u64)
                .sum();
            let mut chosen = 0usize;
            for attempt in 0..8 {
                let mut roll = (rng.below(total as usize)) as u64;
                chosen = pool.len() - 1;
                for (i, m) in pool.iter().enumerate() {
                    let w = Self::pool_weight(rank, m, diff) as u64;
                    if roll < w {
                        chosen = i;
                        break;
                    }
                    roll -= w;
                }
                let m = pool[chosen];
                let excluded = class
                    .map(|c| m.exclude.iter().any(|x| !x.is_empty() && x == c))
                    .unwrap_or(false);
                if !excluded || attempt == 7 {
                    break; // accepté — ou dernier essai : on prend quand même
                }
            }
            picked.push(pool.remove(chosen));
        }
        picked
    }
}

// ---------------------------------------------------------------------------
// EliteInfo — l'exposition JSON du leader élite
// ---------------------------------------------------------------------------

/// Un mod tiré, sérialisable (JSON UI) — les % élémentaires sont les
/// constantes RÉELLES de monumod.txt pour le rang du leader.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EliteModInfo {
    pub id: u32,
    /// Token monumod (`strong`, `cold`…).
    pub name: String,
    /// Libellé fr-FR résolu depuis les TBL réelles (repli : nom EN ; les
    /// variantes [ms]…[fs]… des TBL sont réduites à la forme masculine).
    pub display: String,
    /// Code d'effet stable ([`effect_code`]).
    pub effect: String,
    /// % des dégâts de base ajoutés en élément [min, max] — 0 si le mod
    /// n'ajoute pas d'élément (champion 33/50, unique 66/100 — constants).
    pub elem_dmg_pct: [u32; 2],
}

/// État élite d'un leader de pack — le contrat JSON étape 9b
/// (`monstre doré/bleu côté UI plus tard`).
///
/// Consommation prévue côté moteur (additive) :
/// `hp = hp_base × hp_mult / 100` ([`apply_hp`]),
/// `dmg = dmg_base × dmg_mult / 100` ([`apply_dmg`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EliteInfo {
    /// Mods tirés — 1 pour un champion, 3 distincts pour un unique.
    pub mods: Vec<EliteModInfo>,
    /// Multiplicateur HP en % (300 = ×3, 250 = ×2,5…).
    pub hp_mult: u32,
    /// Multiplicateur dégâts en % (225 = ×2,25 — inclut la pénalité
    /// `DifficultyLevels.UniqueDamageBonus`).
    pub dmg_mult: u32,
    /// Teinte D2 (convention `Utrans`) : 0 = aucune (champion — le bleu UI
    /// vient du rang), sinon une valeur réelle du pool superuniques.txt.
    pub tint_id: u32,
    /// Rang (`"champion"`/`"unique"`) — champ additionnel pour l'UI
    /// doré/bleu, hors du contrat minimal.
    pub rank: String,
}

/// Applique le multiplicateur HP d'une élite (`hp_base × hp_mult / 100`).
pub fn apply_hp(hp_base: u32, info: &EliteInfo) -> u32 {
    hp_base.saturating_mul(info.hp_mult) / 100
}

/// Applique le multiplicateur dégâts d'une élite.
pub fn apply_dmg(dmg_base: u32, info: &EliteInfo) -> u32 {
    dmg_base.saturating_mul(info.dmg_mult) / 100
}

/// Multiplicateur HP (%) d'un rang pour la difficulté —
/// `100 + monumod["champion|unique +hp%"][diff]` (données réelles).
pub fn hp_mult_pct(rank: EliteRank, diff: Difficulty, c: &EliteConstants) -> u32 {
    let boost = match rank {
        EliteRank::Champion => c.champion_hp_pct[diff.index()],
        EliteRank::Unique => c.unique_hp_pct[diff.index()],
    };
    100 + boost
}

/// Multiplicateur dégâts (%) d'un rang pour la difficulté —
/// `(100 + monumod["champion|unique +dmg%"]) × DifficultyLevels[
/// Champion|Unique]DamageBonus / 100` (division entière, plancher).
pub fn dmg_mult_pct(rank: EliteRank, diff: Difficulty, c: &EliteConstants, dlv: &DifficultyLevels) -> u32 {
    let (boost, bonus) = match rank {
        EliteRank::Champion => (c.champion_dmg_pct, dlv.row(diff).champion_damage_bonus),
        EliteRank::Unique => (c.unique_dmg_pct, dlv.row(diff).unique_damage_bonus),
    };
    (100 + boost) * bonus / 100
}

/// Libellé fr depuis les TBL réelles — réduit les variantes
/// `[ms]masculin[fs]féminin` à la forme masculine (Ghostly → fantômatique).
fn fr_display(m: &EliteModDef) -> String {
    if m.tbl_key.is_empty() {
        return m.display_en.clone();
    }
    let raw = crate::tbl_names::translate(&m.tbl_key, &m.display_en);
    match raw.strip_prefix("[ms]") {
        Some(rest) => rest.split("[fs]").next().unwrap_or(&raw).to_string(),
        None => raw,
    }
}

impl EliteMods {
    /// Construit l'[`EliteInfo`] d'un leader avec la table difficultylevels
    /// embarquée de d2rules ([`d2rules::difficulty::embedded`]).
    pub fn build(
        &self,
        rank: EliteRank,
        diff: Difficulty,
        class: Option<&str>,
        rng: &mut SplitMix64,
    ) -> EliteInfo {
        self.build_with(rank, diff, class, &d2rules::difficulty::embedded(), rng)
    }

    /// Constructeur pur avec `DifficultyLevels` explicite (tests, hôtes qui
    /// fournissent leur propre table difficultylevels).
    pub fn build_with(
        &self,
        rank: EliteRank,
        diff: Difficulty,
        class: Option<&str>,
        dlv: &DifficultyLevels,
        rng: &mut SplitMix64,
    ) -> EliteInfo {
        let n = match rank {
            EliteRank::Champion => 1,
            EliteRank::Unique => 3,
        };
        let (elem_pct, tint) = match rank {
            EliteRank::Champion => (self.constants.champion_elem_dmg_pct, 0u32),
            EliteRank::Unique => (
                self.constants.unique_elem_dmg_pct,
                self.utrans_pool
                    .get(rng.below(self.utrans_pool.len()))
                    .copied()
                    .unwrap_or(0),
            ),
        };
        let mods: Vec<EliteModInfo> = self
            .pick_mods(rank, diff, class, rng, n)
            .into_iter()
            .map(|m| {
                let effect = if m.effect == EliteEffect::Aura {
                    let aura = AURA_POOL[rng.below(AURA_POOL.len())];
                    format!("aura ({aura})")
                } else {
                    effect_code(m.effect).to_string()
                };
                let elem = match m.effect {
                    EliteEffect::Elemental { .. } => elem_pct,
                    _ => [0, 0],
                };
                EliteModInfo {
                    id: m.id,
                    name: m.token.clone(),
                    display: fr_display(m),
                    effect,
                    elem_dmg_pct: elem,
                }
            })
            .collect();
        EliteInfo {
            mods,
            hp_mult: hp_mult_pct(rank, diff, &self.constants),
            dmg_mult: dmg_mult_pct(rank, diff, &self.constants, dlv),
            tint_id: tint,
            rank: rank.as_str().to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// Overlay additif sur un plan de spawn existant
// ---------------------------------------------------------------------------

/// Nombre d'uniques garantis d'un niveau pour la difficulté —
/// `MonUMin..MonUMax` (colonnes `(N)`/`(H)` réelles de levels.txt ; cellules
/// vides ⇒ 0 : Blood Moor Normal n'a AUCUN unique garanti, data fait foi).
pub fn roll_unique_count(
    rec: &LevelRecord,
    diff: Difficulty,
    rng: &mut SplitMix64,
) -> u32 {
    let (_, _, _, u_min, u_max) = rec.diff_stats(diff);
    if u_max == 0 && u_min == 0 {
        return 0;
    }
    (u_min + rng.below((u_max.saturating_sub(u_min) + 1) as usize) as u32).max(u_min)
}

/// Overlay élite ADDITIF sur un plan existant ([`LevelSpawnPlan`] produit
/// par `monsters.rs::plan_level_spawns_diff`) : renvoie, PARALLÈLE à
/// `plan.packs`, l'[`EliteInfo`] de chaque leader (None = monstre normal).
///
/// * uniques garantis : [`roll_unique_count`] (MonUMin/MonUMax de la
///   difficulté), posés sur les premiers packs — placement arbitraire mais
///   déterministe (les positions retail vivent côté DRLG/MonPreset) ;
/// * champions : roue `champion chance` (20, convention % du port) par pack
///   restant ;
/// * `SuperUnique` : hors périmètre (les nommés ont déjà leurs mods
///   `SuperUniques.Mod1-3` côté `monsters.rs`).
///
/// Zéro mutation du plan : la couche existante reste inchangée, la couche
/// 9b se superpose (consommation JSON/UI puis moteur plus tard).
pub fn elite_overlay(
    plan: &LevelSpawnPlan,
    rec: &LevelRecord,
    diff: Difficulty,
    mods: &EliteMods,
    dlv: &DifficultyLevels,
    rng: &mut SplitMix64,
) -> Vec<Option<EliteInfo>> {
    let mut out: Vec<Option<EliteInfo>> = Vec::with_capacity(plan.packs.len());
    let n_uniques = (roll_unique_count(rec, diff, rng) as usize).min(plan.packs.len());
    for (i, pack) in plan.packs.iter().enumerate() {
        let info = if i < n_uniques {
            Some(mods.build_with(EliteRank::Unique, diff, Some(&pack.mon_id), dlv, rng))
        } else if rng.below(100) < mods.constants.champion_chance as usize {
            Some(mods.build_with(EliteRank::Champion, diff, Some(&pack.mon_id), dlv, rng))
        } else {
            None
        };
        out.push(info);
    }
    out
}

// ---------------------------------------------------------------------------
// Tests — tables réelles, multipliers golden, distribution Blood Moor
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compose::Lcg;
    use crate::levels::LevelTables;
    use crate::monsters::{MonLvl, MonStats, SuperUniques, plan_level_spawns_diff};

    const MONSTATS: &str = include_str!("../../../../assets/patch_monstats.txt");
    const MONUMOD: &str = include_str!("../../../../assets/monumod.txt");
    const SUPERUNIQUES: &str = include_str!("../../../../assets/superuniques.txt");
    const MONLVL: &str = include_str!("../../../../assets/MonLvl.txt");
    const LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");

    fn mods() -> EliteMods {
        let mut m = EliteMods::parse(MONUMOD).unwrap();
        m.load_utrans_pool(SUPERUNIQUES);
        m
    }

    fn dlv() -> DifficultyLevels {
        DifficultyLevels::parse(include_str!(
            "../../../../assets/patch_difficultylevels.txt"
        ))
        .unwrap()
    }

    fn bloodmoor_plan(diff: Difficulty, seed: u64) -> (LevelSpawnPlan, LevelRecord) {
        let ms = MonStats::parse(MONSTATS).unwrap();
        let mu = crate::monsters::MonUMods::parse(MONUMOD).unwrap();
        let su = SuperUniques::parse(SUPERUNIQUES).unwrap();
        let ml = MonLvl::parse(MONLVL).unwrap();
        let lt = LevelTables::parse(LEVELS).unwrap();
        let bm = lt.get(2).unwrap().clone();
        let resolve = |code: &str| -> Option<String> {
            ["FA", "ZM", "SI", "SK", "FS", "YE"]
                .iter()
                .find(|c| **c == code)
                .map(|c| c.to_string())
        };
        (plan_level_spawns_diff(&bm, &ms, &mu, &ml, &su, diff, &mut Lcg::new(seed), resolve), bm)
    }

    /// (1) Lecture du catalogue monumod réel via `d2rules::tsv`.
    #[test]
    fn parses_real_monumod_catalog() {
        let m = mods();
        assert_eq!(m.mods.len(), 43, "43 records réels");
        let by = |t: &str| m.mods.iter().find(|r| r.token == t).unwrap().clone();
        // Pools réels par difficulté (colonnes cpick/upick brutes).
        let strong = by("strong");
        assert_eq!(strong.unique_pool, [6, 6, 6], "strong : unique N/NM/H");
        assert_eq!(strong.champion_pool, [0, 0, 0], "strong JAMAIS champion (cpick vide)");
        assert_eq!(strong.effect, EliteEffect::PhysicalDamage);
        let champion = by("champion");
        assert_eq!(champion.champion_pool, [1, 1, 1], "pool champion réel");
        assert_eq!(champion.unique_pool, [0, 0, 0]);
        // resist : NM/Enfer SEULEMENT (cellule Normal vide — data fait foi).
        assert_eq!(by("resist").unique_pool, [0, 6, 6]);
        // Exclusion de classe réelle : lightning exclut sandleaper.
        assert_eq!(by("lightning").exclude[0], "sandleaper");
        // Variantes champion 1.10+ (version=100, présentes en 1.12).
        for t in ["ghostly", "fanatic", "possessed", "berserk"] {
            assert_eq!(by(t).version, 100, "{t} : version=100 (1.10+)");
            assert_eq!(by(t).effect, EliteEffect::ChampionVariant);
        }
        // Constantes officielles (colonne constants × *constant desc, N/NM/H).
        let c = &m.constants;
        assert_eq!(c.champion_chance, 20);
        assert_eq!(c.champion_hp_pct, [200, 150, 100], "champion +hp% N/NM/H");
        assert_eq!(c.unique_hp_pct, [300, 200, 100], "unique +hp% N/NM/H");
        assert_eq!(c.minion_hp_pct, [100, 75, 50]);
        assert_eq!(c.champion_dmg_pct, 100, "champion +dmg%");
        assert_eq!(c.unique_dmg_pct, 150, "unique +dmg% (strong)");
        assert_eq!(c.minion_dmg_pct, 75);
        assert_eq!((c.champion_th_pct, c.unique_th_pct, c.minion_th_pct), (75, 100, 50));
        assert_eq!(c.champion_elem_dmg_pct, [33, 50], "champion +elem min/max");
        assert_eq!(c.unique_elem_dmg_pct, [66, 100], "unique +elem min/max");
        assert_eq!(c.minion_elem_dmg_pct, [0, 50, 50]);
        // Pool de teintes réelles (superuniques.txt Utrans distincts).
        assert_eq!(m.utrans_pool.len(), 31, "31 teintes distinctes 4..38 (6, 32-34 absentes)");
        assert_eq!(*m.utrans_pool.first().unwrap(), 4);
        assert_eq!(*m.utrans_pool.last().unwrap(), 38);
        assert!(!m.utrans_pool.contains(&255), "255 = « pas de teinte » (monstats2), exclu");
    }

    /// (2) Multiplicateurs golden — sources : monumod constants ×
    /// DifficultyLevels (90/75/66).
    #[test]
    fn golden_multipliers_per_rank_and_difficulty() {
        let m = mods();
        let d = dlv();
        let cases: &[(EliteRank, Difficulty, u32, u32)] = &[
            // (rang, difficulté, hp_mult, dmg_mult)
            (EliteRank::Champion, Difficulty::Normal, 300, 180), // ×3 ; ×2 × 0.90
            (EliteRank::Champion, Difficulty::Nightmare, 250, 150), // ×2,5 ; ×2 × 0.75
            (EliteRank::Champion, Difficulty::Hell, 200, 132), // ×2 ; ×2 × 0.66
            (EliteRank::Unique, Difficulty::Normal, 400, 225), // ×4 ; ×2,5 × 0.90
            (EliteRank::Unique, Difficulty::Nightmare, 300, 187), // ×3 ; ×2,5 × 0.75 = 187,5 → plancher
            (EliteRank::Unique, Difficulty::Hell, 200, 165), // ×2 ; ×2,5 × 0.66
        ];
        for &(rank, diff, hp, dmg) in cases {
            assert_eq!(hp_mult_pct(rank, diff, &m.constants), hp, "{rank:?} {diff:?} HP");
            assert_eq!(dmg_mult_pct(rank, diff, &m.constants, &d), dmg, "{rank:?} {diff:?} DMG");
            let mut rng = SplitMix64::new(1);
            let info = m.build_with(rank, diff, None, &d, &mut rng);
            assert_eq!(info.hp_mult, hp);
            assert_eq!(info.dmg_mult, dmg);
            assert_eq!(info.rank, rank.as_str());
        }
        // Application additive aux stats d'un monstre spawné.
        let mut rng = SplitMix64::new(2);
        let info = m.build_with(EliteRank::Unique, Difficulty::Normal, None, &d, &mut rng);
        assert_eq!(apply_hp(100, &info), 400);
        assert_eq!(apply_dmg(40, &info), 90);
    }

    /// (3) Tirage : pools par difficulté, 1 mod champion / 3 uniques
    /// distincts, exclusion lightning/sandleaper respectée.
    #[test]
    fn picking_respects_pools_counts_and_exclusions() {
        let m = mods();
        // Champion : pool réel {champion, ghostly, fanatic, possessed, berserk}.
        let mut rng = SplitMix64::new(7);
        for _ in 0..100 {
            let got = m.pick_mods(EliteRank::Champion, Difficulty::Normal, None, &mut rng, 1);
            assert_eq!(got.len(), 1);
            assert!(
                ["champion", "ghostly", "fanatic", "possessed", "berserk"]
                    .contains(&got[0].token.as_str()),
                "mod champion hors pool réel : {}",
                got[0].token
            );
        }
        // Unique : 3 distincts du pool upick de la difficulté.
        for diff in [Difficulty::Normal, Difficulty::Nightmare, Difficulty::Hell] {
            let mut rng = SplitMix64::new(8);
            for _ in 0..50 {
                let got = m.pick_mods(EliteRank::Unique, diff, None, &mut rng, 3);
                assert_eq!(got.len(), 3);
                let names: Vec<&str> = got.iter().map(|r| r.token.as_str()).collect();
                let unique_set = names.len();
                assert_eq!(unique_set, 3, "mods uniques distincts : {names:?}");
                for t in &names {
                    assert!(m.mods.iter().any(|r| r.token == *t && r.unique_pool[diff.index()] > 0),
                            "{t} hors pool unique {diff:?}");
                }
                if diff == Difficulty::Normal {
                    assert!(!names.contains(&"resist"), "resist : NM/Enfer seulement (upick Normal vide)");
                }
            }
        }
        // Exclusion réelle : sandleaper ne tombe JAMAIS lightning.
        let mut rng = SplitMix64::new(9);
        for _ in 0..300 {
            let got = m.pick_mods(EliteRank::Unique, Difficulty::Hell, Some("sandleaper"), &mut rng, 3);
            assert!(!got.iter().any(|r| r.token == "lightning"), "exclude1=sandleaper violé");
        }
        // Témoin positif : sans classe, lightning tombe bien.
        let mut rng = SplitMix64::new(9);
        let mut seen = false;
        for _ in 0..300 {
            let got = m.pick_mods(EliteRank::Unique, Difficulty::Hell, None, &mut rng, 3);
            if got.iter().any(|r| r.token == "lightning") {
                seen = true;
                break;
            }
        }
        assert!(seen, "lightning doit rester tirable hors exclusion");
    }

    /// (4) Distribution Blood Moor réelle : MonUMin/Max 0/0 → 4/5 → 7/9
    /// (`patch_levels.txt` id 2), overlay déterministe, JSON propre.
    #[test]
    fn bloodmoor_elite_distribution_is_real_and_deterministic() {
        let m = mods();
        let d = dlv();
        // Normal : 0 unique garanti (cellules vides) — data fait foi.
        assert_eq!(roll_unique_count(&bloodmoor_plan(Difficulty::Normal, 1).1, Difficulty::Normal, &mut SplitMix64::new(3)), 0);
        // NM/Hell : bornes réelles.
        let bm_n = bloodmoor_plan(Difficulty::Nightmare, 1).1;
        let bm_h = bloodmoor_plan(Difficulty::Hell, 1).1;
        for _ in 0..200 {
            let n = roll_unique_count(&bm_n, Difficulty::Nightmare, &mut SplitMix64::new(4));
            assert!((4..=5).contains(&n), "MonUMin/Max(N) = 4/5, eu {n}");
            let h = roll_unique_count(&bm_h, Difficulty::Hell, &mut SplitMix64::new(5));
            assert!((7..=9).contains(&h), "MonUMin/Max(H) = 7/9, eu {h}");
        }
        // Overlay sur les plans réels de chaque difficulté.
        for (diff, u_lo, u_hi) in [
            (Difficulty::Normal, 0usize, 0usize),
            (Difficulty::Nightmare, 4, 5),
            (Difficulty::Hell, 7, 9),
        ] {
            let (plan, bm) = bloodmoor_plan(diff, 42);
            let ov = elite_overlay(&plan, &bm, diff, &m, &d, &mut SplitMix64::new(77));
            assert_eq!(ov.len(), plan.packs.len(), "overlay parallèle aux packs");
            let uniques = ov.iter().filter(|o| matches!(o, Some(i) if i.rank == "unique")).count();
            assert!((u_lo..=u_hi).contains(&uniques), "{diff:?} : uniques {u_lo}..={u_hi}, eu {uniques}");
            let champs = ov.iter().filter(|o| matches!(o, Some(i) if i.rank == "champion")).count();
            assert!(champs <= plan.packs.len() - uniques);
            for (pack, info) in plan.packs.iter().zip(&ov) {
                let Some(info) = info else { continue };
                let want = match info.rank.as_str() {
                    "champion" => 1,
                    _ => 3,
                };
                assert_eq!(info.mods.len(), want, "{} mods", info.rank);
                // Teinte : champion 0, unique dans le pool réel.
                if info.rank == "champion" {
                    assert_eq!(info.tint_id, 0, "champion : pas de palette (réel)");
                } else {
                    assert!(m.utrans_pool.contains(&info.tint_id), "teinte {} hors pool réel", info.tint_id);
                }
                // Multiplicateurs golden par rang × difficulté.
                let rank = if info.rank == "champion" { EliteRank::Champion } else { EliteRank::Unique };
                assert_eq!(info.hp_mult, hp_mult_pct(rank, diff, &m.constants));
                assert_eq!(info.dmg_mult, dmg_mult_pct(rank, diff, &m.constants, &d));
                // % élémentaires réels sur un mod Enchanted éventuel.
                for modd in &info.mods {
                    let elem = if rank == EliteRank::Unique { [66, 100] } else { [33, 50] };
                    if modd.effect.starts_with("elem_") {
                        assert_eq!(modd.elem_dmg_pct, elem, "{} elem %", modd.name);
                    } else {
                        assert_eq!(modd.elem_dmg_pct, [0, 0]);
                    }
                }
            }
            // Déterminisme : même graine ⇒ même overlay (JSON strict).
            let again = elite_overlay(&plan, &bm, diff, &m, &d, &mut SplitMix64::new(77));
            let js = |v: &Vec<Option<EliteInfo>>| serde_json::to_string(v).unwrap();
            assert_eq!(js(&ov), js(&again), "{diff:?} : même graine ⇒ même overlay");
        }
        // JSON : le contrat sérialise (champ hp_mult visible en clair).
        let mut rng = SplitMix64::new(11);
        let info = m.build_with(EliteRank::Unique, Difficulty::Hell, None, &d, &mut rng);
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("\"hp_mult\":200"), "unique Hell ×2 : {json}");
        assert!(json.contains("\"dmg_mult\":165"));
        assert!(json.contains("\"rank\":\"unique\""));
        assert!(json.contains("\"tint_id\":"));
    }

    /// (5) Libellés fr-FR depuis les TBL réelles (string.tsv /
    /// expansionstring.tsv embarquées) + marqueurs [ms]/[fs] réduits.
    #[test]
    fn fr_display_names_come_from_real_tbl() {
        let m = mods();
        let d = dlv();
        let mut rng = SplitMix64::new(13);
        for _ in 0..200 {
            let info = m.build_with(EliteRank::Champion, Difficulty::Normal, None, &d, &mut rng);
            let disp = &info.mods[0].display;
            assert!(
                ["Le Champion", "fantômatique", "fanatique", "possédé", "berserker"].contains(&disp.as_str()),
                "libellé champion inattendu : {disp}"
            );
        }
        // Clés réelles : coquille Blizzard « uniquextrastrong » comprise.
        assert_eq!(fr_display(m.mods.iter().find(|r| r.token == "fire").unwrap()), "Envoutement Feu");
        assert_eq!(fr_display(m.mods.iter().find(|r| r.token == "strong").unwrap()), "Très fort");
        assert_eq!(fr_display(m.mods.iter().find(|r| r.token == "fast").unwrap()), "Très rapide");
        assert_eq!(fr_display(m.mods.iter().find(|r| r.token == "resist").unwrap()), "Résist. Magie");
        assert_eq!(fr_display(m.mods.iter().find(|r| r.token == "cold").unwrap()), "Envoutement froid");
        assert_eq!(fr_display(m.mods.iter().find(|r| r.token == "ghostly").unwrap()), "fantômatique");
        // Aura : l'effet porte une aura nommée de la liste communauté.
        let mut rng = SplitMix64::new(17);
        let mut seen = false;
        for _ in 0..500 {
            let info = m.build_with(EliteRank::Unique, Difficulty::Hell, None, &d, &mut rng);
            if let Some(a) = info.mods.iter().find(|x| x.name == "aura") {
                assert!(a.effect.starts_with("aura (") && a.effect.ends_with(')'), "{}", a.effect);
                seen = true;
                break;
            }
        }
        assert!(seen, "aura doit tomber sur 500 uniques (1/12 packs de 3 mods)");
    }

    /// (6) Entry point embarqué = mêmes données que le parse explicite.
    #[test]
    fn embedded_matches_explicit_parse() {
        let e = EliteMods::embedded();
        let x = mods();
        assert_eq!(e.mods.len(), x.mods.len());
        assert_eq!(e.constants, x.constants);
        assert_eq!(e.utrans_pool, x.utrans_pool);
    }
}
