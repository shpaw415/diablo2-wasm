//! NPC DE VILLE — W16 (VILLES VIVANTES, `finish-monde.md` §W16 / `finish-jeu.md` #14).
//!
//! Roster data-driven du Camp des Rousses (Acte I) : spawn, patrouilles réelles,
//! rôles (soin / forge / mercenaires / gamble / caravane / érudit), interaction
//! au clic et émission de [`QuestEvent::NpcTalked`] vers le journal de quêtes
//! (contrat `impl-quetes.md` §CONTRAT : « Aucune source avant W16 » — la voici).
//!
//! # Données réelles (preuves, assets/ v1.12)
//!
//! * **Records monstats** : `assets/patch_monstats.txt` lignes Id
//!   `cain1`(hcIdx 146, Code DC), `gheed`(147, GH), `akara`(148, PS),
//!   `kashya`(150, RC), `charsi`(154, CI), `warriv1`(155, WA) — tous
//!   `MonType=human`, `AI=Npc` (`NpcOutOfTown` pour cain1), `Velocity=15`,
//!   `enabled=1`. Tests [`tests::roster_matches_real_tables`] ancre les codes.
//! * **Positions + patrouilles** : section PATH de `assets/map/TownN1.ds1`
//!   (v18, 340 octets après la section OBJECTS finissant à l'offset 57 312).
//!   Format vérifié sur le fichier : `[nb objets=5]` puis par objet
//!   `[nb points, x, y, points × (x, y, action)]` — 5 groupes tombant
//!   EXACTEMENT sur 5 objets `type=1` de la section OBJECTS (94,27 / 95,87 /
//!   192,16 / 126,45 / 153,46). Les positions et points de patrouille de
//!   [`NPCS`] sont CES coordonnées, en cellules de grille d2rules (l'unité
//!   sous-tuile DS1 = la cellule d2rules, même convention que `waypoints.rs`).
//! * **Affectation NPC → objet PATH : ASSUMÉE (MVP documenté).** Aucune table
//!   ne nomme les objets PATH (le lookup original est codé en dur dans les
//!   moteurs, cf. `finish-monde.md` W16 « lookup Go codé en dur ~7 900
//!   lignes »). Affectation retenue par géographie du camp retail : Akara +
//!   Kashya au CENTRE (126,45 / 153,46 — contre la pierre du waypoint
//!   (144,51), comme en D2), Charsi au nord-ouest (94,27, la forge), Gheed au
//!   sud-ouest (95,87, sa charrette), Warriv au nord-est (192,16, vers la
//!   sortie est du camp (282,107)). Chaque entrée porte `pos_assumed`.
//! * **Deckard Cain** : record `cain1` réel, AUCUNE entrée PATH au camp (il
//!   n'y apparaît qu'après sa délivrance à Tristram). MVP : posé IMMOBILE à
//!   côté d'Akara pour que la chaîne q3 (`NpcTalked{cain}`) soit jouable et
//!   testable dans le port — extension MVP documentée, `pos_assumed: true`.
//!
//! # MVP assumés (dits clairement)
//!
//! * **Sprites** : aucun DCC de NPC dans le manifest (vérifié : `assets/dcc/`
//!   ne porte que les tokens héros/monstres) — le rendu passe par des
//!   MARQUEURS d'overlay (demo/npc-ui.js) ; `Entity` reste animé (NU/WL) pour
//!   la cohérence du contrat.
//! * **Vitesse de patrouille** : la conversion exacte `Velocity=15` (index
//!   d'animation monstats → yards/s) n'est pas portée — vitesse MVP
//!   [`NPC_WALK_SPEED`] = 2.0 cellules/s, calme et distincte du héros (6).
//! * **Malédictions** : aucun système de malédiction au port (MVP) — le soin
//!   d'Akara remet vie ET mana pleins ; « combat des malédictions » = rien à
//!   purger (assumé).
//! * **Réparation (Charsi)** : la durabilité/équipement n'existe pas au MVP —
//!   [`World::npc_repair`] expose l'API et répond `NothingToRepair`.
//! * **Mercenaires (Kashya)** : domaine d'un autre chantier —
//!   [`World::npc_resurrect_mercenary`] expose l'API (`NoMercenary` au MVP),
//!   l'intégrateur branchera.
//! * **Voyage (Warriv)** : Lut Gholein non jouable au port —
//!   [`World::npc_travel`] expose l'API (`NotAvailable`), NO-OP acte I.
//!
//! # Attaque
//!
//! Les NPC vivent HORS `monsters()` : ni [`PlayerInput::Attack`], ni les
//! missiles, ni l'FSM des monstres ne peuvent les cibler (non-attaquables par
//! construction) — et ils survivent aux changements de niveau via la save
//! (champ additif `"npcs"`, cf. `save.rs`).

use crate::path::{snap_near, Grid};
use crate::quests::{QuestEvent, QuestLog, QuestReward};
use crate::vendor::VendorId;
use crate::{AnimState, Entity, GameEvent, World};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Constantes MVP (documentées en tête de module)
// ---------------------------------------------------------------------------

/// Vitesse de patrouille MVP (cellules/s) — voir doc du module (Velocity 15).
pub(crate) const NPC_WALK_SPEED: f32 = 2.0;
/// Pause à chaque point de patrouille (ms).
pub(crate) const NPC_WAIT_MS: u32 = 2200;
/// Portée d'interaction clic NPC (cellules, centres).
pub(crate) const NPC_TALK_RANGE: f32 = 2.5;
/// Rayon de snap d'un point PATH vers une tuile marchable (la composition du
/// camp peut bloquer le stand exact — les stands DS1 restent au sol, marge
/// faible volontaire).
pub(crate) const NPC_SNAP_RADIUS: i32 = 6;

// ---------------------------------------------------------------------------
// Table : roster réel Acte I (patch_monstats.txt + PATH TownN1.ds1)
// ---------------------------------------------------------------------------

/// Rôle d'un NPC de ville (JSON : `as_str`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NpcRole {
    /// Akara — prêtresse des Sœurs : soin gratuit + vente de potions.
    Healer,
    /// Kashya — capitaine des rogues : résurrection de mercenaires (API,
    /// NO-OP MVP — domaine d'un autre chantier).
    MercenaryCaptain,
    /// Charsi — forgeronne : réparation (API MVP vide) + achat d'armes.
    Blacksmith,
    /// Gheed — commerçant roublard : gamble (déjà branché côté `vendor.rs`).
    Gambler,
    /// Warriv — chef caravane : voyage vers Lut Gholein (API prête, NO-OP
    /// acte I MVP).
    Caravan,
    /// Deckard Cain — dernier Horadrim : identification (à venir), dialogues
    /// de quête (q3).
    Scholar,
}

impl NpcRole {
    /// Libellé JSON/affichage court.
    pub fn as_str(&self) -> &'static str {
        match self {
            NpcRole::Healer => "healer",
            NpcRole::MercenaryCaptain => "mercenary_captain",
            NpcRole::Blacksmith => "blacksmith",
            NpcRole::Gambler => "gambler",
            NpcRole::Caravan => "caravan",
            NpcRole::Scholar => "scholar",
        }
    }

    /// Actions affichables (FR MVP — TBL fr absentes, W18).
    pub fn actions(&self) -> &'static [&'static str] {
        match self {
            NpcRole::Healer => &["Se faire soigner (gratuit)", "Potions (magasin)"],
            NpcRole::MercenaryCaptain => &["Résurrection de recrue (bientôt)"],
            NpcRole::Blacksmith => &["Réparer (rien à réparer au MVP)", "Armes et armures (magasin)"],
            NpcRole::Gambler => &["Gamble (magasin)"],
            NpcRole::Caravan => &["Voyager vers Lut Gholein (Acte II — bientôt)"],
            NpcRole::Scholar => &["Identifier des objets (bientôt)", "Discuter"],
        }
    }
}

/// Un NPC de ville du roster (données statiques — l'état vit dans le monde).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NpcDef {
    /// Id `patch_monstats.txt` (colonne `Id`, ex. `"akara"`).
    pub id: &'static str,
    /// Code monstats 2 lettres (colonne `Code`, ex. `"PS"`).
    pub code: &'static str,
    /// `hcIdx` de la table (référence des string tables, W18).
    pub hc_idx: u32,
    /// `NameStr` de la table (nom canonique EN ; libellé FR = W18).
    pub name: &'static str,
    /// Code des événements de quête (`QuestEvent::NpcTalked`, bas de casse —
    /// ex. `cain1` → `"cain"`, `warriv1` → `"warriv"`, cf. `quests.rs`).
    pub quest_code: &'static str,
    /// Rôle (soin/forge/mercenaires/gamble/caravane/érudit).
    pub role: NpcRole,
    /// Marchand `vendor.rs` associé au rôle (`None` = pas de magasin).
    pub vendor: Option<VendorId>,
    /// Stand du camp (cellules, coin haut-gauche — centre = +0.5 au spawn).
    /// RÉEL (section PATH de TownN1.ds1) sauf `pos_assumed`.
    pub pos: (f32, f32),
    /// `true` = position authoring MVP (aucune donnée PATH — voir doc).
    pub pos_assumed: bool,
    /// Patrouille RÉELLE du DS1 (points absolus, cellules) ; vide = immobile.
    pub patrol: &'static [(f32, f32)],
}

/// Roster Acte I — Camp des Rousses (`Rogue Encampment`, levels.txt Id 1).
/// Positions/patrouilles = section PATH de `TownN1.ds1` (preuves en tête) ;
/// affectation NPC↔PATH assumée (documentée), ordre stable = indices de save.
pub const NPCS: &[NpcDef] = &[
    // PATH objet (126,45) — centre du camp, contre la pierre (144,51).
    NpcDef {
        id: "akara",
        code: "PS",
        hc_idx: 148,
        name: "Akara",
        quest_code: "akara",
        role: NpcRole::Healer,
        vendor: Some(VendorId::Akara),
        pos: (126.0, 45.0),
        pos_assumed: false,
        patrol: &[(134.0, 42.0), (135.0, 45.0), (126.0, 52.0), (133.0, 53.0)],
    },
    // PATH objet (153,46) — centre, à côté d'Akara (comme le retail).
    NpcDef {
        id: "kashya",
        code: "RC",
        hc_idx: 150,
        name: "Kashya",
        quest_code: "kashya",
        role: NpcRole::MercenaryCaptain,
        vendor: None,
        pos: (153.0, 46.0),
        pos_assumed: false,
        patrol: &[(149.0, 48.0), (149.0, 52.0), (153.0, 54.0), (157.0, 54.0), (160.0, 51.0)],
    },
    // PATH objet (94,27) — nord-ouest : la forge de Charsi.
    NpcDef {
        id: "charsi",
        code: "CI",
        hc_idx: 154,
        name: "Charsi",
        quest_code: "charsi",
        role: NpcRole::Blacksmith,
        vendor: Some(VendorId::Charsi),
        pos: (94.0, 27.0),
        pos_assumed: false,
        patrol: &[(95.0, 21.0), (98.0, 30.0), (94.0, 28.0)],
    },
    // PATH objet (95,87) — sud-ouest : la charrette de Gheed.
    NpcDef {
        id: "gheed",
        code: "GH",
        hc_idx: 147,
        name: "Gheed",
        quest_code: "gheed",
        role: NpcRole::Gambler,
        vendor: Some(VendorId::Gheed),
        pos: (95.0, 87.0),
        pos_assumed: false,
        patrol: &[(100.0, 85.0), (102.0, 89.0), (102.0, 93.0), (99.0, 94.0), (93.0, 93.0)],
    },
    // PATH objet (192,16) — nord-est, vers la sortie est (282,107) : la
    // caravane de Warriv.
    NpcDef {
        id: "warriv1",
        code: "WA",
        hc_idx: 155,
        name: "Warriv",
        quest_code: "warriv",
        role: NpcRole::Caravan,
        vendor: None,
        pos: (192.0, 16.0),
        pos_assumed: false,
        patrol: &[
            (187.0, 21.0),
            (191.0, 23.0),
            (195.0, 20.0),
            (196.0, 14.0),
            (192.0, 11.0),
            (193.0, 14.0),
        ],
    },
    // Pas de PATH (il n'apparaît au camp qu'après Tristram) — MVP : immobile
    // à côté d'Akara, pour rendre la chaîne q3 jouable dans le port.
    NpcDef {
        id: "cain1",
        code: "DC",
        hc_idx: 146,
        name: "Deckard Cain",
        quest_code: "cain",
        role: NpcRole::Scholar,
        vendor: None,
        pos: (131.0, 50.0),
        pos_assumed: true,
        patrol: &[],
    },
];

// ---------------------------------------------------------------------------
// État NPC du monde
// ---------------------------------------------------------------------------

/// État d'un NPC spawné (parallèle à [`NPCS`] via `def` ; sauvegardé —
/// les NPC sont PERSISTANTS, jamais tuables).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcState {
    /// Index dans [`NPCS`].
    pub def: usize,
    /// Entité (position, facing, anim NU/WL — pas de sprites MVP, marqueurs UI).
    pub ent: Entity,
    /// Patrouille résolue (tuiles marchables, centres) — dérivée des points
    /// PATH snapés au spawn ; vide = immobile.
    pub patrol: Vec<(f32, f32)>,
    /// Index du point de patrouille visé.
    pub patrol_ix: usize,
    /// Pause restante avant le prochain départ.
    pub wait_ms: u32,
    /// Chemin en cours.
    pub path: Vec<(f32, f32)>,
    /// Horloge d'animation.
    pub anim_ms: u32,
    /// Nombre de dialogues (statistique + save round-trip).
    pub talked: u32,
}

/// Erreur d'interaction NPC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcTalkError {
    /// Héros mort (pas de discussion avec les morts).
    HeroDead,
    /// NPC trop loin (clic : l'UI marche d'abord vers lui).
    OutOfRange,
}

impl NpcTalkError {
    /// Message affichable (d2game/UI).
    pub fn as_str(&self) -> &'static str {
        match self {
            NpcTalkError::HeroDead => "impossible de parler : vous êtes mort",
            NpcTalkError::OutOfRange => "trop loin — approchez-vous du NPC",
        }
    }
}

/// Résultat d'une interaction NPC (UI : bulle de dialogue).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NpcInteraction {
    /// Index du NPC dans [`World::town_npcs`].
    pub index: usize,
    /// Id monstats (`"akara"`…).
    pub id: String,
    /// Nom affichable (`NameStr` de la table).
    pub name: String,
    /// Rôle (`NpcRole::as_str`).
    pub role: String,
    /// Ligne de dialogue MVP (FR provisoire — TBL fr = W18).
    pub dialogue: String,
    /// Actions du rôle (libellés FR MVP).
    pub actions: Vec<String>,
    /// Le rôle soigneur a appliqué un soin CET interaction.
    pub healed: bool,
    /// Récompenses de quête nouvellement acquises (journal branché seulement).
    pub quest_rewards: Vec<QuestReward>,
}

/// Résultat de la résurrection de mercenaire (Kashya) — API pour le chantier
/// mercenaires ; MVP : [`MercenaryResurrect::NoMercenary`] toujours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MercenaryResurrect {
    /// Une recrue est ressuscitée (coût retail : or, joueur futur).
    Success,
    /// Aucun mercenaire dans la partie (MVP : toujours — API prête).
    NoMercenary,
}

/// Résultat d'une demande de voyage NPC (Warriv) — API prête, NO-OP MVP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NpcTravel {
    /// Voyage effectué vers `to` (futur : acte II).
    Ready { to: &'static str },
    /// Voyage refusé, raison affichable (MVP : Lut Gholein non jouable).
    NotAvailable { reason: &'static str },
}

/// Résultat d'une demande de réparation (Charsi) — API MVP (durabilité
/// absente : `NothingToRepair` toujours, documenté).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NpcRepair {
    /// `n` pièces réparées (futur : système de durabilité).
    Repaired { pieces: u32 },
    /// Rien à réparer (aucune pièce équipée / pas de système au MVP).
    NothingToRepair,
}

// ---------------------------------------------------------------------------
// Blocs impl World ADDITIFS (spawn, tick hook, interaction, rôles, quêtes)
// ---------------------------------------------------------------------------

impl World {
    // -- spawn ------------------------------------------------------------

    /// Spawn le roster de ville [`NPCS`] (Camp des Rousses, levels.txt Id 1).
    /// IDEMPOTENT : un roster déjà spawné n'est jamais dupliqué (renvoie le
    /// compte courant). Chaque stand/point PATH est snapé vers une tuile
    /// marchable (composition du camp) ; un point injoignable est RETIRÉ de
    /// la patrouille (jamais de NPC sur un toit). Les NPC spawnés ne vont
    /// JAMAIS dans `monsters()` : non-attaquables par construction.
    pub fn spawn_town_npcs(&mut self) -> usize {
        if !self.npcs.is_empty() {
            return self.npcs.len(); // idempotent — le roster est persistant
        }
        for (i, def) in NPCS.iter().enumerate() {
            let g = Grid { w: self.w, h: self.h, blocked: &self.grid };
            let stand = snap_near(&g, (def.pos.0 as i32, def.pos.1 as i32), NPC_SNAP_RADIUS);
            let Some((tx, ty)) = stand else { continue }; // camp trop étroit : NPC absent, jamais bloqué
            let patrol = def
                .patrol
                .iter()
                .filter_map(|&(px, py)| {
                    snap_near(&g, (px as i32, py as i32), NPC_SNAP_RADIUS)
                        .map(|(sx, sy)| (sx as f32 + 0.5, sy as f32 + 0.5))
                })
                .collect();
            self.npcs.push(NpcState {
                def: i,
                ent: Entity { x: tx as f32 + 0.5, y: ty as f32 + 0.5, facing: 0, anim: AnimState::NU, anim_frame: 0 },
                patrol,
                patrol_ix: 0,
                wait_ms: self.rng.range_u32(800, 2400),
                path: Vec::new(),
                anim_ms: 0,
                talked: 0,
            });
        }
        self.npcs.len()
    }

    /// NPC de ville spawnés (lecture — rendu marqueurs + clic UI).
    pub fn town_npcs(&self) -> &[NpcState] {
        &self.npcs
    }

    // -- tick hook (UNE ligne dans World::tick, comme les waypoints) -------

    /// Patrouilles + anims des NPC (appelé par `World::tick`).
    pub(crate) fn update_npcs(&mut self, dt: u32) {
        for i in 0..self.npcs.len() {
            if !self.npcs[i].path.is_empty() {
                advance_npc(&mut self.npcs[i], dt);
            }
            if self.npcs[i].path.is_empty() && !self.npcs[i].patrol.is_empty() {
                self.npcs[i].wait_ms = self.npcs[i].wait_ms.saturating_sub(dt);
                if self.npcs[i].wait_ms == 0 {
                    let npc = &mut self.npcs[i];
                    npc.patrol_ix = (npc.patrol_ix + 1) % npc.patrol.len();
                    let target = npc.patrol[npc.patrol_ix];
                    let from = (npc.ent.x.floor() as i32, npc.ent.y.floor() as i32);
                    // A* réel (jamais à travers les tentes) ; injoignable =
                    // pause de plus (la patrouille réessaie, aucun blocage).
                    if let Some(p) = self.path_between(from, (target.0 as i32, target.1 as i32)) {
                        if !p.is_empty() {
                            self.npcs[i].path = p;
                        }
                    }
                    self.npcs[i].wait_ms = NPC_WAIT_MS;
                }
            }
            // anim : WL en marche, NU à l'arrêt (16 frames, comme le contrat).
            let st = if self.npcs[i].path.is_empty() { AnimState::NU } else { AnimState::WL };
            if self.npcs[i].ent.anim != st {
                self.npcs[i].ent.anim = st;
                self.npcs[i].anim_ms = 0;
                self.npcs[i].ent.anim_frame = 0;
            }
            let f = (self.npcs[i].anim_ms + dt) / crate::world::FRAME_MS;
            self.npcs[i].anim_ms = self.npcs[i].anim_ms.saturating_add(dt);
            self.npcs[i].ent.anim_frame = (f % crate::world::FRAMES) as u8;
        }
    }

    // -- interaction (clic) -------------------------------------------------

    /// Clic sur le NPC `index` : interaction si le héros est à portée
    /// ([`NPC_TALK_RANGE`]) et vivant. Effets du rôle (soin Akara), émission
    /// [`GameEvent::NpcTalked`] (flux de tick suivant) et AVANCEMENT DU
    /// JOURNAL DE QUÊTES si un log est branché
    /// ([`QuestEvent::NpcTalked`] — débloque q3/q4, contrat impl-quetes).
    pub fn talk_to_npc(&mut self, index: usize) -> Result<NpcInteraction, NpcTalkError> {
        let Some(npc) = self.npcs.get(index) else {
            return Err(NpcTalkError::OutOfRange); // hors roster : même erreur que « trop loin »
        };
        // 3b-1 (coop) : le PARLEUR est le héros vivant le plus proche du NPC
        // (quêtes partagées ; solo : identique, le slot 0 est le seul héros).
        // 3c-b (dialogue.rs) : chemin historique — l'UI coop route elle-même
        // SON slot via `talk_to_npc_slot`.
        let talker = self.nearest_living_hero_slot(npc.ent.x, npc.ent.y);
        self.talk_to_npc_slot(talker, index)
    }

    /// 3c-b (coop) : clic NPC routé au héros du SLOT `slot` — portée/mort
    /// vérifiées pour CE slot, face-à-face, soin (Akara : vie ET mana DU
    /// PARLEUR), `NpcTalked` et journal de quêtes identiques au chemin
    /// historique. Slot hors monde : `OutOfRange` (jamais de panique).
    pub fn talk_to_npc_slot(&mut self, slot: usize, index: usize) -> Result<NpcInteraction, NpcTalkError> {
        let Some(npc) = self.npcs.get(index) else {
            return Err(NpcTalkError::OutOfRange); // hors roster : même erreur que « trop loin »
        };
        if slot >= self.hero_count() {
            return Err(NpcTalkError::OutOfRange); // slot inconnu : pas de parleur
        }
        if self.hero_slot_dead(slot).unwrap_or(true) {
            return Err(NpcTalkError::HeroDead);
        }
        let (hx, hy) = self.hero_slot_pos(slot).expect("parleur vivant a une position");
        let d = ((npc.ent.x - hx).powi(2) + (npc.ent.y - hy).powi(2)).sqrt();
        if d > NPC_TALK_RANGE {
            return Err(NpcTalkError::OutOfRange);
        }
        let def = NPCS[npc.def];
        // Face-à-face (comme le retail : les deux se tournent vers l'autre).
        let (dx, dy) = (hx - npc.ent.x, hy - npc.ent.y);
        self.npcs[index].ent.facing = crate::world::facing_from(dx, dy);
        self.players[slot].ent.facing = crate::world::facing_from(-dx, -dy);
        self.npcs[index].talked += 1;

        // Rôle : soin gratuit (Akara) — vie ET mana pleins DU PARLEUR ;
        // malédictions : aucun système au MVP, rien à purger (assumé, doc du
        // module).
        let mut healed = false;
        if def.role == NpcRole::Healer {
            let stats = &mut self.players[slot].data.stats;
            let (life, max_life) = (stats.life, stats.max_life);
            let (mana, max_mana) = (stats.mana, stats.max_mana);
            if life < max_life || mana < max_mana {
                stats.life = max_life;
                stats.mana = max_mana;
                healed = true;
            }
        }

        // Événement (flux du tick suivant — le monde émet toujours via tick).
        self.pending_events.push(GameEvent::NpcTalked { npc: def.quest_code.to_string() });

        // Journal de quêtes : NpcTalked → états q3/q4 (+ récompenses rendues).
        let mut rewards = Vec::new();
        if let Some(log) = self.quests.as_mut() {
            rewards = log.on_event(&QuestEvent::NpcTalked { npc: def.quest_code.to_string() });
            self.apply_quest_rewards(&rewards);
        }

        Ok(NpcInteraction {
            index,
            id: def.id.to_string(),
            name: def.name.to_string(),
            role: def.role.as_str().to_string(),
            dialogue: dialogue_for(def.role).to_string(),
            actions: def.role.actions().iter().map(|s| s.to_string()).collect(),
            healed,
            quest_rewards: rewards,
        })
    }

    /// Applique les récompenses rendues par le journal (XP → héros, item →
    /// inventaire ou sol au pied du héros ; `Unlock` = état du journal,
    /// consommé par l'UI via [`NpcInteraction::quest_rewards`]).
    fn apply_quest_rewards(&mut self, rewards: &[QuestReward]) {
        for r in rewards {
            match r {
                QuestReward::Xp { amount } => {
                    // Même moteur que les kills (paliers + événements) —
                    // évènements poussés dans le flux du tick suivant.
                    let mut ev = Vec::new();
                    self.apply_xp(*amount, &mut ev);
                    self.pending_events.extend(ev);
                }
                QuestReward::Item { code } => {
                    // 3b-1 (coop) : bénéficiaire = héros vivant le plus
                    // proche du slot 0 (déterministe ; solo : slot 0).
                    let ben = self.nearest_living_hero_slot(
                        self.players[0].ent.x,
                        self.players[0].ent.y,
                    );
                    let def = quest_item_def(code);
                    if let Some(slot) = self.players[ben].data.inv.iter().position(|s| s.is_none()) {
                        self.players[ben].data.inv[slot] = Some(def);
                    } else {
                        // Inventaire plein : au sol au pied du héros (même
                        // convention que le ramassage).
                        let (x, y) = (self.players[ben].ent.x, self.players[ben].ent.y);
                        self.items.push(crate::ItemOnGround { def, x, y });
                    }
                }
                QuestReward::Unlock { .. } => {} // flag porté par le journal
            }
        }
    }

    // -- journal de quêtes (branché par d2game/l'intégrateur — optionnel) ----

    /// Branche le journal de quêtes (W13) : les [`QuestEvent::NpcTalked`] des
    /// clics NPC y avancent q3/q4. `None` par défaut (aucune dépendance
    /// forcée — le câblage complet du journal reste à l'intégration).
    pub fn set_quest_log(&mut self, log: QuestLog) {
        self.quests = Some(log);
    }

    /// Journal branché (lecture).
    pub fn quest_log(&self) -> Option<&QuestLog> {
        self.quests.as_ref()
    }

    /// Journal branché (mutation — pour alimenter les AUTRES événements
    /// quête : MonsterKilled/ItemPickedUp/ZoneEntered, câblage intégration).
    pub fn quest_log_mut(&mut self) -> Option<&mut QuestLog> {
        self.quests.as_mut()
    }

    // -- API de rôle (prêtes pour l'intégrateur) -----------------------------

    /// Kashya — résurrection de mercenaire. MVP : [`MercenaryResurrect::
    /// NoMercenary`] toujours (les mercenaires sont le domaine d'un autre
    /// chantier — l'intégrateur branchera sa logique ici).
    pub fn npc_resurrect_mercenary(&mut self) -> MercenaryResurrect {
        MercenaryResurrect::NoMercenary
    }

    /// Warriv — voyage vers Lut Gholein. MVP : `NotAvailable` (l'acte II
    /// n'est pas jouable) ; l'API est prête (`Ready` au branchement acte II).
    pub fn npc_travel(&mut self) -> NpcTravel {
        NpcTravel::NotAvailable { reason: "Lut Gholein n'est pas encore jouable (acte II à venir)" }
    }

    /// Charsi — réparation de l'équipement. MVP : `NothingToRepair` (pas de
    /// durabilité au port — assumé, doc du module) ; l'API est prête.
    pub fn npc_repair(&mut self) -> NpcRepair {
        NpcRepair::NothingToRepair
    }

    /// Définition du NPC `index` (`None` hors bornes).
    pub fn town_npc_def(&self, index: usize) -> Option<&'static NpcDef> {
        self.npcs.get(index).map(|n| &NPCS[n.def])
    }
}

/// Avance un NPC le long de son chemin (vitesse [`NPC_WALK_SPEED`]).
fn advance_npc(npc: &mut NpcState, dt: u32) {
    let mut step = NPC_WALK_SPEED * dt as f32 / 1000.0;
    while step > 0.0 {
        let Some(wp) = npc.path.first().copied() else { break };
        let (dx, dy) = (wp.0 - npc.ent.x, wp.1 - npc.ent.y);
        let d = (dx * dx + dy * dy).sqrt();
        if d <= step {
            npc.ent.facing = crate::world::facing_from(dx, dy);
            npc.ent.x = wp.0;
            npc.ent.y = wp.1;
            npc.path.remove(0);
            step -= d;
        } else {
            npc.ent.facing = crate::world::facing_from(dx, dy);
            npc.ent.x += dx / d * step;
            npc.ent.y += dy / d * step;
            step = 0.0;
        }
    }
}

/// Dialogue MVP par rôle (FR provisoire — TBL fr absentes, W18).
fn dialogue_for(role: NpcRole) -> &'static str {
    match role {
        NpcRole::Healer => "Restez, je vais vous soigner. Prenez aussi des potions.",
        NpcRole::MercenaryCaptain => "Mes rogues veillent sur le camp. (Recrues : bientôt)",
        NpcRole::Blacksmith => "Mon enclume ne demande qu'à travailler !",
        NpcRole::Gambler => "Un petit jeu de hasard, étranger ? Tout peut arriver…",
        NpcRole::Caravan => "Ma caravane partira bientôt pour Lut Gholein.",
        NpcRole::Scholar => "Stay a while, and listen…",
    }
}

/// `ItemDef` d'une récompense de quête (codes authoring q3/q4 : `rin`,
/// `hamm`) — noms des tables, catégories pures.
fn quest_item_def(code: &str) -> crate::ItemDef {
    match code {
        "rin" => crate::ItemDef { code: "rin".into(), name: "Ring".into(), kind: crate::ItemKind::Misc, amount: 0 },
        "hamm" => crate::ItemDef {
            code: "hamm".into(),
            name: "Horadric Malus".into(),
            kind: crate::ItemKind::Misc,
            amount: 0,
        },
        other => crate::ItemDef {
            code: other.to_string(),
            name: other.to_ascii_uppercase(),
            kind: crate::ItemKind::Misc,
            amount: 0,
        },
    }
}

// ---------------------------------------------------------------------------
// Tests — données réelles ancrées, spawn, clic→quête, soin, save/load.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quests::{QuestState, QuestTrigger};
    use serde_json::json;

    /// Grille 20×20 toute marchable, manifest minimal (0 monstre auto-spawn).
    fn world20() -> World {
        World::with_seed(vec![vec![false; 20]; 20], json!({}), 42)
    }

    /// Monde 300×300 (assez grand pour le roster réel 93..196 en x).
    fn world_town_like() -> World {
        World::with_seed(vec![vec![false; 300]; 300], json!({}), 0xD2D2)
    }

    // --- table réelle -------------------------------------------------------

    #[test]
    fn roster_matches_real_tables() {
        // patch_monstats.txt (id / hcIdx / NameStr / Code / human / AI=Npc).
        let expected = [
            ("akara", 148u32, "Akara", "PS"),
            ("kashya", 150, "Kashya", "RC"),
            ("charsi", 154, "Charsi", "CI"),
            ("gheed", 147, "Gheed", "GH"),
            ("warriv1", 155, "Warriv", "WA"),
            ("cain1", 146, "Deckard Cain", "DC"),
        ];
        assert_eq!(NPCS.len(), expected.len());
        for (def, (id, hc, name, code)) in NPCS.iter().zip(expected) {
            assert_eq!(def.id, id);
            assert_eq!(def.hc_idx, hc, "hcIdx {} (patch_monstats.txt)", id);
            assert_eq!(def.name, name, "NameStr {} (patch_monstats.txt)", id);
            assert_eq!(def.code, code, "Code {} (patch_monstats.txt)", id);
        }
        // Rôles de mission (W16) + marchands vendor.rs.
        assert_eq!(NPCS[0].role, NpcRole::Healer);
        assert_eq!(NPCS[0].vendor, Some(VendorId::Akara));
        assert_eq!(NPCS[1].role, NpcRole::MercenaryCaptain);
        assert_eq!(NPCS[1].vendor, None);
        assert_eq!(NPCS[2].role, NpcRole::Blacksmith);
        assert_eq!(NPCS[2].vendor, Some(VendorId::Charsi));
        assert_eq!(NPCS[3].role, NpcRole::Gambler);
        assert_eq!(NPCS[3].vendor, Some(VendorId::Gheed));
        assert_eq!(NPCS[4].role, NpcRole::Caravan);
        assert_eq!(NPCS[5].role, NpcRole::Scholar);
        // Codes quête = ceux de l'authoring quests.rs (q3 : akara + cain,
        // q4 : charsi).
        let log = QuestLog::act1();
        let wants: Vec<&str> = log
            .defs()
            .iter()
            .flat_map(|d| d.steps.iter())
            .filter_map(|s| match s.trigger {
                QuestTrigger::NpcTalked { ref npc } => Some(npc.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(wants, vec!["akara", "cain", "charsi"]);
        for want in &wants {
            assert!(
                NPCS.iter().any(|d| d.quest_code == *want),
                "NPC manquant pour la quête : {want}"
            );
        }
        // Positions PATH réelles (TownN1.ds1, section PATH @57312) sauf Cain.
        assert_eq!(NPCS[0].pos, (126.0, 45.0));
        assert_eq!(NPCS[1].pos, (153.0, 46.0));
        assert_eq!(NPCS[2].pos, (94.0, 27.0));
        assert_eq!(NPCS[3].pos, (95.0, 87.0));
        assert_eq!(NPCS[4].pos, (192.0, 16.0));
        assert!(NPCS.iter().filter(|d| !d.pos_assumed).all(|d| !d.patrol.is_empty()));
        assert!(NPCS[5].pos_assumed && NPCS[5].patrol.is_empty(), "cain : authoring MVP immobile");
        // Points PATH réels (échantillon exact du DS1).
        assert_eq!(NPCS[0].patrol[0], (134.0, 42.0));
        assert_eq!(NPCS[4].patrol.len(), 6);
        assert_eq!(NPCS[4].patrol[4], (192.0, 11.0));
    }

    // --- spawn --------------------------------------------------------------

    #[test]
    fn spawn_town_npcs_places_all_and_is_idempotent() {
        let mut w = world_town_like();
        let n = w.spawn_town_npcs();
        assert_eq!(n, NPCS.len(), "tout le roster Acte I");
        assert_eq!(w.town_npcs().len(), NPCS.len());
        // Positions marchables, dans les bornes, en centres de tuiles.
        for npc in w.town_npcs() {
            let (tx, ty) = (npc.ent.x.floor() as i32, npc.ent.y.floor() as i32);
            assert!(w.is_walkable(tx, ty), "NPC {} sur tuile bloquée", NPCS[npc.def].id);
            assert!((npc.ent.x - tx as f32 - 0.5).abs() < 1e-4);
        }
        // Idempotent : re-spawn = aucun doublon.
        assert_eq!(w.spawn_town_npcs(), NPCS.len());
        assert_eq!(w.town_npcs().len(), NPCS.len());
        // Non-attaquables : le monde ne porte AUCUN monstre de plus.
        assert!(w.monsters().is_empty(), "NPC hors monsters() — non-attaquables");
    }

    #[test]
    fn spawn_snaps_to_walkable_or_skips_cleanly() {
        // Le stand d'Akara (126,45) est entouré de murs : snap dans un passage.
        let mut grid = vec![vec![false; 300]; 300];
        for (bx, by) in [(125, 44), (126, 44), (127, 44), (125, 45), (127, 45), (125, 46)] {
            grid[by][bx] = true;
        }
        let mut w = World::with_seed(grid, json!({}), 7);
        let n = w.spawn_town_npcs();
        assert_eq!(n, NPCS.len(), "tout le roster spawn (snap), jamais de skip");
        for npc in w.town_npcs() {
            let (tx, ty) = (npc.ent.x.floor() as i32, npc.ent.y.floor() as i32);
            assert!(w.is_walkable(tx, ty));
        }
        // Carte minuscule : aucun stand joignable → roster vide, sans panique.
        let mut w2 = World::with_seed(vec![vec![false; 3]; 3], json!({}), 7);
        assert_eq!(w2.spawn_town_npcs(), 0);
        assert!(w2.town_npcs().is_empty());
    }

    // --- patrouille -----------------------------------------------------------

    #[test]
    fn npcs_patrol_along_real_paths_and_stay_walkable() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        // 30 s de patrouille : tous les NPC bougent (sauf Cain, immobile),
        // restent marchables, NU entre les points / WL en marche.
        let mut saw_walking = [false; 6];
        for _ in 0..600 {
            w.tick(50, None);
            for npc in w.town_npcs() {
                let (tx, ty) = (npc.ent.x.floor() as i32, npc.ent.y.floor() as i32);
                assert!(w.is_walkable(tx, ty), "patrouille sur tuile bloquée");
                if npc.ent.anim == AnimState::WL {
                    saw_walking[npc.def] = true;
                }
            }
        }
        for (i, seen) in saw_walking.iter().enumerate() {
            assert_eq!(*seen, !NPCS[i].patrol.is_empty(), "NPC {} doit patrouiller", NPCS[i].id);
        }
        // Cain n'a jamais bougé de son stand.
        let cain = &w.town_npcs()[5];
        assert_eq!(cain.ent.anim, AnimState::NU);
    }

    // --- clic → NpcTalked → quêtes (mission #3) ------------------------------

    #[test]
    fn click_talks_and_advances_npc_quests() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        // Journal branché : le contrat impl-quetes (source NpcTalked = W16).
        w.set_quest_log(QuestLog::act1());
        let akara_ix = w.town_npcs().iter().position(|n| n.def == 0).unwrap();
        // Le héros marche à côté d'Akara (position du spawn du roster).
        let (a_x, a_y) = { let e = &w.town_npcs()[akara_ix].ent; (e.x, e.y) };
        w.players[0].ent.x = a_x + 1.0;
        w.players[0].ent.y = a_y;
        // Hors ordre : parler à Cain maintenant ne fait rien (q3 étape 0).
        let cain_ix = w.town_npcs().iter().position(|n| n.def == 5).unwrap();
        let cain_pos = (w.town_npcs()[cain_ix].ent.x, w.town_npcs()[cain_ix].ent.y);
        w.players[0].ent.x = cain_pos.0 + 1.0;
        w.players[0].ent.y = cain_pos.1;
        assert!(w.talk_to_npc(cain_ix).is_ok());
        assert_eq!(w.quest_log().unwrap().state("a1q3"), Some(QuestState::NotStarted));
        // Étape 1 : l'écorce d'Inifuss (bks) — via le journal (câblage
        // intégration des autres événements).
        w.quest_log_mut()
            .unwrap()
            .on_event(&crate::quests::QuestEvent::ItemPickedUp { code: "bks".into() });
        assert_eq!(w.quest_log().unwrap().progress("a1q3").unwrap().step, 1);
        // Clic sur Akara : interaction ok + étape 1 franchie (NpcTalked).
        let (a_x, a_y) = { let e = &w.town_npcs()[akara_ix].ent; (e.x, e.y) };
        w.players[0].ent.x = a_x - 1.0;
        w.players[0].ent.y = a_y;
        let talk = w.talk_to_npc(akara_ix).expect("interaction Akara");
        assert_eq!(talk.id, "akara");
        assert_eq!(talk.role, "healer");
        assert_eq!(w.quest_log().unwrap().progress("a1q3").unwrap().step, 2);
        // L'événement part dans le flux du TICK (world émet via tick).
        let ev = w.tick(16, None);
        assert!(
            ev.iter().any(|e| matches!(e, GameEvent::NpcTalked { npc } if npc == "akara")),
            "NpcTalked attendu dans le flux : {ev:?}"
        );
        // Étape 2 → Tristram (câblage intégration) → clic sur Cain = q3 DONE.
        w.quest_log_mut()
            .unwrap()
            .on_event(&crate::quests::QuestEvent::ZoneEntered { level_id: 38 });
        w.players[0].ent.x = cain_pos.0 - 1.0;
        w.players[0].ent.y = cain_pos.1;
        let talk = w.talk_to_npc(cain_ix).expect("interaction Cain");
        assert_eq!(talk.id, "cain1", "la bonne fiche NPC (pas de mélange d'indices)");
        assert_eq!(
            w.quest_log().unwrap().state("a1q3"),
            Some(QuestState::Done),
            "q3 terminée par le clic NPC"
        );
        // Récompenses rendues : rin (inventaire) + cain_free_id (flag).
        assert_eq!(
            talk.quest_rewards,
            vec![
                QuestReward::Item { code: "rin".into() },
                QuestReward::Unlock { flag: "cain_free_id".into() },
            ]
        );
        let inv: Vec<&str> = w.inventory().iter().flatten().map(|d| d.code.as_str()).collect();
        assert!(inv.contains(&"rin"), "récompense rin dans l'inventaire : {inv:?}");
        // q4 : chaîne Smith→Malus→Charsi (les 2 premiers via le journal).
        let log = w.quest_log_mut().unwrap();
        log.on_event(&crate::quests::QuestEvent::BossDead { code: "smith".into() });
        log.on_event(&crate::quests::QuestEvent::ItemPickedUp { code: "hamm".into() });
        assert_eq!(w.quest_log().unwrap().progress("a1q4").unwrap().step, 2);
        // Charsi est loin : interaction hors de portée refusée SANS avancement.
        w.players[0].ent.x = 1.0;
        w.players[0].ent.y = 1.0;
        assert_eq!(w.talk_to_npc(2), Err(NpcTalkError::OutOfRange));
        assert_eq!(w.quest_log().unwrap().state("a1q4"), Some(QuestState::InProgress));
        // Le héros se téléporte à la forge : clic → q4 Done + charsi_imbue.
        let (c_x, c_y) = { let e = &w.town_npcs()[2].ent; (e.x, e.y) };
        w.players[0].ent.x = c_x + 1.5;
        w.players[0].ent.y = c_y;
        let talk = w.talk_to_npc(2).expect("interaction Charsi");
        assert_eq!(w.quest_log().unwrap().state("a1q4"), Some(QuestState::Done));
        assert_eq!(
            talk.quest_rewards,
            vec![QuestReward::Unlock { flag: "charsi_imbue".into() }]
        );
    }

    #[test]
    fn talk_requires_alive_hero_and_range() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        // Trop loin : OutOfRange, ni événement, ni compteur.
        let before = w.town_npcs()[0].talked;
        assert_eq!(w.talk_to_npc(0), Err(NpcTalkError::OutOfRange));
        assert_eq!(w.town_npcs()[0].talked, before);
        assert!(w.tick(16, None).iter().all(|e| !matches!(e, GameEvent::NpcTalked { .. })));
        // Héros mort : HeroDead même à côté.
        let (a_x, a_y) = { let e = &w.town_npcs()[0].ent; (e.x, e.y) };
        w.players[0].ent.x = a_x + 1.0;
        w.players[0].ent.y = a_y;
        w.players[0].data.dead_ms = 1;
        assert_eq!(w.talk_to_npc(0), Err(NpcTalkError::HeroDead));
        // Index hors bornes : jamais de panique.
        assert_eq!(w.talk_to_npc(999), Err(NpcTalkError::OutOfRange));
    }

    // --- rôle soigneur (mission #2 : heal Akara) ------------------------------

    #[test]
    fn akara_heal_restores_life_and_mana() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        // Héros blessé à côté d'Akara : via save/edit/load (pas de setter
        // public — le save porte hero.stats).
        let (a_x, a_y) = { let e = &w.town_npcs()[0].ent; (e.x, e.y) };
        w.players[0].ent.x = a_x + 1.0;
        w.players[0].ent.y = a_y;
        let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
        v["hero"]["stats"]["life"] = json!(5);
        v["hero"]["stats"]["mana"] = json!(0);
        // 3b-1 : l'édition vise les champs de tête (rétro-compat) — sans
        // `players` le champ v4 prioritaire masquerait le hack de test.
        v.as_object_mut().unwrap().remove("players");
        let mut w = World::load(&v.to_string()).expect("reload blessé");
        let (max_life, max_mana) = {
            let s = w.hero_stats();
            (s.max_life, s.max_mana)
        };
        assert_ne!(w.hero_stats().life, max_life);
        // Le NPC Akara reste à côté après le reload ? Le save porte le roster
        // ET le monde recharge — le héros aussi (position sauvegardée).
        let talk = w.talk_to_npc(0).expect("soin Akara");
        assert!(talk.healed, "soin appliqué");
        assert_eq!(w.hero_stats().life, max_life, "vie pleine");
        assert_eq!(w.hero_stats().mana, max_mana, "mana plein");
        // Pleine vie : le soin ne se « re-applique » pas (healed=false, no-op).
        let talk = w.talk_to_npc(0).unwrap();
        assert!(!talk.healed);
    }

    // --- API de rôle (mission #2 : Kashya/Warriv/Charsi/Gheed) ---------------

    #[test]
    fn role_apis_are_exposed_and_documented_noops() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        // Kashya : mercenaires absents au MVP — API prête pour l'intégrateur.
        assert_eq!(w.npc_resurrect_mercenary(), MercenaryResurrect::NoMercenary);
        // Warriv : Lut Gholein non jouable — API prête (Ready à l'acte II).
        assert_eq!(
            w.npc_travel(),
            NpcTravel::NotAvailable { reason: "Lut Gholein n'est pas encore jouable (acte II à venir)" }
        );
        // Charsi : pas de durabilité au MVP — NothingToRepair assumé.
        assert_eq!(w.npc_repair(), NpcRepair::NothingToRepair);
        // Gheed : le NPC expose bien le marchand gamble (branchement vendor).
        assert_eq!(w.town_npc_def(3).unwrap().vendor, Some(VendorId::Gheed));
        // Interaction : actions affichables par rôle (héros téléporté à côté).
        let (wv_x, wv_y) = { let e = &w.town_npcs()[4].ent; (e.x, e.y) };
        w.players[0].ent.x = wv_x + 1.5;
        w.players[0].ent.y = wv_y;
        let talk = w.talk_to_npc(4).unwrap();
        assert!(talk.actions.iter().any(|a| a.contains("Lut Gholein")));
    }

    // --- save/load (mission #5 : NPC persistants) -----------------------------

    #[test]
    fn save_load_round_trip_keeps_persistent_npcs() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        // Dialogue (compteur) + déplacement réel (patrouille) à figer.
        let (a_x, a_y) = { let e = &w.town_npcs()[0].ent; (e.x, e.y) };
        w.players[0].ent.x = a_x + 1.0;
        w.players[0].ent.y = a_y;
        w.talk_to_npc(0).unwrap();
        w.talk_to_npc(0).unwrap();
        for _ in 0..40 {
            w.tick(50, None);
        }
        let before: Vec<(usize, f32, f32, u32)> = w
            .town_npcs()
            .iter()
            .map(|n| (n.def, n.ent.x, n.ent.y, n.talked))
            .collect();
        let json = w.save();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["npcs"].as_array().map(|a| a.len()), Some(6), "champ save additif npcs");
        // Round-trip : positions/compteurs identiques, roster re-spawn idempotent.
        let mut w2 = World::load(&json).expect("reload avec NPC");
        let after: Vec<(usize, f32, f32, u32)> =
            w2.town_npcs().iter().map(|n| (n.def, n.ent.x, n.ent.y, n.talked)).collect();
        assert_eq!(before, after, "NPC persistants (position + talked)");
        assert_eq!(w2.spawn_town_npcs(), 6, "re-spawn après load = idempotent");
        // La patrouille continue après reload (dérivée de la table).
        for _ in 0..40 {
            w2.tick(50, None);
        }
        assert!(w2.town_npcs().iter().any(|n| !n.path.is_empty() || n.ent.anim == AnimState::WL));
        // Et re-save : le champ reste.
        let v2: serde_json::Value = serde_json::from_str(&w2.save()).unwrap();
        assert_eq!(v2["npcs"].as_array().map(|a| a.len()), Some(6));
    }

    #[test]
    fn retrocompat_save_without_npcs_or_quests_fields() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        w.set_quest_log(QuestLog::act1());
        let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
        // Saves antérieures au chantier : ni "npcs" ni "quests" → chargent.
        v.as_object_mut().unwrap().remove("npcs");
        v.as_object_mut().unwrap().remove("quests");
        let mut w2 = World::load(&v.to_string()).expect("save sans champs npc");
        assert!(w2.town_npcs().is_empty(), "roster vide : l'attach d2game re-spawn");
        assert!(w2.quest_log().is_none());
        assert_eq!(w2.spawn_town_npcs(), 6);
        // Le journal rechargé (champ présent) garde son état : q1 partiel.
        let mut w3 = world_town_like();
        w3.spawn_town_npcs();
        w3.set_quest_log(QuestLog::act1());
        w3.quest_log_mut()
            .unwrap()
            .on_event(&crate::quests::QuestEvent::ZoneEntered { level_id: 8 });
        let v3: serde_json::Value = serde_json::from_str(&w3.save()).unwrap();
        assert!(v3["quests"].is_object(), "journal sérialisé dans la save (JSON)");
        let w4 = World::load(&v3.to_string()).unwrap();
        assert_eq!(w4.quest_log().unwrap().state("a1q1"), Some(QuestState::InProgress));
    }

    #[test]
    fn load_rejects_corrupted_npc_defs() {
        let mut w = world_town_like();
        w.spawn_town_npcs();
        let mut v: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
        // def hors table ⇒ Err (jamais d'état incohérent).
        v["npcs"][0]["def"] = json!(999);
        assert!(World::load(&v.to_string()).is_err());
        // NPC hors carte ⇒ Err (même règle que les monstres).
        let mut v2: serde_json::Value = serde_json::from_str(&w.save()).unwrap();
        v2["npcs"][0]["ent"]["x"] = json!(99999.0);
        assert!(World::load(&v2.to_string()).is_err());
    }
}
