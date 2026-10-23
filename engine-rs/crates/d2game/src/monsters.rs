//! Contenu monstre data-driven v1.12 — W06/W07/W08 + super-uniques + boss
//! (items 6, brief `impl-monstres-boss` ; référentiel `finish-monde.md`
//! W06-W11).
//!
//! Ce module lit les 4 tables réelles (assets extraits des MPQ v1.12) et
//! produit un **plan de spawn** pur par niveau :
//!
//! * `patch_monstats.txt` (735 records) — stats brutes, packs `MinGrp`/`MaxGrp`,
//!   rareté, TC drop normal/champion/unique (`TreasureClass1/2/3`), code sprite.
//! * `monumod.txt` (43 mods) — mods champion/unique pondérés (`cpick`/`upick`)
//!   et **les constantes officielles du moteur** (colonne `constants` par
//!   `*constant desc`) : chance champion 20 %, boosts HP/dégâts/toucher.
//! * `superuniques.txt` (67 records) — boss nommés (Bishibosh, Rakanishu…),
//!   class monstats, mods `Mod1-3`, groupe, TC dédiée.
//! * `MonLvl.txt` (110 lignes) — facteurs effectifs par niveau de monstre
//!   (AC/TH/HP/DM/XP, colonnes Normal) : la ligne 1 redonne EXACTEMENT les
//!   constantes historiques du port.
//!
//! Le plan ([`plan_level_spawns`]) est déterministe au RNG fourni (graine par
//! clé de niveau côté `game.rs`) : planche `mon1-10` pondérée par `Rarity`,
//! packs `MinGrp`..`MaxGrp`, densité `MonDen × SizeX × SizeY / 100 000`,
//! leaders champion (roue monumod) / uniques (`MonUMin`..`MonUMax`),
//! super-uniques de la planche en doré nommé, TC effectives par rang.
//!
//! Zéro IO : les textes arrivent en `&str` (assets fournis via
//! `GameCore::provide`).

use crate::compose::Lcg;
use crate::levels::{each_row, LevelRecord};
use d2rules::{MonKind, SpawnRule};
use std::collections::HashMap;

/// Cible de population maximale d'un niveau (garde-fou perf A*/rendu ;
/// Blood Moor retail ≈ 33, on laisse de la marge).
pub const MAX_SPAWN_TARGET: u32 = 64;

// ---------------------------------------------------------------------------
// patch_monstats.txt (W07)
// ---------------------------------------------------------------------------

/// Un record de monstats.txt — colonnes consommées par le plan de spawn.
#[derive(Debug, Clone, Default)]
pub struct MonStatsRec {
    /// Id table (`zombie1`) — la clé des planches mon1-10.
    pub id: String,
    /// Code sprite (`ZM`) — résolu vers le manifest par `game.rs`.
    pub code: String,
    /// Nom affichable (`NameStr`).
    pub name: String,
    pub level: u32,
    pub hp_min: u32,
    pub hp_max: u32,
    pub dmg_min: u32,
    pub dmg_max: u32,
    pub exp: u32,
    /// Vitesse (`Velocity`).
    pub speed: f32,
    /// TC drop [normal, champion, unique] (`TreasureClass1/2/3`).
    pub tc: [String; 3],
    /// Pack (`MinGrp`..`MaxGrp`, 0 ⇒ 1).
    pub min_grp: u32,
    pub max_grp: u32,
    /// Poids de tirage dans la planche (`Rarity`, 0 ⇒ 1).
    pub rarity: u32,
}

/// La table monstats (735 records v1.12), indexée par Id.
#[derive(Debug, Clone, Default)]
pub struct MonStats {
    recs: Vec<MonStatsRec>,
    index: HashMap<String, usize>,
}

impl MonStats {
    pub fn parse(tsv: &str) -> Result<MonStats, String> {
        let mut t = MonStats::default();
        each_row(tsv, |row| {
            let id = row.s("Id");
            if id.is_empty() || row.s("Code").is_empty() {
                return;
            }
            let tc = |n: &str| row.s(n).to_string();
            let rec = MonStatsRec {
                id: id.to_string(),
                code: row.s("Code").to_string(),
                name: row.s("NameStr").to_string(),
                level: row.u("Level"),
                hp_min: row.u("minHP"),
                hp_max: row.u("maxHP"),
                dmg_min: row.u("A1MinD"),
                dmg_max: row.u("A1MaxD"),
                exp: row.u("Exp"),
                speed: row.i("Velocity").max(0) as f32,
                tc: [tc("TreasureClass1"), tc("TreasureClass2"), tc("TreasureClass3")],
                min_grp: row.u("MinGrp"),
                max_grp: row.u("MaxGrp"),
                rarity: row.u("Rarity"),
            };
            t.index.insert(id.to_string(), t.recs.len());
            t.recs.push(rec);
        });
        if t.recs.len() < 100 {
            return Err(format!("monstats.txt : {} records seulement", t.recs.len()));
        }
        Ok(t)
    }

    pub fn len(&self) -> usize {
        self.recs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.recs.is_empty()
    }

    pub fn get(&self, id: &str) -> Option<&MonStatsRec> {
        self.index.get(id).map(|&i| &self.recs[i])
    }
}

// ---------------------------------------------------------------------------
// monumod.txt (W08) — mods + constantes officielles
// ---------------------------------------------------------------------------

/// Un modificateur champion/unique (`monumod.txt`).
#[derive(Debug, Clone, Default)]
pub struct MonUModRec {
    /// Id numérique de la table (référencé par `SuperUniques.Mod1-3`).
    pub id: u32,
    pub name: String,
    /// Peut tomber sur un CHAMPION (colonne `champion`).
    pub champion: bool,
    /// Poids de tirage champion [Normal, N, H] (`cpick`).
    pub cpick: [u32; 3],
    /// Poids de tirage unique [Normal, N, H] (`upick`).
    pub upick: [u32; 3],
}

/// Constantes moteur lues dans `monumod.txt` (colonne `constants` indexée par
/// `*constant desc`). Valeurs = fallback identiques si la colonne manque.
#[derive(Debug, Clone)]
pub struct UModConstants {
    /// Chance (%, roue /100) qu'un pack ait un leader champion.
    pub champion_chance: u32,
    /// Champion : +% HP (200 ⇒ ×3).
    pub champion_hp_pct: u32,
    /// Champion : +% toucher (75 ⇒ ×1,75).
    pub champion_th_pct: u32,
    /// Champion : +% dégâts (100 ⇒ ×2).
    pub champion_dmg_pct: u32,
    /// Unique : +% HP (300 ⇒ ×4).
    pub unique_hp_pct: u32,
    /// Unique : +% toucher (100 ⇒ ×2).
    pub unique_th_pct: u32,
    /// Unique : +% dégâts (150 ⇒ ×2,5).
    pub unique_dmg_pct: u32,
    /// Minions d'un pack champion/unique : +% HP (100 ⇒ ×2).
    pub minion_hp_pct: u32,
    /// Multiplicateurs XP — PAS dans monumod.txt : valeurs communauté 1.10+
    /// (champion ×3, unique ×5), assumées et documentées.
    pub champion_xp_mult: u32,
    pub unique_xp_mult: u32,
}

impl Default for UModConstants {
    fn default() -> UModConstants {
        UModConstants {
            champion_chance: 20,
            champion_hp_pct: 200,
            champion_th_pct: 75,
            champion_dmg_pct: 100,
            unique_hp_pct: 300,
            unique_th_pct: 100,
            unique_dmg_pct: 150,
            minion_hp_pct: 100,
            champion_xp_mult: 3,
            unique_xp_mult: 5,
        }
    }
}

/// Table monumod + constantes.
#[derive(Debug, Clone, Default)]
pub struct MonUMods {
    pub mods: Vec<MonUModRec>,
    pub constants: UModConstants,
}

impl MonUMods {
    pub fn parse(tsv: &str) -> Result<MonUMods, String> {
        let mut m = MonUMods { mods: Vec::new(), constants: UModConstants::default() };
        each_row(tsv, |row| {
            let name = row.s("uniquemod");
            if name.is_empty() {
                return;
            }
            m.mods.push(MonUModRec {
                id: row.u("id"),
                name: name.to_string(),
                champion: row.i("champion") == 1,
                cpick: [row.u("cpick"), row.u("cpick (N)"), row.u("cpick (H)")],
                upick: [row.u("upick"), row.u("upick (N)"), row.u("upick (H)")],
            });
            // Constantes moteur : (desc → valeur) sur la même ligne.
            let (v, desc) = (row.u("constants"), row.s("*constant desc").to_string());
            match desc.as_str() {
                "champion chance" => m.constants.champion_chance = v,
                "champion +hp%" => m.constants.champion_hp_pct = v,
                "champion +tohit%" => m.constants.champion_th_pct = v,
                "champion +dmg%" => m.constants.champion_dmg_pct = v,
                "unique +hp%" => m.constants.unique_hp_pct = v,
                "unique +tohit%" => m.constants.unique_th_pct = v,
                "unique +dmg%" => m.constants.unique_dmg_pct = v,
                "minion +hp%" => m.constants.minion_hp_pct = v,
                _ => {}
            }
        });
        if m.mods.len() < 40 {
            return Err(format!("monumod.txt : {} records seulement", m.mods.len()));
        }
        Ok(m)
    }

    /// Mod champion tiré aux poids `cpick` (difficulté 0) — `None` si aucun
    /// mod éligible (poids tous nuls).
    pub fn pick_champion_mod(&self, rng: &mut Lcg) -> Option<&str> {
        pick_weighted(
            self.mods
                .iter()
                .filter(|r| r.champion)
                .map(|r| (r.name.as_str(), r.cpick[0] as u64)),
            rng,
        )
    }

    /// Mod unique tiré aux poids `upick` (difficulté 0).
    pub fn pick_unique_mod(&self, rng: &mut Lcg) -> Option<&str> {
        pick_weighted(
            self.mods.iter().map(|r| (r.name.as_str(), r.upick[0] as u64)),
            rng,
        )
    }

    /// Nom d'un mod par id numérique (références `SuperUniques.Mod1-3`).
    pub fn name_by_id(&self, id: u32) -> Option<&str> {
        self.mods.iter().find(|r| r.id == id).map(|r| r.name.as_str())
    }
}

fn pick_weighted<'a, I: Iterator<Item = (&'a str, u64)>>(items: I, rng: &mut Lcg) -> Option<&'a str> {
    let items: Vec<(&str, u64)> = items.filter(|(_, w)| *w > 0).collect();
    let total: u64 = items.iter().map(|(_, w)| w).sum();
    if items.is_empty() || total == 0 {
        return None;
    }
    let mut roll = (rng.below(total as u32)) as u64;
    for (name, w) in items {
        if roll < w {
            return Some(name);
        }
        roll -= w;
    }
    None
}

// ---------------------------------------------------------------------------
// SuperUniques.txt (W10)
// ---------------------------------------------------------------------------

/// Un super-unique nommé (`SuperUniques.txt`).
#[derive(Debug, Clone, Default)]
pub struct SuperUniqueRec {
    /// Nom affichable (`Bishibosh`, `Rakanishu`…).
    pub name: String,
    /// Id monstats de la classe (`fallenshaman1`).
    pub class: String,
    /// Mods `Mod1-3` BRUTS (ids monumod en texte, "" = aucun) — résolus en
    /// noms dans [`SuSpawn::mods`] par le planner (table monumod dispo là).
    pub mods: Vec<String>,
    /// Groupe de minions (`MinGrp`..`MaxGrp`).
    pub min_grp: u32,
    pub max_grp: u32,
    /// Position auto (colonne `AutoPos` ; les positions réelles retail vivent
    /// dans MonPreset/DS1 — hors périmètre, cf. impl-monstres-boss).
    pub auto_pos: u32,
    /// Id de teinte palshift (`Utrans` — teintes non implémentées, statut).
    pub utrans: u32,
    /// TC de drop dédiée (`Act 1 Super A`…).
    pub tc: String,
}

#[derive(Debug, Clone, Default)]
pub struct SuperUniques {
    pub recs: Vec<SuperUniqueRec>,
}

impl SuperUniques {
    pub fn parse(tsv: &str) -> Result<SuperUniques, String> {
        let mut t = SuperUniques::default();
        each_row(tsv, |row| {
            let name = row.s("Superunique");
            if name.is_empty() {
                return;
            }
            let mod_name = |col: &str| -> String {
                let raw = row.s(col);
                if raw.is_empty() || raw == "0" {
                    String::new()
                } else {
                    raw.to_string()
                }
            };
            t.recs.push(SuperUniqueRec {
                name: name.to_string(),
                class: row.s("Class").to_string(),
                // Mod1-3 = ids monumod (bruts ; résolution par le planner).
                mods: [mod_name("Mod1"), mod_name("Mod2"), mod_name("Mod3")]
                    .iter()
                    .filter(|m| !m.is_empty())
                    .cloned()
                    .collect(),
                min_grp: row.u("MinGrp"),
                max_grp: row.u("MaxGrp"),
                auto_pos: row.u("AutoPos"),
                utrans: row.u("Utrans"),
                tc: row.s("TC").to_string(),
            });
        });
        if t.recs.len() < 60 {
            return Err(format!("SuperUniques.txt : {} records seulement", t.recs.len()));
        }
        Ok(t)
    }

    pub fn len(&self) -> usize {
        self.recs.len()
    }
}

// ---------------------------------------------------------------------------
// MonLvl.txt (W08 partiel : colonnes Normal par niveau de monstre)
// ---------------------------------------------------------------------------

/// Facteurs effectifs d'une ligne MonLvl (colonnes Normal, % pour HP/DM/XP).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MonLvlRow {
    pub ac: u32,
    pub th: u32,
    /// % HP (7 = ×7/100).
    pub hp_pct: u32,
    /// % dégâts.
    pub dm_pct: u32,
    /// % XP.
    pub xp_pct: u32,
}

/// Table MonLvl (110 lignes), indexée par niveau de monstre.
#[derive(Debug, Clone, Default)]
pub struct MonLvl {
    rows: Vec<MonLvlRow>,
}

impl MonLvl {
    pub fn parse(tsv: &str) -> Result<MonLvl, String> {
        let mut t = MonLvl::default();
        each_row(tsv, |row| {
            let Some(level) = row.s("Level").parse::<usize>().ok() else { return };
            let r = MonLvlRow {
                ac: row.u("AC"),
                th: row.u("TH"),
                hp_pct: row.u("HP"),
                dm_pct: row.u("DM"),
                xp_pct: row.u("XP"),
            };
            if t.rows.len() <= level {
                t.rows.resize(level + 1, MonLvlRow::default());
            }
            t.rows[level] = r;
        });
        if t.rows.len() < 100 {
            return Err(format!("MonLvl.txt : {} lignes seulement", t.rows.len()));
        }
        Ok(t)
    }

    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// Ligne d'un niveau (clamp aux bornes ; vide ⇒ repli niveau 1 historique).
    pub fn row(&self, level: u32) -> MonLvlRow {
        if self.rows.is_empty() {
            return MonLvlRow { ac: 6, th: 8, hp_pct: 7, dm_pct: 2, xp_pct: 30 };
        }
        let i = (level as usize).min(self.rows.len() - 1).max(0);
        let r = self.rows[i];
        if r == MonLvlRow::default() && i != 0 {
            // ligne absente (trou) : repli niveau 1
            self.rows[1.min(self.rows.len() - 1)]
        } else {
            r
        }
    }
}

// ---------------------------------------------------------------------------
// Plan de spawn (W06/W07/W08 + super-uniques)
// ---------------------------------------------------------------------------

/// Un pack : leader (+ minions) du même monstats, posé autour d'un spot.
#[derive(Debug, Clone)]
pub struct PackSpawn {
    /// Code sprite manifest résolu (`FA`, `ZM`…).
    pub code: String,
    /// Id monstats d'origine (debug/E2E).
    pub mon_id: String,
    /// Taille du pack (leader compris) — `MinGrp`..`MaxGrp`.
    pub count: u32,
    /// Règle du LEADER (kind + boosts + TC effective).
    pub rule: SpawnRule,
    /// Boost HP des minions du pack (0 = aucun).
    pub minion_hp_pct: u32,
    /// Mod monumod tiré pour le leader ("" = aucun).
    pub mod_name: String,
}

/// Un super-unique nommé + ses minions.
#[derive(Debug, Clone)]
pub struct SuSpawn {
    /// Nom affichable (`Rakanishu`).
    pub name: String,
    /// Code sprite manifest résolu.
    pub code: String,
    /// Id monstats de la classe.
    pub mon_id: String,
    /// Taille du groupe (leader compris).
    pub count: u32,
    /// Règle du leader (kind SuperUnique, boosts uniques, TC dédiée).
    pub rule: SpawnRule,
    /// Boost HP des minions.
    pub minion_hp_pct: u32,
    /// Mods `Mod1-3` (noms monumod).
    pub mods: Vec<String>,
}

/// Plan de spawn pur d'un niveau — d2game le consomme (positions/tuiles),
/// `d2rules` reçoit les règles via `spawn_monster_rule`.
#[derive(Debug, Clone, Default)]
pub struct LevelSpawnPlan {
    pub packs: Vec<PackSpawn>,
    pub super_uniques: Vec<SuSpawn>,
    /// Population cible (formule densité, avant arrondi packs).
    pub target: u32,
}

/// Population cible d'un niveau : `MonDen × SizeX × SizeY / 100 000`
/// (Blood Moor : 520 × 80 × 80 / 100 000 = 33), plafonnée à
/// [`MAX_SPAWN_TARGET`].
pub fn density_target(mon_den: u32, size_x: u32, size_y: u32) -> u32 {
    (mon_den.saturating_mul(size_x).saturating_mul(size_y) / 100_000).min(MAX_SPAWN_TARGET)
}

/// Construit le plan de spawn data-driven d'un niveau.
///
/// * `resolve_code` : monstats `Code` → code sprite manifest (`None` = pas de
///   sprite : l'entrée de planche est ignorée) ;
/// * `rng` : RNG de niveau (déterminisme par clé côté appelant).
///
/// Le niveau sans planche ou sans densité renvoie un plan vide (l'appelant
/// garde son repli historique).
pub fn plan_level_spawns(
    rec: &LevelRecord,
    monstats: &MonStats,
    monumod: &MonUMods,
    monlvl: &MonLvl,
    superuniques: &SuperUniques,
    rng: &mut Lcg,
    mut resolve_code: impl FnMut(&str) -> Option<String>,
) -> LevelSpawnPlan {
    let mut plan = LevelSpawnPlan::default();
    if rec.mon.is_empty() || rec.mon_den == 0 {
        return plan;
    }
    plan.target = density_target(rec.mon_den, rec.size_x.max(1), rec.size_y.max(1));

    // Planche pondérée (Rarity, 0 ⇒ 1) restreinte aux codes à sprite.
    struct Entry<'a> {
        rec: &'a MonStatsRec,
        code: String,
        weight: u64,
    }
    let entries: Vec<Entry<'_>> = rec
        .mon
        .iter()
        .filter_map(|id| {
            let r = monstats.get(id)?;
            let code = resolve_code(&r.code)?;
            Some(Entry { rec: r, code, weight: r.rarity.max(1) as u64 })
        })
        .collect();
    if entries.is_empty() {
        return plan; // aucun sprite : l'appelant garde son repli
    }
    let total_weight: u64 = entries.iter().map(|e| e.weight).sum();

    // Uniques garantis du niveau (MonUMin..MonUMax ; vide ⇒ 0 — data fait foi).
    let u_target = if rec.mon_u_max > 0 || rec.mon_u_min > 0 {
        rng.below(rec.mon_u_max.saturating_sub(rec.mon_u_min) + 1) + rec.mon_u_min
    } else {
        0
    };
    let c = &monumod.constants;

    let mut placed: u32 = 0;
    let mut uniques: u32 = 0;
    let mut guard = 0u32;
    while placed < plan.target && guard < 500 {
        guard += 1;
        // Planche : tirage pondéré.
        let mut roll = rng.below(total_weight as u32) as u64;
        let entry = entries
            .iter()
            .find(|e| {
                if roll < e.weight {
                    true
                } else {
                    roll -= e.weight;
                    false
                }
            })
            .expect("total_weight > 0");
        let r = entry.rec;
        // Pack MinGrp..MaxGrp (0 ⇒ 1).
        let (lo, hi) = (r.min_grp.max(1), r.max_grp.max(r.min_grp.max(1)));
        let count = rng.below(hi - lo + 1) + lo;
        // Rang du leader : unique garanti d'abord, sinon roue champion.
        let (kind, mod_name, rule) = if uniques < u_target {
            uniques += 1;
            let base = monlvl.row(r.level);
            let mut rule = SpawnRule {
                kind: MonKind::Unique,
                hp_pct: pct_boost(base.hp_pct, c.unique_hp_pct),
                dmg_pct: pct_boost(base.dm_pct, c.unique_dmg_pct),
                th_pct: pct_boost(base.th, c.unique_th_pct),
                xp_pct: base.xp_pct.saturating_mul(c.unique_xp_mult),
                ac: base.ac,
                treasure_class: Some(tc_of(r, MonKind::Unique)),
            };
            rule.kind = MonKind::Unique;
            (MonKind::Unique, monumod.pick_unique_mod(rng).unwrap_or("").to_string(), rule)
        } else if rng.below(100) < c.champion_chance {
            let base = monlvl.row(r.level);
            let rule = SpawnRule {
                kind: MonKind::Champion,
                hp_pct: pct_boost(base.hp_pct, c.champion_hp_pct),
                dmg_pct: pct_boost(base.dm_pct, c.champion_dmg_pct),
                th_pct: pct_boost(base.th, c.champion_th_pct),
                xp_pct: base.xp_pct.saturating_mul(c.champion_xp_mult),
                ac: base.ac,
                treasure_class: Some(tc_of(r, MonKind::Champion)),
            };
            (MonKind::Champion, monumod.pick_champion_mod(rng).unwrap_or("").to_string(), rule)
        } else {
            let base = monlvl.row(r.level);
            let rule = SpawnRule {
                kind: MonKind::Normal,
                hp_pct: base.hp_pct,
                dmg_pct: base.dm_pct,
                th_pct: base.th,
                xp_pct: base.xp_pct,
                ac: base.ac,
                treasure_class: None, // TC du gabarit (= TreasureClass1)
            };
            (MonKind::Normal, String::new(), rule)
        };
        plan.packs.push(PackSpawn {
            code: entry.code.clone(),
            mon_id: r.id.clone(),
            count,
            rule,
            minion_hp_pct: if kind == MonKind::Normal { 0 } else { c.minion_hp_pct },
            mod_name,
        });
        placed += count;
    }

    // Super-uniques : ceux dont la classe est DANS la planche du niveau
    // (approximation assumée des positions retail MonPreset — ex. Blood Moor
    // → Corpsefire [Den of Evil en retail], Cold Plains → Bishibosh,
    // Dark Wood → Rakanishu ; cf. impl-monstres-boss « positions »).
    for su in &superuniques.recs {
        if !rec.mon.iter().any(|m| *m == su.class) {
            continue;
        }
        let Some(cls) = monstats.get(&su.class) else { continue };
        let Some(code) = resolve_code(&cls.code) else { continue };
        let base = monlvl.row(cls.level);
        let rule = SpawnRule {
            kind: MonKind::SuperUnique,
            hp_pct: pct_boost(base.hp_pct, c.unique_hp_pct),
            dmg_pct: pct_boost(base.dm_pct, c.unique_dmg_pct),
            th_pct: pct_boost(base.th, c.unique_th_pct),
            xp_pct: base.xp_pct.saturating_mul(c.unique_xp_mult),
            ac: base.ac,
            treasure_class: if su.tc.is_empty() { None } else { Some(su.tc.clone()) },
        };
        let (lo, hi) = (su.min_grp.max(1), su.max_grp.max(su.min_grp.max(1)));
        let count = rng.below(hi - lo + 1) + lo;
        let su_mods: Vec<String> = su
            .mods
            .iter()
            .filter_map(|raw| raw.parse::<u32>().ok())
            .filter_map(|id| monumod.name_by_id(id).map(|s| s.to_string()))
            .collect();
        plan.super_uniques.push(SuSpawn {
            name: su.name.clone(),
            code,
            mon_id: su.class.clone(),
            count,
            rule,
            minion_hp_pct: c.minion_hp_pct,
            mods: su_mods,
        });
    }
    plan
}

/// `base × (100 + boost)/100` (boost monumod en %).
fn pct_boost(base: u32, boost_pct: u32) -> u32 {
    base.saturating_mul(100 + boost_pct) / 100
}

/// TC effective d'un rang : TreasureClass2 (champion) / 3 (unique), repli TC1.
fn tc_of(r: &MonStatsRec, kind: MonKind) -> String {
    let t = match kind {
        MonKind::Champion => &r.tc[1],
        _ => &r.tc[2],
    };
    if t.is_empty() {
        r.tc[0].clone()
    } else {
        t.clone()
    }
}

// ---------------------------------------------------------------------------
// Tests — les vraies tables extraites + le plan Blood Moor / Cold Plains
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const MONSTATS: &str = include_str!("../../../../assets/patch_monstats.txt");
    const MONUMOD: &str = include_str!("../../../../assets/monumod.txt");
    const SUPERUNIQUES: &str = include_str!("../../../../assets/superuniques.txt");
    const MONLVL: &str = include_str!("../../../../assets/MonLvl.txt");
    const LEVELS: &str = include_str!("../../../../assets/patch_levels.txt");

    fn tables() -> (MonStats, MonUMods, SuperUniques, MonLvl, crate::levels::LevelTables) {
        (
            MonStats::parse(MONSTATS).unwrap(),
            MonUMods::parse(MONUMOD).unwrap(),
            SuperUniques::parse(SUPERUNIQUES).unwrap(),
            MonLvl::parse(MONLVL).unwrap(),
            crate::levels::LevelTables::parse(LEVELS).unwrap(),
        )
    }

    #[test]
    fn parses_real_monstats() {
        let (ms, _, _, _, _) = tables();
        assert_eq!(ms.len(), 734, "{} records monstats", ms.len());
        let z = ms.get("zombie1").unwrap();
        assert_eq!(z.code, "ZM");
        assert_eq!((z.hp_min, z.hp_max, z.exp), (101, 181, 111));
        assert_eq!((z.min_grp, z.max_grp, z.rarity), (1, 2, 2));
        assert_eq!(z.tc[0], "Act 1 H2H A");
        let f = ms.get("fallen1").unwrap();
        assert_eq!((f.min_grp, f.max_grp), (2, 3), "packs fallen réels");
        assert_eq!(f.tc[1], "Act 1 Champ A", "TC champion réelle");
        assert_eq!(f.tc[2], "Act 1 Unique A", "TC unique réelle");
        let a = ms.get("andariel").unwrap();
        assert_eq!((a.level, a.hp_max, a.tc[0].as_str()), (12, 2562, "Andariel"));
        assert!(ms.get("inexistant42").is_none());
    }

    #[test]
    fn parses_real_monumod_with_official_constants() {
        let (_, mu, _, _, _) = tables();
        assert_eq!(mu.mods.len(), 43, "{} mods monumod", mu.mods.len());
        // Constantes officielles de la colonne constants/*constant desc :
        assert_eq!(mu.constants.champion_chance, 20, "champion chance");
        assert_eq!(mu.constants.champion_hp_pct, 200, "champion +hp%");
        assert_eq!(mu.constants.champion_dmg_pct, 100, "champion +dmg%");
        assert_eq!(mu.constants.unique_hp_pct, 300, "unique +hp%");
        assert_eq!(mu.constants.unique_dmg_pct, 150, "unique +dmg%");
        assert_eq!(mu.constants.minion_hp_pct, 100, "minion +hp%");
        // Mods éligibles : strong/fast/curse… champion=1, poids cpick 6.
        let mut rng = Lcg::new(1);
        for _ in 0..50 {
            let m = mu.pick_champion_mod(&mut rng).unwrap();
            assert!(
                ["strong", "fast", "curse", "resist", "fire", "cold", "manahit", "teleport",
                 "spectralhit", "stoneskin", "multishot", "lightning", "champion", "ghostly",
                 "fanatic", "possessed", "berserk"]
                    .contains(&m),
                "mod champion inattendu {m}"
            );
        }
        let mut rng = Lcg::new(2);
        for _ in 0..20 {
            mu.pick_unique_mod(&mut rng).expect("mod unique disponible");
        }
    }

    #[test]
    fn parses_real_superuniques() {
        let (_, _, su, _, _) = tables();
        assert_eq!(su.len(), 67, "{} super-uniques", su.len());
        let b = su.recs.iter().find(|s| s.name == "Bishibosh").unwrap();
        assert_eq!(b.class, "fallenshaman1");
        assert_eq!(b.tc, "Act 1 Super A");
        assert_eq!(b.mods, vec!["8", "9"], "Mod1/Mod2 bruts (ids monumod)");
        let r = su.recs.iter().find(|s| s.name == "Rakanishu").unwrap();
        assert_eq!(r.class, "fallen2");
        assert_eq!((r.min_grp, r.max_grp), (8, 8), "bande de 8");
    }

    #[test]
    fn parses_real_monlvl_rows() {
        let (_, _, _, ml, _) = tables();
        assert_eq!(ml.len(), 111, "lignes 0..110");
        // Ligne 1 = EXACTEMENT les constantes historiques du port.
        assert_eq!(ml.row(1), MonLvlRow { ac: 6, th: 8, hp_pct: 7, dm_pct: 2, xp_pct: 30 });
        // Ligne 12 (andariel) réelle et croissante.
        let r12 = ml.row(12);
        assert!(r12.hp_pct > ml.row(1).hp_pct);
        assert!(r12.th > ml.row(1).th);
        assert_eq!(ml.row(12), ml.row(12));
        // Niveau hors bornes : clamp (pas de panic).
        assert_eq!(ml.row(999), ml.row(110));
    }

    #[test]
    fn density_formula_bloodmoor() {
        assert_eq!(density_target(520, 80, 80), 33, "Blood Moor retail");
        assert_eq!(density_target(0, 80, 80), 0, "pas de densité = pas de spawn");
        assert_eq!(density_target(600, 24, 24), 3, "Den of Evil (maze)");
        assert_eq!(density_target(10_000, 200, 200), MAX_SPAWN_TARGET, "garde-fou");
    }

    #[test]
    fn bloodmoor_plan_is_deterministic_and_in_packs() {
        let (ms, mu, su, ml, lt) = tables();
        let bm = lt.get(2).unwrap();
        let resolve = |code: &str| -> Option<String> {
            // sprites du manifest : FA/ZM/SI (+SK/FS/YE) présents.
            ["FA", "ZM", "SI", "SK", "FS", "YE"]
                .iter()
                .find(|c| **c == code)
                .map(|c| c.to_string())
        };
        let mut a = plan_level_spawns(bm, &ms, &mu, &ml, &su, &mut Lcg::new(42), resolve);
        let mut b = plan_level_spawns(bm, &ms, &mu, &ml, &su, &mut Lcg::new(42), resolve);
        assert_eq!(a.target, 33);
        // Déterminisme exact au seed.
        assert_eq!(format!("{a:?}"), format!("{b:?}"), "même graine ⇒ même plan");
        // Population ≈ cible (arrondi packs), packs dans MinGrp..MaxGrp.
        let pop: u32 = a.packs.iter().map(|p| p.count).sum();
        assert!(pop >= a.target.saturating_sub(4) && pop <= a.target + 6, "{pop} vs {}", a.target);
        let resolve_stats = |id: &str| ms.get(id).unwrap();
        for p in &a.packs {
            let r = resolve_stats(&p.mon_id);
            assert!((p.count as i32 - r.min_grp as i32) >= 0 && p.count <= r.max_grp.max(1),
                    "pack {} hors MinGrp..MaxGrp", p.mon_id);
            assert!(["FA", "ZM", "SI"].contains(&p.code.as_str()), "code sprite {}", p.code);
            assert_eq!(p.rule.ac, ml.row(r.level).ac);
        }
        // Champion : ~20 % des packs (chance monumod) — au moins un ici.
        let champs = a.packs.iter().filter(|p| p.rule.kind == MonKind::Champion).count();
        assert!(champs >= 1, "au moins un pack champion (seed 42), eu {champs}");
        for p in a.packs.iter().filter(|p| p.rule.kind == MonKind::Champion) {
            let base = ml.row(resolve_stats(&p.mon_id).level);
            assert_eq!(p.rule.hp_pct, base.hp_pct * 300 / 100, "champion ×3 HP");
            assert_eq!(p.rule.xp_pct, base.xp_pct * 3, "champion ×3 XP");
            assert_eq!(p.rule.treasure_class.as_deref(), Some("Act 1 Champ A"));
            assert_eq!(p.minion_hp_pct, 100, "minions ×2 HP");
            assert!(!p.mod_name.is_empty(), "mod champion tiré");
        }
        // Blood Moor : MonUMin/Max vides ⇒ zéro unique GARANTI (data fait foi).
        assert!(a.packs.iter().all(|p| p.rule.kind != MonKind::Unique));
        // Super-unique de la planche : Corpsefire (zombie1) — retail = Den of
        // Evil ; approximation assumée des positions (classe ∈ planche, cf.
        // impl-monstres-boss « positions »).
        assert_eq!(a.super_uniques.len(), 1, "Corpsefire seul");
        let cf = &a.super_uniques[0];
        assert_eq!((cf.name.as_str(), cf.code.as_str()), ("Corpsefire", "ZM"));
        assert_eq!(cf.rule.treasure_class.as_deref(), Some("Act 1 Super A"));
        assert_eq!(cf.rule.kind, MonKind::SuperUnique);
        // Graine différente ⇒ plan (très probablement) différent.
        let c = plan_level_spawns(bm, &ms, &mu, &ml, &su, &mut Lcg::new(43), resolve);
        assert_ne!(format!("{a:?}"), format!("{c:?}"), "graine ≠ ⇒ plan ≠");
    }

    #[test]
    fn cold_plains_plan_has_guaranteed_unique_and_bishibosh() {
        let (ms, mu, su, ml, lt) = tables();
        let cp = lt.get(3).unwrap(); // Cold Plains : MonUMin=1, planche brute/cr/fs/lancer
        let resolve = |code: &str| -> Option<String> {
            ["FA", "ZM", "SI", "SK", "FS", "YE"].iter().find(|c| **c == code).map(|c| c.to_string())
        };
        let plan = plan_level_spawns(cp, &ms, &mu, &ml, &su, &mut Lcg::new(9), resolve);
        // Bishibosh (fallenshaman1 ∈ planche) : doré, nommé, TC super réelle.
        let bishi = plan
            .super_uniques
            .iter()
            .find(|s| s.name == "Bishibosh")
            .expect("Bishibosh sur Cold Plains");
        assert_eq!(bishi.code, "FS");
        assert_eq!(bishi.rule.kind, MonKind::SuperUnique);
        assert_eq!(bishi.rule.treasure_class.as_deref(), Some("Act 1 Super A"));
        assert_eq!(bishi.rule.hp_pct, ml.row(ms.get("fallenshaman1").unwrap().level).hp_pct * 4,
            "super-unique ×4 HP");
        assert_eq!(bishi.mods, vec!["resist", "fire"], "Mod1/Mod2 résolus monumod");
        // 1 unique garanti (MonUMin=1/MonUMax=1).
        let uniques = plan.packs.iter().filter(|p| p.rule.kind == MonKind::Unique).count();
        assert_eq!(uniques, 1, "MonUMin/MonUMax = 1/1");
        // Rakanishu (fallen2) : PAS sur Cold Plains (planche Dark Wood).
        assert!(plan.super_uniques.iter().all(|s| s.name != "Rakanishu"));
    }

    #[test]
    fn dark_wood_plan_spawns_rakanishu() {
        let (ms, mu, su, ml, lt) = tables();
        let dw = lt.get(5).unwrap(); // Dark Wood : fallen2 ∈ planche
        let resolve = |code: &str| -> Option<String> {
            ["FA", "ZM", "SI", "SK", "FS", "YE"].iter().find(|c| **c == code).map(|c| c.to_string())
        };
        let plan = plan_level_spawns(dw, &ms, &mu, &ml, &su, &mut Lcg::new(5), resolve);
        let raka = plan.super_uniques.iter().find(|s| s.name == "Rakanishu").expect("Rakanishu");
        assert_eq!(raka.code, "FA", "fallen2 → FA");
        assert_eq!(raka.count, 8, "bande de Rakanishu = 8 (MinGrp/MaxGrp)");
        assert_eq!(raka.rule.treasure_class.as_deref(), Some("Act 1 Super B"));
    }

    #[test]
    fn plan_empty_without_monsters_or_density() {
        let (ms, mu, su, ml, lt) = tables();
        let town = lt.get(1).unwrap(); // pas de planche
        let resolve = |_: &str| -> Option<String> { None };
        let plan = plan_level_spawns(town, &ms, &mu, &ml, &su, &mut Lcg::new(1), resolve);
        assert!(plan.packs.is_empty() && plan.target == 0);
        // Planche présente mais aucun sprite résolu : plan vide (repli appelant).
        let bm = lt.get(2).unwrap();
        let none = |_: &str| -> Option<String> { None };
        let plan = plan_level_spawns(bm, &ms, &mu, &ml, &su, &mut Lcg::new(1), none);
        assert!(plan.packs.is_empty() && plan.super_uniques.is_empty());
    }
}
