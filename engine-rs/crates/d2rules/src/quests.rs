//! Moteur de quêtes W13 (finish-jeu P2) — états, triggers, récompenses,
//! authoring Acte I. 100 % pur : aucune dépendance à [`crate::World`], pas de
//! rendu, pas d'IO. Le câblage des événements (world.rs/game.rs) et la
//! persistance (save.rs) sont faits EN INTÉGRATION — ce module expose pour ça
//! [`QuestLog::to_json`]/[`QuestLog::from_json`] (état seul, validé).
//!
//! # Modèle
//! * Une quête = [`QuestDef`] (données sérialisables, authoring) : étapes
//!   **séquentielles**, chaque étape = un [`QuestTrigger`] + un compteur requis.
//! * Un événement [`QuestEvent`] n'avance QUE l'étape courante des quêtes
//!   `InProgress`, et DÉMARRE (`NotStarted → InProgress`) celles dont l'étape 0
//!   correspond — un événement hors ordre est ignoré (pas de rattrapage).
//! * Un trigger [`QuestTrigger::BossDead`] matche AUSSI
//!   [`QuestEvent::MonsterKilled`] du même code : l'intégration peut n'émettre
//!   que des kills tant que les spawns scriptés de boss (W11) n'existent pas.
//! * Dernière étape franchie ⇒ quête `Done` ⇒ ses [`QuestReward`] sont rendus
//!   (l'ordre des récompenses : ordre d'authoring). Quand la (ou les) quête(s)
//!   `gates_act` d'un acte sont toutes `Done`, un déverrouillage
//!   `Unlock { flag: "act{N}_done" }` est émis (ex. Andariel → « Acte I
//!   terminé »). Acte sans quête porteuse : jamais débloqué automatiquement.
//!
//! # Codes réels (assets/, autorité absolue)
//! * Boss : `bloodraven` (monstats 267), `smith` (monstats 402, super-unique
//!   « The Smith »), `andariel` (monstats 156). `griswold` existe aussi.
//! * Objets de quête : `bks` Bark Scroll (Arbre d'Inifuss), `hamm` Horadric
//!   Malus (armes), `rin` bague (récompense convention Akara).
//! * Niveaux `patch_levels.txt` : 8 = Act 1 - Cave 1 (Den of Evil),
//!   28 = Barracks, 37 = Catacombs 4 (antre d'Andariel), 38 = Tristram,
//!   5 = Dark Wood (arbre d'Inifuss, implicite via le `bks`).
//!
//! # Contrat d'intégration (FAIT EN INTÉGRATION, pas ici)
//! * `GameEvent::MonsterDied { index }` → [`QuestEvent::MonsterKilled`] avec le
//!   code `mon_codes[index]` de d2game (parallèle à `monsters()`), lu AVANT le
//!   retrait ; les codes de boss de quête peuvent passer tels quels (le trigger
//!   `BossDead` les accepte aussi en `MonsterKilled`).
//! * `GameEvent::HeroPickedUp { slot }` → [`QuestEvent::ItemPickedUp`] avec le
//!   code de `inventory()[slot]`.
//! * `CoreEvent::LevelSwitch { key, name }` → [`QuestEvent::ZoneEntered`] avec
//!   l'id `levelsId` du niveau résolu (le moteur compare aux ids de table).
//! * `NpcTalked` : aucune source d'événement tant que les NPC de ville
//!   (W16) n'existent — l'intégration synthétisera l'événement.
//! * Persistance : champ `quests: Option<Value>` = `serde_json::from_str::<
//!   serde_json::Value>(&log.to_json())` dans le Save de save.rs (W19), avec
//!   `QuestLog::from_json` au chargement ; format versionné, tout écart ⇒ Err.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Types du moteur
// ---------------------------------------------------------------------------

/// État d'une quête dans le journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuestState {
    /// Jamais déclenchée (pas encore au journal actif).
    #[default]
    NotStarted,
    /// Au moins l'étape 0 amorcée ; étapes séquentielles en cours.
    InProgress,
    /// Terminée (récompenses déjà rendues, événements ignorés).
    Done,
    /// Échouée manuellement (aucun échec automatique au MVP ; événements ignorés).
    Failed,
}

/// Événement de jeu typé, émis par l'intégration (world.rs/game.rs) — le
/// moteur ne lit JAMAIS le monde directement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestEvent {
    /// Un monstre est mort (`code` = code table `patch_monstats.txt`, ex.
    /// `FA`/`ZM`/`SI`… ou `bloodraven`). `*` n'existe pas côté événement :
    /// c'est le trigger qui porte le joker.
    MonsterKilled { code: String },
    /// Un objet est ramassé (`code` = code items, ex. `bks`, `hamm`).
    ItemPickedUp { code: String },
    /// Un niveau est entré (`level_id` = id `patch_levels.txt`, ex. 8).
    ZoneEntered { level_id: u32 },
    /// Un NPC a été parlé (W16 : source à venir ; `npc` = code bas de casse,
    /// ex. `akara`, `charsi`, `cain`).
    NpcTalked { npc: String },
    /// Un boss de quête est mort (record dédié monstats ; aussi couvert par
    /// [`QuestEvent::MonsterKilled`] de même code, cf. contrat module).
    BossDead { code: String },
}

/// Condition d'avancement d'une étape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestTrigger {
    /// Mort d'un monstre ; `code = "*"` = n'importe lequel (joker authoring).
    MonsterKilled { code: String },
    /// Mort d'un boss de quête (matche aussi `MonsterKilled` du même code).
    BossDead { code: String },
    /// Ramassage d'un objet par code.
    ItemPickedUp { code: String },
    /// Entrée dans un niveau par id de table.
    ZoneEntered { level_id: u32 },
    /// Dialogue avec un NPC par code.
    NpcTalked { npc: String },
}

impl QuestTrigger {
    /// Cet événement fait-il progresser ce trigger ?
    fn matches(&self, e: &QuestEvent) -> bool {
        match (self, e) {
            (QuestTrigger::MonsterKilled { code }, QuestEvent::MonsterKilled { code: c }) => {
                code == "*" || code == c
            }
            // Un boss de quête tué comme un monstre banal compte quand même :
            // l'intégration n'a pas besoin de distinguer avant W11.
            (QuestTrigger::BossDead { code }, QuestEvent::BossDead { code: c })
            | (QuestTrigger::BossDead { code }, QuestEvent::MonsterKilled { code: c }) => code == c,
            (QuestTrigger::ItemPickedUp { code }, QuestEvent::ItemPickedUp { code: c }) => code == c,
            (QuestTrigger::ZoneEntered { level_id }, QuestEvent::ZoneEntered { level_id: l }) => {
                level_id == l
            }
            (QuestTrigger::NpcTalked { npc }, QuestEvent::NpcTalked { npc: n }) => npc == n,
            _ => false,
        }
    }
}

/// Récompense de fin de quête (rendue par [`QuestLog::on_event`], consommée
/// par l'intégration : XP → héros, item → inventaire/sol, unlock → flag).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuestReward {
    /// XP brute pour le héros.
    Xp { amount: u32 },
    /// Objet par code table (`rin`, `hamm`…) — le nom/affmage est côté items.
    Item { code: String },
    /// Déverrouillage symbolique (flag de jeu) : `skill_point`,
    /// `rogue_scout`, `cain_free_id`, `charsi_imbue`, `act1_done`…
    Unlock { flag: String },
}

/// Une étape séquentielle de quête.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestStep {
    /// Objectif affichable au journal (FR provisoire — TBL fr = W18).
    pub desc: String,
    /// Ce qui fait avancer l'étape.
    pub trigger: QuestTrigger,
    /// Nombre d'occurrences requises (≥ 1 ; 1 pour les triggers unitaires).
    pub count: u32,
}

/// Définition d'une quête (authoring sérialisable — pas d'état ici).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestDef {
    /// Identifiant stable (`a1q1`…`a1q5`) — clé de l'état sauvegardé.
    pub id: String,
    /// Numéro d'acte (1..=5).
    pub act: u8,
    /// Nom canonique (EN wiki ; le libellé fr viendra des TBL, W18).
    pub name: String,
    /// Résumé affichable au journal (FR provisoire).
    pub summary: String,
    /// Quête porteuse : quand TOUTES les porteuses d'un acte sont `Done`,
    /// l'acte est débloqué (`Unlock { flag: "act{N}_done" }`). Ex. Acte I :
    /// q5 seule (tuer Andariel suffit, comme le retail).
    pub gates_act: bool,
    /// Étapes séquentielles (≥ 1 recommandé ; quête sans étape = jamais finie).
    pub steps: Vec<QuestStep>,
    /// Récompenses rendues à la complétion, dans cet ordre.
    pub rewards: Vec<QuestReward>,
}

/// Progression lisible d'une quête (HUD/journal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestProgress {
    /// État courant.
    pub state: QuestState,
    /// Index de l'étape courante (0-based ; dernier index si `Done`).
    pub step: usize,
    /// Description de l'étape courante (vide si hors `InProgress`).
    pub desc: String,
    /// Occurrences acquises sur l'étape courante.
    pub count: u32,
    /// Occurrences requises (0 si hors `InProgress`).
    pub required: u32,
}

/// État d'exécution d'une quête (sérialisé par [`QuestLog::to_json`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct QuestRun {
    state: QuestState,
    /// Index de l'étape courante (figé à la dernière si `Done`).
    step: usize,
    /// Compteur de l'étape courante (figé au requis si `Done`).
    count: u32,
}

/// Journal de quêtes : définitions (authoring statique) + état par quête.
///
/// Constructeurs : [`QuestLog::act1`] (authoring embarqué) ou
/// [`QuestLog::with_defs`] (tests, actes II-V futurs). Les définitions ne sont
/// PAS sérialisées — seul l'état l'est, validé contre les définitions au
/// chargement (dérive d'authoring ⇒ `Err`, jamais d'état incohérent).
#[derive(Debug, Clone)]
pub struct QuestLog {
    defs: Vec<QuestDef>,
    runs: Vec<QuestRun>,
}

impl QuestLog {
    /// Journal neuf avec l'authoring Acte I (5 quêtes, cf. [`quest_defs_act1`]).
    pub fn act1() -> QuestLog {
        QuestLog::with_defs(quest_defs_act1())
    }

    /// Journal neuf avec des définitions arbitraires (tests / actes futurs).
    pub fn with_defs(defs: Vec<QuestDef>) -> QuestLog {
        QuestLog {
            runs: defs.iter().map(|_| QuestRun { state: QuestState::NotStarted, step: 0, count: 0 }).collect(),
            defs,
        }
    }

    /// Définitions (authoring) du journal.
    pub fn defs(&self) -> &[QuestDef] {
        &self.defs
    }

    /// État d'une quête par id (None si id inconnu).
    pub fn state(&self, id: &str) -> Option<QuestState> {
        self.index_of(id).map(|i| self.runs[i].state)
    }

    /// Liste (id, état) dans l'ordre d'authoring — base du journal UI.
    pub fn states(&self) -> Vec<(&str, QuestState)> {
        self.defs.iter().map(|d| d.id.as_str()).zip(self.runs.iter().map(|r| r.state)).collect()
    }

    /// Progression lisible d'une quête (None si id inconnu).
    pub fn progress(&self, id: &str) -> Option<QuestProgress> {
        let i = self.index_of(id)?;
        let (d, r) = (&self.defs[i], &self.runs[i]);
        let step_desc = |s: usize| d.steps.get(s).map(|s| s.desc.clone()).unwrap_or_default();
        match r.state {
            QuestState::InProgress => {
                let req = d.steps[r.step].count;
                Some(QuestProgress {
                    state: r.state,
                    step: r.step,
                    desc: step_desc(r.step),
                    count: r.count,
                    required: req,
                })
            }
            QuestState::Done => Some(QuestProgress {
                state: r.state,
                step: d.steps.len().saturating_sub(1),
                desc: step_desc(d.steps.len().saturating_sub(1)),
                count: d.steps.last().map(|s| s.count).unwrap_or(0),
                required: d.steps.last().map(|s| s.count).unwrap_or(0),
            }),
            QuestState::NotStarted | QuestState::Failed => Some(QuestProgress {
                state: r.state,
                step: r.step,
                desc: String::new(),
                count: 0,
                required: 0,
            }),
        }
    }

    /// L'acte `act` est-il débloqué ? (toutes ses quêtes porteuses `Done` ;
    /// acte sans porteuse ou sans quête ⇒ false).
    pub fn act_done(&self, act: u8) -> bool {
        let gates: Vec<bool> = self
            .defs
            .iter()
            .zip(self.runs.iter())
            .filter(|(d, _)| d.act == act && d.gates_act)
            .map(|(_, r)| r.state == QuestState::Done)
            .collect();
        !gates.is_empty() && gates.iter().all(|&done| done)
    }

    /// Échoue manuellement une quête `InProgress` (MVP : aucun échec auto).
    /// Renvoie true si l'état a changé.
    pub fn fail(&mut self, id: &str) -> bool {
        match self.index_of(id) {
            Some(i) if self.runs[i].state == QuestState::InProgress => {
                self.runs[i].state = QuestState::Failed;
                true
            }
            _ => false,
        }
    }

    /// Fait avancer le journal avec un événement de jeu et rend TOUTES les
    /// récompenses nouvellement acquises (ordre : quêtes d'authoring, puis
    /// récompenses de quête, puis déverrouillage d'acte le cas échéant).
    ///
    /// Un même événement n'avance qu'UNE occurrence par quête concernée ;
    /// les quêtes `Done`/`Failed` ignorent tout ; l'ordre des étapes est
    /// strict (un événement en avance de phase est perdu, comme le retail).
    pub fn on_event(&mut self, event: &QuestEvent) -> Vec<QuestReward> {
        let mut rewards = Vec::new();
        let mut acts_completed: Vec<u8> = Vec::new();
        for i in 0..self.defs.len() {
            if self.runs[i].state != QuestState::NotStarted
                && self.runs[i].state != QuestState::InProgress
            {
                continue;
            }
            let def = &self.defs[i];
            // L'étape courante correspond-elle ? (NotStarted : l'étape 0
            // démarrée par un événement correspondant = quête au journal.)
            let step = self.runs[i].step;
            let (matches, required) = match def.steps.get(step) {
                Some(s) => (s.trigger.matches(event), s.count.max(1)),
                None => (false, 1),
            };
            if !matches {
                continue;
            }
            if self.runs[i].state == QuestState::NotStarted {
                self.runs[i].state = QuestState::InProgress;
            }
            self.runs[i].count += 1;
            if self.runs[i].count < required {
                continue; // progression partielle, rien à rendre
            }
            // Étape franchie : suivante, ou quête terminée (état figé).
            if step + 1 < def.steps.len() {
                self.runs[i].step = step + 1;
                self.runs[i].count = 0;
                continue;
            }
            self.runs[i].step = def.steps.len() - 1;
            self.runs[i].count = def.steps[def.steps.len() - 1].count.max(1);
            self.runs[i].state = QuestState::Done;
            rewards.extend(def.rewards.iter().cloned());
            if def.gates_act && !acts_completed.contains(&def.act) && self.act_done(def.act) {
                acts_completed.push(def.act);
            }
        }
        for act in acts_completed {
            rewards.push(QuestReward::Unlock { flag: format!("act{}_done", act) });
        }
        rewards
    }

    // -- Persistance (contrat save.rs / W19 — le champ Save est en intégration)

    /// Sérialise l'ÉTAT SEUL (JSON versionné) — à embarquer tel quel dans le
    /// save (`quests: Option<Value>`). Les définitions ne sont pas dupliquées ;
    /// chaque entrée porte son `id` (lisible, validé au chargement).
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct RunSave<'a> {
            id: &'a str,
            state: QuestState,
            step: usize,
            count: u32,
        }
        #[derive(Serialize)]
        struct State<'a> {
            version: u32,
            quests: Vec<RunSave<'a>>,
        }
        let quests = self
            .defs
            .iter()
            .zip(self.runs.iter())
            .map(|(d, r)| RunSave { id: &d.id, state: r.state, step: r.step, count: r.count })
            .collect();
        let s = State { version: STATE_VERSION, quests };
        serde_json::to_string(&s).expect("d2rules: sérialisation quests")
    }

    /// Recharge un état produit par [`QuestLog::to_json`] sur l'authoring
    /// Acte I. Validation stricte : version, ids connus et dans l'ordre
    /// d'authoring, compteur dans les bornes de l'étape, états figés
    /// cohérents — tout écart ⇒ Err(()) (l'appelant garde son état précédent,
    /// jamais d'état corrompu).
    pub fn from_json(json: &str) -> Result<QuestLog, ()> {
        #[derive(Deserialize)]
        struct RunSave {
            id: String,
            state: QuestState,
            step: usize,
            count: u32,
        }
        #[derive(Deserialize)]
        struct State {
            version: u32,
            quests: Vec<RunSave>,
        }
        let s: State = serde_json::from_str(json).map_err(|_| ())?;
        if s.version != STATE_VERSION {
            return Err(());
        }
        let defs = quest_defs_act1();
        if s.quests.len() != defs.len() {
            return Err(());
        }
        for (save, def) in s.quests.iter().zip(defs.iter()) {
            if save.id != def.id {
                return Err(());
            }
            let last = def.steps.len().saturating_sub(1);
            let ok = match save.state {
                QuestState::NotStarted => save.step == 0 && save.count == 0,
                QuestState::InProgress => {
                    save.step <= last && save.count < def.steps[save.step].count.max(1)
                }
                QuestState::Done => {
                    save.step == last && save.count == def.steps[last].count.max(1)
                }
                QuestState::Failed => {
                    save.step <= last && save.count <= def.steps[save.step].count.max(1)
                }
            };
            if !ok {
                return Err(());
            }
        }
        Ok(QuestLog {
            defs,
            runs: s
                .quests
                .into_iter()
                .map(|q| QuestRun { state: q.state, step: q.step, count: q.count })
                .collect(),
        })
    }

    fn index_of(&self, id: &str) -> Option<usize> {
        self.defs.iter().position(|d| d.id == id)
    }
}

const STATE_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Authoring Acte I (source : wiki D2 — AUCUNE table de quêtes dans les MPQ,
// prouvé finish-monde §1 ; codes objets/niveaux vérifiés dans assets/).
// Libellés FR provisoires : les TBL fr manquent (W18).
// ---------------------------------------------------------------------------

/// Authoring des 5 quêtes de l'Acte I (Den of Evil → Andariel).
pub fn quest_defs_act1() -> Vec<QuestDef> {
    vec![
        // q1 — Den of Evil : nettoyer la grotte ; Akara donne 1 point de skill.
        // Compteur aligné à la densité du port (5-8 monstres/niveau) ; le
        // retail = « tous les monstres de la grotte » (~40).
        QuestDef {
            id: "a1q1".into(),
            act: 1,
            name: "Den of Evil".into(),
            summary: "Purifiez la Tanière du Mal qui souille la forêt du Camp des Rousses."
                .into(),
            gates_act: false,
            steps: vec![
                QuestStep {
                    desc: "Entrez dans la Tanière du Mal (Act 1 - Cave 1).".into(),
                    trigger: QuestTrigger::ZoneEntered { level_id: 8 },
                    count: 1,
                },
                QuestStep {
                    desc: "Purifiez la Tanière : abattez ses monstres.".into(),
                    trigger: QuestTrigger::MonsterKilled { code: "*".into() },
                    count: 6,
                },
            ],
            rewards: vec![QuestReward::Unlock { flag: "skill_point".into() }],
        },
        // q2 — Sisters' Burial Grounds : Blood Raven (monstats bloodraven).
        // Récompense retail : Kashya offre une recrue Rogues.
        QuestDef {
            id: "a1q2".into(),
            act: 1,
            name: "Sisters' Burial Grounds".into(),
            summary: "Rendez le repos à Blood Raven, qui corrompt le cimetière des Blood Moor."
                .into(),
            gates_act: false,
            steps: vec![QuestStep {
                desc: "Abattez Blood Raven au cimetière des Blood Moor.".into(),
                trigger: QuestTrigger::BossDead { code: "bloodraven".into() },
                count: 1,
            }],
            rewards: vec![QuestReward::Unlock { flag: "rogue_scout".into() }],
        },
        // q3 — The Search for Cain : bark scroll (bks) → déchiffrage Akara →
        // Tristram (id 38) → délivrer Cain. Récompense retail : Akara offre
        // une bague (rin) et Cain identifie gratuitement au camp.
        QuestDef {
            id: "a1q3".into(),
            act: 1,
            name: "The Search for Cain".into(),
            summary: "Découvrez le sort du dernier Horadrim : l'Arbre d'Inifuss garde la clé des pierres de Cairn.".into(),
            gates_act: false,
            steps: vec![
                QuestStep {
                    desc: "Récoltez l'écorce sur l'Arbre d'Inifuss (Dark Wood).".into(),
                    trigger: QuestTrigger::ItemPickedUp { code: "bks".into() },
                    count: 1,
                },
                QuestStep {
                    desc: "Faites déchiffrer le parchemin par Akara.".into(),
                    trigger: QuestTrigger::NpcTalked { npc: "akara".into() },
                    count: 1,
                },
                QuestStep {
                    desc: "Ouvrez le portail et entrez à Tristram.".into(),
                    trigger: QuestTrigger::ZoneEntered { level_id: 38 },
                    count: 1,
                },
                QuestStep {
                    desc: "Délivrez Deckard Cain de sa cage.".into(),
                    trigger: QuestTrigger::NpcTalked { npc: "cain".into() },
                    count: 1,
                },
            ],
            rewards: vec![
                QuestReward::Item { code: "rin".into() },
                QuestReward::Unlock { flag: "cain_free_id".into() },
            ],
        },
        // q4 — Tools of the Trade : le Horadric Malus (hamm) est détenu par le
        // Smith (monstats smith, super-unique « The Smith ») dans la Caserne
        // (id 28). Récompense retail : Charsi enchanteresse un objet.
        QuestDef {
            id: "a1q4".into(),
            act: 1,
            name: "Tools of the Trade".into(),
            summary: "Reprennez le marteau Horadrique aux mains du Smith, dans la Caserne du monastère.".into(),
            gates_act: false,
            steps: vec![
                QuestStep {
                    desc: "Abattez le Smith dans la Caserne.".into(),
                    trigger: QuestTrigger::BossDead { code: "smith".into() },
                    count: 1,
                },
                QuestStep {
                    desc: "Ramassez le Horadric Malus (hamm).".into(),
                    trigger: QuestTrigger::ItemPickedUp { code: "hamm".into() },
                    count: 1,
                },
                QuestStep {
                    desc: "Rapportez le Malus à Charsi.".into(),
                    trigger: QuestTrigger::NpcTalked { npc: "charsi".into() },
                    count: 1,
                },
            ],
            rewards: vec![QuestReward::Unlock { flag: "charsi_imbue".into() }],
        },
        // q5 — Sisters to the Slaughter : Andariel (monstats andariel) au fond
        // des Catacombes (id 37 = Catacombs 4). Quête PORTEUSE : sa mort
        // débloque l'acte via Unlock { flag: "act1_done" } (émis par le
        // moteur), « Acte I terminé ».
        QuestDef {
            id: "a1q5".into(),
            act: 1,
            name: "Sisters to the Slaughter".into(),
            summary: "Descendez les Catacombes et abattez Andariel pour venger les Sœurs des Yeux Aveugles.".into(),
            gates_act: true,
            steps: vec![QuestStep {
                desc: "Abattez Andariel, au fond des Catacombes.".into(),
                trigger: QuestTrigger::BossDead { code: "andariel".into() },
                count: 1,
            }],
            // Le déverrouillage d'acte est émis par le MOTEUR (gates_act) :
            // pas de récompense d'authoring redondante ici.
            rewards: vec![],
        },
    ]
}

// ---------------------------------------------------------------------------
// Tests : événements → états → récompenses, chaîne A1 complète, round-trip.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- fabriques d'événements ------------------------------------------------
    fn kill(code: &str) -> QuestEvent {
        QuestEvent::MonsterKilled { code: code.into() }
    }
    fn boss(code: &str) -> QuestEvent {
        QuestEvent::BossDead { code: code.into() }
    }
    fn item(code: &str) -> QuestEvent {
        QuestEvent::ItemPickedUp { code: code.into() }
    }
    fn zone(id: u32) -> QuestEvent {
        QuestEvent::ZoneEntered { level_id: id }
    }
    fn talk(npc: &str) -> QuestEvent {
        QuestEvent::NpcTalked { npc: npc.into() }
    }
    fn unlocks(rewards: &[QuestReward]) -> Vec<String> {
        rewards
            .iter()
            .filter_map(|r| match r {
                QuestReward::Unlock { flag } => Some(flag.clone()),
                _ => None,
            })
            .collect()
    }

    // -- états -----------------------------------------------------------------

    #[test]
    fn fresh_log_all_not_started() {
        let log = QuestLog::act1();
        assert_eq!(log.defs().len(), 5);
        assert!(log.states().iter().all(|(_, s)| *s == QuestState::NotStarted));
        assert!(!log.act_done(1));
        assert_eq!(log.state("a1q1"), Some(QuestState::NotStarted));
        assert_eq!(log.state("a1q6"), None);
    }

    #[test]
    fn den_of_evil_chain_zone_then_kills() {
        let mut log = QuestLog::act1();
        // Tuer avant d'entrer : l'étape 0 est l'entrée de zone — rien.
        assert!(log.on_event(&kill("FA")).is_empty());
        assert_eq!(log.state("a1q1"), Some(QuestState::NotStarted));
        // Entrée dans la grotte (id 8) : la quête démarre, étape kills.
        assert!(log.on_event(&zone(8)).is_empty());
        assert_eq!(log.state("a1q1"), Some(QuestState::InProgress));
        let p = log.progress("a1q1").unwrap();
        assert_eq!((p.step, p.count, p.required), (1, 0, 6));
        // 5 kills : progression partielle, aucune récompense.
        for _ in 0..5 {
            assert!(log.on_event(&kill("ZM")).is_empty());
        }
        let p = log.progress("a1q1").unwrap();
        assert_eq!((p.count, p.required), (5, 6));
        // 6e kill : quête terminée + récompense Akara.
        let r = log.on_event(&kill("SI"));
        assert_eq!(log.state("a1q1"), Some(QuestState::Done));
        assert_eq!(unlocks(&r), vec!["skill_point".to_string()]);
        // Redondance : les kills suivants ne rendent plus rien.
        assert!(log.on_event(&kill("FA")).is_empty());
        assert_eq!(log.progress("a1q1").unwrap().state, QuestState::Done);
    }

    #[test]
    fn blood_raven_boss_trigger_via_kill_or_boss_event() {
        // Voie 1 : événement BossDead dédié.
        let mut log = QuestLog::act1();
        let r = log.on_event(&boss("bloodraven"));
        assert_eq!(log.state("a1q2"), Some(QuestState::Done));
        assert_eq!(unlocks(&r), vec!["rogue_scout".to_string()]);
        // Voie 2 : le boss tombe comme un monstre banal (contrat MonsterDied).
        let mut log2 = QuestLog::act1();
        let r2 = log2.on_event(&kill("bloodraven"));
        assert_eq!(log2.state("a1q2"), Some(QuestState::Done));
        assert_eq!(unlocks(&r2), vec!["rogue_scout".to_string()]);
        // Un autre boss ne compte pas.
        let mut log3 = QuestLog::act1();
        assert!(log3.on_event(&boss("smith")).is_empty());
        assert_eq!(log3.state("a1q2"), Some(QuestState::NotStarted));
    }

    #[test]
    fn cain_chain_order_enforced() {
        let mut log = QuestLog::act1();
        // Hors ordre : parler à Cain tout de suite ne fait rien.
        assert!(log.on_event(&talk("cain")).is_empty());
        assert_eq!(log.state("a1q3"), Some(QuestState::NotStarted));
        // Mauvais objet : rien.
        assert!(log.on_event(&item("isc")).is_empty());
        // Écorce d'Inifuss → Akara → Tristram → Cain.
        assert!(log.on_event(&item("bks")).is_empty());
        assert_eq!(log.state("a1q3"), Some(QuestState::InProgress));
        assert_eq!(log.progress("a1q3").unwrap().step, 1);
        assert!(log.on_event(&talk("akara")).is_empty());
        assert_eq!(log.progress("a1q3").unwrap().step, 2);
        assert!(log.on_event(&zone(38)).is_empty());
        assert_eq!(log.progress("a1q3").unwrap().step, 3);
        let r = log.on_event(&talk("cain"));
        assert_eq!(log.state("a1q3"), Some(QuestState::Done));
        assert_eq!(
            r,
            vec![
                QuestReward::Item { code: "rin".into() },
                QuestReward::Unlock { flag: "cain_free_id".into() },
            ]
        );
    }

    #[test]
    fn charsi_chain_smith_malus_imbue() {
        let mut log = QuestLog::act1();
        assert!(log.on_event(&boss("smith")).is_empty());
        assert!(log.on_event(&item("hamm")).is_empty());
        let r = log.on_event(&talk("charsi"));
        assert_eq!(log.state("a1q4"), Some(QuestState::Done));
        assert_eq!(unlocks(&r), vec!["charsi_imbue".to_string()]);
    }

    #[test]
    fn andariel_gates_act_done_alone() {
        // Comme le retail : tuer Andariel suffit à « finir » l'acte, même les
        // quêtes secondaires inachevées.
        let mut log = QuestLog::act1();
        assert!(!log.act_done(1));
        let r = log.on_event(&boss("andariel"));
        assert_eq!(log.state("a1q5"), Some(QuestState::Done));
        assert_eq!(unlocks(&r), vec!["act1_done".to_string()]);
        assert!(log.act_done(1));
        // Récompense d'acte unique : la rejouer ne redonne rien.
        assert!(log.on_event(&boss("andariel")).is_empty());
        // Actes sans quêtes/porteuses : jamais débloqués.
        assert!(!log.act_done(2));
    }

    // -- chaîne complète -------------------------------------------------------

    #[test]
    fn full_act1_replay_den_to_andariel() {
        let mut log = QuestLog::act1();
        let mut all = Vec::new();
        for ev in [
            zone(8),
            kill("FA"),
            kill("FA"),
            kill("ZM"),
            kill("ZM"),
            kill("SI"),
            kill("SK"),
            boss("bloodraven"),
            item("bks"),
            talk("akara"),
            zone(38),
            talk("cain"),
            boss("smith"),
            item("hamm"),
            talk("charsi"),
            boss("andariel"),
        ] {
            all.extend(log.on_event(&ev));
        }
        // Tout est Done, l'acte est fini.
        assert!(log.states().iter().all(|(_, s)| *s == QuestState::Done));
        assert!(log.act_done(1));
        // Récompenses complètes, dans l'ordre d'authoring + unlock d'acte.
        assert_eq!(
            all,
            vec![
                QuestReward::Unlock { flag: "skill_point".into() },
                QuestReward::Unlock { flag: "rogue_scout".into() },
                QuestReward::Item { code: "rin".into() },
                QuestReward::Unlock { flag: "cain_free_id".into() },
                QuestReward::Unlock { flag: "charsi_imbue".into() },
                QuestReward::Unlock { flag: "act1_done".into() },
            ]
        );
        // Replay intégral : plus rien à rendre.
        for ev in [zone(8), kill("FA"), boss("bloodraven"), item("bks"), talk("akara"), zone(38), talk("cain"), boss("smith"), item("hamm"), talk("charsi"), boss("andariel")] {
            assert!(log.on_event(&ev).is_empty());
        }
    }

    // -- échec manuel ----------------------------------------------------------

    #[test]
    fn manual_fail_ignores_events() {
        let mut log = QuestLog::act1();
        log.on_event(&zone(8));
        assert!(log.fail("a1q1"));
        assert_eq!(log.state("a1q1"), Some(QuestState::Failed));
        // Échouer deux fois / une quête non démarrée : no-op.
        assert!(!log.fail("a1q1"));
        assert!(!log.fail("a1q2"));
        // Une échouée ignore tout événement.
        assert!(log.on_event(&kill("FA")).is_empty());
        assert_eq!(log.state("a1q1"), Some(QuestState::Failed));
    }

    // -- variantes de récompenses (XP/item) via authoring arbitraire ----------

    #[test]
    fn with_defs_xp_and_item_rewards() {
        let defs = vec![QuestDef {
            id: "t1".into(),
            act: 9,
            name: "Test".into(),
            summary: String::new(),
            gates_act: true, // acte 9 : porteuse seule → unlock act9_done
            steps: vec![QuestStep {
                desc: String::new(),
                trigger: QuestTrigger::MonsterKilled { code: "ZZ".into() },
                count: 2,
            }],
            rewards: vec![
                QuestReward::Xp { amount: 100 },
                QuestReward::Item { code: "hp1".into() },
            ],
        }];
        let mut log = QuestLog::with_defs(defs);
        assert!(log.on_event(&kill("ZZ")).is_empty());
        let r = log.on_event(&kill("ZZ"));
        assert_eq!(
            r,
            vec![
                QuestReward::Xp { amount: 100 },
                QuestReward::Item { code: "hp1".into() },
                QuestReward::Unlock { flag: "act9_done".into() },
            ]
        );
        assert!(log.act_done(9));
    }

    #[test]
    fn joker_and_shared_events_feed_multiple_quests() {
        // Un kill joker n'avance qu'une occurrence par quête concernée : ici
        // seul a1q1 a un trigger joker — et un ZoneEntered commun reste scoping
        // par quête (étape courante uniquement).
        let mut log = QuestLog::act1();
        log.on_event(&zone(8));
        assert!(log.on_event(&kill("FA")).is_empty());
        assert_eq!(log.progress("a1q1").unwrap().count, 1);
        assert_eq!(log.state("a1q2"), Some(QuestState::NotStarted));
    }

    // -- sérialisation ---------------------------------------------------------

    #[test]
    fn json_shape_and_fresh_round_trip() {
        let log = QuestLog::act1();
        let json = log.to_json();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["quests"].as_array().unwrap().len(), 5);
        assert_eq!(v["quests"][0]["id"], "a1q1");
        assert_eq!(v["quests"][0]["state"], "not_started");
        assert_eq!(v["quests"][0]["step"], 0);
        assert_eq!(v["quests"][0]["count"], 0);
        let log2 = QuestLog::from_json(&json).unwrap();
        assert_eq!(log.to_json(), log2.to_json());
        assert!(log2.states().iter().all(|(_, s)| *s == QuestState::NotStarted));
    }

    #[test]
    fn json_round_trip_partial_progress() {
        let mut log = QuestLog::act1();
        log.on_event(&zone(8));
        for _ in 0..3 {
            log.on_event(&kill("FA"));
        }
        log.on_event(&item("bks")); // q3 démarre aussi
        let json = log.to_json();
        let mut log2 = QuestLog::from_json(&json).unwrap();
        assert_eq!(json, log2.to_json());
        let p1 = log.progress("a1q1").unwrap();
        let p2 = log2.progress("a1q1").unwrap();
        assert_eq!((p1.state, p1.step, p1.count), (p2.state, p2.step, p2.count));
        assert_eq!(log2.state("a1q3"), Some(QuestState::InProgress));
        // Le rechargé poursuit exactement où il en était.
        for _ in 0..3 {
            log2.on_event(&kill("ZM"));
        }
        assert_eq!(log2.state("a1q1"), Some(QuestState::Done));
    }

    #[test]
    fn json_round_trip_full_chain() {
        let mut log = QuestLog::act1();
        for ev in [
            zone(8),
            kill("FA"),
            kill("FA"),
            kill("ZM"),
            kill("ZM"),
            kill("SI"),
            kill("SK"),
            boss("bloodraven"),
            item("bks"),
            talk("akara"),
            zone(38),
            talk("cain"),
            boss("smith"),
            item("hamm"),
            talk("charsi"),
            boss("andariel"),
        ] {
            log.on_event(&ev);
        }
        let log2 = QuestLog::from_json(&log.to_json()).unwrap();
        assert!(log2.states().iter().all(|(_, s)| *s == QuestState::Done));
        assert!(log2.act_done(1));
        assert_eq!(log.to_json(), log2.to_json());
    }

    #[test]
    fn json_rejects_corrupted_states() {
        let base = QuestLog::act1().to_json();
        // Version inconnue.
        let bad = r#"{"version":99,"quests":[]}"#;
        assert!(QuestLog::from_json(bad).is_err());
        // JSON invalide.
        assert!(QuestLog::from_json("{oops").is_err());
        // Mauvais nombre de quêtes.
        let bad = serde_json::json!({"version":1,"quests":[]}).to_string();
        assert!(QuestLog::from_json(&bad).is_err());
        // Fabrique un état valide avec quests[0] remplacé par `patch`.
        let patched = |patch: serde_json::Value| {
            let mut v: serde_json::Value = serde_json::from_str(&base).unwrap();
            let mut q = patch;
            q["id"] = "a1q1".into();
            v["quests"][0] = q;
            QuestLog::from_json(&v.to_string())
        };
        // Id inconnu (id remplacé sur un état sinon valide).
        let mut v: serde_json::Value = serde_json::from_str(&base).unwrap();
        v["quests"][0]["id"] = "zz1".into();
        assert!(QuestLog::from_json(&v.to_string()).is_err());
        // Compteur hors bornes (>= requis de l'étape 1 : 6).
        assert!(patched(serde_json::json!({"state":"in_progress","step":1,"count":6})).is_err());
        // Done incohérent (pas figé sur la dernière étape).
        assert!(patched(serde_json::json!({"state":"done","step":0,"count":1})).is_err());
        // NotStarted avec résidus.
        assert!(patched(serde_json::json!({"state":"not_started","step":2,"count":1})).is_err());
        // Étape inexistante.
        assert!(patched(serde_json::json!({"state":"in_progress","step":9,"count":0})).is_err());
        // Contrôle positif : la même forme avec des valeurs cohérentes passe.
        assert!(patched(serde_json::json!({"state":"in_progress","step":1,"count":5})).is_ok());
    }

    #[test]
    fn json_accepts_failed_state_round_trip() {
        let mut log = QuestLog::act1();
        log.on_event(&zone(8));
        assert!(log.fail("a1q1"));
        let log2 = QuestLog::from_json(&log.to_json()).unwrap();
        assert_eq!(log2.state("a1q1"), Some(QuestState::Failed));
        assert_eq!(log.to_json(), log2.to_json());
    }
}
