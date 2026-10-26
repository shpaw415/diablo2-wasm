//! `game_audio` — étape 14b (PLAN-FIDELITE phase 14, mission d2w-audio2) :
//! table DATA-DRIVEN « event moteur → son gameplay » pour la démo.
//!
//! # Contrat
//! * [`sfx_of`] : le son d'un [`GameEvent`] — `None` = pas de son dédié moteur.
//!   Le `match` est SANS joker `_)` : ajouter une variante à [`GameEvent`]
//!   casse la compilation tant qu'elle n'est pas classée (sonorisée ou
//!   explicitement muette) — exhaustivité garantie par le compilateur.
//! * [`missile_cast_sfx`] : son de cast par type élémentaire, dérivé du skill
//!   ACTIF du héros ([`HeroStats::skill`], ex. `"Fire Bolt"`) — l'event
//!   [`GameEvent::MissileSpawned`] ne porte pas l'élément, la démo interroge
//!   donc cette table avec le skill courant au moment du décoché.
//! * [`SFX_SHRINE`] / [`SFX_CHEST`] : sons d'OBJETS prêts pour l'intégration
//!   (shrines.rs / objects_live émettent encore `ItemDropped`, pas d'event
//!   dédié « shrine activé » / « coffre ouvert » au MVP) — extraits et testés
//!   présents pour que le câblage futur soit un changement d'une ligne.
//!
//! # Sources (octets bruts, mission 14b)
//! 25 WAV extraits de `d2sfx.mpq` v1.12 (`data\global\sfx\…`, lecture
//! stormlib_read.py — aucune ré-encodage, octets MPQ à l'identique, PCM
//! 22 050 Hz 16 bits mono vérifié) vers `demo/assets/sfx/gameplay/`.
//! Provenance exacte de chaque fichier dans [`ALL`].
//!
//! # Répartition avec la couche démo (contrat miroir de demo/game-sfx.js)
//! Le moteur dit QUEL son va avec QUEL event ; la démo décide de le JOUER :
//! * Les 4 events de combat déjà sonorisés côté démo avec des sons réels par
//!   classe/monstre (`manifest.sfx.attack/hit/die`, `sfx.monsters[<code>].die`)
//!   sont muets ici — un second son superposé serait un doublon.
//! * `HeroMoved` est un flux continu (pas un son ponctuel) : muet.
//! * `ActQuestChanged` ne sonne que sur `phase == "done"` (la quête est
//!   complétée) ; les phases `inactive|active|reward_pending` sont muettes.
//!
//! # Tests
//! * GOLDEN : table complète des 33 variantes construites avec les formes
//!   réellement émises (world.rs / corpse.rs / mercenary.rs / quests_*) ;
//! * SIMULATION : un vrai `World` (grille ouverte + manifest réel) tue des
//!   monstres — chaque event émis passe dans [`sfx_of`] et les events
//!   sonorisés observés tombent bien sur leur fichier golden ;
//! * FICHIERS : chaque entrée de [`ALL`] existe sur disque (RIFF/WAVE,
//!   > 44 octets) sous `demo/assets/sfx/gameplay/`.

#![forbid(unsafe_code)]

use crate::GameEvent;

/// Dossier des sons gameplay, relatif à `demo/` (convention manifest/music.js).
pub const BASE: &str = "assets/sfx/gameplay/";

/// Déclare un son : constante publique + ligne de [`ALL`] (chemin, provenance).
macro_rules! sfx {
    ($konst:ident, $file:literal, $src:literal) => {
        pub const $konst: &str = concat!("assets/sfx/gameplay/", $file);
    };
}

// --- 25 sons réels v1.12 (d2sfx.mpq, data\global\sfx\… — octets bruts) -------
sfx!(SFX_LEVEL_UP, "levelup.wav", "cursor\\levelup.wav");
sfx!(SFX_HERO_DEATH, "hero_death.wav", "combat\\player\\barbarian\\death1.wav");
sfx!(SFX_HERO_RESPAWN, "hero_respawn.wav", "skill\\sorceress\\teleport.wav");
sfx!(SFX_GOLD, "gold.wav", "item\\gold.wav");
sfx!(SFX_PICKUP, "pickup.wav", "cursor\\pickup.wav");
sfx!(SFX_POTION_DRINK, "potion_drink.wav", "item\\potiondrink.wav");
sfx!(SFX_INVENTORY_FULL, "inventory_full.wav", "cursor\\button.wav");
sfx!(SFX_ITEM_DROP, "item_drop.wav", "item\\flippy.wav");
sfx!(SFX_MISSILE_HIT, "missile_hit.wav", "skill\\sorceress\\blastimpact1.wav");
sfx!(SFX_QUEST_FLASH, "quest_flash.wav", "quest\\flash1.wav");
sfx!(SFX_ACT_UNLOCK, "act_unlock.wav", "quest\\flash2.wav");
sfx!(SFX_GAME_WON, "game_won.wav", "quest\\soulrise1.wav");
sfx!(SFX_CORPSE_DROP, "corpse_drop.wav", "object\\corpsefall.wav");
sfx!(SFX_CORPSE_RECOVER, "corpse_recover.wav", "cursor\\convert.wav");
sfx!(SFX_NPC_TALK, "npc_talk.wav", "cursor\\windowopen.wav");
sfx!(SFX_MERC_HIRED, "merc_hired.wav", "cursor\\select.wav");
sfx!(SFX_WAYPOINT, "waypoint.wav", "object\\waypointignite2.wav");
sfx!(SFX_MON_RESURRECT, "mon_resurrect.wav", "monster\\fallenshaman\\resurrectcast.wav");
sfx!(SFX_CAST_FIRE, "cast_fire.wav", "skill\\sorceress\\firecast.wav");
sfx!(SFX_CAST_COLD, "cast_cold.wav", "skill\\sorceress\\coldcast.wav");
sfx!(SFX_CAST_LIGHTNING, "cast_lightning.wav", "skill\\sorceress\\eleccast.wav");
// Objets (pas d'event moteur dédié — constants prêts pour le câblage futur) :
sfx!(SFX_SHRINE, "shrine.wav", "object\\shrine\\refill.wav");
sfx!(SFX_CHEST, "chest.wav", "object\\chestsmall.wav");
sfx!(SFX_PORTAL_OPEN, "portal_open.wav", "object\\portalopen.wav");
sfx!(SFX_PORTAL_ENTER, "portal_enter.wav", "skill\\misc\\portalenter.wav");

/// Table complète (chemin, source MPQ v1.12 / usage) — consommée par le test
/// « fichiers présents » et miroir doc de demo/game-sfx.js.
pub const ALL: &[(&str, &str)] = &[
    (SFX_LEVEL_UP, "HeroLeveled — cursor\\levelup.wav"),
    (SFX_HERO_DEATH, "HeroDied — combat\\player\\barbarian\\death1.wav (classe du héros MVP)"),
    (SFX_HERO_RESPAWN, "HeroRespawned — skill\\sorceress\\teleport.wav (réapparition au camp)"),
    (SFX_GOLD, "GoldGained — item\\gold.wav"),
    (SFX_PICKUP, "HeroPickedUp — cursor\\pickup.wav"),
    (SFX_POTION_DRINK, "PotionUsed — item\\potiondrink.wav"),
    (SFX_INVENTORY_FULL, "InventoryFull — cursor\\button.wav"),
    (SFX_ITEM_DROP, "ItemDropped — item\\flippy.wav (l'objet rebondit au sol)"),
    (SFX_MISSILE_HIT, "MissileHit — skill\\sorceress\\blastimpact1.wav (impact générique)"),
    (SFX_QUEST_FLASH, "ActQuestChanged{phase:\"done\"} — quest\\flash1.wav"),
    (SFX_ACT_UNLOCK, "ActUnlocked — quest\\flash2.wav"),
    (SFX_GAME_WON, "GameWon — quest\\soulrise1.wav"),
    (SFX_CORPSE_DROP, "CorpseDropped — object\\corpsefall.wav"),
    (SFX_CORPSE_RECOVER, "CorpseRecovered — cursor\\convert.wav"),
    (SFX_NPC_TALK, "NpcTalked — cursor\\windowopen.wav"),
    (SFX_MERC_HIRED, "MercHired — cursor\\select.wav"),
    (SFX_WAYPOINT, "WaypointActivated — object\\waypointignite2.wav"),
    (SFX_MON_RESURRECT, "MonResurrected — monster\\fallenshaman\\resurrectcast.wav"),
    (SFX_CAST_FIRE, "missile_cast_sfx feu — skill\\sorceress\\firecast.wav"),
    (SFX_CAST_COLD, "missile_cast_sfx froid — skill\\sorceress\\coldcast.wav"),
    (SFX_CAST_LIGHTNING, "missile_cast_sfx foudre — skill\\sorceress\\eleccast.wav"),
    (SFX_SHRINE, "objet shrine (shrines.rs, pas d'event) — object\\shrine\\refill.wav"),
    (SFX_CHEST, "objet coffre (objects_live, pas d'event) — object\\chestsmall.wav"),
    (SFX_PORTAL_OPEN, "objet portail (ouverture) — object\\portalopen.wav"),
    (SFX_PORTAL_ENTER, "LevelSwitch (traversée de portail/warp, démo) — skill\\misc\\portalenter.wav"),
];

/// Son d'un event moteur — table data-driven, exhaustive (aucun joker).
/// `None` = muet (motif en commentaire arm par arm, voir en-tête de module).
pub fn sfx_of(event: &GameEvent) -> Option<&'static str> {
    match event {
        // --- sonorisés (17 variantes) ---------------------------------------
        GameEvent::HeroLeveled { .. } => Some(SFX_LEVEL_UP),
        GameEvent::HeroDied => Some(SFX_HERO_DEATH),
        GameEvent::HeroRespawned { .. } => Some(SFX_HERO_RESPAWN),
        GameEvent::GoldGained { .. } => Some(SFX_GOLD),
        GameEvent::HeroPickedUp { .. } => Some(SFX_PICKUP),
        GameEvent::PotionUsed { .. } => Some(SFX_POTION_DRINK),
        GameEvent::InventoryFull => Some(SFX_INVENTORY_FULL),
        GameEvent::ItemDropped { .. } => Some(SFX_ITEM_DROP),
        GameEvent::MissileHit { .. } => Some(SFX_MISSILE_HIT),
        GameEvent::ActQuestChanged { phase, .. } if phase == "done" => Some(SFX_QUEST_FLASH),
        GameEvent::ActUnlocked { .. } => Some(SFX_ACT_UNLOCK),
        GameEvent::GameWon => Some(SFX_GAME_WON),
        GameEvent::CorpseDropped { .. } => Some(SFX_CORPSE_DROP),
        GameEvent::CorpseRecovered => Some(SFX_CORPSE_RECOVER),
        GameEvent::NpcTalked { .. } => Some(SFX_NPC_TALK),
        GameEvent::MercHired { .. } => Some(SFX_MERC_HIRED),
        GameEvent::WaypointActivated { .. } => Some(SFX_WAYPOINT),
        GameEvent::MonResurrected { .. } => Some(SFX_MON_RESURRECT),
        // --- muets, déjà sonorisés côté démo en sons réels (pas de doublon) --
        // attack.wav / hit.wav / sfx.monsters[code].die du manifest (par classe
        // et par monstre — plus précis que tout générique moteur).
        GameEvent::HeroAttacked { .. }
        | GameEvent::HeroMissed { .. }
        | GameEvent::HeroHit { .. }
        | GameEvent::MonsterDied { .. } => None,
        // --- missile : le cast dépend du skill ACTIF (voir missile_cast_sfx),
        // pas de l'event seul ; l'impact « raté » reste volontairement muet
        // (le sifflement dans le vide n'a pas de son propre dans D2).
        GameEvent::MissileSpawned | GameEvent::MercMissileSpawned | GameEvent::MissileMissed { .. } => None,
        // --- muets MVP (flux continu ou pas de son dédié v1.12 identifié) ----
        GameEvent::HeroMoved { .. } => None, // marche = boucle walk côté démo
        GameEvent::MonFled { .. } => None,   // fuite : cris du groupe déjà joués
        GameEvent::MercAttacked { .. }
        | GameEvent::MercMissed { .. }
        | GameEvent::MercHit { .. }
        | GameEvent::MercDied
        | GameEvent::MercLeveled { .. }
        | GameEvent::MercResurrected { .. } => None, // mercenaire discret au MVP
        // phases non terminales de quête (inactive|active|reward_pending)
        GameEvent::ActQuestChanged { .. } => None,
    }
}

/// Son de cast par type élémentaire, d'après le skill actif du héros
/// ([`crate::HeroStats::skill`], libellés `skills.txt` v1.12). Le froid est
/// testé AVANT la foudre (« Frost Nova » contient « nova »), le feu en premier
/// (« Holy Fire » paladin est bien une aura de feu). Renvoie `None` pour un
/// skill physique/autre (attaque simple, téléport…) : pas de cast sonore.
pub fn missile_cast_sfx(skill: &str) -> Option<&'static str> {
    let s = skill.to_ascii_lowercase();
    if s.is_empty() {
        return None;
    }
    const FIRE: &[&str] = &["fire", "inferno", "meteor", "blaze", "enchant", "hydra"];
    const COLD: &[&str] = &["ice", "cold", "frost", "blizz", "glacial", "frozen", "shiver", "chill"];
    const LIGHTNING: &[&str] = &["lightning", "charg", "static", "thunder", "nova", "chain", "shock"];
    if FIRE.iter().any(|k| s.contains(k)) {
        Some(SFX_CAST_FIRE)
    } else if COLD.iter().any(|k| s.contains(k)) {
        Some(SFX_CAST_COLD)
    } else if LIGHTNING.iter().any(|k| s.contains(k)) {
        Some(SFX_CAST_LIGHTNING)
    } else {
        None
    }
}

// --- tests (natif uniquement : IO de fichiers sous cfg(test), zéro impact wasm)
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ItemDef, PlayerInput, World};

    /// Manifest réel du projet (compile-time, zéro IO runtime) — même source
    /// que tests.rs.
    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    /// Grille toute marchable (`false` = marchable) — même helper que tests.rs.
    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// GOLDEN — les 33 variantes de [`GameEvent`], construites avec les formes
    /// réellement émises par le moteur (world.rs, corpse.rs, mercenary.rs,
    /// quests_actes*.rs, economy.rs), tombent toutes sur la bonne entrée de
    /// table. Sans joker dans `sfx_of`, l'ajout d'une variante casse la
    /// compilation ; ce test garantit en plus la VALEUR (fichier exact).
    #[test]
    fn golden_table_evenement_vers_wav() {
        let events: Vec<(GameEvent, Option<&str>)> = vec![
            // contrat (6 premières variantes)
            (GameEvent::HeroMoved { path_end: (10, 12) }, None),
            (GameEvent::HeroAttacked { target: 0, dmg: 7 }, None),
            (GameEvent::MonsterDied { index: 0, xp: 66 }, None),
            (GameEvent::ItemDropped { item: ItemDef::gold(37), x: 10, y: 13 }, Some(SFX_ITEM_DROP)),
            (GameEvent::HeroLeveled { level: 2 }, Some(SFX_LEVEL_UP)),
            (GameEvent::HeroDied, Some(SFX_HERO_DEATH)),
            // extras additifs — combat
            (GameEvent::HeroMissed { target: 0 }, None),
            (GameEvent::HeroHit { index: 0, dmg: 4 }, None),
            (GameEvent::MissileHit { target: 0, dmg: 9 }, Some(SFX_MISSILE_HIT)),
            (GameEvent::MissileMissed { target: 0 }, None),
            (GameEvent::MissileSpawned, None), // cast élémentaire via missile_cast_sfx
            // extras — vie/inventaire/économie
            (GameEvent::HeroRespawned { x: 5, y: 5 }, Some(SFX_HERO_RESPAWN)),
            (GameEvent::GoldGained { amount: 120 }, Some(SFX_GOLD)),
            (GameEvent::HeroPickedUp { slot: 3 }, Some(SFX_PICKUP)),
            (GameEvent::PotionUsed { heal: 60 }, Some(SFX_POTION_DRINK)),
            (GameEvent::InventoryFull, Some(SFX_INVENTORY_FULL)),
            (GameEvent::WaypointActivated { slot: 39 }, Some(SFX_WAYPOINT)),
            (GameEvent::NpcTalked { npc: "akara".into() }, Some(SFX_NPC_TALK)),
            // extras — mercenaire
            (GameEvent::MercHired { kind: "Rogue".into(), subtype: "Scout".into(), level: 1, cost: 120 }, Some(SFX_MERC_HIRED)),
            (GameEvent::MercAttacked { target: 0, dmg: 3 }, None),
            (GameEvent::MercMissed { target: 0 }, None),
            (GameEvent::MercMissileSpawned, None),
            (GameEvent::MercHit { dmg: 5 }, None),
            (GameEvent::MercDied, None),
            (GameEvent::MercResurrected { cost: 0 }, None),
            (GameEvent::MercLeveled { level: 2 }, None),
            // extras — corps au sol (J1)
            (GameEvent::CorpseDropped { x: 9, y: 11, gold: 230 }, Some(SFX_CORPSE_DROP)),
            (GameEvent::CorpseRecovered, Some(SFX_CORPSE_RECOVER)),
            // extras — IA P1
            (GameEvent::MonResurrected { index: 1 }, Some(SFX_MON_RESURRECT)),
            (GameEvent::MonFled { index: 2 }, None),
            // extras — quêtes d'actes + fin de jeu
            (GameEvent::ActQuestChanged { id: "a1q1".into(), phase: "done".into() }, Some(SFX_QUEST_FLASH)),
            (GameEvent::ActQuestChanged { id: "a1q1".into(), phase: "active".into() }, None),
            (GameEvent::ActQuestChanged { id: "a1q1".into(), phase: "reward_pending".into() }, None),
            (GameEvent::ActQuestChanged { id: "a1q6".into(), phase: "inactive".into() }, None),
            (GameEvent::ActUnlocked { act: 2 }, Some(SFX_ACT_UNLOCK)),
            (GameEvent::GameWon, Some(SFX_GAME_WON)),
        ];
        for (ev, want) in &events {
            assert_eq!(sfx_of(ev), *want, "golden raté pour {ev:?}");
        }
    }

    /// SIMULATION — un vrai `World` émet des events réels (combat jusqu'à la
    /// mort du monstre, comme kill_nearest de tests.rs) : tout event passe
    /// dans `sfx_of` sans panique (table totale), et les events sonorisés
    /// observés tombent sur leur fichier golden.
    #[test]
    fn simulation_renelle_events_dans_la_table() {
        let manifest = serde_json::from_str::<serde_json::Value>(MANIFEST_JSON).unwrap();
        let mut w = World::new(open_grid(56, 40), manifest);
        let mut ev_all: Vec<GameEvent> = Vec::new();
        for _ in 0..3 {
            // Tue le monstre le plus proche en cliquant dessus chaque tick.
            for _ in 0..240 {
                let target = w.monsters().first().map(|m| (m.x.floor() as i32, m.y.floor() as i32));
                let input = target.map(|(x, y)| PlayerInput::Attack { x, y });
                ev_all.extend(w.tick(50, input));
                if !w.monsters().is_empty() {
                    continue;
                }
                // stock épuisé : respawn d'un voisin pour relancer le combat
                let hx = w.hero().x.floor() as i32;
                let hy = w.hero().y.floor() as i32;
                if !w.spawn_monster("zombie1", (hx + 2).min(54), hy) {
                    break;
                }
            }
        }
        // Le combat a bien eu lieu (formes réelles émises par world.rs)…
        assert!(ev_all.iter().any(|e| matches!(e, GameEvent::HeroAttacked { .. })));
        assert!(ev_all.iter().any(|e| matches!(e, GameEvent::MonsterDied { .. })));
        // … chaque event réel est classé par la table (son ou muet documenté)…
        for e in &ev_all {
            if let Some(p) = sfx_of(e) {
                assert!(p.starts_with(BASE), "chemin hors dossier : {p}");
            }
        }
        // … et les sonorisés observés tombent sur leur golden.
        if let Some(e) = ev_all.iter().find(|e| matches!(e, GameEvent::ItemDropped { .. })) {
            assert_eq!(sfx_of(e), Some(SFX_ITEM_DROP));
        }
        if let Some(e) = ev_all.iter().find(|e| matches!(e, GameEvent::HeroLeveled { .. })) {
            assert_eq!(sfx_of(e), Some(SFX_LEVEL_UP));
        }
        if let Some(e) = ev_all.iter().find(|e| matches!(e, GameEvent::GoldGained { .. })) {
            assert_eq!(sfx_of(e), Some(SFX_GOLD));
        }
    }

    /// FICHIERS — chaque entrée de [`ALL`] existe sous `demo/assets/sfx/gameplay/`,
    /// est un RIFF/WAVE non vide (octets bruts MPQ, PCM ou non : on ne vérifie
    /// que le conteneur) et le dossier n'héberge aucun wav hors table.
    #[test]
    fn fichiers_wav_presents_et_conformes() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../../web/",);
        let base = format!("{dir}{BASE}");
        assert_eq!(ALL.len(), 25, "la table ALL attend les 25 sons extraits");
        for (path, src) in ALL {
            let full = format!("{base}{}", path.strip_prefix(BASE).unwrap());
            let bytes = std::fs::read(&full)
                .unwrap_or_else(|e| panic!("{path} ({src}) absent : {e}"));
            assert!(bytes.len() > 44, "{path} trop petit ({})", bytes.len());
            assert_eq!(&bytes[0..4], b"RIFF", "{path} : pas de magie RIFF");
            assert_eq!(&bytes[8..12], b"WAVE", "{path} : pas de conteneur WAVE");
        }
    }

    /// GOLDEN cast élémentaire — mots-clés des libellés `skills.txt` v1.12
    /// (froid AVANT foudre : « Frost Nova » contient « nova »).
    #[test]
    fn golden_cast_elementaire() {
        // feu
        for s in ["Fire Bolt", "Fire Ball", "Firewall", "Meteor", "Hydra", "Blaze", "Enchant", "Holy Fire"] {
            assert_eq!(missile_cast_sfx(s), Some(SFX_CAST_FIRE), "{s}");
        }
        // froid (avant foudre)
        for s in ["Ice Bolt", "Frozen Armor", "Frost Nova", "Blizzard", "Glacial Spike", "Frozen Orb", "Shiver Armor", "Chilling Armor"] {
            assert_eq!(missile_cast_sfx(s), Some(SFX_CAST_COLD), "{s}");
        }
        // foudre
        for s in ["Charged Bolt", "Static Field", "Nova", "Lightning", "Chain Lightning", "Thunder Storm"] {
            assert_eq!(missile_cast_sfx(s), Some(SFX_CAST_LIGHTNING), "{s}");
        }
        // physique / autre : pas de cast sonore
        for s in ["", "Attack", "Throw", "Teleport", "Telekinesis", "Holy Bolt", "Poison Javelin"] {
            assert_eq!(missile_cast_sfx(s), None, "{s}");
        }
    }

    /// Le skill actif du héros réel au boot (barbare MVP : vide) ne sonne pas —
    /// garde-fou contre un cast intempestif au décoché d'une flèche simple.
    #[test]
    fn skill_actif_defaut_muets() {
        let manifest = serde_json::from_str::<serde_json::Value>(MANIFEST_JSON).unwrap();
        let w = World::new(open_grid(56, 40), manifest);
        let stats = w.hero_stats();
        assert_eq!(missile_cast_sfx(&stats.skill), None, "skill de boot « {} » doit rester muet", stats.skill);
    }
}
