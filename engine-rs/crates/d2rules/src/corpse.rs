//! Corps au sol (J1 — mission CORPS AU SOL, 2026-10-23).
//!
//! À la mort du héros, son corps reste au sol à l'endroit de la mort avec
//! l'équipement porté ; le héros réapparaît au camp SANS rien, et doit
//! revenir cliquer sur son corps pour tout récupérer. Un seul corps actif.
//!
//! # Sources (règles officielles D2 v1.10+ / v1.12)
//! * La page Arreat Summit « Death » historique n'est plus en ligne
//!   (`classic.battle.net/diablo2exp/basics/death.shtml` → 404, aucun
//!   snapshot Wayback) ; source de substitution rigoureuse : Basin Wiki
//!   « Death (Basic) » <https://www.theamazonbasin.com/wiki/index.php/Death_(Basic)>
//!   (règles v1.10+, vérifiées D2R).
//! * Pénalité XP : `patch_difficultylevels.txt` col `DeathExpPenalty` réelle
//!   v1.12 = **0 / 5 / 10 %** (Normal / Nightmare / Hell) — déjà branchée
//!   (chantier K3, `world.rs::monster_strike`, plafonnée au plancher du
//!   niveau courant : un niveau gagné ne se perd jamais, Basin « Experience
//!   lost »).
//! * Or porté (Basin « Death takes its toll ») : le personnage perd un
//!   PÉAGE de clvl % si clvl ≤ 20, sinon 20 % de son or porté ; en Solo,
//!   **500 × clvl d'or porté sont exonérés** (le stash n'existe pas dans ce
//!   moteur) ; **le reliquat tombe au sol à l'endroit de la mort** et
//!   reste récupérable (« Any remaining gold in inventory is dropped by
//!   corpse, awaiting retrieval »).
//! * Respawn : le héros réapparaît en ville pleine vie/mana/endurance mais
//!   l'équipement reste sur le corps au lieu de la mort (Basin « Otherwise,
//!   pressing the Esc key resurrects the character in town … However, all
//!   equipped items remain on a corpse left at place of death »). Ici :
//!   retour au point d'apparition du camp (`hero.spawn`), torse nu.
//! * Récupération : cliquer sur le corps ré-équipe (les objets retournent
//!   dans l'inventaire) ; un corps ne disparaît que lorsque tout son
//!   contenu a été replacé (Basin « Character corpses do not disappear
//!   until every item … ») ; si l'inventaire est plein, le surplus RESTE
//!   sur le corps.
//! * Corps multiples : le retail autorise jusqu'à 16 corps, et après
//!   sauvegarde/rejoin seul le corps « le plus riche » survit à la ville,
//!   les autres se vident au sol (Basin « Equipped items » §3). La mission
//!   J1 impose la SIMPLIFICATION **un seul corps actif** : à la nouvelle
//!   mort, l'ancien corps se vide au sol en items à sa position (cf.
//!   `drop_corpse`).
//!
//! # Choix de représentation (le « plus simple ROBUSTE » demandé)
//! * Le World n'a pas de slots d'équipement séparés (le « porté » de ce
//!   moteur = l'inventaire 10×4 `hero.inv` ; le type `Equipment` à 8 slots
//!   de `inventory.rs` n'est pas branché sur le World) : le corps porte la
//!   **snapshot de l'inventaire** au moment de la mort.
//! * État DÉDIÉ `Option<Corpse>` (et non items au sol spéciaux) : un clic =
//!   récupération atomique, sérialisable telle quelle dans la save, aucun
//!   risque de collision avec le loot normal ni de re-spool accidentel.

use crate::world::{dist, INV_SLOTS, PICKUP_RANGE, World};
use crate::{GameEvent, ItemDef, ItemOnGround};
use serde::{Deserialize, Serialize};

/// Plafond du péage d'or à la mort (Basin « Death takes its toll » :
/// « this equals character level for clvl 20 or less, and 20% thereafter »).
pub(crate) const GOLD_TOLL_CAP_PCT: u32 = 20;
/// Exonération Solo : 500 × clvl d'or porté intouchables (Basin, même
/// section : « 500 * clvl gold in inventory is also exempt »).
pub(crate) const GOLD_SP_EXEMPT_PER_LEVEL: u32 = 500;

/// Corps du héros resté au sol à l'endroit de la mort, portant la snapshot
/// de son inventaire (« l'équipement porté » de ce moteur, cf. choix
/// ci-dessus). Sérialisé tel quel dans la save (champ additif).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Corpse {
    /// Position x du corps (espace tuile continu, centre de tuile = tx+0.5).
    pub x: f32,
    /// Position y du corps.
    pub y: f32,
    /// Inventaire du héros au moment de la mort (40 slots, ordre conservé).
    pub inv: Vec<Option<ItemDef>>,
}

impl World {
    /// Corps actif au sol (lecture, pour d2game/UI), s'il y en a un.
    pub fn corpse(&self) -> Option<&Corpse> {
        self.corpse.as_ref()
    }

    /// À la mort : pose le corps + applique les règles d'or. Appelé par
    /// `world::monster_strike` au passage de la vie à 0 (avant
    /// `HeroDied`), et directement par les tests.
    ///
    /// Effets : (1) un éventuel ancien corps se vide au sol (mission J1,
    /// un seul corps actif) ; (2) l'or porté subit le péage officiel et le
    /// reliquat tombe en pile au sol à l'endroit de la mort ; (3)
    /// l'inventaire entier migre dans le corps ; (4) événement
    /// [`GameEvent::CorpseDropped`] (tuile + or au sol).
    pub(crate) fn drop_corpse(&mut self, ev: &mut Vec<GameEvent>) {
        // Plus de course auto en cours vers un ancien corps (transitoire).
        self.corpse_pending = false;
        let (x, y) = (self.hero_ent.x, self.hero_ent.y);

        // (1) Ancien corps non récupéré : il se vide au sol à SA position
        // (simplification mission J1 ; retail Basin « Equipped items » §3 :
        // les corps perdus disparaissent en lâchant leurs items).
        if let Some(old) = self.corpse.take() {
            for def in old.inv.into_iter().flatten() {
                ev.push(GameEvent::ItemDropped {
                    item: def.clone(),
                    x: old.x.floor() as i32,
                    y: old.y.floor() as i32,
                });
                self.items.push(ItemOnGround { def, x: old.x, y: old.y });
            }
        }

        // (2) Or porté : péage officiel min(clvl, 20) % au-delà de
        // l'exonération Solo 500×clvl ; le reliquat tombe au sol, à
        // récupérer comme n'importe quelle pile d'or (Pickup).
        let carried = self.hero.stats.gold;
        let mut dropped = carried;
        if carried > 0 {
            let toll_pct = self.hero.stats.level.min(GOLD_TOLL_CAP_PCT);
            let exempt = GOLD_SP_EXEMPT_PER_LEVEL * self.hero.stats.level;
            let tollable = carried.saturating_sub(exempt);
            let lost = tollable * toll_pct / 100;
            dropped = carried - lost;
            if dropped > 0 {
                let def = ItemDef::gold(dropped);
                ev.push(GameEvent::ItemDropped {
                    item: def.clone(),
                    x: x.floor() as i32,
                    y: y.floor() as i32,
                });
                self.items.push(ItemOnGround { def, x, y });
            }
        }
        self.hero.stats.gold = 0;

        // (3) Le corps emporte TOUT l'inventaire (le respawn se fera nu) ;
        // le héros repart avec une grille vide toute neuve.
        let inv = std::mem::replace(&mut self.hero.inv, vec![None; INV_SLOTS]);
        self.corpse = Some(Corpse { x, y, inv });

        ev.push(GameEvent::CorpseDropped {
            x: x.floor() as i32,
            y: y.floor() as i32,
            gold: dropped,
        });
    }

    /// Récupère le corps si le héros est à portée ([`PICKUP_RANGE`], la
    /// portée de ramassage standard) : les objets retournent dans
    /// l'inventaire (slots libres, ordre du corps conservé) ; si
    /// l'inventaire est plein, le surplus RESTE sur le corps (Basin :
    /// « remain on the corpse if there isn't sufficient space »). Émet
    /// [`GameEvent::CorpseRecovered`] si au moins un objet est repris.
    /// Renvoie true si au moins un objet a été récupéré.
    pub fn recover_corpse(&mut self, ev: &mut Vec<GameEvent>) -> bool {
        if self.hero.dead_ms > 0 {
            return false; // mort : pas de récup (tick ne livre l'input de toute façon pas)
        }
        let Some(c) = self.corpse.clone() else { return false };
        if dist(c.x - self.hero_ent.x, c.y - self.hero_ent.y) > PICKUP_RANGE {
            return false;
        }
        let mut taken = 0usize;
        let mut leftovers: Vec<Option<ItemDef>> = Vec::new();
        for def in c.inv.into_iter().flatten() {
            if let Some(slot) = self.hero.inv.iter().position(|s| s.is_none()) {
                self.hero.inv[slot] = Some(def);
                taken += 1;
            } else {
                leftovers.push(Some(def)); // inventaire plein : reste au corps
            }
        }
        if taken == 0 {
            return false;
        }
        self.corpse = if leftovers.is_empty() {
            None // tout est repris : le corps disparaît (Basin)
        } else {
            Some(Corpse { x: c.x, y: c.y, inv: leftovers })
        };
        self.corpse_pending = false;
        ev.push(GameEvent::CorpseRecovered);
        true
    }

    /// Position du corps actif (lecture rapide pour l'UI).
    pub fn corpse_position(&self) -> Option<(f32, f32)> {
        self.corpse.as_ref().map(|c| (c.x, c.y))
    }
}

/// Bras de [`crate::PlayerInput::RecoverCorpse`] : clic sur le corps. À
/// portée → récupération immédiate ; sinon le héros MARCHE vers le corps et
/// la récupération s'exécute automatiquement à l'arrivée (comportement D2
/// « grab your body » : un clic suffit). `corpse_pending` est transient
/// (jamais sauvegardé : après un load, il suffit de recliquer).
pub(crate) fn handle_recover_corpse(w: &mut World, ev: &mut Vec<GameEvent>) {
    let Some(c) = w.corpse().cloned() else { return };
    if dist(c.x - w.hero_ent.x, c.y - w.hero_ent.y) <= PICKUP_RANGE {
        w.recover_corpse(ev);
        return;
    }
    // Hors de portée : chemin A* vers la tuile du corps (comme l'attaque
    // hors de portée), récup auto à l'arrivée (update_hero).
    let ct = (c.x.floor() as i32, c.y.floor() as i32);
    if let Some(path) = w.find_path_from_hero(ct) {
        w.corpse_pending = true;
        if !path.is_empty() {
            w.hero.path = path;
        }
    }
}

/// Fin du module : les tests couvrent la mission J1 (§6 du brief).
#[cfg(test)]
mod tests {
    use super::*;
    use crate::difficulty::Difficulty;
    use crate::AnimState;
    use crate::PlayerInput;

    /// Manifest réel du projet (compile-time, zéro IO runtime).
    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn manifest_no_monsters() -> serde_json::Value {
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        m
    }

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    /// Monde 24×24 sans monstres (héros au centre, spawn automatique).
    fn mk_world() -> World {
        World::with_seed(open_grid(24, 24), manifest_no_monsters(), 0xD2C0_0001)
    }

    /// Avance `ms` de simulation par pas de 50 ms, sans input.
    fn run(w: &mut World, ms: u32) -> Vec<GameEvent> {
        let mut ev = Vec::new();
        let mut left = ms;
        while left > 0 {
            let dt = left.min(50);
            ev.extend(w.tick(dt, None));
            left -= dt;
        }
        ev
    }

    fn ev_kind(ev: &[GameEvent], pred: impl Fn(&GameEvent) -> bool) -> bool {
        ev.iter().any(pred)
    }

    /// (1) Mort → corps posé à l'endroit de la mort AVEC l'inventaire ;
    /// le héros repart grille vide.
    #[test]
    fn mort_pose_le_corps_avec_linventaire() {
        let mut w = mk_world();
        let (dx, dy) = (w.hero_ent.x, w.hero_ent.y);
        w.hero.inv[3] = Some(ItemDef::potion());
        w.hero.inv[12] = Some(ItemDef { code: "hax".into(), name: "Hache".into(), ..Default::default() });
        w.hero.stats.gold = 0;

        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);

        let c = w.corpse().expect("le corps doit être posé");
        assert_eq!(c.x, dx, "corps à l'endroit exact de la mort (x)");
        assert_eq!(c.y, dy, "corps à l'endroit exact de la mort (y)");
        assert_eq!(c.inv[3].as_ref().unwrap().code, "hp1");
        assert_eq!(c.inv[12].as_ref().unwrap().code, "hax");
        assert_eq!(c.inv.iter().filter(|s| s.is_some()).count(), 2);
        assert!(w.hero.inv.iter().all(|s| s.is_none()), "le héros repart nu");
        assert!(
            ev_kind(&ev, |e| matches!(e, GameEvent::CorpseDropped { x, y, .. }
                if *x == dx.floor() as i32 && *y == dy.floor() as i32)),
            "CorpseDropped émis à la tuile de la mort : {ev:?}"
        );
    }

    /// (2) Or porté : péage officiel + pile d'or au sol, bourse vidée.
    /// Cas : clvl 10, 25 000 or → exonérés 5 000 → péage 10 % de 20 000
    /// = 2 000 perdus → 23 000 au sol. Puis clvl 30 (plafond 20 %) et
    /// clvl 3 sous l'exonération (tout tombe).
    #[test]
    fn or_porte_peage_officiel_et_pile_au_sol() {
        let mut w = mk_world();
        w.hero.stats.level = 10;
        w.hero.stats.gold = 25_000;
        let (dx, dy) = (w.hero_ent.x, w.hero_ent.y);
        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);

        assert_eq!(w.hero.stats.gold, 0, "la bourse est vidée");
        let pile = w
            .items
            .iter()
            .find(|it| it.def.kind == crate::ItemKind::Gold)
            .expect("une pile d'or doit être au sol");
        assert_eq!(pile.def.amount, 23_000, "25 000 − 2 000 (péage 10 % clvl 10)");
        assert_eq!(pile.x, dx, "pile à l'endroit de la mort");
        assert_eq!(
            ev_kind(&ev, |e| matches!(e, GameEvent::CorpseDropped { gold: 23_000, .. })),
            true,
            "CorpseDropped porte l'or tombé au sol : {ev:?}"
        );

        // clvl 30 : péage plafonné à 20 % (exonérés 15 000).
        let mut w2 = mk_world();
        w2.hero.stats.level = 30;
        w2.hero.stats.gold = 25_000;
        let mut ev2 = Vec::new();
        w2.drop_corpse(&mut ev2);
        let pile2 = w2.items.iter().find(|it| it.def.kind == crate::ItemKind::Gold).unwrap();
        assert_eq!(pile2.def.amount, 25_000 - (25_000 - 15_000) * 20 / 100);

        // clvl 3, 1 000 or : exonération 1 500 ⇒ tout tombe, rien de perdu.
        let mut w3 = mk_world();
        w3.hero.stats.level = 3;
        w3.hero.stats.gold = 1_000;
        let mut ev3 = Vec::new();
        w3.drop_corpse(&mut ev3);
        let pile3 = w3.items.iter().find(|it| it.def.kind == crate::ItemKind::Gold).unwrap();
        assert_eq!(pile3.def.amount, 1_000);
    }

    /// (3) Respawn au camp : position = spawn, pleine vie, GRILLE VIDE
    /// (torse nu — l'équipement est resté sur le corps). La pénalité XP
    /// (DeathExpPenalty NM 5 %) vit sur le chemin RÉEL de la mort — elle
    /// est ancrée dans `mort_par_monstre_pose_le_corps_via_le_tik_reel`.
    #[test]
    fn respawn_au_camp_nu_plein_vie() {
        let mut w = mk_world();
        w.hero.inv[0] = Some(ItemDef::potion());
        w.hero.stats.gold = 500;
        let (sx, sy) = (w.hero.spawn.0.floor() as i32, w.hero.spawn.1.floor() as i32);
        // Éloigne le héros du spawn pour vérifier le retour au camp.
        w.hero_ent.x = 3.5;
        w.hero_ent.y = 3.5;

        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);
        assert_eq!(w.hero.stats.gold, 0, "l'or porté est tombé au sol");
        assert!(w.items.iter().any(|it| it.def.kind == crate::ItemKind::Gold));
        // Appel direct : émule le compte à rebours posé par monster_strike
        // (dead_ms = 1) pour déclencher le respawn dans update_hero.
        w.hero.dead_ms = 1;

        let ev2 = run(&mut w, crate::world::RESPAWN_MS + 100);
        assert!(ev2.iter().any(|e| matches!(e, GameEvent::HeroRespawned { .. })));
        assert_eq!(w.hero_ent.x.floor() as i32, sx, "respawn au camp (x)");
        assert_eq!(w.hero_ent.y.floor() as i32, sy, "respawn au camp (y)");
        assert_eq!(w.hero.stats.life, w.hero.stats.max_life, "pleine vie");
        assert!(w.hero.inv.iter().all(|s| s.is_none()), "respawn nu");
        assert!(w.corpse().is_some(), "le corps attend toujours au sol");
    }

    /// (4) Récupération : héros à côté du corps + clic (`RecoverCorpse`)
    /// → tout ré-équipé aux mêmes slots, corps disparu, événement émis.
    #[test]
    fn recuperation_corps_re_equipe_tout() {
        let mut w = mk_world();
        w.hero.inv[0] = Some(ItemDef::potion());
        w.hero.inv[39] = Some(ItemDef { code: "hax".into(), name: "Hache".into(), ..Default::default() });
        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);

        // Téléporte le héros à côté du corps (post-respawn retour au corps).
        let c = w.corpse().cloned().unwrap();
        w.hero_ent.x = c.x + 1.0;
        w.hero_ent.y = c.y;
        let ev2 = w.tick(16, Some(PlayerInput::RecoverCorpse));

        assert_eq!(w.hero.inv[0].as_ref().unwrap().code, "hp1", "slot 0 ré-équipé");
        assert!(
            w.hero.inv.iter().any(|s| s.as_ref().map(|d| d.code.as_str()) == Some("hax")),
            "hache ré-équipée quelque part (la récup remplit les slots libres \
             dans l'ordre, comme D2 ré-équipe dans l'ordre du corps) : {ev2:?}"
        );
        assert!(w.corpse().is_none(), "corps vide = disparu");
        assert!(ev2.iter().any(|e| matches!(e, GameEvent::CorpseRecovered)));
        assert!(w.hero.stats.gold == 0 && w.items.iter().all(|i| i.def.kind != crate::ItemKind::Gold));
    }

    /// (5) Récupération à distance : un clic loin du corps fait MARCHER le
    /// héros vers lui, récup automatique à l'arrivée (un seul clic).
    #[test]
    fn clic_loin_marche_vers_le_corps_et_recupere_a_larrivee() {
        let mut w = mk_world();
        w.hero.inv[5] = Some(ItemDef::potion());
        // Meurt LOIN du spawn : le respawn au camp sera hors de portée.
        w.hero_ent.x = 3.5;
        w.hero_ent.y = 3.5;
        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);
        // Appel direct : émule le compte à rebours de monster_strike.
        w.hero.dead_ms = 1;
        // Respawn camp (éloigné du corps).
        run(&mut w, crate::world::RESPAWN_MS + 100);
        let c = w.corpse().cloned().unwrap();
        assert!(
            (c.x - w.hero_ent.x).abs() + (c.y - w.hero_ent.y).abs() > PICKUP_RANGE,
            "héros loin du corps après respawn : corps=({},{}) héros=({},{})",
            c.x,
            c.y,
            w.hero_ent.x,
            w.hero_ent.y
        );

        let ev2 = w.tick(16, Some(PlayerInput::RecoverCorpse));
        assert!(w.corpse_pending, "intention posée : le héros marche vers le corps");
        assert!(!w.hero.path.is_empty(), "chemin A* vers le corps");

        // Avance jusqu'à l'arrivée (budget 8 s pour une grande carte).
        let mut got = false;
        for _ in 0..160 {
            let e = w.tick(50, None);
            got |= e.iter().any(|e| matches!(e, GameEvent::CorpseRecovered));
            if w.corpse().is_none() {
                break;
            }
        }
        assert!(got, "CorpseRecovered émis à l'arrivée : {ev2:?}");
        assert!(
            w.hero.inv.iter().any(|s| s.as_ref().map(|d| d.code.as_str()) == Some("hp1")),
            "potion ré-équipée (premier slot libre)"
        );
        assert!(!w.corpse_pending, "intention consommée");
    }

    /// (6) Seconde mort avant récupération : l'ANCIEN corps se vide au sol
    /// à sa position (items) et le nouveau corps remplace (un seul corps
    /// actif — mission J1 ; retail Basin « Equipped items » §3 cité).
    #[test]
    fn seconde_mort_ancien_corps_se_vide_au_sol() {
        let mut w = mk_world();
        // Mort 1 : deux objets au corps A.
        w.hero.inv[1] = Some(ItemDef::potion());
        w.hero.inv[2] = Some(ItemDef { code: "hax".into(), name: "Hache".into(), ..Default::default() });
        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);
        let corpse_a = w.corpse().cloned().unwrap();

        // Respawn nu, puis le héros ramasse une potion et remporte.
        run(&mut w, crate::world::RESPAWN_MS + 100);
        w.hero.inv[0] = Some(ItemDef::potion());
        w.hero_ent.x = corpse_a.x + 10.5; // meurt AILLEURS
        w.hero_ent.y = corpse_a.y;
        let mut ev2 = Vec::new();
        w.drop_corpse(&mut ev2);

        // L'ancien corps A s'est vidé : 2 items au sol à SA position.
        let at_a = w
            .items
            .iter()
            .filter(|it| it.x == corpse_a.x && it.y == corpse_a.y)
            .count();
        assert_eq!(at_a, 2, "les 2 objets du corps A sont au sol à sa position");
        assert!(
            ev2.iter().any(|e| matches!(e, GameEvent::ItemDropped { .. })),
            "ItemDropped émis pour chaque objet de l'ancien corps"
        );
        // Nouveau corps unique, à la nouvelle position, avec la potion.
        let b = w.corpse().expect("un seul corps actif");
        assert_eq!(b.x, corpse_a.x + 10.5);
        assert_eq!(b.inv.iter().filter(|s| s.is_some()).count(), 1);
        assert_eq!(b.inv[0].as_ref().unwrap().code, "hp1");
    }

    /// (7) Save/load : le corps (position + inventaire) traverse la
    /// sauvegarde ; une save v3 SANS champ corpse (ancienne) charge à None.
    #[test]
    fn save_load_corps_et_retro_compat() {
        let mut w = mk_world();
        w.hero.inv[7] = Some(ItemDef { code: "hax".into(), name: "Hache".into(), ..Default::default() });
        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);

        let json = crate::save::save_json(&w);
        let w2 = crate::save::load_json(&json).expect("save avec corps se recharge");
        let c1 = w.corpse().cloned().unwrap();
        let c2 = w2.corpse().cloned().expect("corps restauré");
        assert_eq!(c1, c2, "position + inventaire identiques");

        // Rétro-compat : la même save sans "corpse" (champ plus ancien).
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        let obj = v.as_object_mut().unwrap();
        obj.remove("corpse");
        let old = serde_json::to_string(&v).unwrap();
        let w3 = crate::save::load_json(&old).expect("save sans corpse se charge");
        assert!(w3.corpse().is_none());
    }

    /// (8) Inventaire presque plein : la récup remplit les slots libres et
    /// laisse le SURPLUS sur le corps (le corps ne disparaît que vide).
    #[test]
    fn inventaire_pleins_recup_partielle_surplus_au_corps() {
        let mut w = mk_world();
        // Deux objets au corps, mais UN SEUL slot libre à la récup.
        w.hero.inv[0] = Some(ItemDef::potion());
        w.hero.inv[1] = Some(ItemDef { code: "hax".into(), name: "Hache".into(), ..Default::default() });
        let mut ev = Vec::new();
        w.drop_corpse(&mut ev);
        let c = w.corpse().cloned().unwrap();

        // Héros à côté, inventaire rempli sauf UN slot (le 0).
        w.hero_ent.x = c.x + 0.5;
        w.hero_ent.y = c.y;
        for i in 1..INV_SLOTS {
            w.hero.inv[i] = Some(ItemDef::potion());
        }
        assert!(w.recover_corpse(&mut ev), "au moins un objet repris");
        assert_eq!(
            w.hero.inv.iter().filter(|s| s.is_some()).count(),
            INV_SLOTS,
            "grille pleine après récup"
        );
        let rest = w.corpse().expect("le surplus reste sur le corps");
        assert_eq!(rest.inv.iter().filter(|s| s.is_some()).count(), 1);
        assert!(rest.inv.iter().any(|s| s.as_ref().map(|d| d.code.as_str()) == Some("hax")));
        assert_eq!(rest.x, c.x, "le corps reste à sa position");
    }

    /// (9) Chemin réel : le héros meurt d'un coup de monstre (FSM
    /// complet) → CorpseDropped émis par `monster_strike` et corps posé.
    #[test]
    fn mort_par_monstre_pose_le_corps_via_le_tik_reel() {
        let mut m = manifest_no_monsters();
        m["monsters"] = serde_json::json!([
            { "code": "Z1", "name": "Zombie", "id": "zombie1",
              "stats": { "level": 1, "hp": 61, "hpMin": 21, "dmgMin": 51,
                         "dmgMax": 101, "exp": 61, "speed": 0 } }
        ]);
        let mut w = World::with_seed(open_grid(24, 24), m, 0xD2C0_0002);
        assert!(!w.mon_ent.is_empty(), "monstre auto-spawné");
        w.hero.stats.life = 1; // un seul coup suffit
        w.hero.inv[0] = Some(ItemDef::potion());
        // Pénalité XP officielle sur le chemin RÉEL : Nightmare = 5 %
        // (`patch_difficultylevels.txt` col DeathExpPenalty — 0/5/10).
        w.set_difficulty(Difficulty::Nightmare);
        w.hero.stats.level = 2;
        w.hero.stats.xp = 1_600; // > XP_TABLE[0]=500 : plancher non contraignant

        // Colle le monstre au héros : FSM Idle→Chase→Attack→frappe.
        w.mon_ent[0].x = w.hero_ent.x + 1.0;
        w.mon_ent[0].y = w.hero_ent.y;
        let mut died = false;
        for _ in 0..120 {
            let e = w.tick(50, None);
            if e.iter().any(|ev| matches!(ev, GameEvent::CorpseDropped { .. })) {
                died = true;
                break;
            }
        }
        assert!(died, "le coup de monstre pose le corps (event CorpseDropped)");
        assert!(w.hero.dead_ms > 0 || w.hero_ent.anim == AnimState::DD);
        // Pénalité XP NM sur le chemin réel : 5 % perdus, niveau gardé.
        assert_eq!(w.hero.stats.xp, 1_600 - 1_600 * 5 / 100, "DeathExpPenalty NM 5 %");
        assert_eq!(w.hero.stats.level, 2, "un niveau gagné ne se perd jamais");
        let c = w.corpse().expect("corps posé");
        assert_eq!(c.x, w.hero_ent.x, "corps à la position du héros");
        assert_eq!(c.inv[0].as_ref().unwrap().code, "hp1");
        assert!(w.hero.inv.iter().all(|s| s.is_none()));
    }

}
