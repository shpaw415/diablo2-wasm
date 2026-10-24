//! `shrines` — sanctuaires (shrines) v1.12 data-driven + règles d'usage des
//! objets interactifs (étape 9c « monde vivant », campagne diablo-2-wasm).
//!
//! Sources réelles (lecture seule, embarquées en l'état) :
//! * `d2w-re/tables/monde/shrines.txt` — 23 sanctuaires : `Code` (0..=22),
//!   `Arg0`/`Arg1`, `Duration in frames`, `reset time in minutes`, `rarity`
//!   (poids de tirage — convention Diablerie : plus haut = plus fréquent).
//!   Les 4 marqueurs `(OUT)` de la colonne `Effect` (Health/Mana Boost,
//!   Health/Mana Exchange) sont RETIRÉS du jeu v1.12 : jamais tirés.
//!
//! Règles portées (MVP réel, écarts documentés) :
//! * tirage de l'EFFET à l'apparition (pas à l'usage), pondéré par `rarity`,
//!   parmi le pool IMPLÉMENTÉ = Recharge (refill) + Boosters (armor/combat/
//!   résists/skill/recharge/stamina/xp). Les sanctuaires Magic (portail,
//!   gemme, tempête, warping, explosif…) demandent des systèmes hors
//!   périmètre 9c : exclus du pool, documentés ici ;
//! * Recharge (code 1) : instantané — vie + mana au maximum ;
//! * effets temporisés : durée = `Duration in frames` × 40 ms (25 ips D2 —
//!   durée GAMEPLAY ; le FRAME_MS=100 de `world.rs` ne vaut que pour l'anim) ;
//! * `reset time in minutes` : réutilisable après N minutes (0 = une seule
//!   fois par partie) ;
//! * les contributions de stats passent par le socle EXISTANT `statlist`
//!   ([`ShrineBuff::stat_lines`] → ids `ItemStatCost` — même interface que
//!   les affixes) : le câblage combat = inclure la source buff dans
//!   `StatList::from_sources`. Recharge/Stamina/Experience n'ont pas d'équivalent
//!   stat propre : buff porté pour le HUD, effet gameplay documenté ici ;
//! * usage des CONTENEURS (coffres/cadavres) : `World::use_container` —
//!   même chaîne que les drops monstres (`loot::Loot::roll` + `to_def`),
//!   événements `ItemDropped` poussés dans `pending_events` (pattern
//!   `NpcTalked` : émis hors tick, flushés en tête du tick suivant).
//!
//! Zéro IO, zéro panique : la table est embarquée, les accesseurs sont
//! lenient (table illisible ⇒ pool vide, les usages renvoient une erreur).

use crate::statlist::{
    StatLine, ARMOR_PERCENT, COLDRESIST, FIRERESIST, LIGHTRESIST, MAXDAMAGE_PERCENT, POISONRESIST,
};
use crate::tsv::Tsv;
use crate::{GameEvent, ItemDef, ItemOnGround, World};
use std::sync::OnceLock;

/// `shrines.txt` v1.12 (24 lignes = en-tête + 23 sanctuaires), embarquées
/// EN L'ÉTAT depuis `d2w-re/tables/monde/shrines.txt` (2026-10-24). Champs
/// quotés Excel (`""` = quote littérale) : le parseur TSV générique les gère.
const SHRINES_TSV: &str = r#"Shrine Type	Shrine name	Effect	Code	Arg0	Arg1	Duration in frames	reset time in minutes	rarity	view name	niftyphrase	effectclass	LevelMin
None	None	none	0	100	100	0	2	1	blah	blah	0	0
Recharge	Refill	fills health and mana	1	100	100	0	2	1	blah	blah	4	1
Recharge	Health Boost	doubles current health (even over maximum) (OUT)	2	200	400	0	5	2	blah	blah	2	1
Recharge	Mana Boost	"doubles current mana ("") (OUT)"	3	200	400	0	5	2	blah	blah	3	1
Recharge	Health Exchange	"takes 1/2 health, gives 5 times that number to current mana (OUT)"	4	50	500	0	0	3	blah	blah	2	2
Recharge	Mana Exchange	"takes 1/2 mana, gives 5 times that number to current health (OUT)"	5	50	500	0	0	3	blah	blah	3	2
Booster	Armor Boost	+100% AC	6	100	1	2400	5	2	blah	blah	4	5
Booster	Combat Boost	"+200% to hit, +200% min and max damage"	7	200	200	2400	5	2	blah	blah	4	8
Booster	Resist Fire Boost	+75% to resist fire	8	75	0	3600	5	2	blah	blah	4	5
Booster	Resist Cold Boost	+75% to resist cold	9	75	0	3600	5	2	blah	blah	4	26
Booster	Resist Lightning Boost	+75% to resist lightning	10	75	0	3600	5	2	blah	blah	4	32
Booster	Resist Poison Boost	"+75% to resist poison, and poison duration = 0"	11	75	0	3600	5	2	blah	blah	4	21
Booster	Skill Boost	+2 to all skill levels	12	2	0	2400	5	2	blah	blah	4	1
Booster	Recharge Boost	+400% mana recharge rate	13	400	0	2400	5	2	blah	blah	4	1
Booster	Stamina Boost	Unlimited stamina	14	200	0	4800	5	3	blah	blah	4	1
Booster	Experience Boost	50% more experience per kill	15	50	50	3600	0	3	blah	blah	4	1
Magic	Shrine of Enirhs	reverses character name (OUT)	16	0	0	0	0	3	blah	blah	1	1
Magic	Portal to unknown	opens a portal to town	17	0	0	0	0	2	blah	blah	1	3
Magic	Gem Upgrade	"upgrades a random gem (up to one before max level if available), or gives a random gem"	18	0	0	0	0	3	blah	blah	1	4
Magic	Storm Shrine	"all players and monsters lose 1/2 current HP, shoots fireballs"	19	50	2000	0	0	2	blah	blah	1	1
Magic	Warping Shrine	nearest monster becomes unique	20	0	0	0	0	3	blah	blah	1	3
Magic	Exploding Shrine	drops 5-10 exploding potions & tosses out exploding potions in random directions/distances	21	5	10	0	0	3	blah	blah	1	1
Magic	Poison Shrine	drops 5-10 poison gas potions & creates ring of poison gas around shrine	22	5	10	0	0	3	blah	blah	1	1"#;

/// 40 ms par frame D2 (25 ips) — conversion officielle des durées gameplay
/// de `shrines.txt` (`Duration in frames` : 2400 f = 96 s).
pub const D2_FRAME_MS: u64 = 40;

/// ids `ItemStatCost.txt` v1.12 absents des constantes `statlist`
/// (vérifiés sur `assets/patch_itemstatcost.txt`, colonne `ID`).
/// `item_tohit_percent` (id 119).
pub const TOHIT_PERCENT: i32 = 119;
/// `item_allskills` (id 127) — +2 aux tous les skills (Skill Boost).
pub const ITEM_ALLSKILLS: i32 = 127;

/// Famille d'effet d'un sanctuaire (colonne `Code`, 0..=22 — port fidèle de
/// la table v1.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShrineKind {
    /// Code 0 — ligne sentinelle `None` (jamais tirée).
    None,
    /// 1 — Recharge : remplit vie et mana (instantané).
    Refill,
    /// 2 — Health Boost (OUT : retiré du jeu v1.12).
    HealthBoost,
    /// 3 — Mana Boost (OUT).
    ManaBoost,
    /// 4 — Health Exchange (OUT).
    HealthExchange,
    /// 5 — Mana Exchange (OUT).
    ManaExchange,
    /// 6 — Armor Boost : +100 % défense.
    Armor,
    /// 7 — Combat Boost : +200 % toucher, +200 % dégâts.
    Combat,
    /// 8 — Resist Fire : +75 %.
    ResFire,
    /// 9 — Resist Cold : +75 %.
    ResCold,
    /// 10 — Resist Lightning : +75 %.
    ResLightning,
    /// 11 — Resist Poison : +75 %.
    ResPoison,
    /// 12 — Skill Boost : +2 à tous les skills.
    Skill,
    /// 13 — Recharge Boost : +400 % régénération de mana.
    Recharge,
    /// 14 — Stamina Boost : endurance illimitée.
    Stamina,
    /// 15 — Experience Boost : +50 % XP par kill.
    Experience,
    /// 16 — Enirhs (inverse le nom — gag, hors pool).
    Enirhs,
    /// 17 — Portal : ouvre un portail vers la ville (hors pool 9c).
    Portal,
    /// 18 — Gem Upgrade (hors pool 9c).
    GemUpgrade,
    /// 19 — Storm Shrine (hors pool 9c).
    Storm,
    /// 20 — Warping (hors pool 9c).
    Warping,
    /// 21 — Exploding (hors pool 9c).
    Exploding,
    /// 22 — Poison (hors pool 9c).
    PoisonGas,
}

/// Un sanctuaire de la table v1.12 (`shrines.txt`, 1 ligne = 1 record).
#[derive(Debug, Clone, PartialEq)]
pub struct ShrineDef {
    /// Colonne `Code` (0..=22).
    pub code: u8,
    /// Colonne `Shrine name` (ex. « Refill », « Combat Boost »).
    pub name: String,
    /// Famille d'effet.
    pub kind: ShrineKind,
    /// Colonne `Arg0` (ex. 100 = +100 %).
    pub arg0: i32,
    /// Colonne `Arg1` (ex. 200 pour Combat = dégâts %).
    pub arg1: i32,
    /// Colonne `Duration in frames` (0 = instantané).
    pub duration_frames: u32,
    /// Colonne `reset time in minutes` (0 = une seule fois par partie).
    pub reset_minutes: u32,
    /// Colonne `rarity` — POIDS de tirage (convention Diablerie :
    /// `ShrineSelection` somme les rarety, plus haut = plus fréquent).
    pub rarity: u32,
    /// Marqueur `(OUT)` dans `Effect` — retiré du jeu v1.12, jamais tiré.
    pub out: bool,
}

/// Table embarquée parsée une fois (lenient : illisible ⇒ vide).
pub fn all() -> &'static [ShrineDef] {
    static T: OnceLock<Vec<ShrineDef>> = OnceLock::new();
    T.get_or_init(|| {
        let Ok(t) = Tsv::parse(SHRINES_TSV) else { return Vec::new() };
        (0..t.len())
            .filter_map(|i| {
                let r = t.rec(i)?;
                let code = r.u("Code") as u8;
                let effect = r.s("Effect");
                Some(ShrineDef {
                    code,
                    name: r.s("Shrine name").to_string(),
                    kind: match code {
                        0 => ShrineKind::None,
                        1 => ShrineKind::Refill,
                        2 => ShrineKind::HealthBoost,
                        3 => ShrineKind::ManaBoost,
                        4 => ShrineKind::HealthExchange,
                        5 => ShrineKind::ManaExchange,
                        6 => ShrineKind::Armor,
                        7 => ShrineKind::Combat,
                        8 => ShrineKind::ResFire,
                        9 => ShrineKind::ResCold,
                        10 => ShrineKind::ResLightning,
                        11 => ShrineKind::ResPoison,
                        12 => ShrineKind::Skill,
                        13 => ShrineKind::Recharge,
                        14 => ShrineKind::Stamina,
                        15 => ShrineKind::Experience,
                        16 => ShrineKind::Enirhs,
                        17 => ShrineKind::Portal,
                        18 => ShrineKind::GemUpgrade,
                        19 => ShrineKind::Storm,
                        20 => ShrineKind::Warping,
                        21 => ShrineKind::Exploding,
                        _ => ShrineKind::PoisonGas,
                    },
                    arg0: r.i("Arg0"),
                    arg1: r.i("Arg1"),
                    duration_frames: r.u("Duration in frames"),
                    reset_minutes: r.u("reset time in minutes"),
                    rarity: r.u("rarity"),
                    out: effect.contains("(OUT)"),
                })
            })
            .collect()
    })
}

/// Sanctuaire par code (`Code` 0..=22).
pub fn by_code(code: u8) -> Option<&'static ShrineDef> {
    all().iter().find(|d| d.code == code)
}

/// Pool TIRABLE : non-`OUT`, code ≠ 0 et effet implémenté en 9c (Recharge +
/// Boosters). Les Magic (portail/gemme/tempête/warping/explosif/poison/gag)
/// demandent des systèmes hors périmètre — exclus, cf. doc de module.
pub fn spawn_pool() -> Vec<&'static ShrineDef> {
    all()
        .iter()
        .filter(|d| {
            !d.out
                && d.code != 0
                && matches!(
                    d.kind,
                    ShrineKind::Refill
                        | ShrineKind::Armor
                        | ShrineKind::Combat
                        | ShrineKind::ResFire
                        | ShrineKind::ResCold
                        | ShrineKind::ResLightning
                        | ShrineKind::ResPoison
                        | ShrineKind::Skill
                        | ShrineKind::Recharge
                        | ShrineKind::Stamina
                        | ShrineKind::Experience
                )
        })
        .collect()
}

/// Poids total du pool (dénominateur du tirage — exposé pour les appelants
/// qui roulent leur propre entier : `pick_weighted(rng.below(total))`).
pub fn pool_weight() -> u32 {
    spawn_pool().iter().map(|d| d.rarity.max(1)).sum()
}

/// Tirage pondéré par `rarity` (plus haut = plus fréquent) sur un rouleau
/// `roll ∈ 0..pool_weight()` — PUR et déterministe. Ordre = table.
pub fn pick_weighted(roll: u32) -> Option<&'static ShrineDef> {
    let pool = spawn_pool();
    let total: u32 = pool.iter().map(|d| d.rarity.max(1)).sum();
    if pool.is_empty() || total == 0 {
        return None;
    }
    let mut roll = roll % total;
    for d in &pool {
        let w = d.rarity.max(1);
        if roll < w {
            return Some(d);
        }
        roll -= w;
    }
    pool.last().copied()
}

/// Effet temporisé actif sur le héros (porté par le plan d'objets côté
/// moteur, `objects_live::LiveObject.buff`) — durée comptée en ms simulés.
#[derive(Debug, Clone, PartialEq)]
pub struct ShrineBuff {
    /// Code du sanctuaire source (`shrines.txt`).
    pub code: u8,
    /// `Arg0` de la table (intensité, ex. 100 = +100 %).
    pub arg0: i32,
    /// `Arg1` de la table.
    pub arg1: i32,
    /// Durée restante en ms (décrémentée par le tick du plan d'objets).
    pub remaining_ms: u64,
}

impl ShrineBuff {
    /// Buff temporisé depuis la table : durée = frames × 40 ms (25 ips D2).
    /// Un sanctuaire instantané (frames = 0) ne produit pas de buff.
    pub fn from_def(def: &ShrineDef) -> Option<ShrineBuff> {
        if def.duration_frames == 0 {
            return None;
        }
        Some(ShrineBuff {
            code: def.code,
            arg0: def.arg0,
            arg1: def.arg1,
            remaining_ms: def.duration_frames as u64 * D2_FRAME_MS,
        })
    }

    /// Un pas de temps simulé. Renvoie `true` tant que le buff est actif.
    pub fn tick(&mut self, dt_ms: u32) -> bool {
        if self.remaining_ms > 0 {
            self.remaining_ms = self.remaining_ms.saturating_sub(dt_ms as u64);
        }
        self.remaining_ms > 0
    }

    /// Buff actif ?
    pub fn is_active(&self) -> bool {
        self.remaining_ms > 0
    }

    /// Contributions au socle `statlist` EXISTANT (ids `ItemStatCost` v1.12 —
    /// même interface que les affixes : `StatList::from_sources([base,
    /// équipement, buff])`). Combat = +Arg0 % toucher + Arg1 % dégâts ;
    /// Armor = +Arg0 % défense ; résists = +Arg0 ; Skill = +Arg0 all skills.
    /// Recharge/Stamina/Experience : pas de stat `ItemStatCost` directe —
    /// vide ici, effet gameplay documenté au module (câblage au pas suivant).
    pub fn stat_lines(&self) -> Vec<StatLine> {
        let Some(def) = by_code(self.code) else { return Vec::new() };
        match def.kind {
            ShrineKind::Armor => vec![StatLine::new(ARMOR_PERCENT, self.arg0 as i64)],
            ShrineKind::Combat => vec![
                StatLine::new(TOHIT_PERCENT, self.arg0 as i64),
                StatLine::new(MAXDAMAGE_PERCENT, self.arg1 as i64),
            ],
            ShrineKind::ResFire => vec![StatLine::new(FIRERESIST, self.arg0 as i64)],
            ShrineKind::ResCold => vec![StatLine::new(COLDRESIST, self.arg0 as i64)],
            ShrineKind::ResLightning => vec![StatLine::new(LIGHTRESIST, self.arg0 as i64)],
            ShrineKind::ResPoison => vec![StatLine::new(POISONRESIST, self.arg0 as i64)],
            ShrineKind::Skill => vec![StatLine::new(ITEM_ALLSKILLS, self.arg0 as i64)],
            _ => Vec::new(),
        }
    }
}

// --------------------------------------------------------------------------
// Usage côté World — même pattern additif que waypoints/npc (impl séparés).
// --------------------------------------------------------------------------

impl World {
    /// Effet instantané Recharge (code 1) : vie et mana au maximum.
    /// Renvoie `(soin rendu, mana rendu)` (0 si déjà pleine).
    pub fn apply_shrine_recharge(&mut self) -> (u32, u32) {
        let s = &mut self.hero.stats;
        let healed = s.max_life.saturating_sub(s.life);
        let mana = s.max_mana.saturating_sub(s.mana);
        s.life = s.max_life;
        s.mana = s.max_mana;
        (healed, mana)
    }

    /// Ouverture d'un conteneur (coffre, casket, cadavre…) : butin roulé
    /// depuis la TreasureClass du conteneur (ex. « Act 1 Chest B »), objets
    /// posés au sol à `(x, y)` (tuiles) et événements `ItemDropped` poussés
    /// dans `pending_events` — flushés en tête du tick suivant (pattern
    /// `NpcTalked`, même chaîne que les drops monstres). TC inconnue ⇒ repli
    /// MVP (`Loot::roll_fallback`), jamais de panique. Renvoie les définitions
    /// posées (contrat UI).
    pub fn use_container(&mut self, x: f32, y: f32, tc_name: &str, mlvl: u32) -> Vec<ItemDef> {
        let mlvl = mlvl.max(1);
        let loot = self.loot.as_ref();
        let drops = match loot {
            Some(l) if l.has_tc(tc_name) => l.roll(&mut self.rng, tc_name, mlvl),
            _ => crate::loot::Loot::roll_fallback(&mut self.rng, mlvl),
        };
        let mut defs = Vec::with_capacity(drops.len());
        for d in drops {
            let def = match loot {
                Some(l) => l.to_def(&d),
                None => {
                    if d.code == "gld" {
                        ItemDef::gold(d.gold)
                    } else {
                        ItemDef::potion()
                    }
                }
            };
            self.pending_events.push(GameEvent::ItemDropped {
                item: def.clone(),
                x: x.floor() as i32,
                y: y.floor() as i32,
            });
            self.items.push(ItemOnGround { def: def.clone(), x, y });
            defs.push(def);
        }
        defs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::Rng;
    use crate::World;

    fn world() -> World {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../../../../assets/manifest.json")).unwrap();
        let grid = vec![vec![true; 24]; 24];
        World::with_seed(grid, manifest, 9)
    }

    #[test]
    fn table_has_the_23_real_shrines() {
        assert_eq!(all().len(), 23, "shrines.txt v1.12 = 23 sanctuaires");
        let codes: Vec<u8> = all().iter().map(|d| d.code).collect();
        let mut sorted = codes.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (0..=22).collect::<Vec<u8>>(), "codes 0..=22 uniques");
        let refill = by_code(1).expect("Refill présent");
        assert_eq!(refill.name, "Refill");
        assert_eq!(refill.kind, ShrineKind::Refill);
        assert_eq!(refill.reset_minutes, 2);
        assert_eq!(refill.rarity, 1);
        let combat = by_code(7).expect("Combat Boost présent");
        assert_eq!(combat.duration_frames, 2400);
        assert_eq!(combat.reset_minutes, 5);
        assert_eq!(combat.arg0, 200, "+200 % toucher");
        assert_eq!(combat.arg1, 200, "+200 % dégâts");
        // Les OUT v1.12 ne sont jamais tirables (Health/Mana Boost + Exchange
        // + le gag Enirhs — marqueurs `(OUT)` de la colonne `Effect`).
        let out_codes: Vec<u8> = all().iter().filter(|d| d.out).map(|d| d.code).collect();
        assert_eq!(out_codes, vec![2, 3, 4, 5, 16]);
        assert!(spawn_pool().iter().all(|d| !d.out && d.code != 0));
        assert!(spawn_pool().iter().all(|d| {
            !matches!(
                d.kind,
                ShrineKind::Portal
                    | ShrineKind::GemUpgrade
                    | ShrineKind::Storm
                    | ShrineKind::Warping
                    | ShrineKind::Exploding
                    | ShrineKind::PoisonGas
                    | ShrineKind::Enirhs
            )
        }), "pool 9c = Recharge + Boosters uniquement");
    }

    #[test]
    fn pick_weighted_is_deterministic_and_bounded() {
        let total = pool_weight();
        assert_eq!(
            pick_weighted(total).map(|d| d.code),
            pick_weighted(0).map(|d| d.code),
            "roll modulo le poids total"
        );
        assert_eq!(pick_weighted(0).map(|d| d.code), Some(1), "roll 0 → premier du pool (Refill)");
        // Même graine ⇒ même flux de tirages (déterminisme SplitMix64).
        let mut a = Rng::new(11);
        let mut b = Rng::new(11);
        for _ in 0..500 {
            assert_eq!(
                pick_weighted(a.below(pool_weight())).map(|d| d.code),
                pick_weighted(b.below(pool_weight())).map(|d| d.code),
            );
        }
    }

    #[test]
    fn buff_tick_expires_at_duration() {
        // Combat Boost : 2400 frames × 40 ms = 96 000 ms.
        let mut buff = ShrineBuff::from_def(by_code(7).unwrap()).expect("effet temporisé");
        assert_eq!(buff.remaining_ms, 96_000);
        assert!(buff.tick(95_999), "actif juste avant l'expiration");
        assert!(!buff.tick(1), "expiré au pas suivant (temps simulé)");
        assert!(!buff.tick(5_000), "rester expiré");
        // Recharge : instantané ⇒ pas de buff.
        assert!(ShrineBuff::from_def(by_code(1).unwrap()).is_none());
    }

    #[test]
    fn buff_stat_lines_target_real_itemstatcost_ids() {
        let combat = ShrineBuff::from_def(by_code(7).unwrap()).unwrap();
        assert_eq!(
            combat.stat_lines(),
            vec![
                StatLine::new(TOHIT_PERCENT, 200),  // item_tohit_percent (119)
                StatLine::new(MAXDAMAGE_PERCENT, 200), // item_maxdamage_percent (17)
            ]
        );
        let armor = ShrineBuff::from_def(by_code(6).unwrap()).unwrap();
        assert_eq!(armor.stat_lines(), vec![StatLine::new(ARMOR_PERCENT, 100)]);
        let resfire = ShrineBuff::from_def(by_code(8).unwrap()).unwrap();
        assert_eq!(resfire.stat_lines(), vec![StatLine::new(FIRERESIST, 75)]);
        // Les buffs sans stat propre sont vides (documenté, HUD seulement).
        let stamina = ShrineBuff::from_def(by_code(14).unwrap()).unwrap();
        assert!(stamina.stat_lines().is_empty());
    }

    #[test]
    fn world_recharge_refills_life_and_mana() {
        let mut w = world();
        w.hero.stats.life = 10;
        w.hero.stats.mana = 5;
        let (max_life, max_mana) = (w.hero.stats.max_life, w.hero.stats.max_mana);
        let (healed, mana) = w.apply_shrine_recharge();
        assert_eq!(healed, max_life - 10);
        assert_eq!(mana, max_mana - 5);
        assert_eq!(w.hero.stats.life, max_life);
        assert_eq!(w.hero_stats().mana, max_mana, "accesseur public en phase");
        // Pleine vie : no-op propre.
        assert_eq!(w.apply_shrine_recharge(), (0, 0));
    }

    #[test]
    fn world_container_drops_from_tc_and_flushes_events_next_tick() {
        let mut w = world();
        let defs = w.use_container(3.5, 4.5, "Act 1 Chest B", 1);
        assert!(!defs.is_empty(), "un coffre Act 1 lâche au moins un drop");
        // Pattern NpcTalked : émis hors tick, flushés en tête du tick suivant.
        assert!(w
            .pending_events
            .iter()
            .all(|e| matches!(e, GameEvent::ItemDropped { .. })));
        assert_eq!(w.pending_events.len(), defs.len());
        let items_before = w.items().len();
        assert!(items_before >= defs.len(), "chaque drop est posé au sol");
        for (i, d) in defs.iter().enumerate() {
            assert_eq!(w.items()[items_before - defs.len() + i].x, 3.5);
        }
        let ev = w.tick(0, None);
        assert_eq!(
            ev.iter().filter(|e| matches!(e, GameEvent::ItemDropped { .. })).count(),
            defs.len(),
            "ItemDropped livrés au tick suivant"
        );
        assert!(w.pending_events.is_empty());
    }

    #[test]
    fn world_container_unknown_tc_falls_back_without_panic() {
        let mut w = world();
        // TC inconnue : repli MVP (`roll_fallback` — peut lâcher 0 drop de
        // façon légitime, colonne « rien ») — l'important : jamais de panique.
        let _ = w.use_container(1.0, 1.0, "Act 9 Chest Z", 1);
        // Junk Act 1 : potions/misc/or — au moins un drop, codes plausibles.
        let junk = w.use_container(2.0, 2.0, "Act 1 Junk", 1);
        assert!(!junk.is_empty());
        assert!(junk.iter().all(|d| matches!(d.kind, crate::ItemKind::Potion | crate::ItemKind::Gold | crate::ItemKind::Misc)));
    }
}
