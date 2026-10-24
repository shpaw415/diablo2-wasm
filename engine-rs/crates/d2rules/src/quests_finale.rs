//! Moteur de quêtes FINALES A4-A5 — fin du jeu (campagne diablo-2-wasm,
//! étape 10b). Implémentation de la spec `d2w-re/specs/quests-actes.md` §5/§6.
//!
//! Même pattern que [`crate::quests_actes2`] (actes II-III — LIS-LE) : events
//! d'entrée [`ActEvent`] RÉUTILISÉS (aucun nouveau type d'event), machine à
//! phases [`ActPhase`], flags u16 façon .d2s (bits identiques), save JSON
//! versionnée strict, 100 % pur. Le bloc `impl World` additif en bas fournit
//! le branchement + la save (`quest_finale`, champ `#[serde(default)]` — save
//! v3 inchangée) et le spawn scripté des boss finaux (stats runtime tables).
//!
//! # Portée (mission étape 10b — « boss finaux »)
//! 5 quêtes = les clés de sauvegarde .d2s portées par CE module :
//! * ACTE IV (u16 @395/397/399 Normal) : `a4q1` **Izual** (L'Ange déchu,
//!   slot 1), `a4q2` **Diablo** (Epilogue de la terreur, slot 2),
//!   `a4q3` **Hephasto** (la Forge de l'Enfer, slot 3) — ORDRE DE
//!   SAUVEGARDE ≠ ordre du journal wiki (spec §5, même piège que l'acte III).
//! * ACTE V (u16 @423/425 Normal) : `a5q5` **les Anciens** (Rite of
//!   Passage, slot 5), `a5q6` **Baal** (Le Crépuscule de la Destruction,
//!   slot 6). Les quêtes de ville a5q1-a5q4 (Shenk, sauvetage, Anya,
//!   Nihlathak) sont HORS SCOPE de l'étape 10b (le record `nihlathakboss`
//!   existe bien, `patch_monstats.txt:529`, mais sa chaîne est portée
//!   ailleurs) — cf. spec §6.
//!
//! # Chaîne finale (mission)
//! * **Diablo** : record monstats `diablo` (`patch_monstats.txt:245`,
//!   Level 40 N, HP 6427, AC 87, Exp 3462, résists `[0,0,33,33,33,50]`, TC
//!   `Diablo` — TreasureClassEx.txt:695-700 « Diablo/Diabloq » [VÉRIFIÉ-
//!   LOCAL]), spawn scripté à l'entrée du **Sanctuaire du Chaos = id 108**
//!   (`patch_levels.txt:110`, « Act 4 - Diablo 1 »). ⚠ CORRECTION DONNÉES :
//!   la mission annonçait « Chaos Sanctuary id 83 » — le id 83 est
//!   **Travincal** (acte III, `patch_levels.txt:85`, déjà consommé par a3q2/
//!   a3q5 de [`crate::quests_actes2`]) ; le id réel du Sanctuaire du Chaos
//!   est 108, corroboré par la spec §5 (« Chaos Sanctum = id 108 ») — les
//!   DONNÉES gagnent ([`crate::quests_actes2`] : re-vérification awk/python
//!   contre les tables avant de conclure). Izual (Plaine du Désespoir
//!   **id 105** :107) et Hephasto (Rivière de Feu **id 107** :109) sont
//!   traités comme les 2 autres étapes a4.
//! * **Baal** : chaîne complète du trône — 5 VAGUES réelles « Baal Subject
//!   1-5 » (`superuniques.txt:64-68`, classes `fallenshaman5`/`unraveler5`/
//!   `baalhighpriest`/`venomlord`/`baalminion1`, TC superunique « Act 5
//!   Champ C » :536 [VÉRIFIÉ-LOCAL]) spawnées SÉQUENTIELLEMENT dans le
//!   Trône de la Destruction (**id 131**, QuestFlag 39, `patch_levels.txt:
//!   134`) : les données le PERMETTENT (superuniques nommés + niveau réel),
//!   donc PAS de vagues codées par listes — cf. [`finale_spawns_for_trigger`]
//!   (l'entrée du trône lance la vague 1 ; chaque kill de superunique lance
//!   la suivante [ARRANGÉ-MVP : le pack MinGrp/MaxGrp=5 des données est
//!   réduit au superunique porteur]). Puis **baalcrab** (Baal lui-même,
//!   `patch_monstats.txt:547`, Level 60 N, HP 6336, TC `Baal` —
//!   TreasureClassEx.txt:722-727 [VÉRIFIÉ-LOCAL]) spawné à l'entrée de la
//!   Chambre de la Pierre Mondiale (**id 132** :135) ; **la mort de Baal =
//!   [`GameEvent::GameWon`]** (variante additive, écran victoire).
//! * **ANCIENS** : les 3 superuniques réels sont « Ancient Barbarian 1/2/3 »
//!   (`superuniques.txt:46-48`, classes `ancientbarb1/2/3`, codes `0D/0F/
//!   0E` ; les noms FR retail Korlic/Madawc/Talic ne sont PAS dans la table
//!   v1.12 — [VÉRIFIÉ-LOCAL]), au Sommet d'Arreat (**id 120** « Mountain
//!   Top » :123). TC **vide** dans les deux tables (pas de drop dédié) et
//!   colonne `Exp` **vide** (le XP vient du % MonLvl du gabarit ; la
//!   récompense XP de quête est [INVENTÉ]). Leur victoire ouvre la porte
//!   Worldstone ([`QuestsFinale::worldstone_open`], gâteau d'a5q6).
//!
//! # Récompenses / gating (mission point 3)
//! * A4Q2 (Diablo) → remise Tyrael → **[`GameEvent::ActUnlocked`]{ act: 5 }**
//!   (accès acte V, spec §5 « accès acte V via Tyrael [FIDÈLE] ») + TC
//!   finale `Diablo` portée par la règle de spawn du boss.
//! * A4Q1 → Tyrael → **+2 points de compétence** [CITÉ spec §5] ; A4Q3 →
//!   Cain → gemme `gcv` [ARRANGÉ-MVP : la TC gemmes retail est absente de
//!   TreasureClassEx, cf. spec §5].
//! * A5Q6 → kill Baal → **[`FinaleReward::GameWon`]** → le World émet
//!   [`GameEvent::GameWon`] (variante GameEvent ADDITIVE, documentée lib.rs).
//!   La remise retail Tyrael (titre/portail de fin) est [ARRANGÉ-MVP] :
//!   l'écran victoire est porté par l'event.
//! * Bonus XP de quêtes [INVENTÉ] (plafonds de notre cru — spec §7), comme
//!   les journaux A1/A2-A3.
//!
//! # Stats runtime (aucune constante dupliquée)
//! [`finale_stats`] lit `patch_monstats.txt` (Level/minHP/maxHP/AC/Exp/
//! TreasureClass1/Résists), `superuniques.txt` (TC de la vague) et la ligne
//! `MonLvl.txt` (N) du niveau du boss — même convention [`crate::SpawnRule`]
//! que duriel/mephisto ([`crate::quests_actes2::boss_stats`]) : HP/DM/XP = %
//! de la ligne MonLvl, AC/TH directes. Table inattendue ⇒ `None` (jamais de
//! stat fantaisiste).
//!
//! # Save additive
//! Champ `quest_finale` (JSON versionné strict, ids = clés de sauvegarde,
//! validation phases/flags/gating — tout écart ⇒ `Err`) : rétro-compat
//! totale, save v3 inchangée. Offsets u16 .d2s golden : acte IV =
//! 393 + 2·slot + 96·diff, acte V = 413 + 2·slot + 96·diff (spec §1.3/§5/§6
//! — @395/397/399 et @423/425, NM +96, Enfer +192, verrouillés par tests).

use serde::{Deserialize, Serialize};

use crate::quests_actes::{ActEvent, ActPhase};
use crate::world::World;
use crate::AnimState;

// Ré-export des bits u16 .d2s (sémantique identique actes I-III, spec §1.3).
pub use crate::quests_actes::{FLAG_DONE, FLAG_GIVEN_BY_NPC, FLAG_REWARD_PENDING};

// ---------------------------------------------------------------------------
// Récompenses (spec §5/§6 — events de sortie A4-A5)
// ---------------------------------------------------------------------------

/// Récompense émise par le journal final (appliquée par
/// [`World::quest_finale_event`], à l'exception de l'accès d'acte — porté
/// par [`QuestsFinale::access_act`] — et de la victoire, portée par
/// [`GameEvent::GameWon`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FinaleReward {
    /// XP de quête ([INVENTÉ] — bonus retail duré dans le DLL).
    Xp(u32),
    /// Point(s) de compétence (a4q1 Izual +2 [CITÉ spec §5]).
    SkillPoint(u32),
    /// Objet par code table (a4q3 gemme `gcv` [ARRANGÉ-MVP, patch_misc.txt]).
    Item(&'static str),
    /// Accès à l'acte V (a4q2 remise Tyrael [FIDÈLE]) — porté AUSSI par
    /// [`QuestsFinale::access_act`] + l'event [`GameEvent::ActUnlocked`].
    UnlockAccessAct(u8),
    /// **Fin du jeu** (a5q6 : Baal est mort) — le World émet
    /// [`GameEvent::GameWon`] (écran victoire) ; documenté en tête de module.
    GameWon,
}

// ---------------------------------------------------------------------------
// Triggers / authoring
// ---------------------------------------------------------------------------

/// Trigger d'une étape (authoring statique — même esprit que l'acte I
/// et les actes II-III, plus la variante [`Trig::Any`] réutilisée pour les
/// Anciens — 3 kills HORS ORDRE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trig {
    /// Dialogue NPC (code quête bas de casse).
    Talk(&'static str),
    /// Entrée dans un niveau (id `patch_levels.txt`).
    Zone(u32),
    /// Mort d'un monstre — n'importe quel alias : code monstats, code sprite
    /// manifest OU nom de superunique (`SuperUniques.txt`, ex. « Baal
    /// Subject 1 »). Les alias de vagues sont les NOMS superuniques : le
    /// kill d'un sbire de classe ne fait PAS avancer la vague.
    Kill(&'static [&'static str]),
    /// Ramassage d'un objet par code (aucune étape finale ne l'utilise —
    /// conservé pour le pattern commun A1-A5).
    Item(&'static str),
    /// Sous-objectifs HORS ORDRE (les 3 Anciens) : chaque alternative
    /// comptée une fois via le masque `subdone` (état JSON additif, PAS un
    /// bit .d2s).
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

    /// Ce trigger mentionne-t-il statiquement ce record (alias Kill/Any ou
    /// Zone du repaire) ? Sert au spawn scripté (re-entrée au repaire AVANT
    /// le kill = re-spawn si aucun exemplaire vivant — garde côté World).
    fn mentions(&self, rec: &FinaleRec) -> bool {
        let kill_hits = |aliases: &[&'static str]| {
            aliases
                .iter()
                .any(|a| a.eq_ignore_ascii_case(rec.id) || a.eq_ignore_ascii_case(rec.code) || rec.su.map_or(false, |s| a.eq_ignore_ascii_case(s)))
        };
        match self {
            Trig::Zone(id) => *id == rec.lair,
            Trig::Kill(aliases) => kill_hits(aliases),
            Trig::Any(alts) => alts.iter().any(|t| t.mentions(rec)),
            Trig::Talk(_) | Trig::Item(_) => false,
        }
    }
}

/// Étape séquentielle d'une quête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepDef {
    /// Libellé FR (spec §5/§6, condensé).
    pub desc: &'static str,
    /// Ce qui fait avancer l'étape.
    pub trig: Trig,
    /// Occurrences requises (occurrences d'alternatives pour [`Trig::Any`]).
    pub count: u32,
}

/// Définition d'une quête finale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefFinale {
    /// Identifiant stable = CLÉ DE SAUVEGARDE .d2s (`a4q1`…`a4q3`,
    /// `a5q5`, `a6` — clés `qstsa4qN`/`qstsa5qN` du string.tbl, cf.
    /// en-tête de module). Les quêtes de ville a5q1-a5q4 sont hors scope.
    pub id: &'static str,
    /// Nom EN (wiki ; libellé fr = TBL `qstsa4q*`/`qstsa5q*`).
    pub name: &'static str,
    /// Acte de sauvegarde (4 ou 5) — base d'offset u16 .d2s.
    pub act: u8,
    /// Slot LOCAL d'acte (1..=3 pour l'acte IV, 5..=6 pour l'acte V) —
    /// l'acte IV n'a pas de slot 0 porté ici (slot .d2s 393 inutilisé par
    /// le journal retail) et l'acte V commence au slot 5 (a5q1-a5q4 hors
    /// scope, cf. en-tête).
    pub slot: usize,
    /// Étapes séquentielles.
    pub steps: &'static [StepDef],
    /// PNJ de remise de récompense (`None` = complétion directe à la
    /// dernière étape).
    pub reward_npc: Option<&'static str>,
    /// Récompenses rendues à la complétion, dans cet ordre.
    pub rewards: &'static [FinaleReward],
    /// Slot LOCAL du journal qui doit être FLAG_DONE pour ACTIVER cette
    /// quête (`None` = activation au premier trigger).
    pub requires_done: Option<usize>,
}

/// Ids dans l'ordre des slots de CE journal (5 quêtes).
pub const FINALE_QUEST_IDS: [&str; 5] = ["a4q1", "a4q2", "a4q3", "a5q5", "a5q6"];

/// Slots locaux du journal 5 quêtes.
mod slot {
    pub const A4Q2_DIABLO: usize = 1;
    pub const A5Q5_ANCIENS: usize = 3;
    pub const A5Q6_BAAL: usize = 4;
}

/// `patch_levels.txt` : ids de niveaux réels des triggers (re-vérifiés le
/// 2026-10-24 — cf. en-tête de module pour la correction id 83 → 108).
mod lvl {
    /// Plaine du Désespoir (« Act 4 - Mesa 2 », patch_levels.txt:107).
    pub const PLAINS_OF_DESPAIR: u32 = 105;
    /// Rivière de Feu (« Act 4 - Lava 1 », :109).
    pub const RIVER_OF_FLAME: u32 = 107;
    /// Sanctuaire du Chaos (« Act 4 - Diablo 1 », :110) — PAS 83 (Travincal).
    pub const CHAOS_SANCTUARY: u32 = 108;
    /// Sommet d'Arreat (« Act 5 - Mountain Top », :123).
    pub const ARREAT_SUMMIT: u32 = 120;
    /// Trône de la Destruction (« Act 5 - Throne Room », QuestFlag 39, :134).
    pub const THRONE_OF_DESTRUCTION: u32 = 131;
    /// Chambre de la Pierre Mondiale (« Act 5 - World Stone », QuestFlag 39, :135).
    pub const WORLDSTONE_CHAMBER: u32 = 132;
}

/// Authoring complet des quêtes finales (spec §5/§6 — ordre de SAUVEGARDE).
pub(crate) static DEFS_FINALE: [DefFinale; 5] = [
    // a4q1 (acte IV slot 1, u16 @395) — L'Ange déchu : Plaine du Désespoir
    // id 105 → kill Izual (record de quête `izual`, PAS un superunique,
    // spec §5 [VÉRIFIÉ-LOCAL]) → Tyrael → +2 points de compétence [CITÉ].
    DefFinale {
        id: "a4q1",
        name: "The Fallen Angel",
        act: 4,
        slot: 1,
        steps: &[
            StepDef {
                desc: "Traversez la Plaine du Désespoir et retrouvez Izual.",
                trig: Trig::Zone(lvl::PLAINS_OF_DESPAIR),
                count: 1,
            },
            StepDef {
                desc: "Délivrez l'âme d'Izual en abattant son corps corrompu.",
                trig: Trig::Kill(&["izual", "22"]),
                count: 1,
            },
            StepDef {
                desc: "Retournez voir Tyrael au Fort de la Pandémonium.",
                trig: Trig::Talk("tyrael"),
                count: 1,
            },
        ],
        reward_npc: None,
        rewards: &[FinaleReward::SkillPoint(2), FinaleReward::Xp(500)],
        requires_done: None,
    },
    // a4q2 (acte IV slot 2, u16 @397 — ORDRE DE SAUVEGARDE ≠ wiki !) —
    // Epilogue de la terreur : Sanctuaire du Chaos id 108 (PAS 83, cf.
    // en-tête) → kill Diablo (spawn scripté même event, TC « Diablo ») →
    // Tyrael → accès acte V [FIDÈLE]. QUÊTE PORTEUSE.
    DefFinale {
        id: "a4q2",
        name: "Terror's End",
        act: 4,
        slot: 2,
        steps: &[
            StepDef {
                desc: "Trouvez Diablo dans son Sanctuaire du Chaos.",
                trig: Trig::Zone(lvl::CHAOS_SANCTUARY),
                count: 1,
            },
            StepDef {
                desc: "Tuez Diablo.",
                trig: Trig::Kill(&["diablo", "DI"]),
                count: 1,
            },
            StepDef {
                desc: "Parlez à Tyrael : il ouvre le portail vers l'acte V.",
                trig: Trig::Talk("tyrael"),
                count: 1,
            },
        ],
        reward_npc: None,
        rewards: &[FinaleReward::UnlockAccessAct(5), FinaleReward::Xp(1000)],
        requires_done: None,
    },
    // a4q3 (acte IV slot 3, u16 @399) — la Forge de l'Enfer : Rivière de
    // Feu id 107 → kill Hephasto (class `hephasto`, superunique « The
    // Feature Creep » en est le variant, spec §5 [ARRANGÉ-MVP]) → Cain
    // (gemme `gcv` [ARRANGÉ-MVP : TC gemmes retail absente]) → la pierre
    // d'âme est détruite (narratif, pas de trigger dur).
    DefFinale {
        id: "a4q3",
        name: "Hell's Forge",
        act: 4,
        slot: 3,
        steps: &[
            StepDef {
                desc: "Descendez la Rivière de Feu jusqu'à la Forge de l'Enfer.",
                trig: Trig::Zone(lvl::RIVER_OF_FLAME),
                count: 1,
            },
            StepDef {
                desc: "Abattez Hephasto le Forgeron et détruisez la pierre d'âme.",
                trig: Trig::Kill(&["hephasto", "5P"]),
                count: 1,
            },
            StepDef {
                desc: "Consultez Cain pour la récompense de la Forge.",
                trig: Trig::Talk("cain"),
                count: 1,
            },
        ],
        reward_npc: None,
        rewards: &[FinaleReward::Item("gcv"), FinaleReward::Xp(300)],
        requires_done: None,
    },
    // a5q5 (acte V slot 5, u16 @423) — Rite of Passage : Sommet d'Arreat
    // id 120 (l'activation de l'Autel des Cieux est portée par l'entrée
    // [ARRANGÉ-MVP : pas d'event objet dans le flux ActEvent]) → les 3
    // Anciens HORS ORDRE (`ancientbarb1/2/3`, superuniques.txt:46-48) →
    // porte Worldstone ouverte. Sortir du niveau réinitialise au retail —
    // porté par le re-spawn du trigger zone (garde « vivant » côté World).
    DefFinale {
        id: "a5q5",
        name: "Rite of Passage",
        act: 5,
        slot: 5,
        steps: &[
            StepDef {
                desc: "Montez au Sommet d'Arreat et consultez l'Autel des Cieux.",
                trig: Trig::Zone(lvl::ARREAT_SUMMIT),
                count: 1,
            },
            StepDef {
                desc: "Affrontez les trois Anciens sans quitter le Mont Arreat.",
                trig: Trig::Any(&[
                    &Trig::Kill(&["ancientbarb1", "0D"]),
                    &Trig::Kill(&["ancientbarb2", "0F"]),
                    &Trig::Kill(&["ancientbarb3", "0E"]),
                ]),
                count: 3,
            },
        ],
        reward_npc: None,
        // XP [INVENTÉ] : la colonne Exp des `ancientbarb*` est VIDE dans
        // patch_monstats (le drop l'est aussi — TC vide, spec §6).
        rewards: &[FinaleReward::Xp(1500)],
        requires_done: None,
    },
    // a5q6 (acte V slot 6, u16 @425) — Le Crépuscule de la Destruction :
    // trône id 131 → 5 VAGUES « Baal Subject 1-5 » SÉQUENTIELLES (spawn
    // scripté, superuniques.txt:64-68) → Chambre de la Pierre Mondiale
    // id 132 → kill baalcrab → **GAME WON**. GATÉE sur les Anciens
    // (« ACTIF (après A5Q5) », spec §6). QUÊTE PORTEUSE (fin du jeu).
    DefFinale {
        id: "a5q6",
        name: "Eve of Destruction",
        act: 5,
        slot: 6,
        steps: &[
            StepDef {
                desc: "Entrez dans le Trône de la Destruction.",
                trig: Trig::Zone(lvl::THRONE_OF_DESTRUCTION),
                count: 1,
            },
            StepDef { desc: "Vague 1 : repoussez les serviteurs de Baal.", trig: Trig::Kill(&["Baal Subject 1"]), count: 1 },
            StepDef { desc: "Vague 2 : repoussez les serviteurs de Baal.", trig: Trig::Kill(&["Baal Subject 2"]), count: 1 },
            StepDef { desc: "Vague 3 : repoussez les serviteurs de Baal.", trig: Trig::Kill(&["Baal Subject 3"]), count: 1 },
            StepDef { desc: "Vague 4 : repoussez les serviteurs de Baal.", trig: Trig::Kill(&["Baal Subject 4"]), count: 1 },
            StepDef { desc: "Vague 5 : repoussez les serviteurs de Baal.", trig: Trig::Kill(&["Baal Subject 5"]), count: 1 },
            StepDef {
                desc: "Descendez dans la Chambre de la Pierre Mondiale et tuez Baal.",
                trig: Trig::Kill(&["baal", "baalcrab", "42"]),
                count: 1,
            },
        ],
        reward_npc: None,
        // La remise retail (Tyrael : titre/portail) est absorbée par l'écran
        // victoire [ARRANGÉ-MVP, documenté en tête de module].
        rewards: &[FinaleReward::GameWon, FinaleReward::Xp(2000)],
        requires_done: Some(slot::A5Q5_ANCIENS),
    },
];

// ---------------------------------------------------------------------------
// État + moteur
// ---------------------------------------------------------------------------

/// État d'exécution d'une quête finale (sérialisé, façon flags u16 .d2s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinaleRun {
    /// Phase courante.
    pub phase: ActPhase,
    /// Index de l'étape courante (figé à la dernière quand la quête sort
    /// d'Active).
    pub step: usize,
    /// Occurrences acquises sur l'étape courante.
    pub count: u32,
    /// Flags u16 .d2s (bits identiques aux actes I-III : 0 = faite,
    /// 1 = récompense à prendre, 2 = donnée par PNJ).
    pub flags: u16,
    /// Masque des sous-objectifs d'une étape [`Trig::Any`] (bit i =
    /// alternative i déjà comptée) — état MOTEUR additif, PAS un bit .d2s
    /// (aucun bit sous-objectif documenté dans la spec) ; remis à zéro à
    /// chaque changement d'étape.
    pub subdone: u32,
}

/// Journal de quêtes FINALES A4-A5 (spec quests-actes.md §5/§6, étape 10b).
/// 5 slots : 0-2 = acte IV (`a4q1`/`a4q2`/`a4q3` — ORDRE DE SAUVEGARDE :
/// slot 2 = DIABLO, slot 3 = la Forge), 3-4 = acte V finale (`a5q5`,
/// `a5q6`). Authoring statique ; seul l'état est sérialisé.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestsFinale {
    runs: [FinaleRun; 5],
}

impl Default for QuestsFinale {
    fn default() -> Self {
        Self::new()
    }
}

impl QuestsFinale {
    /// Journal neuf : tout INACTIF, flags 0.
    pub fn new() -> QuestsFinale {
        QuestsFinale {
            runs: [FinaleRun { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0, subdone: 0 }; 5],
        }
    }

    /// Déf d'un slot (0-4).
    pub(crate) fn def_of(&self, slot: usize) -> &'static DefFinale {
        &DEFS_FINALE[slot]
    }

    /// Ids des 5 slots (ordre de sauvegarde).
    pub fn ids(&self) -> [&'static str; 5] {
        FINALE_QUEST_IDS
    }

    /// État d'une quête par id (`None` si id inconnu).
    pub fn run(&self, id: &str) -> Option<&FinaleRun> {
        self.ids().iter().position(|s| *s == id).map(|i| &self.runs[i])
    }

    /// Phases dans l'ordre des slots (diff/events UI).
    pub fn phases(&self) -> [ActPhase; 5] {
        let mut out = [ActPhase::Inactive; 5];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.phase;
        }
        out
    }

    /// Flags u16 dans l'ordre des slots — image directe des u16 .d2s
    /// (offsets [`QuestsFinale::d2s_flag_offset`]).
    pub fn quest_flags(&self) -> [u16; 5] {
        let mut out = [0u16; 5];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.flags;
        }
        out
    }

    /// Gating d'accès (spec §1.4). Ce journal couvre la FIN : actes 1-4 =
    /// prérequis portés par les journaux A1/A2-A3 — considérés ouverts ;
    /// **acte 5 = a4q2 bit 0** (remise Tyrael après la mort de Diablo
    /// [FIDÈLE]) ; au-delà : n'existe pas (fin du jeu).
    pub fn access_act(&self, act: u8) -> bool {
        match act {
            1..=4 => true,
            5 => self.runs[slot::A4Q2_DIABLO].flags & FLAG_DONE != 0,
            _ => false,
        }
    }

    /// Porte de la Chambre de la Pierre Mondiale : ouverte à la complétion
    /// des Anciens (a5q5, spec §6 « accès Worldstone Chamber [FIDÈLE] »).
    pub fn worldstone_open(&self) -> bool {
        self.runs[slot::A5Q5_ANCIENS].phase == ActPhase::Done
    }

    /// Fait avancer le journal avec un événement ; renvoie les récompenses
    /// rendues. Un événement n'avance qu'UNE occurrence de l'étape courante
    /// d'UNE quête (par slot) ; hors ordre = ignoré (comme les journaux
    /// A1/A2-A3 et le retail).
    pub fn on_event(&mut self, ev: &ActEvent) -> Vec<FinaleReward> {
        let mut out = Vec::new();
        for slot in 0..5 {
            match self.runs[slot].phase {
                ActPhase::Inactive => {
                    // Gating inter-quêtes (a5q6 ← a5q5 Anciens).
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
    fn advance_after_step(&mut self, slot: usize, done_step: usize, out: &mut Vec<FinaleReward>) {
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

    // -- mapping .d2s (spec §1.3/§5/§6 — table golden finale) ----------------

    /// Offset absolu du u16 de quête : acte IV = 393 + 2·slot + 96·diff
    /// (slots 1-3 → 395/397/399), acte V = 413 + 2·slot + 96·diff
    /// (slots 1-6 → 415…425 ; CE journal porte 5/6) ; Cauchemar +96,
    /// Enfer +192. Spec §5 (« @395 / 491 / 587 » etc.) et §6 (« @423 / 519 /
    /// 615 », « @425 / 521 / 617 ») — verrouillé par les tests golden
    /// `golden_d2s_offsets_finale`. `act` = 4 ou 5, `slot` = slot LOCAL
    /// d'acte (1..=6), `difficulty` 0..=2.
    pub fn d2s_flag_offset(act: u8, slot: usize, difficulty: usize) -> u32 {
        let base = match act {
            4 => 393,
            5 => 413,
            _ => 0,
        };
        base + 2 * slot as u32 + 96 * difficulty as u32
    }

    // -- persistance (contrat save.rs — champ additif `quest_finale`) --------

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
                act: self.def_of(slot).act,
                phase: r.phase,
                step: r.step,
                count: r.count,
                flags: r.flags,
                subdone: r.subdone,
            })
            .collect();
        let s = State { version: STATE_VERSION, quests };
        serde_json::to_string(&s).expect("d2rules: sérialisation quest_finale")
    }

    /// Recharge un état produit par [`QuestsFinale::to_json`]. Validation
    /// stricte (version, ids/actes dans l'ordre, bornes d'étapes, masque
    /// `subdone` cohérent, gating `requires_done` respecté, flags cohérents
    /// avec la phase) — tout écart ⇒ Err(()) : l'appelant garde son état.
    pub fn from_json(json: &str) -> Result<QuestsFinale, ()> {
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
        if s.version != STATE_VERSION || s.quests.len() != 5 {
            return Err(());
        }
        let mut runs = [FinaleRun { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0, subdone: 0 }; 5];
        for (slot, save) in s.quests.into_iter().enumerate() {
            let def = &DEFS_FINALE[slot];
            if save.id != FINALE_QUEST_IDS[slot] || save.act != def.act {
                return Err(());
            }
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
                    Some(alts) => subdone < (1u32 << alts) && subdone.count_ones() == count,
                    None => subdone == 0,
                }
            };
            let ok = match save.phase {
                ActPhase::Inactive => save.step == 0 && save.count == 0 && save.flags == 0 && save.subdone == 0,
                ActPhase::Active => {
                    save.step <= last
                        && save.count < def.steps[save.step].count.max(1)
                        && subdone_ok(save.step, save.subdone, save.count)
                        && save.flags == 0 // aucune activation NPC dans ce journal
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
                FinaleRun { phase: save.phase, step: save.step, count: save.count, flags: save.flags, subdone: save.subdone };
        }
        Ok(QuestsFinale { runs })
    }
}

const STATE_VERSION: u32 = 1;

/// Gating : la dépendance `requires_done` du slot est-elle satisfaite dans
/// l'état en cours de reconstruction ?
fn gating_ok(slot: usize, requires: Option<usize>, runs: &[FinaleRun; 5]) -> bool {
    match requires {
        Some(dep) if dep != slot => runs[dep].flags & FLAG_DONE != 0,
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// BOSS finaux scriptés — DONNÉES RÉELLES (patch_monstats + MonLvl +
// superuniques), même contrat que quests_actes2::boss_stats
// ---------------------------------------------------------------------------

/// Tables embarquées (même arbre assets que d2rules/tables.rs — v1.12).
const PATCH_MONSTATS: &str = include_str!("../../../../assets/patch_monstats.txt");
const MONLVL: &str = include_str!("../../../../assets/MonLvl.txt");
const SUPERUNIQUES: &str = include_str!("../../../../assets/superuniques.txt");

/// Record statique d'un boss/superunique final (identité + ancrage). Les
/// VALEURS (niveau, HP, AC, XP, TC, résists) sont lues RUNTIME dans
/// `patch_monstats.txt` / `MonLvl.txt` / `superuniques.txt` — aucune
/// constante dupliquée : [`finale_stats`] renvoie None sur toute table
/// inattendue (build de tests).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinaleRec {
    /// Code monstats (`diablo` — col `Id`, event `MonsterKilled`).
    pub id: &'static str,
    /// Code sprite manifest (`DI` — col `Code`, spawn).
    pub code: &'static str,
    /// Nom affichable.
    pub name: &'static str,
    /// TC de drop STATIQUE de repli (`TreasureClass1` monstats — « Diablo »,
    /// « Baal », « Izual », « Haphesto » [orthographe réelle de la table,
    /// TreasureClassEx.txt:716] ; vides pour les Anciens : PAS de drop
    /// dédié [VÉRIFIÉ-LOCAL]). Les vagues (TC « Act 5 Champ C ») sont
    /// résolues RUNTIME depuis superuniques.txt (col `TC` du superunique).
    pub tc: &'static str,
    /// Nom SuperUniques.txt le cas échéant (« Baal Subject 1 »…,
    /// « Ancient Barbarian 1 »…) — alias de l'event `MonsterKilled`.
    pub su: Option<&'static str>,
    /// Niveau du spawn scripté (`patch_levels.txt`).
    pub lair: u32,
    /// Rang de vague du trône (1..=5) — `None` pour les boss/Anciens.
    pub wave: Option<usize>,
    /// Slot local du journal porteur (0-4, cf. [`slot`]).
    pub quest_slot: usize,
}

/// Les 12 unités finales scriptées (mission étape 10b). Ordre : boss a4,
/// Anciens, vagues du trône (1→5), Baal.
pub const FINALE_RECORDS: [FinaleRec; 12] = [
    FinaleRec { id: "izual", code: "22", name: "Izual", tc: "Izual", su: None, lair: lvl::PLAINS_OF_DESPAIR, wave: None, quest_slot: 0 },
    FinaleRec { id: "hephasto", code: "5P", name: "Hephasto the Armorer", tc: "Haphesto", su: None, lair: lvl::RIVER_OF_FLAME, wave: None, quest_slot: 2 },
    FinaleRec { id: "diablo", code: "DI", name: "Diablo", tc: "Diablo", su: None, lair: lvl::CHAOS_SANCTUARY, wave: None, quest_slot: 1 },
    FinaleRec { id: "ancientbarb1", code: "0D", name: "Korlic the Protector", tc: "", su: Some("Ancient Barbarian 1"), lair: lvl::ARREAT_SUMMIT, wave: None, quest_slot: 3 },
    FinaleRec { id: "ancientbarb2", code: "0F", name: "Madawc the Mad", tc: "", su: Some("Ancient Barbarian 2"), lair: lvl::ARREAT_SUMMIT, wave: None, quest_slot: 3 },
    FinaleRec { id: "ancientbarb3", code: "0E", name: "Talic the Guardian", tc: "", su: Some("Ancient Barbarian 3"), lair: lvl::ARREAT_SUMMIT, wave: None, quest_slot: 3 },
    FinaleRec { id: "fallenshaman5", code: "FS", name: "Baal Subject 1", tc: "", su: Some("Baal Subject 1"), lair: lvl::THRONE_OF_DESTRUCTION, wave: Some(1), quest_slot: 4 },
    FinaleRec { id: "unraveler5", code: "GY", name: "Baal Subject 2", tc: "", su: Some("Baal Subject 2"), lair: lvl::THRONE_OF_DESTRUCTION, wave: Some(2), quest_slot: 4 },
    FinaleRec { id: "baalhighpriest", code: "HP", name: "Baal Subject 3", tc: "", su: Some("Baal Subject 3"), lair: lvl::THRONE_OF_DESTRUCTION, wave: Some(3), quest_slot: 4 },
    FinaleRec { id: "venomlord", code: "DM", name: "Baal Subject 4", tc: "", su: Some("Baal Subject 4"), lair: lvl::THRONE_OF_DESTRUCTION, wave: Some(4), quest_slot: 4 },
    FinaleRec { id: "baalminion1", code: "43", name: "Baal Subject 5", tc: "", su: Some("Baal Subject 5"), lair: lvl::THRONE_OF_DESTRUCTION, wave: Some(5), quest_slot: 4 },
    FinaleRec { id: "baalcrab", code: "42", name: "Baal", tc: "Baal", su: None, lair: lvl::WORLDSTONE_CHAMBER, wave: None, quest_slot: 4 },
];

/// Valeurs réelles d'une unité finale lues dans les tables ([`finale_stats`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FinaleStats {
    /// Record statique source.
    pub rec: FinaleRec,
    /// Niveau Normal (`Level` monstats ; NM/Enfer = `Level(N)/(H)`, hors
    /// scope ce module — le journal MVP couvre Normal).
    pub level: u32,
    /// PV bruts min (`minHP` monstats — = `maxHP` pour les 4 boss quête ;
    /// 500/600 pour les Anciens [VÉRIFIÉ-LOCAL]).
    pub hp: u32,
    /// PV bruts max (`maxHP` monstats).
    pub hp_max: u32,
    /// AC de table (`AC` monstats).
    pub ac: u32,
    /// XP de table (`Exp` monstats — vide (= 0) pour les Anciens).
    pub exp: u32,
    /// Résists Normal `[ResDm, ResMa, ResFi, ResLi, ResCo, ResPo]` (vides = 0).
    pub res: [i32; 6],
    /// TC de drop effective : col `TC` superuniques.txt pour les vagues
    /// (« Act 5 Champ C »), sinon `TreasureClass1` monstats, sinon `None`
    /// (Anciens : pas de drop dédié).
    pub tc_effective: Option<String>,
    /// Règle de spawn effective (MonLvl (N) du niveau de l'unité + TC).
    pub rule: crate::SpawnRule,
}

/// Ligne MonLvl.txt (N) d'un niveau — parse runtime lenient, cache OnceLock
/// (copie conforme de `quests_actes2::monlvl_row` — modules frères).
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

/// Record patch_monstats d'une unité finale — parse runtime, cache OnceLock.
/// Renvoie `(level, minHP, maxHP, AC, Exp, TreasureClass1, res)`.
fn monstats_finale(id: &str) -> Option<(u32, u32, u32, u32, u32, String, [i32; 6])> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<String, (u32, u32, u32, u32, u32, String, [i32; 6])>> = OnceLock::new();
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
                    (r.u("Level"), r.u("minHP"), r.u("maxHP"), r.u("AC"), r.u("Exp"), r.s("TreasureClass1").to_string(), res),
                );
            }
        }
        m
    });
    map.get(&id.to_ascii_lowercase()).cloned()
}

/// TC superunique (col `TC` = Normal de `superuniques.txt`), par CLASSE —
/// parse runtime, cache OnceLock. `None` : classe absente ou TC vide
/// (Anciens [VÉRIFIÉ-LOCAL]).
fn superunique_tc(class: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::OnceLock;
    static TABLE: OnceLock<HashMap<String, String>> = OnceLock::new();
    let map = TABLE.get_or_init(|| {
        let mut m = HashMap::new();
        if let Ok(t) = crate::tsv::Tsv::parse(SUPERUNIQUES) {
            for i in 0..t.len() {
                let Some(r) = t.rec(i) else { continue };
                let key = r.s("Class").to_ascii_lowercase();
                if key.is_empty() {
                    continue;
                }
                m.insert(key, r.s("TC").to_string());
            }
        }
        m
    });
    map.get(&class.to_ascii_lowercase()).cloned().filter(|tc| !tc.is_empty())
}

/// Stats RÉELLES d'une unité finale : record `patch_monstats.txt` + TC
/// superunique (`superuniques.txt`) + ligne `MonLvl.txt` (N) du niveau →
/// [`crate::SpawnRule`] prête pour [`crate::World::spawn_monster_rule`]
/// (même convention que [`crate::quests_actes2::boss_stats`] : HP/DM/XP = %
/// de la ligne MonLvl, AC/TH directes, TC dédiée, résists Normal). Lookup
/// par id monstats (`"baalcrab"`), code manifest (`"42"`) ou nom
/// superunique (`"Baal Subject 1"`), insensible à la casse. `None` : unité
/// inconnue ou table absente/inattendue.
pub fn finale_stats(id_or_code: &str) -> Option<FinaleStats> {
    let rec = FINALE_RECORDS.iter().find(|b| {
        b.id.eq_ignore_ascii_case(id_or_code)
            || b.code.eq_ignore_ascii_case(id_or_code)
            || b.su.map_or(false, |s| s.eq_ignore_ascii_case(id_or_code))
    })?;
    let (level, hp, hp_max, ac, exp, tc1, res) = monstats_finale(rec.id)?;
    if level == 0 || hp == 0 {
        return None; // table inattendue : pas de règle fantaisiste
    }
    let (ml_ac, ml_th, ml_hp, ml_dm, ml_xp) = monlvl_row(level)?;
    // TC effective : superunique (vagues) > monstats TC1 > repli statique >
    // aucune (Anciens — les deux tables sont vides).
    let tc_effective = superunique_tc(rec.id)
        .or_else(|| (!tc1.is_empty()).then_some(tc1.clone()))
        .or_else(|| (!rec.tc.is_empty()).then_some(rec.tc.to_string()));
    let rule = crate::SpawnRule {
        kind: if rec.wave.is_some() || rec.su.is_some() {
            crate::MonKind::SuperUnique
        } else {
            crate::MonKind::Boss
        },
        hp_pct: ml_hp,
        dmg_pct: ml_dm,
        th_pct: ml_th,
        xp_pct: ml_xp,
        ac: ml_ac,
        treasure_class: tc_effective.clone(),
        res,
    };
    Some(FinaleStats { rec: *rec, level, hp, hp_max, ac, exp, res, tc_effective, rule })
}

/// Unités finales dont l'étape COURANTE (ou d'activation) du journal réagit
/// à `ev` — déclencheur du spawn scripté (décision sur l'état AVANT
/// avancement). Renvoie les couples `(code manifest, id monstats)` :
/// * boss/Anciens : entrée du repaire pendant l'étape zone OU l'étape kill
///   (re-entrée avant kill = reload — la garde « vivant » du World évite
///   les doublons ; pour les Anciens, le reset retail « sortir du niveau »
///   [CITÉ spec §6] est porté par ce re-spawn) ;
/// * vagues : l'entrée du trône (id 131) lance la vague PENDANTE (étape 0
///   = activation → vague 1 ; étape i = vague i) ; le kill du superunique
///   de la vague w (décision pré-avancement) lance la vague w+1 — les
///   vagues avancent SÉQUENTIELLEMENT dans le trône, données réelles
///   (superuniques.txt:64-68), cf. en-tête de module.
pub(crate) fn finale_spawns_for_trigger(log: &QuestsFinale, ev: &ActEvent) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    let throne_wave_pending = |log: &QuestsFinale| -> Option<usize> {
        let run = &log.runs[slot::A5Q6_BAAL];
        match run.phase {
            ActPhase::Inactive => Some(1), // activation = entrée du trône → vague 1
            ActPhase::Active if run.step <= 5 => Some(run.step.max(1)), // étape vague
            _ => None,
        }
    };
    for rec in FINALE_RECORDS {
        let run = &log.runs[rec.quest_slot];
        let step = match run.phase {
            ActPhase::Inactive => 0,
            ActPhase::Active => run.step,
            _ => continue,
        };
        let def = log.def_of(rec.quest_slot);
        // Gating inter-quêtes (a5q6 ← a5q5 Anciens) : rien ne spawn tant que
        // la dépendance n'est pas DONE (même règle que l'avancement).
        if def.requires_done.map_or(false, |dep| log.runs[dep].flags & FLAG_DONE == 0) {
            continue;
        }
        match rec.wave {
            None => {
                // Boss / Anciens : entrée du repaire à l'étape zone ou kill.
                let zone_here = matches!(ev, ActEvent::ZoneEntered { level_id } if *level_id == rec.lair);
                if zone_here && def.steps.get(step).map_or(false, |s| s.trig.mentions(&rec)) {
                    out.push((rec.code, rec.id));
                }
            }
            Some(w) => {
                match ev {
                    // Entrée du trône : la vague PENDANTE apparaît.
                    ActEvent::ZoneEntered { level_id } if *level_id == lvl::THRONE_OF_DESTRUCTION => {
                        if throne_wave_pending(log) == Some(w) {
                            out.push((rec.code, rec.id));
                        }
                    }
                    // Cascade : kill de la vague w-1 (étape courante = w-1,
                    // décision pré-avancement) → la vague w apparaît.
                    ActEvent::MonsterKilled { .. } if w > 1 => {
                        let prev = w - 1;
                        if step == prev
                            && def.steps.get(prev).map_or(false, |s| s.trig.matches(ev))
                        {
                            out.push((rec.code, rec.id));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Branchement World (bloc impl ADDITIF — save `quest_finale`, events de fin)
// ---------------------------------------------------------------------------

/// Objet de quête générique (récompense `Item`) — même MVP que l'acte I.
fn finale_item_def(code: &str) -> crate::ItemDef {
    crate::ItemDef { code: code.to_string(), name: code.to_string(), kind: crate::ItemKind::Misc, amount: 0 }
}

impl World {
    /// Branche le journal spec final (étape 10b). `None` par défaut.
    pub fn set_quest_finale(&mut self, log: QuestsFinale) {
        self.quests_finale = Some(log);
    }

    /// Journal spec branché (lecture).
    pub fn quest_finale(&self) -> Option<&QuestsFinale> {
        self.quests_finale.as_ref()
    }

    /// Journal spec branché (mutation directe — inspection tests/UI).
    pub fn quest_finale_mut(&mut self) -> Option<&mut QuestsFinale> {
        self.quests_finale.as_mut()
    }

    /// Point d'entrée unique de l'intégrateur : avance le journal final
    /// avec un event de gameplay (MÊME flux [`ActEvent`] que les autres
    /// journaux — l'appelant feede les trois), applique les récompenses
    /// motrices (XP, skill points, objet → inventaire/sol), émet
    /// [`GameEvent::ActQuestChanged`] par quête changée de phase,
    /// [`GameEvent::ActUnlocked { act: 5 }`] au déblocage de l'acte V
    /// (remise Tyrael après Diablo) et **[`GameEvent::GameWon`] à la mort
    /// de Baal** (écran victoire, cf. en-tête de module), fait SPAWNER les
    /// boss/vagues scriptées au trigger zone ([`finale_stats`]) et renvoie
    /// les récompenses. Sans journal branché : no-op, renvoie vide.
    pub fn quest_finale_event(&mut self, ev: &ActEvent) -> Vec<FinaleReward> {
        // Décision de spawn scripté sur l'état AVANT avancement.
        let spawns = self
            .quests_finale
            .as_ref()
            .map(|log| finale_spawns_for_trigger(log, ev))
            .unwrap_or_default();
        let Some(log) = self.quests_finale.as_mut() else {
            return Vec::new();
        };
        let before = log.phases();
        let unlocked5_before = log.access_act(5);
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
        if !unlocked5_before && log.access_act(5) {
            self.pending_events.push(crate::GameEvent::ActUnlocked { act: 5 });
        }
        for r in &rewards {
            match *r {
                FinaleReward::Xp(n) => {
                    let mut leveled = Vec::new();
                    self.apply_xp(n, &mut leveled);
                    self.pending_events.extend(leveled);
                }
                FinaleReward::SkillPoint(n) => self.grant_skill_points(n),
                FinaleReward::Item(code) => {
                    let def = finale_item_def(code);
                    if let Some(slot) = self.hero.inv.iter().position(|s| s.is_none()) {
                        self.hero.inv[slot] = Some(def);
                    } else {
                        let (x, y) = (self.hero_ent.x, self.hero_ent.y);
                        self.items.push(crate::ItemOnGround { def, x, y });
                    }
                }
                // Porté par le journal + l'event de transition (accès acte V).
                FinaleReward::UnlockAccessAct(_) => {}
                // FIN DU JEU : Baal est mort — l'écran victoire consomme
                // l'event (documenté en tête de module).
                FinaleReward::GameWon => {
                    self.pending_events.push(crate::GameEvent::GameWon);
                }
            }
        }
        // Spawn scripté APRÈS avancement (la garde « vivant » lit l'état à
        // jour ; l'échec de spawn n'annule jamais la progression de quête).
        for (code, _) in spawns {
            self.spawn_scripted_finale(code);
        }
        rewards
    }

    /// Spawn d'une unité finale près du héros avec ses stats réelles
    /// ([`finale_stats`] → MonLvl (N) de son niveau). Garde anti-doublon :
    /// un exemplaire VIVANT du même code manifest suffit. `false` : unité
    /// inconnue, déjà vivante, ou aucune tuile marchable.
    pub fn spawn_scripted_finale(&mut self, code: &str) -> bool {
        // Garde : un exemplaire vivant du même code manifest suffit.
        let alive = self
            .mon_ent
            .iter()
            .zip(self.mon_dat.iter())
            .any(|(e, d)| e.anim != AnimState::DD && self.mon_tpls[d.tpl].code.eq_ignore_ascii_case(code));
        if alive {
            return false;
        }
        let Some(stats) = finale_stats(code) else {
            return false;
        };
        let rule = stats.rule;
        let (hx, hy) = (self.hero_ent.x as i32, self.hero_ent.y as i32);
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
// Tests : machines a4q1→a4q3 / a5q5-a5q6, gates (Anciens → trône, Diablo →
// acte V), flags/d2s golden, save additive, boss finaux (données réelles),
// vagues séquentielles, GameWon, intégration World. Les grilles des tests
// World sont des STUBS marchables (carré ouvert) — cf. en-tête de
// quests_actes2.rs pour le contrat DRLG.
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
    fn phase(log: &QuestsFinale, id: &str) -> ActPhase {
        log.run(id).unwrap().phase
    }
    fn flags(log: &QuestsFinale, id: &str) -> u16 {
        log.run(id).unwrap().flags
    }
    fn rewards_of(log: &mut QuestsFinale, ev: &ActEvent) -> Vec<FinaleReward> {
        log.on_event(ev)
    }

    /// Amène a5q5 à Done (Sommet + 3 Anciens) — prérequis du trône.
    fn complete_ancients(log: &mut QuestsFinale) {
        log.on_event(&zone(lvl::ARREAT_SUMMIT));
        for c in ["ancientbarb2", "ancientbarb1", "ancientbarb3"] {
            log.on_event(&kill(c));
        }
    }

    // -- a4q1 : Izual → Tyrael (+2 points de compétence) -----------------------

    #[test]
    fn a4q1_izual_chain_two_skill_points() {
        let mut log = QuestsFinale::new();
        // Hors ordre : rien (Tyrael seul ne déclenche rien).
        assert!(rewards_of(&mut log, &talk("tyrael")).is_empty());
        assert_eq!(phase(&log, "a4q1"), ActPhase::Inactive);
        // Zone puis kill (code monstats OU code sprite 22) : étape Tyrael.
        assert!(rewards_of(&mut log, &zone(lvl::PLAINS_OF_DESPAIR)).is_empty());
        assert!(rewards_of(&mut log, &kill("22")).is_empty());
        assert_eq!(log.run("a4q1").unwrap().step, 2, "dernière étape = dialogue Tyrael");
        // N'importe quel autre PNJ ne suffit pas.
        assert!(rewards_of(&mut log, &talk("cain")).is_empty());
        assert_eq!(phase(&log, "a4q1"), ActPhase::Active);
        let r = rewards_of(&mut log, &talk("tyrael"));
        assert_eq!(r, vec![FinaleReward::SkillPoint(2), FinaleReward::Xp(500)]);
        assert_eq!(phase(&log, "a4q1"), ActPhase::Done);
        assert_eq!(flags(&log, "a4q1"), FLAG_DONE);
    }

    // -- a4q2 : Diablo → Tyrael → ACTE V (porteuse) ----------------------------

    #[test]
    fn a4q2_diablo_gates_act5_via_tyrael() {
        let mut log = QuestsFinale::new();
        assert!(!log.access_act(5));
        // Le kill sans entrée au Sanctuaire n'active pas (ordre des étapes).
        assert!(rewards_of(&mut log, &kill("diablo")).is_empty());
        assert_eq!(phase(&log, "a4q2"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &zone(lvl::CHAOS_SANCTUARY)).is_empty());
        assert!(rewards_of(&mut log, &kill_su("DI", "Diablo")).is_empty());
        assert_eq!(log.run("a4q2").unwrap().step, 2, "dernière étape = dialogue Tyrael");
        // Warriv ≠ Tyrael : l'accès acte V attend la remise.
        assert!(rewards_of(&mut log, &talk("warriv")).is_empty());
        assert!(!log.access_act(5));
        let r = rewards_of(&mut log, &talk("tyrael"));
        assert_eq!(r, vec![FinaleReward::UnlockAccessAct(5), FinaleReward::Xp(1000)]);
        assert_eq!(phase(&log, "a4q2"), ActPhase::Done);
        assert!(log.access_act(5));
        // L'acte V n'est PAS la fin du jeu : rien d'autre ne bouge.
        assert!(!log.worldstone_open());
    }

    // -- a4q3 : Hephasto → Cain (gemme) -----------------------------------------

    #[test]
    fn a4q3_hephasto_forge_gives_gem_at_cain() {
        let mut log = QuestsFinale::new();
        assert!(rewards_of(&mut log, &zone(lvl::RIVER_OF_FLAME)).is_empty());
        assert!(rewards_of(&mut log, &kill("hephasto")).is_empty());
        assert_eq!(log.run("a4q3").unwrap().step, 2, "dernière étape = dialogue Cain");
        let r = rewards_of(&mut log, &talk("cain"));
        assert_eq!(r, vec![FinaleReward::Item("gcv"), FinaleReward::Xp(300)]);
        assert_eq!(phase(&log, "a4q3"), ActPhase::Done);
    }

    // -- a5q5 : Anciens — 3 kills HORS ORDRE → porte Worldstone -----------------

    #[test]
    fn a5q5_ancients_any_order_open_worldstone() {
        let mut log = QuestsFinale::new();
        // Les kills sans entrée au Sommet n'activent pas.
        assert!(rewards_of(&mut log, &kill("ancientbarb1")).is_empty());
        assert_eq!(phase(&log, "a5q5"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &zone(lvl::ARREAT_SUMMIT)).is_empty());
        assert_eq!(log.run("a5q5").unwrap(), &FinaleRun { phase: ActPhase::Active, step: 1, count: 0, flags: 0, subdone: 0 });
        // Ordre brouillé : 0F, 0E, puis doublon ignoré, puis 0D.
        assert!(rewards_of(&mut log, &kill("0F")).is_empty());
        assert_eq!(log.run("a5q5").unwrap().subdone, 0b010);
        assert!(rewards_of(&mut log, &kill("ancientbarb3")).is_empty());
        assert!(rewards_of(&mut log, &kill("ancientbarb3")).is_empty(), "doublon ignoré");
        assert_eq!(log.run("a5q5").unwrap().count, 2);
        let r = rewards_of(&mut log, &kill_su("0D", "Ancient Barbarian 1"));
        assert_eq!(r, vec![FinaleReward::Xp(1500)]);
        assert_eq!(phase(&log, "a5q5"), ActPhase::Done);
        assert!(log.worldstone_open(), "les Anciens ouvrent la porte Worldstone");
    }

    // -- a5q6 : trône GATÉ sur les Anciens, vagues séquentielles, GAME WON ------

    #[test]
    fn a5q6_throne_gated_on_ancients_then_five_waves_then_baal() {
        let mut log = QuestsFinale::new();
        // Avant les Anciens : le trône n'active RIEN (gate requires_done).
        assert!(rewards_of(&mut log, &zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        assert!(rewards_of(&mut log, &kill_su("fallenshaman5", "Baal Subject 1")).is_empty());
        assert!(rewards_of(&mut log, &zone(lvl::WORLDSTONE_CHAMBER)).is_empty());
        assert!(rewards_of(&mut log, &kill("baal")).is_empty());
        assert_eq!(phase(&log, "a5q6"), ActPhase::Inactive);
        // Anciens d'abord.
        complete_ancients(&mut log);
        assert!(log.worldstone_open());
        // Trône : 5 vagues SUPERUNIQUES séquentielles (le kill d'un sbire de
        // classe ne fait pas avancer — seuls les noms « Baal Subject N »).
        assert!(rewards_of(&mut log, &zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        assert_eq!(log.run("a5q6").unwrap().step, 1);
        assert!(rewards_of(&mut log, &kill("fallenshaman5")).is_empty(), "sbire sans nom : vague non avancée");
        assert_eq!(log.run("a5q6").unwrap().step, 1);
        // Les 5 vagues, dans l'ordre, par nom superunique (code de classe
        // fallenshaman5/unraveler5/baalhighpriest/venomlord/baalminion1).
        for (i, su) in [
            "Baal Subject 1",
            "Baal Subject 2",
            "Baal Subject 3",
            "Baal Subject 4",
            "Baal Subject 5",
        ]
        .into_iter()
        .enumerate()
        {
            assert!(rewards_of(&mut log, &kill_su("FS", su)).is_empty(), "vague {}", i + 1);
            let step = log.run("a5q6").unwrap().step;
            if i + 1 < 5 {
                assert_eq!(step, i + 2, "vague {} franchie → étape vague {}", i + 1, i + 2);
            }
        }
        assert_eq!(log.run("a5q6").unwrap().step, 6, "dernière étape : kill Baal");
        // L'entrée Worldstone seule ne complète pas (spawn World ; c'est le
        // kill qui compte pour le journal).
        assert!(rewards_of(&mut log, &zone(lvl::WORLDSTONE_CHAMBER)).is_empty());
        assert_eq!(phase(&log, "a5q6"), ActPhase::Active);
        // Kill de Baal = FIN DU JEU.
        let r = rewards_of(&mut log, &kill("baalcrab"));
        assert_eq!(r, vec![FinaleReward::GameWon, FinaleReward::Xp(2000)]);
        assert_eq!(phase(&log, "a5q6"), ActPhase::Done);
    }

    // -- Golden : offsets .d2s des u16 finaux (spec §1.3/§5/§6) ------------------

    #[test]
    fn golden_d2s_offsets_finale() {
        // Acte IV : base 393, slots 1-3 → 395/397/399 (+96 NM, +192 H).
        for (slot, normal) in [(1, 395), (2, 397), (3, 399)] {
            assert_eq!(QuestsFinale::d2s_flag_offset(4, slot, 0), normal);
            assert_eq!(QuestsFinale::d2s_flag_offset(4, slot, 1), normal + 96);
            assert_eq!(QuestsFinale::d2s_flag_offset(4, slot, 2), normal + 192);
        }
        // Acte V : base 413, slots 1-6 → 415…425 (ce journal : 5, 6).
        assert_eq!(QuestsFinale::d2s_flag_offset(5, 5, 0), 423);
        assert_eq!(QuestsFinale::d2s_flag_offset(5, 6, 0), 425);
        assert_eq!(QuestsFinale::d2s_flag_offset(5, 5, 1), 519);
        assert_eq!(QuestsFinale::d2s_flag_offset(5, 6, 2), 617);
        // Spécimens exacts de la spec : a4q1 @395/491/587, Forge @399/495/591
        // (slot 3), A5Q5 @423/519/615, A5Q6 @425/521/617.
        assert_eq!(QuestsFinale::d2s_flag_offset(4, 1, 1), 491);
        assert_eq!(QuestsFinale::d2s_flag_offset(4, 1, 2), 587);
        assert_eq!(QuestsFinale::d2s_flag_offset(4, 3, 1), 495);
        assert_eq!(QuestsFinale::d2s_flag_offset(4, 3, 2), 591);
        assert_eq!(QuestsFinale::d2s_flag_offset(5, 5, 2), 615);
        assert_eq!(QuestsFinale::d2s_flag_offset(5, 6, 1), 521);
    }

    // -- Golden : flags u16 de la chaîne complète ---------------------------------

    #[test]
    fn golden_full_finale_flags() {
        let mut log = QuestsFinale::new();
        for ev in [
            zone(lvl::PLAINS_OF_DESPAIR),
            kill("izual"),
            talk("tyrael"),       // a4q1 done (+2 skill)
            zone(lvl::CHAOS_SANCTUARY),
            kill("DI"),           // Diablo tombe
            talk("tyrael"),       // a4q2 done → accès acte V
            zone(lvl::RIVER_OF_FLAME),
            kill("5P"),           // Hephasto
            talk("cain"),         // a4q3 done (gemme)
            zone(lvl::ARREAT_SUMMIT),
            kill("ancientbarb2"),
            kill("0E"),
            kill("0D"),           // a5q5 done → porte Worldstone
            zone(lvl::THRONE_OF_DESTRUCTION),
            kill_su("FS", "Baal Subject 1"),
            kill_su("GY", "Baal Subject 2"),
            kill_su("HP", "Baal Subject 3"),
            kill_su("DM", "Baal Subject 4"),
            kill_su("43", "Baal Subject 5"),
            zone(lvl::WORLDSTONE_CHAMBER), // spawn Baal côté World
            kill("baal"),                  // a5q6 done → GAME WON
        ] {
            log.on_event(&ev);
        }
        assert!(
            log.phases().iter().all(|p| *p == ActPhase::Done),
            "les 5 quêtes finales sont Done : {:?}",
            log.phases()
        );
        // Aucune activation NPC dans ce journal : tous les flags à 1.
        assert_eq!(log.quest_flags(), [0x0001; 5]);
        assert!(log.access_act(5));
        assert!(log.worldstone_open());
        // Un event isolé ne franchit jamais deux étapes ni ne force une gate.
        let mut fresh = QuestsFinale::new();
        fresh.on_event(&zone(lvl::ARREAT_SUMMIT));
        assert!(rewards_of(&mut fresh, &kill_su("FS", "Baal Subject 1")).is_empty());
        assert_eq!(phase(&fresh, "a5q6"), ActPhase::Inactive);
    }

    // -- Golden : JSON de save (état complet + round trip + partiel) --------------

    #[test]
    fn golden_json_save_full_run() {
        let mut log = QuestsFinale::new();
        for ev in [
            zone(lvl::PLAINS_OF_DESPAIR),
            kill("izual"),
            talk("tyrael"),
            zone(lvl::CHAOS_SANCTUARY),
            kill("DI"),
            talk("tyrael"),
            zone(lvl::RIVER_OF_FLAME),
            kill("hephasto"),
            talk("cain"),
            zone(lvl::ARREAT_SUMMIT),
            kill("ancientbarb2"),
            kill("ancientbarb3"),
            kill("ancientbarb1"),
            zone(lvl::THRONE_OF_DESTRUCTION),
            kill_su("FS", "Baal Subject 1"),
            kill_su("GY", "Baal Subject 2"),
            kill_su("HP", "Baal Subject 3"),
            kill_su("DM", "Baal Subject 4"),
            kill_su("43", "Baal Subject 5"),
            zone(lvl::WORLDSTONE_CHAMBER),
            kill("baalcrab"),
        ] {
            log.on_event(&ev);
        }
        let v: serde_json::Value = serde_json::from_str(&log.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "version": 1,
                "quests": [
                    {"id": "a4q1", "act": 4, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a4q2", "act": 4, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a4q3", "act": 4, "phase": "done", "step": 2, "count": 1, "flags": 1, "subdone": 0},
                    {"id": "a5q5", "act": 5, "phase": "done", "step": 1, "count": 3, "flags": 1, "subdone": 0},
                    {"id": "a5q6", "act": 5, "phase": "done", "step": 6, "count": 1, "flags": 1, "subdone": 0},
                ]
            })
        );
        let log2 = QuestsFinale::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log.to_json(), log2.to_json());
    }

    #[test]
    fn json_round_trip_partial_states() {
        let mut log = QuestsFinale::new();
        // a4q1 : Izual en attente de Tyrael.
        log.on_event(&zone(lvl::PLAINS_OF_DESPAIR));
        log.on_event(&kill("22"));
        // a5q5 : deux Anciens sur trois (hors ordre 0F + 0E).
        log.on_event(&zone(lvl::ARREAT_SUMMIT));
        log.on_event(&kill("0F"));
        log.on_event(&kill("0E"));
        let log2 = QuestsFinale::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log2.run("a5q5").unwrap().subdone, 0b110);
        // Le rechargé poursuit exactement : 3e Ancien → Done (+XP [INVENTÉ]),
        // trône déverrouillé.
        let mut log = log2;
        let r = rewards_of(&mut log, &kill("ancientbarb1"));
        assert_eq!(r, vec![FinaleReward::Xp(1500)]);
        assert_eq!(phase(&log, "a5q5"), ActPhase::Done);
        assert!(log.worldstone_open());
        // a4q1 rechargée à l'étape Tyrael : le dialogue COMPLÈTE la quête.
        let r = rewards_of(&mut log, &talk("tyrael"));
        assert_eq!(r, vec![FinaleReward::SkillPoint(2), FinaleReward::Xp(500)]);
        assert_eq!(phase(&log, "a4q1"), ActPhase::Done);
    }
    #[test]
    fn json_rejects_corrupted_states() {
        let base = QuestsFinale::new();
        // Version inconnue / JSON invalide / mauvaise longueur.
        assert!(QuestsFinale::from_json(r#"{"version":9,"quests":[]}"#).is_err());
        assert!(QuestsFinale::from_json("{oops").is_err());
        assert!(QuestsFinale::from_json(r#"{"version":1,"quests":[]}"#).is_err());
        // Id inconnu (slot 2 = a4q3, PAS a5q5 — ordre de sauvegarde).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][2]["id"] = "a5q5".into();
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // Acte incohérent avec le slot.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][0]["act"] = serde_json::json!(5);
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // Phase incohérente : Done sans bit 0.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1] =
            serde_json::json!({"id":"a4q2","act":4,"phase":"done","step":1,"count":1,"flags":0,"subdone":0});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // RewardPending sans PNJ de remise (a5q6 n'en a pas).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][4] =
            serde_json::json!({"id":"a5q6","act":5,"phase":"reward_pending","step":6,"count":1,"flags":2,"subdone":0});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // subdone fantaisiste : bits hors bornes (a5q5 a 3 alternatives).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][3] =
            serde_json::json!({"id":"a5q5","act":5,"phase":"active","step":1,"count":2,"flags":0,"subdone":0b1001});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // subdone ≠ popcount(count) (1 bit posé, count=2).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][3] =
            serde_json::json!({"id":"a5q5","act":5,"phase":"active","step":1,"count":2,"flags":0,"subdone":0b001});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // Gate bafouée : a5q6 Active alors que a5q5 est Inactive.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][4] =
            serde_json::json!({"id":"a5q6","act":5,"phase":"active","step":1,"count":0,"flags":0,"subdone":0});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // Active avec flag « donnée par NPC » : impossible ici (aucune
        // activation NPC dans l'authoring).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][0] =
            serde_json::json!({"id":"a4q1","act":4,"phase":"active","step":0,"count":0,"flags":4,"subdone":0});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // Inactive avec résidus.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][2] =
            serde_json::json!({"id":"a4q3","act":4,"phase":"inactive","step":1,"count":1,"flags":2,"subdone":0});
        assert!(QuestsFinale::from_json(&v.to_string()).is_err());
        // Contrôle positif : la base propre passe.
        assert!(QuestsFinale::from_json(&base.to_json()).is_ok());
    }

    // -- Authoring : invariants de slots (ordre de sauvegarde !) -------------------

    #[test]
    fn authoring_slot_order_and_gating() {
        assert_eq!(FINALE_QUEST_IDS, ["a4q1", "a4q2", "a4q3", "a5q5", "a5q6"]);
        // ORDRE DE SAUVEGARDE acte IV : slot 1 = Izual, slot 2 = DIABLO,
        // slot 3 = la Forge (spec §5 — ≠ ordre du journal wiki).
        assert_eq!(DEFS_FINALE[1].name, "Terror's End");
        assert_eq!(DEFS_FINALE[1].id, "a4q2");
        assert_eq!(DEFS_FINALE[2].name, "Hell's Forge");
        // Slots d'acte réels (bases u16 .d2s 393 / 413).
        assert_eq!((DEFS_FINALE[0].act, DEFS_FINALE[0].slot), (4, 1));
        assert_eq!((DEFS_FINALE[1].act, DEFS_FINALE[1].slot), (4, 2));
        assert_eq!((DEFS_FINALE[2].act, DEFS_FINALE[2].slot), (4, 3));
        assert_eq!((DEFS_FINALE[3].act, DEFS_FINALE[3].slot), (5, 5));
        assert_eq!((DEFS_FINALE[4].act, DEFS_FINALE[4].slot), (5, 6));
        // Gating : a5q6 attend les Anciens (slot 3).
        assert_eq!(DEFS_FINALE[4].requires_done, Some(slot::A5Q5_ANCIENS));
        // Une seule porteuse par évènement clé : 1 seul UnlockAccessAct(5)
        // (a4q2) et 1 seul GameWon (a5q6).
        assert_eq!(
            DEFS_FINALE.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, FinaleReward::UnlockAccessAct(5))).count(),
            1
        );
        assert_eq!(
            DEFS_FINALE.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, FinaleReward::GameWon)).count(),
            1
        );
        // Ids `patch_levels.txt` documentés (re-vérifiés 2026-10-24) —
        // ancrage des niveaux cités dans les libellés d'étapes.
        assert_eq!(
            (lvl::PLAINS_OF_DESPAIR, lvl::RIVER_OF_FLAME, lvl::CHAOS_SANCTUARY),
            (105, 107, 108)
        );
        assert_eq!(
            (lvl::ARREAT_SUMMIT, lvl::THRONE_OF_DESTRUCTION, lvl::WORLDSTONE_CHAMBER),
            (120, 131, 132)
        );
    }

    // -- BOSS : données réelles des tables (patch_monstats + MonLvl +
    //    superuniques) ------------------------------------------------------------

    #[test]
    fn finale_stats_golden_real_tables() {
        // Diablo — patch_monstats.txt:245 (Level 40 N, HP 6427, AC 87,
        // Exp 3462, TC « Diablo », res [0,0,33,33,33,50]) × MonLvl.txt
        // ligne 40 N (AC 240, TH 419, HP 215 %, DM 25 %, XP 1297 %).
        let d = finale_stats("diablo").expect("diablo présent des tables");
        assert_eq!(d.rec.code, "DI");
        assert_eq!(d.level, 40);
        assert_eq!(d.hp, 6427);
        assert_eq!(d.hp_max, 6427, "minHP = maxHP pour Diablo");
        assert_eq!(d.ac, 87);
        assert_eq!(d.exp, 3462);
        assert_eq!(d.res, [0, 0, 33, 33, 33, 50], "résists réels Diablo N");
        assert_eq!(d.tc_effective.as_deref(), Some("Diablo"));
        assert_eq!(d.rule.kind, crate::MonKind::Boss);
        assert_eq!(d.rule.hp_pct, 215);
        assert_eq!(d.rule.dmg_pct, 25);
        assert_eq!(d.rule.th_pct, 419);
        assert_eq!(d.rule.xp_pct, 1297);
        assert_eq!(d.rule.ac, 240);
        assert_eq!(d.rule.treasure_class.as_deref(), Some("Diablo"));
        // Izual — :258 (Level 29 N, HP 9481, AC 84, Exp 3695, TC « Izual »,
        // res [30,30,30,30,75,30]) × MonLvl 29 N (174/298/126/19/616).
        let i = finale_stats("22").expect("izual par code sprite");
        assert_eq!(i.rec.id, "izual");
        assert_eq!(i.level, 29);
        assert_eq!(i.hp, 9481);
        assert_eq!(i.exp, 3695);
        assert_eq!(i.res, [30, 30, 30, 30, 75, 30], "résists réels Izual N");
        assert_eq!(i.rule.kind, crate::MonKind::Boss);
        assert_eq!(i.rule.hp_pct, 126);
        assert_eq!(i.rule.th_pct, 298);
        assert_eq!(i.rule.ac, 174);
        assert_eq!(i.rule.treasure_class.as_deref(), Some("Izual"));
        // Hephasto — :411 (Level 25 N, HP 760, AC 107, Exp 339, TC
        // « Haphesto » [orthographe table], res Po -50) × MonLvl 25 N
        // (150/254/100/16/470).
        let h = finale_stats("hephasto").expect("hephasto présent");
        assert_eq!(h.level, 25);
        assert_eq!(h.hp, 760);
        assert_eq!(h.res, [0, 0, 0, 0, 0, -50], "Hephasto amplifié poison (res -50)");
        assert_eq!(h.tc_effective.as_deref(), Some("Haphesto"));
        assert_eq!(h.rule.hp_pct, 100);
        assert_eq!(h.rule.ac, 150);
        // Baal — baalcrab :547 (Level 60 N, HP 6336, AC 87, Exp 6460, TC
        // « Baal », res [0,0,33,33,33,50]) × MonLvl 60 N (360/639/418/37/3357).
        let b = finale_stats("baalcrab").expect("baal par id monstats");
        assert_eq!(finale_stats("42").unwrap().rec.id, "baalcrab", "lookup par code sprite");
        assert_eq!(b.rec.id, "baalcrab");
        assert_eq!(b.rec.lair, lvl::WORLDSTONE_CHAMBER);
        assert_eq!(b.level, 60);
        assert_eq!(b.hp, 6336);
        assert_eq!(b.exp, 6460);
        assert_eq!(b.tc_effective.as_deref(), Some("Baal"));
        assert_eq!(b.rule.kind, crate::MonKind::Boss);
        assert_eq!(b.rule.hp_pct, 418);
        assert_eq!(b.rule.dmg_pct, 37);
        assert_eq!(b.rule.th_pct, 639);
        assert_eq!(b.rule.xp_pct, 3357);
        assert_eq!(b.rule.ac, 360);
        // Anciens — ancientbarb1/2/3 (Level 37 N, HP 500/600, AC 84, Exp
        // VIDE, TC VIDE des DEUX tables [VÉRIFIÉ-LOCAL], res partiels) ×
        // MonLvl 37 N (222/386/190/23/1059).
        let a1 = finale_stats("0D").expect("ancient 1 par code sprite");
        assert_eq!(a1.rec.id, "ancientbarb1");
        assert_eq!(a1.rec.su, Some("Ancient Barbarian 1"));
        assert_eq!(a1.level, 37);
        assert_eq!(a1.hp, 500);
        assert_eq!(a1.hp_max, 600, "min≠max pour les Anciens");
        assert_eq!(a1.exp, 0, "colonne Exp vide dans patch_monstats");
        assert_eq!(a1.tc_effective, None, "TC vide : pas de drop dédié");
        assert_eq!(a1.res, [70, 0, 70, 0, 0, 70], "résists réels Korlic N");
        assert_eq!(a1.rule.kind, crate::MonKind::SuperUnique);
        assert_eq!(a1.rule.hp_pct, 190);
        assert_eq!(a1.rule.ac, 222);
        assert_eq!(finale_stats("ancientbarb2").unwrap().res, [0, 0, 0, 70, 70, 0], "résists Madawc");
        assert_eq!(finale_stats("ancientbarb3").unwrap().res, [70, 0, 70, 0, 0, 70], "résists Talic");
        // Vagues — TC « Act 5 Champ C » lue RUNTIME dans superuniques.txt
        // (col TC du superunique, superuniques.txt:64-68), PAS la TC de
        // classe (« Act 5 Cast C » de fallenshaman5).
        let w1 = finale_stats("Baal Subject 1").expect("vague 1 par nom superunique");
        assert_eq!(w1.rec.id, "fallenshaman5");
        assert_eq!(w1.rec.wave, Some(1));
        assert_eq!(w1.level, 40);
        assert_eq!(w1.tc_effective.as_deref(), Some("Act 5 Champ C"));
        assert_eq!(w1.rule.kind, crate::MonKind::SuperUnique);
        assert_eq!(w1.rule.treasure_class.as_deref(), Some("Act 5 Champ C"));
        assert_eq!(w1.rule.hp_pct, 215, "MonLvl 40 N");
        let w5 = finale_stats("baalminion1").expect("vague 5 par classe");
        assert_eq!(w5.rec.wave, Some(5));
        assert_eq!(w5.level, 55);
        assert_eq!(w5.rule.hp_pct, 363, "MonLvl 55 N");
        assert_eq!(w5.rule.ac, 330);
        assert_eq!(w5.tc_effective.as_deref(), Some("Act 5 Champ C"));
        // Lookup insensible à la casse + inconnus → None.
        assert_eq!(finale_stats("DIABLO").unwrap().level, 40);
        assert!(finale_stats("andariel").is_none(), "hors scope finaux");
        assert!(finale_stats("duriel").is_none(), "actes II-III = quests_actes2");
        assert!(finale_stats("diabloclone").is_none(), "clone cyber hors campagne");
        assert!(finale_stats("zz").is_none());
        // Records statiques : lairs et slots porteurs.
        assert_eq!(FINALE_RECORDS[2].lair, lvl::CHAOS_SANCTUARY);
        assert_eq!(FINALE_RECORDS[2].quest_slot, slot::A4Q2_DIABLO);
        assert_eq!(FINALE_RECORDS[3].lair, lvl::ARREAT_SUMMIT);
        assert_eq!(FINALE_RECORDS[6].lair, lvl::THRONE_OF_DESTRUCTION);
        assert_eq!(FINALE_RECORDS[11].lair, lvl::WORLDSTONE_CHAMBER);
        assert_eq!(FINALE_RECORDS[11].quest_slot, slot::A5Q6_BAAL);
    }

    // -- Spawn scripté : vagues séquentielles + boss au trigger zone ---------------

    #[test]
    fn scripted_spawns_follow_the_wave_sequence() {
        // Journal neuf : l'entrée du trône n'active pas la vague 1 (gate
        // Anciens non franchie).
        let log = QuestsFinale::new();
        assert!(finale_spawns_for_trigger(&log, &zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        // Anciens faits : l'entrée du trône lance UNIQUEMENT la vague 1.
        let mut log = QuestsFinale::new();
        complete_ancients(&mut log);
        let spawns = finale_spawns_for_trigger(&log, &zone(lvl::THRONE_OF_DESTRUCTION));
        assert_eq!(spawns, vec![("FS", "fallenshaman5")], "vague 1 seulement");
        // Mauvaise zone (84 = Spider Cave, acte III) : rien — et l'entrée
        // d'un AUTRE repaire (108 = Diablo) ne spawn QUE Diablo.
        assert!(finale_spawns_for_trigger(&log, &zone(84)).is_empty());
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::CHAOS_SANCTUARY)),
            vec![("DI", "diablo")]
        );
        // Kill de la vague 1 (état pré-avancement : step = 1) → vague 2.
        log.on_event(&zone(lvl::THRONE_OF_DESTRUCTION)); // a5q6 Active, vague 1 pendante
        let ev = kill_su("FS", "Baal Subject 1");
        assert_eq!(finale_spawns_for_trigger(&log, &ev), vec![("GY", "unraveler5")]);
        log.on_event(&ev); // la vague 2 est maintenant l'étape pendante
        // Kill d'un sbire sans nom : PAS de cascade.
        assert!(finale_spawns_for_trigger(&log, &kill("unraveler5")).is_empty());
        // Re-entrée au trône : la vague PENDANTE re-spawn (garde vivant
        // côté World) — la vague 2, pas la 1.
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::THRONE_OF_DESTRUCTION)),
            vec![("GY", "unraveler5")]
        );
        // Jusqu'à la vague 5 : chaque kill cascade la suivante (décision
        // pré-avancement) ; après, le kill de la vague 5 ne spawn RIEN.
        for su in ["Baal Subject 2", "Baal Subject 3", "Baal Subject 4"] {
            let ev = kill_su("X", su);
            let next = match su {
                "Baal Subject 2" => ("HP", "baalhighpriest"),
                "Baal Subject 3" => ("DM", "venomlord"),
                _ => ("43", "baalminion1"),
            };
            assert_eq!(finale_spawns_for_trigger(&log, &ev), vec![next], "cascade {su}");
            log.on_event(&ev);
        }
        let ev5 = kill_su("43", "Baal Subject 5");
        let spawn5 = finale_spawns_for_trigger(&log, &ev5);
        assert!(spawn5.is_empty(), "pas de vague 6 : le kill de la vague 5 ne spawn rien");
        log.on_event(&ev5);
        // Étape kill Baal : l'entrée Worldstone (132) spawn Baal, PAS le trône.
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::WORLDSTONE_CHAMBER)),
            vec![("42", "baalcrab")]
        );
        assert!(finale_spawns_for_trigger(&log, &zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        // Après le kill de Baal (Done) : plus AUCUN spawn du trône.
        log.on_event(&kill("baal"));
        assert!(finale_spawns_for_trigger(&log, &zone(lvl::WORLDSTONE_CHAMBER)).is_empty());
    }

    #[test]
    fn scripted_spawns_bosses_at_their_lairs() {
        let mut log = QuestsFinale::new();
        // a4q1 : l'entrée de la Plaine du Désespoir spawn Izual.
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::PLAINS_OF_DESPAIR)),
            vec![("22", "izual")]
        );
        // Re-entrée AVANT le kill (reload) : re-spawn (garde vivant côté
        // World) — l'étape kill mentionne le boss.
        log.on_event(&zone(lvl::PLAINS_OF_DESPAIR));
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::PLAINS_OF_DESPAIR)),
            vec![("22", "izual")]
        );
        // a4q3 : Hephasto en Rivière de Feu (quête fraîche : l'entrée spawn).
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::RIVER_OF_FLAME)),
            vec![("5P", "hephasto")]
        );
        // Mauvaise zone pour Izual (84 = Spider Cave, acte III) : rien.
        assert!(finale_spawns_for_trigger(&log, &zone(84)).is_empty());
        // a4q2 : Diablo au Sanctuaire du Chaos (id 108 — PAS 83 Travincal).
        assert!(finale_spawns_for_trigger(&log, &zone(83)).is_empty());
        assert_eq!(
            finale_spawns_for_trigger(&log, &zone(lvl::CHAOS_SANCTUARY)),
            vec![("DI", "diablo")]
        );
        // a5q5 : les TROIS Anciens au Sommet (même zone, un spawn chacun).
        log.on_event(&zone(lvl::ARREAT_SUMMIT));
        let ancients = finale_spawns_for_trigger(&log, &zone(lvl::ARREAT_SUMMIT));
        assert_eq!(
            ancients,
            vec![("0D", "ancientbarb1"), ("0F", "ancientbarb2"), ("0E", "ancientbarb3")]
        );
    }

    // -- Intégration World : spawn scripté + récompenses + events ------------------

    use crate::{GameEvent, MonKind, World};

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    /// Manifest réel + les unités finales SAUF celles de `codes` (valeurs
    /// brutes patch_monstats) — STUBS de grille : la DRLG A4-A5 n'est pas
    /// fusionnée (cf. en-tête de quests_actes2.rs).
    fn manifest_finale() -> serde_json::Value {
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([
            {"code": "22", "name": "Izual", "stats": {
                "level": 29, "hpMin": 9481, "hp": 9481,
                "dmgMin": 230, "dmgMax": 300, "exp": 3695,
                "speed": 4.0, "treasureClass": "Izual"}},
            {"code": "5P", "name": "Hephasto", "stats": {
                "level": 25, "hpMin": 760, "hp": 760,
                "dmgMin": 120, "dmgMax": 160, "exp": 339,
                "speed": 4.0, "treasureClass": "Haphesto"}},
            {"code": "DI", "name": "Diablo", "stats": {
                "level": 40, "hpMin": 6427, "hp": 6427,
                "dmgMin": 190, "dmgMax": 260, "exp": 3462,
                "speed": 4.0, "treasureClass": "Diablo"}},
            {"code": "0D", "name": "Korlic", "stats": {
                "level": 37, "hpMin": 500, "hp": 500,
                "dmgMin": 140, "dmgMax": 190, "exp": 0,
                "speed": 4.0}},
            {"code": "0F", "name": "Madawc", "stats": {
                "level": 37, "hpMin": 500, "hp": 500,
                "dmgMin": 140, "dmgMax": 190, "exp": 0,
                "speed": 4.0}},
            {"code": "0E", "name": "Talic", "stats": {
                "level": 37, "hpMin": 500, "hp": 500,
                "dmgMin": 140, "dmgMax": 190, "exp": 0,
                "speed": 4.0}},
            {"code": "FS", "name": "Baal Subject 1", "stats": {
                "level": 40, "hpMin": 65, "hp": 65,
                "dmgMin": 100, "dmgMax": 130, "exp": 125,
                "speed": 4.0, "treasureClass": "Act 5 Champ C"}},
            {"code": "GY", "name": "Baal Subject 2", "stats": {
                "level": 40, "hpMin": 192, "hp": 192,
                "dmgMin": 90, "dmgMax": 120, "exp": 211,
                "speed": 4.0, "treasureClass": "Act 5 Champ C"}},
            {"code": "HP", "name": "Baal Subject 3", "stats": {
                "level": 40, "hpMin": 190, "hp": 190,
                "dmgMin": 100, "dmgMax": 130, "exp": 225,
                "speed": 4.0, "treasureClass": "Act 5 Champ C"}},
            {"code": "DM", "name": "Baal Subject 4", "stats": {
                "level": 40, "hpMin": 190, "hp": 190,
                "dmgMin": 110, "dmgMax": 140, "exp": 239,
                "speed": 4.0, "treasureClass": "Act 5 Champ C"}},
            {"code": "43", "name": "Baal Subject 5", "stats": {
                "level": 55, "hpMin": 472, "hp": 472,
                "dmgMin": 150, "dmgMax": 200, "exp": 634,
                "speed": 4.0, "treasureClass": "Act 5 Champ C"}},
            {"code": "42", "name": "Baal", "stats": {
                "level": 60, "hpMin": 6336, "hp": 6336,
                "dmgMin": 250, "dmgMax": 340, "exp": 6460,
                "speed": 4.0, "treasureClass": "Baal"}},
        ]);
        m
    }

    /// Manifest des finales PRIVÉ de certains gabarits (les spawns de ces
    /// codes échouent proprement — cf. `world_no_monster_tpl_is_a_clean_no_spawn`).
    fn manifest_without(codes: &[&str]) -> serde_json::Value {
        let mut m = manifest_finale();
        let kept: Vec<serde_json::Value> = m["bosses"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|b| !codes.iter().any(|c| b["code"] == *c))
            .cloned()
            .collect();
        m["bosses"] = serde_json::Value::Array(kept);
        m
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Amène a5q5 à Done côté World (Sommet + 3 Anciens, sans spawn :
    /// les kills sont injectés en events — les monstres n'ont pas à être
    /// abattus « pour de vrai » ici).
    fn complete_ancients_world(w: &mut World) {
        for ev in [
            zone(lvl::ARREAT_SUMMIT),
            kill("ancientbarb2"),
            kill("ancientbarb3"),
            kill("ancientbarb1"),
        ] {
            w.quest_finale_event(&ev);
        }
    }

    #[test]
    fn world_spawns_diablo_at_chaos_sanctuary_with_real_stats() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_finale(), 0xD2_10B);
        w.set_quest_finale(QuestsFinale::new());
        assert!(w.monsters().is_empty(), "manifest sans monstres de planche");
        // L'entrée du Sanctuaire (id 108, PAS 83) fait apparaître Diablo —
        // UNE fois.
        let rewards = w.quest_finale_event(&zone(lvl::CHAOS_SANCTUARY));
        assert!(rewards.is_empty());
        assert_eq!(w.monsters().len(), 1, "Diablo spawné au trigger zone");
        assert_eq!(w.monster_kind(0), Some(MonKind::Boss));
        // HP effectifs = 6427 × 215 % (ligne MonLvl 40 N) = 13818.
        assert_eq!(w.monster_hp(0), Some((13818, 13818)));
        // TC/résists réels visibles dans la save (MonSave.tc/res).
        let save = w.save();
        assert!(save.contains("\"Diablo\""), "TC dédiée « Diablo » persistée");
        // Re-trigger : pas de doublon (garde vivant).
        w.quest_finale_event(&zone(lvl::CHAOS_SANCTUARY));
        assert_eq!(w.monsters().len(), 1);
        // Kill → remise Tyrael → accès acte V.
        assert!(w.quest_finale_event(&kill("DI")).is_empty());
        let r = w.quest_finale_event(&talk("tyrael"));
        assert_eq!(r, vec![FinaleReward::UnlockAccessAct(5), FinaleReward::Xp(1000)]);
        assert!(w.quest_finale().unwrap().access_act(5));
        // Les events sont flushés au PROCHAIN tick (contrat d2rules).
        let ev = w.tick(10, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::ActUnlocked { act: 5 })),
            "ActUnlocked(5) émis à la remise Tyrael"
        );
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::ActQuestChanged { id, .. } if id == "a4q2")),
            "ActQuestChanged a4q2 émis"
        );
    }

    #[test]
    fn world_spawns_the_five_waves_then_baal_game_won() {
        // Sans les gabarits des Anciens : les spawns 0D/0F/0E échouent
        // proprement (a5q5 progresse quand même) — la séquence du trône est
        // isolée des spawns du Sommet.
        let mut w = World::with_seed(open_grid(24, 24), manifest_without(&["0D", "0F", "0E"]), 0xD2_10B);
        w.set_quest_finale(QuestsFinale::new());
        complete_ancients_world(&mut w);
        assert!(w.quest_finale().unwrap().worldstone_open());
        // Entrée du trône : vague 1 SEULE.
        assert!(w.quest_finale_event(&zone(lvl::THRONE_OF_DESTRUCTION)).is_empty());
        assert_eq!(w.monsters().len(), 1, "vague 1 spawnée");
        assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
        // HP vague 1 = 65 × 215 % (MonLvl 40 N) = 139.
        assert_eq!(w.monster_hp(0), Some((139, 139)));
        // Cascade : chaque kill de superunique spawn la vague suivante
        // (les indices s'accumulent : 1, 2, 3, 4, 5 unités).
        for (i, su) in [
            "Baal Subject 1",
            "Baal Subject 2",
            "Baal Subject 3",
            "Baal Subject 4",
            "Baal Subject 5",
        ]
        .into_iter()
        .enumerate()
        {
            assert!(w.quest_finale_event(&kill_su("FS", su)).is_empty(), "vague {}", i + 1);
            let expected = if i + 1 < 5 { i + 2 } else { 5 };
            assert_eq!(w.monsters().len(), expected, "vague {} → cascade", i + 1);
        }
        // L'entrée Worldstone (132) fait apparaître Baal — pas le trône (131).
        w.quest_finale_event(&zone(lvl::THRONE_OF_DESTRUCTION)); // garde vivant : rien
        assert_eq!(w.monsters().len(), 5);
        w.quest_finale_event(&zone(lvl::WORLDSTONE_CHAMBER));
        assert_eq!(w.monsters().len(), 6, "Baal spawné en Chambre");
        assert_eq!(w.monster_kind(5), Some(MonKind::Boss));
        // HP Baal = 6336 × 418 % (MonLvl 60 N) = 26484.
        assert_eq!(w.monster_hp(5), Some((26484, 26484)));
        // Kill de Baal = GAME WON (event flushé au prochain tick) + XP.
        let rewards = w.quest_finale_event(&kill("baalcrab"));
        assert_eq!(rewards, vec![FinaleReward::GameWon, FinaleReward::Xp(2000)]);
        assert_eq!(w.quest_finale().unwrap().run("a5q6").unwrap().phase, ActPhase::Done);
        let ev = w.tick(10, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::GameWon)),
            "GameWon émis à la mort de Baal (écran victoire)"
        );
        // Re-trigger Worldstone : plus aucun spawn (Baal est Done).
        w.quest_finale_event(&zone(lvl::WORLDSTONE_CHAMBER));
        assert_eq!(w.monsters().len(), 6);
    }

    #[test]
    fn world_ancients_spawn_trio_and_rewards_apply() {
        let mut w = World::with_seed(open_grid(24, 24), manifest_finale(), 0xD2_10B);
        w.set_quest_finale(QuestsFinale::new());
        // Entrée du Sommet : les TROIS Anciens apparaissent (stats MonLvl 37
        // : HP 500 × 190 % = 950 ; res réels 70/0/70/0/0/70 pour Korlic).
        w.quest_finale_event(&zone(lvl::ARREAT_SUMMIT));
        assert_eq!(w.monsters().len(), 3, "Korlic + Madawc + Talic");
        assert_eq!(w.monster_kind(0), Some(MonKind::SuperUnique));
        assert_eq!(w.monster_hp(0), Some((950, 950)));
        // Le kill d'un Ancien déjà compté ne re-avance pas la quête…
        assert!(w.quest_finale_event(&kill("ancientbarb2")).is_empty());
        assert!(w.quest_finale_event(&kill("0F")).is_empty(), "doublon ignoré (masque subdone)");
        // …mais les deux autres complètent a5q5 (+XP [INVENTÉ]).
        assert!(w.quest_finale_event(&kill("0E")).is_empty());
        let r = w.quest_finale_event(&kill("ancientbarb1"));
        assert_eq!(r, vec![FinaleReward::Xp(1500)]);
        assert!(w.quest_finale().unwrap().worldstone_open());
        // a4q1 : Izual → Tyrael = +2 points de compétence [CITÉ spec §5].
        w.quest_finale_event(&zone(lvl::PLAINS_OF_DESPAIR));
        assert_eq!(w.monsters().len(), 4, "Izual spawné en Plaine du Désespoir");
        assert_eq!(w.monster_kind(3), Some(MonKind::Boss));
        // HP Izual = 9481 × 126 % (MonLvl 29 N) = 11946.
        assert_eq!(w.monster_hp(3), Some((11946, 11946)));
        // +2 points de compétence [CITÉ spec §5] — les bonus XP de quêtes
        // (Anciens) peuvent avoir fait LEVELLER le héros (+1 pt/niveau) :
        // on asserte le DELTA.
        let sp_before = w.unspent_skill_points();
        w.quest_finale_event(&kill("izual"));
        let r = w.quest_finale_event(&talk("tyrael"));
        assert_eq!(r, vec![FinaleReward::SkillPoint(2), FinaleReward::Xp(500)]);
        assert_eq!(w.unspent_skill_points(), sp_before + 2);
        // a4q3 : Hephasto → Cain = gemme `gcv` dans l'inventaire.
        w.quest_finale_event(&zone(lvl::RIVER_OF_FLAME));
        w.quest_finale_event(&kill("5P"));
        let r = w.quest_finale_event(&talk("cain"));
        assert_eq!(r, vec![FinaleReward::Item("gcv"), FinaleReward::Xp(300)]);
        assert!(w.inventory().iter().any(|s| s.as_ref().map_or(false, |d| d.code == "gcv")));
    }

    #[test]
    fn world_save_roundtrip_keeps_finale_journal() {
        // Manifest SANS boss spawnables : le journal avance sans spawn — la
        // save ne porte que le journal. (Un monde CONTENANT une unité
        // spawnée paresseusement ne se recharge pas : gabarit absent du
        // manifest ⇒ Err contractuel — validation tpl de `load_json`.)
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2_10B);
        w.set_quest_finale(QuestsFinale::new());
        w.quest_finale_event(&zone(lvl::CHAOS_SANCTUARY)); // progresse, sans spawn
        w.quest_finale_event(&kill("DI"));
        w.quest_finale_event(&talk("tyrael")); // a4q2 → Done (accès acte V)
        let json = w.save();
        // Rétro-compat : champ quest_finale ABSENT → charge (journal vide).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.get("quest_finale").is_some(), "le champ additif est présent dans la save fraîche");
        v.as_object_mut().unwrap().remove("quest_finale");
        let w2 = World::load(&v.to_string()).unwrap();
        // Sans champ : PAS de journal branché (rétro-compat, jamais d'état
        // à moitié reconstitué).
        assert!(w2.quest_finale().is_none());
        // Round trip exact : re-save identique.
        let w3 = World::load(&json).unwrap();
        let log2 = w3.quest_finale().unwrap();
        assert_eq!(log2.run("a4q2").unwrap().phase, ActPhase::Done);
        assert_eq!(log2.run("a4q2").unwrap().step, 2, "dernière étape = Tyrael franchi");
        assert_eq!(log2.run("a4q2").unwrap().flags, FLAG_DONE);
        assert_eq!(log2.to_json(), w.quest_finale().unwrap().to_json());
        // Journal PRÉSENT mais invalide ⇒ Err (contrat strict).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v["quest_finale"] = serde_json::json!({"version": 99, "quests": []});
        assert!(World::load(&v.to_string()).is_err());
        // Sans journal branché : no-op.
        let mut w4 = World::with_seed(open_grid(8, 8), manifest_finale(), 7);
        assert!(w4.quest_finale_event(&kill("baalcrab")).is_empty());
        assert!(w4.quest_finale().is_none());
    }

    #[test]
    fn world_no_monster_tpl_is_a_clean_no_spawn() {
        // Manifest SANS bloc boss DI : le trigger zone avance quand même la
        // quête (le spawn échoue proprement, jamais de panique).
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m["bosses"] = serde_json::json!([]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2_10B);
        w.set_quest_finale(QuestsFinale::new());
        w.quest_finale_event(&zone(lvl::CHAOS_SANCTUARY));
        assert!(w.monsters().is_empty(), "pas de gabarit DI : pas de spawn");
        assert_eq!(
            w.quest_finale().unwrap().run("a4q2").unwrap().step,
            1,
            "la progression de quête n'est pas bloquée"
        );
    }
}
