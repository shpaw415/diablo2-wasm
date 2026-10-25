//! `dialogue` — arbres de dialogue des PNJ de ville (étape 22a du corps
//! maître, PLAN-COMPLET §22), textes fr RÉELS des tables TBL v1.12
//! embarquées — AUCUN texte inventé. PORT 3c-b (coop `d2w-coop`) : sessions
//! MULTI-JOUEURS — verrou par slot, gel du monde GLOBAL (cf. § coop bas).
//!
//! # Modèle (mission 22a)
//! * [`DialogueNode`] `{ id, lignes (fr réel), options }` ;
//!   [`DialogueOption`] `{ label_key, label, next, cond, action }` —
//!   `label_key`/lignes citent leur clé TBL (source `[CITÉ]`).
//! * UN arbre par NPC (`quest_code` de [`crate::npc::NPCS`]) × phase de
//!   quête [`DialoguePhase`] — la phase vient du journal spec
//!   [`ActQuests`] (branché via `World::set_quest_act1`) :
//!   - `Avant` : quête du donneur `Inactive` → réplique `A1QnInit<Npc>` ;
//!   - `Pendant` : quête `Active` → `A1QnEarlyReturn<Npc>` pour le donneur,
//!     `A1QnAfterInit*` pour les autres PNJ (comme le retail, tout le camp
//!     commente la quête en cours) ;
//!   - `Remise` : `RewardPending` → `A1QnSuccessful<Npc>` (a1q4 :
//!     `A1Q4QuestSuccessful<Npc>`, nommage retail distinct) ;
//!   - `Apres` : `Done` ou aucune quête → ragots (`IntroGossip`, `Gossip`).
//! * Nœuds : `root` = réplique de quête ; `gossip<i>` = ragots (menu
//!   « divers »), « dites-m'en plus » reboucle sur le premier ragot.
//!
//! # Conditions d'options (mission 2)
//! Une option n'est visible que si sa [`DialogueCond`] est remplie
//! (évaluation VIVE à chaque affichage/choix) — ex. le voyage de Warriv
//! (`WarrivMenu1b` « aller vers l'est », action [`DialogueAction::Travel`])
//! n'apparaît que `a1q6 == Done`. Les actions branchent les API existantes :
//! `NpcTalked` émis à l'ouverture (via `talk_to_npc` + journal spec),
//! voyage (`npc_travel`), boutique (OpenShop → `vendor.rs`), engagement
//! mercenaire (Kashya → `mercenary.rs`), réparation (Charsi → `npc_repair`).
//!
//! # Données 100 % TBL (mission 3)
//! Les lignes sont résolues À L'EXÉCUTION par clé dans les 3 tables fr
//! embarquées ([`crate::tbl::TblSet::bundled_fra`], ordre patch > exp >
//! string — contrat W18) ; chaque entrée des tables statiques ci-dessous
//! CITE sa clé `string.tbl`. Clé absente ⇒ ligne `TBD…` documentée (cas
//! réel : Cain × a1q4 en phase `Pendant` — `A1Q4EarlyReturnCain` et
//! `A1Q4AfterInitScrollCain` n'existent pas dans string.tbl v1.12 ; test
//! `cain_tbd_documente`). Le préfixe numérique des répliques (index sonore
//! du format TBL, ex. `45\nMon nom est Akara…`) est retiré à l'affichage.
//!
//! # Monde gelé pendant la session (mission 4)
//! Ouverte ([`World::dialogue_session`]), la session GÈLE le monde :
//! `World::tick` ne simule plus rien — héros, monstres, NPC, missiles,
//! auras, horloge (`now_ms`) — et se contente de flusher les événements en
//! attente (le `GameEvent::NpcTalked` de l'ouverture, quêtes). « partir »
//! ([`World::close_dialogue`]) rend le tick au monde. La session est
//! TRANSITOIRE (jamais persistée — comme `corpse_pending`) : après un
//! load, l'UI rouvre le dialogue d'un clic.
//!
//! # COOP 3c-b — sessions multi-joueurs (verrou + gel global)
//! Port du corps maître adapté au monde `players[]` (3b-1, 1 slot = 1
//! héros) — DÉCISIONS documentées (contrat UI demo/dialogue-ui.js) :
//! * **TENEUR** : [`DialogueSession`] porte `slot` — le joueur qui tient la
//!   conversation. Ouverture via [`World::open_dialogue_slot`] (portée/mort
//!   vérifiées pour CE slot via `talk_to_npc_slot`) ; le chemin historique
//!   [`World::open_dialogue`] route au héros le plus proche (solo : slot 0,
//!   strictement identique).
//! * **VERROU** : le monde est partagé — UNE SEULE session à la fois. Un
//!   slot AUTRE que le teneur qui ouvre reçoit
//!   [`DialogueError::Busy`] (NPC + slot teneur ; l'UI affiche le bandeau
//!   « <NPC> discute avec <joueur> » et bloque le clic). Le brief « ne
//!   peuvent pas ouvrir le même NPC » est couvert A FORTIORI : le gel
//!   global rend toute autre conversation (même NPC ou pas) impossible —
//!   un verrou plus fin n'aurait aucun effet observable. Le TENEUR peut
//!   rouvrir (clic UI = remplacement, sémantique 22a conservée).
//! * **GEL GLOBAL** (vérifié, décision style retail) : la session gèle
//!   `World::tick_inputs` ENTIER — donc `tick`, `tick_slot` et
//!   `tick_queued` du pont d2game — pour TOUS les slots à la fois : ni
//!   déplacement, ni monstre, ni missile, ni horloge ; seuls les événements
//!   en attente (le `NpcTalked` de l'ouverture, quêtes) continuent de
//!   couler. C'est le comportement du corps maître (22a, mission 4) étendu
//!   aux slots : un gel par slot casserait le determinisme lockstep (le
//!   temps de monde divergerait entre clients) — testé par
//!   `coop_gel_global_tous_les_slots`.
//! * **CHOIX/FERMETURE** : seul le teneur choisit
//!   ([`World::choose_dialogue_slot`]) et referme
//!   ([`World::close_dialogue_slot`]) ; les chemins historiques
//!   `choose_dialogue`/`close_dialogue` restent la vue sans garde (tests +
//!   UI solo).
//! * **VOIX 23a NON PORTÉE** : ce corps n'a pas d'assets `sfx/speech` — la
//!   section vocale du maître (voice_file_for/all_line_keys) n'est pas
//!   reprise ; à ré-extraire si les wavs arrivent.

use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::quests_actes::{ActEvent, ActPhase, ActQuests, ACT1_QUEST_IDS};
use crate::vendor::VendorId;
use crate::world::World;

// ---------------------------------------------------------------------------
// Types publics
// ---------------------------------------------------------------------------

/// Phase de quête qui pilote l'arbre d'un NPC (dérivée du journal spec
/// [`ActQuests`] — `remise` = `RewardPending` du retail, détail au-delà des
/// 3 phases de mission : la machine A1 la distingue).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DialoguePhase {
    /// Quête du donneur `Inactive` — il la donne (`A1QnInit*`).
    Avant,
    /// Quête `Active` — relances (`EarlyReturn`) et commentaires du camp.
    Pendant,
    /// Quête `RewardPending` — répliques de remise (`A1QnSuccessful*`).
    Remise,
    /// `Done` ou aucune quête — ragots (`IntroGossip`/`Gossip`).
    Apres,
}

impl DialoguePhase {
    /// Code stable (JSON UI/save d2game).
    pub fn as_str(&self) -> &'static str {
        match self {
            DialoguePhase::Avant => "avant",
            DialoguePhase::Pendant => "pendant",
            DialoguePhase::Remise => "remise",
            DialoguePhase::Apres => "apres",
        }
    }
}

/// Action d'une option — branche les API existantes (aucune simulation
/// nouvelle : le voyage appelle `npc_travel`, le reste est consommé par
/// l'UI/d2game qui ouvrent boutique/mercenaire/réparation déjà branchés).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DialogueAction {
    /// Pure progression de dialogue (ragots).
    Say,
    /// « partir » — referme la session (`close_dialogue`).
    Close,
    /// Warriv — `npc_travel()` appelé (`NotAvailable` MVP acte II, doc).
    Travel,
    /// Ouvre le magasin `vendor.rs` du vendeur (nom = `VendorId::name`).
    OpenShop { vendor: String },
    /// Kashya — engagement mercenaire (API `hire_merc`, chantier H).
    HireMercenary,
    /// Charsi — réparation (API `npc_repair`).
    Repair,
}

/// Condition de visibilité d'une option (évaluée VIVE sur le journal spec).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DialogueCond {
    /// Toujours visible.
    Always,
    /// Visible seulement si la quête (`a1q1`…`a1q6`) est `Done` — ex.
    /// voyage Warriv ⇔ `a1q6` terminé (mission 22a §2).
    QuestDone { quest: String },
}

/// Une option de dialogue : libellé (clé TBL citée), nœud suivant, condition
/// d'apparition, action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueOption {
    /// Clé TBL du libellé (citation `[CITÉ]` — ex. `NPCMenuTrade`).
    pub label_key: String,
    /// Libellé fr réel résolu (ex. « acheter/vendre »).
    pub label: String,
    /// Nœud suivant (`gossip<i>`) — `None` = reste sur le nœud courant.
    pub next: Option<String>,
    /// Condition d'apparition ([`World::dialogue_visible_options`]).
    pub cond: DialogueCond,
    /// Action produite par [`World::choose_dialogue`].
    pub action: DialogueAction,
}

/// Nœud de dialogue résolu (mission 22a §1) : id, lignes fr RÉELLES
/// (clés citées), options.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueNode {
    /// `"root"` (réplique de quête) ou `"gossip1"`… (ragots).
    pub id: String,
    /// Clés TBL des lignes (citation ; `TBD…` documenté si clé absente).
    pub line_keys: Vec<String>,
    /// Lignes fr réelles (préfixe sonore TBL retiré).
    pub lignes: Vec<String>,
    /// Options du nœud (visibilité = [`DialogueCond`]).
    pub options: Vec<DialogueOption>,
}

/// Session de dialogue ouverte (TRANSITOIRE — jamais persistée).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DialogueSession {
    /// 3c-b (coop) : slot du JOUEUR qui tient la conversation (0..8). Le
    /// monde gelé s'applique à tous ; lui seul choisit/referme (verrou).
    pub slot: usize,
    /// Index du NPC dans [`World::town_npcs`].
    pub npc_index: usize,
    /// Code quête (`"akara"`… — clé des tables de ce module).
    pub npc: String,
    /// Nom affichable (`NameStr` du roster).
    pub npc_name: String,
    /// Quête motrice au moment de l'ouverture (`a1q1`…`a1q6`), si relevante.
    pub quest: Option<String>,
    /// Phase de quête capturée à l'ouverture (la remise s'affiche pendant
    /// que le `NpcTalked` d'ouverture complète la quête : les CONDITIONS
    /// des options restent évaluées en vivant — cf. Warriv).
    pub phase: DialoguePhase,
    /// Nœud courant (résolu — lignes fr réelles + options).
    pub node: DialogueNode,
}

/// 3c-b (coop) : erreur d'ouverture de dialogue — le VERROU (`Busy`) porte
/// la session en cours (NPC + slot teneur) pour le bandeau des autres
/// clients ; les autres variantes reflètent `NpcTalkError` + slot inconnu.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum DialogueError {
    /// Une session est déjà ouverte par le slot `slot` (le monde gelé est
    /// partagé — une seule conversation à la fois, cf. § coop).
    Busy { npc: String, npc_name: String, slot: usize },
    /// NPC hors de portée du héros du slot demandé (2.5 cellules).
    OutOfRange,
    /// Le héros du slot demandé est mort.
    HeroDead,
    /// Slot hors monde (`>= hero_count()`).
    BadSlot,
    /// Index hors roster.
    BadIndex,
}

impl DialogueError {
    /// Message lisible UI (même famille que `NpcTalkError::as_str`).
    pub fn as_str(&self) -> String {
        match self {
            DialogueError::Busy { npc: _, npc_name, slot } => format!(
                "{npc_name} discute avec le joueur {} (slot {slot}) — patientez",
                slot + 1
            ),
            DialogueError::OutOfRange => "trop loin — approchez-vous du NPC".into(),
            DialogueError::HeroDead => "impossible de parler : vous êtes mort".into(),
            DialogueError::BadSlot => "slot joueur inconnu".into(),
            DialogueError::BadIndex => "NPC inconnu".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Tables de données — clés string.tbl v1.12 réelles (sonde tbl-fra, d2w-re)
// ---------------------------------------------------------------------------

/// Réplique de DONNE DE QUÊTE : (npc, quête, clé). Le NPC donneur entre en
/// `Avant` tant que sa quête est `Inactive`.
static INIT_QUEST: &[(&str, &str, &str)] = &[
    ("akara", "a1q1", "A1Q1InitAkara"),
    ("kashya", "a1q2", "A1Q2InitKashya"),
    ("charsi", "a1q3", "A1Q3InitCharsi"),
    ("akara", "a1q4", "A1Q4InitAkara"),
    ("cain", "a1q6", "A1Q6InitCain"),
    // a1q5 : donnée par le tome (A1Q5InitQuestTome) — aucun NPC donneur.
];

/// Phase `Pendant` : (npc, quête, clé) — `EarlyReturn` pour le donneur,
/// `AfterInit*` pour le camp (variants retail `CharsiMain`/`Alt` : `Main`).
static PENDANT: &[(&str, &str, &str)] = &[
    // a1q1 — Cain n'est pas au camp avant a1q4 : aucune clé retail (ni TBD
    // affichée : quête sans implication Cain, cf. `involved`).
    ("akara", "a1q1", "A1Q1EarlyReturnAkara"),
    ("kashya", "a1q1", "A1Q1AfterInitKashya"),
    ("charsi", "a1q1", "A1Q1AfterInitCharsiMain"),
    ("gheed", "a1q1", "A1Q1AfterInitGheed"),
    ("warriv", "a1q1", "A1Q1AfterInitWarriv"),
    // a1q2
    ("kashya", "a1q2", "A1Q2EarlyReturnKashya"),
    ("akara", "a1q2", "A1Q2AfterInitAkara"),
    ("charsi", "a1q2", "A1Q2AfterInitCharsi"),
    ("gheed", "a1q2", "A1Q2AfterInitGheed"),
    ("warriv", "a1q2", "A1Q2AfterInitWarriv"),
    // a1q3
    ("charsi", "a1q3", "A1Q3EarlyReturnCharsi"),
    ("akara", "a1q3", "A1Q3AfterInitAkara"),
    ("kashya", "a1q3", "A1Q3AfterInitKashya"),
    ("gheed", "a1q3", "A1Q3AfterInitGheed"),
    ("warriv", "a1q3", "A1Q3AfterInitWarriv"),
    ("cain", "a1q3", "A1Q3AfterInitCain"),
    // a1q4 — étapes scroll retail (AfterInitScroll/EarlyReturnS) condensées
    // en UNE phase `Pendant` du port (documenté) ; Cain : AUCUNE clé retail
    // en phase active ⇒ TBD documenté (test `cain_tbd_documente`).
    ("akara", "a1q4", "A1Q4EarlyReturnAkara"),
    ("kashya", "a1q4", "A1Q4AfterInitScrollKashya"),
    ("charsi", "a1q4", "A1Q4AfterInitScrollCharsi"),
    ("gheed", "a1q4", "A1Q4AfterInitScrollGheed"),
    ("warriv", "a1q4", "A1Q4AfterInitScrollWarriv"),
    // a1q5 (aucun donneur NPC — tome)
    ("akara", "a1q5", "A1Q5AfterInitAkara"),
    ("kashya", "a1q5", "A1Q5AfterInitKashya"),
    ("charsi", "a1q5", "A1Q5AfterInitCharsi"),
    ("gheed", "a1q5", "A1Q5AfterInitGheed"),
    ("warriv", "a1q5", "A1Q5AfterInitWarriv"),
    ("cain", "a1q5", "A1Q5AfterInitCain"),
    // a1q6
    ("cain", "a1q6", "A1Q6EarlyReturnCain"),
    ("akara", "a1q6", "A1Q6AfterInitAkara"),
    ("kashya", "a1q6", "A1Q6AfterInitKashya"),
    ("charsi", "a1q6", "A1Q6AfterInitCharsi"),
    ("gheed", "a1q6", "A1Q6AfterInitGheed"),
    ("warriv", "a1q6", "A1Q6AfterInitWarriv"),
];

/// Phase `Remise` : (npc, quête, clé) — a1q4 = nommage retail
/// `A1Q4QuestSuccessful*` (remise Cain), autres `A1QnSuccessful*`.
static REMISE: &[(&str, &str, &str)] = &[
    ("akara", "a1q1", "A1Q1SuccessfulAkara"),
    ("kashya", "a1q1", "A1Q1SuccessfulKashya"),
    ("charsi", "a1q1", "A1Q1SuccessfulCharsi"),
    ("gheed", "a1q1", "A1Q1SuccessfulGheed"),
    ("warriv", "a1q1", "A1Q1SuccessfulWarriv"),
    ("akara", "a1q2", "A1Q2SuccessfulAkara"),
    ("kashya", "a1q2", "A1Q2SuccessfulKashya"),
    ("charsi", "a1q2", "A1Q2SuccessfulCharsi"),
    ("gheed", "a1q2", "A1Q2SuccessfulGheed"),
    ("warriv", "a1q2", "A1Q2SuccessfulWarriv"),
    ("akara", "a1q3", "A1Q3SuccessfulAkara"),
    ("kashya", "a1q3", "A1Q3SuccessfulKashya"),
    ("charsi", "a1q3", "A1Q3SuccessfulCharsi"),
    ("gheed", "a1q3", "A1Q3SuccessfulGheed"),
    ("warriv", "a1q3", "A1Q3SuccessfulWarriv"),
    ("cain", "a1q3", "A1Q3SuccessfulCain"),
    ("akara", "a1q4", "A1Q4QuestSuccessfulAkara"),
    ("kashya", "a1q4", "A1Q4QuestSuccessfulKashya"),
    ("charsi", "a1q4", "A1Q4QuestSuccessfulCharsi"),
    ("gheed", "a1q4", "A1Q4QuestSuccessfulGheed"),
    ("warriv", "a1q4", "A1Q4QuestSuccessfulWarriv"),
    ("cain", "a1q4", "A1Q4QuestSuccessfulCain"),
    ("akara", "a1q5", "A1Q5SuccessfulAkara"),
    ("kashya", "a1q5", "A1Q5SuccessfulKashya"),
    ("charsi", "a1q5", "A1Q5SuccessfulCharsi"),
    ("gheed", "a1q5", "A1Q5SuccessfulGheed"),
    ("warriv", "a1q5", "A1Q5SuccessfulWarriv"),
    ("cain", "a1q5", "A1Q5SuccessfulCain"),
    ("akara", "a1q6", "A1Q6SuccessfulAkara"),
    ("kashya", "a1q6", "A1Q6SuccessfulKashya"),
    ("charsi", "a1q6", "A1Q6SuccessfulCharsi"),
    ("gheed", "a1q6", "A1Q6SuccessfulGheed"),
    ("warriv", "a1q6", "A1Q6SuccessfulWarriv"),
    ("cain", "a1q6", "A1Q6SuccessfulCain"),
];

/// Ragots (`Apres` / menu « divers ») : intro puis ragots — clés réelles.
/// (Cain : pas de `CainIntroGossip1` dans string.tbl v1.12 — ragots seuls.)
static GOSSIP: &[(&str, &[&str])] = &[
    ("akara", &["AkaraIntroGossip1", "AkaraGossip1", "AkaraGossip2", "AkaraGossip3"]),
    ("kashya", &["KashyaIntroGossip1", "KashyaGossip1", "KashyaGossip2"]),
    ("charsi", &["CharsiIntroGossip1", "CharsiGossip1", "CharsiGossip2"]),
    ("gheed", &["GheedIntroGossip1", "GheedGossip1", "GheedGossip2"]),
    ("warriv", &["WarrivAct1IntroGossip1", "WarrivGossip1", "WarrivGossip2"]),
    ("cain", &["CainGossip1", "CainGossip2", "CainGossip3"]),
];

/// Clés TBL des libellés de menu (réelles, testées).
const K_GOSSIP: &str = "NPCGossipMenu"; // « divers »
const K_MORE: &str = "NPCMenuTalkMore"; // « dites-m'en plus »
const K_LEAVE: &str = "NPCMenuLeave"; // « partir »
const K_TRADE: &str = "NPCMenuTrade"; // « acheter/vendre »
const K_TRADE_REPAIR: &str = "NPCMenuTradeRepair"; // « échanger/réparer »
const K_HIRE: &str = "NPCMenuHire"; // « engager »
const K_TRAVEL_EAST: &str = "WarrivMenu1b"; // « aller vers l'est »
// `WarrivMenu1c` (« aller vers l'ouest ») : retour acte II — non câblé au
// MVP (acte II non jouable), clé citée pour parité.

// ---------------------------------------------------------------------------
// Résolution TBL (runtime — tables fr embarquées, contrat W18)
// ---------------------------------------------------------------------------

/// Tables fr embarquées (OnceLock — décodage une fois par process ; les
/// octets sont validés par les golden tests `tests/tbl_golden.rs`).
fn tbl() -> &'static crate::tbl::TblSet {
    static T: OnceLock<crate::tbl::TblSet> = OnceLock::new();
    T.get_or_init(crate::tbl::TblSet::bundled_fra)
}

/// Résout une clé TBL : préfixe sonore (chiffres + `\n` du format TBL)
/// retiré, soft-wraps internes des répliques retail joints en espaces
/// (bulle mono-ligne du port) ; clé absente ⇒ ligne `TBD…` documentée
/// (jamais de panique).
fn tbl_line(key: &str) -> String {
    match tbl().get(key) {
        Some(v) => {
            // Préfixe sonore : première ligne toute numérique (ex. `45\n…`).
            let body = match v.split_once('\n') {
                Some((head, tail)) if !head.is_empty() && head.bytes().all(|b| b.is_ascii_digit()) => tail,
                _ => v.as_str(),
            };
            body.split('\n').map(str::trim).filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" ")
        }
        None => format!("TBD — clé absente des TBL v1.12 : {key} (cf. dialogue.rs)"),
    }
}

/// Libellé de menu (aucun préfixe sonore sur ces clés — même repli TBD).
fn tbl_label(key: &str) -> String {
    tbl_line(key)
}

fn lines_from(keys: &[&str]) -> (Vec<String>, Vec<String>) {
    (
        keys.iter().map(|k| tbl_line(k)).collect(),
        keys.iter().map(|k| k.to_string()).collect(),
    )
}

/// Ligne TBD documentée (clé retail manquante — mission 22a §3).
fn tbd_lines(npc: &str, quest: &str, phase: DialoguePhase) -> (Vec<String>, Vec<String>) {
    let doc = format!(
        "TBD — aucune clé TBL v1.12 répertoriée pour le dialogue {npc} × {quest} en phase {} (sonde tbl-fra d2w-re ; cf. tables dialogue.rs)",
        phase.as_str()
    );
    (vec![doc.clone()], vec![doc])
}

// ---------------------------------------------------------------------------
// Machine de phases (journal spec — quests_actes.rs)
// ---------------------------------------------------------------------------

/// Le NPC est-il impliqué dans la quête (au moins une réplique retail dans
/// les tables de ce module) ? Cain est absent d'a1q1/a1q2 (pas encore
/// sauvé) — exact retail.
fn involved(npc: &str, quest: &str) -> bool {
    PENDANT.iter().chain(REMISE.iter()).chain(INIT_QUEST.iter()).any(|(n, q, _)| *n == npc && *q == quest)
}

fn init_key(npc: &str, quest: &str) -> Option<&'static str> {
    INIT_QUEST.iter().find(|(n, q, _)| *n == npc && *q == quest).map(|(_, _, k)| *k)
}

fn pendant_key(npc: &str, quest: &str) -> Option<&'static str> {
    PENDANT.iter().find(|(n, q, _)| *n == npc && *q == quest).map(|(_, _, k)| *k)
}

fn remise_key(npc: &str, quest: &str) -> Option<&'static str> {
    REMISE.iter().find(|(n, q, _)| *n == npc && *q == quest).map(|(_, _, k)| *k)
}

/// (quête motrice, phase) d'un NPC — ordre du journal a1q1→a1q6 :
/// 1. première quête impliquant le NPC en `Active`/`RewardPending` ;
/// 2. sinon première quête de donneur encore `Inactive` (`Avant`) ;
/// 3. sinon `Apres` (ragots).
pub fn dialogue_phase_for(npc: &str, act1: Option<&ActQuests>) -> (Option<&'static str>, DialoguePhase) {
    let phase_of = |q: &str| act1.and_then(|l| l.run(q)).map_or(ActPhase::Inactive, |r| r.phase);
    for q in ACT1_QUEST_IDS {
        if !involved(npc, q) {
            continue;
        }
        match phase_of(q) {
            ActPhase::RewardPending => return (Some(q), DialoguePhase::Remise),
            ActPhase::Active => return (Some(q), DialoguePhase::Pendant),
            _ => {}
        }
    }
    for (n, q, _) in INIT_QUEST {
        if *n == npc && phase_of(q) == ActPhase::Inactive {
            return (Some(q), DialoguePhase::Avant);
        }
    }
    (None, DialoguePhase::Apres)
}

/// Condition visible sur le journal spec (aucun journal ⇒ seules `Always`).
pub fn cond_visible(cond: &DialogueCond, act1: Option<&ActQuests>) -> bool {
    match cond {
        DialogueCond::Always => true,
        DialogueCond::QuestDone { quest } => {
            act1.and_then(|l| l.run(quest)).map_or(false, |r| r.phase == ActPhase::Done)
        }
    }
}

// ---------------------------------------------------------------------------
// Construction des nœuds
// ---------------------------------------------------------------------------

/// Options de rôle d'un NPC (libellés TBL réels) + « divers » + « partir ».
fn role_options(npc: &str) -> Vec<DialogueOption> {
    let mut v = vec![opt(K_GOSSIP, Some("gossip1"), DialogueCond::Always, DialogueAction::Say)];
    match npc {
        "akara" => v.push(opt(K_TRADE, None, DialogueCond::Always, shop(VendorId::Akara))),
        "charsi" => v.push(opt(K_TRADE_REPAIR, None, DialogueCond::Always, shop(VendorId::Charsi))),
        "gheed" => v.push(opt(K_TRADE, None, DialogueCond::Always, shop(VendorId::Gheed))),
        "kashya" => v.push(opt(K_HIRE, None, DialogueCond::Always, DialogueAction::HireMercenary)),
        "warriv" => v.push(opt(
            K_TRAVEL_EAST,
            None,
            DialogueCond::QuestDone { quest: "a1q6".into() },
            DialogueAction::Travel,
        )),
        _ => {} // cain : identification « bientôt » (chantier items) — rien
    }
    v.push(opt(K_LEAVE, None, DialogueCond::Always, DialogueAction::Close));
    v
}

fn shop(v: VendorId) -> DialogueAction {
    DialogueAction::OpenShop { vendor: v.name().to_string() }
}

fn opt(label_key: &str, next: Option<&str>, cond: DialogueCond, action: DialogueAction) -> DialogueOption {
    DialogueOption {
        label_key: label_key.to_string(),
        label: tbl_label(label_key),
        next: next.map(str::to_string),
        cond,
        action,
    }
}

/// Nœud `root` : réplique de quête (phase) ou premier ragot (`Apres`).
fn build_root(npc: &str, quest: Option<&str>, phase: DialoguePhase) -> DialogueNode {
    if phase == DialoguePhase::Apres {
        return build_gossip(npc, 0);
    }
    let quest = quest.unwrap_or("a1q1");
    let (lignes, line_keys) = match phase {
        DialoguePhase::Avant => match init_key(npc, quest) {
            Some(k) => lines_from(&[k]),
            None => tbd_lines(npc, quest, phase),
        },
        DialoguePhase::Pendant => match pendant_key(npc, quest) {
            Some(k) => lines_from(&[k]),
            None => tbd_lines(npc, quest, phase),
        },
        DialoguePhase::Remise => match remise_key(npc, quest) {
            Some(k) => lines_from(&[k]),
            None => tbd_lines(npc, quest, phase),
        },
        DialoguePhase::Apres => unreachable!("traité plus haut"),
    };
    DialogueNode { id: "root".into(), line_keys, lignes, options: role_options(npc) }
}

/// Nœud `gossip<i>` (1-based dans l'id) : ragot + menu (« dites-m'en plus »
/// reboucle sur le premier, comme le retail) + options de rôle + « partir ».
fn build_gossip(npc: &str, ix: usize) -> DialogueNode {
    let Some((_, keys)) = GOSSIP.iter().find(|(n, _)| *n == npc) else {
        // NPC sans table (jamais : roster complet) — repli neutre documenté.
        return DialogueNode {
            id: "gossip1".into(),
            line_keys: vec!["TBD".into()],
            lignes: vec![format!("TBD — aucune réplique TBL pour {npc}")],
            options: role_options(npc),
        };
    };
    let key = keys[ix % keys.len()];
    let next_ix = (ix + 1) % keys.len();
    let mut options = vec![opt(
        K_MORE,
        Some(&format!("gossip{}", next_ix + 1)),
        DialogueCond::Always,
        DialogueAction::Say,
    )];
    options.extend(role_options(npc).into_iter().skip(1)); // pas de « divers » dans les ragots
    DialogueNode {
        id: format!("gossip{}", ix + 1),
        line_keys: vec![key.to_string()],
        lignes: vec![tbl_line(key)],
        options,
    }
}

fn gossip_index(id: &str) -> Option<usize> {
    id.strip_prefix("gossip").and_then(|n| n.parse::<usize>().ok()).map(|n| n - 1)
}

// ---------------------------------------------------------------------------
// API World (bloc impl ADDITIF — session transitoire, monde gelé)
// 3c-b coop : variantes _slot (verrou, teneur) + chemins historiques solo.
// ---------------------------------------------------------------------------

impl World {
    /// Ouvre le dialogue du NPC `index` pour le JOUEUR `slot` (3c-b coop) :
    /// même contrat que [`World::talk_to_npc_slot`] (portée 2.5, héros du
    /// slot vivant, soin Akara, `GameEvent::NpcTalked` flushé au tick
    /// suivant) PLUS le journal spec : le `ActEvent::NpcTalked` avance
    /// `quest_act1` (remises NPC — la remise Warriv complète a1q6 et
    /// débloque l'acte II). La phase/le nœud `root` sont capturés AVANT
    /// l'avancement (la réplique `Successful` s'affiche), les CONDITIONS
    /// d'options restent évaluées en vivant.
    ///
    /// VERROU (3c-b) : une session ouverte par un AUTRE slot ⇒
    /// [`DialogueError::Busy`] (monde gelé partagé — une seule conversation
    /// à la fois, cf. § coop en tête) ; le TENEUR peut rouvrir (clic UI =
    /// remplacement, sémantique 22a). `Err` : slot inconnu, index hors
    /// bornes, héros du slot mort ou hors de portée.
    pub fn open_dialogue_slot(
        &mut self,
        slot: usize,
        npc_index: usize,
    ) -> Result<DialogueSession, DialogueError> {
        if slot >= self.hero_count() {
            return Err(DialogueError::BadSlot);
        }
        // VERROU : le teneur remplace SA session (clic UI), les autres slots
        // sont refusés avec la session en cours (bandeau UI).
        if let Some(s) = &self.dialogue {
            if s.slot != slot {
                return Err(DialogueError::Busy {
                    npc: s.npc.clone(),
                    npc_name: s.npc_name.clone(),
                    slot: s.slot,
                });
            }
        }
        let def = *self.town_npc_def(npc_index).ok_or(DialogueError::BadIndex)?; // Copy — borrow rendu
        let npc = def.quest_code;
        // Phase AVANT le journal (remise affichée, cf. doc).
        let (quest, phase) = dialogue_phase_for(npc, self.quests_act1.as_ref());
        // Interaction réelle DU SLOT (portée/mort/soin + NpcTalked journal).
        let _talk = self.talk_to_npc_slot(slot, npc_index).map_err(|e| match e {
            crate::npc::NpcTalkError::OutOfRange => DialogueError::OutOfRange,
            crate::npc::NpcTalkError::HeroDead => DialogueError::HeroDead,
        })?;
        // Journal spec (récompenses appliquées + events ActQuestChanged /
        // ActUnlocked en attente). Sans journal branché : no-op.
        let _rewards = self.quest_act1_event(&ActEvent::NpcTalked { npc: npc.to_string() });
        let sess = DialogueSession {
            slot,
            npc_index,
            npc: npc.to_string(),
            npc_name: def.name.to_string(),
            quest: quest.map(str::to_string),
            phase,
            node: build_root(npc, quest, phase),
        };
        self.dialogue = Some(sess.clone());
        Ok(sess)
    }

    /// Chemin historique (solo / UI sans slot) : le PARLEUR est le héros
    /// vivant le plus proche du NPC (solo = slot 0, strictement identique ;
    /// 3b-1 coop : routage `talk_to_npc` d'origine). Sous verrou : refusé
    /// comme [`World::open_dialogue_slot`] quand un AUTRE slot tient une
    /// session (le monde gelé s'applique à tous). `None` : idem `Err` du
    /// chemin slot (API 22a à `Option`, conservée pour les tests maître).
    pub fn open_dialogue(&mut self, npc_index: usize) -> Option<DialogueSession> {
        let (nx, ny) = {
            let npc = self.npcs.get(npc_index)?;
            (npc.ent.x, npc.ent.y)
        };
        let talker = self.nearest_living_hero_slot(nx, ny);
        self.open_dialogue_slot(talker, npc_index).ok()
    }

    /// Session ouverte (lecture) — `None` = pas de dialogue (monde actif).
    pub fn dialogue_session(&self) -> Option<&DialogueSession> {
        self.dialogue.as_ref()
    }

    /// Monde gelé ? (contrat « pause » — cf. `tick_inputs` : gel GLOBAL
    /// pour tous les slots).
    pub fn dialogue_active(&self) -> bool {
        self.dialogue.is_some()
    }

    /// La session ouverte appartient-elle au slot `slot` ? (garde UI —
    /// seul le teneur voit le panneau de choix ; les autres voient le
    /// bandeau « discute avec »).
    pub fn dialogue_owned_by(&self, slot: usize) -> bool {
        self.dialogue.as_ref().map_or(false, |s| s.slot == slot)
    }

    /// Indices des options VISIBLES du nœud courant (conditions évaluées en
    /// vivant sur le journal spec — la remise Warriv révèle « aller vers
    /// l'est » dans la MÊME session).
    pub fn dialogue_visible_options(&self) -> Vec<usize> {
        let Some(s) = &self.dialogue else { return Vec::new() };
        let act1 = self.quests_act1.as_ref();
        s.node
            .options
            .iter()
            .enumerate()
            .filter(|(_, o)| cond_visible(&o.cond, act1))
            .map(|(i, _)| i)
            .collect()
    }

    /// Choisit l'option `option_ix` (INDEX BRUT dans `node.options` — la
    /// UI choisit dans `dialogue_visible_options`) : applique l'action
    /// (`Close` referme, `Travel` appelle `npc_travel`), avance au nœud
    /// `next` (ragots). `None` : pas de session, option inconnue ou
    /// condition NON remplie (session inchangée). Chemin historique (sans
    /// garde de slot — tests + UI solo) ; le pont coop passe par
    /// [`World::choose_dialogue_slot`].
    pub fn choose_dialogue(&mut self, option_ix: usize) -> Option<DialogueAction> {
        let (npc, opt) = {
            let sess = self.dialogue.as_ref()?;
            let opt = sess.node.options.get(option_ix)?.clone();
            if !cond_visible(&opt.cond, self.quests_act1.as_ref()) {
                return None; // condition quête non remplie
            }
            (sess.npc.clone(), opt)
        };
        match opt.action {
            DialogueAction::Close => self.dialogue = None,
            DialogueAction::Travel => {
                // API existante (MVP acte II : NotAvailable documenté).
                self.npc_travel();
            }
            // OpenShop/HireMercenary/Repair : consommés par l'UI via les
            // API déjà branchées (vendor.rs / hire_merc / npc_repair) —
            // aucun effet moteur ici (documenté). 3c-b : l'UI n'ouvre le
            // panneau vendeur que sur le CLIENT du teneur (slot vérifié
            // côté pont + page).
            DialogueAction::Say | DialogueAction::OpenShop { .. } | DialogueAction::HireMercenary
            | DialogueAction::Repair => {}
        }
        if let (Some(next), Some(sess)) = (&opt.next, self.dialogue.as_mut()) {
            if let Some(ix) = gossip_index(next) {
                sess.node = build_gossip(&npc, ix);
            }
        }
        Some(opt.action)
    }

    /// 3c-b (coop) : choix réservé au TENEUR de la session — un autre slot
    /// (ou sans session) reçoit `None`, session inchangée.
    pub fn choose_dialogue_slot(&mut self, slot: usize, option_ix: usize) -> Option<DialogueAction> {
        if !self.dialogue_owned_by(slot) {
            return None; // verrou : seul le teneur choisit
        }
        self.choose_dialogue(option_ix)
    }

    /// Referme la session (« partir » / Échap UI) — `true` si une session
    /// a été fermée. Le monde reprend son tick. Chemin historique (sans
    /// garde — tests + UI solo).
    pub fn close_dialogue(&mut self) -> bool {
        self.dialogue.take().is_some()
    }

    /// 3c-b (coop) : fermeture réservée au TENEUR — `false` sans session
    /// ou si la session appartient à un autre slot (session inchangée).
    pub fn close_dialogue_slot(&mut self, slot: usize) -> bool {
        if !self.dialogue_owned_by(slot) {
            return false;
        }
        self.close_dialogue()
    }
}

// ---------------------------------------------------------------------------
// Tests — chaînes complètes × phases, clés TBL citées, monde gelé
// ---------------------------------------------------------------------------

/// Fichier vocal (basename minuscule, relatif à `demo/assets/sfx/speech/`)
/// attendu pour une clé TBL parlée — règle dérivée du listfile réel :
/// * `{Npc}IntroGossip1` / `WarrivAct1IntroGossip1` → `{p}_act1_intro.wav` ;
/// * `{Npc}Gossip<i>` → `{p}_act1_gossip_{i:02}.wav` ;
/// * `A1Q<n>{Init|EarlyReturn|AfterInit|AfterInitScroll|Successful|
///   QuestSuccessful}{Npc}` → `{p}_act1_q<n>_<variante>.wav`, variant retail
///   `A1Q1AfterInitCharsiMain` → `cha_act1_q1_after_main.wav`.
///
/// `None` : clé hors périmètre parlé (libellés de menu, `TBD…`, clés
/// étrangères). Miroir EXACT de `tools/extract_speech.py::voice_file_for`
/// (le script génère `index.json` depuis CES tables ; le golden test
/// `tests/speech_golden.rs` croise moteur × index × wavs sur disque).
/// Préfixe de fichier vocal par NPC — nommage RÉEL du listfile d2speech.mpq
/// (`data\local\sfx\Act1\Akara\Aka_act1_…`, `Cain\Cain_act1_…`, énuméré par
/// SFileFindFirstFile à l'étape 23a).

/// Préfixe de fichier vocal par NPC — nommage RÉEL du listfile d2speech.mpq
/// (`data\local\sfx\Act1\Akara\Aka_act1_…`, `Cain\Cain_act1_…`, énuméré par
/// SFileFindFirstFile à l'étape 23a).

/// Préfixe de fichier vocal par NPC — nommage RÉEL du listfile d2speech.mpq
/// (`data\local\sfx\Act1\Akara\Aka_act1_…`, `Cain\Cain_act1_…`, énuméré par
/// SFileFindFirstFile à l'étape 23a).

static VOICE_PREFIX: &[(&str,&str)] = &[];

pub fn voice_file_for(key: &str) -> Option<String> {
    // Ragots : le nom du NPC est préfixe (AkaraIntroGossip1, CainGossip2…).
    for (name, p) in VOICE_PREFIX {
        if let Some(rest) = key.strip_prefix(name) {
            // Intro : `IntroGossip1` (Akara…) comme `Act1IntroGossip1`
            // (Warriv — nommage retail du listfile).
            if rest.ends_with("IntroGossip1") {
                return Some(format!("{p}_act1_intro.wav"));
            }
            if let Some(num) = rest.strip_prefix("Gossip") {
                let n: u32 = num.parse().ok()?;
                return Some(format!("{p}_act1_gossip_{n:02}.wav"));
            }
        }
    }
    // Quêtes : `A1Q<n><Variante><Npc>` — le nom du NPC est suffixe.
    let rest = key.strip_prefix("A1Q")?;
    let head = rest.chars().next()?;
    if !head.is_ascii_digit() {
        return None;
    }
    let q = head; // quêtes acte I : a1q1…a1q6, un seul chiffre
    let rest = &rest[1..];
    for (name, p) in VOICE_PREFIX {
        // variant retail `…CharsiMain` (a1q1) : suffixe étendu en second.
        for (suffix, tail) in [(name.to_string(), ""), (format!("{name}Main"), "_main")] {
            let Some(mid) = rest.strip_suffix(suffix.as_str()) else { continue };
            let base = match mid {
                "Init" => "init",
                "EarlyReturn" => "early",
                "AfterInit" => "after",
                "AfterInitScroll" => "after_scroll",
                "Successful" => "successful",
                "QuestSuccessful" => "successful",
                _ => continue,
            };
            return Some(format!("{p}_act1_q{q}_{base}{tail}.wav"));
        }
    }
    None
}

/// Inventaire des clés PARLÉES du moteur : INIT + PENDANT + REMISE puis
/// GOSSIP, dédupliquées en conservant l'ordre des tables (91 clés v1.12) —
/// consommé par le golden test `tests/speech_golden.rs` (couverture voix).
pub fn all_line_keys() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    let mut push = |out: &mut Vec<&'static str>, k: &'static str| {
        if !out.contains(&k) {
            out.push(k);
        }
    };
    for (_, _, k) in INIT_QUEST {
        push(&mut out, k);
    }
    for (_, _, k) in PENDANT {
        push(&mut out, k);
    }
    for (_, _, k) in REMISE {
        push(&mut out, k);
    }
    for (_, keys) in GOSSIP {
        for k in *keys {
            push(&mut out, k);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quests_actes::ActQuests;
    use serde_json::json;

    /// Grande ville toute marchable + roster + journal spec branché,
    /// héros à côté d'Akara (portée 2.5).
    fn world() -> World {
        let mut w = World::with_seed(vec![vec![false; 300]; 300], json!({}), 0xD1A7);
        w.spawn_town_npcs();
        w.set_quest_act1(ActQuests::new());
        let (ax, ay) = { let e = &w.town_npcs()[0].ent; (e.x, e.y) };
        w.place_hero_at(ax + 1.0, ay);
        w
    }

    /// Index du roster par code quête.
    fn ix(w: &World, npc: &str) -> usize {
        w.town_npcs().iter().position(|n| crate::npc::NPCS[n.def].quest_code == npc).unwrap()
    }

    /// Place le héros à côté du NPC (portée de dialogue 2.5).
    fn near(w: &mut World, npc: &str) {
        let i = ix(w, npc);
        let (x, y) = { let e = &w.town_npcs()[i].ent; (e.x, e.y) };
        w.place_hero_at(x + 1.0, y);
    }

    /// Phase de la quête `id` dans le journal du monde.
    fn qphase(w: &World, id: &str) -> ActPhase {
        w.quest_act1().unwrap().run(id).unwrap().phase
    }

    fn zone(w: &mut World, id: u32) {
        w.quest_act1_event(&ActEvent::ZoneEntered { level_id: id });
    }
    fn kill(w: &mut World, code: &str) {
        w.quest_act1_event(&ActEvent::MonsterKilled { code: code.into(), superunique: None });
    }

    // --- données : 100 % des clés citées existent (mission 3) -------------

    #[test]
    fn toutes_les_cles_citees_resolvent_dans_les_tbl_reelles() {
        for (n, q, k) in INIT_QUEST.iter().chain(PENDANT.iter()).chain(REMISE.iter()) {
            let l = tbl_line(k);
            assert!(!l.starts_with("TBD"), "{n}×{q} : clé {k} manquante — {l}");
            assert!(!l.contains('\n'), "{k} : préfixe sonore non retiré ? {l:?}");
        }
        for (n, keys) in GOSSIP.iter() {
            for k in keys.iter() {
                assert!(!tbl_line(k).starts_with("TBD"), "{n} : clé ragot {k} manquante");
            }
        }
        for k in [K_GOSSIP, K_MORE, K_LEAVE, K_TRADE, K_TRADE_REPAIR, K_HIRE, K_TRAVEL_EAST] {
            assert!(!tbl_label(k).starts_with("TBD"), "libellé {k} manquant");
        }
        // Citations littérales (clés => texte fr réel).
        assert_eq!(tbl_label(K_TRADE), "acheter/vendre");
        assert_eq!(tbl_label(K_TRADE_REPAIR), "échanger/réparer");
        assert_eq!(tbl_label(K_TRAVEL_EAST), "aller vers l'est");
        assert_eq!(tbl_label(K_LEAVE), "partir");
        assert_eq!(tbl_label(K_MORE), "dites-m'en plus");
        assert_eq!(tbl_label(K_GOSSIP), "divers");
        assert_eq!(tbl_label(K_HIRE), "engager");
        assert!(tbl_line("AkaraIntroGossip1").starts_with("Mon nom est Akara"));
        assert!(tbl_line("A1Q6SuccessfulWarriv").starts_with("La caravane est prête"));
    }

    // --- Akara : chaîne complète a1q1 avant → pendant → remise → après ----

    #[test]
    fn akara_chaine_complete_a1q1_quatre_phases() {
        let mut w = world();
        // AVANT : Akara donne la quête (A1Q1InitAkara) — le NpcTalked
        // d'ouverture ACTIVE a1q1 dans le journal spec.
        let s = w.open_dialogue(ix(&w, "akara")).expect("ouverture Akara");
        assert_eq!(s.npc, "akara");
        assert_eq!(s.quest.as_deref(), Some("a1q1"));
        assert_eq!(s.phase, DialoguePhase::Avant);
        assert_eq!(s.node.id, "root");
        assert_eq!(s.node.line_keys, vec!["A1Q1InitAkara"]);
        assert!(s.node.lignes[0].starts_with("Au beau milieu de la jungle"));
        // Menu complet au root : divers / acheter-vendre / partir.
        assert_eq!(w.dialogue_visible_options(), vec![0, 1, 2]);
        assert_eq!(s.node.options[0].label_key, K_GOSSIP);
        assert_eq!(s.node.options[1].label, "acheter/vendre");
        assert_eq!(s.node.options[2].label, "partir");
        assert_eq!(qphase(&w, "a1q1"), ActPhase::Active);
        // Les events (NpcTalked + ActQuestChanged) sortent au tick suivant.
        let ev = w.tick(16, None);
        assert!(ev.iter().any(|e| matches!(e, crate::GameEvent::NpcTalked { ref npc } if npc == "akara")));
        assert!(ev.iter().any(|e| matches!(e, crate::GameEvent::ActQuestChanged { ref id, phase } if id == "a1q1" && phase == "active")));
        // PENDANT : relance du donneur (A1Q1EarlyReturnAkara).
        let s = w.open_dialogue(ix(&w, "akara")).expect("réouverture");
        assert_eq!(s.phase, DialoguePhase::Pendant);
        assert_eq!(s.node.line_keys, vec!["A1Q1EarlyReturnAkara"]);
        assert!(s.node.lignes[0].starts_with("Votre mission ne sera terminée"));
        // Progression réelle du journal : zone → 6 kills → RewardPending.
        zone(&mut w, 8);
        for _ in 0..6 {
            kill(&mut w, "zombie1");
        }
        assert_eq!(qphase(&w, "a1q1"), ActPhase::RewardPending);
        // REMISE : réplique Successful — le NpcTalked d'ouverture complète
        // la quête (point de compétence appliqué).
        let pts_avant = w.unspent_skill_points();
        let s = w.open_dialogue(ix(&w, "akara")).expect("remise");
        assert_eq!(s.phase, DialoguePhase::Remise);
        assert_eq!(s.node.line_keys, vec!["A1Q1SuccessfulAkara"]);
        assert!(s.node.lignes[0].starts_with("Vous avez rayé cette grotte"));
        assert_eq!(qphase(&w, "a1q1"), ActPhase::Done);
        assert_eq!(w.unspent_skill_points(), pts_avant + 1, "récompense a1q1");
        // Quête suivante du donneur : Akara enchaîne sur a1q4 (AVANT, donne
        // la quête — elle s'activera à l'entrée Dark Wood, comme le retail).
        let s = w.open_dialogue(ix(&w, "akara")).expect("quête suivante");
        assert_eq!(s.quest.as_deref(), Some("a1q4"));
        assert_eq!(s.phase, DialoguePhase::Avant);
        assert_eq!(s.node.line_keys, vec!["A1Q4InitAkara"]);
        // a1q4 Active : relance du donneur (EarlyReturnAkara).
        zone(&mut w, 5); // Dark Wood (activation étape 1)
        let s = w.open_dialogue(ix(&w, "akara")).unwrap();
        assert_eq!(s.phase, DialoguePhase::Pendant);
        assert_eq!(s.node.line_keys, vec!["A1Q4EarlyReturnAkara"]);
        w.close_dialogue();
    }

    // --- Kashya : a1q2 (donneuse + remise) --------------------------------

    #[test]
    fn kashya_chaine_complete_a1q2() {
        let mut w = world();
        near(&mut w, "kashya");
        let k = ix(&w, "kashya");
        let s = w.open_dialogue(k).expect("ouverture Kashya");
        assert_eq!(s.node.line_keys, vec!["A1Q2InitKashya"]);
        assert!(s.node.lignes[0].starts_with("Mes Rogues m'apprennent une funeste nouvelle"));
        // L'étape 0 de a1q2 est un KILL : parler ne l'active pas (retail).
        assert_eq!(qphase(&w, "a1q2"), ActPhase::Inactive);
        let s = w.open_dialogue(k).unwrap();
        assert_eq!(s.phase, DialoguePhase::Avant, "Inactive ⇒ toujours la donne");
        // Blood Raven tombe → RewardPending direct (quête 1 étape, remise
        // Kashya), la réplique retail est celle de REMISE.
        kill(&mut w, "bloodraven");
        assert_eq!(qphase(&w, "a1q2"), ActPhase::RewardPending);
        let s = w.open_dialogue(k).unwrap();
        assert_eq!(s.phase, DialoguePhase::Remise);
        assert_eq!(s.node.line_keys, vec!["A1Q2SuccessfulKashya"]);
        assert_eq!(qphase(&w, "a1q2"), ActPhase::Done, "remise par le NpcTalked d'ouverture");
        // Menu « engager » (libellé réel) présent dans les ragots + nav.
        let s = w.open_dialogue(k).unwrap();
        assert_eq!(s.phase, DialoguePhase::Apres);
        assert_eq!(s.node.id, "gossip1");
        assert_eq!(s.node.line_keys, vec!["KashyaIntroGossip1"]);
        let hire = s.node.options.iter().find(|o| o.action == DialogueAction::HireMercenary).unwrap();
        assert_eq!(hire.label_key, K_HIRE);
        assert_eq!(hire.label, "engager");
        // Navigation ragots : gossip1 → gossip2 (« dites-m'en plus »).
        assert_eq!(w.choose_dialogue(0), Some(DialogueAction::Say));
        assert_eq!(w.dialogue_session().unwrap().node.line_keys, vec!["KashyaGossip1"]);
        // « partir » referme (dernière option du nœud).
        let last = w.dialogue_session().unwrap().node.options.len() - 1;
        assert_eq!(w.choose_dialogue(last), Some(DialogueAction::Close));
        assert!(!w.dialogue_active());
    }

    // --- Charsi : a1q3 (smith → malus → imbu) -----------------------------

    #[test]
    fn charsi_chaine_complete_a1q3() {
        let mut w = world();
        near(&mut w, "charsi");
        let c = ix(&w, "charsi");
        let s = w.open_dialogue(c).expect("ouverture Charsi");
        assert_eq!(s.node.line_keys, vec!["A1Q3InitCharsi"]);
        assert!(s.node.lignes[0].starts_with("Lorsque je me suis enfuie du Monastère"));
        // Smith → Malus → RewardPending (remise Charsi).
        kill(&mut w, "smith");
        w.quest_act1_event(&ActEvent::ItemPicked { code: "hdm".into() });
        assert_eq!(qphase(&w, "a1q3"), ActPhase::RewardPending);
        let s = w.open_dialogue(c).unwrap();
        assert_eq!(s.phase, DialoguePhase::Remise);
        assert_eq!(s.node.line_keys, vec!["A1Q3SuccessfulCharsi"]);
        assert_eq!(qphase(&w, "a1q3"), ActPhase::Done);
        // Libellé forgeron réel : « échanger/réparer » (NPCMenuTradeRepair).
        let s = w.open_dialogue(c).unwrap();
        assert_eq!(s.node.id, "gossip1");
        assert_eq!(s.node.line_keys, vec!["CharsiIntroGossip1"]);
        let shop = s
            .node
            .options
            .iter()
            .find(|o| matches!(o.action, DialogueAction::OpenShop { .. }))
            .unwrap();
        assert_eq!(shop.label_key, K_TRADE_REPAIR);
        assert_eq!(shop.label, "échanger/réparer");
        assert_eq!(w.choose_dialogue(w.dialogue_visible_options()[1]).map(|a| matches!(a, DialogueAction::OpenShop { ref vendor } if vendor == "Charsi")), Some(true));
    }

    // --- Warriv : ragots, gating voyage a1q6 Done (mission 2) -------------

    #[test]
    fn warriv_voyage_gated_sur_a1q6_done_et_remise_dans_la_meme_session() {
        let mut w = world();
        near(&mut w, "warriv");
        let v = ix(&w, "warriv");
        // Sans quête : ragots d'intro, PAS d'option voyage (cond non remplie).
        let s = w.open_dialogue(v).expect("ouverture Warriv");
        assert_eq!(s.phase, DialoguePhase::Apres, "Warriv n'est donneur d'aucune quête");
        assert_eq!(s.node.line_keys, vec!["WarrivAct1IntroGossip1"]);
        let vis = w.dialogue_visible_options();
        assert!(!vis.iter().any(|&i| w.dialogue_session().unwrap().node.options[i].action == DialogueAction::Travel));
        // Navigation ragots avec rebouclage retail (« dites-m'en plus ») :
        // gossip1 → gossip2 → gossip3 → gossip1.
        assert_eq!(w.choose_dialogue(0), Some(DialogueAction::Say));
        assert_eq!(w.dialogue_session().unwrap().node.line_keys, vec!["WarrivGossip1"]);
        w.choose_dialogue(0);
        assert_eq!(w.dialogue_session().unwrap().node.line_keys, vec!["WarrivGossip2"]);
        w.choose_dialogue(0);
        assert_eq!(w.dialogue_session().unwrap().node.line_keys, vec!["WarrivAct1IntroGossip1"]);
        w.close_dialogue();
        // La condition seule : a1q6 pas Done ⇒ invisible (helper public).
        assert!(!cond_visible(&DialogueCond::QuestDone { quest: "a1q6".into() }, w.quest_act1()));
        // Andariel tombe (zone 37 → kill) : RewardPending, cond toujours fausse.
        zone(&mut w, 37);
        kill(&mut w, "andariel");
        assert_eq!(qphase(&w, "a1q6"), ActPhase::RewardPending);
        assert!(!cond_visible(&DialogueCond::QuestDone { quest: "a1q6".into() }, w.quest_act1()));
        // REMISE : la réplique retail s'affiche ET le NpcTalked d'ouverture
        // complète a1q6 → la condition devient vraie DANS la session.
        let s = w.open_dialogue(v).unwrap();
        assert_eq!(s.phase, DialoguePhase::Remise);
        assert_eq!(s.node.line_keys, vec!["A1Q6SuccessfulWarriv"]);
        assert!(s.node.lignes[0].starts_with("La caravane est prête"));
        assert_eq!(qphase(&w, "a1q6"), ActPhase::Done);
        let vis = w.dialogue_visible_options();
        let travel_ix = *vis
            .iter()
            .find(|&&i| w.dialogue_session().unwrap().node.options[i].action == DialogueAction::Travel)
            .expect("voyage révélé après la remise");
        // L'acte II est débloqué (gating spec) — event au prochain tick.
        assert!(w.quest_act1().unwrap().access_act(2));
        assert!(w.tick(16, None).iter().any(|e| matches!(e, crate::GameEvent::ActUnlocked { act: 2 })));
        // Choix du voyage : action rendue + API npc_travel() appelée.
        assert_eq!(w.choose_dialogue(travel_ix), Some(DialogueAction::Travel));
        assert!(w.dialogue_active(), "le voyage ne referme pas la session");
        // Nouvelle session (Apres) : voyage toujours visible (a1q6 Done).
        let s = w.open_dialogue(v).unwrap();
        assert_eq!(s.phase, DialoguePhase::Apres);
        assert!(w.dialogue_visible_options().iter().any(|&i| {
            w.dialogue_session().unwrap().node.options[i].action == DialogueAction::Travel
        }));
    }

    // --- Gheed / Cain : quêtes d'AUTRES (AfterInit) + TBD documenté -------

    #[test]
    fn gheed_commente_la_quete_du_camp_et_cain_tbd_documente() {
        let mut w = world();
        // a1q5 (tour, aucun donneur NPC) Active : Gheed commente (AfterInit).
        zone(&mut w, 20);
        assert_eq!(qphase(&w, "a1q5"), ActPhase::Active);
        near(&mut w, "gheed");
        let s = w.open_dialogue(ix(&w, "gheed")).expect("ouverture Gheed");
        assert_eq!(s.phase, DialoguePhase::Pendant);
        assert_eq!(s.node.line_keys, vec!["A1Q5AfterInitGheed"]);
        // Cain × a1q4 Active : AUCUNE clé retail en phase Pendant ⇒ TBD
        // documenté (mission 3 : pas de texte inventé).
        w.quest_act1_event(&ActEvent::ZoneEntered { level_id: 5 }); // a1q4 : Dark Wood
        near(&mut w, "cain");
        let s = w.open_dialogue(ix(&w, "cain")).expect("ouverture Cain");
        assert_eq!(s.phase, DialoguePhase::Pendant);
        assert!(s.node.lignes[0].starts_with("TBD —"), "pas de texte inventé : {:?}", s.node.lignes);
        assert!(s.node.lignes[0].contains("cain") && s.node.lignes[0].contains("a1q4"));
        // Cain neuf (a1q4 non démarrée) : il DONNE a1q6 (A1Q6InitCain).
        let mut w2 = world();
        near(&mut w2, "cain");
        let s = w2.open_dialogue(ix(&w2, "cain")).unwrap();
        assert_eq!(s.phase, DialoguePhase::Avant);
        assert_eq!(s.node.line_keys, vec!["A1Q6InitCain"]);
        assert!(s.node.lignes[0].starts_with("Andarielle, reine des Enfers"));
    }

    // --- Monde gelé pendant la session (mission 4) -------------------------

    #[test]
    fn tick_gelé_pendant_la_session_et_repris_apres_partir() {
        let mut w = world();
        let a = ix(&w, "akara");
        w.open_dialogue(a).expect("session");
        let (hx, hy) = { let e = w.hero(); (e.x, e.y) };
        let (nx, ny, nwait) = {
            let n = &w.town_npcs()[a];
            (n.ent.x, n.ent.y, n.wait_ms)
        };
        // Ticks pendant la session : RIEN ne bouge, horloge figée, seuls les
        // events en attente sortent (NpcTalked de l'ouverture).
        let ev = w.tick(100, Some(crate::PlayerInput::MoveTo { x: 250, y: 250 }));
        assert!(ev.iter().any(|e| matches!(e, crate::GameEvent::NpcTalked { .. })));
        assert_eq!((w.hero().x, w.hero().y), (hx, hy), "héros gelé");
        assert_eq!((w.town_npcs()[a].ent.x, w.town_npcs()[a].ent.y, w.town_npcs()[a].wait_ms), (nx, ny, nwait));
        assert!(w.tick(100, None).is_empty(), "plus rien à flusher, monde gelé");
        // « partir » : le monde reprend (le clic MoveTo déplace le héros).
        assert!(w.close_dialogue());
        assert!(!w.close_dialogue(), "double fermeture : false");
        w.tick(100, Some(crate::PlayerInput::MoveTo { x: 250, y: 250 }));
        w.tick(100, Some(crate::PlayerInput::MoveTo { x: 250, y: 250 }));
        assert_ne!((w.hero().x, w.hero().y), (hx, hy), "clic MoveTo repris après la fermeture");
    }

    // --- Erreurs propres + helpers publics ---------------------------------

    #[test]
    fn ouvertures_et_choix_invalides_sans_panique() {
        let mut w = world();
        // Hors de portée : None (héros au spawn du camp).
        let mut far = World::with_seed(vec![vec![false; 300]; 300], json!({}), 0xD1A7);
        far.spawn_town_npcs();
        far.set_quest_act1(ActQuests::new());
        assert!(far.open_dialogue(ix(&far, "akara")).is_none(), "trop loin");
        assert!(far.open_dialogue(999).is_none(), "hors roster");
        // Sans session : choose/close inopérants.
        assert_eq!(w.choose_dialogue(0), None);
        assert!(!w.close_dialogue());
        // Session ouverte : index inconnu refusé, condition non remplie
        // refusée (l'option voyage invisible d'un NPC non éligible).
        near(&mut w, "gheed");
        w.open_dialogue(ix(&w, "gheed")).unwrap(); // Gheed : pas d'option voyage
        assert_eq!(w.choose_dialogue(99), None, "hors options");
        let n_opts = w.dialogue_session().unwrap().node.options.len();
        let _ = n_opts;
        w.close_dialogue();
        // Sans journal spec branché : phases par défaut (Avant/Apres) et
        // cond QuestDone jamais visible.
        let mut w2 = World::with_seed(vec![vec![false; 300]; 300], json!({}), 7);
        w2.spawn_town_npcs();
        let (ax, ay) = { let e = &w2.town_npcs()[0].ent; (e.x, e.y) };
        w2.place_hero_at(ax + 1.0, ay);
        assert!(!cond_visible(&DialogueCond::QuestDone { quest: "a1q1".into() }, w2.quest_act1()));
        let s = w2.open_dialogue(ix(&w2, "akara")).unwrap();
        assert_eq!(s.phase, DialoguePhase::Avant);
    }

    // --- Machine de phases : priorités (journal ordre a1q1→a1q6) -----------

    #[test]
    fn machine_de_phases_premiere_quete_en_cours_puis_donneur_puis_ragots() {
        let mut log = ActQuests::new();
        // Tout Inactive : Akara donne a1q1 (premier donneur Inactive).
        assert_eq!(dialogue_phase_for("akara", Some(&log)), (Some("a1q1"), DialoguePhase::Avant));
        // Warriv/Gheed ne donnent rien ⇒ ragots.
        assert_eq!(dialogue_phase_for("warriv", Some(&log)), (None, DialoguePhase::Apres));
        assert_eq!(dialogue_phase_for("gheed", Some(&log)), (None, DialoguePhase::Apres));
        // a1q2 single-step (Kill Blood Raven) : kill ⇒ RewardPending direct
        // (remise Kashya), commentée par tout le camp en Remise aussi.
        log.on_event(&ActEvent::MonsterKilled { code: "bloodraven".into(), superunique: None });
        assert_eq!(dialogue_phase_for("kashya", Some(&log)), (Some("a1q2"), DialoguePhase::Remise));
        assert_eq!(dialogue_phase_for("akara", Some(&log)), (Some("a1q2"), DialoguePhase::Remise));
        // Remise prise (NpcTalked Kashya) : a1q2 Done ⇒ Kashya en ragots,
        // Akara retombe sur sa quête de donneur encore Inactive (a1q1).
        log.on_event(&ActEvent::NpcTalked { npc: "kashya".into() });
        assert_eq!(dialogue_phase_for("kashya", Some(&log)), (None, DialoguePhase::Apres));
        assert_eq!(dialogue_phase_for("akara", Some(&log)), (Some("a1q1"), DialoguePhase::Avant));
        // Akara a1q1 Done + a1q4 Inactive ⇒ Avant a1q4 (deuxième donne).
        let mut log = ActQuests::new();
        log.on_event(&ActEvent::NpcTalked { npc: "akara".into() }); // active
        for id in [8u32] {
            log.on_event(&ActEvent::ZoneEntered { level_id: id });
        }
        for _ in 0..6 {
            log.on_event(&ActEvent::MonsterKilled { code: "zombie1".into(), superunique: None });
        }
        log.on_event(&ActEvent::NpcTalked { npc: "akara".into() }); // done
        assert_eq!(dialogue_phase_for("akara", Some(&log)), (Some("a1q4"), DialoguePhase::Avant));
        // Sans journal : quêtes Inactive ⇒ comportement « Avant/Apres » neuf.
        assert_eq!(dialogue_phase_for("charsi", None), (Some("a1q3"), DialoguePhase::Avant));
        assert_eq!(dialogue_phase_for("cain", None), (Some("a1q6"), DialoguePhase::Avant));
    }

    // --- Session sérialisable (contrat JSON d2game futur) ------------------

    #[test]
    fn session_serde_roundtrip_fidele() {
        let mut w = world();
        let s = w.open_dialogue(ix(&w, "akara")).unwrap();
        let j = serde_json::to_string(&s).expect("sérialisation");
        let back: DialogueSession = serde_json::from_str(&j).expect("désérialisation");
        assert_eq!(back, s);
        assert!(j.contains("\"phase\":\"avant\""));
        assert!(j.contains("A1Q1InitAkara"), "clé TBL citée dans le JSON");
    }

    // --- 3c-b COOP : verrou, gel global, teneur seul ------------------------

    /// Monde 2 slots (BA + SOR), les DEUX héros à côté d'Akara (portée
    /// 2.5), journal spec branché — socle des tests coop de ce module.
    fn world2() -> World {
        let mut w = World::with_seed(vec![vec![false; 300]; 300], json!({}), 0xD1A7);
        w.spawn_town_npcs();
        w.set_quest_act1(ActQuests::new());
        assert!(w.add_player(1, "SOR"), "slot 1 ajouté");
        let (ax, ay) = { let e = &w.town_npcs()[0].ent; (e.x, e.y) };
        for slot in 0..2 {
            w.players[slot].ent.x = ax + 1.0 + slot as f32;
            w.players[slot].ent.y = ay;
        }
        w
    }

    #[test]
    fn coop_verrou_une_seule_session_le_teneur_peut_rouvrir() {
        let mut w = world2();
        let a = ix(&w, "akara");
        let s1 = w.open_dialogue_slot(1, a).expect("slot 1 ouvre Akara");
        assert_eq!(s1.slot, 1, "la session porte son teneur");
        // VERROU : le slot 0 ne peut ouvrir NI le même NPC NI un autre —
        // le monde gelé est partagé ; Busy porte la session en cours
        // (bandeau « <NPC> discute avec <joueur> ») AVANT tout test de
        // portée (le bandeau s'affiche même de loin).
        for target in [a, ix(&w, "gheed")] {
            match w.open_dialogue_slot(0, target) {
                Err(DialogueError::Busy { npc, slot, .. }) => {
                    assert_eq!(npc, "akara");
                    assert_eq!(slot, 1);
                }
                other => panic!("verrou attendu, obtenu : {:?}", other.map(|s| s.npc)),
            }
        }
        // Chemin historique (solo/nearest) : même verrou.
        assert!(w.open_dialogue(ix(&w, "gheed")).is_none(), "verrou via open_dialogue");
        // Le TENEUR rouvre (clic UI) : remplacement, pas d'erreur.
        assert!(w.open_dialogue_slot(1, a).is_ok(), "le teneur remplace sa session");
        // Fermeture par un autre slot : refusée ; par le teneur : ok.
        assert!(!w.close_dialogue_slot(0));
        assert!(w.close_dialogue_slot(1));
        assert!(!w.dialogue_active(), "verrou levé");
        // Le slot 0 ouvre à son tour (présenté près du NPC visé).
        let (gx, gy) = { let e = &w.town_npcs()[ix(&w, "gheed")].ent; (e.x, e.y) };
        w.players[0].ent.x = gx + 1.0;
        w.players[0].ent.y = gy;
        assert!(w.open_dialogue_slot(0, ix(&w, "gheed")).is_ok(), "après « partir », le verrou passe au slot 0");
    }

    #[test]
    fn coop_gel_global_tous_les_slots() {
        let mut w = world2();
        let a = ix(&w, "akara");
        w.open_dialogue_slot(1, a).expect("session slot 1");
        let avant: Vec<(f32, f32)> =
            (0..2).map(|s| { let e = w.hero_slot_ent(s).unwrap(); (e.x, e.y) }).collect();
        let now0 = w.now_ms;
        // Entrées des DEUX slots pendant la session : RIEN ne bouge, ni
        // l'horloge ; seuls les events en attente sortent (NpcTalked).
        let ev = w.tick_inputs(
            100,
            &[
                (0, crate::PlayerInput::MoveTo { x: 250, y: 250 }),
                (1, crate::PlayerInput::MoveTo { x: 5, y: 5 }),
            ],
        );
        assert!(
            ev.iter().any(|e| matches!(e, crate::GameEvent::NpcTalked { .. })),
            "events de l'ouverture flushés même monde gelé"
        );
        for (s, (x0, y0)) in avant.iter().enumerate() {
            let e = w.hero_slot_ent(s).unwrap();
            assert_eq!((e.x, e.y), (*x0, *y0), "héros du slot {s} gelé (gel GLOBAL)");
        }
        assert_eq!(w.now_ms, now0, "horloge gelée pour tous les slots");
        assert!(w.tick_inputs(100, &[]).is_empty(), "plus rien à flusher, monde gelé");
        // « partir » du TENEUR : le monde reprend pour TOUS les slots.
        assert!(w.close_dialogue_slot(1));
        for _ in 0..2 {
            w.tick_inputs(
                100,
                &[
                    (0, crate::PlayerInput::MoveTo { x: 250, y: 250 }),
                    (1, crate::PlayerInput::MoveTo { x: 5, y: 5 }),
                ],
            );
        }
        for (s, (x0, y0)) in avant.iter().enumerate() {
            let e = w.hero_slot_ent(s).unwrap();
            assert_ne!((e.x, e.y), (*x0, *y0), "slot {s} redémarre après la fermeture");
        }
        assert!(w.now_ms > now0, "horloge rendue au monde");
    }

    #[test]
    fn coop_seul_le_teneur_choisit_et_le_soin_va_au_parleur() {
        let mut w = world2();
        let a = ix(&w, "akara");
        // Les DEUX blessés : le soin d'Akara doit aller au PARLEUR (slot 1)
        // et à lui seul (point (3) mission — le panneau/effet est celui du
        // joueur qui parle, jamais celui des autres).
        w.players[0].data.stats.life = 10;
        w.players[1].data.stats.life = 20;
        let s = w.open_dialogue_slot(1, a).expect("slot 1 parle à Akara");
        assert_eq!(s.slot, 1);
        assert_eq!(w.town_npcs()[a].talked, 1);
        assert_eq!(
            w.players[1].data.stats.life,
            w.players[1].data.stats.max_life,
            "le parleur est soigné"
        );
        assert_eq!(w.players[0].data.stats.life, 10, "l'autre joueur n'est PAS soigné");
        // Choix/fermeture par le mauvais slot : refusés, session intacte.
        assert_eq!(w.choose_dialogue_slot(0, 0), None, "seul le teneur choisit");
        assert!(w.dialogue_active());
        assert!(!w.close_dialogue_slot(0));
        // Choix du teneur (« partir », dernière option visible) : ok.
        let last = w.dialogue_visible_options().len() - 1;
        assert_eq!(w.choose_dialogue_slot(1, last), Some(DialogueAction::Close));
        assert!(!w.dialogue_active());
        // Slot hors monde : jamais accepté (jamais de panique réseau).
        match w.open_dialogue_slot(9, a) {
            Err(DialogueError::BadSlot) => {}
            other => panic!("BadSlot attendu, obtenu : {:?}", other.map(|s| s.npc)),
        }
    }

    #[test]
    fn coop_remove_player_referme_ou_decale_la_session() {
        // Compaction des slots (3c-c removePlayer) × session de dialogue :
        // le teneur partant ⇒ session refermée (verrou orphelin impossible)
        // ; un teneur AU-DELÀ du slot retiré ⇒ décalage -1 (même joueur).
        let mut w = World::with_seed(vec![vec![false; 300]; 300], json!({}), 0xD1A7);
        w.spawn_town_npcs();
        w.set_quest_act1(ActQuests::new());
        assert!(w.add_player(1, "SOR"));
        assert!(w.add_player(2, "PA"));
        let a = ix(&w, "akara");
        let (ax, ay) = { let e = &w.town_npcs()[a].ent; (e.x, e.y) };
        for slot in 0..3 {
            w.players[slot].ent.x = ax + 0.8 + slot as f32 * 0.7;
            w.players[slot].ent.y = ay;
        }
        // Cas 1 : le teneur (slot 2) reste, le slot 1 part → session -1.
        let s = w.open_dialogue_slot(2, a).expect("session slot 2");
        assert_eq!(s.slot, 2);
        assert!(w.remove_player(1), "slot 1 retiré");
        assert_eq!(w.dialogue_session().unwrap().slot, 1, "teneur décalé avec sa session");
        assert!(w.close_dialogue_slot(1), "le MÊME joueur ferme sous son nouveau slot");
        // Cas 2 : le teneur part lui-même → session refermée, verrou levé.
        let s = w.open_dialogue_slot(1, a).expect("session slot 1 (ex-slot 2)");
        assert_eq!(s.slot, 1);
        assert!(w.remove_player(1), "le teneur part");
        assert!(!w.dialogue_active(), "pas de verrou orphelin");
        // Le slot restant ouvre à nouveau (monde rendu à tous).
        assert!(w.open_dialogue_slot(0, a).is_ok());
    }
}
