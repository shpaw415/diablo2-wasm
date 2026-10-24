//! `equipment` — ÉQUIPEMENT RÉEL 10 slots bodyloc `.d2s` + grille inventaire
//! multi-cases + ceinture `belts.txt` (F6/F7, étape 7c loot D2 réel).
//!
//! Complète ADDITIVEMENT [`crate::inventory`] (modèle od2 8 slots, MVP) avec
//! la géométrie D2 v1.12 exacte :
//!
//! * [`BodyLoc`] — les 10 emplacements d'équipement du format de sauvegarde
//!   `.d2s` (head 0, neck 1, torso 2, rarm 3, larm 4, belt 5, feet 6,
//!   gloves 7, rring 8, lrin 9) : tête, amulette, torse, arme principale +
//!   secondaire (« swap »), ceinture, bottes, gants, 2 anneaux ;
//! * règles d'équippage par `wtype` des tables v1.12 (helm→Head, tors→Torso,
//!   amul→Neck, ring→anneaux, belt→Belt, boot→Feet, glov→Gloves,
//!   shie→mains, armes→mains ; [`crate::loot::is_weapon_type`]) ;
//! * vérification des prérequis (`reqstr`/`reqdex`/`levelreq` des tables) ;
//! * armes deux mains ([`patch_weapons.txt`] `2handed`) : « toujours 2 mains »
//!   (`2handed=1` sans `1or2handed`, ex. `wst`) occupe les DEUX mains et
//!   bloque bouclier/échange ; « 1 ou 2 mains » (ex. `2hs`) porte ses stats
//!   2 mains de table mais se joue 1 main ici (toggle hors MVP) ;
//! * [`Equipment::derive`] — dérivation PURE équipement →
//!   [`crate::statlist::StatList`] : AC des bases roulées × ac% (id 16) + ac
//!   plat (id 31), dégâts de l'arme principale × ED affixes (id 17) + plats
//!   (21/22), tous les autres ids mergés tels quels ;
//! * [`InventoryGrid`] 10×4 (inventory.txt `Trade Page 1`) via
//!   [`place_item`] avec les tailles réelles `invwidth`/`invheight` ;
//! * ceinture variable [`ItemBelt`] : 4/8/12/16 boîtes selon la ligne
//!   `belts.txt` (colonne `belt` de `patch_armor.txt`).
//!
//! Fonctions PURES et testées, zéro IO ; `HeroStats`/`world.rs` intouchés —
//! l'intégrateur appelle `equip`/`unequip`/`derive` et additionne les
//! [`crate::statlist::StatList`] à sa base héros (save v3 intacte).
//!
//! # Conventions documentées (écarts MVP)
//! * Dégâts : seule l'arme en [`BodyLoc::RArm`] (main principale) produit
//!   les dégâts ; `LArm` = arme secondaire/« swap » (ses affixes et l'AC d'un
//!   bouclier posé là comptent, pas ses dégâts — double manie barbare hors
//!   MVP). Écart assumé vs `inventory.rs` (cumul deux mains MVP).
//! * ac%/ED cumulés appliqués aux totaux de bases (convention
//!   `inventory.rs::derive_equipped`, ordre D2 base → plats → %).

use crate::inventory::{BaseInfo, InventoryGrid};
use crate::statlist::{
    ARMOR_PERCENT, ARMORCLASS, MAXDAMAGE, MAXDAMAGE_PERCENT, MINDAMAGE, StatLine, StatList,
};

/// Les 10 emplacements d'équipement — ordre du format de sauvegarde `.d2s`
/// (body locations 0..9, Phrozen Keep / Diablerie).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum BodyLoc {
    /// Casque (helm/circ/pelt/phlm) — d2s 0.
    Head = 0,
    /// Amulette (amul) — d2s 1.
    Neck = 1,
    /// Armure de torse (tors) — d2s 2.
    Torso = 2,
    /// Main principale : arme 1 main OU bouclier — d2s 3.
    RArm = 3,
    /// Main secondaire : bouclier (1 main + shield) ou arme secondaire
    /// (« swap »), double manie barbare hors MVP — d2s 4.
    LArm = 4,
    /// Ceinture (types `belt`) — d2s 5.
    Belt = 5,
    /// Bottes (boot) — d2s 6.
    Feet = 6,
    /// Gants (glov) — d2s 7.
    Gloves = 7,
    /// Anneau droit (ring) — d2s 8.
    RingR = 8,
    /// Anneau gauche (ring) — d2s 9.
    RingL = 9,
}

/// Nombre d'emplacements d'équipement.
pub const BODY_SLOTS: usize = 10;

impl BodyLoc {
    /// Index `.d2s` (0..9).
    pub fn d2s_index(self) -> usize {
        self as usize
    }

    /// Depuis l'index `.d2s` ; `None` hors 0..9.
    pub fn from_d2s_index(i: usize) -> Option<BodyLoc> {
        Some(match i {
            0 => BodyLoc::Head,
            1 => BodyLoc::Neck,
            2 => BodyLoc::Torso,
            3 => BodyLoc::RArm,
            4 => BodyLoc::LArm,
            5 => BodyLoc::Belt,
            6 => BodyLoc::Feet,
            7 => BodyLoc::Gloves,
            8 => BodyLoc::RingR,
            9 => BodyLoc::RingL,
            _ => return None,
        })
    }
}

/// Emplacements acceptés pour un `wtype` des tables v1.12 (ordre de préférence
/// d'équippage automatique ; vide = non équipable).
pub fn allowed_bodylocs(wtype: &str) -> &'static [BodyLoc] {
    const RINGS: [BodyLoc; 2] = [BodyLoc::RingR, BodyLoc::RingL];
    const HANDS: [BodyLoc; 2] = [BodyLoc::RArm, BodyLoc::LArm];
    match wtype {
        "helm" | "circ" | "pelt" | "phlm" => &[BodyLoc::Head],
        "tors" => &[BodyLoc::Torso],
        "amul" => &[BodyLoc::Neck],
        "ring" => &RINGS,
        "belt" => &[BodyLoc::Belt],
        "boot" => &[BodyLoc::Feet],
        "glov" => &[BodyLoc::Gloves],
        // Boucliers et armes partagent les deux mains (D2 réel : bouclier à
        // LArm quand l'arme 1 main est à RArm).
        "shie" | "ashd" => &HANDS,
        w if crate::loot::is_weapon_type(w) => &HANDS,
        _ => &[],
    }
}

/// Une pièce d'équipement : base réelle des tables + AC roulé + lignes
/// d'affixes `(id ItemStatCost, valeur)` (interface « affix_ids »).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct EquipItem {
    /// Code table de la base (`ssd`, `buc`, `cap`…).
    pub code: String,
    /// Nom affichable (composé, affixes inclus).
    pub name: String,
    /// Type D2 (`swor`, `shie`, `helm`… — pilote [`allowed_bodylocs`]).
    pub wtype: String,
    /// Dégâts min 1 main (`mindam`, 0 si non arme).
    pub min_dam: u32,
    /// Dégâts max 1 main (`maxdam`).
    pub max_dam: u32,
    /// Dégâts min en mode 2 mains (`2handmindam`).
    pub twoh_min_dam: u32,
    /// Dégâts max en mode 2 mains (`2handmaxdam`).
    pub twoh_max_dam: u32,
    /// MODE courant : portée à deux mains (occupe RArm + LArm).
    pub two_handed: bool,
    /// Table : arme TOUJOURS à deux mains (`2handed=1` sans `1or2handed`,
    /// ex. `wst`) — le mode ne peut pas être coupé au MVP.
    pub always_two_handed: bool,
    /// Défense de base ROULÉE (minac..=maxac roulé à la génération).
    pub base_ac: u32,
    /// Force requise (`reqstr`).
    pub req_str: u32,
    /// Dextérité requise (`reqdex`).
    pub req_dex: u32,
    /// Niveau requis (`levelreq`).
    pub req_level: u32,
    /// Durabilité max (`durability`, 0 = indestructible/non applicable).
    pub max_durability: u32,
    /// Durabilité courante (usure/réparation, vendors F10).
    pub durability: u32,
    /// Taille inventaire X (`invwidth`).
    pub inv_w: u32,
    /// Taille inventaire Y (`invheight`).
    pub inv_h: u32,
    /// Lignes d'affixes / mods : `(id ItemStatCost, valeur)`.
    pub stats: Vec<StatLine>,
}

impl EquipItem {
    /// Depuis la base réelle [`BaseInfo`] (produite par [`crate::loot`]) + l'AC
    /// roulé + les lignes d'affixes. Durabilité/2 mains à poser ensuite via
    /// [`EquipItem::with_durability`]/[`EquipItem::with_two_handed`].
    pub fn from_base(base: &BaseInfo, rolled_ac: u32, stats: Vec<StatLine>) -> EquipItem {
        EquipItem {
            code: String::new(),
            name: base.name.clone(),
            wtype: base.wtype.clone(),
            min_dam: base.min_dam,
            max_dam: base.max_dam,
            base_ac: rolled_ac,
            req_str: base.req_str,
            req_dex: base.req_dex,
            req_level: base.req_level,
            inv_w: base.w,
            inv_h: base.h,
            stats,
            ..Default::default()
        }
    }

    /// Pose le code table (chaînage).
    pub fn with_code(mut self, code: &str) -> EquipItem {
        self.code = code.into();
        self
    }

    /// Pose la durabilité (`durability` des tables) — chaînage.
    pub fn with_durability(mut self, max: u32) -> EquipItem {
        self.max_durability = max;
        self.durability = max;
        self
    }

    /// Arme portée À DEUX MAINS avec ses dégâts 2 mains de table — chaînage.
    /// Marque aussi `always_two_handed` (le mode ne se coupe pas ici ; un
    /// toggle futur passerait par [`EquipItem::set_two_handed_mode`]).
    pub fn with_two_handed(mut self, min: u32, max: u32) -> EquipItem {
        self.twoh_min_dam = min;
        self.twoh_max_dam = max;
        self.two_handed = true;
        self.always_two_handed = true;
        self
    }

    /// Stats 2 mains de table pour une arme « 1 ou 2 mains » (ex. `2hs`,
    /// `1or2handed=1`) portée 1 main ici : porte les valeurs sans le mode.
    pub fn with_twoh_stats(mut self, min: u32, max: u32) -> EquipItem {
        self.twoh_min_dam = min;
        self.twoh_max_dam = max;
        self
    }

    /// Bascule le mode 2 mains (toggle I/II futur — 1or2h uniquement).
    pub fn set_two_handed_mode(&mut self, on: bool) {
        if !self.always_two_handed {
            self.two_handed = on;
        }
    }

    /// Dégâts effectifs selon le mode (2 mains ⇒ paire `2handmindam/maxdam`).
    fn effective_dmg(&self) -> (i64, i64) {
        if self.two_handed {
            (self.twoh_min_dam as i64, self.twoh_max_dam as i64)
        } else {
            (self.min_dam as i64, self.max_dam as i64)
        }
    }
}

/// Prérequis d'une pièce (`reqstr`/`reqdex`/`levelreq` des tables) vs
/// attributs du héros.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Missing {
    /// Force requise.
    pub req_str: u32,
    /// Dextérité requise.
    pub req_dex: u32,
    /// Niveau requis.
    pub req_level: u32,
    /// Force possédée.
    pub have_str: u32,
    /// Dextérité possédée.
    pub have_dex: u32,
    /// Niveau possédé.
    pub have_level: u32,
}

impl Missing {
    /// Tous les prérequis sont satisfaits ?
    pub fn is_empty(&self) -> bool {
        self.have_str >= self.req_str
            && self.have_dex >= self.req_dex
            && self.have_level >= self.req_level
    }
}

/// Vérifie les prérequis d'une pièce contre les attributs du héros (pur).
pub fn requirements(item: &EquipItem, strength: u32, dexterity: u32, level: u32) -> Missing {
    Missing {
        req_str: item.req_str,
        req_dex: item.req_dex,
        req_level: item.req_level,
        have_str: strength,
        have_dex: dexterity,
        have_level: level,
    }
}

/// Erreurs d'équippage (règles des tables).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EquipError {
    /// Le `wtype` n'accepte pas cet emplacement (ou type non équipable).
    NotAllowed { code: String, loc: BodyLoc },
    /// Aucun emplacement libre pour ce type (équipage automatique).
    NoSlot { code: String },
    /// Prérequis non satisfaits.
    Requirements(Missing),
    /// Une arme deux mains est portée : les deux mains sont prises.
    TwoHandedBlocked { code: String },
}

/// Résultat d'un échange d'équipement : ce qui a été retiré (à reposer dans
/// l'inventaire, convention D2).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EquipSwap {
    /// Ancienne pièce de la main principale (RArm).
    pub displaced_main: Option<EquipItem>,
    /// Ancienne pièce de la main secondaire (LArm, échange par une 2 mains).
    pub displaced_off: Option<EquipItem>,
}

/// Équipement porté : 10 slots bodyloc `.d2s` ([`None`] = vide).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Equipment {
    /// Slots indexés par [`BodyLoc::d2s_index`].
    pub slots: [Option<EquipItem>; BODY_SLOTS],
    /// Une arme deux mains occupe les deux mains (LArm virtuellement pris).
    two_handed_worn: bool,
}

impl Equipment {
    /// Équipement vide.
    pub fn new() -> Equipment {
        Equipment { slots: std::array::from_fn(|_| None), two_handed_worn: false }
    }

    /// Pièce portée à cet emplacement.
    pub fn get(&self, loc: BodyLoc) -> Option<&EquipItem> {
        self.slots[loc.d2s_index()].as_ref()
    }

    /// Une arme 2 mains est-elle portée ?
    pub fn is_two_handed_worn(&self) -> bool {
        self.two_handed_worn
    }

    /// Équipe `item` à l'emplacement `loc` : règles de type + prérequis
    /// (`strength`/`dexterity`/`level` du héros) + gestion 2 mains. Renvoie
    /// les pièces déplacées (échange). Pur.
    pub fn equip(
        &mut self,
        mut item: EquipItem,
        loc: BodyLoc,
        hero: (u32, u32, u32),
    ) -> Result<EquipSwap, EquipError> {
        let missing = requirements(&item, hero.0, hero.1, hero.2);
        if !missing.is_empty() {
            return Err(EquipError::Requirements(missing));
        }
        if item.always_two_handed {
            item.two_handed = true;
        }
        let locs = allowed_bodylocs(&item.wtype);
        if !locs.contains(&loc) {
            return Err(EquipError::NotAllowed { code: item.code.clone(), loc });
        }
        let mut swap = EquipSwap::default();
        if item.two_handed {
            // Les deux mains sont libérées (D2 : le bouclier retourne à
            // l'inventaire via `displaced_off`).
            swap.displaced_off = self.slots[BodyLoc::LArm.d2s_index()].take();
            swap.displaced_main = self.slots[BodyLoc::RArm.d2s_index()].replace(item);
            self.two_handed_worn = true;
        } else {
            if self.two_handed_worn {
                if loc != BodyLoc::RArm {
                    return Err(EquipError::TwoHandedBlocked { code: item.code });
                }
                // Échange à la main principale : la 2 mains retourne à
                // l'inventaire (convention D2).
                self.two_handed_worn = false;
            }
            swap.displaced_main = self.slots[loc.d2s_index()].replace(item);
        }
        Ok(swap)
    }

    /// Équipe au premier emplacement accepté et libre ([`allowed_bodylocs`],
    /// ordre fixe ; anneaux : droit puis gauche). `NoSlot` si tout est pris
    /// ou type non équipable.
    pub fn equip_auto(
        &mut self,
        item: EquipItem,
        hero: (u32, u32, u32),
    ) -> Result<EquipSwap, EquipError> {
        let missing = requirements(&item, hero.0, hero.1, hero.2);
        if !missing.is_empty() {
            return Err(EquipError::Requirements(missing));
        }
        let locs = allowed_bodylocs(&item.wtype);
        if locs.is_empty() {
            return Err(EquipError::NoSlot { code: item.code });
        }
        if item.two_handed || item.always_two_handed {
            return self.equip(item, BodyLoc::RArm, hero);
        }
        if self.two_handed_worn {
            return Err(EquipError::TwoHandedBlocked { code: item.code });
        }
        let free = locs.iter().copied().find(|l| self.get(*l).is_none());
        let Some(loc) = free else {
            return Err(EquipError::NoSlot { code: item.code });
        };
        let displaced = self.slots[loc.d2s_index()].replace(item);
        Ok(EquipSwap { displaced_main: displaced, displaced_off: None })
    }

    /// Retire la pièce d'un emplacement (arme 2 mains : libère les deux).
    pub fn unequip(&mut self, loc: BodyLoc) -> Option<EquipItem> {
        if loc == BodyLoc::RArm {
            self.two_handed_worn = false;
        }
        self.slots[loc.d2s_index()].take()
    }

    /// « Swap » D2 (touches I/II) : échange le contenu des deux mains
    /// (arme principale ⇔ secondaire). `false` si une arme 2 mains bloque.
    pub fn swap_weapons(&mut self) -> bool {
        if self.two_handed_worn {
            return false;
        }
        let (r, l) = (BodyLoc::RArm.d2s_index(), BodyLoc::LArm.d2s_index());
        self.slots.swap(r, l);
        true
    }

    /// L'arme principale (RArm) est-elle une arme (et pas un bouclier) ?
    fn main_weapon(&self) -> Option<&EquipItem> {
        self.get(BodyLoc::RArm).filter(|it| crate::loot::is_weapon_type(&it.wtype))
    }

    /// Dérivation PURE de l'équipement → [`StatList`] :
    /// * AC = Σ bases roulées × (1 + Σ ac% / 100) + Σ ac plat (id 31) ;
    /// * dégâts = arme principale (1H, ou paire 2H si mode 2 mains) :
    ///   (base + Σ plats) × (1 + Σ ED% / 100), floor — min ≤ max garanti ;
    /// * tous les autres ids des affixes mergés tels quels (str/dex/vit/ene,
    ///   hp, mana, res, MF/GF, AR…) ;
    /// * ids spéciaux 16/17 portent les cumuls bruts (affichage UI).
    pub fn derive(&self) -> StatList {
        let mut out = StatList::new();
        let (mut ac_pct, mut ed_pct) = (0i64, 0i64);
        let (mut flat_min, mut flat_max, mut flat_ac) = (0i64, 0i64, 0i64);
        let mut base_ac = 0u64;
        for slot in &self.slots {
            let Some(item) = slot else { continue };
            base_ac += item.base_ac as u64;
            for l in &item.stats {
                match l.id {
                    ARMOR_PERCENT => ac_pct += l.value,
                    MAXDAMAGE_PERCENT => ed_pct += l.value,
                    MINDAMAGE => flat_min += l.value,
                    MAXDAMAGE => flat_max += l.value,
                    ARMORCLASS => flat_ac += l.value,
                    _ => out.add(l.id, l.value),
                }
            }
        }
        // Défense : bases roulées × (1 + ac%/100) + ac plat.
        let ac_final = scale_u64(base_ac, ac_pct).saturating_add(flat_ac.max(0) as u64);
        out.set(ARMOR_PERCENT, ac_pct);
        out.set(ARMORCLASS, ac_final.min(u32::MAX as u64) as i64);
        // Dégâts : main principale uniquement (conventions en-tête).
        let (base_min, base_max) = self
            .main_weapon()
            .map(EquipItem::effective_dmg)
            .unwrap_or((0, 0));
        let factor = (100 + ed_pct).max(0);
        let dmg_min = (base_min + flat_min).max(0) * factor / 100;
        let dmg_max = ((base_max + flat_max).max(0) * factor / 100).max(dmg_min);
        out.set(MAXDAMAGE_PERCENT, ed_pct);
        out.set(MINDAMAGE, dmg_min);
        out.set(MAXDAMAGE, dmg_max);
        out.clamp_combat();
        out
    }
}

/// `base × (100 + pct) / 100` arrondi vers le bas (D2 floor), jamais négatif.
fn scale_u64(base: u64, pct: i64) -> u64 {
    ((base as i128 * (100 + pct).max(0) as i128) / 100) as u64
}

// --- Grille inventaire multi-cases (F7, géométrie inventory.txt v1.12) ---

/// Pose un item dans la grille (10×4 `Trade Page 1` d'`inventory.txt`) à la
/// première place libre accueillant ses tailles réelles `invwidth`×
/// `invheight` ; `idx` = index de l'item dans la liste du propriétaire
/// (port `inventory_grid.go::canFit`). Renvoie `(x, y)` ou `None` si aucune
/// place. Déterministe (balayage rangée par rangée).
pub fn place_item(grid: &mut InventoryGrid, idx: usize, item: &EquipItem) -> Option<(usize, usize)> {
    let (x, y) = grid.find_first_fit(item.inv_w, item.inv_h, None)?;
    grid.place(idx, item.inv_w, item.inv_h, x, y);
    Some((x, y))
}

// --- Ceinture variable (belts.txt v1.12) ---

/// Lignes `belts.txt` v1.12 dans l'ORDRE DU FICHIER avec leur `numboxes` :
/// l'index est la colonne `belt` de `patch_armor.txt` (0=belt, 1=sash,
/// 2=default, 3=girdle, 4=light belt, 5=heavy belt, 6=uber belt).
pub const BELT_ROWS: [(&str, usize); 7] = [
    ("belt", 12),
    ("sash", 8),
    ("default", 4),
    ("girdle", 16),
    ("light belt", 8),
    ("heavy belt", 12),
    ("uber belt", 16),
];

/// Nombre de boîtes de potion pour l'index de colonne `belt` d'`armor.txt`
/// (ceinture portée). Hors bornes ⇒ ceinture par défaut (4).
pub fn belt_boxes(index: usize) -> usize {
    BELT_ROWS.get(index).map(|(_, n)| *n).unwrap_or(4)
}

/// Ceinture de l'héros : 4 colonnes (touches 1-4) × rangées selon la
/// ceinture portée ([`belt_boxes`] / 4). Même convention de remplissage que
/// [`crate::inventory::Belt`] (rangée par rangée : une potion par touche
/// d'abord, puis empilement), taille variable data-driven.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ItemBelt {
    /// `[rangée][colonne]`, colonne 0 = touche 1.
    pub columns: Vec<[Option<EquipItem>; 4]>,
}

impl ItemBelt {
    /// Ceinture vide de `boxes` boîtes (4, 8, 12 ou 16 ; autre ⇒ 4).
    pub fn new(boxes: usize) -> ItemBelt {
        let rows = (boxes / 4).clamp(1, 4);
        ItemBelt { columns: vec![Default::default(); rows] }
    }

    /// Nombre total de boîtes (rangées × 4).
    pub fn capacity(&self) -> usize {
        self.columns.len() * 4
    }

    /// Première cellule libre (rangée puis colonne, ordre fixe).
    pub fn first_free(&self) -> Option<(usize, usize)> {
        for r in 0..self.columns.len() {
            for c in 0..4 {
                if self.columns[r][c].is_none() {
                    return Some((c, r));
                }
            }
        }
        None
    }

    /// Range un objet (potion 1×1…) ; `(colonne, rangée)` ou `None` si plein.
    pub fn store(&mut self, item: EquipItem) -> Option<(usize, usize)> {
        let (c, r) = self.first_free()?;
        self.columns[r][c] = Some(item);
        Some((c, r))
    }

    /// Prend la potion de la touche `key` (1-4, première non vide de la
    /// colonne) — même convention MVP que [`crate::inventory::Belt`].
    pub fn take_key(&mut self, key: usize) -> Option<EquipItem> {
        let c = key.checked_sub(1)?;
        (c < 4).then_some(())?;
        for r in 0..self.columns.len() {
            if let Some(it) = self.columns[r][c].take() {
                return Some(it);
            }
        }
        None
    }
}

// ---------------------------------------------------------------------------
// Tests — goldens tables v1.12 réelles : épée de départ Paladin (ssd),
// Buckler, 2 mains, swap, anneaux, canFit, ceintures.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::statlist::{HITPOINTS, MAGICBONUS, STRENGTH};

    /// Attributs du Paladin niv 1 (`patch_charstats` : str 25, dex 20, vit 25).
    const PAL: (u32, u32, u32) = (25, 20, 1);

    /// Short Sword (`patch_weapons.txt` ssd : mindam 2, maxdam 7, dur 24,
    /// 1×3, aucun prérequis) — item1 du PALADIN (`patch_charstats`, loc rarm).
    fn short_sword() -> EquipItem {
        let base = BaseInfo {
            name: "Short Sword".into(),
            wtype: "swor".into(),
            w: 1,
            h: 3,
            min_dam: 2,
            max_dam: 7,
            ..Default::default()
        };
        EquipItem::from_base(&base, 0, Vec::new()).with_code("ssd").with_durability(24)
    }

    /// Buckler (`patch_armor.txt` buc : minac 4..6, reqstr 12, dur 12, 2×2).
    fn buckler(rolled_ac: u32) -> EquipItem {
        let base = BaseInfo {
            name: "Buckler".into(),
            wtype: "shie".into(),
            w: 2,
            h: 2,
            min_ac: 4,
            max_ac: 6,
            req_str: 12,
            ..Default::default()
        };
        EquipItem::from_base(&base, rolled_ac, Vec::new())
            .with_code("buc")
            .with_durability(12)
    }

    /// GOLDEN départ réel : l'épée de départ du Paladin (charstats item1 =
    /// `ssd`, loc rarm) équipée → dégâts dérivés EXACTS de la table
    /// `patch_weapons.txt` (2-7), AC 0, durabilité 24, aucun prérequis.
    #[test]
    fn golden_paladin_starting_sword() {
        let mut eq = Equipment::new();
        let sword = short_sword();
        assert_eq!(sword.req_str, 0, "ssd : aucun prérequis en v1.12");
        let swap = eq.equip(sword, BodyLoc::RArm, PAL).expect("ssd s'équipe à rarm");
        assert!(swap.displaced_main.is_none() && swap.displaced_off.is_none());
        let d = eq.derive();
        assert_eq!(d.dmg_min(), 2, "mindam table");
        assert_eq!(d.dmg_max(), 7, "maxdam table");
        assert_eq!(d.armor_class(), 0);
        assert_eq!(d.edmg_pct(), 0);
        let worn = eq.get(BodyLoc::RArm).unwrap();
        assert_eq!((worn.max_durability, worn.durability), (24, 24), "durability table");
        assert_eq!((worn.inv_w, worn.inv_h), (1, 3), "invwidth/invheight table");
    }

    /// GOLDEN affixes : ED +50 % (id 17) et +2 min dmg (id 21) sur l'épée de
    /// départ → (2+2)×1,5 = 6 min, 7×1,5 = 10 max (floor D2).
    #[test]
    fn golden_weapon_affixes_ed_and_flat() {
        let mut eq = Equipment::new();
        let mut sword = short_sword();
        sword.stats = vec![
            StatLine::new(MAXDAMAGE_PERCENT, 50),
            StatLine::new(MINDAMAGE, 2),
        ];
        eq.equip(sword, BodyLoc::RArm, PAL).unwrap();
        let d = eq.derive();
        assert_eq!(d.dmg_min(), 6, "(2+2)×1,5 floor");
        assert_eq!(d.dmg_max(), 10, "7×1,5 floor");
        assert_eq!(d.edmg_pct(), 50, "ED brut conservé pour l'affichage");
    }

    /// GOLDEN bouclier : Buckler AC roulé 4 (minac 4..6), reqstr 12 — refus
    /// sous 12 de force, AC × ac% + ac plat sinon ; à LArm (arme 1 main +
    /// bouclier, D2 réel).
    #[test]
    fn golden_buckler_requirements_and_ac() {
        let mut eq = Equipment::new();
        let swap = eq.equip(buckler(4), BodyLoc::LArm, PAL).unwrap();
        assert!(swap.displaced_main.is_none());
        assert_eq!(eq.derive().armor_class(), 4, "AC = base roulée");
        // affixes ac% +30 (id 16) et ac plat +5 (id 31) → floor(4×1,3)+5 = 10.
        let mut eq2 = Equipment::new();
        let mut buc = buckler(4);
        buc.stats = vec![StatLine::new(ARMOR_PERCENT, 30), StatLine::new(ARMORCLASS, 5)];
        eq2.equip(buc, BodyLoc::LArm, PAL).unwrap();
        assert_eq!(eq2.derive().armor_class(), 10);
        // Prérequis : héros str 11 < reqstr 12 → refus (erreur typée exacte).
        let mut eq3 = Equipment::new();
        assert_eq!(
            eq3.equip(buckler(4), BodyLoc::LArm, (11, 20, 1)),
            Err(EquipError::Requirements(Missing {
                req_str: 12,
                req_dex: 0,
                req_level: 0,
                have_str: 11,
                have_dex: 20,
                have_level: 1,
            }))
        );
        assert!(requirements(&buckler(4), 12, 0, 1).is_empty(), "12 pile : OK");
    }

    /// Deux mains réel : War Staff (`patch_weapons.txt` wst : `2handed=1`
    /// sans `1or2handed`, 2h 12-28, dur 50, 2×4) occupe les DEUX mains,
    /// bloque le bouclier, s'échange et se retire proprement.
    #[test]
    fn golden_two_handed_war_staff() {
        let wst = EquipItem {
            code: "wst".into(),
            name: "War Staff".into(),
            wtype: "staf".into(),
            inv_w: 2,
            inv_h: 4,
            max_durability: 50,
            durability: 50,
            ..Default::default()
        }
        .with_two_handed(12, 28);
        assert!(wst.always_two_handed);
        let mut eq = Equipment::new();
        eq.equip_auto(wst.clone(), PAL).expect("2 mains OK");
        assert!(eq.get(BodyLoc::RArm).is_some() && eq.get(BodyLoc::LArm).is_none());
        let d = eq.derive();
        assert_eq!(d.dmg_min(), 12, "2handmindam table");
        assert_eq!(d.dmg_max(), 28, "2handmaxdam table");
        // Bouclier refusé, les deux mains sont prises.
        assert_eq!(
            eq.equip_auto(buckler(4), PAL),
            Err(EquipError::TwoHandedBlocked { code: "buc".into() })
        );
        assert!(!eq.swap_weapons(), "swap bloqué en 2 mains");
        // Échange de l'arme : l'ancienne est rendue (retour inventaire D2).
        let swap = eq.equip(short_sword(), BodyLoc::RArm, PAL).unwrap();
        assert_eq!(swap.displaced_main.unwrap().code, "wst");
        assert!(swap.displaced_off.is_none(), "LArm était vide");
        assert!(!eq.two_handed_worn, "2 mains libérées après échange");
        assert_eq!(eq.derive().dmg_min(), 2);
        // Le bouclier passe maintenant (LArm libre).
        assert!(eq.equip_auto(buckler(4), PAL).is_ok());
    }

    /// Arme « 1 ou 2 mains » (`2hs` Two-Handed Sword : 2h 8-17 de table) :
    /// portée 1 main elle garde ses dégâts 1 main de table (2-9) et une seule
    /// main est occupée (le bouclier passe).
    #[test]
    fn one_or_two_handed_uses_1h_stats_when_worn() {
        let twohs = EquipItem {
            code: "2hs".into(),
            name: "Two-Handed Sword".into(),
            wtype: "swor".into(),
            min_dam: 2,
            max_dam: 9,
            req_str: 35,
            req_dex: 27,
            max_durability: 44,
            durability: 44,
            inv_w: 1,
            inv_h: 4,
            ..Default::default()
        }
        .with_twoh_stats(8, 17);
        assert!(!twohs.two_handed && !twohs.always_two_handed);
        let mut eq = Equipment::new();
        // Paladin str 25 < reqstr 35 → refus avec prérequis exacts.
        assert!(matches!(
            eq.equip(twohs.clone(), BodyLoc::RArm, PAL),
            Err(EquipError::Requirements(_))
        ));
        // Héros str 40 / dex 27 fictif : OK.
        eq.equip(twohs, BodyLoc::RArm, (40, 27, 1)).unwrap();
        let d = eq.derive();
        assert_eq!(d.dmg_min(), 2, "1H : mindam table");
        assert_eq!(d.dmg_max(), 9, "1H : maxdam table (mode 2H = toggle hors MVP)");
        // Une seule main occupée : le bouclier passe à LArm.
        assert!(eq.equip_auto(buckler(4), (40, 27, 1)).is_ok());
        assert_eq!(eq.derive().armor_class(), 4, "AC du bouclier compté");
    }

    /// Swap D2 : épée à RArm + javeline secondaire à LArm → swap → dégâts
    /// javeline (1-5, `patch_weapons.txt` jav, dur 2).
    #[test]
    fn swap_weapons_exchanges_hands() {
        let mut eq = Equipment::new();
        eq.equip(short_sword(), BodyLoc::RArm, PAL).unwrap();
        let jav = BaseInfo {
            name: "Javelin".into(),
            wtype: "jave".into(),
            w: 1,
            h: 3,
            min_dam: 1,
            max_dam: 5,
            ..Default::default()
        };
        let jav = EquipItem::from_base(&jav, 0, Vec::new()).with_code("jav").with_durability(2);
        eq.equip(jav, BodyLoc::LArm, PAL).unwrap();
        assert_eq!(eq.derive().dmg_min(), 2, "main principale = ssd");
        assert_eq!(eq.derive().dmg_max(), 7);
        assert!(eq.swap_weapons());
        let d = eq.derive();
        assert_eq!(d.dmg_min(), 1, "après swap : jav en principale");
        assert_eq!(d.dmg_max(), 5);
        assert_eq!(eq.get(BodyLoc::RArm).unwrap().code, "jav");
        assert!(eq.swap_weapons());
        assert_eq!(eq.get(BodyLoc::RArm).unwrap().code, "ssd");
    }

    /// Règles de slots par `wtype` des tables + 2 anneaux (droit puis gauche).
    #[test]
    fn bodyloc_rules_and_two_rings() {
        assert_eq!(allowed_bodylocs("helm"), &[BodyLoc::Head]);
        assert_eq!(allowed_bodylocs("tors"), &[BodyLoc::Torso]);
        assert_eq!(allowed_bodylocs("amul"), &[BodyLoc::Neck]);
        assert_eq!(allowed_bodylocs("boot"), &[BodyLoc::Feet]);
        assert_eq!(allowed_bodylocs("glov"), &[BodyLoc::Gloves]);
        assert_eq!(allowed_bodylocs("belt"), &[BodyLoc::Belt]);
        assert_eq!(allowed_bodylocs("ring"), &[BodyLoc::RingR, BodyLoc::RingL]);
        assert_eq!(allowed_bodylocs("shie"), &[BodyLoc::RArm, BodyLoc::LArm]);
        assert_eq!(allowed_bodylocs("swor"), &[BodyLoc::RArm, BodyLoc::LArm]);
        assert!(allowed_bodylocs("hp1").is_empty(), "potion non équipable");
        assert_eq!(BodyLoc::from_d2s_index(9), Some(BodyLoc::RingL));
        assert_eq!(BodyLoc::from_d2s_index(10), None);
        assert_eq!(BodyLoc::RingL.d2s_index(), 9);

        let ring = |v: i64| EquipItem {
            code: "rin".into(),
            name: "Ring".into(),
            wtype: "ring".into(),
            inv_w: 1,
            inv_h: 1,
            stats: vec![StatLine::new(STRENGTH, v), StatLine::new(MAGICBONUS, 10)],
            ..Default::default()
        };
        let mut eq = Equipment::new();
        eq.equip_auto(ring(2), PAL).unwrap();
        eq.equip_auto(ring(3), PAL).unwrap();
        assert_eq!(eq.get(BodyLoc::RingR).unwrap().stats[0].value, 2, "1er anneau à droite");
        assert_eq!(eq.get(BodyLoc::RingL).unwrap().stats[0].value, 3, "2e à gauche");
        assert_eq!(eq.equip_auto(ring(4), PAL), Err(EquipError::NoSlot { code: "rin".into() }));
        assert_eq!(eq.derive().strength(), 5, "affixes des 2 anneaux mergés");
        assert_eq!(eq.derive().magic_find(), 20);
        // Mauvais slot : amulette à la tête refusée (règle bodyloc).
        let mut amu = ring(0);
        amu.wtype = "amul".into();
        amu.code = "amu".into();
        assert_eq!(
            eq.equip(amu, BodyLoc::Head, PAL),
            Err(EquipError::NotAllowed { code: "amu".into(), loc: BodyLoc::Head })
        );
    }

    /// Unequip + dérivation réversible (additif : rien ne fuit hors la pièce).
    #[test]
    fn unequip_reverts_derivation() {
        let mut eq = Equipment::new();
        let mut sword = short_sword();
        sword.stats = vec![StatLine::new(MAXDAMAGE_PERCENT, 50)];
        eq.equip(sword, BodyLoc::RArm, PAL).unwrap();
        assert_eq!(eq.derive().dmg_max(), 10);
        let back = eq.unequip(BodyLoc::RArm).expect("pièce rendue");
        assert_eq!(back.code, "ssd");
        assert_eq!(back.max_durability, 24);
        let d = eq.derive();
        assert_eq!(d.dmg_min(), 0);
        assert_eq!(d.dmg_max(), 0);
        assert_eq!(d.armor_class(), 0);
        assert!(d.len() <= 5, "au pire les 5 cumuls spéciaux à 0 : {:?}", d.iter().collect::<Vec<_>>());
    }

    /// GOLDEN canFit : tailles réelles v1.12 — ssd 1×3, buc 2×2, 2hs 1×4 ;
    /// la grille 10×4 se sature, les cas bornes connus refusent.
    #[test]
    fn golden_canfit_real_sizes_10x4() {
        let mut g = InventoryGrid::inventory();
        assert_eq!((g.w, g.h), (10, 4), "inventory.txt Trade Page 1");
        let mut idx = 0usize;
        let ssd = short_sword();
        assert_eq!(place_item(&mut g, idx, &ssd), Some((0, 0)), "ssd 1×3 posé");
        idx += 1;
        assert!(place_item(&mut g, idx, &buckler(4)).is_some(), "buc 2×2 posé (1,0)");
        idx += 1;
        let twohs = EquipItem {
            code: "2hs".into(),
            name: "Two-Handed Sword".into(),
            wtype: "swor".into(),
            inv_w: 1,
            inv_h: 4,
            ..Default::default()
        };
        // Colonnes 1×4 jusqu'à saturation (les colonnes 3..9 : 7 places).
        let mut placed = 0;
        while place_item(&mut g, idx, &twohs).is_some() {
            idx += 1;
            placed += 1;
        }
        assert_eq!(placed, 7, "7 colonnes entières libres après ssd+buc");
        // Cas bornes connus : vrai/faux exacts.
        assert_eq!(g.find_first_fit(1, 1, None), Some((1, 2)), "première case libre (sous le buc)");
        assert!(!g.can_fit(2, 2, 9, 3, None), "2×2 à (9,3) déborde en X");
        assert!(!g.can_fit(1, 4, 0, 1, None), "1×4 sous le ssd déborde en Y");
        assert!(g.can_fit(2, 2, 1, 2, None), "2×2 sous le buc : OK");
    }

    /// GOLDEN ceintures `belts.txt` : colonne `belt` d'`armor.txt` → boîtes
    /// (mbl/Belt 12, lbl/Sash 8, hbl/Girdle 16, vbl 8, tbl 12, uber 16) ;
    /// capacité variable et touches 1-4.
    #[test]
    fn golden_belt_rows_from_table() {
        // Index = colonne `belt` de patch_armor.txt (ordre belts.txt fichier).
        assert_eq!(belt_boxes(0), 12, "mbl « Belt(M) » → ligne belt");
        assert_eq!(belt_boxes(1), 8, "lbl « Sash(L) » → ligne sash");
        assert_eq!(belt_boxes(2), 4, "ligne default");
        assert_eq!(belt_boxes(3), 16, "hbl « Girdle(H) » → ligne girdle");
        assert_eq!(belt_boxes(4), 8, "vbl « Light Belt » → light belt");
        assert_eq!(belt_boxes(5), 12, "tbl « Heavy Belt » → heavy belt");
        assert_eq!(belt_boxes(6), 16, "expansion (zlb…) → uber belt (table)");
        assert_eq!(belt_boxes(7), 4, "hors table ⇒ défaut");
        assert_eq!(belt_boxes(usize::MAX), 4);

        // Ceinture 12 boîtes (Belt) : 4 touches × 3 rangées.
        let mut belt = ItemBelt::new(belt_boxes(0));
        assert_eq!(belt.capacity(), 12);
        let pot = |n: &str| EquipItem {
            code: "hp1".into(),
            name: n.into(),
            wtype: "hpot".into(),
            inv_w: 1,
            inv_h: 1,
            ..Default::default()
        };
        for i in 0..12 {
            assert!(belt.store(pot(&format!("p{i}"))).is_some(), "place {i}");
        }
        assert!(belt.store(pot("overflow")).is_none(), "ceinture pleine");
        let t = belt.take_key(1).expect("touche 1");
        assert_eq!(t.name, "p0");
        assert!(belt.take_key(5).is_none(), "touche 5 inexistante");
        // Girdle 16 boîtes, ceinture par défaut 4.
        assert_eq!(ItemBelt::new(belt_boxes(3)).capacity(), 16);
        assert_eq!(ItemBelt::new(belt_boxes(2)).capacity(), 4);
    }

    /// Le socle agrège base héros + dérivation équipement (contrat A1).
    #[test]
    fn hero_total_is_base_plus_derived() {
        let mut eq = Equipment::new();
        let ring = EquipItem {
            code: "rin".into(),
            name: "Ring".into(),
            wtype: "ring".into(),
            inv_w: 1,
            inv_h: 1,
            stats: vec![StatLine::new(STRENGTH, 5)],
            ..Default::default()
        };
        eq.equip_auto(ring, PAL).unwrap();
        let base: Vec<StatLine> = vec![
            StatLine::new(STRENGTH, 25),
            StatLine::new(HITPOINTS, 55),
        ];
        let total = StatList::from_sources([base, eq.derive().as_lines()]);
        assert_eq!(total.strength(), 30, "25 paladin + 5 anneau");
        assert_eq!(total.hp(), 55);
    }
}
