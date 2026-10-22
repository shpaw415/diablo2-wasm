//! `d2missiles` — projectiles du port Diablo 2 v1.12 (MVP Phase 2, missile).
//!
//! Crate PURE comme `d2rules` : pas de rendu, pas d'IO, pas de wasm —
//! `serde`/`serde_json` seuls. Les définitions viennent de la table RÉELLE
//! `assets/patch_missiles.txt` (embarquée en compile-time, cf. `table.rs`) ;
//! la simulation avance des missiles en ligne droite, teste la collision avec
//! les entités via une closure fournie par l'appelant, et expire à portée.
//!
//! # Contrat suggéré (brief) — respecté à l'identique
//! ```ignore
//! let mut sys = MissileSystem::new();              // table réelle embarquée
//! sys.spawn("arrow", (10.0, 10.0), (30.0, 10.0));  // flèche vers l'est
//! let evs = sys.tick(50, &mut |x, y| find_entity(x, y)); // -> Vec<MissileEvent>
//! ```
//!
//! # Données réelles (patch_missiles.txt v1.12)
//! * `vel` (colonne `Vel`, yards/s = tuiles/s — 1 tuile = 1 yard) : arrow 24,
//!   firebolt 20, icebolt/chargedbolt 12.
//! * `range` (colonne `Range`, yards) : arrow 40, bolt 50, chargedbolt 98,
//!   throwaxe 20.
//! * `hit_class` (colonne `HitClass`) et `cel_file` (colonne `CelFile`,
//!   sprite DC6 `data\global\missiles\<CelFile>.dc6`) — pour le rendu d2game.
//! * `elem` : colonne `EType` de `patch_missiles.txt`, complétée par la
//!   colonne `EType` du skill porteur de `patch_skills.txt` (firebolt → fire,
//!   icebolt → cold, chargedbolt → ltng, magicarrow → mag).
//! * Dégâts : les carriers d'arme (arrow/bolt/javelin/throwaxe/firearrow/
//!   icearrow) transportent les dégâts de l'arme (règle D2, colonnes `SrcDmg`
//!   vides) — l'appelant les résout via [`MissileSystem::spawn_weapon`].
//!   Les bolts de compétence portent leurs dégâts officiels niveau 1
//!   (firebolt 3–8, icebolt 3–7, chargedbolt 2–6, magicarrow +1) : les
//!   colonnes `MinDam/MaxDam` de `patch_skills.txt` sont vides en v1.12
//!   (dégâts calculés par le code interne des niveaux).
//!
//! # Déterminisme
//! Même graine ([`MissileSystem::with_seed`]) ⇒ mêmes événements tick par
//! tick. Le jet de dégâts utilise un SplitMix64 privé, sérialisé avec l'état.

#![forbid(unsafe_code)]

mod rng;
mod table;

#[cfg(test)]
mod tests;

pub use rng::Rng;
use serde::{Deserialize, Serialize};

/// Élémentaire de dégâts (colonne `EType` des tables réelles).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DamageType {
    /// Aucun `EType` (flèches physiques).
    #[default]
    Physical,
    /// `fire`.
    Fire,
    /// `cold`.
    Cold,
    /// `ltng`.
    Lightning,
    /// `mag`.
    Magic,
    /// `pois`.
    Poison,
}

impl DamageType {
    /// Mappe la chaîne `EType` des tables (valeur inconnue ⇒ Physical).
    pub fn parse(s: &str) -> DamageType {
        match s.trim() {
            "fire" | "1" => DamageType::Fire,
            "cold" | "4" => DamageType::Cold,
            "ltng" | "lightning" | "2" => DamageType::Lightning,
            "mag" | "magic" => DamageType::Magic,
            "pois" | "poison" => DamageType::Poison,
            _ => DamageType::Physical,
        }
    }
}

/// Source des dégâts d'un missile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DmgSrc {
    /// Dégâts portés par l'arme (flèche, javelot…) — résolus par l'appelant
    /// via [`MissileSystem::spawn_weapon`] (règle D2 : la flèche transporte
    /// les dégâts de l'arc).
    #[default]
    Weapon,
    /// Dégâts propres de la compétence (bolts de sorcière) — dans `dmg_min/max`.
    Skill,
}

/// Définition d'un missile, lue depuis la table réelle `patch_missiles.txt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MissileDef {
    /// Code table (`arrow`, `firebolt`…).
    pub id: String,
    /// Vitesse en tuiles/s (colonne `Vel`, 1 yard = 1 tuile).
    pub vel: f32,
    /// Portée en tuiles (colonne `Range`) — le missile expire au-delà.
    pub range: f32,
    /// Colonne `HitClass` (0 si absente — résidu de classification D2).
    pub hit_class: u32,
    /// Élémentaire (`EType` missiles.txt + skills.txt).
    pub elem: DamageType,
    /// Dégâts min (`dmgSrc = Skill`) sinon 0 (résolus à l'apparition).
    pub dmg_min: u32,
    /// Dégâts max (idem).
    pub dmg_max: u32,
    /// D'où viennent les dégâts.
    pub dmg_src: DmgSrc,
    /// Sprite DC6 (`data\global\missiles\<cel_file>.dc6`).
    pub cel_file: String,
    /// Frames d'animation (`AnimLen`).
    pub anim_len: u32,
}

/// Missile en vol : position, direction, distance parcourue.
/// Les dégâts sont RÉSOLUS à l'apparition (jet D2 : la flèche "sait" ses
/// dégâts au tir) — l'événement de collision n'a plus qu'à les restituer.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Missile {
    /// Identifiant unique croissant (réutilisable après rechargement).
    pub id: u32,
    /// Code table (`arrow`…) — pour choisir le sprite côté rendu.
    pub def_id: String,
    /// Position x (espace tuile continu).
    pub x: f32,
    /// Position y (y vers le bas).
    pub y: f32,
    /// Vitesse x (tuiles/s) = direction × [`MissileDef::vel`].
    pub vx: f32,
    /// Vitesse y (tuiles/s).
    pub vy: f32,
    /// Distance déjà parcourue (tuiles).
    pub travelled: f32,
    /// Portée totale (tuiles, copiée de la définition).
    pub range: f32,
    /// Dégâts min résolus.
    pub dmg_min: u32,
    /// Dégâts max résolus.
    pub dmg_max: u32,
    /// Élémentaire (copié de la définition).
    pub elem: DamageType,
}

impl Missile {
    /// Direction unitaire courante (0 si vecteur nul).
    pub fn dir(&self) -> (f32, f32) {
        let d = (self.vx * self.vx + self.vy * self.vy).sqrt();
        if d <= f32::EPSILON {
            (0.0, 0.0)
        } else {
            (self.vx / d, self.vy / d)
        }
    }
}

/// Événements produits par [`MissileSystem::tick`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MissileEvent {
    /// Missile `id` (`def`) apparu en `(x, y)`.
    Spawned { id: u32, def: String, x: f32, y: f32 },
    /// Le missile `id` a touché l'entité `target` : `dmg` dégâts (jet au tir),
    /// en `(x, y)`. Le missile disparaît (`CollideKill = 1`, table réelle).
    Hit { id: u32, target: usize, dmg: u32, x: f32, y: f32 },
    /// Le missile `id` a atteint sa portée sans rien toucher.
    Expired { id: u32 },
    /// Le missile `id` a rencontré un mur ( [`MissileSystem::tick_walls`]).
    Blocked { id: u32 },
}

/// Longueur max d'un sous-pas de collision (tuiles) : anti-tunneling, une
/// flèche à 24 tuiles/s avance 2,4 tuiles par tick de 100 ms — on découpe
/// pour ne jamais sauter par-dessus une entité de rayon ≥ 0,5 tuile.
pub(crate) const SUBSTEP_TILES: f32 = 0.5;
/// Plafond de sous-pas par tick (sécurité anti-boucle : 64 × 0,5 = 32 tuiles).
pub(crate) const MAX_SUBSTEPS: u32 = 64;
/// dt max accepté par tick (aligné sur d2rules, tronqué au-delà).
pub(crate) const MAX_DT_MS: u32 = 200;

/// Système de projectiles : table de définitions + missiles en vol + RNG de
/// jet de dégâts. Pur, sérialisable en entier (save/load d2rules).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissileSystem {
    /// Définitions disponibles (table réelle au MVP).
    pub(crate) table: Vec<MissileDef>,
    /// Missiles en vol (ordre d'apparition, stable pendant un tick).
    pub(crate) missiles: Vec<Missile>,
    /// Prochain identifiant de missile.
    pub(crate) next_id: u32,
    /// RNG des jets de dégâts (SplitMix64, état sérialisé).
    pub(crate) rng: Rng,
}

impl MissileSystem {
    /// Table réelle `patch_missiles.txt` embarquée, graine par défaut.
    pub fn new() -> MissileSystem {
        MissileSystem::with_seed(0xD24D_1555_1A4E_D2A5)
    }

    /// Table réelle embarquée avec une graine explicite (tests, replay).
    pub fn with_seed(seed: u64) -> MissileSystem {
        let table = table::parse_table_err(&serde_json::from_str(table::REAL_MISSILES_JSON).expect("table réelle : JSON embarqué valide")).expect("table réelle embarquée valide");
        MissileSystem { table, missiles: Vec::new(), next_id: 1, rng: Rng::new(seed) }
    }

    /// Table personnalisée depuis un JSON `{"missiles": [...]}` (override
    /// manifest ; mêmes champs que la table réelle). `Err` = JSON invalide.
    pub fn from_json(v: serde_json::Value) -> Result<MissileSystem, String> {
        let table = table::parse_table_err(&v)?;
        Ok(MissileSystem { table, missiles: Vec::new(), next_id: 1, rng: Rng::new(0xD24D_1555_1A4E_D2A5) })
    }

    /// Définition par code (insensible à la casse).
    pub fn def(&self, def_id: &str) -> Option<&MissileDef> {
        let up = def_id.to_ascii_lowercase();
        self.table.iter().find(|d| d.id == up)
    }

    /// Table complète (lecture).
    pub fn defs(&self) -> &[MissileDef] {
        &self.table
    }

    /// Missiles en vol (lecture, ordre d'apparition).
    pub fn missiles(&self) -> &[Missile] {
        &self.missiles
    }

    /// Nombre de missiles en vol.
    pub fn len(&self) -> usize {
        self.missiles.len()
    }

    /// Aucun missile en vol ?
    pub fn is_empty(&self) -> bool {
        self.missiles.is_empty()
    }

    /// Missile par identifiant.
    pub fn get(&self, id: u32) -> Option<&Missile> {
        self.missiles.iter().find(|m| m.id == id)
    }

    /// Retire tous les missiles en vol.
    pub fn clear(&mut self) {
        self.missiles.clear();
    }

    /// Tire un missile `def_id` de `from` vers `to` (direction libre — peut
    /// traverser les murs : la collision les gère, cf. `tick_walls`).
    /// Dégâts = ceux de la définition (carriers `Skill` ; 0 pour `Weapon`).
    /// Aucun effet si le code est inconnu (retourne `None`).
    pub fn spawn(&mut self, def_id: &str, from: (f32, f32), to: (f32, f32)) -> Option<u32> {
        let def = self.def(def_id)?.clone();
        Some(self.spawn_def(def, from, to, None))
    }

    /// Comme [`MissileSystem::spawn`] mais avec les dégâts d'arme résolus par
    /// l'appelant (carriers `Weapon` : la flèche transporte les dégâts de
    /// l'arc — l'appelant applique lui-même le multiplicateur de force).
    pub fn spawn_weapon(
        &mut self,
        def_id: &str,
        from: (f32, f32),
        to: (f32, f32),
        dmg_min: u32,
        dmg_max: u32,
    ) -> Option<u32> {
        let def = self.def(def_id)?.clone();
        Some(self.spawn_def(def, from, to, Some((dmg_min.max(1), dmg_max.max(dmg_min.max(1))))))
    }

    /// Cœur d'apparition : résout les dégâts et pousse le missile.
    fn spawn_def(&mut self, def: MissileDef, from: (f32, f32), to: (f32, f32), weapon: Option<(u32, u32)>) -> u32 {
        let (dx, dy) = (to.0 - from.0, to.1 - from.1);
        let d = (dx * dx + dy * dy).sqrt();
        let (vx, vy) = if d <= f32::EPSILON {
            (0.0, 0.0) // tir sur place : le missile expire immédiatement
        } else {
            (dx / d * def.vel, dy / d * def.vel)
        };
        let (dmg_min, dmg_max) = match weapon {
            Some((mn, mx)) => (mn, mx.max(mn)),
            None => (def.dmg_min, def.dmg_max.max(def.dmg_min)),
        };
        let id = self.next_id;
        self.next_id += 1;
        self.missiles.push(Missile {
            id,
            def_id: def.id,
            x: from.0,
            y: from.1,
            vx,
            vy,
            travelled: 0.0,
            range: def.range.max(0.0),
            dmg_min,
            dmg_max,
            elem: def.elem,
        });
        id
    }

    /// Un pas de simulation : avance chaque missile, teste la collision avec
    /// les entités via `hit_test(x, y) -> Option<index>`, expire à portée.
    /// Le premier `Some(i)` gagne (ordre d'apparition des missiles, puis
    /// sous-pas dans l'ordre du vol).
    pub fn tick(
        &mut self,
        dt_ms: u32,
        hit_test: &mut dyn FnMut(f32, f32) -> Option<usize>,
    ) -> Vec<MissileEvent> {
        self.tick_walls(dt_ms, hit_test, &mut |_, _| false)
    }

    /// Comme [`MissileSystem::tick`] avec test de mur : `blocks(x, y) == true`
    /// ⇒ le missile est bloqué (événement [`MissileEvent::Blocked`], disparait
    /// — `CollideKill = 1` dans la table réelle s'applique aussi aux murs).
    pub fn tick_walls(
        &mut self,
        dt_ms: u32,
        hit_test: &mut dyn FnMut(f32, f32) -> Option<usize>,
        blocks: &mut dyn FnMut(f32, f32) -> bool,
    ) -> Vec<MissileEvent> {
        let dt = dt_ms.min(MAX_DT_MS);
        let mut ev = Vec::new();
        if self.missiles.is_empty() || dt == 0 {
            return ev;
        }
        // Sous-pas uniformes (déterminisme) : le plus rapide dicte la coupe.
        let max_dist = self
            .missiles
            .iter()
            .map(|m| m.speed() * dt as f32 / 1000.0)
            .fold(0.0_f32, f32::max);
        let steps = ((max_dist / SUBSTEP_TILES).ceil() as u32).clamp(1, MAX_SUBSTEPS);
        let step_ms = dt as f32 / steps as f32;

        let n = self.missiles.len();
        let mut dead = vec![false; n];
        for mi in 0..n {
            for _ in 0..steps {
                if dead[mi] {
                    break;
                }
                match self.step_one(mi, step_ms, hit_test, blocks) {
                    StepOutcome::Hit { id, target, dmg, x, y } => {
                        ev.push(MissileEvent::Hit { id, target, dmg, x, y });
                        dead[mi] = true;
                    }
                    StepOutcome::Blocked { id } => {
                        ev.push(MissileEvent::Blocked { id });
                        dead[mi] = true;
                    }
                    StepOutcome::Expired { id } => {
                        ev.push(MissileEvent::Expired { id });
                        dead[mi] = true;
                    }
                    StepOutcome::Flying => {}
                }
            }
        }
        for i in (0..n).rev() {
            if dead[i] {
                self.missiles.remove(i);
            }
        }
        ev
    }

    /// Avance le missile `mi` d'un sous-pas et teste collisions/expiration.
    fn step_one(
        &mut self,
        mi: usize,
        step_ms: f32,
        hit_test: &mut dyn FnMut(f32, f32) -> Option<usize>,
        blocks: &mut dyn FnMut(f32, f32) -> bool,
    ) -> StepOutcome {
        let m = &mut self.missiles[mi];
        let move_by = m.speed() * step_ms / 1000.0;
        let remain = m.range - m.travelled;
        let move_by = move_by.min(remain.max(0.0));
        let (ux, uy) = m.dir();
        m.x += ux * move_by;
        m.y += uy * move_by;
        m.travelled += move_by;

        if blocks(m.x, m.y) {
            return StepOutcome::Blocked { id: m.id };
        }
        if let Some(target) = hit_test(m.x, m.y) {
            // Jet de dégâts au moment du contact (le tir a résolu min/max).
            let dmg = self.rng.range_u32(m.dmg_min, m.dmg_max).max(1);
            return StepOutcome::Hit { id: m.id, target, dmg, x: m.x, y: m.y };
        }
        if m.travelled >= m.range {
            return StepOutcome::Expired { id: m.id };
        }
        StepOutcome::Flying
    }
}

impl Default for MissileSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// Résultat d'un sous-pas de [`MissileSystem::step_one`].
enum StepOutcome {
    Flying,
    Hit { id: u32, target: usize, dmg: u32, x: f32, y: f32 },
    Blocked { id: u32 },
    Expired { id: u32 },
}

impl Missile {
    /// Vitesse courante (tuiles/s) — norme du vecteur vitesse.
    pub fn speed(&self) -> f32 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }
}
