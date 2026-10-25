//! Moteur de quêtes ACTES IV-V + FIN DE JEU — implémentation de la spec
//! `d2w-re/specs/quests-actes.md` §5/§6 (campagne diablo-2-wasm, étape 10c).
//!
//! Même pattern que [`crate::quests_actes`] (acte I) et [`crate::quests_actes2`]
//! (actes II-III) : events d'entrée [`ActEvent`] RÉUTILISÉS (pas de nouveau
//! type — le gameplay émet le même flux pour les trois journaux), machine à
//! phases [`ActPhase`], flags u16 façon .d2s, save JSON versionnée strict.
//! 100 % pur ; le bloc `impl World` additif en bas fournit le branchement +
//! la save (`quest_actes45`, champ `#[serde(default)]` — save v3 inchangée).
//!
//! # Ordre des slots = ordre de SAUVEGARDE .d2s (⚠ acte IV ≠ ordre du journal)
//! Spec §1.3/§5 : les clés `qstsa4q*` du string.tbl mappent le slot de
//! sauvegarde — slot 1 = **Izual**, slot 2 = **Diablo** (« Epilogue de la
//! terreur »), slot 3 = la **Forge de l'Enfer** (string.tsv:937-939) [VÉRIFIÉ-
//! LOCAL]. Ici les ids SONT les clés de sauvegarde :
//! * Acte IV (u16 @395 + 2·slot + 96·diff) : `a4q1` L'Ange déchu (Izual) →
//!   `a4q2` Epilogue de la terreur (Diablo, wiki « A4Q3 ») → `a4q3` La Forge
//!   de l'Enfer (wiki « A4Q2 »).
//! * Acte V (u16 @415 + 2·slot + 96·diff) : `a5q1` Siège d'Harrogath →
//!   `a5q2` Sauvetage au Mont Arreat → `a5q3` Prison de glace → `a5q4` Le
//!   traître d'Harrogath → `a5q5` Le Rite de Passage → `a5q6` Le Crépuscule
//!   de la Destruction (ordre wiki = ordre save).
//! Table golden [`Act45Quests::d2s_flag_offset`] verrouillée par tests.
//!
//! # Extensions du pattern actes II-III (justifiées par la spec §5/§6)
//! * [`FLAG_RES_SCROLL`] — bit 7 d'a5q3 « parchemin de résistance consommé »
//!   [CITÉ trevin, spec §6], posé à la remise de Malah (même esprit que le
//!   bit 4 du Repaire du Mal, acte I).
//! * [`Act45Reward`] — variantes A4/A5 : `LarzukSocket` (a5q1 serti [CITÉ]),
//!   `ResistAll` (a5q3 +10 % tout [CITÉ trevin ; valeur CONFIRMÉE par le
//!   README de l'oracle d2s-file-generator, étape 19b]), `AnyaPersonalize`
//!   (a5q3/a5q4 [FIDÈLE]), `BarbarianMercs` (a5q2 [CITÉ]).
//! * Gate retail « Rite de Passage » (étape 19b — README oracle d2s : niveau
//!   minimal **pour faire** la quête des Anciens = 20/40/60 selon la
//!   difficulté) : [`Act45Quests::ancients_min_level`] + contexte RUNTIME
//!   posé par [`World::quest_actes45_event`] (jamais sérialisé) — l'autel
//!   des Cieux n'active pas, ni ne spawn les Anciens, sous ce niveau.
//! * Fin de jeu : a5q6 complète → event `GameEvent::GameWon` (additif,
//!   spec §6 « fin de jeu ») + [`Act45Quests::game_won`].
//! * Gating inter-actes : accès acte V = a4q2 bit 0 (portail de Tyrael) ;
//!   portail d'Anya = entre a5q3 et a5q4 [CITÉ] ; Chambre de la Pierre
//!   d'Worldstone = a5q5 [FIDÈLE].
//!
//! # Répartition des boss (mission 10c — coéquipier « Boss », quests_finale.rs)
//! Ce module NE SPAWNE PAS Diablo ni Baal (ni les 5 vagues du Trône) : la
//! chaîne Diablo/Baal/vagues appartient au crate voisin `quests_finale.rs`
//! — les machines ici CONSOMMENT ses kills via le flux [`ActEvent`] partagé
//! (`MonsterKilled{code:"diablo"|"baalcrab"}`). Les superuniques data-driven
//! (Siege Boss superuniques.txt:45, Frozenstein :62, Nihlathak Boss :63,
//! gardiens de sceaux :38-40) restent portés par le chemin d2game
//! (`plan_level_spawns_diff`) — seuls les SPAWNS DE QUÊTE non superuniques
//! sont scriptés ici, pattern duriel/mephisto d'étape 8d :
//! **Izual** (spawn de quête, PAS un superunique [VÉRIFIÉ-LOCAL spec §5]),
//! **Hephasto** (classe de quête `hephasto`, patch_monstats.txt:411) et les
//! **3 Anciens** (activés à l'Autel des Cieux, spec §6 — superuniques :46-48
//! mais spawn scripté par la quête, colonne TC vide = pas de drop dédié).
//!
//! # Triggers réels (spec §5/§6, ids re-vérifiés `patch_levels.txt` 2026-10-24)
//! * a4q1 : Plaines du Désespoir = id 105 « Mesa 2 », Izual (`izual`,
//!   patch_monstats.txt:258) → Tyrael (+2 skills [CITÉ]).
//! * a4q2 : Chaos Sanctuaire = id 108 ; sceaux = 3 gardiens superuniques
//!   Infector of Souls/Lord De Seis/Grand Vizier of Chaos (5 sceaux retail,
//!   3 packs — [ARRANGÉ-MVP]) → Diablo (`diablo`, monstats.txt:245) →
//!   Tyrael → accès acte V + SetProgressionByte.
//! * a4q3 : pierre d'âme `mss` → Rivière de feu = id 107 « Lava 1 » →
//!   Hephasto → Marteau de la Forge infernale `hfh` (`patch_weapons.txt`,
//!   [VÉRIFIÉ-LOCAL]) → Cain. Gemmes [MVP : 3 parfaites `gp*`].
//! * a5q1 : Contreforts sanglants = id 110 « Siege 1 » → Siege Boss/Shenk
//!   (superuniques.txt:45, class `overseer1`) → Larzuk serti.
//! * a5q2 : 4 extérieurs ids 110/111/112/117 (15 cages retail [ARRANGÉ-MVP :
//!   1 zone = 1 vague de sauvetage]) → Qual-Kehk → Ral+Ort+Tal (`r08/r09/r07`,
//!   patch_misc.txt [VÉRIFIÉ-LOCAL]) + mercenaires barbares.
//! * a5q3 : Caverne de glace = id 119 « Ice Cave 3A » → Frozenstein → Anya
//!   (potion de Malah) → Malah → `tr2` +10 résistances [CITÉ] + objet
//!   personnalisé.
//! * a5q4 : portail d'Anya → Repaire de Vaught = id 124 « Temple Boss » →
//!   Nihlathak (superuniques.txt:63) → Anya (personnalisation) [FIDÈLE].
//! * a5q5 : Sommet d'Arreat = id 120 « Mountain Top » → Autel des Cieux
//!   [ARRANGÉ-MVP : l'entrée vaut activation, pattern autel Gidbinn] → 3
//!   Anciens (`ancientbarb1/2/3`, superuniques.txt:46-48) → accès Worldstone.
//! * a5q6 : Trône de la Destruction = id 131 (QuestFlag 39) → vagues Baal
//!   (quests_finale.rs) → Baal (`baalcrab`, patch_monstats.txt:547) en
//!   Chambre de la Pierre d'Worldstone = id 132 → Tyrael → FIN DE JEU.
//!
//! # Récompenses
//! Récompenses NPC [CITÉ Arreat Summit via spec §5/§6] portées par le
//! journal ; bonus XP **[INVENTÉ]** (plafonds de notre cru — spec §7).
//! Étape 19b (interop .d2s) : les valeurs chiffrées sont réconciliées avec
//! le README de l'oracle d2s-file-generator (valeurs de format = faits) —
//! +4 points de skill PAR difficulté = 1 (Den of Evil A1Q1) + 1 (Radament
//! A2Q1) + 2 (Izual A4Q1), +5 stats (Lam Esen), +20 vie (Oiseau de Feu),
//! +10 % résists tout (Prison de glace, appliqué au héros via
//! `HeroStats::res_all_pct`) ; or du stash interop = 2 500 000
//! (economy.rs, confirmé par le vrai .d2s « goldbank » d2d2s).

use serde::{Deserialize, Serialize};

use crate::quests_actes::{ActEvent, ActPhase};
use crate::quests_actes2::{BossRec, BossStats};
use crate::world::World;
use crate::AnimState;

// Ré-export des bits u16 .d2s (sémantique identique actes I-III, spec §1.3).
pub use crate::quests_actes::{FLAG_DONE, FLAG_GIVEN_BY_NPC, FLAG_REWARD_PENDING};

/// bit 7 — a5q3 : parchemin de résistance consommé [CITÉ trevin, spec §6].
/// Posé quand la quête passe Done (remise de Malah) ; jamais sur les autres
/// slots (validation `from_json`).
pub const FLAG_RES_SCROLL: u16 = 0x0080;

// ---------------------------------------------------------------------------
// Récompenses (spec §1.2 — events de sortie A4/A5)
// ---------------------------------------------------------------------------

/// Récompense émise par le journal A4-A5 (appliquée par
/// [`World::quest_actes45_event`] sauf les variantes journal/UI : serti,
/// personnalisation, résistances, mercenaires — consommées via la valeur de
/// retour, même contrat que `CharsiImbue` de l'acte I).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Act45Reward {
    /// XP de quête ([INVENTÉ] — bonus retail duré dans le DLL).
    Xp(u32),
    /// Point(s) de compétence (a4q1 Tyrael +2 [CITÉ]).
    SkillPoint(u32),
    /// Objet par code table (a4q3 gemmes `gp*`, a5q2 runes `r07-r09`).
    Item(&'static str),
    /// Larzuk sertit 1 objet (a5q1 [CITÉ] ; MVP +1 socket [ARRANGÉ-MVP]) —
    /// porté par le journal, consommé via la valeur de retour.
    LarzukSocket,
    /// +n % à TOUTES les résistances, PERMANENT (a5q3 parchemin `tr2` [CITÉ
    /// trevin ; +10 CONFIRMÉ par le README oracle d2s, étape 19b]) —
    /// appliqué au héros par [`World::quest_actes45_event`] (champ save
    /// additif `HeroStats::res_all_pct`, +10 par difficulté, max +30).
    ResistAll(u32),
    /// Anya personnalise 1 objet gratuitement (a5q3/a5q4 [FIDÈLE]) — porté
    /// par le journal, consommé via la valeur de retour.
    AnyaPersonalize,
    /// Accès aux mercenaires barbares de Qual-Kehk (a5q2 [CITÉ]) — porté par
    /// le journal, consommé via la valeur de retour.
    BarbarianMercs,
    /// Accès à l'acte suivant (a4q2 portail de Tyrael → acte V [FIDÈLE]) +
    /// progression — porté par [`Act45Quests::access_act`].
    UnlockAccessAct(u8),
}

// ---------------------------------------------------------------------------
// Triggers / authoring
// ---------------------------------------------------------------------------

/// Trigger d'une étape (authoring statique — même esprit que les actes I-III,
/// [`Trig::Any`] pour les sous-objectifs hors ordre : sceaux, cages, Anciens).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trig {
    /// Dialogue NPC (code quête bas de casse).
    Talk(&'static str),
    /// Entrée dans un niveau (id `patch_levels.txt`).
    Zone(u32),
    /// Mort d'un monstre — n'importe quel alias : code monstats (base ou
    /// record de quête) OU nom de superunique.
    Kill(&'static [&'static str]),
    /// Ramassage d'un objet par code.
    Item(&'static str),
    /// Sous-objectifs HORS ORDRE : chaque alternative doit se produire une
    /// fois (masque `subdone` de [`Act45Run`]) pour franchir l'étape.
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
    /// Libellé FR (string.tsv/expansionstring.tsv via spec §5/§6, condensé).
    pub desc: &'static str,
    /// Ce qui fait avancer l'étape.
    pub trig: Trig,
    /// Occurrences requises (occurrences d'alternatives pour [`Trig::Any`]).
    pub count: u32,
}

/// Définition d'une quête des actes IV-V.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefAct45 {
    /// Identifiant stable = CLÉ DE SAUVEGARDE (`a4q1`…`a4q3`, `a5q1`…`a5q6`
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
    pub rewards: &'static [Act45Reward],
    /// Slot (journal 9 quêtes) qui doit être FLAG_DONE pour ACTIVER cette
    /// quête (`None` = activation au premier trigger).
    pub requires_done: Option<usize>,
}

/// Ids dans l'ordre des slots de save — ACTE IV (u16 @395/397/399 Normal).
/// ATTENTION : `a4q2` = **Terror's End/Diablo** (wiki « A4Q3 ») et `a4q3` =
/// **Hell's Forge** (wiki « A4Q2 ») — mapping `qstsa4q*` string.tsv:937-939,
/// PAS l'ordre du journal wiki (spec §5).
pub const ACT4_QUEST_IDS: [&str; 3] = ["a4q1", "a4q2", "a4q3"];
/// Ids dans l'ordre des slots de save — ACTE V (u16 @415-425 Normal).
pub const ACT5_QUEST_IDS: [&str; 6] = ["a5q1", "a5q2", "a5q3", "a5q4", "a5q5", "a5q6"];

/// Slots GLOBAUX du journal 9 quêtes (0-2 = acte IV, 3-8 = acte V).
mod slot {
    pub const A4Q2_DIABLO: usize = 1;
    pub const A5Q3: usize = 5;
    pub const A5Q5: usize = 7;
    pub const A5Q6: usize = 8;
}

/// Gate retail « Rite de Passage » (étape 19b — README oracle d2s :
/// « minimal level constraint to finish each difficulty (due to "Rite of
/// Passage" quest requirements) ») : niveau MINIMAL POUR FAIRE la quête des
/// Anciens (a5q5), par difficulté (index 0/1/2 = Normal/Cauchemar/Enfer).
/// En-dessous, l'autel des Cieux n'active pas (et les Anciens ne spawnent
/// pas) — la difficulté ne peut littéralement pas se finir avant ce niveau.
pub const ANCIENTS_MIN_LEVELS: [u32; 3] = [20, 40, 60];

/// `patch_levels.txt` : ids de niveaux réels des triggers (re-vérifiés le
/// 2026-10-24 — cf. en-tête de module).
mod lvl {
    pub const PLAINS_OF_DESPAIR: u32 = 105; // « Mesa 2 »
    pub const RIVER_OF_FLAME: u32 = 107; // « Lava 1 »
    pub const CHAOS_SANCTUARY: u32 = 108; // « Diablo 1 »
    pub const BLOODY_FOOTHILLS: u32 = 110; // « Siege 1 »
    /// 4 extérieurs A5 disponibles pour les cages (spec §6 [ARRANGÉ-MVP]) :
    /// Barricade 1, Barricade 2, Barricade Snow (+ Bloody Foothills 110).
    pub const RESCUE_AREAS: [u32; 4] = [110, 111, 112, 117];
    pub const ICE_CAVE_3A: u32 = 119;
    pub const MOUNTAIN_TOP: u32 = 120; // Sommet d'Arreat / Autel des Cieux
    pub const HALLS_OF_VAUGHT: u32 = 124; // « Temple Boss »
    pub const THRONE_OF_DESTRUCTION: u32 = 131; // QuestFlag 39
    pub const WORLDSTONE_CHAMBER: u32 = 132;
}

/// Authoring complet de l'ACTE IV (spec §5 — ordre de sauvegarde).
pub(crate) static DEFS_ACT4: [DefAct45; 3] = [
    // slot 0 — a4q1 The Fallen Angel (Izual) : Plaines du Désespoir id 105 →
    // kill (spawn de quête, PAS un superunique) → remise Tyrael +2 skills.
    DefAct45 {
        id: "a4q1",
        name: "The Fallen Angel",
        steps: &[
            StepDef {
                desc: "Traversez les Plaines du Désespoir.",
                trig: Trig::Zone(lvl::PLAINS_OF_DESPAIR),
                count: 1,
            },
            StepDef { desc: "Abattez Izual pour libérer son âme.", trig: Trig::Kill(&["izual"]), count: 1 },
        ],
        reward_npc: Some("tyrael"),
        rewards: &[Act45Reward::SkillPoint(2), Act45Reward::Xp(1500)],
        requires_done: None,
    },
    // slot 1 — a4q2 Terror's End (wiki « A4Q3 », DIABLO) : Chaos Sanctuaire
    // id 108 → 3 gardiens de sceaux (5 sceaux retail [ARRANGÉ-MVP]) →
    // Diablo (spawn = quests_finale.rs) → Tyrael → accès acte V. PORTEUSE.
    DefAct45 {
        id: "a4q2",
        name: "Terror's End",
        steps: &[
            StepDef {
                desc: "Trouvez Diablo dans son sanctuaire (Chaos Sanctuaire).",
                trig: Trig::Zone(lvl::CHAOS_SANCTUARY),
                count: 1,
            },
            StepDef {
                desc: "Brisez les sceaux et abattez leurs gardiens (Infector, De Seis, Grand Vizier).",
                trig: Trig::Any(&[
                    &Trig::Kill(&["infector of souls", "megademon3"]),
                    &Trig::Kill(&["lord de seis", "doomknight3"]),
                    &Trig::Kill(&["grand vizier of chaos", "fingermage3"]),
                ]),
                count: 3,
            },
            StepDef { desc: "Tuez Diablo.", trig: Trig::Kill(&["diablo"]), count: 1 },
        ],
        reward_npc: Some("tyrael"),
        rewards: &[Act45Reward::UnlockAccessAct(5), Act45Reward::Xp(2000)],
        requires_done: None,
    },
    // slot 2 — a4q3 Hell's Forge (wiki « A4Q2 ») : `mss` (drop A3Q6 — gating
    // par l'objet) → Rivière de feu id 107 → Hephasto (spawn de quête
    // class `hephasto`, variante superunique « The Feature Creep ») →
    // marteau `hfh` → Cain. Gemmes aléatoires retail [MVP : 3 parfaites].
    DefAct45 {
        id: "a4q3",
        name: "Hell's Forge",
        steps: &[
            StepDef {
                desc: "Portez la Pierre d'âme de Mephisto.",
                trig: Trig::Item("mss"),
                count: 1,
            },
            StepDef {
                desc: "Descendez dans la Rivière de feu.",
                trig: Trig::Zone(lvl::RIVER_OF_FLAME),
                count: 1,
            },
            StepDef {
                desc: "Abattez Hephasto le forgeron.",
                trig: Trig::Kill(&["hephasto", "The Feature Creep"]),
                count: 1,
            },
            StepDef {
                desc: "Emparez-vous du Marteau de la Forge infernale.",
                trig: Trig::Item("hfh"),
                count: 1,
            },
        ],
        reward_npc: Some("cain"),
        rewards: &[
            Act45Reward::Item("gpv"),
            Act45Reward::Item("gpy"),
            Act45Reward::Item("gpb"),
            Act45Reward::Xp(1000),
        ],
        requires_done: None,
    },
];

/// Authoring complet de l'ACTE V (spec §6 — ordre wiki = ordre save).
pub(crate) static DEFS_ACT5: [DefAct45; 6] = [
    // a5q1 Siege on Harrogath : Contreforts sanglants id 110 → Siege Boss
    // (superuniques.txt:45, class `overseer1`, alias Shenk) → Larzuk serti.
    DefAct45 {
        id: "a5q1",
        name: "Siege on Harrogath",
        steps: &[
            StepDef {
                desc: "Percéez les Contreforts sanglants et levez le siège.",
                trig: Trig::Zone(lvl::BLOODY_FOOTHILLS),
                count: 1,
            },
            StepDef {
                desc: "Abattez Shenk le Surveillant (Siege Boss).",
                trig: Trig::Kill(&["siegeboss", "Siege Boss", "overseer1", "shenk"]),
                count: 1,
            },
        ],
        reward_npc: Some("larzuk"),
        rewards: &[Act45Reward::LarzukSocket, Act45Reward::Xp(1000)],
        requires_done: None,
    },
    // a5q2 Rescue on Mount Arreat : cages ×15 retail sur 4 extérieurs ids
    // 110/111/112/117 [ARRANGÉ-MVP : 1 extérieur = 1 vague] → Qual-Kehk →
    // runes Ral+Ort+Tal (recette Ancient's Pledge, `qstsa5q24`) + barbares.
    DefAct45 {
        id: "a5q2",
        name: "Rescue on Mount Arreat",
        steps: &[StepDef {
            desc: "Libérez les barbares prisonniers des Hautes-Glaces (4 extérieurs).",
            trig: Trig::Any(&[
                &Trig::Zone(lvl::RESCUE_AREAS[0]),
                &Trig::Zone(lvl::RESCUE_AREAS[1]),
                &Trig::Zone(lvl::RESCUE_AREAS[2]),
                &Trig::Zone(lvl::RESCUE_AREAS[3]),
            ]),
            count: 4,
        }],
        reward_npc: Some("qual-kehk"),
        rewards: &[
            Act45Reward::Item("r08"),
            Act45Reward::Item("r09"),
            Act45Reward::Item("r07"),
            Act45Reward::BarbarianMercs,
            Act45Reward::Xp(1000),
        ],
        requires_done: None,
    },
    // a5q3 Prison of Ice : Caverne de glace id 119 → Frozenstein
    // (superuniques.txt:62, class `snowyeti4`) → Anya (potion de Malah) →
    // Malah → `tr2` (+10 résistances, bit 7 posé [CITÉ trevin]) + objet
    // personnalisé Anya.
    DefAct45 {
        id: "a5q3",
        name: "Prison of Ice",
        steps: &[
            StepDef {
                desc: "Trouvez Anya gelée sous le Passage Cristallin (Ice Cave 3A).",
                trig: Trig::Zone(lvl::ICE_CAVE_3A),
                count: 1,
            },
            StepDef {
                desc: "Abattez Frozenstein qui garde la prison de glace.",
                trig: Trig::Kill(&["frozenstein", "snowyeti4", "Frozenstein"]),
                count: 1,
            },
            StepDef {
                desc: "Donnez à Anya la potion de Malah et ramenez-la en ville.",
                trig: Trig::Talk("anya"),
                count: 1,
            },
        ],
        reward_npc: Some("malah"),
        rewards: &[Act45Reward::ResistAll(10), Act45Reward::AnyaPersonalize, Act45Reward::Xp(1000)],
        requires_done: None,
    },
    // a5q4 Betrayal of Harrogath : GATÉE sur a5q3 (« ACTIF (après A5Q3) »,
    // spec §6) — portail d'Anya → Repaire de Vaught id 124 → Nihlathak
    // (superuniques.txt:63, class `nihlathakboss`) → Anya (personnalisation).
    DefAct45 {
        id: "a5q4",
        name: "Betrayal of Harrogath",
        steps: &[
            StepDef {
                desc: "Prenez le portail d'Anya vers le temple de Nihlathak.",
                trig: Trig::Talk("anya"),
                count: 1,
            },
            StepDef {
                desc: "Descendez jusqu'au Repaire de Vaught (Halls of Vaught).",
                trig: Trig::Zone(lvl::HALLS_OF_VAUGHT),
                count: 1,
            },
            StepDef {
                desc: "Tuez Nihlathak le traître.",
                trig: Trig::Kill(&["nihlathak", "nihlathakboss", "Nihlathak Boss"]),
                count: 1,
            },
        ],
        reward_npc: Some("anya"),
        rewards: &[Act45Reward::AnyaPersonalize, Act45Reward::Xp(1500)],
        requires_done: Some(slot::A5Q3),
    },
    // a5q5 Rite of Passage : Sommet d'Arreat id 120 → Autel des Cieux
    // [ARRANGÉ-MVP : l'entrée vaut activation, pattern autel Gidbinn a3q3] →
    // 3 Anciens (spawn scripté ici — activés par la quête, TC vide) →
    // accès Chambre de la Pierre d'Worldstone [FIDÈLE]. XP très élevé
    // [INVENTÉ — Exp des Anciens vide dans patch_monstats].
    DefAct45 {
        id: "a5q5",
        name: "Rite of Passage",
        steps: &[
            StepDef {
                desc: "Consultez les Anciens en activant l'Autel des Cieux (Sommet d'Arreat).",
                trig: Trig::Zone(lvl::MOUNTAIN_TOP),
                count: 1,
            },
            StepDef {
                desc: "Affrontez les trois Anciens sans quitter le Mont Arreat.",
                trig: Trig::Any(&[
                    &Trig::Kill(&["ancientbarb1", "Ancient Barbarian 1"]),
                    &Trig::Kill(&["ancientbarb2", "Ancient Barbarian 2"]),
                    &Trig::Kill(&["ancientbarb3", "Ancient Barbarian 3"]),
                ]),
                count: 3,
            },
        ],
        reward_npc: None,
        rewards: &[Act45Reward::Xp(4000)],
        requires_done: None,
    },
    // a5q6 Eve of Destruction : GATÉE sur a5q5 (« ACTIF (après A5Q5) », spec
    // §6) — Trône id 131 (vagues = quests_finale.rs) → Baal `baalcrab`
    // (Chambre id 132) → Tyrael → portail → COMPLET = FIN DE JEU (event
    // GameWon + progression). PORTEUSE FINALE.
    DefAct45 {
        id: "a5q6",
        name: "Eve of Destruction",
        steps: &[
            StepDef {
                desc: "Affrontez les vagues de Baal dans le Trône de la Destruction.",
                trig: Trig::Zone(lvl::THRONE_OF_DESTRUCTION),
                count: 1,
            },
            StepDef {
                desc: "Tuez Baal dans la Chambre de la Pierre d'Worldstone.",
                trig: Trig::Kill(&["baal", "baalcrab"]),
                count: 1,
            },
            StepDef {
                desc: "Parlez à Tyrael et prenez son portail.",
                trig: Trig::Talk("tyrael"),
                count: 1,
            },
        ],
        reward_npc: None,
        rewards: &[Act45Reward::Xp(4000)],
        requires_done: Some(slot::A5Q5),
    },
];

// ---------------------------------------------------------------------------
// État + moteur
// ---------------------------------------------------------------------------

/// État d'exécution d'une quête A4/A5 (sérialisé, façon flags u16 .d2s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Act45Run {
    /// Phase courante.
    pub phase: ActPhase,
    /// Index de l'étape courante (figé à la dernière quand la quête sort
    /// d'Active).
    pub step: usize,
    /// Occurrences acquises sur l'étape courante.
    pub count: u32,
    /// Flags u16 .d2s (bits 0/1/2 génériques + bit 7 a5q3 [`FLAG_RES_SCROLL`]).
    pub flags: u16,
    /// Masque des sous-objectifs d'une étape [`Trig::Any`] (bit i = alternative
    /// i déjà comptée). État MOTEUR additif — PAS un bit .d2s ; remis à zéro
    /// à chaque changement d'étape.
    pub subdone: u32,
}

/// Journal de quêtes des ACTES IV-V + fin de jeu (spec quests-actes.md
/// §5/§6). 9 slots : 0-2 = acte IV (`a4q1`→`a4q3` en ORDRE DE SAUVEGARDE —
/// slot 1 = Diablo !), 3-8 = acte V (`a5q1`→`a5q6`). Authoring statique ;
/// seul l'état est sérialisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Act45Quests {
    runs: [Act45Run; 9],
    /// Contexte RUNTIME de la gate retail a5q5 (étape 19b) : niveau courant
    /// du héros. `None` = journal PUR sans contexte de jeu (défaut des
    /// tests et de `from_json` : AUCUNE gate). Jamais sérialisé — la save
    /// porte l'état des quêtes, pas le contexte.
    gate_level: Option<u32>,
    /// Difficulté du contexte de gate (index 0/1/2).
    gate_difficulty: usize,
}

impl Default for Act45Quests {
    fn default() -> Self {
        Self::new()
    }
}

impl Act45Quests {
    /// Journal neuf : tout INACTIF, flags 0, sans contexte de gate.
    pub fn new() -> Act45Quests {
        Act45Quests {
            runs: [Act45Run { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0, subdone: 0 }; 9],
            gate_level: None,
            gate_difficulty: 0,
        }
    }

    /// Pose le contexte de la gate retail a5q5 (étape 19b) : niveau courant
    /// du héros + difficulté (index 0/1/2). Appelé par
    /// [`World::quest_actes45_event`] à chaque event ; `None` = journal pur
    /// sans gate (défaut).
    pub fn set_level_gate(&mut self, hero_level: Option<u32>, difficulty: usize) {
        self.gate_level = hero_level;
        self.gate_difficulty = difficulty;
    }

    /// Niveau minimal retail POUR FAIRE la quête des Anciens (a5q5) à la
    /// difficulté `difficulty` (index 0/1/2 ; hors bornes = Enfer) —
    /// oracle d2s-file-generator : 20 Normal / 40 Cauchemar / 60 Enfer.
    pub fn ancients_min_level(difficulty: usize) -> u32 {
        ANCIENTS_MIN_LEVELS[difficulty.min(2)]
    }

    /// Gate a5q5 ACTIVE ? (contexte présent ET niveau du héros sous le
    /// seuil de la difficulté : l'autel des Cieux n'active pas, ni les
    /// Anciens ne spawnent.) Contexte absent (`None`) = jamais bloquée.
    pub fn ancients_gate_blocked(&self) -> bool {
        match self.gate_level {
            Some(level) => level < Self::ancients_min_level(self.gate_difficulty),
            None => false,
        }
    }

    /// Déf d'un slot global (0-8).
    pub(crate) fn def_of(&self, slot: usize) -> &'static DefAct45 {
        if slot < 3 {
            &DEFS_ACT4[slot]
        } else {
            &DEFS_ACT5[slot - 3]
        }
    }

    /// Ids des 9 slots (ordre de sauvegarde).
    pub fn ids(&self) -> [&'static str; 9] {
        let mut out = [""; 9];
        for (i, id) in ACT4_QUEST_IDS.into_iter().chain(ACT5_QUEST_IDS).enumerate() {
            out[i] = id;
        }
        out
    }

    /// État d'une quête par id (`None` si id inconnu).
    pub fn run(&self, id: &str) -> Option<&Act45Run> {
        self.ids().iter().position(|s| *s == id).map(|i| &self.runs[i])
    }

    /// Phases dans l'ordre des slots (diff/events UI).
    pub fn phases(&self) -> [ActPhase; 9] {
        let mut out = [ActPhase::Inactive; 9];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.phase;
        }
        out
    }

    /// Flags u16 dans l'ordre des slots — image directe des u16 .d2s des
    /// actes IV-V (offsets [`Act45Quests::d2s_flag_offset`]).
    pub fn quest_flags(&self) -> [u16; 9] {
        let mut out = [0u16; 9];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.flags;
        }
        out
    }

    /// Gating d'accès (spec §1.4). Ce journal couvre les actes IV-V : actes
    /// 1-4 = prérequis portés par les journaux A1 (a1q6) et A2-A3 (a2q6,
    /// a3q6) — considérés ouverts ; acte 5 = a4q2 bit 0 (portail de Tyrael)
    /// ; au-delà : la fin de jeu est [`Act45Quests::game_won`].
    pub fn access_act(&self, act: u8) -> bool {
        match act {
            1..=4 => true,
            5 => self.runs[slot::A4Q2_DIABLO].flags & FLAG_DONE != 0,
            _ => false,
        }
    }

    /// Portail d'Anya vers le temple de Nihlathak : ouvert dès a5q3 Done,
    /// FERMÉ après a5q4 (spec §1.4/§6 [CITÉ]).
    pub fn nihlathak_portal_open(&self) -> bool {
        self.runs[slot::A5Q3].phase == ActPhase::Done
            && self.runs[slot::A5Q3 + 1].phase != ActPhase::Done
    }

    /// Chambre de la Pierre d'Worldstone (id 132) : ouverte à la complétion
    /// du Rite de Passage (a5q5, spec §6 [FIDÈLE]).
    pub fn worldstone_open(&self) -> bool {
        self.runs[slot::A5Q5].phase == ActPhase::Done
    }

    /// Fin de jeu : a5q6 Done (Baal mort, portail de Tyrael pris — spec §6
    /// « fin de jeu »). Le World émet [`crate::GameEvent::GameWon`] à la
    /// transition.
    pub fn game_won(&self) -> bool {
        self.runs[slot::A5Q6].phase == ActPhase::Done
    }

    /// Octet de progression .d2s 0x0025 — part locale de CE journal :
    /// acte V débloqué (a4q2) + fin de jeu (a5q6). L'octet complet = cette
    /// valeur + les parts des journaux A1 et A2-A3.
    pub fn progression_byte(&self) -> u8 {
        u8::from(self.access_act(5)) + u8::from(self.game_won())
    }

    /// Fait avancer le journal avec un événement ; renvoie les récompenses
    /// rendues. Un événement n'avance qu'UNE occurrence de l'étape courante
    /// d'UNE quête (par slot) ; hors ordre = ignoré (comme les actes I-III).
    pub fn on_event(&mut self, ev: &ActEvent) -> Vec<Act45Reward> {
        let mut out = Vec::new();
        for slot in 0..9 {
            // Gate retail 19b : la quête des Anciens (a5q5) n'avance JAMAIS
            // sous le niveau minimal de la difficulté (20/40/60, oracle d2s)
            // — ni activation, ni kills : l'autel ne « fait » pas la quête.
            if slot == slot::A5Q5 && self.ancients_gate_blocked() {
                continue;
            }
            match self.runs[slot].phase {
                ActPhase::Inactive => {
                    // Gating inter-quêtes (a5q4 ← a5q3, a5q6 ← a5q5).
                    if let Some(dep) = self.def_of(slot).requires_done {
                        if self.runs[dep].flags & FLAG_DONE == 0 {
                            continue;
                        }
                    }
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
                            // a5q3 : le parchemin de résistance est consommé
                            // à la remise de Malah (bit 7 [CITÉ trevin]).
                            if slot == slot::A5Q3 {
                                self.runs[slot].flags |= FLAG_RES_SCROLL;
                            }
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
    /// de l'alternative matchée (0 pour les triggers simples ; [`Trig::Any`]
    /// = première alternative non encore comptée).
    fn step_match_index(&self, slot: usize, step: usize, ev: &ActEvent) -> Option<usize> {
        let def = self.def_of(slot);
        match def.steps.get(step) {
            Some(StepDef { trig: Trig::Any(alts), .. }) => alts.iter().enumerate().find_map(|(i, t)| {
                ((self.runs[slot].subdone >> i) & 1 == 0 && t.matches(ev)).then_some(i)
            }),
            Some(s) => s.trig.matches(ev).then_some(0),
            None => None,
        }
    }

    /// Étape `done_step` franchie : suivante, ou sortie d'Active
    /// (RewardPending / Done direct selon `reward_npc`).
    fn advance_after_step(&mut self, slot: usize, done_step: usize, out: &mut Vec<Act45Reward>) {
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

    // -- mapping .d2s (spec §1.3 — table golden actes IV-V) ------------------

    /// Offset absolu du u16 de quête : acte IV = 395 + 2·slot + 96·diff
    /// (slots 395/397/399), acte V = 415 + 2·slot + 96·diff (415/…/425) ;
    /// Cauchemar +96, Enfer +192 (spec §1.3, table verrouillée par les tests
    /// golden `golden_d2s_offsets_actes_45`). `act` = 4 ou 5, `slot` =
    /// 0..=2 (IV) ou 0..=5 (V) (slot LOCAL d'acte), `difficulty` 0..=2.
    pub fn d2s_flag_offset(act: u8, slot: usize, difficulty: usize) -> u32 {
        let base = match act {
            4 => 395,
            5 => 415,
            _ => 0,
        };
        base + 2 * slot as u32 + 96 * difficulty as u32
    }

    // -- persistance (contrat save.rs — champ additif `quest_actes45`) -------

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
                act: if slot < 3 { 4 } else { 5 },
                phase: r.phase,
                step: r.step,
                count: r.count,
                flags: r.flags,
                subdone: r.subdone,
            })
            .collect();
        let s = State { version: STATE_VERSION, quests };
        serde_json::to_string(&s).expect("d2rules: sérialisation quest_actes45")
    }

    /// Recharge un état produit par [`Act45Quests::to_json`]. Validation
    /// stricte (version, ids/actes dans l'ordre, bornes d'étapes, masque
    /// `subdone` cohérent, gating `requires_done` respecté, flags cohérents
    /// avec la phase, bit 7 réservé à a5q3 Done) — tout écart ⇒ Err(()).
    pub fn from_json(json: &str) -> Result<Act45Quests, ()> {
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
        if s.version != STATE_VERSION || s.quests.len() != 9 {
            return Err(());
        }
        let mut runs = [Act45Run { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0, subdone: 0 }; 9];
        for (slot, save) in s.quests.into_iter().enumerate() {
            if save.id != ids_slot(slot) || save.act != if slot < 3 { 4 } else { 5 } {
                return Err(());
            }
            let def = defs_slot(slot);
            let last = def.steps.len() - 1;
            let req_last = def.steps[last].count.max(1);
            let any_alts = |step: usize| -> Option<usize> {
                match def.steps.get(step).map(|s| &s.trig) {
                    Some(Trig::Any(alts)) => Some(alts.len()),
                    _ => None,
                }
            };
            let subdone_ok = |step: usize, subdone: u32, count: u32| -> bool {
                match any_alts(step) {
                    Some(alts) => subdone < (1u32 << alts) && subdone.count_ones() == count,
                    None => subdone == 0,
                }
            };
            // Bit 7 « parchemin de résistance » : a5q3 Done UNIQUEMENT.
            let res_ok = if slot == slot::A5Q3 {
                save.phase == ActPhase::Done || save.flags & FLAG_RES_SCROLL == 0
            } else {
                save.flags & FLAG_RES_SCROLL == 0
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
                        && res_ok
                        && gating_ok(slot, def.requires_done, &runs)
                }
                ActPhase::Done => {
                    save.step == last
                        && save.count == req_last
                        && save.subdone == 0
                        && save.flags & FLAG_DONE != 0
                        && save.flags & FLAG_REWARD_PENDING == 0
                        && res_ok
                        && gating_ok(slot, def.requires_done, &runs)
                }
            };
            if !ok {
                return Err(());
            }
            runs[slot] =
                Act45Run { phase: save.phase, step: save.step, count: save.count, flags: save.flags, subdone: save.subdone };
        }
        Ok(Act45Quests { runs, gate_level: None, gate_difficulty: 0 })
    }
}

const STATE_VERSION: u32 = 1;

/// Id du slot global (helper from_json).
fn ids_slot(slot: usize) -> &'static str {
    if slot < 3 {
        ACT4_QUEST_IDS[slot]
    } else {
        ACT5_QUEST_IDS[slot - 3]
    }
}

/// Déf du slot global (helper from_json).
fn defs_slot(slot: usize) -> &'static DefAct45 {
    if slot < 3 {
        &DEFS_ACT4[slot]
    } else {
        &DEFS_ACT5[slot - 3]
    }
}

/// Gating : la dépendance `requires_done` du slot est-elle satisfaite dans
/// l'état en cours de reconstruction ?
fn gating_ok(slot: usize, requires: Option<usize>, runs: &[Act45Run; 9]) -> bool {
    match requires {
        Some(dep) if dep != slot => runs[dep].flags & FLAG_DONE != 0,
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// BOSS scriptés A4/A5 — IZUAL + HEPHASTO + 3 ANCIENS (DONNÉES RÉELLES).
// Diablo/Baal/vagues = quests_finale.rs (coéquipier) ; superuniques
// data-driven (Siege Boss, Frozenstein, Nihlathak, gardiens de sceaux) =
// chemin d2game. Mêmes types que l'étape 8d (quests_actes2::BossRec).
// ---------------------------------------------------------------------------

/// Tables embarquées (même arbre assets que d2rules/tables.rs — v1.12).
const PATCH_MONSTATS: &str = include_str!("../../../../assets/patch_monstats.txt");
const MONLVL: &str = include_str!("../../../../assets/MonLvl.txt");

/// Les 5 spawns de quête scriptés des actes IV-V (mission 10c). Les VALEURS
/// (niveau, HP, AC, XP, TC, résists) sont lues RUNTIME dans
/// `patch_monstats.txt` / `MonLvl.txt` ([`boss_stats45`]) — aucune constante
/// dupliquée ; `boss_stats45` renvoie None sur toute table inattendue.
pub const BOSSES_ACTS45: [BossRec; 5] = [
    // Izual — patch_monstats `izual` (spawn de quête, PAS un superunique),
    // Plaines du Désespoir id 105, quête a4q1 (slot 0). Code sprite « 22 ».
    BossRec { id: "izual", code: "22", name: "Izual", tc: "Izual", lair: lvl::PLAINS_OF_DESPAIR, quest_slot: 0 },
    // Hephasto — classe de quête `hephasto` (patch_monstats), Rivière de feu
    // id 107, quête a4q3 (slot 2). Code sprite « 5P ». TC « Haphesto »
    // (superunique « The Feature Creep », superuniques.txt:43).
    BossRec { id: "hephasto", code: "5P", name: "Hephasto", tc: "Haphesto", lair: lvl::RIVER_OF_FLAME, quest_slot: 2 },
    // Les 3 Anciens — superuniques.txt:46-48, classes `ancientbarb1/2/3`,
    // Sommet d'Arreat id 120, quête a5q5 (slot 7). Spawn scripté à
    // l'activation de l'Autel des Cieux ; colonne TC VIDE = pas de drop
    // dédié [VÉRIFIÉ-LOCAL].
    BossRec { id: "ancientbarb1", code: "0D", name: "Ancient Barbarian 1", tc: "", lair: lvl::MOUNTAIN_TOP, quest_slot: 7 },
    BossRec { id: "ancientbarb2", code: "0F", name: "Ancient Barbarian 2", tc: "", lair: lvl::MOUNTAIN_TOP, quest_slot: 7 },
    BossRec { id: "ancientbarb3", code: "0E", name: "Ancient Barbarian 3", tc: "", lair: lvl::MOUNTAIN_TOP, quest_slot: 7 },
];

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

/// Record patch_monstats d'un boss scripté — parse runtime, cache OnceLock.
/// Renvoie `(level, hp, ac, exp, tc, res)`.
fn monstats_boss45(id: &str) -> Option<(u32, u32, u32, u32, String, [i32; 6])> {
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

/// Stats RÉELLES d'un boss scripté (actes IV-V) : record
/// `patch_monstats.txt` + ligne `MonLvl.txt` (N) du niveau →
/// [`crate::SpawnRule`] prête pour [`crate::World::spawn_monster_rule`]
/// (même convention que d2game : HP/DM/XP = % de la ligne MonLvl, AC/TH
/// directes, résists Normal). Lookup par id monstats (`"izual"`) ou code
/// manifest (`"22"`), insensible à la casse. `None` : boss inconnu, table
/// absente/inattendue, ou AUCUNE TC (les 3 Anciens : `rule.treasure_class`
/// = None — pas de drop dédié [VÉRIFIÉ-LOCAL]).
pub fn boss_stats45(id_or_code: &str) -> Option<BossStats> {
    let rec = BOSSES_ACTS45
        .iter()
        .find(|b| b.id.eq_ignore_ascii_case(id_or_code) || b.code.eq_ignore_ascii_case(id_or_code))?;
    let (level, hp, ac, exp, tc, res) = monstats_boss45(rec.id)?;
    if level == 0 || hp == 0 {
        return None; // table inattendue : pas de règle fantaisiste
    }
    let (ml_ac, ml_th, ml_hp, ml_dm, ml_xp) = monlvl_row(level)?;
    let tc = if !tc.is_empty() { Some(tc) } else if !rec.tc.is_empty() { Some(rec.tc.to_string()) } else { None };
    let rule = crate::SpawnRule {
        kind: crate::MonKind::SuperUnique,
        hp_pct: ml_hp,
        dmg_pct: ml_dm,
        th_pct: ml_th,
        xp_pct: ml_xp,
        ac: ml_ac,
        treasure_class: tc,
        res,
    };
    Some(BossStats { rec: *rec, level, hp, ac, exp, res, rule })
}

/// Boss scriptés dont l'étape COURANTE (ou d'activation) du journal réagit
/// à `ev` — déclencheur du spawn au trigger zone (Plaines du Désespoir,
/// Rivière de feu, Sommet d'Arreat). Renvoie les `(code manifest, id
/// monstats)` à spawner DANS L'ORDRE (3 Anciens d'un coup). Appelé AVANT
/// avancement : un boss déjà au-delà (récompense/fini) ne re-spawn jamais ;
/// la garde « vivant » de [`World::spawn_scripted_boss`] évite les doublons.
pub(crate) fn scripted_bosses45_for_trigger(log: &Act45Quests, ev: &ActEvent) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    for b in BOSSES_ACTS45 {
        // Gate retail 19b : niveau insuffisant ⇒ l'autel des Anciens
        // n'active pas et AUCUN Ancien ne spawn (oracle d2s 20/40/60).
        if b.quest_slot == slot::A5Q5 && log.ancients_gate_blocked() {
            continue;
        }
        let run = &log.runs[b.quest_slot];
        let step = match run.phase {
            ActPhase::Inactive => 0,
            ActPhase::Active => run.step,
            _ => continue,
        };
        let def = log.def_of(b.quest_slot);
        let step_trig = def.steps.get(step).map(|s| &s.trig);
        let zone_now = matches!(ev, ActEvent::ZoneEntered { level_id } if *level_id == b.lair);
        // Activation : l'étape courante (ou 0) EST le trigger zone du repaire.
        let matches_zone = matches!(step_trig, Some(Trig::Zone(id)) if *id == b.lair) && zone_now;
        // Re-entrée AVANT le kill (reload) : re-spawn si l'étape de kill est
        // courante — simple (Kill) OU hors ordre (Any contenant ce boss).
        let step_wants_kill = match step_trig {
            Some(Trig::Kill(aliases)) => aliases.contains(&b.id) || aliases.contains(&b.code) || aliases.contains(&b.name),
            Some(Trig::Any(alts)) => alts.iter().any(|t| {
                matches!(t, Trig::Kill(aliases) if aliases.contains(&b.id) || aliases.contains(&b.code) || aliases.contains(&b.name))
            }),
            _ => false,
        };
        let matches_kill_step = step_wants_kill && zone_now;
        if matches_zone || matches_kill_step {
            out.push((b.code, b.id));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Branchement World (bloc impl ADDITIF — save `quest_actes45`, events d'acte)
// ---------------------------------------------------------------------------

/// Objet de quête générique (récompense `Item`) — même MVP que les actes I-III.
fn quest_item_def(code: &str) -> crate::ItemDef {
    crate::ItemDef { code: code.to_string(), name: code.to_string(), kind: crate::ItemKind::Misc, amount: 0 }
}

impl World {
    /// Branche le journal spec des actes IV-V (étape 10c). `None` par défaut.
    pub fn set_quest_actes45(&mut self, log: Act45Quests) {
        self.quests_actes45 = Some(log);
    }

    /// Journal spec branché (lecture).
    pub fn quest_actes45(&self) -> Option<&Act45Quests> {
        self.quests_actes45.as_ref()
    }

    /// Journal spec branché (mutation directe — inspection tests/UI).
    pub fn quest_actes45_mut(&mut self) -> Option<&mut Act45Quests> {
        self.quests_actes45.as_mut()
    }

    /// Point d'entrée unique de l'intégrateur : avance le journal A4-A5 avec
    /// un event de gameplay (MÊME flux [`ActEvent`] que les journaux A1 et
    /// A2-A3 — l'appelant feede les trois), applique les récompenses motrices
    /// (XP, skill points, objets → inventaire/sol ; 19b : +10 % résists tout
    /// → `HeroStats::res_all_pct` ; serti/personnalisation/mercenaires =
    /// valeur de retour journal), émet
    /// [`GameEvent::ActQuestChanged`] par quête changée de phase,
    /// [`GameEvent::ActUnlocked`] au déblocage de l'acte V, **`GameEvent::
    /// GameWon` à la complétion d'a5q6** (fin de jeu — spec §6), fait
    /// SPAWNER les boss de quête au trigger zone (Izual id 105, Hephasto
    /// id 107, 3 Anciens id 120 — stats réelles [`boss_stats45`]) et renvoie
    /// les récompenses. Sans journal branché : no-op, renvoie vide.
    pub fn quest_actes45_event(&mut self, ev: &ActEvent) -> Vec<Act45Reward> {
        // Gate retail 19b (oracle d2s : Anciens = niveau 20/40/60) : le
        // contexte niveau héros + difficulté est posé à CHAQUE event (état
        // RUNTIME, jamais persisté — `from_json` recharge sans contexte).
        if let Some(log) = self.quests_actes45.as_mut() {
            log.set_level_gate(Some(self.players[0].data.stats.level), self.difficulty.index());
        }
        // Décision de spawn scripté sur l'état AVANT avancement.
        let spawns = self
            .quests_actes45
            .as_ref()
            .map(|log| scripted_bosses45_for_trigger(log, ev))
            .unwrap_or_default();
        let Some(log) = self.quests_actes45.as_mut() else {
            return Vec::new();
        };
        let before = log.phases();
        let gate_before = log.access_act(5);
        let won_before = log.game_won();
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
        if !gate_before && log.access_act(5) {
            self.pending_events.push(crate::GameEvent::ActUnlocked { act: 5 });
        }
        if !won_before && log.game_won() {
            self.pending_events.push(crate::GameEvent::GameWon);
        }
        for r in &rewards {
            match *r {
                Act45Reward::Xp(n) => {
                    let mut leveled = Vec::new();
                    self.apply_xp(n, &mut leveled);
                    self.pending_events.extend(leveled);
                }
                Act45Reward::SkillPoint(n) => self.grant_skill_points(n),
                Act45Reward::Item(code) => {
                    let def = quest_item_def(code);
                    if let Some(slot) = self.players[0].data.inv.iter().position(|s| s.is_none()) {
                        self.players[0].data.inv[slot] = Some(def);
                    } else {
                        let (x, y) = (self.players[0].ent.x, self.players[0].ent.y);
                        self.items.push(crate::ItemOnGround { def, x, y });
                    }
                }
                // 19b : Prison de glace — +10 % résists tout PERMANENT
                // appliqué au héros (champ save additif `res_all_pct`,
                // +10 par difficulté [CITÉ + oracle d2s confirmé]).
                Act45Reward::ResistAll(n) => {
                    self.players[0].data.stats.res_all_pct = self.players[0].data.stats.res_all_pct.saturating_add(n);
                }
                // Portés par le journal (gating d'acte) ou par la valeur de
                // retour (serti/personnalisation/mercenaires — l'UI et la
                // couche statlist les consomment) :
                Act45Reward::LarzukSocket
                | Act45Reward::AnyaPersonalize
                | Act45Reward::BarbarianMercs
                | Act45Reward::UnlockAccessAct(_) => {}
            }
        }
        // Spawns scriptés APRÈS avancement (garde « vivant » sur l'état à
        // jour ; l'échec de spawn n'annule jamais la progression de quête).
        for (code, _) in spawns {
            self.spawn_scripted_boss45(code);
        }
        rewards
    }

    /// Spawn d'un boss de quête A4/A5 près du héros avec ses stats réelles
    /// ([`boss_stats45`] → MonLvl (N) du boss) — miroir A4/A5 du
    /// `spawn_scripted_boss` d'étape 8d (qui reste scoping duriel/mephisto).
    /// Garde anti-doublon : un exemplaire VIVANT du même code manifest
    /// suffit. `false` : boss inconnu, déjà vivant, ou aucune tuile
    /// marchable.
    pub fn spawn_scripted_boss45(&mut self, code: &str) -> bool {
        // Garde : un exemplaire vivant du même code manifest suffit.
        let alive = self
            .mon_ent
            .iter()
            .zip(self.mon_dat.iter())
            .any(|(e, d)| e.anim != AnimState::DD && self.mon_tpls[d.tpl].code.eq_ignore_ascii_case(code));
        if alive {
            return false;
        }
        let Some(stats) = boss_stats45(code) else {
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
                    if self.spawn_monster_rule(stats.rec.code, hx + dx, hy + dy, rule.clone()) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Tests : machines a4q1→a4q3 / a5q1→a5q6, gates inter-quêtes, flags/d2s
// golden, save additive, boss scriptés (données réelles), intégration World,
// fin de jeu GameWon. Les grilles des tests World sont des STUBS marchables
// (carré ouvert) — cf. en-tête quests_actes2.rs pour le contrat DRLG.
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
    fn phase(log: &Act45Quests, id: &str) -> ActPhase {
        log.run(id).unwrap().phase
    }
    fn flags(log: &Act45Quests, id: &str) -> u16 {
        log.run(id).unwrap().flags
    }
    fn rewards_of(log: &mut Act45Quests, ev: &ActEvent) -> Vec<Act45Reward> {
        log.on_event(ev)
    }

    // -- a4q1 : Izual → Tyrael (+2 skills) -------------------------------------

    #[test]
    fn a4q1_fallen_angel_full_machine() {
        let mut log = Act45Quests::new();
        // Hors ordre : kill avant la zone — rien.
        assert!(rewards_of(&mut log, &kill("izual")).is_empty());
        assert_eq!(phase(&log, "a4q1"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &zone(lvl::PLAINS_OF_DESPAIR)).is_empty());
        assert_eq!(phase(&log, "a4q1"), ActPhase::Active);
        // Mauvais niveau (Mesa 1 id 104) : rien.
        assert!(rewards_of(&mut log, &zone(104)).is_empty());
        assert_eq!(log.run("a4q1").unwrap().step, 1);
        assert!(rewards_of(&mut log, &kill("izual")).is_empty());
        assert_eq!(phase(&log, "a4q1"), ActPhase::RewardPending);
        assert_eq!(flags(&log, "a4q1"), FLAG_REWARD_PENDING);
        let r = rewards_of(&mut log, &talk("tyrael"));
        assert_eq!(r, vec![Act45Reward::SkillPoint(2), Act45Reward::Xp(1500)]);
        assert_eq!(phase(&log, "a4q1"), ActPhase::Done);
        assert_eq!(flags(&log, "a4q1"), FLAG_DONE);
        // Rejouer : plus rien.
        assert!(rewards_of(&mut log, &talk("tyrael")).is_empty());
    }

    // -- a4q2 : sceaux hors ordre → Diablo → Tyrael → acte V (PORTEUSE) --------

    #[test]
    fn a4q2_seals_any_order_then_diablo_gates_act5() {
        let mut log = Act45Quests::new();
        assert!(!log.access_act(5));
        assert_eq!(log.progression_byte(), 0);
        // Diablo avant les sceaux : ignoré (hors ordre).
        assert!(rewards_of(&mut log, &zone(lvl::CHAOS_SANCTUARY)).is_empty());
        assert!(rewards_of(&mut log, &kill("diablo")).is_empty());
        assert_eq!(log.run("a4q2").unwrap().step, 1, "Diablo ignoré avant les sceaux");
        // 3 gardiens dans un ordre quelconque ; un doublon ne compte pas.
        assert!(rewards_of(&mut log, &kill_su("doomknight3", "Lord De Seis")).is_empty());
        assert!(rewards_of(&mut log, &kill("Infector of Souls")).is_empty());
        assert!(rewards_of(&mut log, &kill_su("fingermage3", "Grand Vizier of Chaos")).is_empty());
        assert!(rewards_of(&mut log, &kill("Infector of Souls")).is_empty(), "déjà compté");
        assert_eq!(log.run("a4q2").unwrap().step, 2, "étape Diablo atteinte (count remis à 0)");
        assert_eq!(log.run("a4q2").unwrap().subdone, 0, "masque remis à zéro au changement d'étape");
        // Diablo : PRÊT, acte V encore FERMÉ (portail de Tyrael).
        assert!(rewards_of(&mut log, &kill("diablo")).is_empty());
        assert_eq!(phase(&log, "a4q2"), ActPhase::RewardPending);
        assert!(!log.access_act(5), "l'accès acte V attend le portail de Tyrael");
        let r = rewards_of(&mut log, &talk("tyrael"));
        assert_eq!(r, vec![Act45Reward::UnlockAccessAct(5), Act45Reward::Xp(2000)]);
        assert_eq!(phase(&log, "a4q2"), ActPhase::Done);
        assert!(log.access_act(5));
        assert_eq!(log.progression_byte(), 1);
    }

    // -- a4q3 : mss → Rivière de feu → Hephasto → hfh → Cain --------------------

    #[test]
    fn a4q3_hells_forge_chain_order_enforced() {
        let mut log = Act45Quests::new();
        // Tout tenter en vrac : rien ne passe.
        assert!(rewards_of(&mut log, &zone(lvl::RIVER_OF_FLAME)).is_empty());
        assert!(rewards_of(&mut log, &item("hfh")).is_empty());
        assert_eq!(phase(&log, "a4q3"), ActPhase::Inactive);
        for ev in [item("mss"), zone(lvl::RIVER_OF_FLAME), kill_su("5P", "The Feature Creep"), item("hfh")] {
            assert!(rewards_of(&mut log, &ev).is_empty(), "étapes intermédiaires sans récompense");
        }
        assert_eq!(phase(&log, "a4q3"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("cain"));
        assert_eq!(
            r,
            vec![
                Act45Reward::Item("gpv"),
                Act45Reward::Item("gpy"),
                Act45Reward::Item("gpb"),
                Act45Reward::Xp(1000)
            ]
        );
        assert_eq!(phase(&log, "a4q3"), ActPhase::Done);
    }

    // -- a5q1 : Shenk → Larzuk ---------------------------------------------------

    #[test]
    fn a5q1_siege_shenk_then_larzuk() {
        let mut log = Act45Quests::new();
        assert!(rewards_of(&mut log, &kill("shenk")).is_empty()); // hors ordre
        assert_eq!(phase(&log, "a5q1"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &zone(lvl::BLOODY_FOOTHILLS)).is_empty());
        // Alias : superunique « Siege Boss », class `overseer1` OU « shenk ».
        assert!(rewards_of(&mut log, &kill_su("overseer1", "Siege Boss")).is_empty());
        assert_eq!(phase(&log, "a5q1"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("larzuk"));
        assert_eq!(r, vec![Act45Reward::LarzukSocket, Act45Reward::Xp(1000)]);
        assert_eq!(phase(&log, "a5q1"), ActPhase::Done);
    }

    // -- a5q2 : 4 extérieurs hors ordre → Qual-Kehk (runes Ral+Ort+Tal) ----------

    #[test]
    fn a5q2_rescue_four_areas_then_qual_kehk() {
        let mut log = Act45Quests::new();
        assert!(rewards_of(&mut log, &zone(lvl::RESCUE_AREAS[1])).is_empty());
        assert_eq!(phase(&log, "a5q2"), ActPhase::Active);
        assert_eq!(log.run("a5q2").unwrap().subdone, 0b0010);
        assert!(rewards_of(&mut log, &zone(lvl::RESCUE_AREAS[1])).is_empty(), "déjà comptée");
        assert_eq!(log.run("a5q2").unwrap().count, 1);
        for id in [lvl::RESCUE_AREAS[3], lvl::RESCUE_AREAS[0], lvl::RESCUE_AREAS[2]] {
            assert!(rewards_of(&mut log, &zone(id)).is_empty());
        }
        assert_eq!(phase(&log, "a5q2"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("qual-kehk"));
        assert_eq!(
            r,
            vec![
                Act45Reward::Item("r08"),
                Act45Reward::Item("r09"),
                Act45Reward::Item("r07"),
                Act45Reward::BarbarianMercs,
                Act45Reward::Xp(1000)
            ]
        );
        assert_eq!(phase(&log, "a5q2"), ActPhase::Done);
    }

    // -- a5q3 : Frozenstein → Anya → Malah (bit 7 + résistances) -----------------

    #[test]
    fn a5q3_prison_of_ice_sets_res_scroll_bit() {
        let mut log = Act45Quests::new();
        assert!(rewards_of(&mut log, &talk("malah")).is_empty()); // hors ordre
        assert_eq!(phase(&log, "a5q3"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &zone(lvl::ICE_CAVE_3A)).is_empty());
        assert!(rewards_of(&mut log, &kill_su("snowyeti4", "Frozenstein")).is_empty());
        // Anya (potion donnée) : PRÊT — bit 7 PAS encore posé.
        assert!(rewards_of(&mut log, &talk("anya")).is_empty());
        assert_eq!(phase(&log, "a5q3"), ActPhase::RewardPending);
        assert_eq!(flags(&log, "a5q3"), FLAG_REWARD_PENDING);
        let r = rewards_of(&mut log, &talk("malah"));
        assert_eq!(r, vec![Act45Reward::ResistAll(10), Act45Reward::AnyaPersonalize, Act45Reward::Xp(1000)]);
        assert_eq!(phase(&log, "a5q3"), ActPhase::Done);
        assert_eq!(flags(&log, "a5q3"), FLAG_DONE | FLAG_RES_SCROLL, "bit 7 posé à la remise de Malah");
    }

    // -- a5q4 : GATÉE sur a5q3 — portail d'Anya, Nihlathak -----------------------

    #[test]
    fn a5q4_betrayal_gated_on_prison_of_ice() {
        let mut log = Act45Quests::new();
        // Avant a5q3 : rien (gate requires_done), portail fermé.
        assert!(!log.nihlathak_portal_open());
        assert!(rewards_of(&mut log, &talk("anya")).is_empty());
        assert!(rewards_of(&mut log, &kill("nihlathak")).is_empty());
        assert_eq!(phase(&log, "a5q4"), ActPhase::Inactive);
        // Chaîne a5q3 complète.
        for ev in [zone(lvl::ICE_CAVE_3A), kill("frozenstein"), talk("anya"), talk("malah")] {
            log.on_event(&ev);
        }
        assert!(log.nihlathak_portal_open(), "portail d'Anya ouvert entre a5q3 et a5q4");
        // La même étape talk("anya") de a5q3 n'a PAS activé a5q4 (gate).
        assert_eq!(phase(&log, "a5q4"), ActPhase::Inactive);
        // Maintenant : portail → Vaught → Nihlathak → remise Anya.
        assert!(rewards_of(&mut log, &talk("anya")).is_empty());
        assert_eq!(flags(&log, "a5q4"), FLAG_GIVEN_BY_NPC);
        assert!(rewards_of(&mut log, &zone(121)).is_empty(), "Temple Entrance ≠ Halls of Vaught");
        assert!(rewards_of(&mut log, &zone(lvl::HALLS_OF_VAUGHT)).is_empty());
        assert!(rewards_of(&mut log, &kill_su("nihlathakboss", "Nihlathak Boss")).is_empty());
        assert_eq!(phase(&log, "a5q4"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("anya"));
        assert_eq!(r, vec![Act45Reward::AnyaPersonalize, Act45Reward::Xp(1500)]);
        assert_eq!(phase(&log, "a5q4"), ActPhase::Done);
        assert!(!log.nihlathak_portal_open(), "portail d'Anya FERMÉ après a5q4 [CITÉ]");
    }

    // -- a5q5 : Autel des Cieux → 3 Anciens hors ordre → Worldstone --------------

    #[test]
    fn a5q5_rite_of_passage_three_ancients() {
        let mut log = Act45Quests::new();
        assert!(!log.worldstone_open());
        assert!(rewards_of(&mut log, &kill("ancientbarb2")).is_empty()); // hors ordre
        assert_eq!(phase(&log, "a5q5"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &zone(lvl::MOUNTAIN_TOP)).is_empty());
        // 3 Anciens dans un ordre quelconque, par id ou nom de superunique.
        assert!(rewards_of(&mut log, &kill_su("0F", "Ancient Barbarian 2")).is_empty());
        assert!(rewards_of(&mut log, &kill("ancientbarb3")).is_empty());
        assert_eq!(phase(&log, "a5q5"), ActPhase::Active);
        // Le 3e kill complète direct (pas de PNJ de remise) : XP [INVENTÉ] +
        // accès Chambre de la Pierre d'Worldstone [FIDÈLE].
        let r = rewards_of(&mut log, &kill("ancientbarb1"));
        assert_eq!(r, vec![Act45Reward::Xp(4000)]);
        assert_eq!(phase(&log, "a5q5"), ActPhase::Done);
        assert_eq!(flags(&log, "a5q5"), FLAG_DONE);
        assert!(log.worldstone_open());
        // Sortir/ré-entrer le mont après coup : plus rien (terminée).
        assert!(rewards_of(&mut log, &zone(lvl::MOUNTAIN_TOP)).is_empty());
    }

    // -- a5q6 : GATÉE sur a5q5 — vagues (quests_finale.rs) → Baal → GameWon ------

    #[test]
    fn a5q6_eve_of_destruction_gated_on_rite_ends_game() {
        let mut log = Act45Quests::new();
        assert!(!log.game_won());
        // Avant a5q5 : rien (gate requires_done).
        assert!(rewards_of(&mut log, &zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        assert!(rewards_of(&mut log, &kill("baalcrab")).is_empty());
        assert_eq!(phase(&log, "a5q6"), ActPhase::Inactive);
        // a5q5 complète.
        log.on_event(&zone(lvl::MOUNTAIN_TOP));
        for b in ["ancientbarb1", "ancientbarb2", "ancientbarb3"] {
            log.on_event(&kill(b));
        }
        assert!(log.worldstone_open());
        // Trône (les vagues sont consommées côté quests_finale) → Baal par
        // code monstats `baalcrab` OU alias `baal` → Tyrael = FIN DE JEU.
        assert!(rewards_of(&mut log, &zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        assert!(rewards_of(&mut log, &kill("baalcrab")).is_empty());
        assert_eq!(phase(&log, "a5q6"), ActPhase::Active);
        let r = rewards_of(&mut log, &talk("tyrael"));
        assert_eq!(r, vec![Act45Reward::Xp(4000)]);
        assert_eq!(phase(&log, "a5q6"), ActPhase::Done);
        assert!(log.game_won());
        // a4q2 pas faite dans CE test : acte V pas débloqué — la part locale
        // de l'octet ne compte que la fin de jeu.
        assert_eq!(log.progression_byte(), 1);
    }

    // -- Golden : offsets .d2s des u16 des actes IV-V (spec §1.3) ----------------

    #[test]
    fn golden_d2s_offsets_actes_45() {
        let normal4 = [395, 397, 399];
        let normal5 = [415, 417, 419, 421, 423, 425];
        for slot in 0..3 {
            assert_eq!(Act45Quests::d2s_flag_offset(4, slot, 0), normal4[slot]);
            assert_eq!(Act45Quests::d2s_flag_offset(4, slot, 1), normal4[slot] + 96);
            assert_eq!(Act45Quests::d2s_flag_offset(4, slot, 2), normal4[slot] + 192);
        }
        for slot in 0..6 {
            assert_eq!(Act45Quests::d2s_flag_offset(5, slot, 0), normal5[slot]);
            assert_eq!(Act45Quests::d2s_flag_offset(5, slot, 1), normal5[slot] + 96);
            assert_eq!(Act45Quests::d2s_flag_offset(5, slot, 2), normal5[slot] + 192);
        }
        // Spécimens exacts de la spec : Izual @395/491/587, Diablo @397/493/589,
        // Forge @399/495/591 ; Siège @415/511/607, Prison de glace @419/515/611,
        // Traître @421/517/613, Rite @423/519/615, Crépuscule @425/521/617.
        assert_eq!(Act45Quests::d2s_flag_offset(4, 0, 2), 587);
        assert_eq!(Act45Quests::d2s_flag_offset(4, 1, 1), 493);
        assert_eq!(Act45Quests::d2s_flag_offset(4, 2, 1), 495);
        assert_eq!(Act45Quests::d2s_flag_offset(5, 0, 1), 511);
        assert_eq!(Act45Quests::d2s_flag_offset(5, 2, 1), 515);
        assert_eq!(Act45Quests::d2s_flag_offset(5, 3, 1), 517);
        assert_eq!(Act45Quests::d2s_flag_offset(5, 4, 1), 519);
        assert_eq!(Act45Quests::d2s_flag_offset(5, 5, 2), 617);
    }

    // -- Golden : flags u16 de la chaîne complète (9 machines) -------------------

    #[test]
    fn golden_full_acts45_flags() {
        let mut log = Act45Quests::new();
        for ev in [
            zone(lvl::PLAINS_OF_DESPAIR),
            kill("izual"),
            talk("tyrael"), // a4q1 done (+2 skills)
            zone(lvl::CHAOS_SANCTUARY),
            kill("Lord De Seis"),
            kill("Infector of Souls"),
            kill("Grand Vizier of Chaos"),
            kill("diablo"),
            talk("tyrael"), // a4q2 done → acte V
            item("mss"),
            zone(lvl::RIVER_OF_FLAME),
            kill_su("hephasto", "The Feature Creep"),
            item("hfh"),
            talk("cain"), // a4q3 done (gemmes)
            zone(lvl::BLOODY_FOOTHILLS),
            kill_su("overseer1", "Siege Boss"),
            talk("larzuk"), // a5q1 done (serti)
            zone(lvl::RESCUE_AREAS[1]),
            zone(lvl::RESCUE_AREAS[2]),
            zone(lvl::RESCUE_AREAS[3]),
            zone(lvl::RESCUE_AREAS[0]),
            talk("qual-kehk"), // a5q2 done (runes)
            zone(lvl::ICE_CAVE_3A),
            kill("frozenstein"),
            talk("anya"),
            talk("malah"), // a5q3 done (bit 7)
            talk("anya"),
            zone(lvl::HALLS_OF_VAUGHT),
            kill("nihlathak"),
            talk("anya"), // a5q4 done (personnalisation)
            zone(lvl::MOUNTAIN_TOP),
            kill("ancientbarb1"),
            kill("ancientbarb2"),
            kill("ancientbarb3"), // a5q5 done → worldstone
            zone(lvl::THRONE_OF_DESTRUCTION),
            kill("baalcrab"),
            talk("tyrael"), // a5q6 done → FIN DE JEU
        ] {
            log.on_event(&ev);
        }
        assert!(
            log.phases().iter().all(|p| *p == ActPhase::Done),
            "les 9 quêtes sont Done : {:?}",
            log.phases()
        );
        // Seule a5q4 (activation talk Anya) porte le bit 2 ; seule a5q3 le bit 7.
        assert_eq!(
            log.quest_flags(),
            [0x0001, 0x0001, 0x0001, 0x0001, 0x0001, 0x0081, 0x0005, 0x0001, 0x0001]
        );
        assert!(log.access_act(5));
        assert!(log.worldstone_open());
        assert!(!log.nihlathak_portal_open(), "portail fermé en fin de partie");
        assert!(log.game_won());
        assert_eq!(log.progression_byte(), 2);
        // Un event isolé ne franchit jamais deux étapes ni ne force une gate.
        let mut fresh = Act45Quests::new();
        fresh.on_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(phase(&fresh, "a5q5"), ActPhase::Active, "a5q5 s'active (pas de gate)");
        assert_eq!(phase(&fresh, "a5q6"), ActPhase::Inactive, "a5q6 attend a5q5 Done");
        fresh.on_event(&kill("baalcrab"));
        assert_eq!(phase(&fresh, "a5q6"), ActPhase::Inactive);
    }

    // -- Golden : JSON de save (état complet + round trip + partiel) --------------

    #[test]
    fn golden_json_save_full_run() {
        let mut log = Act45Quests::new();
        for ev in [
            zone(lvl::PLAINS_OF_DESPAIR),
            kill("izual"),
            talk("tyrael"),
            zone(lvl::CHAOS_SANCTUARY),
            kill("Infector of Souls"),
            kill("Lord De Seis"),
            kill("Grand Vizier of Chaos"),
            kill("diablo"),
            talk("tyrael"),
            item("mss"),
            zone(lvl::RIVER_OF_FLAME),
            kill("hephasto"),
            item("hfh"),
            talk("cain"),
            zone(lvl::BLOODY_FOOTHILLS),
            kill("shenk"),
            talk("larzuk"),
            zone(lvl::RESCUE_AREAS[0]),
            zone(lvl::RESCUE_AREAS[1]),
            zone(lvl::RESCUE_AREAS[2]),
            zone(lvl::RESCUE_AREAS[3]),
            talk("qual-kehk"),
            zone(lvl::ICE_CAVE_3A),
            kill("frozenstein"),
            talk("anya"),
            talk("malah"),
            talk("anya"),
            zone(lvl::HALLS_OF_VAUGHT),
            kill("nihlathak"),
            talk("anya"),
            zone(lvl::MOUNTAIN_TOP),
            kill("ancientbarb1"),
            kill("ancientbarb2"),
            kill("ancientbarb3"),
            zone(lvl::THRONE_OF_DESTRUCTION),
            kill("baalcrab"),
            talk("tyrael"),
        ] {
            log.on_event(&ev);
        }
        let v: serde_json::Value = serde_json::from_str(&log.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "version": 1,
                "quests": [
                    {"id": "a4q1", "act": 4, "phase": "done", "step": 1, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a4q2", "act": 4, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a4q3", "act": 4, "phase": "done", "step": 3, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a5q1", "act": 5, "phase": "done", "step": 1, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a5q2", "act": 5, "phase": "done", "step": 0, "count": 4, "flags": 1, "subdone": 0},
                    {"id": "a5q3", "act": 5, "phase": "done", "step": 2, "count": 1, "flags": 129, "subdone": 0},
                    {"id": "a5q4", "act": 5, "phase": "done", "step": 2, "count": 1, "flags": 5, "subdone": 0},
                    {"id": "a5q5", "act": 5, "phase": "done", "step": 1, "count": 3, "flags": 1, "subdone": 0},
                    {"id": "a5q6", "act": 5, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                ]
            })
        );
        let log2 = Act45Quests::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log.to_json(), log2.to_json());
    }

    #[test]
    fn json_round_trip_partial_states_with_subdone() {
        let mut log = Act45Quests::new();
        // a4q2 : deux gardiens sur trois.
        log.on_event(&zone(lvl::CHAOS_SANCTUARY));
        log.on_event(&kill("Infector of Souls"));
        log.on_event(&kill("Lord De Seis"));
        // a5q2 : trois extérieurs sur quatre.
        log.on_event(&zone(lvl::RESCUE_AREAS[0]));
        log.on_event(&zone(lvl::RESCUE_AREAS[1]));
        log.on_event(&zone(lvl::RESCUE_AREAS[2]));
        // a4q1 : Izual en attente de Tyrael.
        log.on_event(&zone(lvl::PLAINS_OF_DESPAIR));
        log.on_event(&kill("izual"));
        let log2 = Act45Quests::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log2.run("a4q2").unwrap().subdone, 0b011);
        assert_eq!(log2.run("a5q2").unwrap().subdone, 0b0111);
        assert_eq!(phase(&log2, "a4q1"), ActPhase::RewardPending);
        // Le rechargé poursuit exactement.
        let mut log = log2;
        log.on_event(&talk("tyrael"));
        assert_eq!(phase(&log, "a4q1"), ActPhase::Done);
        log.on_event(&kill("Grand Vizier of Chaos"));
        assert_eq!(phase(&log, "a4q2"), ActPhase::Active, "diablo reste à tuer");
        log.on_event(&zone(lvl::RESCUE_AREAS[3]));
        assert_eq!(phase(&log, "a5q2"), ActPhase::RewardPending);
    }

    #[test]
    fn json_rejects_corrupted_states() {
        let base = Act45Quests::new();
        // Version inconnue / JSON invalide / mauvaise longueur.
        assert!(Act45Quests::from_json(r#"{"version":9,"quests":[]}"#).is_err());
        assert!(Act45Quests::from_json("{oops").is_err());
        assert!(Act45Quests::from_json(r#"{"version":1,"quests":[]}"#).is_err());
        // Id inconnu (slot 1 = a4q2 DIABLO, PAS a4q2 « Forge » wiki — ordre
        // de sauvegarde acte IV).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1]["id"] = "zz".into();
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // Acte incohérent avec le slot.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][4]["act"] = serde_json::json!(4);
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // Phase incohérente : Done sans bit 0.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][0] =
            serde_json::json!({"id":"a4q1","act":4,"phase":"done","step":1,"count":1,"flags":0,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // RewardPending sans PNJ de remise (a5q5/a5q6 n'en ont pas).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][7] =
            serde_json::json!({"id":"a5q5","act":5,"phase":"reward_pending","step":1,"count":3,"flags":2,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // subdone fantaisiste : bits hors bornes (a4q2 a 3 alternatives).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1] =
            serde_json::json!({"id":"a4q2","act":4,"phase":"active","step":1,"count":2,"flags":0,"subdone":0b1111});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // subdone ≠ popcount(count) (2 bits posés, count=1).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1] =
            serde_json::json!({"id":"a4q2","act":4,"phase":"active","step":1,"count":1,"flags":0,"subdone":0b011});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // Gate bafouée : a5q4 Active alors que a5q3 est Inactive.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][6] =
            serde_json::json!({"id":"a5q4","act":5,"phase":"active","step":0,"count":0,"flags":0,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // idem a5q6 sans a5q5.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][8] =
            serde_json::json!({"id":"a5q6","act":5,"phase":"active","step":0,"count":0,"flags":0,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // Bit 7 hors a5q3 : a5q1 « Done » avec le parchemin consommé ⇒ non.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][3] =
            serde_json::json!({"id":"a5q1","act":5,"phase":"done","step":1,"count":1,"flags":0x0081,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // a5q3 RewardPending (pas encore Done) avec bit 7 ⇒ non.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][5] =
            serde_json::json!({"id":"a5q3","act":5,"phase":"reward_pending","step":2,"count":1,"flags":0x0082,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // Inactive avec résidus.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][0] =
            serde_json::json!({"id":"a4q1","act":4,"phase":"inactive","step":1,"count":1,"flags":2,"subdone":0});
        assert!(Act45Quests::from_json(&v.to_string()).is_err());
        // Contrôle positif : la base propre passe.
        assert!(Act45Quests::from_json(&base.to_json()).is_ok());
    }

    // -- Authoring : invariants de slots (ordre de sauvegarde !) ------------------

    #[test]
    fn authoring_slot_order_is_save_order_not_journal_order() {
        // ACTE IV : a4q2 = Diablo (wiki « A4Q3 »), a4q3 = Forge (wiki
        // « A4Q2 ») — mapping qstsa4q* string.tsv:937-939 (spec §5).
        assert_eq!(ACT4_QUEST_IDS, ["a4q1", "a4q2", "a4q3"]);
        assert_eq!(DEFS_ACT4[0].name, "The Fallen Angel");
        assert_eq!(DEFS_ACT4[1].name, "Terror's End");
        assert_eq!(DEFS_ACT4[2].name, "Hell's Forge");
        assert_eq!(ACT5_QUEST_IDS, ["a5q1", "a5q2", "a5q3", "a5q4", "a5q5", "a5q6"]);
        assert_eq!(DEFS_ACT5[5].name, "Eve of Destruction");
        let log = Act45Quests::new();
        assert_eq!(log.run("a4q2").map(|r| r.phase), Some(ActPhase::Inactive));
        // Gates : a5q4 attend a5q3, a5q6 attend a5q5.
        assert_eq!(DEFS_ACT5[3].requires_done, Some(slot::A5Q3));
        assert_eq!(DEFS_ACT5[5].requires_done, Some(slot::A5Q5));
        // Une seule porteuse d'acte (UnlockAccessAct(5)) : a4q2.
        assert_eq!(
            DEFS_ACT4.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, Act45Reward::UnlockAccessAct(5))).count(),
            1
        );
        assert_eq!(
            DEFS_ACT5.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, Act45Reward::UnlockAccessAct(_))).count(),
            0
        );
        // Deux personnisations Anya (a5q3 + a5q4) [FIDÈLE].
        assert_eq!(
            DEFS_ACT5.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, Act45Reward::AnyaPersonalize)).count(),
            2
        );
        // Aucun quête A5 ne débloque un « acte 6 » : la fin = GameWon.
        // Ids `patch_levels.txt` documentés (re-vérifiés 2026-10-24).
        assert_eq!(
            (lvl::PLAINS_OF_DESPAIR, lvl::RIVER_OF_FLAME, lvl::CHAOS_SANCTUARY),
            (105, 107, 108)
        );
        assert_eq!(
            (lvl::BLOODY_FOOTHILLS, lvl::ICE_CAVE_3A, lvl::MOUNTAIN_TOP, lvl::HALLS_OF_VAUGHT),
            (110, 119, 120, 124)
        );
        assert_eq!((lvl::THRONE_OF_DESTRUCTION, lvl::WORLDSTONE_CHAMBER), (131, 132));
        assert_eq!(lvl::RESCUE_AREAS, [110, 111, 112, 117]);
    }

    // -- BOSS : données réelles des tables (patch_monstats + MonLvl) --------------

    #[test]
    fn boss_stats45_golden_real_tables() {
        // Izual — patch_monstats (`izual`, Level 29 N, HP 9481, AC 84,
        // Exp 3695, TC « Izual », Res [30,30,30,30,75,30]) × MonLvl ligne
        // 29 N (AC 174, TH 298, HP 126 %, DM 19 %, XP 616 %).
        let i = boss_stats45("izual").expect("izual présent des tables");
        assert_eq!(i.rec.code, "22");
        assert_eq!(i.level, 29);
        assert_eq!(i.hp, 9481);
        assert_eq!(i.ac, 84);
        assert_eq!(i.exp, 3695);
        assert_eq!(i.res, [30, 30, 30, 30, 75, 30], "résists réels Izual N");
        assert_eq!(i.rule.kind, crate::MonKind::SuperUnique);
        assert_eq!(i.rule.hp_pct, 126);
        assert_eq!(i.rule.dmg_pct, 19);
        assert_eq!(i.rule.th_pct, 298);
        assert_eq!(i.rule.xp_pct, 616);
        assert_eq!(i.rule.ac, 174);
        assert_eq!(i.rule.treasure_class.as_deref(), Some("Izual"));
        assert_eq!(i.rule.res, [30, 30, 30, 30, 75, 30]);
        // Hephasto — Level 25 N, HP 760, AC 107, Exp 339, TC « Haphesto » ×
        // MonLvl ligne 25 N (AC 150, TH 254, HP 100 %, DM 16 %, XP 470 %).
        let h = boss_stats45("5P").expect("hephasto par code sprite");
        assert_eq!(h.rec.id, "hephasto");
        assert_eq!(h.level, 25);
        assert_eq!(h.hp, 760);
        assert_eq!(h.exp, 339);
        assert_eq!(h.rule.treasure_class.as_deref(), Some("Haphesto"));
        assert_eq!(h.rule.hp_pct, 100);
        assert_eq!(h.rule.ac, 150);
        // Ancien 1 — Level 37 N, HP 500-600 (minHP lu), AC 84, Exp vide,
        // TC VIDE (pas de drop dédié) × MonLvl ligne 37 N (AC 222, TH 386,
        // HP 190 %, DM 23 %, XP 1059 %).
        let a = boss_stats45("0D").expect("ancient 1 par code sprite");
        assert_eq!(a.rec.id, "ancientbarb1");
        assert_eq!(a.level, 37);
        assert_eq!(a.hp, 500);
        assert_eq!(a.exp, 0, "Exp vide dans patch_monstats");
        assert_eq!(a.rule.treasure_class, None, "colonne TC vide = pas de drop dédié");
        assert_eq!(a.rule.hp_pct, 190);
        assert_eq!(a.rule.ac, 222);
        // Lookup insensible à la casse + hors scope → None.
        assert_eq!(boss_stats45("IZUAL").unwrap().level, 29);
        assert!(boss_stats45("diablo").is_none(), "Diablo = quests_finale.rs");
        assert!(boss_stats45("baalcrab").is_none(), "Baal = quests_finale.rs");
        assert!(boss_stats45("nihlathakboss").is_none(), "superunique = chemin data-driven d2game");
        assert!(boss_stats45("frozenstein").is_none());
        assert!(boss_stats45("zz").is_none());
        // Records statiques : lairs et slots porteurs.
        assert_eq!(BOSSES_ACTS45[0].lair, lvl::PLAINS_OF_DESPAIR);
        assert_eq!(BOSSES_ACTS45[0].quest_slot, 0);
        assert_eq!(BOSSES_ACTS45[1].lair, lvl::RIVER_OF_FLAME);
        assert_eq!(BOSSES_ACTS45[1].quest_slot, 2);
        assert!(BOSSES_ACTS45[2..].iter().all(|b| b.lair == lvl::MOUNTAIN_TOP && b.quest_slot == 7));
    }

    #[test]
    fn scripted_bosses_trigger_matches_zone_steps_only() {
        // a4q1 : Zone(105) à l'activation → Izual ; Zone(104) non ; après le
        // kill (étape finie) : plus de re-spawn.
        let mut log = Act45Quests::new();
        assert_eq!(scripted_bosses45_for_trigger(&log, &zone(lvl::PLAINS_OF_DESPAIR)).len(), 1);
        assert!(scripted_bosses45_for_trigger(&log, &zone(104)).is_empty());
        log.on_event(&zone(lvl::PLAINS_OF_DESPAIR));
        log.on_event(&kill("izual"));
        assert!(scripted_bosses45_for_trigger(&log, &zone(lvl::PLAINS_OF_DESPAIR)).is_empty());
        // a5q5 : Zone(120) à l'activation → les 3 ANCIENS d'un coup ; après
        // les 3 kills, plus rien (étape Any consommée + Done).
        let log2 = Act45Quests::new();
        let spawns = scripted_bosses45_for_trigger(&log2, &zone(lvl::MOUNTAIN_TOP));
        assert_eq!(spawns.len(), 3, "les 3 Anciens à l'Autel des Cieux");
        assert!(spawns.iter().all(|(c, _)| ["0D", "0F", "0E"].contains(c)));
        // Re-entrée à l'étape des kills (reload) : re-spawn possible — la
        // garde « vivant » du World évite les doublons.
        let mut log3 = Act45Quests::new();
        log3.on_event(&zone(lvl::MOUNTAIN_TOP));
        log3.on_event(&kill("ancientbarb1"));
        assert_eq!(
            scripted_bosses45_for_trigger(&log3, &zone(lvl::MOUNTAIN_TOP)).len(),
            3,
            "re-spawn à l'étape kills (garde vivant côté World)"
        );
        // Hephasto : PAS avant la pierre d'âme (étape 0 = Item mss).
        let mut log4 = Act45Quests::new();
        assert!(scripted_bosses45_for_trigger(&log4, &zone(lvl::RIVER_OF_FLAME)).is_empty());
        log4.on_event(&item("mss"));
        assert_eq!(scripted_bosses45_for_trigger(&log4, &zone(lvl::RIVER_OF_FLAME)).len(), 1);
    }

    // -- Intégration World : spawns scriptés + récompenses + GameWon -------------

    use crate::{GameEvent, MonKind, World};

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    /// Manifest réel + Izual/Hephasto/3 Anciens (valeurs réelles
    /// patch_monstats) — STUBS de grille : DRLG A4-A5 pas fusionnée.
    fn manifest_bosses45() -> serde_json::Value {
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([
            {"code": "22", "name": "Izual", "stats": {
                "level": 29, "hpMin": 9481, "hp": 9481,
                "dmgMin": 250, "dmgMax": 350, "exp": 3695,
                "speed": 4.0, "treasureClass": "Izual"}},
            {"code": "5P", "name": "Hephasto", "stats": {
                "level": 25, "hpMin": 760, "hp": 760,
                "dmgMin": 150, "dmgMax": 210, "exp": 339,
                "speed": 4.0, "treasureClass": "Haphesto"}},
            {"code": "0D", "name": "Ancient Barbarian 1", "stats": {
                "level": 37, "hpMin": 500, "hp": 500,
                "dmgMin": 140, "dmgMax": 180, "exp": 0,
                "speed": 4.0}},
            {"code": "0F", "name": "Ancient Barbarian 2", "stats": {
                "level": 37, "hpMin": 500, "hp": 500,
                "dmgMin": 140, "dmgMax": 180, "exp": 0,
                "speed": 4.0}},
            {"code": "0E", "name": "Ancient Barbarian 3", "stats": {
                "level": 37, "hpMin": 500, "hp": 500,
                "dmgMin": 140, "dmgMax": 180, "exp": 0,
                "speed": 4.0}},
        ]);
        m
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    #[test]
    fn world_spawns_scripted_izual_at_despair_with_real_stats() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses45(), 0xD2_45);
        w.set_quest_actes45(Act45Quests::new());
        assert!(w.monsters().is_empty(), "manifest sans monstres de planche");
        // L'entrée des Plaines du Désespoir (id 105) fait apparaître Izual.
        let rewards = w.quest_actes45_event(&zone(lvl::PLAINS_OF_DESPAIR));
        assert!(rewards.is_empty());
        assert_eq!(w.monsters().len(), 1, "Izual spawné au trigger zone");
        assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
        // HP effectifs = 9481 × 126 % (ligne MonLvl 29 N) = 11946.
        assert_eq!(w.monster_hp(0), Some((11946, 11946)));
        // Re-trigger : pas de doublon (garde vivant).
        w.quest_actes45_event(&zone(lvl::PLAINS_OF_DESPAIR));
        assert_eq!(w.monsters().len(), 1);
        // Kill → Tyrael : +2 points de skill + acte rien (a4q1 sans unlock).
        let r = w.quest_actes45_event(&kill("izual"));
        assert!(r.is_empty());
        w.quest_actes45_event(&talk("tyrael"));
        // +2 de Tyrael [CITÉ] ; le bonus XP de 1500 peut en ajouter (paliers
        // de niveau, C2 +1/niveau) — on ne verrouille que la récompense.
        assert!(w.unspent_skill_points() >= 2, "Tyrael +2 skills");
    }

    #[test]
    fn world_spawns_three_ancients_and_completes_rite() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses45(), 0xD2_45);
        w.set_quest_actes45(Act45Quests::new());
        // Gate retail 19b : le héros est au niveau minimal Normal (20) —
        // l'autel des Cieux répond (le niveau 19 est testé en golden).
        w.players[0].data.stats.level = 20;
        // Entrée Sommet d'Arreat : les 3 Anciens apparaissent (activation +
        // spawns même event).
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(w.monsters().len(), 3, "les 3 Anciens spawnés à l'Autel");
        // HP effectifs = 500 × 190 % (ligne MonLvl 37 N) = 950.
        assert_eq!(w.monster_hp(0), Some((950, 950)));
        assert_eq!(w.monster_kind(2), Some(MonKind::SuperUnique));
        // Les 3 kills complètent le Rite (XP directe, pas de PNJ).
        w.quest_actes45_event(&kill("ancientbarb1"));
        w.quest_actes45_event(&kill("ancientbarb2"));
        let r = w.quest_actes45_event(&kill("ancientbarb3"));
        assert_eq!(r, vec![Act45Reward::Xp(4000)]);
        assert!(w.quest_actes45().unwrap().worldstone_open());
    }

    #[test]
    fn world_events_act_unlock_and_game_won_are_emitted() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses45(), 0xD2_45);
        w.set_quest_actes45(Act45Quests::new());
        // Gate retail 19b : niveau minimal Normal (l'autel exige 20).
        w.players[0].data.stats.level = 20;
        // A4Q2 complète (sceaux + Diablo + Tyrael) : ActUnlocked(5).
        w.quest_actes45_event(&zone(lvl::CHAOS_SANCTUARY));
        for g in ["Lord De Seis", "Infector of Souls", "Grand Vizier of Chaos"] {
            w.quest_actes45_event(&kill(g));
        }
        w.quest_actes45_event(&kill("diablo"));
        w.quest_actes45_event(&talk("tyrael"));
        // A5Q5 puis A5Q6 : GameWon.
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        for b in ["ancientbarb1", "ancientbarb2", "ancientbarb3"] {
            w.quest_actes45_event(&kill(b));
        }
        w.quest_actes45_event(&zone(lvl::THRONE_OF_DESTRUCTION));
        w.quest_actes45_event(&kill("baalcrab"));
        w.quest_actes45_event(&talk("tyrael"));
        // Les events sont flushés au PROCHAIN tick (contrat d2rules).
        let ev = w.tick(10, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 5 })),
            "ActUnlocked(5) émis au portail de Tyrael"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::GameWon)),
            "GameWon émis à la fin de jeu (a5q6)"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::ActQuestChanged { id, .. } if id == "a5q6")),
            "ActQuestChanged a5q6 émis"
        );
        assert!(w.quest_actes45().unwrap().game_won());
    }

    #[test]
    fn world_applies_item_and_skill_rewards_of_acts45() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_bosses45(), 0xD2_45);
        w.set_quest_actes45(Act45Quests::new());
        // a4q1 Izual → Tyrael : +2 skill points (le bonus XP de 1500 peut
        // faire monter des niveaux — C2 +1/niveau — on verrouille la part
        // récompense).
        w.quest_actes45_event(&zone(lvl::PLAINS_OF_DESPAIR));
        w.quest_actes45_event(&kill("izual"));
        w.quest_actes45_event(&talk("tyrael"));
        assert!(w.unspent_skill_points() >= 2, "Tyrael +2 skills [CITÉ]");
        // a4q3 Forge → Cain : 3 gemmes parfaites dans l'inventaire.
        for ev in [item("mss"), zone(lvl::RIVER_OF_FLAME), kill("hephasto"), item("hfh"), talk("cain")] {
            w.quest_actes45_event(&ev);
        }
        let inv = w.inventory();
        for gem in ["gpv", "gpy", "gpb"] {
            assert!(
                inv.iter().any(|s| s.as_ref().map_or(false, |d| d.code == gem)),
                "gemme {gem} dans l'inventaire"
            );
        }
        // a5q2 → Qual-Kehk : runes Ral+Ort+Tal dans l'inventaire.
        for id in lvl::RESCUE_AREAS {
            w.quest_actes45_event(&zone(id));
        }
        w.quest_actes45_event(&talk("qual-kehk"));
        let inv = w.inventory();
        for rune in ["r07", "r08", "r09"] {
            assert!(
                inv.iter().any(|s| s.as_ref().map_or(false, |d| d.code == rune)),
                "rune {rune} dans l'inventaire"
            );
        }
    }

    #[test]
    fn world_save_roundtrip_keeps_acts45_journal() {
        // Manifest SANS boss spawnables : le journal avance sans spawn.
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2_45);
        w.set_quest_actes45(Act45Quests::new());
        w.quest_actes45_event(&zone(lvl::PLAINS_OF_DESPAIR));
        w.quest_actes45_event(&kill("izual"));
        w.quest_actes45_event(&talk("tyrael"));
        let json = w.save();
        // Rétro-compat : champ quest_actes45 ABSENT → charge (journal vide).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("quest_actes45").is_some(), "le champ additif est présent dans la save fraîche");
        v.as_object_mut().unwrap().remove("quest_actes45");
        let w2 = World::load(&v.to_string()).unwrap();
        assert!(w2.quest_actes45().is_none(), "sans champ : PAS de journal branché");
        // Round trip exact : re-save identique.
        let w3 = World::load(&json).unwrap();
        let log3 = w3.quest_actes45().unwrap();
        assert_eq!(log3.run("a4q1").unwrap().phase, ActPhase::Done);
        assert_eq!(log3.to_json(), w.quest_actes45().unwrap().to_json());
        assert_eq!(w3.save(), json, "re-save identique");
        // Journal PRÉSENT mais invalide ⇒ Err (contrat strict).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["quest_actes45"] = serde_json::json!({"version": 99, "quests": []});
        assert!(World::load(&v.to_string()).is_err());
        // Sans journal branché : no-op.
        let mut w4 = World::with_seed(open_grid(8, 8), manifest_bosses45(), 7);
        assert!(w4.quest_actes45_event(&kill("izual")).is_empty());
        assert!(w4.quest_actes45().is_none());
    }

    #[test]
    fn world_no_monster_tpl_is_a_clean_no_spawn() {
        // Manifest SANS bloc boss « 22 » : le trigger zone avance quand même
        // la quête (le spawn échoue proprement, jamais de panique).
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2_45);
        w.set_quest_actes45(Act45Quests::new());
        w.quest_actes45_event(&zone(lvl::PLAINS_OF_DESPAIR));
        assert!(w.monsters().is_empty(), "pas de gabarit 22 : pas de spawn");
        assert_eq!(
            w.quest_actes45().unwrap().run("a4q1").unwrap().step,
            1,
            "la progression de quête n'est pas bloquée"
        );
    }

    // -------------------------------------------------------------------------
    // Étape 19b — interop .d2s (PLAN-FIDÉLITÉ phase 19) : récompenses CHIFFRÉES
    // citées par le README de l'oracle d2s-file-generator (GPL : valeurs de
    // format = faits, réimplémentation propre), gate niveau des Anciens
    // 20/40/60, chaîne des 3 difficultés, save additive rétro-compatible.
    // -------------------------------------------------------------------------

    use crate::Difficulty;

    /// Golden 19a — oracle d2s : « +4 skill points for each completed
    /// difficulty ("Den of Evil", "Radament's Lair" and "The Fallen Angel"),
    /// +5 stats points ("Lam Esen's Tome"), +20 life ("The Golden Bird"),
    /// +10% to all resistances ("Prison of Ice") ». Les trois journaux
    /// attribuent EXACTEMENT ces valeurs (1 + 1 + 2 = 4 SP par difficulté) ;
    /// l'or de stash interop = 2 500 000 (confirmé par le vrai .d2s javazon :
    /// goldbank = 2 500 000, test d2d2s).
    #[test]
    fn golden_interop_oracle_cited_rewards_per_difficulty() {
        use crate::quests_actes::{ActReward, DEFS_ACT1};
        use crate::quests_actes2::{Act2Reward, DEFS_ACT2, DEFS_ACT3};
        // +4 SP par difficulté = Den of Evil 1 + Radament 1 + Izual 2.
        let sp1 = |rs: &[ActReward]| {
            rs.iter()
                .filter_map(|r| match r {
                    ActReward::SkillPoint(n) => Some(*n),
                    _ => None,
                })
                .sum::<u32>()
        };
        let sp23 = |rs: &[Act2Reward]| {
            rs.iter()
                .filter_map(|r| match r {
                    Act2Reward::SkillPoint(n) => Some(*n),
                    _ => None,
                })
                .sum::<u32>()
        };
        let sp45 = |rs: &[Act45Reward]| {
            rs.iter()
                .filter_map(|r| match r {
                    Act45Reward::SkillPoint(n) => Some(*n),
                    _ => None,
                })
                .sum::<u32>()
        };
        assert_eq!(sp1(DEFS_ACT1[0].rewards), 1, "Den of Evil (A1Q1)");
        assert_eq!(sp23(DEFS_ACT2[0].rewards), 1, "Radament's Lair (A2Q1)");
        assert_eq!(sp45(DEFS_ACT4[0].rewards), 2, "The Fallen Angel (A4Q1)");
        assert_eq!(
            sp1(DEFS_ACT1[0].rewards) + sp23(DEFS_ACT2[0].rewards) + sp45(DEFS_ACT4[0].rewards),
            4,
            "oracle d2s : +4 skill points par difficulté complétée"
        );
        // +5 stats (Lam Esen — slot 0 acte III = clé de save a3q1).
        let stats = |rs: &[Act2Reward]| {
            rs.iter()
                .filter_map(|r| match r {
                    Act2Reward::StatPoint(n) => Some(*n),
                    _ => None,
                })
                .sum::<u32>()
        };
        assert_eq!(stats(DEFS_ACT3[0].rewards), 5, "Lam Esen's Tome (+5 stats)");
        // +20 vie (Oiseau de Feu — slot 3 acte III = clé de save a3q4).
        let life = |rs: &[Act2Reward]| {
            rs.iter()
                .filter_map(|r| match r {
                    Act2Reward::MaxLife(n) => Some(*n),
                    _ => None,
                })
                .sum::<u32>()
        };
        assert_eq!(life(DEFS_ACT3[3].rewards), 20, "The Golden Bird (+20 vie)");
        // +10 % résists tout (Prison de glace — slot 5 du journal = a5q3).
        let res = DEFS_ACT5[2]
            .rewards
            .iter()
            .filter_map(|r| match r {
                Act45Reward::ResistAll(n) => Some(*n),
                _ => None,
            })
            .sum::<u32>();
        assert_eq!(res, 10, "Prison of Ice (+10 % résists tout)");
        // Cap d'or stash interop .d2s = 2 500 000, identique sur les 3
        // difficultés (README oracle ; economy.rs aligné étape 19b).
        assert_eq!(crate::economy::stash_gold_cap(Difficulty::Normal), 2_500_000);
        assert_eq!(crate::economy::stash_gold_cap(Difficulty::Nightmare), 2_500_000);
        assert_eq!(crate::economy::stash_gold_cap(Difficulty::Hell), 2_500_000);
    }

    /// Golden 19b — gate retail du Rite de Passage : niveau minimal POUR
    /// FAIRE la quête des Anciens (oracle : contrainte de fin de difficulté)
    /// — 20 Normal / 40 Cauchemar / 60 Enfer. Sous le seuil : ni activation
    /// (zone 120), ni kills. Journal pur (sans contexte) : jamais bloqué.
    #[test]
    fn golden_a5q5_level_gate_oracle_20_40_60() {
        assert_eq!(ANCIENTS_MIN_LEVELS, [20, 40, 60]);
        assert_eq!(Act45Quests::ancients_min_level(0), 20);
        assert_eq!(Act45Quests::ancients_min_level(1), 40);
        assert_eq!(Act45Quests::ancients_min_level(2), 60);
        assert!(!Act45Quests::new().ancients_gate_blocked(), "journal pur : pas de contexte, pas de gate");
        // Normal : 19 bloqué (zone ET kills), 20 passe.
        let mut log = Act45Quests::new();
        log.set_level_gate(Some(19), 0);
        assert!(log.ancients_gate_blocked());
        assert!(rewards_of(&mut log, &zone(lvl::MOUNTAIN_TOP)).is_empty());
        assert_eq!(phase(&log, "a5q5"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &kill("ancientbarb1")).is_empty());
        log.set_level_gate(Some(20), 0);
        assert!(!log.ancients_gate_blocked());
        assert!(rewards_of(&mut log, &zone(lvl::MOUNTAIN_TOP)).is_empty());
        assert_eq!(phase(&log, "a5q5"), ActPhase::Active);
        // Cauchemar : 39 bloqué, 40 passe.
        let mut nm = Act45Quests::new();
        nm.set_level_gate(Some(39), 1);
        assert!(nm.ancients_gate_blocked());
        nm.on_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(phase(&nm, "a5q5"), ActPhase::Inactive);
        nm.set_level_gate(Some(40), 1);
        assert!(!nm.ancients_gate_blocked());
        // Enfer : 59 bloqué, 60 passe.
        let mut hell = Act45Quests::new();
        hell.set_level_gate(Some(59), 2);
        assert!(hell.ancients_gate_blocked());
        hell.set_level_gate(Some(60), 2);
        assert!(!hell.ancients_gate_blocked());
    }

    /// Golden 19c — chaîne des 3 difficultés dans un même monde : la gate
    /// suit la difficulté courante (20/40/60) et la Prison de glace
    /// s'additionne (+10 % par difficulté → +30) sur `HeroStats::res_all_pct`
    /// (oracle : « for each completed difficulty »).
    #[test]
    fn golden_world_three_difficulties_chain_gates_and_resists() {
        let mut m = manifest_bosses45();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(16, 16), m, 0x19B_01);
        w.set_quest_actes45(Act45Quests::new());
        // NORMAL — 19 : l'autel reste muet ; 20 : le Rite s'active.
        w.players[0].data.stats.level = 19;
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert!(w.monsters().is_empty(), "19 < 20 : aucun Ancien");
        assert_eq!(w.quest_actes45().unwrap().run("a5q5").unwrap().phase, ActPhase::Inactive);
        w.players[0].data.stats.level = 20;
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(w.quest_actes45().unwrap().run("a5q5").unwrap().phase, ActPhase::Active);
        // Prison de glace Normal : Malah → +10 %.
        for ev in [zone(lvl::ICE_CAVE_3A), kill("frozenstein"), talk("anya"), talk("malah")] {
            w.quest_actes45_event(&ev);
        }
        assert_eq!(w.hero_stats().res_all_pct, 10);
        // CAUCHEMAR — journal neuf (une course par difficulté) : 39 bloqué,
        // 40 passe ; a5q3 → +20 cumulé.
        w.set_difficulty(Difficulty::Nightmare);
        w.set_quest_actes45(Act45Quests::new());
        w.players[0].data.stats.level = 39;
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(w.quest_actes45().unwrap().run("a5q5").unwrap().phase, ActPhase::Inactive, "NM : 39 < 40");
        w.players[0].data.stats.level = 40;
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(w.quest_actes45().unwrap().run("a5q5").unwrap().phase, ActPhase::Active);
        for ev in [zone(lvl::ICE_CAVE_3A), kill("frozenstein"), talk("anya"), talk("malah")] {
            w.quest_actes45_event(&ev);
        }
        assert_eq!(w.hero_stats().res_all_pct, 20, "chaque difficulté ajoute +10 %");
        // ENFER — 59 bloqué, 60 passe ; a5q3 → +30 cumulé.
        w.set_difficulty(Difficulty::Hell);
        w.set_quest_actes45(Act45Quests::new());
        w.players[0].data.stats.level = 59;
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(w.quest_actes45().unwrap().run("a5q5").unwrap().phase, ActPhase::Inactive, "Enfer : 59 < 60");
        w.players[0].data.stats.level = 60;
        w.quest_actes45_event(&zone(lvl::MOUNTAIN_TOP));
        assert_eq!(w.quest_actes45().unwrap().run("a5q5").unwrap().phase, ActPhase::Active);
        for ev in [zone(lvl::ICE_CAVE_3A), kill("frozenstein"), talk("anya"), talk("malah")] {
            w.quest_actes45_event(&ev);
        }
        assert_eq!(w.hero_stats().res_all_pct, 30, "Normal + NM + Enfer = +30 % résists");
    }

    /// Golden 19d — save additive rétro-compatible : `res_all_pct` ABSENT
    /// (saves antérieures à 19b) charge à 0 ; présent, traverse le round
    /// trip (+10). La gate 19b n'est PAS sérialisée (contexte runtime —
    /// `from_json` recharge sans contexte, cf. `ancients_gate_blocked`).
    #[test]
    fn golden_world_save_res_all_pct_retro_compatible() {
        let mut m = manifest_bosses45();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(16, 16), m, 0x19B_02);
        w.set_quest_actes45(Act45Quests::new());
        w.players[0].data.stats.level = 20;
        for ev in [zone(lvl::ICE_CAVE_3A), kill("frozenstein"), talk("anya"), talk("malah")] {
            w.quest_actes45_event(&ev);
        }
        assert_eq!(w.hero_stats().res_all_pct, 10);
        let json = w.save();
        // Rétro-compat : save SANS le champ ⇒ charge à 0 (jamais d'erreur).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["hero"]["stats"].as_object_mut().unwrap().remove("res_all_pct");
        v.as_object_mut().unwrap().remove("players"); // 3b-1 : rétro-compat additive
        let old = World::load(&v.to_string()).expect("save sans res_all_pct");
        assert_eq!(old.hero_stats().res_all_pct, 0);
        // Round trip : la save fraîche porte le bonus.
        let back = World::load(&json).expect("save 19b");
        assert_eq!(back.hero_stats().res_all_pct, 10);
        // La gate n'apparaît nulle part dans la save (contexte runtime).
        assert!(!json.contains("gate_level"), "contexte de gate jamais sérialisé");
    }
}
