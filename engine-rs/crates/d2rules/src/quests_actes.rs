//! Moteur de quêtes ACTE I — implémentation de la spec
//! `d2w-re/specs/quests-actes.md` (campagne RE d2w-re, étape 4, intégration).
//!
//! Coexiste avec le journal W13 ([`crate::quests::QuestLog`], authoring
//! historique 5 quêtes) : ce module est l'implémentation SPEC (6 quêtes
//! A1Q1→A1Q6, machine à phases, gating d'acte, flags u16 façon .d2s).
//! 100 % pur : aucune dépendance de rendu/IO ; le bloc `impl World` additif
//! en bas du fichier fournit le branchement + la save (`quest_act1`,
//! champ `#[serde(default)]` — save v3 inchangée).
//!
//! # Modèle (spec §1.1/§1.3)
//! * Events d'entrée [`ActEvent`] : `ZoneEntered`, `MonsterKilled`
//!   (+ superunique), `NpcTalked`, `ItemPicked` — émis par le gameplay,
//!   consommés par [`ActQuests::on_event`].
//! * Phases par quête [`ActPhase`] : `Inactive` → `Active` (étapes
//!   séquentielles) → `RewardPending` (bit 1, récompense à prendre auprès du
//!   PNJ) → `Done` (bit 0). Quête sans PNJ de remise (A1Q5, loot = récompense)
//!   : `RewardPending` sauté, récompenses rendues à la dernière étape.
//! * Un événement n'avance qu'UNE occurrence de l'étape courante d'UNE quête ;
//!   hors ordre = ignoré (comme le retail).
//! * Flags u16 par quête, bits .d2s [CITÉ trevin via spec §1.3] :
//!   bit 0 = complétée, bit 1 = récompense à prendre, bit 2 = donnée par PNJ,
//!   bit 4 = repaire du Mal entré (spécifique A1Q1). Le bit 10 « niveau vache »
//!   (A1Q4) n'est pas produit ici — extra hors scope strict.
//! * Mapping .d2s : u16 de l'acte I = offsets 347/349/351/353/355/357
//!   (Normal), +96 (Cauchemar), +192 (Enfer) — spec §1.3, table verrouillée
//!   par les tests golden `d2s_offsets_act1_golden`.
//!
//! # Triggers réels (spec §2, [VÉRIFIÉ-LOCAL])
//! * A1Q1 : niveau **8** « Den of Evil » (`patch_levels.txt:10`, Quest=1).
//! * A1Q2 : **Blood Raven** — code monstats `corruptrogue2`
//!   (`patch_monstats.txt:46`) / record de quête `bloodraven` ; cimetière id 17.
//! * A1Q3 : **The Smith** (superuniques.txt:22) + Horadric Malus `hdm`
//!   (`patch_weapons.txt:91`) ; caserne id 28.
//! * A1Q4 : arbre d'Inifuss (Dark Wood id 5), `bks` → Akara → pierres de Cairn
//!   (Stony Field id 4) → Tristram id 38 → Cain.
//! * A1Q5 : Tour oubliée id 20, **The Countess** (superuniques.txt:8).
//! * A1Q6 : Catacombs Level 4 id 37, **Andariel** (`monstats.txt:158`) ;
//!   remise Warriv → accès acte II (gating testé, spec §1.4).
//!
//! # Récompenses
//! Récompenses NPC [CITÉ Arreat Summit via spec §2] portées par le journal
//! (skill point, mercenaire Rogue, imbu Charsi, anneau Akara, Cain id
//! gratuit) + bonus XP/or de quête **[INVENTÉ]** (durés dans le DLL, à
//! remplacer si la carto D2Game sort mieux — spec §7).

use serde::{Deserialize, Serialize};

use crate::world::World;

// ---------------------------------------------------------------------------
// Events / phases / récompenses
// ---------------------------------------------------------------------------

/// Événement de jeu consommé par le moteur (émis par le gameplay — spec §1.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActEvent {
    /// Un niveau est entré (`level_id` = id `patch_levels.txt`).
    ZoneEntered { level_id: u32 },
    /// Un monstre est mort : `code` = code monstats (ex. `andariel`),
    /// `superunique` = nom SuperUniques.txt le cas échéant (ex. `countess`).
    MonsterKilled { code: String, superunique: Option<String> },
    /// Un NPC a été parlé (code quête bas de casse, ex. `akara`, `warriv`).
    NpcTalked { npc: String },
    /// Un objet de quête est ramassé (code items, ex. `bks`, `hdm`).
    ItemPicked { code: String },
}

/// Phase d'une quête (machine spec §2 : INACTIF → ACTIF → PRÊT → COMPLET).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActPhase {
    /// Jamais déclenchée.
    #[default]
    Inactive,
    /// Étapes séquentielles en cours.
    Active,
    /// Toutes les étapes franchies, récompense à prendre auprès du PNJ (bit 1).
    RewardPending,
    /// Terminée, récompense rendue (bit 0).
    Done,
}

impl ActPhase {
    /// Code stable (events/events UI/save).
    pub fn as_str(&self) -> &'static str {
        match self {
            ActPhase::Inactive => "inactive",
            ActPhase::Active => "active",
            ActPhase::RewardPending => "reward_pending",
            ActPhase::Done => "done",
        }
    }
}

/// Récompense émise par le moteur (spec §1.2 — events de sortie). Les
/// `Unlock`-like sont portés par le journal lui-même ; XP/or sont appliqués
/// par [`World::quest_act1_event`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActReward {
    /// XP de quête ([INVENTÉ] — bonus retail duré dans le DLL).
    Xp(u32),
    /// Or de quête ([INVENTÉ] — proxy des TC `gld,mul=…` boss/Tour).
    Gold(u32),
    /// Point(s) de compétence (A1Q1 Akara [CITÉ]).
    SkillPoint(u32),
    /// Objet par code table (A1Q4 : anneau `rin` [CITÉ]).
    Item(&'static str),
    /// Mercenaire Rogue gratuite (A1Q2 Kashya [CITÉ]).
    MercenaryRogue,
    /// Imbu Charsi consommable (A1Q3 [CITÉ]).
    CharsiImbue,
    /// Cain identifie gratuitement (A1Q4 [CITÉ]).
    CainFreeIdentify,
    /// Accès à l'acte suivant (A1Q6 Warriv → acte II [FIDÈLE]) +
    /// SetProgressionByte(+1) — portés par [`ActQuests::access_act`].
    UnlockAccessAct(u8),
}

// ---------------------------------------------------------------------------
// Bits de flags u16 (façon .d2s — spec §1.3)
// ---------------------------------------------------------------------------

/// bit 0 — quête complétée.
pub const FLAG_DONE: u16 = 0x0001;
/// bit 1 — prérequis fait, récompense à prendre.
pub const FLAG_REWARD_PENDING: u16 = 0x0002;
/// bit 2 — donnée par PNJ.
pub const FLAG_GIVEN_BY_NPC: u16 = 0x0004;
/// bit 4 — repaire du Mal entré (spécifique A1Q1 [CITÉ trevin]).
pub const FLAG_DEN_ENTERED: u16 = 0x0010;

// ---------------------------------------------------------------------------
// Authoring Acte I (spec §2 — 6 quêtes, ordre du journal retail)
// ---------------------------------------------------------------------------

/// Trigger d'une étape (authoring statique).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Trig {
    /// Dialogue NPC (code quête bas de casse).
    Talk(&'static str),
    /// Entrée dans un niveau (id `patch_levels.txt`).
    Zone(u32),
    /// Mort d'un monstre — n'importe quel alias du tableau : code monstats
    /// (base ou record de quête) OU nom de superunique.
    Kill(&'static [&'static str]),
    /// Ramassage d'un objet par code.
    Item(&'static str),
}

impl Trig {
    fn matches(&self, ev: &ActEvent) -> bool {
        match (self, ev) {
            (Trig::Talk(n), ActEvent::NpcTalked { npc }) => npc.eq_ignore_ascii_case(n),
            (Trig::Zone(id), ActEvent::ZoneEntered { level_id }) => id == level_id,
            (Trig::Item(c), ActEvent::ItemPicked { code }) => code.eq_ignore_ascii_case(c),
            (Trig::Kill(aliases), ActEvent::MonsterKilled { code, superunique }) => aliases
                .iter()
                .any(|a| code.eq_ignore_ascii_case(a) || superunique.as_deref().map_or(false, |s| s.eq_ignore_ascii_case(a))),
            _ => false,
        }
    }
}

/// Étape séquentielle d'une quête.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct StepDef {
    /// Libellé FR (string.tsv via spec §2, condensé).
    pub desc: &'static str,
    /// Ce qui fait avancer l'étape.
    pub trig: Trig,
    /// Occurrences requises (A1Q1 : compteur « monstres restants »).
    pub count: u32,
}

/// Définition d'une quête de l'acte I.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefAct1 {
    /// Identifiant stable (`a1q1`…`a1q6`) — clé de save/events.
    pub id: &'static str,
    /// Nom EN (wiki ; libellé fr = TBL `qstsa1q*`, W18).
    pub name: &'static str,
    /// Étapes séquentielles.
    pub steps: &'static [StepDef],
    /// PNJ de remise de récompense (`None` = complétion directe, loot = récompense).
    pub reward_npc: Option<&'static str>,
    /// Récompenses rendues à la complétion, dans cet ordre.
    pub rewards: &'static [ActReward],
}

/// Ids dans l'ordre des slots de save (a1q1 → a1q6, spec §1.3).
pub const ACT1_QUEST_IDS: [&str; 6] = ["a1q1", "a1q2", "a1q3", "a1q4", "a1q5", "a1q6"];

/// `patch_levels.txt` : ids de niveaux réels des triggers (spec §2).
mod lvl {
    pub const DARK_WOOD: u32 = 5;
    pub const STONY_FIELD: u32 = 4;
    pub const DEN_OF_EVIL: u32 = 8;
    pub const FORGOTTEN_TOWER: u32 = 20;
    pub const CATACOMBS_4: u32 = 37;
    pub const TRISTRAM: u32 = 38;
}

/// Authoring complet de l'acte I (spec §2 — A1Q1 → A1Q6).
pub(crate) static DEFS_ACT1: [DefAct1; 6] = [
    // A1Q1 — Den of Evil : INACTIF → (Akara) ACTIF → (Cave 1 id 8) ENTRÉ+
    // compteur → PRÊT → (Akara) COMPLET. Compteur MVP = 6 kills (densité du
    // port ; retail « tous les monstres » [ARRANGÉ-MVP]).
    DefAct1 {
        id: "a1q1",
        name: "Den of Evil",
        steps: &[
            StepDef { desc: "Parlez à Akara au Camp des Rousses.", trig: Trig::Talk("akara"), count: 1 },
            StepDef {
                desc: "Entrez dans le Repaire du Mal (Cave 1, id 8).",
                trig: Trig::Zone(lvl::DEN_OF_EVIL),
                count: 1,
            },
            StepDef {
                desc: "Purifiez le Repaire : abattez tous ses monstres.",
                trig: Trig::Kill(&[]), // joker géré par `step_matches` (KILL_ANY)
                count: 6,
            },
        ],
        reward_npc: Some("akara"),
        rewards: &[ActReward::SkillPoint(1), ActReward::Xp(100)],
    },
    // A1Q2 — Sisters' Burial Grounds : activation immédiate [ARRANGÉ-MVP] ;
    // Blood Raven = corruptrogue2 / bloodraven ; Kashya donne la Rogue.
    DefAct1 {
        id: "a1q2",
        name: "Sisters' Burial Grounds",
        steps: &[StepDef {
            desc: "Tuez Blood Raven au cimetière (id 17).",
            trig: Trig::Kill(&["bloodraven", "corruptrogue2"]),
            count: 1,
        }],
        reward_npc: Some("kashya"),
        rewards: &[ActReward::MercenaryRogue, ActReward::Xp(200)],
    },
    // A1Q3 — Tools of the Trade : The Smith (caserne id 28) → Malus `hdm`
    // (patch_weapons.txt:91) → Charsi (imbu).
    DefAct1 {
        id: "a1q3",
        name: "Tools of the Trade",
        steps: &[
            StepDef { desc: "Tuez le Smith dans la Caserne (id 28).", trig: Trig::Kill(&["smith"]), count: 1 },
            StepDef { desc: "Ramassez le Horadric Malus (hdm).", trig: Trig::Item("hdm"), count: 1 },
        ],
        reward_npc: Some("charsi"),
        rewards: &[ActReward::CharsiImbue, ActReward::Xp(300)],
    },
    // A1Q4 — The Search for Cain : arbre d'Inifuss (Dark Wood 5) → `bks` →
    // Akara (déchiffrage) → pierres de Cairn (Stony Field 4) → Tristram 38 →
    // Cain libéré. Bit 10 « niveau vache » : hors scope strict.
    DefAct1 {
        id: "a1q4",
        name: "The Search for Cain",
        steps: &[
            StepDef { desc: "Rejoignez l'Arbre d'Inifuss (Dark Wood).", trig: Trig::Zone(lvl::DARK_WOOD), count: 1 },
            StepDef { desc: "Récoltez le parchemin d'écorce (bks).", trig: Trig::Item("bks"), count: 1 },
            StepDef { desc: "Faites déchiffrer le parchemin par Akara.", trig: Trig::Talk("akara"), count: 1 },
            StepDef { desc: "Activez les pierres de Cairn (Stony Field).", trig: Trig::Zone(lvl::STONY_FIELD), count: 1 },
            StepDef { desc: "Entrez à Tristram par le portail.", trig: Trig::Zone(lvl::TRISTRAM), count: 1 },
        ],
        reward_npc: Some("cain"),
        rewards: &[ActReward::Item("rin"), ActReward::CainFreeIdentify, ActReward::Xp(300)],
    },
    // A1Q5 — The Forgotten Tower : Tour (id 20) → Countess ; loot = récompense
    // (TC « Countess » or ×1280 + runes — MVP : or fixe [INVENTÉ]).
    DefAct1 {
        id: "a1q5",
        name: "The Forgotten Tower",
        steps: &[
            StepDef { desc: "Descendez dans la Tour oubliée (id 20).", trig: Trig::Zone(lvl::FORGOTTEN_TOWER), count: 1 },
            StepDef { desc: "Abattez la Comtesse.", trig: Trig::Kill(&["countess", "corruptrogue3"]), count: 1 },
        ],
        reward_npc: None,
        rewards: &[ActReward::Xp(500), ActReward::Gold(500)],
    },
    // A1Q6 — Sisters to the Slaughter : Catacombs 4 (id 37) → Andariel →
    // Warriv → UnlockAccessAct(2) + SetProgressionByte(+1). PORTEUSE.
    DefAct1 {
        id: "a1q6",
        name: "Sisters to the Slaughter",
        steps: &[
            StepDef { desc: "Trouvez le repaire d'Andariel (Catacombes 4, id 37).", trig: Trig::Zone(lvl::CATACOMBS_4), count: 1 },
            StepDef { desc: "Tuez Andariel.", trig: Trig::Kill(&["andariel"]), count: 1 },
        ],
        reward_npc: Some("warriv"),
        rewards: &[ActReward::UnlockAccessAct(2), ActReward::Xp(1000), ActReward::Gold(250)],
    },
];

// ---------------------------------------------------------------------------
// État + moteur
// ---------------------------------------------------------------------------

/// État d'exécution d'une quête (sérialisé, façon flags u16 .d2s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActRun {
    /// Phase courante.
    pub phase: ActPhase,
    /// Index de l'étape courante (figé à la dernière quand la quête sort d'Active).
    pub step: usize,
    /// Occurrences acquises sur l'étape courante.
    pub count: u32,
    /// Flags u16 .d2s (bits cf. constantes FLAG_*).
    pub flags: u16,
}

/// Journal de quêtes de l'acte I (spec quests-actes.md). Authoring statique ;
/// seul l'état est sérialisé ([`ActQuests::to_json`]/[`ActQuests::from_json`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActQuests {
    runs: [ActRun; 6],
}

impl Default for ActQuests {
    fn default() -> Self {
        Self::new()
    }
}

impl ActQuests {
    /// Journal neuf : tout INACTIF, flags 0.
    pub fn new() -> ActQuests {
        ActQuests {
            runs: [ActRun { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0 }; 6],
        }
    }

    /// Défs (authoring) dans l'ordre des slots (crate — types d'authoring
    /// internes ; l'UI passe par [`ActQuests::run`]/`ids`).
    pub(crate) fn defs(&self) -> &'static [DefAct1] {
        &DEFS_ACT1
    }

    /// Ids des slots (`"a1q1"`…`"a1q6"`).
    pub fn ids(&self) -> [&'static str; 6] {
        ACT1_QUEST_IDS
    }

    /// État d'une quête par id (`None` si id inconnu).
    pub fn run(&self, id: &str) -> Option<&ActRun> {
        ACT1_QUEST_IDS.iter().position(|s| *s == id).map(|i| &self.runs[i])
    }

    /// Phases dans l'ordre des slots (diff/events UI).
    pub fn phases(&self) -> [ActPhase; 6] {
        let mut out = [ActPhase::Inactive; 6];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.phase;
        }
        out
    }

    /// Flags u16 dans l'ordre des slots — image directe des u16 .d2s de
    /// l'acte I (offsets [`ActQuests::d2s_flag_offset`]).
    pub fn quest_flags(&self) -> [u16; 6] {
        let mut out = [0u16; 6];
        for (i, r) in self.runs.iter().enumerate() {
            out[i] = r.flags;
        }
        out
    }

    /// Gating d'accès (spec §1.4) : acte 1 toujours ouvert ; acte 2 = A1Q6
    /// bit 0 (remise Warriv) ; au-delà : jamais (hors scope acte I).
    pub fn access_act(&self, act: u8) -> bool {
        match act {
            1 => true,
            2 => self.runs[5].flags & FLAG_DONE != 0,
            _ => false,
        }
    }

    /// Octet de progression .d2s 0x0025 (spec §1.2) — MVP : nb de quêtes
    /// porteuses complétées (0 ou 1 à l'acte I).
    pub fn progression_byte(&self) -> u8 {
        u8::from(self.access_act(2))
    }

    /// Fait avancer le journal avec un événement ; renvoie les récompenses
    /// rendues (quêtes sans PNJ de remise — A1Q5 — et remises NPC au moment
    /// du dialogue). Un événement = une progression par quête, hors ordre ignoré.
    pub fn on_event(&mut self, ev: &ActEvent) -> Vec<ActReward> {
        let mut out = Vec::new();
        for slot in 0..6 {
            match self.runs[slot].phase {
                ActPhase::Inactive => {
                    // L'étape 0 démarre la quête (activation).
                    if self.step_matches(slot, 0, ev) {
                        self.runs[slot].phase = ActPhase::Active;
                        if matches!(ev, ActEvent::NpcTalked { .. }) {
                            self.runs[slot].flags |= FLAG_GIVEN_BY_NPC;
                        }
                        self.advance_after_step(slot, 0, &mut out);
                    }
                }
                ActPhase::Active => {
                    let step = self.runs[slot].step;
                    if self.step_matches(slot, step, ev) {
                        // bits spécifiques en cours de route (A1Q1 bit 4)
                        if slot == 0 && matches!(ev, ActEvent::ZoneEntered { level_id: lvl::DEN_OF_EVIL }) {
                            self.runs[slot].flags |= FLAG_DEN_ENTERED;
                        }
                        self.runs[slot].count += 1;
                        let req = DEFS_ACT1[slot].steps[step].count.max(1);
                        if self.runs[slot].count >= req {
                            self.advance_after_step(slot, step, &mut out);
                        }
                    }
                }
                ActPhase::RewardPending => {
                    if let ActEvent::NpcTalked { npc } = ev {
                        if DEFS_ACT1[slot]
                            .reward_npc
                            .map_or(false, |r| npc.eq_ignore_ascii_case(r))
                        {
                            self.runs[slot].phase = ActPhase::Done;
                            self.runs[slot].flags =
                                (self.runs[slot].flags & !FLAG_REWARD_PENDING) | FLAG_DONE;
                            out.extend(DEFS_ACT1[slot].rewards.iter().copied());
                        }
                    }
                }
                ActPhase::Done => {} // terminée : ignore tout
            }
        }
        out
    }

    /// L'étape `step` de la quête `slot` réagit-elle à cet événement ?
    /// Joker A1Q1 : étape sans alias (Kill(&[])) = n'importe quel kill.
    fn step_matches(&self, slot: usize, step: usize, ev: &ActEvent) -> bool {
        match DEFS_ACT1[slot].steps.get(step) {
            Some(StepDef { trig: Trig::Kill(aliases), .. }) if aliases.is_empty() => {
                matches!(ev, ActEvent::MonsterKilled { .. })
            }
            Some(s) => s.trig.matches(ev),
            None => false,
        }
    }

    /// Étape `done_step` franchie : suivante, ou sortie d'Active
    /// (RewardPending / Done direct selon `reward_npc`).
    fn advance_after_step(&mut self, slot: usize, done_step: usize, out: &mut Vec<ActReward>) {
        let def = &DEFS_ACT1[slot];
        if done_step + 1 < def.steps.len() {
            self.runs[slot].step = done_step + 1;
            self.runs[slot].count = 0;
            return;
        }
        let last = def.steps.len() - 1;
        self.runs[slot].step = last;
        self.runs[slot].count = def.steps[last].count.max(1);
        match def.reward_npc {
            Some(_) => {
                self.runs[slot].phase = ActPhase::RewardPending;
                self.runs[slot].flags |= FLAG_REWARD_PENDING;
            }
            None => {
                // Loot = récompense (A1Q5) : complétion immédiate.
                self.runs[slot].phase = ActPhase::Done;
                self.runs[slot].flags = (self.runs[slot].flags & !FLAG_REWARD_PENDING) | FLAG_DONE;
                out.extend(def.rewards.iter().copied());
            }
        }
    }

    // -- mapping .d2s (spec §1.3 — table golden) ----------------------------

    /// Offset absolu du u16 de quête de l'acte I : section quêtes .d2s
    /// (345), slots 347/349/351/353/355/357 en Normal, +96 par difficulté
    /// au-delà (Cauchemar 443-453, Enfer 539-549). `slot` 0..=5,
    /// `difficulty` 0..=2.
    pub fn d2s_flag_offset(slot: usize, difficulty: usize) -> u32 {
        347 + 2 * slot as u32 + 96 * difficulty as u32
    }

    // -- persistance (contrat save.rs — champ additif `quest_act1`) ---------

    /// Sérialise l'ÉTAT SEUL (JSON versionné, ids inclus, lisible).
    pub fn to_json(&self) -> String {
        #[derive(Serialize)]
        struct RunSave<'a> {
            id: &'a str,
            phase: ActPhase,
            step: usize,
            count: u32,
            flags: u16,
        }
        #[derive(Serialize)]
        struct State<'a> {
            version: u32,
            quests: Vec<RunSave<'a>>,
        }
        let quests = self
            .runs
            .iter()
            .zip(ACT1_QUEST_IDS.iter())
            .map(|(r, id)| RunSave { id, phase: r.phase, step: r.step, count: r.count, flags: r.flags })
            .collect();
        let s = State { version: STATE_VERSION, quests };
        serde_json::to_string(&s).expect("d2rules: sérialisation quest_act1")
    }

    /// Recharge un état produit par [`ActQuests::to_json`]. Validation stricte
    /// (version, ids dans l'ordre, bornes d'étapes, flags cohérents avec la
    /// phase) — tout écart ⇒ Err(()) : l'appelant garde son état précédent.
    pub fn from_json(json: &str) -> Result<ActQuests, ()> {
        #[derive(Deserialize)]
        struct RunSave {
            id: String,
            phase: ActPhase,
            step: usize,
            count: u32,
            flags: u16,
        }
        #[derive(Deserialize)]
        struct State {
            version: u32,
            quests: Vec<RunSave>,
        }
        let s: State = serde_json::from_str(json).map_err(|_| ())?;
        if s.version != STATE_VERSION || s.quests.len() != 6 {
            return Err(());
        }
        let mut runs = [ActRun { phase: ActPhase::Inactive, step: 0, count: 0, flags: 0 }; 6];
        for (slot, save) in s.quests.into_iter().enumerate() {
            if save.id != ACT1_QUEST_IDS[slot] {
                return Err(());
            }
            let def = &DEFS_ACT1[slot];
            let last = def.steps.len() - 1;
            let ok = match save.phase {
                ActPhase::Inactive => save.step == 0 && save.count == 0 && save.flags == 0,
                ActPhase::Active => {
                    // Active : l'étape courante n'est pas complète — dernier
                    // index inclus (compteur de la dernière étape en cours).
                    save.step <= last
                        && save.count < def.steps[save.step].count.max(1)
                        && save.flags & !(FLAG_GIVEN_BY_NPC | FLAG_DEN_ENTERED) == 0
                        && (slot != 0 || save.step < 2 || save.flags & FLAG_DEN_ENTERED != 0)
                }
                ActPhase::RewardPending => {
                    def.reward_npc.is_some()
                        && save.step == last
                        && save.count == def.steps[last].count.max(1)
                        && save.flags & FLAG_REWARD_PENDING != 0
                        && save.flags & FLAG_DONE == 0
                }
                ActPhase::Done => {
                    save.step == last
                        && save.count == def.steps[last].count.max(1)
                        && save.flags & FLAG_DONE != 0
                        && save.flags & FLAG_REWARD_PENDING == 0
                }
            };
            if !ok {
                return Err(());
            }
            runs[slot] = ActRun { phase: save.phase, step: save.step, count: save.count, flags: save.flags };
        }
        Ok(ActQuests { runs })
    }
}

const STATE_VERSION: u32 = 1;

// ---------------------------------------------------------------------------
// Branchement World (bloc impl ADDITIF — save `quest_act1`, events d'acte)
// ---------------------------------------------------------------------------

/// Objet de quête générique (récompense `Item`) — libellé brut au MVP, le
/// nommage riche côté items est un autre chantier.
fn quest_item_def(code: &str) -> crate::ItemDef {
    crate::ItemDef { code: code.to_string(), name: code.to_string(), kind: crate::ItemKind::Misc, amount: 0 }
}

impl World {
    /// Branche le journal spec de l'acte I (W-étape 4) : les events passés à
    /// [`World::quest_act1_event`] y avancent les quêtes. `None` par défaut.
    pub fn set_quest_act1(&mut self, log: ActQuests) {
        self.quests_act1 = Some(log);
    }

    /// Journal spec branché (lecture).
    pub fn quest_act1(&self) -> Option<&ActQuests> {
        self.quests_act1.as_ref()
    }

    /// Journal spec branché (mutation directe — inspection tests/UI).
    pub fn quest_act1_mut(&mut self) -> Option<&mut ActQuests> {
        self.quests_act1.as_mut()
    }

    /// Point d'entrée unique de l'intégrateur : avance le journal spec avec un
    /// event de gameplay, applique les récompenses motrices (XP → paliers,
    /// or → bourse, skill point → réserve, objet → inventaire/sol), émet les
    /// events nécessaires à l'UI ([`GameEvent::ActQuestChanged`] par quête
    /// changée de phase, [`GameEvent::ActUnlocked`] au déblocage d'acte) et
    /// renvoie les récompenses (portées aussi par le journal pour UI).
    /// Sans journal branché : no-op, renvoie vide.
    pub fn quest_act1_event(&mut self, ev: &ActEvent) -> Vec<ActReward> {
        let Some(log) = self.quests_act1.as_mut() else {
            return Vec::new();
        };
        let before = log.phases();
        let gate_before = log.access_act(2);
        let rewards = log.on_event(ev);
        let after = log.phases();
        for (slot, (b, a)) in before.iter().zip(after.iter()).enumerate() {
            if b != a {
                self.pending_events.push(crate::GameEvent::ActQuestChanged {
                    id: ACT1_QUEST_IDS[slot].to_string(),
                    phase: a.as_str().to_string(),
                });
            }
        }
        if !gate_before && log.access_act(2) {
            self.pending_events.push(crate::GameEvent::ActUnlocked { act: 2 });
        }
        for r in &rewards {
            match *r {
                ActReward::Xp(n) => {
                    let mut leveled = Vec::new();
                    self.apply_xp(n, &mut leveled);
                    self.pending_events.extend(leveled);
                }
                ActReward::Gold(n) => {
                    self.hero.stats.gold = self.hero.stats.gold.saturating_add(n);
                    self.pending_events.push(crate::GameEvent::GoldGained { amount: n });
                }
                ActReward::SkillPoint(n) => self.grant_skill_points(n),
                ActReward::Item(code) => {
                    let def = quest_item_def(code);
                    if let Some(slot) = self.hero.inv.iter().position(|s| s.is_none()) {
                        self.hero.inv[slot] = Some(def);
                    } else {
                        let (x, y) = (self.hero_ent.x, self.hero_ent.y);
                        self.items.push(crate::ItemOnGround { def, x, y });
                    }
                }
                // Portés par le journal (flags d'acte) — l'UI les consomme
                // via la valeur de retour / `quest_act1()`.
                ActReward::MercenaryRogue
                | ActReward::CharsiImbue
                | ActReward::CainFreeIdentify
                | ActReward::UnlockAccessAct(_) => {}
            }
        }
        rewards
    }
}

// ---------------------------------------------------------------------------
// Tests : machine A1Q1→A1Q6, triggers réels, gating, flags/d2s golden, save.
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
    fn phase(log: &ActQuests, id: &str) -> ActPhase {
        log.run(id).unwrap().phase
    }
    fn flags(log: &ActQuests, id: &str) -> u16 {
        log.run(id).unwrap().flags
    }
    fn rewards_of(log: &mut ActQuests, ev: &ActEvent) -> Vec<ActReward> {
        log.on_event(ev)
    }

    // -- A1Q1 : machine complète (activation PNJ → zone → compteur → remise) --

    #[test]
    fn a1q1_den_of_evil_full_machine_and_flags() {
        let mut log = ActQuests::new();
        // Hors ordre : kills/zone avant l'intro Akara — rien.
        assert!(rewards_of(&mut log, &kill("FA")).is_empty());
        assert!(rewards_of(&mut log, &zone(8)).is_empty());
        assert_eq!(phase(&log, "a1q1"), ActPhase::Inactive);
        // Intro Akara : ACTIF, bit 2 « donnée par PNJ ».
        assert!(rewards_of(&mut log, &talk("akara")).is_empty());
        assert_eq!(phase(&log, "a1q1"), ActPhase::Active);
        assert_eq!(flags(&log, "a1q1"), FLAG_GIVEN_BY_NPC);
        assert_eq!((log.run("a1q1").unwrap().step, log.run("a1q1").unwrap().count), (1, 0));
        // Entrée Cave 1 (id 8) : étape franchie + bit 4 « repaire entré ».
        assert!(rewards_of(&mut log, &zone(8)).is_empty());
        assert_eq!(flags(&log, "a1q1"), FLAG_GIVEN_BY_NPC | FLAG_DEN_ENTERED);
        assert_eq!(log.run("a1q1").unwrap().step, 2);
        // Compteur « monstres restants » : 6 kills, un seul événement = un kill.
        for k in 1..=5 {
            assert!(rewards_of(&mut log, &kill("ZM")).is_empty(), "kill {k} : progression partielle");
            assert_eq!(log.run("a1q1").unwrap().count, k);
        }
        assert_eq!(phase(&log, "a1q1"), ActPhase::Active);
        // 6e kill : toutes les étapes franchies → PRÊT (bit 1), pas de récompense encore.
        assert!(rewards_of(&mut log, &kill("SI")).is_empty());
        assert_eq!(phase(&log, "a1q1"), ActPhase::RewardPending);
        assert_eq!(flags(&log, "a1q1"), FLAG_GIVEN_BY_NPC | FLAG_DEN_ENTERED | FLAG_REWARD_PENDING);
        // Retour Akara : COMPLET (bit 0, bit 1 effacé) + récompenses.
        let r = rewards_of(&mut log, &talk("akara"));
        assert_eq!(phase(&log, "a1q1"), ActPhase::Done);
        assert_eq!(flags(&log, "a1q1"), FLAG_GIVEN_BY_NPC | FLAG_DEN_ENTERED | FLAG_DONE);
        assert_eq!(r, vec![ActReward::SkillPoint(1), ActReward::Xp(100)]);
        // Rejouer : plus rien.
        assert!(rewards_of(&mut log, &talk("akara")).is_empty());
    }

    #[test]
    fn a1q1_wrong_zone_or_npc_does_not_advance() {
        let mut log = ActQuests::new();
        log.on_event(&talk("akara"));
        // Mauvais niveau (Blood Moor id 2) : rien.
        assert!(rewards_of(&mut log, &zone(2)).is_empty());
        assert_eq!(log.run("a1q1").unwrap().step, 1);
        // Mauvais NPC : rien.
        assert!(rewards_of(&mut log, &talk("kashya")).is_empty());
        assert_eq!(phase(&log, "a1q1"), ActPhase::Active);
    }

    // -- A1Q2 : Blood Raven par code monstats OU superunique, remise Kashya --

    #[test]
    fn a1q2_blood_raven_aliases_then_kashya() {
        // Voie 1 : record de quête bloodraven.
        let mut log = ActQuests::new();
        assert!(rewards_of(&mut log, &kill("bloodraven")).is_empty()); // active+franchit
        assert_eq!(phase(&log, "a1q2"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("kashya"));
        assert_eq!(phase(&log, "a1q2"), ActPhase::Done);
        assert_eq!(r, vec![ActReward::MercenaryRogue, ActReward::Xp(200)]);
        // Voie 2 : classe de base corruptrogue2 + superunique « bloodraven ».
        let mut log2 = ActQuests::new();
        assert!(rewards_of(&mut log2, &kill_su("corruptrogue2", "BloodRaven")).is_empty());
        assert_eq!(phase(&log2, "a1q2"), ActPhase::RewardPending);
        // Autre boss : rien.
        let mut log3 = ActQuests::new();
        assert!(rewards_of(&mut log3, &kill("smith")).is_empty());
        assert_eq!(phase(&log3, "a1q2"), ActPhase::Inactive);
    }

    // -- A1Q3 : Smith → hdm → Charsi -----------------------------------------

    #[test]
    fn a1q3_smith_malus_charsi_imbue() {
        let mut log = ActQuests::new();
        // Objet avant le kill : hors ordre, ignoré.
        assert!(rewards_of(&mut log, &item("hdm")).is_empty());
        assert_eq!(phase(&log, "a1q3"), ActPhase::Inactive);
        assert!(rewards_of(&mut log, &kill_su("smith", "The Smith")).is_empty());
        assert_eq!(log.run("a1q3").unwrap().step, 1);
        // Mauvais objet (bks) : rien.
        assert!(rewards_of(&mut log, &item("bks")).is_empty());
        assert!(rewards_of(&mut log, &item("hdm")).is_empty());
        assert_eq!(phase(&log, "a1q3"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("charsi"));
        assert_eq!(r, vec![ActReward::CharsiImbue, ActReward::Xp(300)]);
        assert_eq!(phase(&log, "a1q3"), ActPhase::Done);
    }

    // -- A1Q4 : chaîne Cain complète, ordre strict ----------------------------

    #[test]
    fn a1q4_cain_chain_order_enforced() {
        let mut log = ActQuests::new();
        // Tout tenter en vrac : rien ne doit passer.
        assert!(rewards_of(&mut log, &talk("cain")).is_empty());
        assert!(rewards_of(&mut log, &zone(38)).is_empty());
        assert_eq!(phase(&log, "a1q4"), ActPhase::Inactive);
        for ev in [zone(5), item("bks"), talk("akara"), zone(4), zone(38)] {
            assert!(rewards_of(&mut log, &ev).is_empty(), "étape intermédiaire sans récompense");
        }
        assert_eq!(phase(&log, "a1q4"), ActPhase::RewardPending);
        let r = rewards_of(&mut log, &talk("cain"));
        assert_eq!(
            r,
            vec![ActReward::Item("rin"), ActReward::CainFreeIdentify, ActReward::Xp(300)]
        );
        assert_eq!(phase(&log, "a1q4"), ActPhase::Done);
    }

    #[test]
    fn a1q4_wrong_zone_breaks_chain() {
        let mut log = ActQuests::new();
        log.on_event(&zone(5));
        log.on_event(&item("bks"));
        log.on_event(&talk("akara"));
        // Pierres de Cairn : Stony Field = 4 — Blood Moor (2) ne compte pas.
        assert!(rewards_of(&mut log, &zone(2)).is_empty());
        assert_eq!(log.run("a1q4").unwrap().step, 3);
        log.on_event(&zone(4));
        assert_eq!(log.run("a1q4").unwrap().step, 4);
    }

    // -- A1Q5 : Tour oubliée, complétion immédiate (loot = récompense) -------

    #[test]
    fn a1q5_tower_completes_on_countess_kill() {
        let mut log = ActQuests::new();
        assert!(rewards_of(&mut log, &zone(20)).is_empty());
        assert_eq!(phase(&log, "a1q5"), ActPhase::Active);
        // Countess par superunique ou par classe corruptrogue3.
        let r = rewards_of(&mut log, &kill_su("corruptrogue3", "The Countess"));
        assert_eq!(r, vec![ActReward::Xp(500), ActReward::Gold(500)]);
        assert_eq!(phase(&log, "a1q5"), ActPhase::Done);
        assert_eq!(flags(&log, "a1q5"), FLAG_DONE);
    }

    // -- A1Q6 : Andariel → Warriv → gating acte II ----------------------------

    #[test]
    fn a1q6_andariel_gates_act2_via_warriv() {
        let mut log = ActQuests::new();
        assert!(log.access_act(1));
        assert!(!log.access_act(2));
        assert_eq!(log.progression_byte(), 0);
        log.on_event(&zone(37));
        // Andariel morte : PRÊT mais l'acte II reste FERMÉ (remise Warriv).
        assert!(rewards_of(&mut log, &kill("andariel")).is_empty());
        assert_eq!(phase(&log, "a1q6"), ActPhase::RewardPending);
        assert_eq!(flags(&log, "a1q6"), FLAG_REWARD_PENDING);
        assert!(!log.access_act(2), "l'accès acte II attend la caravane de Warriv");
        let r = rewards_of(&mut log, &talk("warriv"));
        assert_eq!(r, vec![ActReward::UnlockAccessAct(2), ActReward::Xp(1000), ActReward::Gold(250)]);
        assert_eq!(phase(&log, "a1q6"), ActPhase::Done);
        assert_eq!(flags(&log, "a1q6"), FLAG_DONE);
        assert!(log.access_act(2));
        assert_eq!(log.progression_byte(), 1);
        assert!(!log.access_act(3), "acte III hors scope acte I");
    }

    // -- Golden : flags u16 de la chaîne complète -----------------------------

    #[test]
    fn golden_full_act1_flags() {
        let mut log = ActQuests::new();
        for ev in [
            talk("akara"),
            zone(8),
            kill("FA"),
            kill("FA"),
            kill("ZM"),
            kill("ZM"),
            kill("SI"),
            kill("SK"),
            kill("bloodraven"),
            talk("kashya"),
            kill("smith"),
            item("hdm"),
            talk("charsi"),
            zone(5),
            item("bks"),
            talk("akara"),
            zone(4),
            zone(38),
            talk("cain"),
            zone(20),
            kill_su("countess", "The Countess"),
            zone(37),
            kill("andariel"),
            talk("warriv"),
        ] {
            log.on_event(&ev);
        }
        assert!(
            log.phases().iter().all(|p| *p == ActPhase::Done),
            "les 6 quêtes sont Done : {:?}",
            log.phases()
        );
        assert_eq!(log.quest_flags(), [0x0015, 0x0001, 0x0001, 0x0001, 0x0001, 0x0001]);
        assert!(log.access_act(2));
        // Un event isolé ne franchit jamais deux étapes : la machine ci-dessus
        // ne aurait pas pu « sauter » l'intro d'A1Q1.
        let mut fresh = ActQuests::new();
        fresh.on_event(&zone(8));
        assert_eq!(phase(&fresh, "a1q1"), ActPhase::Inactive);
    }

    // -- Golden : offsets .d2s des u16 de l'acte I (spec §1.3) ----------------

    #[test]
    fn golden_d2s_offsets_act1() {
        let normal = [347, 349, 351, 353, 355, 357];
        let nm = [443, 445, 447, 449, 451, 453];
        let hell = [539, 541, 543, 545, 547, 549];
        for slot in 0..6 {
            assert_eq!(ActQuests::d2s_flag_offset(slot, 0), normal[slot]);
            assert_eq!(ActQuests::d2s_flag_offset(slot, 1), nm[slot]);
            assert_eq!(ActQuests::d2s_flag_offset(slot, 2), hell[slot]);
        }
    }

    // -- Golden : JSON de save (état complet) ---------------------------------

    #[test]
    fn golden_json_save_full_run() {
        let mut log = ActQuests::new();
        for ev in [
            talk("akara"),
            zone(8),
            kill("FA"),
            kill("FA"),
            kill("ZM"),
            kill("ZM"),
            kill("SI"),
            kill("SK"),
            kill("bloodraven"),
            talk("kashya"),
            kill("smith"),
            item("hdm"),
            talk("charsi"),
            zone(5),
            item("bks"),
            talk("akara"),
            zone(4),
            zone(38),
            talk("cain"),
            zone(20),
            kill("countess"),
            zone(37),
            kill("andariel"),
            talk("warriv"),
        ] {
            log.on_event(&ev);
        }
        let v: serde_json::Value = serde_json::from_str(&log.to_json()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({
                "version": 1,
                "quests": [
                    {"id": "a1q1", "phase": "done", "step": 2, "count": 6, "flags": 21},
                    {"id": "a1q2", "phase": "done", "step": 0, "count": 1, "flags": 1},
                    {"id": "a1q3", "phase": "done", "step": 1, "count": 1, "flags": 1},
                    {"id": "a1q4", "phase": "done", "step": 4, "count": 1, "flags": 1},
                    {"id": "a1q5", "phase": "done", "step": 1, "count": 1, "flags": 1},
                    {"id": "a1q6", "phase": "done", "step": 1, "count": 1, "flags": 1},
                ]
            })
        );
        let log2 = ActQuests::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        assert_eq!(log.to_json(), log2.to_json());
    }

    #[test]
    fn json_round_trip_partial_states() {
        let mut log = ActQuests::new();
        log.on_event(&talk("akara"));
        log.on_event(&zone(8));
        for _ in 0..4 {
            log.on_event(&kill("FA"));
        }
        log.on_event(&kill("bloodraven")); // q2 démarre aussi
        let mut log2 = ActQuests::from_json(&log.to_json()).unwrap();
        assert_eq!(log, log2);
        // Le kill joker d'A1Q2 compte AUSSI dans le compteur « monstres
        // restants » d'A1Q1 (toute mort avance le joker) : 4 + 1 = 5.
        assert_eq!(log2.run("a1q1").unwrap().count, 5);
        assert_eq!(phase(&log2, "a1q2"), ActPhase::RewardPending);
        // Le rechargé poursuit exactement.
        log.on_event(&kill("ZM"));
        log.on_event(&kill("ZM"));
        log2.on_event(&kill("ZM"));
        log2.on_event(&kill("ZM"));
        assert_eq!(phase(&log, "a1q1"), ActPhase::RewardPending);
        assert_eq!(phase(&log2, "a1q1"), ActPhase::RewardPending);
        assert_eq!(log, log2);
    }

    #[test]
    fn json_rejects_corrupted_states() {
        let base = ActQuests::new();
        // Version inconnue / JSON invalide / mauvaise longueur.
        assert!(ActQuests::from_json(r#"{"version":9,"quests":[]}"#).is_err());
        assert!(ActQuests::from_json("{oops").is_err());
        assert!(ActQuests::from_json(r#"{"version":1,"quests":[]}"#).is_err());
        // Id inconnu.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][2]["id"] = "zz".into();
        assert!(ActQuests::from_json(&v.to_string()).is_err());
        // Phase incohérente : Done sans bit 0.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][0] = serde_json::json!({"id":"a1q1","phase":"done","step":2,"count":6,"flags":0});
        assert!(ActQuests::from_json(&v.to_string()).is_err());
        // RewardPending sans PNJ de remise (a1q5 n'en a pas).
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][4] =
            serde_json::json!({"id":"a1q5","phase":"reward_pending","step":1,"count":1,"flags":2});
        assert!(ActQuests::from_json(&v.to_string()).is_err());
        // Inactive avec résidus.
        let mut v: serde_json::Value = serde_json::from_str(&base.to_json()).unwrap();
        v["quests"][1] = serde_json::json!({"id":"a1q2","phase":"inactive","step":1,"count":1,"flags":2});
        assert!(ActQuests::from_json(&v.to_string()).is_err());
        // Contrôle positif : la base propre passe.
        assert!(ActQuests::from_json(&base.to_json()).is_ok());
    }

    // -- Authoring : invariants de slots --------------------------------------

    #[test]
    fn authoring_slot_order_matches_d2s_slots() {
        assert_eq!(ACT1_QUEST_IDS, ["a1q1", "a1q2", "a1q3", "a1q4", "a1q5", "a1q6"]);
        assert_eq!(DEFS_ACT1[0].name, "Den of Evil");
        assert_eq!(DEFS_ACT1[5].name, "Sisters to the Slaughter");
        // A1Q5 est la seule sans PNJ de remise (loot = récompense).
        assert!(DEFS_ACT1.iter().enumerate().all(|(i, d)| d.reward_npc.is_some() != (i == 4)));
        // A1Q6 est la seule à débloquer un acte.
        assert_eq!(
            DEFS_ACT1.iter().flat_map(|d| d.rewards.iter()).filter(|r| matches!(r, ActReward::UnlockAccessAct(_))).count(),
            1
        );
    }
}
