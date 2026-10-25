//! Moteur de quêtes ACTES II-III — implémentation de la spec
//! `d2w-re/specs/quests-actes.md` §3/§4 (campagne RE d2w-re, étape 8d).
//!
//! Même pattern que [`crate::quests_actes`] (acte I) : events d'entrée
//! [`ActEvent`] RÉUTILISÉS (pas de nouveau type — le gameplay émet le même
//! flux pour les deux journaux), machine à phases [`ActPhase`], flags u16
//! façon .d2s (bits identiques), save JSON versionnée strict. 100 % pur ;
//! le bloc `impl World` additif en bas fournit le branchement + la save
//! (`quest_actes2`, champ `#[serde(default)]` — save v3 inchangée).
//!
//! # Ordre des slots = ordre de SAUVEGARDE .d2s (≠ ordre du journal wiki)
//! L'acte III est le cas d'école de la spec §1.3 : les clés `qstsa3q*` du
//! string.tbl mappent le slot de sauvegarde, PAS l'ordre wiki. Ici les ids
//! SONT les clés de sauvegarde :
//! * Acte II (u16 @363 + 2·slot + 96·diff) : `a2q1` Radament → `a2q2` Bâton
//!   Horadrim → `a2q3` Soleil Maudit → `a2q4` Sanctuaire des Arcanes →
//!   `a2q5` Invocateur → `a2q6` Les Sept Tombeaux (ordre wiki = ordre save).
//! * Acte III (u16 @379 + 2·slot + 96·diff) : slot 0 `a3q1` = **Livre de
//!   Lam Esen** (wiki « A3Q4 », `qstsa3q1`), slot 1 `a3q2` = **Volonté de
//!   Khalim** (wiki « A3Q3 », `qstsa3q2`), slot 2 `a3q3` = **Gidbinn**
//!   (wiki « A3Q2 », `qstsa3q3`), slot 3 `a3q4` = **L'Oiseau de Feu**
//!   (wiki « A3Q1 », `qstsa3q4`), slot 4 `a3q5` = **Le Temple noirci**
//!   (wiki « A3Q5 »), slot 5 `a3q6` = **Le Gardien** (wiki « A3Q6 »).
//! Table golden [`Act2Quests::d2s_flag_offset`] verrouillée par tests.
//!
//! # Extensions du pattern acte I (justifiées par les machines de la spec)
//! * [`Trig::Any`] — sous-objectifs HORS ORDRE : le Cube/le manche/l'amulette
//!   (a2q2, spec §3 « 3 sous-objectifs ») et les 4 reliques de Khalim
//!   (a3q2) se prennent dans n'importe quel ordre ; chaque alternative est
//!   comptée une fois via le masque `subdone` (état JSON additif, PAS un bit
//!   .d2s — les u16 .d2s restent l'image exacte des bits documentés).
//! * `requires_done` — gating inter-quêtes explicites de la spec : a2q5 et
//!   a2q6 attendent a2q4 (« COMPLET (déclenche A2Q5/A2Q6) », spec §3),
//!   a3q6 attend a3q2 Khalim (« ACTIF (après A3Q3) », spec §4).
//! * [`Act2Reward`] — variantes A2/A3 : `StatPoint` (Lam Esen +5 [CITÉ
//!   Arreat Summit]), `MaxLife` (Oiseau de Feu +20 [CITÉ]), `OpenPortal`
//!   (portail rouge a2q5 → Canyon id 46 [FIDÈLE]).
//!
//! # Boss scriptés (mission étape 8d — DONNÉES RÉELLES uniquement)
//! * Records patch_monstats v1.12 (lues RUNTIME, aucune constante dupliquée)
//!   : **duriel** (`patch_monstats.txt:213`, Level **22** N, HP 4757, AC 85,
//!   Exp 1655, TC « Duriel » :677-679 + `tsc` ×1 garanti « Duriel »
//!   TreasureClassEx.txt:830) et **mephisto** (`patch_monstats.txt:244`,
//!   Level **26** N, HP 5695, AC 124, Exp 2131, TC « Mephisto » :689-694).
//! * Stats effectives = ligne `MonLvl.txt` (N) du niveau du boss
//!   ([`boss_stats`] → [`SpawnRule`]) — même convention que d2game
//!   (`plan_level_spawns_diff`) : HP% ligne 22 = 84, ligne 26 = 106, AC
//!   directe 132/156, TH 221/265. Résists Real `[ResDm,Ma,Fi,Li,Co,Po]`.
//! * Spawn scripté au trigger zone (pattern « boss scriptés ») : entrer le
//!   repaire de Duriel (**id 73**, `patch_levels.txt:75`) pendant que a2q6
//!   attend le kill fait apparaître Duriel près du héros
//!   ([`World::quest_actes2_event`]) ; idem Mephisto en Durance 3
//!   (**id 102**, `patch_levels.txt:104`) pour a3q6.
//! * Les superuniques A2/A3 (Radament `superuniques.txt:12`, Bloodwitch :13,
//!   Fangskin :14, Coldworm :17, Summoner :20, Endugu :24, Grand Conseil
//!   :28-33) restent portés par le chemin data-driven d2game
//!   (`plan_level_spawns_diff` / superuniques) — ce module ne fait que
//!   consommer leurs kills via [`ActEvent::MonsterKilled`].
//!
//! # DRLG (cartes A2-A3, coéquipiers council)
//! Les niveaux A2-A3 sont générés en parallèle (contrats
//! `generate(level_id, seed)` / `warp_graph` de d2game). À ce jour l'API
//! A2-A3 n'est PAS fusionnée : les tests d'intégration de ce module
//! utilisent des grilles marchables STUBS (carré ouvert) et ne consomment
//! QUE des ids de niveaux (`patch_levels.txt`) — aucun layout codé en dur.
//! Le branchement warp (portail rouge Canyon/Durance) reste d2game.
//!
//! # Triggers réels (spec §3/§4, ids re-vérifiés `patch_levels.txt`)
//! * a2q1 : Radament (superunique :12 / record quête `radament` :231),
//!   égouts id 47 [ARRANGÉ-MVP : étage retail non prouvé sans DLL].
//! * a2q2 : Cube `box` (Halls of the Dead 3 id 60, Bloodwitch :13), manche
//!   `msf` (Maggot Lair 3 id 64, Coldworm :17), amulette `vip` (Claw Viper
//!   2 id 61, Fangskin :14) → transmute → Bâton `hst` (`patch_weapons`).
//! * a2q3 : Oasis lointaine id 43 → autel (Claw Viper 2 id 61) → villageois.
//! * a2q4 : Jerhyn → Sanctuaire des Arcanes id 74 (`Quest = 11`,
//!   `patch_levels.txt:76`) ; palais ouvert dès l'activation [CITÉ].
//! * a2q5 : Invocateur (superunique :20 / record `summoner` :252) →
//!   révélation du symbole (retour ville, Drognan) → portail rouge Canyon
//!   id 46, wp 17.
//! * a2q6 : Tombeau de Tal Rasha — 7 tombeaux ids 66-72 (`QuestFlag = 13`)
//!   [ARRANGÉ-MVP : id 68 fixe] → orifice `hst` → repaire id 73 → Duriel →
//!   Tyrael → Meshif → accès acte III.
//! * a3q1 : livre `bbb` (Ruined Temple id 94 [ARRANGÉ-MVP : 1 des 6 temples
//!   ids 94-99]) → Alkor +5 stats.
//! * a3q2 : reliques `qey` (Spider Cave id 84), `qbr` (Flayer Dungeon ids
//!   88-89), `qf1` (Grand Conseil, Travincal id 83), `qhr` (A3 Sewers 2
//!   id 93) → transmute `qf2` → orbe (Travincal) → accès Durance 100-102.
//! * a3q3 : autel Gidbinn (Flayer Jungle id 78) → Gidbinn `g33` → Ormus.
//! * a3q4 : Witch Doctor Endugu (Flayer Dungeon [ARRANGÉ-MVP]) → figurine
//!   `j34` → Meshif → Alkor → +20 vie max.
//! * a3q5 : jalon Travincal id 83 [ARRANGÉ-MVP].
//! * a3q6 : Durance 3 id 102 → Mephisto → pierre d'âme `mss` → accès acte IV.
//!
//! # Récompenses
//! Récompenses NPC [CITÉ Arreat Summit via spec §3/§4] portées par le
//! journal ; bonus XP **[INVENTÉ]** (plafonds de notre cru, à remplacer si
//! la carto D2Game.dll sort mieux — spec §7).

use serde::{Deserialize, Serialize};

use crate::quests_actes::{ActEvent, ActPhase};
use crate::world::World;
use crate::AnimState;

// Ré-export des bits u16 .d2s (sémantique identique acte I, spec §1.3).
pub use crate::quests_actes::{FLAG_DONE, FLAG_GIVEN_BY_NPC, FLAG_REWARD_PENDING};

// ---------------------------------------------------------------------------
// Récompenses (spec §1.2 — events de sortie A2/A3)
// ---------------------------------------------------------------------------

/// Récompense émise par le journal A2-A3 (appliquée par
/// [`World::quest_actes2_event`], à l'exception des portées par le journal
/// lui-même : accès/portails, cf. variantes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act2Reward {
    /// XP de quête ([INVENTÉ] — bonus retail duré dans le DLL).
    Xp(u32),
    /// Point(s) de compétence (a2q1 Book of Skill [FIDÈLE]).
    SkillPoint(u32),
    /// Point(s) de caractéristique (a3q1 Lam Esen +5 [CITÉ Arreat Summit]).
    StatPoint(u32),
    /// Vie max PERMANENTE (a3q4 Potion of Life +20 [CITÉ]).
    MaxLife(u32),
    /// Objet par code table (a2q2 Bâton `hst`, a3q3 anneau `rin` [CITÉ]).
    Item(&'static str),
    /// Portail ouvert vers un niveau id (a2q5 → Canyon of the Magi id 46
    /// [FIDÈLE]) — porté AUSSI par [`Act2Quests::canyon_portal_open`] ;
    /// le branchement warp (contrat `warp_graph` DRLG) est d2game.
    OpenPortal(u32),
    /// Accès à l'acte suivant (a2q6 Meshif → acte III, a3q6 portail rouge →
    /// acte IV [FIDÈLE]) + progression — portés par
    /// [`Act2Quests::access_act`].
    UnlockAccessAct(u8),
}

// ---------------------------------------------------------------------------
// Triggers / authoring
// ---------------------------------------------------------------------------

/// Trigger d'une étape (authoring statique — même esprit que l'acte I, plus
/// la variante [`Trig::Any`] pour les sous-objectifs hors ordre).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trig {
    /// Dialogue NPC (code quête bas de casse).
    Talk(&'static str),
    /// Entrée dans un niveau (id `patch_levels.txt`).
    Zone(u32),
    /// Mort d'un monstre — n'importe quel alias : code monstats (base ou
    /// record de quête) OU code sprite manifest OU nom de superunique.
    /// Table vide = JOKER (n'importe quel kill — unique invoqué a3q3).
    Kill(&'static [&'static str]),
    /// Ramassage d'un objet par code.
    Item(&'static str),
    /// Sous-objectifs HORS ORDRE : chaque alternative doit se produire une
    /// fois (bit `subdone` de [`Act2Run`]) pour franchir l'étape.
    Any(&'static [&'static Trig]),
}

impl Trig {
    fn matches(&self, ev: &ActEvent) -> bool {
        match (self, ev) {
            (Trig::Talk(n), ActEvent::NpcTalked { npc }) => npc.eq_ignore_ascii_case(n),
            (Trig::Zone(id), ActEvent::ZoneEntered { level_id }) => id == level_id,
            (Trig::Item(c), ActEvent::ItemPicked { code }) => code.eq_ignore_ascii_case(c),
            (Trig::Kill(aliases), ActEvent::MonsterKilled { code, superunique }) => aliases
                .iter()
                .any(|a| {
                    code.eq_ignore_ascii_case(a)
                        || superunique.as_deref().map_or(false, |s| s.eq_ignore_ascii_case(a))
                }),
            _ => false,
        }
    }
}

/// Étape séquentielle d'une quête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepDef {
    /// Libellé FR (string.tsv via spec §3/§4, condensé).
    pub desc: &'static str,
    /// Ce qui fait avancer l'étape.
    pub trig: Trig,
    /// Occurrences requises (occurrences d'alternatives pour [`Trig::Any`]).
    pub count: u32,
}

/// Définition d'une quête des actes II-III.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefAct23 {
    /// Identifiant stable = CLÉ DE SAUVEGARDE (`a2q1`…`a2q6`, `a3q1`…`a3q6`
    /// — clés `qstsaNqM` du string.tbl, cf. en-tête de module).
    pub id: &'static str,
    /// Nom EN (wiki ; libellé fr = TBL `qstsaNq*`).
    pub name: &'static str,
    /// Étapes séquentielles.
    pub steps: &'static [StepDef],
    /// PNJ de remise de récompense (`None` = complétion directe à la
    /// dernière étape).
    pub reward_npc: Option<&'static str>,
    /// Récompenses rendues à la complétion, dans cet ordre.
    pub rewards: &'static [Act2Reward],
    /// Slot (journal 12 quêtes) qui doit être FLAG_DONE pour ACTIVER cette
    /// quête (`None` = activation au premier trigger, comme l'acte I).
    pub requires_done: Option<usize>,
}

/// Ids dans l'ordre des slots de save — ACTE II (u16 @363-373 Normal).
pub const ACT2_QUEST_IDS: [&str; 6] = ["a2q1", "a2q2", "a2q3", "a2q4", "a2q5", "a2q6"];
/// Ids dans l'ordre des slots de save — ACTE III (u16 @379-389 Normal).
/// ATTENTION : `a3q1` = Lam Esen (wiki « A3Q4 »), `a3q4` = Oiseau de Feu
/// (wiki « A3Q1 ») — mapping `qstsa3q*` string.tbl, PAS l'ordre du journal.
pub const ACT3_QUEST_IDS: [&str; 6] = ["a3q1", "a3q2", "a3q3", "a3q4", "a3q5", "a3q6"];

/// Slots GLOBAUX du journal 12 quêtes (0-5 = acte II, 6-11 = acte III).
mod slot {
    pub const A2Q6: usize = 5;
    pub const A3Q2: usize = 7; // Khalim (clé de save a3q2) — dépendance a3q6
    pub const A3Q6: usize = 11;
}

/// `patch_levels.txt` : ids de niveaux réels des triggers (re-vérifiés le
/// 2026-10-24 — cf. en-tête de module).
mod lvl {
    pub const FAR_OASIS: u32 = 43;
    pub const CANYON_OF_THE_MAGI: u32 = 46;
    pub const A2_SEWERS_1: u32 = 47;
    pub const PALACE_CELLAR_3: u32 = 54;
    pub const HALLS_OF_DEAD_3: u32 = 60;
    pub const CLAW_VIPER_2: u32 = 61;
    pub const MAGGOT_LAIR_3: u32 = 64;
    /// 7 tombeaux de Tal Rasha = ids 66..=72 (QuestFlag 13).
    pub const TAL_TOMBS: [u32; 7] = [66, 67, 68, 69, 70, 71, 72];
    /// [ARRANGÉ-MVP] vrai tombeau fixe = id 68 (tirage retail aléatoire).
    pub const TAL_TOMB_MVP: u32 = 68;
    pub const DURIELS_LAIR: u32 = 73;
    pub const ARCANE_SANCTUARY: u32 = 74;
    pub const FLAYER_JUNGLE: u32 = 78;
    pub const TRAVINCAL: u32 = 83;
    pub const SPIDER_CAVE: u32 = 84;
    pub const FLAYER_DUNGEON_1: u32 = 88;
    pub const FLAYER_DUNGEON_2: u32 = 89;
    pub const A3_SEWERS_2: u32 = 93;
    pub const RUINED_TEMPLE: u32 = 94;
    pub const DURANCE_3: u32 = 102;
    /// Catacombes 4 (repaire d'Andariel, a1q6 — `patch_levels.txt:39`
    /// « Act 1 - Catacombs 4 ») — repère du record [`super::ANDARIEL`]
    /// (étape 21a, GAPS 6).
    pub const CATACOMBS_4: u32 = 37;
}

/// Villageois de Lut Gholein acceptés pour a2q3 étape 3 (« Parler avec les
/// villageois » — n'importe lequel, spec §3).
static A2_VILLAGER_TRIGS: [&Trig; 8] = [
    &Trig::Talk("atma"),
    &Trig::Talk("cain"),
    &Trig::Talk("greiz"),
    &Trig::Talk("drognan"),
    &Trig::Talk("elzix"),
    &Trig::Talk("fara"),
    &Trig::Talk("lysander"),
    &Trig::Talk("warriv"),
];

/// Authoring complet de l'ACTE II (spec §3 — ordre de sauvegarde a2q1→a2q6).
pub(crate) static DEFS_ACT2: [DefAct23; 6] = [
    // a2q1 — Radament's Lair : Kill(Radament) → remise Atma → Book of Skill
    // (+1 skill). Ramassage du livre optionnel absorbé par la récompense
    // [ARRANGÉ-MVP].
    DefAct23 {
        id: "a2q1",
        name: "Radament's Lair",
        steps: &[StepDef {
            desc: "Trouvez le repaire de Radament dans les égouts de Lut Gholein et abattez-le.",
            trig: Trig::Kill(&["radament", "RD"]),
            count: 1,
        }],
        reward_npc: Some("atma"),
        rewards: &[Act2Reward::SkillPoint(1), Act2Reward::Xp(300)],
        requires_done: None,
    },
    // a2q2 — The Horadric Staff : 3 sous-objectifs HORS ORDRE (Cube `box`
    // id 60 / manche `msf` id 64 / amulette `vip` id 61) → remise à Cain
    // (transmute msf+vip→hst [ARRANGÉ-MVP : recette implicite à la remise]).
    DefAct23 {
        id: "a2q2",
        name: "The Horadric Staff",
        steps: &[StepDef {
            desc: "Retrouvez le Cube, le manche du Bâton et l'Amulette du Bâton (dans cet ordre ou un autre).",
            trig: Trig::Any(&[&Trig::Item("box"), &Trig::Item("msf"), &Trig::Item("vip")]),
            count: 3,
        }],
        reward_npc: Some("cain"),
        rewards: &[Act2Reward::Item("hst"), Act2Reward::Xp(300)],
        requires_done: None,
    },
    // a2q3 — Tainted Sun : éclipse (Oasis lointaine id 43) → autel détruit
    // (Claw Viper 2 id 61 [ARRANGÉ-MVP : l'entrée vaut destruction]) →
    // n'importe quel villageois → COMPLET. XP seulement [FIDÈLE].
    DefAct23 {
        id: "a2q3",
        name: "Tainted Sun",
        steps: &[
            StepDef { desc: "Recherchez la source de l'obscurité (Oasis lointaine).", trig: Trig::Zone(lvl::FAR_OASIS), count: 1 },
            StepDef { desc: "Détruisez l'autel du Temple de la Vipère de Griffe.", trig: Trig::Zone(lvl::CLAW_VIPER_2), count: 1 },
            StepDef {
                desc: "Parlez avec les villageois.",
                trig: Trig::Any(&A2_VILLAGER_TRIGS),
                count: 1,
            },
        ],
        reward_npc: None,
        rewards: &[Act2Reward::Xp(200)],
        requires_done: None,
    },
    // a2q4 — Arcane Sanctuary : Jerhyn (palais ouvert [CITÉ]) → Sanctuaire
    // id 74 (traversée Harem/Basements ids 50-54 implicite ; journal
    // d'Horazon [ARRANGÉ-MVP : 1 aile = l'entrée]) → COMPLET → déclenche
    // a2q5/a2q6 (requires_done).
    DefAct23 {
        id: "a2q4",
        name: "Arcane Sanctuary",
        steps: &[
            StepDef { desc: "Parlez à Jerhyn et entrez dans le Palais.", trig: Trig::Talk("jerhyn"), count: 1 },
            StepDef { desc: "Trouvez le Sanctuaire des Arcanes et le Journal d'Horazon.", trig: Trig::Zone(lvl::ARCANE_SANCTUARY), count: 1 },
        ],
        reward_npc: None,
        rewards: &[Act2Reward::Xp(250)],
        requires_done: None,
    },
    // a2q5 — The Summoner : Kill(Invocateur) → révélation du symbole au
    // retour ville (Drognan [ARRANGÉ-MVP]) → COMPLET + portail rouge Canyon
    // (id 46, wp 17). GATÉ sur a2q4 (spec : « déclenche A2Q5 »).
    DefAct23 {
        id: "a2q5",
        name: "The Summoner",
        steps: &[
            StepDef { desc: "Tuez l'Invocateur au cœur du Sanctuaire des Arcanes.", trig: Trig::Kill(&["summoner", "SU"]), count: 1 },
            StepDef { desc: "Retournez en ville faire révéler le symbole du vrai tombeau.", trig: Trig::Talk("drognan"), count: 1 },
        ],
        reward_npc: None,
        rewards: &[Act2Reward::OpenPortal(lvl::CANYON_OF_THE_MAGI), Act2Reward::Xp(400)],
        requires_done: Some(3),
    },
    // a2q6 — The Seven Tombs : vrai tombeau (id 68 MVP) → orifice `hst` →
    // portail → repaire de Duriel (id 73, spawn scripté) → kill → Tyrael →
    // Meshif → UnlockAccessAct(3). GATÉ sur a2q4 (spec : « gating vers
    // A2Q5/A2Q6 »). QUÊTE PORTEUSE.
    DefAct23 {
        id: "a2q6",
        name: "The Seven Tombs",
        steps: &[
            StepDef { desc: "Trouvez le Tombeau de Tal Rasha (Canyon des Mages).", trig: Trig::Zone(lvl::TAL_TOMB_MVP), count: 1 },
            StepDef { desc: "Présentez le Bâton Horadrim à l'orifice et entrez dans le repaire de Duriel.", trig: Trig::Zone(lvl::DURIELS_LAIR), count: 1 },
            StepDef { desc: "Tuez Duriel.", trig: Trig::Kill(&["duriel", "DU"]), count: 1 },
            StepDef { desc: "Parlez à Tyrael dans la Chambre de Tal Rasha.", trig: Trig::Talk("tyrael"), count: 1 },
        ],
        reward_npc: Some("meshif"),
        rewards: &[Act2Reward::UnlockAccessAct(3), Act2Reward::Xp(1000)],
        requires_done: Some(3),
    },
];

/// Authoring complet de l'ACTE III (spec §4 — ORDRE DE SAUVEGARDE
/// `qstsa3q1`→`qstsa3q6`, ≠ ordre du journal wiki, cf. en-tête de module).
pub(crate) static DEFS_ACT3: [DefAct23; 6] = [
    // slot 0 — a3q1 = Livre de Lam Esen (wiki « A3Q4 ») : Ruined Temple
    // id 94 [ARRANGÉ-MVP : 1 des 6 temples] → livre `bbb` → Alkor (+5 stats).
    DefAct23 {
        id: "a3q1",
        name: "Lam Esen's Tome",
        steps: &[
            StepDef { desc: "Fouillez les six temples de Kurast (Ruined Temple).", trig: Trig::Zone(lvl::RUINED_TEMPLE), count: 1 },
            StepDef { desc: "Récupérez le livre de Lam Esen.", trig: Trig::Item("bbb"), count: 1 },
        ],
        reward_npc: Some("alkor"),
        rewards: &[Act2Reward::StatPoint(5), Act2Reward::Xp(200)],
        requires_done: None,
    },
    // slot 1 — a3q2 = Khalim's Will (wiki « A3Q3 ») : 4 reliques HORS ORDRE
    // (`qey` id 84 / `qbr` ids 88-89 / `qf1` Grand Conseil id 83 / `qhr`
    // id 93) → Volonté de Khalim `qf2` (transmute) → orbe brisée
    // (Travincal id 83 [ARRANGÉ-MVP : l'entrée avec `qf2`]) → accès
    // Durance ids 100-102 [FIDÈLE, porté par `durance_open`].
    DefAct23 {
        id: "a3q2",
        name: "Khalim's Will",
        steps: &[
            StepDef {
                desc: "Réunissez l'Œil, le Cerveau, le Fléau et le Cœur de Khalim.",
                trig: Trig::Any(&[&Trig::Item("qey"), &Trig::Item("qbr"), &Trig::Item("qf1"), &Trig::Item("qhr")]),
                count: 4,
            },
            StepDef { desc: "Transmutez les reliques en Volonté de Khalim avec le Cube Horadrim.", trig: Trig::Item("qf2"), count: 1 },
            StepDef { desc: "Brissez l'Orbe d'envoûtement à Travincal.", trig: Trig::Zone(lvl::TRAVINCAL), count: 1 },
        ],
        reward_npc: None,
        rewards: &[Act2Reward::Xp(500)],
        requires_done: None,
    },
    // slot 2 — a3q3 = Blade of the Old Religion / Gidbinn (wiki « A3Q2 ») :
    // autel (Flayer Jungle id 78) → unique invoqué (joker kill [ARRANGÉ-MVP])
    // → Gidbinn `g33` → Ormus (anneau `rin` [CITÉ]).
    DefAct23 {
        id: "a3q3",
        name: "Blade of the Old Religion",
        steps: &[
            StepDef { desc: "Recherchez le Gidbinn dans la jungle de l'Écorcheur.", trig: Trig::Zone(lvl::FLAYER_JUNGLE), count: 1 },
            StepDef { desc: "Abattez le gardien invoqué par l'autel.", trig: Trig::Kill(&[]), count: 1 },
            StepDef { desc: "Récupérez le Gidbinn.", trig: Trig::Item("g33"), count: 1 },
        ],
        reward_npc: Some("ormus"),
        rewards: &[Act2Reward::Item("rin"), Act2Reward::Xp(200)],
        requires_done: None,
    },
    // slot 3 — a3q4 = The Golden Bird (wiki « A3Q1 ») : Endugu
    // [ARRANGÉ-MVP : unique aléatoire en retail] → figurine de jade `j34` →
    // Meshif (reçoit l'Oiseau `g34` [ARRANGÉ-MVP : transit implicite]) →
    // Alkor → Potion of Life (+20 vie max [CITÉ]).
    DefAct23 {
        id: "a3q4",
        name: "The Golden Bird",
        steps: &[
            StepDef { desc: "Abattez le Witch Doctor Endugu dans la jungle de l'Écorcheur.", trig: Trig::Kill(&["endugu", "fetishshaman4"]), count: 1 },
            StepDef { desc: "Ramassez la figurine de jade.", trig: Trig::Item("j34"), count: 1 },
            StepDef { desc: "Confiez la figurine à Meshif, puis l'Oiseau de Feu à Alkor.", trig: Trig::Talk("meshif"), count: 1 },
        ],
        reward_npc: Some("alkor"),
        rewards: &[Act2Reward::MaxLife(20), Act2Reward::Xp(300)],
        requires_done: None,
    },
    // slot 4 — a3q5 = The Blackened Temple (wiki « A3Q5 ») : jalon
    // Travincal id 83 [ARRANGÉ-MVP : quête-jalon sans trigger dur].
    DefAct23 {
        id: "a3q5",
        name: "The Blackened Temple",
        steps: &[StepDef {
            desc: "Trouvez le temple noirci dans Travincal.",
            trig: Trig::Zone(lvl::TRAVINCAL),
            count: 1,
        }],
        reward_npc: None,
        rewards: &[Act2Reward::Xp(200)],
        requires_done: None,
    },
    // slot 5 — a3q6 = The Guardian (wiki « A3Q6 ») : Durance of Hate 3
    // id 102 (spawn scripté Mephisto) → kill → pierre d'âme `mss` (drop
    // garanti) → portail rouge → UnlockAccessAct(4). QUÊTE PORTEUSE. GATÉE
    // sur Khalim (a3q2 slot 1) — « ACTIF (après A3Q3) », spec §4.
    DefAct23 {
        id: "a3q6",
        name: "The Guardian",
        steps: &[
            StepDef { desc: "Descendez jusqu'à la Durance de la Haine, niveau 3.", trig: Trig::Zone(lvl::DURANCE_3), count: 1 },
            StepDef { desc: "Tuez Mephisto.", trig: Trig::Kill(&["mephisto", "MP"]), count: 1 },
            StepDef { desc: "Ramassez la Pierre d'âme de Mephisto.", trig: Trig::Item("mss"), count: 1 },
        ],
        reward_npc: None,
        rewards: &[Act2Reward::UnlockAccessAct(4), Act2Reward::Xp(1000)],
        requires_done: Some(slot::A3Q2),
    },
];

// ---------------------------------------------------------------------------
// État + moteur
// ---------------------------------------------------------------------------

/// État d'exécution d'une quête A2/A3 (sérialisé, façon flags u16 .d2s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Act2Run {
    /// Phase courante.
    pub phase: ActPhase,
    /// Index de l'étape courante (figé à la dernière quand la quête sort
    /// d'Active).
    pub step: usize,
    /// Occurrences acquises sur l'étape courante.
    pub count: u32,
    /// Flags u16 .d2s (bits identiques à l'acte I : 0 = faite, 1 = récompense
    /// à prendre, 2 = donnée par PNJ).
    pub flags: u16,
    /// Masque des sous-objectifs d'une étape [`Trig::Any`] (bit i = alternative
    /// i déjà comptée). État MOTEUR additif — PAS un bit .d2s (les u16 de la
    /// spec n'ont pas de bits de sous-objectifs documentés) ; remis à zéro à
    /// chaque changement d'étape.
    pub subdone: u32,
}

/// Journal de quêtes des ACTES II-III (spec quests-actes.md §3/§4).
/// 12 slots : 0-5 = acte II (`a2q1`→`a2q6`), 6-11 = acte III (`a3q1`→`a3q6`
/// en ORDRE DE SAUVEGARDE). Authoring statique ; seul l'état est sérialisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act2Quests {
    runs: [Act2Run; 12],
}

impl Default for Act2Quests {
    fn default() -> Self {
        Self::new()
    }
}

impl Act2Quests {
    /// Journal neuf : tout INACTIF, flags 0.
    pub fn new() -> Act2Quests {
        Act2Quests {
            runs: [Act2Run { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0, subdone: 0 }; 12],
        }
    }

    /// Déf d'un slot global (0-11).
    pub(crate) fn def_of(&self, slot: usize) -> &'static DefAct23 {
        if slot < 6 {
            &DEFS_ACT2[slot]
        } else {
            &DEFS_ACT3[slot - 6]
        }
    }

    /// Ids des 12 slots (ordre de sauvegarde).
    pub fn ids(&self) -> [&'static str; 12] {
        let mut out = [""; 12];
        for (i, id) in ACT2_QUEST_IDS.into_iter().chain(ACT3_QUEST_IDS).enumerate() {
            out[i] = id;
        }
        out
    }

    /// État d'une quête par id (`None` si id inconnu).
    pub fn run(&self, id: &str) -> Option<&Act2Run> {
        self.ids().iter().position(|s| *s == id).map(|i| &self.runs[i])
    }

    /// Phases dans l'ordre des slots (diff/events UI).
    pub fn phases(&self) -> [ActPhase; 12] {
        let mut out = [ActPhase::Inactive; 12];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.phase;
        }
        out
    }

    /// Flags u16 dans l'ordre des slots — image directe des u16 .d2s des
    /// actes II-III (offsets [`Act2Quests::d2s_flag_offset`]).
    pub fn quest_flags(&self) -> [u16; 12] {
        let mut out = [0u16; 12];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.flags;
        }
        out
    }

    /// Gating d'accès (spec §1.4). Cette journal couvre les actes II-III :
    /// acte 2 = prérequis porté par le journal A1 (a1q6) — considéré ouvert ;
    /// acte 3 = a2q6 bit 0 (remise Meshif, bateau) ; acte 4 = a3q6 bit 0
    /// (portail rouge) ; au-delà : jamais (actes IV-V hors scope).
    pub fn access_act(&self, act: u8) -> bool {
        match act {
            1 | 2 => true,
            3 => self.runs[slot::A2Q6].flags & FLAG_DONE != 0,
            4 => self.runs[slot::A3Q6].flags & FLAG_DONE != 0,
            _ => false,
        }
    }

    /// Palais de Lut Gholein : ouvert dès l'ACTIVATION d'a2q4 (spec §1.4,
    /// texte Jerhyn string.tsv:377 [CITÉ]).
    pub fn palace_open(&self) -> bool {
        self.runs[3].phase != ActPhase::Inactive
    }

    /// Portail rouge vers le Canyon des Mages (id 46, wp 17) : ouvert à la
    /// complétion d'a2q5 (symbole révélé, spec §3 [FIDÈLE]).
    pub fn canyon_portal_open(&self) -> bool {
        self.runs[4].phase == ActPhase::Done
    }

    /// Accès à la Durance de la Haine (ids 100-102, QuestFlag 21) : ouvert à
    /// la complétion de Khalim (a3q2, spec §4 [FIDÈLE]).
    pub fn durance_open(&self) -> bool {
        self.runs[slot::A3Q2].phase == ActPhase::Done
    }

    /// Octet de progression .d2s 0x0025 — part locale de CE journal : nb de
    /// quêtes porteuses complétées (0, 1 après a2q6, 2 après a3q6). L'octet
    /// complet = cette valeur + la part du journal A1 ([`crate::quests_actes::ActQuests::progression_byte`]).
    pub fn progression_byte(&self) -> u8 {
        u8::from(self.access_act(3)) + u8::from(self.access_act(4))
    }

    /// Fait avancer le journal avec un événement ; renvoie les récompenses
    /// rendues. Un événement n'avance qu'UNE occurrence de l'étape courante
    /// d'UNE quête (par slot) ; hors ordre = ignoré (comme l'acte I/retail).
    pub fn on_event(&mut self, ev: &ActEvent) -> Vec<Act2Reward> {
        let mut out = Vec::new();
        for slot in 0..12 {
            match self.runs[slot].phase {
                ActPhase::Inactive => {
                    // Gating inter-quêtes (a2q5/a2q6 ← a2q4, a3q6 ← Khalim).
                    if let Some(dep) = self.def_of(slot).requires_done {
                        if self.runs[dep].flags & FLAG_DONE == 0 {
                            continue;
                        }
                    }
                    // Activation = franchissement de l'étape 0 : même
                    // progression que la branche Active (une étape peut
                    // exiger plusieurs occurrences — sous-objectifs Any).
                    if let Some(alt) = self.step_match_index(slot, 0, ev) {
                        self.runs[slot].phase = ActPhase::Active;
                        if matches!(ev, ActEvent::NpcTalked { .. }) {
                            self.runs[slot].flags |= FLAG_GIVEN_BY_NPC;
                        }
                        if matches!(self.def_of(slot).steps[0].trig, Trig::Any(_)) {
                            self.runs[slot].subdone |= 1 << alt;
                        }
                        self.runs[slot].count += 1;
                        let req = self.def_of(slot).steps[0].count.max(1);
                        if self.runs[slot].count >= req {
                            self.advance_after_step(slot, 0, &mut out);
                        }
                    }
                }
                ActPhase::Active => {
                    let step = self.runs[slot].step;
                    if let Some(alt) = self.step_match_index(slot, step, ev) {
                        // Sous-objectifs : marque l'alternative consommée.
                        if matches!(self.def_of(slot).steps[step].trig, Trig::Any(_)) {
                            self.runs[slot].subdone |= 1 << alt;
                        }
                        self.runs[slot].count += 1;
                        let req = self.def_of(slot).steps[step].count.max(1);
                        if self.runs[slot].count >= req {
                            self.advance_after_step(slot, step, &mut out);
                        }
                    }
                }
                ActPhase::RewardPending => {
                    if let ActEvent::NpcTalked { npc } = ev {
                        if self.def_of(slot)
                            .reward_npc
                            .map_or(false, |r| npc.eq_ignore_ascii_case(r))
                        {
                            self.runs[slot].phase = ActPhase::Done;
                            self.runs[slot].flags =
                                (self.runs[slot].flags & !FLAG_REWARD_PENDING) | FLAG_DONE;
                            out.extend(self.def_of(slot).rewards.iter().copied());
                        }
                    }
                }
                ActPhase::Done => {} // terminée : ignore tout
            }
        }
        out
    }

    /// L'étape `step` du slot réagit-elle à cet événement ? Renvoie l'INDEX
    /// de l'alternative matchée (0 pour les triggers simples ; joker Kill =
    /// 0 ; [`Trig::Any`] = première alternative non encore comptée).
    fn step_match_index(&self, slot: usize, step: usize, ev: &ActEvent) -> Option<usize> {
        let def = self.def_of(slot);
        match def.steps.get(step) {
            // Joker (Kill(&[])) : n'importe quelle mort (unique invoqué a3q3).
            Some(StepDef { trig: Trig::Kill(aliases), .. }) if aliases.is_empty() => {
                matches!(ev, ActEvent::MonsterKilled { .. }).then_some(0)
            }
            Some(StepDef { trig: Trig::Any(alts), .. }) => alts.iter().enumerate().find_map(|(i, t)| {
                ((self.runs[slot].subdone >> i) & 1 == 0 && t.matches(ev)).then_some(i)
            }),
            Some(s) => s.trig.matches(ev).then_some(0),
            None => None,
        }
    }

    /// Compat acte I : l'étape `step` réagit-elle à cet événement ?
    #[allow(dead_code)]
    fn step_matches(&self, slot: usize, step: usize, ev: &ActEvent) -> bool {
        self.step_match_index(slot, step, ev).is_some()
    }

    /// Étape `done_step` franchie : suivante, ou sortie d'Active
    /// (RewardPending / Done direct selon `reward_npc`).
    fn advance_after_step(&mut self, slot: usize, done_step: usize, out: &mut Vec<Act2Reward>) {
        let def = self.def_of(slot);
        if done_step + 1 < def.steps.len() {
            self.runs[slot].step = done_step + 1;
            self.runs[slot].count = 0;
            self.runs[slot].subdone = 0;
            return;
        }
        let last = def.steps.len() - 1;
        self.runs[slot].step = last;
        self.runs[slot].count = def.steps[last].count.max(1);
        self.runs[slot].subdone = 0;
        match def.reward_npc {
            Some(_) => {
                self.runs[slot].phase = ActPhase::RewardPending;
                self.runs[slot].flags |= FLAG_REWARD_PENDING;
            }
            None => {
                // Complétion directe à la dernière étape.
                self.runs[slot].phase = ActPhase::Done;
                self.runs[slot].flags = (self.runs[slot].flags & !FLAG_REWARD_PENDING) | FLAG_DONE;
                out.extend(def.rewards.iter().copied());
            }
        }
    }

    // -- mapping .d2s (spec §1.3 — table golden actes II-III) ----------------

    /// Offset absolu du u16 de quête : acte II = 363 + 2·slot + 96·diff
    /// (slots 363/365/…/373), acte III = 379 + 2·slot + 96·diff
    /// (379/381/…/389) ; Cauchemar +96, Enfer +192 (spec §1.3, table
    /// verrouillée par les tests golden `golden_d2s_offsets_actes_23`).
    /// `act` = 2 ou 3, `slot` = 0..=5 (slot LOCAL d'acte), `difficulty` 0..=2.
    pub fn d2s_flag_offset(act: u8, slot: usize, difficulty: usize) -> u32 {
        let base = match act {
            2 => 363,
            3 => 379,
            _ => 0,
        };
        base + 2 * slot as u32 + 96 * difficulty as u32
    }

    // -- persistance (contrat save.rs — champ additif `quest_actes2`) --------

    /// Sérialise l'ÉTAT SEUL (JSON versionné, ids inclus, lisible).
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct RunSave {
            id: &'static str,
            act: u8,
            phase: ActPhase,
            step: usize,
            count: u32,
            flags: u16,
            subdone: u32,
        }
        #[derive(Serialize)]
        struct State {
            version: u32,
            quests: Vec<RunSave>,
        }
        let quests = self
            .runs
            .iter()
            .enumerate()
            .map(|(slot, r)| RunSave {
                id: self.ids()[slot],
                act: if slot < 6 { 2 } else { 3 },
                phase: r.phase,
                step: r.step,
                count: r.count,
                flags: r.flags,
                subdone: r.subdone,
            })
            .collect();
        let s = State { version: STATE_VERSION, quests };
        serde_json::to_string(&s).expect("d2rules: sérialisation quest_actes2")
    }

    /// Recharge un état produit par [`Act2Quests::to_json`]. Validation
    /// stricte (version, ids/actes dans l'ordre, bornes d'étapes, masque
    /// `subdone` cohérent, gating `requires_done` respecté, flags cohérents
    /// avec la phase) — tout écart ⇒ Err(()) : l'appelant garde son état.
    pub fn from_json(json: &str) -> Result<Act2Quests, ()> {
        #[derive(Deserialize)]
        struct RunSave {
            id: String,
            #[serde(default)]
            act: u8,
            phase: ActPhase,
            step: usize,
            count: u32,
            flags: u16,
            #[serde(default)]
            subdone: u32,
        }
        #[derive(Deserialize)]
        struct State {
            version: u32,
            quests: Vec<RunSave>,
        }
        let s: State = serde_json::from_str(json).map_err(|_| ())?;
        if s.version != STATE_VERSION || s.quests.len() != 12 {
            return Err(());
        }
        let mut runs = [Act2Run { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0, subdone: 0 }; 12];
        for (slot, save) in s.quests.into_iter().enumerate() {
            if save.id != ids_slot(slot) || save.act != if slot < 6 { 2 } else { 3 } {
                return Err(());
            }
            let def = defs_slot(slot);
            let last = def.steps.len() - 1;
            let req_last = def.steps[last].count.max(1);
            // Sous-objectifs : bits < nb d'alternatives, popcount = count.
            let any_alts = |step: usize| -> Option<usize> {
                match def.steps.get(step).map(|s| &s.trig) {
                    Some(Trig::Any(alts)) => Some(alts.len()),
                    _ => None,
                }
            };
            let subdone_ok = |step: usize, subdone: u32, count: u32| -> bool {
                match any_alts(step) {
                    Some(alts) => {
                        subdone < (1u32 << alts)
                            && subdone.count_ones() == count
                    }
                    None => subdone == 0,
                }
            };
            let ok = match save.phase {
                ActPhase::Inactive => save.step == 0 && save.count == 0 && save.flags == 0 && save.subdone == 0,
                ActPhase::Active => {
                    save.step <= last
                        && save.count < def.steps[save.step].count.max(1)
                        && subdone_ok(save.step, save.subdone, save.count)
                        && save.flags & !FLAG_GIVEN_BY_NPC == 0
                        && gating_ok(slot, def.requires_done, &runs)
                }
                ActPhase::RewardPending => {
                    def.reward_npc.is_some()
                        && save.step == last
                        && save.count == req_last
                        && save.subdone == 0
                        && save.flags & FLAG_REWARD_PENDING != 0
                        && save.flags & FLAG_DONE == 0
                        && gating_ok(slot, def.requires_done, &runs)
                }
                ActPhase::Done => {
                    save.step == last
                        && save.count == req_last
                        && save.subdone == 0
                        && save.flags & FLAG_DONE != 0
                        && save.flags & FLAG_REWARD_PENDING == 0
                        && gating_ok(slot, def.requires_done, &runs)
                }
            };
            if !ok {
                return Err(());
            }
            runs[slot] =
                Act2Run { phase: save.phase, step: save.step, count: save.count, flags: save.flags, subdone: save.subdone };
        }
        Ok(Act2Quests { runs })
    }
}

const STATE_VERSION: u32 = 1;

/// Id du slot global (helper from_json).
fn ids_slot(slot: usize) -> &'static str {
    if slot < 6 {
        ACT2_QUEST_IDS[slot]
    } else {
        ACT3_QUEST_IDS[slot - 6]
    }
}

/// Déf du slot global (helper from_json).
fn defs_slot(slot: usize) -> &'static DefAct23 {
    if slot < 6 {
        &DEFS_ACT2[slot]
    } else {
        &DEFS_ACT3[slot - 6]
    }
}

/// Gating : la dépendance `requires_done` du slot est-elle satisfaite dans
/// l'état en cours de reconstruction ?
fn gating_ok(slot: usize, requires: Option<usize>, runs: &[Act2Run; 12]) -> bool {
    match requires {
        Some(dep) if dep != slot => runs[dep].flags & FLAG_DONE != 0,
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// BOSS scriptés — duriel (a2q6) + mephisto (a3q6), DONNÉES RÉELLES
// ---------------------------------------------------------------------------

/// Tables embarquées (même arbre assets que d2rules/tables.rs — v1.12).
const PATCH_MONSTATS: &str = include_str!("../../../../assets/patch_monstats.txt");
const MONLVL: &str = include_str!("../../../../assets/MonLvl.txt");

/// Record statique d'un boss scripté (identité + ancrage). Les VALEURS
/// (niveau, HP, AC, XP, TC, résists) sont lues RUNTIME dans
/// `patch_monstats.txt` / `MonLvl.txt` — aucune constante dupliquée :
/// `boss_stats` renvoie Err sur toute table inattendue (build de tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BossRec {
    /// Code monstats (`duriel` — col `Id`, event `MonsterKilled`).
    pub id: &'static str,
    /// Code sprite manifest (`DU` — col `Code`, spawn).
    pub code: &'static str,
    /// Nom affichable.
    pub name: &'static str,
    /// TC de drop (`TreasureClass1-3` monstats — « Duriel » wrap `tsc` ×1
    /// + base, TreasureClassEx.txt:830-835 [VÉRIFIÉ-LOCAL]).
    pub tc: &'static str,
    /// Niveau du spawn scripté (`patch_levels.txt` — repaire de Duriel 73,
    /// Durance 3 102).
    pub lair: u32,
    /// Slot global du journal porteur (a2q6 = 5, a3q6 = 11).
    pub quest_slot: usize,
}

/// Les 2 boss scriptés des actes II-III (mission étape 8d).
pub const BOSSES_ACTS23: [BossRec; 2] = [
    BossRec { id: "duriel", code: "DU", name: "Duriel", tc: "Duriel", lair: lvl::DURIELS_LAIR, quest_slot: 5 },
    BossRec { id: "mephisto", code: "MP", name: "Mephisto", tc: "Mephisto", lair: lvl::DURANCE_3, quest_slot: 11 },
];

/// Valeurs réelles d'un boss lues dans les tables (`boss_stats`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BossStats {
    /// Record statique source.
    pub rec: BossRec,
    /// Niveau Normal (`Level` monstats ; NM/Enfer = `Level(N)/(H)`, hors
    /// scope ce module — le journal MVP couvre Normal).
    pub level: u32,
    /// PV bruts (`minHP` = `maxHP` pour les 2 boss).
    pub hp: u32,
    /// AC de table (`AC` monstats).
    pub ac: u32,
    /// XP de table (`Exp` monstats).
    pub exp: u32,
    /// Résists Normal `[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]` (vides = 0).
    pub res: [i32; 6],
    /// Règle de spawn effective (MonLvl (N) du niveau du boss + TC dédiée).
    pub rule: crate::SpawnRule,
}

/// Ligne MonLvl.txt (N) d'un niveau — parse runtime lenient, cache OnceLock.
fn monlvl_row(level: u32) -> Option<(u32, u32, u32, u32, u32)> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<u32, (u32, u32, u32, u32, u32)>> = OnceLock::new();
    let map = TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        if let Ok(t) = crate::tsv::Tsv::parse(MONLVL) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let lvl = r.u("Level");
                if lvl > 0 {
                    m.insert(lvl, (r.u("AC"), r.u("TH"), r.u("HP"), r.u("DM"), r.u("XP")));
                }
            }
        }
        m
    });
    map.get(&level).copied()
}

/// Record patch_monstats d'un boss — parse runtime, cache OnceLock.
/// Renvoie `(level, hp, ac, exp, tc, res)`.
fn monstats_boss(id: &str) -> Option<(u32, u32, u32, u32, String, [i32; 6])> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<String, (u32, u32, u32, u32, String, [i32; 6])>> = OnceLock::new();
    let map = TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        if let Ok(t) = crate::tsv::Tsv::parse(PATCH_MONSTATS) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let key = r.s("Id").to_ascii_lowercase();
                if key.is_empty() {
                    continue;
                }
                let res = [
                    r.i("ResDm"),
                    r.i("ResMa"),
                    r.i("ResFi"),
                    r.i("ResLi"),
                    r.i("ResCo"),
                    r.i("ResPo"),
                ];
                m.insert(
                    key,
                    (r.u("Level"), r.u("minHP"), r.u("AC"), r.u("Exp"), r.s("TreasureClass1").to_string(), res),
                );
            }
        }
        m
    });
    map.get(&id.to_ascii_lowercase()).cloned()
}

/// Stats RÉELLES d'un boss scripté (actes II-III) : record
/// `patch_monstats.txt` + ligne `MonLvl.txt` (N) du niveau → [`crate::SpawnRule`]
/// prête pour [`crate::World::spawn_monster_rule`] (même convention que le
/// plan d2game : HP/DM/XP = % de la ligne MonLvl, AC/TH directes, TC dédiée,
/// résists de la difficulté Normal). Lookup par id monstats (`"duriel"`) ou
/// code manifest (`"DU"`), insensible à la casse. `None` : boss inconnu ou
/// table absente/inattendue.
pub fn boss_stats(id_or_code: &str) -> Option<BossStats> {
    let rec = BOSSES_ACTS23
        .iter()
        .find(|b| b.id.eq_ignore_ascii_case(id_or_code) || b.code.eq_ignore_ascii_case(id_or_code))?;
    let (level, hp, ac, exp, tc, res) = monstats_boss(rec.id)?;
    if level == 0 || hp == 0 {
        return None; // table inattendue : pas de règle fantaisiste
    }
    let (ml_ac, ml_th, ml_hp, ml_dm, ml_xp) = monlvl_row(level)?;
    let rule = crate::SpawnRule {
        kind: crate::MonKind::SuperUnique,
        hp_pct: ml_hp,
        dmg_pct: ml_dm,
        th_pct: ml_th,
        xp_pct: ml_xp,
        ac: ml_ac,
        treasure_class: if tc.is_empty() { Some(rec.tc.to_string()) } else { Some(tc.clone()) },
        res,
    };
    Some(BossStats { rec: *rec, level, hp, ac, exp, res, rule })
}

/// Andariel (a1q6, acte I — GAPS 6 fermé étape 21a) : record statique dans
/// la MÊME convention [`BossRec`]. Valeurs citées des tables v1.12 :
/// `patch_monstats.txt:158` (Id `andariel`, Code `AN`, TC1 « Andariel »),
/// repaire = Catacombes 4 ([`lvl::CATACOMBS_4`], `patch_levels.txt:39`),
/// slot porteur = a1q6 (5ᵉ slot du journal acte I, `ACT1_QUEST_IDS[5]` —
/// informatif : Andariel n'est PAS dans `BOSSES_ACTS23`, le golden de scope
/// `boss_stats("andariel") == None` reste vrai).
pub const ANDARIEL: BossRec = BossRec {
    id: "andariel",
    code: "AN",
    name: "Andariel",
    tc: "Andariel",
    lair: lvl::CATACOMBS_4,
    quest_slot: 5,
};

/// Stats RÉELLES d'Andariel (GAPS 6 fermé étape 21a) — MÊME convention que
/// [`boss_stats`] : record `patch_monstats.txt` lu RUNTIME (Level 12 N,
/// HP 2562, AC 84, Exp 916, TC « Andariel », résists Normal
/// `[0,0,-50,50,50,80]` — vides = 0) × ligne `MonLvl.txt` (N) 12 (AC 72,
/// TH 111, HP 40 %, DM 8 %, XP 140 %) → [`crate::SpawnRule`] de rang
/// SuperUnique — branchement superuniques : la règle consommable par
/// [`crate::World::spawn_monster_rule`] porte la TC dédiée, et le drop
/// passe par le même chemin que les superuniques (`World::roll_drop`
/// compose la variante retail « Andariel (H) », présente dans
/// TreasureClassEx.txt:673). `None` : table absente/inattendue. Le spawn
/// acte I LIVE reste data-driven d2game (manifest `bosses[]`) — cette API
/// lui donne la source d2rules (golden vs tables : `fidelity_gaps`).
pub fn andariel_stats() -> Option<BossStats> {
    let rec = ANDARIEL;
    let (level, hp, ac, exp, tc, res) = monstats_boss(rec.id)?;
    if level == 0 || hp == 0 {
        return None; // table inattendue : pas de règle fantaisiste
    }
    let (ml_ac, ml_th, ml_hp, ml_dm, ml_xp) = monlvl_row(level)?;
    let rule = crate::SpawnRule {
        kind: crate::MonKind::SuperUnique,
        hp_pct: ml_hp,
        dmg_pct: ml_dm,
        th_pct: ml_th,
        xp_pct: ml_xp,
        ac: ml_ac,
        treasure_class: if tc.is_empty() { Some(rec.tc.to_string()) } else { Some(tc.clone()) },
        res,
    };
    Some(BossStats { rec, level, hp, ac, exp, res, rule })
}

/// Boss scripté dont l'étape COURANTE (ou d'activation) du journal réagit à
/// `ev` — déclencheur du spawn au trigger zone (repaire de Duriel / Durance
/// 3). Renvoie `(code manifest, id monstats)`. Appelé AVANT avancement :
/// un boss déjà au-delà (récompense/fini) ne re-spawn jamais ; un exemplaire
/// vivant suffit (garde côté `World`).
pub(crate) fn scripted_boss_for_trigger(log: &Act2Quests, ev: &ActEvent) -> Option<(&'static str, &'static str)> {
    for b in BOSSES_ACTS23 {
        let run = &log.runs[b.quest_slot];
        let step = match run.phase {
            ActPhase::Inactive => 0,
            ActPhase::Active => run.step,
            _ => continue,
        };
        let def = log.def_of(b.quest_slot);
        let matches_zone = matches!(
            def.steps.get(step).map(|s| &s.trig),
            Some(Trig::Zone(id)) if *id == b.lair
        ) && matches!(ev, ActEvent::ZoneEntered { level_id } if *level_id == b.lair);
        // Re-entrée au repaire AVANT le kill (reload) : re-spawn si l'étape
        // de kill est courante — la garde « vivant » évite les doublons.
        let matches_kill_step = matches!(
            def.steps.get(step).map(|s| &s.trig),
            Some(Trig::Kill(aliases)) if aliases.contains(&b.id) || aliases.contains(&b.code)
        ) && matches!(ev, ActEvent::ZoneEntered { level_id } if *level_id == b.lair);
        if matches_zone || matches_kill_step {
            return Some((b.code, b.id));
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Branchement World (bloc impl ADDITIF — save `quest_actes2`, events d'acte)
// ---------------------------------------------------------------------------

/// Objet de quête générique (récompense `Item`) — même MVP que l'acte I.
fn quest_item_def(code: &str) -> crate::ItemDef {
    crate::ItemDef { code: code.to_string(), name: code.to_string(), kind: crate::ItemKind::Misc, amount: 0 }
}

impl World {
    /// Branche le journal spec des actes II-III (étape 8d). `None` par défaut.
    pub fn set_quest_actes2(&mut self, log: Act2Quests) {
        self.quests_actes2 = Some(log);
    }

    /// Journal spec branché (lecture).
    pub fn quest_actes2(&self) -> Option<&Act2Quests> {
        self.quests_actes2.as_ref()
    }

    /// Journal spec branché (mutation directe — inspection tests/UI).
    pub fn quest_actes2_mut(&mut self) -> Option<&mut Act2Quests> {
        self.quests_actes2.as_mut()
    }

    /// Point d'entrée unique de l'intégrateur : avance le journal A2-A3 avec
    /// un event de gameplay (MÊME flux [`ActEvent`] que l'acte I — l'appelant
    /// feede les deux journaux), applique les récompenses motrices (XP, or —
    /// aucun, skill points, STAT points, vie max, objet → inventaire/sol),
    /// émet [`GameEvent::ActQuestChanged`] par quête changée de phase et
    /// [`GameEvent::ActUnlocked`] aux déblocages d'actes III/IV, fait
    /// SPAWNER les boss scriptés au trigger zone (Duriel id 73, Mephisto
    /// id 102 — stats réelles [`boss_stats`]) et renvoie les récompenses.
    /// Sans journal branché : no-op, renvoie vide.
    pub fn quest_actes2_event(&mut self, ev: &ActEvent) -> Vec<Act2Reward> {
        // Décision de spawn scripté sur l'état AVANT avancement (le trigger
        // zone de l'étape courante — a2q6/a3q6 — fait apparaître le boss).
        let spawn = self.quests_actes2.as_ref().and_then(|log| scripted_boss_for_trigger(log, ev));
        let Some(log) = self.quests_actes2.as_mut() else {
            return Vec::new();
        };
        let before = log.phases();
        let gates_before = (log.access_act(3), log.access_act(4));
        let rewards = log.on_event(ev);
        let after = log.phases();
        for (slot, (b, a)) in before.iter().zip(after.iter()).enumerate() {
            if b != a {
                self.pending_events.push(crate::GameEvent::ActQuestChanged {
                    id: log.ids()[slot].to_string(),
                    phase: a.as_str().to_string(),
                });
            }
        }
        if !gates_before.0 && log.access_act(3) {
            self.pending_events.push(crate::GameEvent::ActUnlocked { act: 3 });
        }
        if !gates_before.1 && log.access_act(4) {
            self.pending_events.push(crate::GameEvent::ActUnlocked { act: 4 });
        }
        for r in &rewards {
            match *r {
                Act2Reward::Xp(n) => {
                    let mut leveled = Vec::new();
                    self.apply_xp(n, &mut leveled);
                    self.pending_events.extend(leveled);
                }
                Act2Reward::SkillPoint(n) => self.grant_skill_points(n),
                Act2Reward::StatPoint(n) => {
                    self.players[0].data.stats.stat_points = self.players[0].data.stats.stat_points.saturating_add(n);
                }
                Act2Reward::MaxLife(n) => {
                    // Potion of Life : +20 vie max PERMANENTE [CITÉ] — la vie
                    // courante suit (buvée au retour d'Alkor, retail).
                    self.players[0].data.stats.max_life = self.players[0].data.stats.max_life.saturating_add(n);
                    self.players[0].data.stats.life = self.players[0].data.stats.life.saturating_add(n);
                }
                Act2Reward::Item(code) => {
                    let def = quest_item_def(code);
                    if let Some(slot) = self.players[0].data.inv.iter().position(|s| s.is_none()) {
                        self.players[0].data.inv[slot] = Some(def);
                    } else {
                        let (x, y) = (self.players[0].ent.x, self.players[0].ent.y);
                        self.items.push(crate::ItemOnGround { def, x, y });
                    }
                }
                // Portés par le journal (portails logiques + gating d'acte) :
                // l'UI consomme la valeur de retour et `quest_actes2()`.
                // Le branchement warp (OpenPortal) est d2game/`warp_graph`.
                Act2Reward::OpenPortal(_) | Act2Reward::UnlockAccessAct(_) => {}
            }
        }
        // Spawn scripté APRÈS avancement (la garde « vivant » lit l'état à
        // jour ; l'échec de spawn n'annule jamais la progression de quête).
        if let Some((code, _)) = spawn {
            self.spawn_scripted_boss(code);
        }
        rewards
    }

    /// Spawn d'un boss scripté près du héros avec ses stats réelles
    /// ([`boss_stats`] → MonLvl (N) du boss). Garde anti-doublon : un
    /// exemplaire VIVANT du même code manifest suffit. `false` : boss
    /// inconnu, déjà vivant, ou aucune tuile marchable.
    pub fn spawn_scripted_boss(&mut self, code: &str) -> bool {
        // Garde : un exemplaire vivant du même code manifest suffit.
        let alive = self
            .mon_ent
            .iter()
            .zip(self.mon_dat.iter())
            .any(|(e, d)| e.anim != AnimState::DD && self.mon_tpls[d.tpl].code.eq_ignore_ascii_case(code));
        if alive {
            return false;
        }
        let Some(stats) = boss_stats(code) else {
            return false;
        };
        let rule = stats.rule;
        // 3b-1 (coop) : ancre = héros vivant le plus proche (déterministe ;
        // solo : slot 0, identique).
        let anchor = self.nearest_living_hero_slot(self.players[0].ent.x, self.players[0].ent.y);
        let (hx, hy) = (
            self.players[anchor].ent.x as i32,
            self.players[anchor].ent.y as i32,
        );
        // Recherche en spirale (rayon 0..=3) d'une tuile marchable — la
        // grille vient de la DRLG (stub carré ouvert dans les tests).
        for r in 0..=3i32 {
            for dy in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dy.abs()) != r {
                        continue; // anneau courant seulement
                    }
                                        let Some(i) = self.resolve_tpl_boss(stats.rec.code) else {
                        continue;
                    };
                    // Les tables font foi (manifest sprites sans /stats, 15a).
                    self.override_tpl_boss_stats(i, stats.level, stats.hp, stats.exp);
                    if !self.is_walkable(hx + dx, hy + dy) {
                        continue;
                    }
                    self.spawn_monster_at_rule(
                        i,
                        (hx + dx) as f32 + 0.5,
                        (hy + dy) as f32 + 0.5,
                        rule.clone(),
                    );
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Tests : machines a2q1→a2q6 / a3q1→a3q6, gates inter-quêtes, flags/d2s
// golden, save additive, boss scriptés (données réelles), intégration World.
// Les grilles des tests World sont des STUBS marchables (carré ouvert) : la
// DRLG A2-A3 des coéquipiers (contrats generate(level_id, seed)/warp_graph)
// n'est pas encore fusionnée — cf. en-tête de module.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn zone(id: u32) -> ActEvent {
        ActEvent::ZoneEntered { level_id: id }
    }
    fn kill(code: &str) -> ActEvent {
        ActEvent::MonsterKilled { code: code.into(), superunique: None }
    }
    fn kill_su(code: &str, su: &str) -> ActEvent {
        ActEvent::MonsterKilled { code: code.into(), superunique: Some(su.into()) }
    }
    fn talk(npc: &str) -> ActEvent {
        ActEvent::NpcTalked { npc: npc.into() }
    }
    fn item(code: &str) -> ActEvent {
        ActEvent::ItemPicked { code: code.into() }
    }
    fn phase(log: &Act2Quests, id: &str) -> ActPhase {
        log.run(id).unwrap().phase
    }
    fn flags(log: &Act2Quests, id: &str) -> u16 {
        log.run(id).unwrap().flags
    }
    fn rewards_of(log: &mut Act2Quests, ev: &ActEvent) -> Vec<Act2Reward> {
        log.on_event(ev)
    }

    // -- a2q1 : Radament → Atma (Book of Skill) --------------------------------

    #[test]
    fn a2q1_radament_full_machine() {
        let mut log = Act2Quests::new();
        // Hors ordre : rien.
        assert!(rewards_of(&mut log, &talk("atma")).is_empty());
        assert_eq!(phase(&log, "a2q1"), ActPhase::Inactive);
        // Radament par record de quête OU superunique OU code sprite.
        assert!(rewards_of(&mut log, &kill_su("radament", "Radament")).is_empty());
        assert_eq!(phase(&log, "a2q1"), ActPhase::RewardPending);
        assert_eq!(flags(&log, "a2q1"), FLAG_REWARD_PENDING);
        let r = rewards_of(&mut log, &talk("atma"));
        assert_eq!(r, vec![Act2Reward::SkillPoint(1), Act2Reward::Xp(300)]);
        assert_eq!(phase(&log, "a2q1"), ActPhase::Done);
        assert_eq!(flags(&log, "a2q1"), FLAG_DONE);
        // Autre PNJ : rien.
        let mut log2 = Act2Quests::new();
        log2.on_event(&kill("radament"));
        assert!(rewards_of(&mut log2, &talk("cain")).is_empty());
        assert_eq!(phase(&log2, "a2q1"), ActPhase::RewardPending);
    }

    // -- a2q2 : 3 sous-objectifs HORS ORDRE (Cube/manche/amulette) → Cain -------

    #[test]
    fn a2q2_horadric_staff_subobjectives_any_order() {
        let mut log = Act2Quests::new();
        // Ordre volontairement brouillé : vip, box, msf.
        assert!(rewards_of(&mut log, &item("vip")).is_empty());
        assert_eq!(log.run("a2q2").unwrap(), &Act2Run { phase: ActPhase::Active, step: 0, count: 1, flags: 0, subdone: 0b100 });
        assert!(rewards_of(&mut log, &item("box")).is_empty());
        // Doublon : le même objectif ne compte PAS deux fois.
        assert!(rewards_of(&mut log, &item("box")).is_empty());
        assert_eq!(log.run("a2q2").unwrap().count, 2);
        assert!(rewards_of(&mut log, &item("msf")).is_empty());
        assert_eq!(phase(&log, "a2q2"), ActPhase::RewardPending);
        // Remise Cain = transmute + Bâton en main.
        let r = rewards_of(&mut log, &talk("cain"));
        assert_eq!(r, vec![Act2Reward::Item("hst"), Act2Reward::Xp(300)]);
        assert_eq!(phase(&log, "a2q2"), ActPhase::Done);
    }

    #[test]
    fn a2q2_objects_dont_cross_contaminate_other_slots() {
        // `vip` ramassé AVANT tout : il active a2q2 mais ne touche pas a2q3
        // (qui attend l'Oasis id 43, pas un objet).
        let mut log = Act2Quests::new();
        log.on_event(&item("vip"));
        assert_eq!(phase(&log, "a2q2"), ActPhase::Active);
        assert_eq!(phase(&log, "a2q3"), ActPhase::Inactive);
    }

    // -- a2q3 : Soleil Maudit — villageois alternatifs --------------------------

    #[test]
    fn a2q3_tainted_sun_chain_and_villagers() {
        let mut log = Act2Quests::new();
        assert!(rewards_of(&mut log, &zone(lvl::CLAW_VIPER_2)).is_empty()); // hors ordre
        assert_eq!(phase(&log, "a2q3"), ActPhase::Inactive);
        log.on_event(&zone(lvl::FAR_OASIS));
        log.on_event(&zone(lvl::CLAW_VIPER_2));
        assert_eq!(log.run("a2q3").unwrap().step, 2);
        // N'importe quel villageois complète (XP seul [FIDÈLE]).
        let r = rewards_of(&mut log, &talk("greiz"));
        assert_eq!(r, vec![Act2Reward::Xp(200)]);
        assert_eq!(phase(&log, "a2q3"), ActPhase::Done);
        // Voleur de runes Elzix marche aussi (nouveau journal).
        let mut log2 = Act2Quests::new();
        for ev in [zone(lvl::FAR_OASIS), zone(lvl::CLAW_VIPER_2), talk("elzix")] {
            log2.on_event(&ev);
        }
        assert_eq!(phase(&log2, "a2q3"), ActPhase::Done);
        // Jerhyn n'est pas un « villageois » du tournis (palais).
        let mut log3 = Act2Quests::new();
        for ev in [zone(lvl::FAR_OASIS), zone(lvl::CLAW_VIPER_2), talk("jerhyn")] {
            log3.on_event(&ev);
        }
        assert_eq!(phase(&log3, "a3q5"), ActPhase::Inactive);
        assert_eq!(phase(&log3, "a2q3"), ActPhase::Active);
    }

    // -- a2q4 : Sanctuaire — palais + gates a2q5/a2q6 ---------------------------

    #[test]
    fn a2q4_arcane_opens_palace_and_gates_a2q5_a2q6() {
        let mut log = Act2Quests::new();
        assert!(!log.palace_open());
        // a2q5/a2q6 ne s'activent PAS avant a2q4 (gates requires_done).
        assert!(rewards_of(&mut log, &kill("summoner")).is_empty());
        assert!(rewards_of(&mut log, &zone(lvl::TAL_TOMB_MVP)).is_empty());
        assert_eq!(phase(&log, "a2q5"), ActPhase::Inactive);
        assert_eq!(phase(&log, "a2q6"), ActPhase::Inactive);
        // Jerhyn active a2q4 (bit 2 « donnée par PNJ ») et ouvre le palais.
        assert!(rewards_of(&mut log, &talk("jerhyn")).is_empty());
        assert_eq!(flags(&log, "a2q4"), FLAG_GIVEN_BY_NPC);
        assert!(log.palace_open());
        // Traversée implicite Harem/Basements puis Sanctuaire id 74 : complet.
        assert!(rewards_of(&mut log, &zone(lvl::PALACE_CELLAR_3)).is_empty());
        let r = rewards_of(&mut log, &zone(lvl::ARCANE_SANCTUARY));
        assert_eq!(r, vec![Act2Reward::Xp(250)]);
        assert_eq!(phase(&log, "a2q4"), ActPhase::Done);
        // Gates libérées : l'Invocateur peut mourir.
        assert!(rewards_of(&mut log, &kill("summoner")).is_empty());
        assert_eq!(phase(&log, "a2q5"), ActPhase::Active);
    }

    // -- a2q5 : Invocateur → Drognan → portail rouge Canyon ---------------------

    #[test]
    fn a2q5_summoner_reveal_and_canyon_portal() {
        let mut log = Act2Quests::new();
        log.on_event(&talk("jerhyn"));
        log.on_event(&zone(lvl::ARCANE_SANCTUARY));
        assert!(!log.canyon_portal_open());
        // Alias : superunique, record `summoner` OU code sprite `SU`.
        assert!(rewards_of(&mut log, &kill_su("SU", "The Summoner")).is_empty());
        assert_eq!(log.run("a2q5").unwrap().step, 1);
        let r = rewards_of(&mut log, &talk("drognan"));
        assert_eq!(r, vec![Act2Reward::OpenPortal(lvl::CANYON_OF_THE_MAGI), Act2Reward::Xp(400)]);
        assert_eq!(phase(&log, "a2q5"), ActPhase::Done);
        assert!(log.canyon_portal_open());
    }

    // -- a2q6 : Sept Tombeaux — gating acte III (porteuse) ----------------------

    #[test]
    fn a2q6_seven_tombs_gates_act3_via_meshif() {
        let mut log = Act2Quests::new();
        log.on_event(&talk("jerhyn"));
        log.on_event(&zone(lvl::ARCANE_SANCTUARY));
        assert!(!log.access_act(3));
        assert_eq!(log.progression_byte(), 0);
        for ev in [zone(lvl::TAL_TOMB_MVP), zone(lvl::DURIELS_LAIR), kill("duriel"), talk("tyrael")] {
            assert!(rewards_of(&mut log, &ev).is_empty(), "étapes intermédiaires sans récompense");
        }
        assert_eq!(phase(&log, "a2q6"), ActPhase::RewardPending);
        // Warriv ≠ Meshif : l'accès acte III attend le bateau.
        assert!(rewards_of(&mut log, &talk("warriv")).is_empty());
        assert!(!log.access_act(3));
        let r = rewards_of(&mut log, &talk("meshif"));
        assert_eq!(r, vec![Act2Reward::UnlockAccessAct(3), Act2Reward::Xp(1000)]);
        assert_eq!(phase(&log, "a2q6"), ActPhase::Done);
        assert!(log.access_act(3));
        assert_eq!(log.progression_byte(), 1);
        assert!(!log.access_act(4), "acte IV attend Mephisto (a3q6)");
        assert_eq!(log.progression_byte(), 1);
    }

    #[test]
    fn a2q6_any_of_seven_tombs_ids_documented_mvp_is_68() {
        // La table : les 7 tombeaux ids 66-72 (QuestFlag 13) ; le MVP fixe le
        // vrai tombeau à id 68 — l'authoring ne doit pas dériver.
        assert_eq!(lvl::TAL_TOMBS, [66, 67, 68, 69, 70, 71, 72]);
        assert_eq!(lvl::TAL_TOMB_MVP, 68);
        let mut log = Act2Quests::new();
        log.on_event(&talk("jerhyn"));
        log.on_event(&zone(lvl::ARCANE_SANCTUARY));
        // id 67 (mauvais tombeau au MVP) : n'active pas.
        assert!(rewards_of(&mut log, &zone(67)).is_empty());
        assert_eq!(phase(&log, "a2q6"), ActPhase::Inactive);
    }

    // -- a3q1 (slot 0) : Livre de Lam Esen → Alkor +5 stats ---------------------

    #[test]
    fn a3q1_lam_esen_tome_chain() {
        let mut log = Act2Quests::new();
        assert!(rewards_of(&mut log, &item("bbb")).is_empty()); // hors ordre
        assert_eq!(phase(&log, "a3q1"), ActPhase::Inactive);
        log.on_event(&zone(lvl::RUINED_TEMPLE));
        log.on_event(&item("bbb"));
        assert_eq!(phase(&log, "a3q1"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("alkor"));
        assert_eq!(r, vec![Act2Reward::StatPoint(5), Act2Reward::Xp(200)]);
        assert_eq!(phase(&log, "a3q1"), ActPhase::Done);
    }

    // -- a3q2 (slot 1) : Khalim — 4 reliques hors ordre → qf2 → orbe ------------

    #[test]
    fn a3q2_khalim_relics_any_order_then_orb() {
        let mut log = Act2Quests::new();
        // Reliques dans un ordre quelconque ; un doublon ne compte pas.
        for c in ["qf1", "qhr", "qey"] {
            assert!(rewards_of(&mut log, &item(c)).is_empty());
        }
        assert_eq!(log.run("a3q2").unwrap().count, 3);
        assert!(rewards_of(&mut log, &item("qf1")).is_empty(), "déjà comptée");
        assert_eq!(log.run("a3q2").unwrap().count, 3, "doublon ignoré");
        // 4e relique : étape suivante = transmute (toujours Active).
        assert!(rewards_of(&mut log, &item("qbr")).is_empty());
        assert_eq!(log.run("a3q2").unwrap().step, 1);
        // Transmute = pickup `qf2` ; l'orbe casse à Travincal (accès Durance).
        // MÊME event : a3q5 (jalon Travincal) s'active et se complète aussi.
        assert!(!log.durance_open());
        assert!(rewards_of(&mut log, &item("qf2")).is_empty());
        let r = rewards_of(&mut log, &zone(lvl::TRAVINCAL));
        assert_eq!(r, vec![Act2Reward::Xp(500), Act2Reward::Xp(200)]);
        assert_eq!(phase(&log, "a3q2"), ActPhase::Done);
        assert_eq!(phase(&log, "a3q5"), ActPhase::Done, "jalon Travincal sur le même event");
        assert!(log.durance_open());
    }

    // -- a3q3 (slot 2) : Gidbinn — joker kill (unique invoqué) ------------------

    #[test]
    fn a3q3_gidbinn_joker_kill_then_ormus() {
        let mut log = Act2Quests::new();
        assert!(rewards_of(&mut log, &zone(lvl::FLAYER_JUNGLE)).is_empty());
        assert_eq!(log.run("a3q3").unwrap().step, 1);
        // N'importe quel kill fait l'affaire (gardien invoqué [ARRANGÉ-MVP]).
        assert!(rewards_of(&mut log, &kill("WM")).is_empty());
        assert!(rewards_of(&mut log, &item("g33")).is_empty());
        assert_eq!(phase(&log, "a3q3"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("ormus"));
        assert_eq!(r, vec![Act2Reward::Item("rin"), Act2Reward::Xp(200)]);
        assert_eq!(phase(&log, "a3q3"), ActPhase::Done);
    }

    // -- a3q4 (slot 3) : Oiseau de Feu — chaîne figurine → vie max --------------

    #[test]
    fn a3q4_golden_bird_chain() {
        let mut log = Act2Quests::new();
        // Alias : superunique, classe `fetishshaman4` (vérifié superuniques.txt:24).
        assert!(rewards_of(&mut log, &kill_su("fetishshaman4", "Witch Doctor Endugu")).is_empty());
        assert_eq!(log.run("a3q4").unwrap().step, 1);
        log.on_event(&item("j34"));
        // Meshif (figurine → oiseau) puis Alkor (récompense).
        assert!(rewards_of(&mut log, &talk("meshif")).is_empty());
        assert_eq!(phase(&log, "a3q4"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("alkor"));
        assert_eq!(r, vec![Act2Reward::MaxLife(20), Act2Reward::Xp(300)]);
        assert_eq!(phase(&log, "a3q4"), ActPhase::Done);
    }

    // -- a3q5 (slot 4) : jalon Travincal ----------------------------------------

    #[test]
    fn a3q5_blackened_temple_milestone() {
        let mut log = Act2Quests::new();
        let r = rewards_of(&mut log, &zone(lvl::TRAVINCAL));
        assert_eq!(r, vec![Act2Reward::Xp(200)]);
        assert_eq!(phase(&log, "a3q5"), ActPhase::Done);
    }

    // -- a3q6 (slot 5) : Gardien — GATÉ sur Khalim, porteuse acte IV ------------

    #[test]
    fn a3q6_guardian_gated_on_khalim_gates_act4() {
        let mut log = Act2Quests::new();
        // Avant Khalim : la Durance n'active RIEN (gate requires_done).
        assert!(rewards_of(&mut log, &zone(lvl::DURANCE_3)).is_empty());
        assert!(rewards_of(&mut log, &kill("mephisto")).is_empty());
        assert_eq!(phase(&log, "a3q6"), ActPhase::Inactive);
        assert!(!log.access_act(4));
        // Chaîne Khalim complète.
        for c in ["qey", "qbr", "qf1", "qhr", "qf2"] {
            log.on_event(&item(c));
        }
        log.on_event(&zone(lvl::TRAVINCAL));
        assert!(log.durance_open());
        // Maintenant la Durance débloque a3q6 ; Mephisto → `mss` → acte IV.
        assert!(rewards_of(&mut log, &zone(lvl::DURANCE_3)).is_empty());
        assert_eq!(phase(&log, "a3q6"), ActPhase::Active);
        assert!(rewards_of(&mut log, &kill_su("MP", "Mephisto")).is_empty());
        let r = rewards_of(&mut log, &item("mss"));
        assert_eq!(r, vec![Act2Reward::UnlockAccessAct(4), Act2Reward::Xp(1000)]);
        assert_eq!(phase(&log, "a3q6"), ActPhase::Done);
        assert!(log.access_act(4));
        // a2q6 pas encore faite dans CE test : 1 porteuse sur 2.
        assert_eq!(log.progression_byte(), 1);
        assert!(!log.access_act(5), "acte V hors scope actes II-III");
    }

    // -- Golden : offsets .d2s des u16 des actes II-III (spec §1.3) -------------

    #[test]
    fn golden_d2s_offsets_actes_23() {
        let normal2 = [363, 365, 367, 369, 371, 373];
        let normal3 = [379, 381, 383, 385, 387, 389];
        for slot in 0..6 {
            assert_eq!(Act2Quests::d2s_flag_offset(2, slot, 0), normal2[slot]);
            assert_eq!(Act2Quests::d2s_flag_offset(3, slot, 0), normal3[slot]);
            assert_eq!(Act2Quests::d2s_flag_offset(2, slot, 1), normal2[slot] + 96);
            assert_eq!(Act2Quests::d2s_flag_offset(3, slot, 2), normal3[slot] + 192);
        }
        // Spécimens exacts de la spec : a2q1 @459 NM, a3q6 @581 Enfer,
        // Golden Bird @481 NM (wiki « A3Q1 » = clé de save a3q4, slot 3).
        assert_eq!(Act2Quests::d2s_flag_offset(2, 0, 1), 459);
        assert_eq!(Act2Quests::d2s_flag_offset(3, 5, 2), 581);
        assert_eq!(Act2Quests::d2s_flag_offset(3, 3, 1), 481);
    }

    // -- Golden : flags u16 de la chaîne complète -------------------------------

    #[test]
    fn golden_full_acts23_flags() {
        let mut log = Act2Quests::new();
        for ev in [
            item("vip"),
            item("box"),
            item("msf"),
            talk("cain"),        // a2q2 done (hst)
            kill("radament"),
            talk("atma"),        // a2q1 done
            zone(lvl::FAR_OASIS),
            zone(lvl::CLAW_VIPER_2),
            talk("atma"),        // a2q3 done
            talk("jerhyn"),      // a2q4 active (GIVEN)
            zone(lvl::ARCANE_SANCTUARY), // a2q4 done → gates
            kill("summoner"),
            talk("drognan"),     // a2q5 done (portail)
            zone(lvl::TAL_TOMB_MVP),
            zone(lvl::DURIELS_LAIR),
            kill("duriel"),
            talk("tyrael"),
            talk("meshif"),      // a2q6 done → acte III
            zone(lvl::RUINED_TEMPLE),
            item("bbb"),
            talk("alkor"),       // a3q1 done (+5 stats)
            item("qey"),
            item("qbr"),
            item("qf1"),
            item("qhr"),
            item("qf2"),
            zone(lvl::TRAVINCAL), // a3q2 done + a3q5 done (même event)
            zone(lvl::FLAYER_JUNGLE),
            kill("WM"),
            item("g33"),
            talk("ormus"),       // a3q3 done
            kill("endugu"),
            item("j34"),
            talk("meshif"),
            talk("alkor"),       // a3q4 done (+20 vie)
            zone(lvl::DURANCE_3),
            kill("mephisto"),
            item("mss"),         // a3q6 done → acte IV
        ] {
            log.on_event(&ev);
        }
        assert!(
            log.phases().iter().all(|p| *p == ActPhase::Done),
            "les 12 quêtes sont Done : {:?}",
            log.phases()
        );
        // Seule a2q4 (activation Jerhyn) porte le bit 2 « donnée par PNJ ».
        assert_eq!(
            log.quest_flags(),
            [0x0001, 0x0001, 0x0001, 0x0005, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001]
        );
        assert!(log.access_act(3) && log.access_act(4));
        assert_eq!(log.progression_byte(), 2);
        // Un event isolé ne franchit jamais deux étapes ni ne force une gate.
        let mut fresh = Act2Quests::new();
        fresh.on_event(&zone(lvl::DURIELS_LAIR));
        assert_eq!(phase(&fresh, "a2q6"), ActPhase::Inactive);
        fresh.on_event(&kill("mephisto"));
        assert_eq!(phase(&fresh, "a3q6"), ActPhase::Inactive);
    }

    // -- Golden : JSON de save (état complet + round trip + partiel) ------------

    #[test]
    fn golden_json_save_full_run() {
        let mut log = Act2Quests::new();
        for ev in [
            item("vip"),
            item("box"),
            item("msf"),
            talk("cain"),
            kill("radament"),
            talk("atma"),
            zone(lvl::FAR_OASIS),
            zone(lvl::CLAW_VIPER_2),
            talk("atma"),
            talk("jerhyn"),
            zone(lvl::ARCANE_SANCTUARY),
            kill("summoner"),
            talk("drognan"),
            zone(lvl::TAL_TOMB_MVP),
            zone(lvl::DURIELS_LAIR),
            kill("duriel"),
            talk("tyrael"),
            talk("meshif"),
            zone(lvl::RUINED_TEMPLE),
            item("bbb"),
            talk("alkor"),
            item("qey"),
            item("qbr"),
            item("qf1"),
            item("qhr"),
            item("qf2"),
            zone(lvl::TRAVINCAL),
            zone(lvl::FLAYER_JUNGLE),
            kill("WM"),
            item("g33"),
            talk("ormus"),
            kill("endugu"),
            item("j34"),
            talk("meshif"),
            talk("alkor"),
            zone(lvl::DURANCE_3),
            kill("mephisto"),
            item("mss"),
        ] {
            log.on_event(&ev);
        }
        let v: serde_json::Value = serde_json::from_str(&log.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "version": 1,
                "quests": [
                    {"id": "a2q1", "act": 2, "phase": "done", "step": 0, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a2q2", "act": 2, "phase": "done", "step": 0, "count": 3, "flags": 1, "subdone": 0},
                    {"id": "a2q3", "act": 2, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a2q4", "act": 2, "phase": "done", "step": 1, "count": 1, "flags": 5, "subdone": 0},
                    {"id": "a2q5", "act": 2, "phase": "done", "step": 1, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a2q6", "act": 2, "phase": "done", "step": 3, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a3q1", "act": 3, "phase": "done", "step": 1, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a3q2", "act": 3, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a3q3", "act": 3, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a3q4", "act": 3, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a3q5", "act": 3, "phase": "done", "step": 0, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a3q6", "act": 3, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                ]
            })
        );
        let log2 = Act2Quests::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log.to_json(), log2.to_json());
    }

    #[test]
    fn json_round_trip_partial_states_with_subdone() {
        let mut log = Act2Quests::new();
        // a2q2 : deux sous-objectifs sur trois.
        log.on_event(&item("msf"));
        log.on_event(&item("box"));
        // a3q2 : trois reliques sur quatre.
        log.on_event(&item("qhr"));
        log.on_event(&item("qey"));
        log.on_event(&item("qf1"));
        // a2q1 : Radament en attente d'Atma.
        log.on_event(&kill("radament"));
        let log2 = Act2Quests::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log2.run("a2q2").unwrap().subdone, 0b011);
        assert_eq!(log2.run("a3q2").unwrap().subdone, 0b1101);
        // Le rechargé poursuit exactement.
        let mut log = log2;
        log.on_event(&item("vip"));
        assert_eq!(phase(&log, "a2q2"), ActPhase::RewardPending);
        log.on_event(&item("qbr"));
        assert_eq!(log.run("a3q2").unwrap().step, 1, "4e relique : étape transmute");
        log.on_event(&talk("atma"));
        assert_eq!(phase(&log, "a2q1"), ActPhase::Done);
    }

    #[test]
    fn json_rejects_corrupted_states() {
        let base = Act2Quests::new();
        // Version inconnue / JSON invalide / mauvaise longueur.
        assert!(Act2Quests::from_json(r#"{"version":9,"quests":[]}"#).is_err());
        assert!(Act2Quests::from_json("{oops").is_err());
        assert!(Act2Quests::from_json(r#"{"version":1,"quests":[]}"#).is_err());
        // Id inconnu (slot 3 = a2q4, PAS a3q4 — ordre de sauvegarde).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][3]["id"] = "a3q4".into();
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // Acte incohérent avec le slot.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][7]["act"] = serde_json::json!(2);
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // Phase incohérente : Done sans bit 0.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][5] =
            serde_json::json!({"id":"a2q6","act":2,"phase":"done","step":3,"count":1,"flags":0,"subdone":0});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // RewardPending sans PNJ de remise (a2q4 n'en a pas).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][3] =
            serde_json::json!({"id":"a2q4","act":2,"phase":"reward_pending","step":1,"count":1,"flags":2,"subdone":0});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // subdone fantaisiste : bits hors bornes (a2q2 a 3 alternatives).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1] =
            serde_json::json!({"id":"a2q2","act":2,"phase":"active","step":0,"count":2,"flags":0,"subdone":0b1011});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // subdone ≠ popcount(count) (2 bits posés, count=1).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1] =
            serde_json::json!({"id":"a2q2","act":2,"phase":"active","step":0,"count":1,"flags":0,"subdone":0b011});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // Gate bafouée : a2q5 Active alors que a2q4 est Inactive.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][4] =
            serde_json::json!({"id":"a2q5","act":2,"phase":"active","step":0,"count":0,"flags":0,"subdone":0});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // idem a3q6 sans Khalim.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][11] =
            serde_json::json!({"id":"a3q6","act":3,"phase":"active","step":0,"count":0,"flags":0,"subdone":0});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // Inactive avec résidus.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][0] =
            serde_json::json!({"id":"a2q1","act":2,"phase":"inactive","step":1,"count":1,"flags":2,"subdone":0});
        assert!(Act2Quests::from_json(&v.to_string()).is_err());
        // Contrôle positif : la base propre passe.
        assert!(Act2Quests::from_json(&base.to_json()).is_ok());
    }

    // -- Authoring : invariants de slots (ordre de sauvegarde !) ----------------

    #[test]
    fn authoring_slot_order_is_save_order_not_journal_order() {
        assert_eq!(ACT2_QUEST_IDS, ["a2q1", "a2q2", "a2q3", "a2q4", "a2q5", "a2q6"]);
        // ACTE III : a3q1 = Lam Esen (wiki « A3Q4 »), a3q4 = Oiseau de Feu
        // (wiki « A3Q1 ») — mapping qstsa3q* string.tbl (spec §1.3).
        assert_eq!(ACT3_QUEST_IDS, ["a3q1", "a3q2", "a3q3", "a3q4", "a3q5", "a3q6"]);
        assert_eq!(DEFS_ACT3[0].name, "Lam Esen's Tome");
        assert_eq!(DEFS_ACT3[3].name, "The Golden Bird");
        assert_eq!(DEFS_ACT3[5].name, "The Guardian");
        let log = Act2Quests::new();
        assert_eq!(log.run("a3q1").map(|r| r.phase), Some(ActPhase::Inactive));
        // Gates : a2q5 + a2q6 attendent a2q4 (slot 3), a3q6 attend Khalim (slot 7).
        assert_eq!(DEFS_ACT2[4].requires_done, Some(3));
        assert_eq!(DEFS_ACT2[5].requires_done, Some(3));
        assert_eq!(DEFS_ACT3[5].requires_done, Some(slot::A3Q2));
        // Une seule porteuse par acte (UnlockAccessAct).
        assert_eq!(
            DEFS_ACT2.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, Act2Reward::UnlockAccessAct(3))).count(),
            1
        );
        assert_eq!(
            DEFS_ACT3.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, Act2Reward::UnlockAccessAct(4))).count(),
            1
        );
        // Seule a2q4 a le talk d'activation Jerhyn (palais, spec §1.4).
        assert!(!log.palace_open());
        // Ids `patch_levels.txt` documentés (re-vérifiés 2026-10-24) —
        // ancrage des niveaux cités dans les libellés d'étapes.
        assert_eq!((lvl::A2_SEWERS_1, lvl::HALLS_OF_DEAD_3, lvl::MAGGOT_LAIR_3), (47, 60, 64));
        assert_eq!(
            (lvl::FLAYER_DUNGEON_1, lvl::FLAYER_DUNGEON_2, lvl::A3_SEWERS_2, lvl::SPIDER_CAVE),
            (88, 89, 93, 84)
        );
        assert_eq!((lvl::DURANCE_3, lvl::DURIELS_LAIR, lvl::ARCANE_SANCTUARY), (102, 73, 74));
    }

    // -- BOSS : données réelles des tables (patch_monstats + MonLvl) ------------

    #[test]
    fn boss_stats_golden_real_tables() {
        // Duriel — patch_monstats.txt:213 (Level 22 N, HP 4757, AC 85,
        // Exp 1655, TC « Duriel ») × MonLvl.txt ligne 22 N (AC 132, TH 221,
        // HP 84 %, DM 14 %, XP 363 %).
        let d = boss_stats("duriel").expect("duriel présent des tables");
        assert_eq!(d.rec.code, "DU");
        assert_eq!(d.level, 22);
        assert_eq!(d.hp, 4757);
        assert_eq!(d.ac, 85);
        assert_eq!(d.exp, 1655);
        assert_eq!(d.res, [0, 0, 20, 20, 50, 20], "résists réels Duriel N");
        assert_eq!(d.rule.kind, crate::MonKind::SuperUnique);
        assert_eq!(d.rule.hp_pct, 84);
        assert_eq!(d.rule.dmg_pct, 14);
        assert_eq!(d.rule.th_pct, 221);
        assert_eq!(d.rule.xp_pct, 363);
        assert_eq!(d.rule.ac, 132);
        assert_eq!(d.rule.treasure_class.as_deref(), Some("Duriel"));
        assert_eq!(d.rule.res, [0, 0, 20, 20, 50, 20]);
        // Mephisto — patch_monstats.txt:244 (Level 26 N, HP 5695, AC 124,
        // Exp 2131, TC « Mephisto ») × MonLvl ligne 26 N (AC 156, TH 265,
        // HP 106 %, DM 17 %, XP 503 %).
        let m = boss_stats("MP").expect("mephisto par code sprite");
        assert_eq!(m.rec.id, "mephisto");
        assert_eq!(m.level, 26);
        assert_eq!(m.hp, 5695);
        assert_eq!(m.exp, 2131);
        assert_eq!(m.res, [0, 0, 33, 33, 25, 50], "résists réels Mephisto N");
        assert_eq!(m.rule.hp_pct, 106);
        assert_eq!(m.rule.th_pct, 265);
        assert_eq!(m.rule.ac, 156);
        assert_eq!(m.rule.treasure_class.as_deref(), Some("Mephisto"));
        // Lookup insensible à la casse + inconnu → None.
        assert_eq!(boss_stats("DURIEL").unwrap().level, 22);
        assert!(boss_stats("andariel").is_none(), "hors scope actes II-III");
        assert!(boss_stats("zz").is_none());
        // Records statiques : lairs et slots porteurs.
        assert_eq!(BOSSES_ACTS23[0].lair, lvl::DURIELS_LAIR);
        assert_eq!(BOSSES_ACTS23[0].quest_slot, 5);
        assert_eq!(BOSSES_ACTS23[1].lair, lvl::DURANCE_3);
        assert_eq!(BOSSES_ACTS23[1].quest_slot, 11);
    }

    #[test]
    fn scripted_boss_trigger_matches_zone_steps_only() {
        // a2q6 Active à l'étape repaire : Zone(73) déclenche, Zone(74) non.
        let mut log = Act2Quests::new();
        log.on_event(&talk("jerhyn"));
        log.on_event(&zone(lvl::ARCANE_SANCTUARY));
        log.on_event(&zone(lvl::TAL_TOMB_MVP));
        assert!(scripted_boss_for_trigger(&log, &zone(lvl::DURIELS_LAIR)).is_some());
        assert!(scripted_boss_for_trigger(&log, &zone(lvl::ARCANE_SANCTUARY)).is_none());
        // Après le kill (étape suivante) : plus de re-spawn au trigger zone.
        log.on_event(&zone(lvl::DURIELS_LAIR));
        log.on_event(&kill("duriel"));
        assert!(scripted_boss_for_trigger(&log, &zone(lvl::DURIELS_LAIR)).is_none());
        // a3q6 : le trigger zone EST l'étape d'activation (id 102).
        let log2 = Act2Quests::new();
        assert!(scripted_boss_for_trigger(&log2, &zone(lvl::DURANCE_3)).is_some());
        assert!(scripted_boss_for_trigger(&log2, &zone(lvl::DURIELS_LAIR)).is_none());
    }

    // -- Intégration World : spawn scripté + récompenses + events ---------------

    use crate::{GameEvent, MonKind, World};

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    /// Manifest réel + boss DU/MP (valeurs réelles patch_monstats) — STUBS de
    /// grille : DRLG A2-A3 pas encore fusionnée (cf. en-tête de module).
    fn manifest_bosses() -> serde_json::Value {
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([
            {"code": "DU", "name": "Duriel", "stats": {
                "level": 22, "hpMin": 4757, "hp": 4757,
                "dmgMin": 140, "dmgMax": 180, "exp": 1655,
                "speed": 4.0, "treasureClass": "Duriel"}},
            {"code": "MP", "name": "Mephisto", "stats": {
                "level": 26, "hpMin": 5695, "hp": 5695,
                "dmgMin": 295, "dmgMax": 442, "exp": 2131,
                "speed": 4.0, "treasureClass": "Mephisto"}},
        ]);
        m
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Amène a2q6 à l'étape « repaire de Duriel » (après les gates a2q4).
    fn reach_duriel_lair(w: &mut World) {
        for ev in [
            talk("jerhyn"),
            zone(lvl::ARCANE_SANCTUARY),
            zone(lvl::TAL_TOMB_MVP),
        ] {
            w.quest_actes2_event(&ev);
        }
    }

    #[test]
    fn world_spawns_scripted_duriel_at_lair_trigger_with_real_stats() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses(), 0xD2_D2);
        w.set_quest_actes2(Act2Quests::new());
        assert!(w.monsters().is_empty(), "manifest sans monstres de planche");
        reach_duriel_lair(&mut w);
        // L'entrée du repaire (id 73) fait apparaître Duriel — UNE fois.
        let rewards = w.quest_actes2_event(&zone(lvl::DURIELS_LAIR));
        assert!(rewards.is_empty());
        assert_eq!(w.monsters().len(), 1, "Duriel spawné au trigger zone");
        assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
        // HP effectifs = 4757 × 84 % (ligne MonLvl 22 N) = 3995.
        assert_eq!(w.monster_hp(0), Some((3995, 3995)));
        // TC/résists réels visibles dans la save (MonSave.tc/res).
        let save = w.save();
        assert!(save.contains("Duriel"), "TC dédiée « Duriel » persistée");
        // Re-trigger : pas de doublon (garde vivant).
        w.quest_actes2_event(&zone(lvl::DURIELS_LAIR));
        assert_eq!(w.monsters().len(), 1);
        // Kill → étape Tyrael (encore Active) ; Tyrael → PRÊT ; Meshif →
        // Done + acte III.
        let a2q6 = |w: &World| {
            let r = w.quest_actes2().unwrap().run("a2q6").unwrap();
            (r.phase, r.step)
        };
        let r = w.quest_actes2_event(&kill("duriel"));
        assert!(r.is_empty());
        assert_eq!(a2q6(&w), (ActPhase::Active, 3), "Tyrael attend son dialogue");
        w.quest_actes2_event(&talk("tyrael"));
        assert_eq!(a2q6(&w).0, ActPhase::RewardPending);
        let r = w.quest_actes2_event(&talk("meshif"));
        assert_eq!(r, vec![Act2Reward::UnlockAccessAct(3), Act2Reward::Xp(1000)]);
        assert_eq!(a2q6(&w).0, ActPhase::Done);
        assert!(w.quest_actes2().unwrap().access_act(3));
    }

    #[test]
    fn world_events_act_unlock_and_quest_changed_are_emitted() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses(), 0xD2_D2);
        w.set_quest_actes2(Act2Quests::new());
        reach_duriel_lair(&mut w);
        w.quest_actes2_event(&zone(lvl::DURIELS_LAIR));
        w.quest_actes2_event(&kill("duriel"));
        w.quest_actes2_event(&talk("tyrael"));
        w.quest_actes2_event(&talk("meshif"));
        // Les events sont flushés au PROCHAIN tick (contrat d2rules).
        let ev = w.tick(10, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 3 })),
            "ActUnlocked(3) émis au déblocage du bateau de Meshif"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::ActQuestChanged { id, .. } if id == "a2q6")),
            "ActQuestChanged a2q6 émis"
        );
    }

    #[test]
    fn world_spawns_scripted_mephisto_and_applies_rewards() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses(), 0xD2_D2);
        w.set_quest_actes2(Act2Quests::new());
        // Khalim d'abord (gate a3q6) : 4 reliques + transmute + orbe.
        for c in ["qey", "qbr", "qf1", "qhr", "qf2"] {
            w.quest_actes2_event(&item(c));
        }
        w.quest_actes2_event(&zone(lvl::TRAVINCAL));
        assert!(w.quest_actes2().unwrap().durance_open());
        // Entrée Durance 3 : Mephisto apparaît (activation + spawn même event).
        w.quest_actes2_event(&zone(lvl::DURANCE_3));
        assert_eq!(w.monsters().len(), 1, "Mephisto spawné en Durance 3");
        // HP effectifs = 5695 × 106 % (ligne MonLvl 26 N) = 6036.
        assert_eq!(w.monster_hp(0), Some((6036, 6036)));
        assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
        // Kill + pierre d'âme : récompenses appliquées (XP + accès acte IV).
        w.quest_actes2_event(&kill("mephisto"));
        let r = w.quest_actes2_event(&item("mss"));
        assert_eq!(r, vec![Act2Reward::UnlockAccessAct(4), Act2Reward::Xp(1000)]);
        assert!(w.quest_actes2().unwrap().access_act(4));
    }

    #[test]
    fn world_applies_skill_stat_and_maxlife_rewards() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses(), 0xD2_D2);
        w.set_quest_actes2(Act2Quests::new());
        // a2q1 Radament → Atma : +1 point de skill (Book of Skill).
        w.quest_actes2_event(&kill("radament"));
        w.quest_actes2_event(&talk("atma"));
        assert_eq!(w.unspent_skill_points(), 1);
        // a3q1 Lam Esen → Alkor : +5 points de caractéristique.
        w.quest_actes2_event(&zone(lvl::RUINED_TEMPLE));
        w.quest_actes2_event(&item("bbb"));
        w.quest_actes2_event(&talk("alkor"));
        assert_eq!(w.hero_stats().stat_points, 5);
        // a3q4 Oiseau de Feu → Alkor : +20 vie max permanente (les bonus XP
        // des quêtes précédentes peuvent avoir fait monter le niveau — on
        // capture la vie max juste avant la récompense).
        let life_before = w.hero_stats().max_life;
        w.quest_actes2_event(&kill("endugu"));
        w.quest_actes2_event(&item("j34"));
        w.quest_actes2_event(&talk("meshif"));
        w.quest_actes2_event(&talk("alkor"));
        assert_eq!(w.hero_stats().max_life, life_before + 20);
        assert_eq!(w.hero_stats().life, life_before + 20, "la vie courante suit la Potion of Life");
        // a2q2 → Cain : le Bâton `hst` entre dans l'inventaire.
        for ev in [item("box"), item("msf"), item("vip"), talk("cain")] {
            w.quest_actes2_event(&ev);
        }
        assert!(w.inventory().iter().any(|s| s.as_ref().map_or(false, |d| d.code == "hst")));
    }

    #[test]
    fn world_save_roundtrip_keeps_acts23_journal() {
        // Manifest SANS boss spawnables : le journal avance sans spawn — la
        // save ne porte que le journal. (Un monde CONTENANT un boss spawné
        // paresseusement ne se recharge pas : gabarit absent du manifest ⇒
        // Err contractuel — validation tpl de `load_json`.)
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2_D2);
        w.set_quest_actes2(Act2Quests::new());
        reach_duriel_lair(&mut w);
        w.quest_actes2_event(&zone(lvl::DURIELS_LAIR)); // progresse, sans spawn
        w.quest_actes2_event(&kill("duriel"));
        // Après le kill, l'étape Tyrael (3) attend son dialogue : on la
        // franchit pour sauvegarder en RewardPending (remise Meshif en attente).
        w.quest_actes2_event(&talk("tyrael"));
        let json = w.save();
        // Rétro-compat : champ quest_actes2 ABSENT → charge (journal vide).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("quest_actes2").is_some(), "le champ additif est présent dans la save fraîche");
        v.as_object_mut().unwrap().remove("quest_actes2");
        let w2 = World::load(&v.to_string()).unwrap();
        // Sans champ : PAS de journal branché (rétro-compat, jamais d'état
        // à moitié reconstitué).
        assert!(w2.quest_actes2().is_none());
        // Round trip exact : re-save identique.
        let w3 = World::load(&json).unwrap();
        let log2 = w3.quest_actes2().unwrap();
        assert_eq!(log2.run("a2q6").unwrap().phase, ActPhase::RewardPending);
        assert_eq!(log2.run("a2q6").unwrap().step, 3, "dernière étape = Tyrael franchi");
        assert_eq!(log2.run("a2q6").unwrap().flags, FLAG_REWARD_PENDING);
        assert_eq!(log2.to_json(), w.quest_actes2().unwrap().to_json());
        // Journal PRÉSENT mais invalide ⇒ Err (contrat strict).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["quest_actes2"] = serde_json::json!({"version": 99, "quests": []});
        assert!(World::load(&v.to_string()).is_err());
        // Sans journal branché : no-op.
        let mut w4 = World::with_seed(open_grid(8, 8), manifest_bosses(), 7);
        assert!(w4.quest_actes2_event(&kill("duriel")).is_empty());
    }

    #[test]
    fn world_no_monster_tpl_is_a_clean_no_spawn() {
        // Manifest SANS bloc boss DU : le trigger zone avance quand même la
        // quête (le spawn échoue proprement, jamais de panique).
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2_D2);
        w.set_quest_actes2(Act2Quests::new());
        reach_duriel_lair(&mut w);
        w.quest_actes2_event(&zone(lvl::DURIELS_LAIR));
        assert!(w.monsters().is_empty(), "pas de gabarit DU : pas de spawn");
        assert_eq!(w.quest_actes2().unwrap().run("a2q6").unwrap().step, 2, "la progression de quête n'est pas bloquée");
    }
}
