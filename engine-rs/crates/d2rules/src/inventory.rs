//! `inventory` — l'ITEM GÉNÉRIQUE riche, la grille 10×4, l'équipement 8
//! slots et la ceinture 4×4 (audit F1/F6/F7 + DoD §5).
//!
//! * [`Item`] : code, qualité, affixes, uniques/sets, props roulées, taille
//!   X×Y (`invwidth`/`invheight` des tables v1.12) — port d'od2
//!   `inventory_grid.go` (canFit) + `item.go::InventoryGridSize`.
//! * [`InventoryGrid`] : grille de slots occupables (`Trade Page 1` =
//!   10×4, géométrie `inventory.txt` v1.12), `can_fit` rectangle à
//!   l'emplacement (item porté ignorable pour l'échange).
//! * [`Equipment`] : 8 slots (réf. od2 `character_equipment.go` :
//!   Head/Torso/Legs/RightArm/LeftArm/LeftHand/RightHand/Shield) + type →
//!   slot D2 réel (helm→Head, tors→Torso, boot→Legs, glov→RightArm,
//!   shie→Shield, arme→LeftHand/RightHand).
//! * [`Belt`] : 4×4 (4 colonnes, touches 1-4), potions uniquement.
//! * [`derive_equipped`] : stats dérivées — AC roulé de la base réelle
//!   (minac..maxac) + `ac%`/`ac` via descripteurs ; dégâts arme
//!   (mindam..maxdam) + `dmg%`/`dmg-min`/`dmg-max` ; str/dex/vit/enr,
//!   vie, mana, résistances, MF/GF, toucher. Déterministe via [`Rng`].
//!
//! Zéro IO ; les bases réelles arrivent de [`crate::loot`] (tables v1.12
//! embarquées) via [`BaseInfo`].

use crate::item_props::StatProp;
use crate::item_quality::Quality;
use crate::rng::Rng;

/// Largeur de la grille d'inventaire (inventory.txt « Trade Page 1 », D2).
pub const GRID_W: usize = 10;
/// Hauteur de la grille d'inventaire (4 rangées, D2).
pub const GRID_H: usize = 4;

/// Stats de base d'un item des tables v1.12 (rempli par [`crate::loot`]
/// depuis patch_weapons/armor/misc.txt — port `item_common_record.go`).
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BaseInfo {
    /// Nom affichable anglais (colonne `name`).
    pub name: String,
    /// Type D2 (`axe`, `shie`, `hpot`… — pilote les slots d'équipement).
    pub wtype: String,
    /// Taille inventaire X (`invwidth`).
    pub w: u32,
    /// Taille inventaire Y (`invheight`).
    pub h: u32,
    /// AC min (`minac`, armures).
    pub min_ac: u32,
    /// AC max (`maxac`, armures).
    pub max_ac: u32,
    /// Dégâts min 1 main (`mindam`, armes).
    pub min_dam: u32,
    /// Dégâts max 1 main (`maxdam`, armes).
    pub max_dam: u32,
    /// Force requise (`reqstr`).
    pub req_str: u32,
    /// Dextérité requise (`reqdex`).
    pub req_dex: u32,
    /// Niveau requis (`levelreq`).
    pub req_level: u32,
}

/// Un item complet généré (base réelle + qualité + affixes + props roulées).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Item {
    /// Code table de la BASE (`hax`, `buc`, `hp1`…).
    pub code: String,
    /// Nom composé (prefix + base + suffix / unique / set).
    pub name: String,
    /// Qualité D2 résolue.
    pub quality: Quality,
    /// Unique identifié (clé TBL `index`), sinon `None`.
    pub unique: Option<String>,
    /// Pièce de set (clé TBL `index`), sinon `None`.
    pub set_item: Option<String>,
    /// Set parent (clé Sets.txt), sinon `None`.
    pub set: Option<String>,
    /// Noms des prefixes magiques (ordre de tirage).
    pub prefixes: Vec<String>,
    /// Noms des suffixes magiques.
    pub suffixes: Vec<String>,
    /// Toutes les propriétés roulées (affixes + uniques + sets), en ordre
    /// fixe : prefixes → suffixes → unique/set.
    pub props: Vec<crate::item_props::RolledProp>,
    /// Taille grille X (`invwidth`).
    pub w: u32,
    /// Taille grille Y (`invheight`).
    pub h: u32,
    /// Position dans la grille (colonne, rangée) si posé.
    pub grid: Option<(usize, usize)>,
    /// Identifié (F9 — les uniques/sets/rares/magiques dropent non-identifiés).
    pub identified: bool,
    /// Bases réelles pour les stats dérivées.
    pub base: BaseInfo,
}

impl Item {
    /// AC de base ROULÉ de l'item (armures : `minac..=maxac`, sinon 0) —
    /// un tirage [`Rng`] ; port `item.go::updateItemAttributes` (le D2 roule
    /// la défense à la génération).
    pub(crate) fn rolled_ac(&self, rng: &mut Rng) -> u32 {
        if self.base.max_ac == 0 {
            return 0;
        }
        rng.range_u32(self.base.min_ac, self.base.max_ac)
    }

    /// Somme des valeurs typées d'une stat sur toutes les props roulées.
    pub fn prop_sum(&self, pick: fn(&StatProp) -> Option<i32>) -> i32 {
        self.props
            .iter()
            .filter_map(|p| p.typed.as_ref().and_then(pick))
            .sum()
    }
}

/// Sélecteur d'une stat typée (ex. `|p| match p { StatProp::Ac(v) => Some(*v), _ => None }`).
pub type StatPick<'a> = fn(&StatProp) -> Option<i32>;

/// Grille de slots (occupant = index d'item), port `inventory_grid.go`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InventoryGrid {
    /// Largeur (10 pour l'inventaire D2).
    pub w: usize,
    /// Hauteur (4).
    pub h: usize,
    /// `occupied[y * w + x]` = Some(index item) ou None.
    pub occupied: Vec<Option<usize>>,
}

impl InventoryGrid {
    /// Grille 10×4 vide (géométrie `inventory.txt` Trade Page 1).
    pub fn inventory() -> InventoryGrid {
        InventoryGrid::new(GRID_W, GRID_H)
    }

    /// Grille vide w×h.
    pub fn new(w: usize, h: usize) -> InventoryGrid {
        InventoryGrid { w, h, occupied: vec![None; w * h] }
    }

    /// Rectangle `w×h` à `(x,y)` libre ? (`ignore` = index porté, pour
    /// l'échange — port `inventory_grid.go::canFit`.)
    pub fn can_fit(&self, w: u32, h: u32, x: usize, y: usize, ignore: Option<usize>) -> bool {
        let (w, h) = (w as usize, h as usize);
        if w == 0 || h == 0 || x + w > self.w || y + h > self.h {
            return false;
        }
        for cy in y..y + h {
            for cx in x..x + w {
                if let Some(occ) = self.occupied[cy * self.w + cx] {
                    if Some(occ) != ignore {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Première position libre qui accueille `w×h` (balayage rangée par
    /// rangée, ordre fixe — déterministe).
    pub fn find_first_fit(&self, w: u32, h: u32, ignore: Option<usize>) -> Option<(usize, usize)> {
        for y in 0..self.h {
            for x in 0..self.w {
                if self.can_fit(w, h, x, y, ignore) {
                    return Some((x, y));
                }
            }
        }
        None
    }

    /// Pose l'item `idx` à `(x,y)` (l'appelant a vérifié `can_fit`).
    pub fn place(&mut self, idx: usize, w: u32, h: u32, x: usize, y: usize) {
        let (w, h) = (w as usize, h as usize);
        for cy in y..y + h {
            for cx in x..x + w {
                self.occupied[cy * self.w + cx] = Some(idx);
            }
        }
    }

    /// Libère toutes les cellules de l'item `idx`.
    pub fn remove(&mut self, idx: usize) {
        for c in self.occupied.iter_mut() {
            if *c == Some(idx) {
                *c = None;
            }
        }
    }
}

/// Les 8 slots d'équipement (port od2 `character_equipment.go`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum EquipSlot {
    /// Tête (helm/circlet/pelt/barbare).
    Head,
    /// Torse (armure).
    Torso,
    /// Jambes = bottes (D2 réel `Legs`).
    Legs,
    /// Bras droit = gants (convention od2, écart documenté vs gants D2 uniques).
    RightArm,
    /// Bras gauche (réserve od2, non assignée par type — restant F6).
    LeftArm,
    /// Main gauche (arme).
    LeftHand,
    /// Main droite (arme).
    RightHand,
    /// Bouclier.
    Shield,
}

/// Nombre de slots d'équipement.
pub const EQUIP_SLOTS: usize = 8;

/// Équipement porté : 8 slots ([`None`] = vide).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Equipment {
    /// Slots par [`EquipSlot::index`].
    pub slots: [Option<Item>; EQUIP_SLOTS],
}

impl EquipSlot {
    /// Index dans [`Equipment::slots`] (ordre de l'enum, 0..8).
    pub fn index(self) -> usize {
        self as usize
    }

    /// Ce slot accepte-t-il un item de type `wtype` ? (mapping type → slot
    /// D2 réel ; `bloc`/`amul`/`ring`/`belt` hors MVP — restant F6.)
    pub fn accepts(self, wtype: &str) -> bool {
        match self {
            EquipSlot::Head => matches!(wtype, "helm" | "circ" | "pelt" | "phlm"),
            EquipSlot::Torso => matches!(wtype, "tors" | "body"),
            EquipSlot::Legs => wtype == "boot",
            EquipSlot::RightArm => wtype == "glov",
            EquipSlot::LeftArm => false,
            EquipSlot::LeftHand | EquipSlot::RightHand => {
                crate::loot::is_weapon_type(wtype)
            }
            EquipSlot::Shield => matches!(wtype, "shie" | "ashd"),
        }
    }
}

impl Equipment {
    /// Vide.
    pub fn new() -> Equipment {
        Equipment { slots: std::array::from_fn(|_| None) }
    }

    /// Premier slot acceptant ce type (armes : main droite d'abord — ordre
    /// fixe, déterministe) ; `None` si aucun.
    pub fn slot_for_type(wtype: &str) -> Option<EquipSlot> {
        const ORDER: [EquipSlot; EQUIP_SLOTS] = [
            EquipSlot::Head,
            EquipSlot::Torso,
            EquipSlot::Legs,
            EquipSlot::RightArm,
            EquipSlot::LeftArm,
            EquipSlot::RightHand,
            EquipSlot::LeftHand,
            EquipSlot::Shield,
        ];
        ORDER.into_iter().find(|s| s.accepts(wtype))
    }

    /// Équipe l'item (le type doit correspondre, slot vide requis) ;
    /// retourne l'ancien item du slot (échange) ou `None`.
    pub fn equip(&mut self, item: Item) -> Result<Option<Item>, String> {
        let Some(slot) = Self::slot_for_type(item_slot_type(&item)) else {
            return Err(format!("type non équipable : {}", item.code));
        };
        let old = self.slots[slot.index()].replace(item);
        Ok(old)
    }
}

/// Type effectif pour l'équipement : la base connaît son `wtype`
/// (rempli par [`crate::loot`] depuis les tables v1.12).
fn item_slot_type(item: &Item) -> &str {
    &item.base.wtype
}

/// Ceinture 4×4 (4 colonnes = touches 1-4, 4 rangées) — potions uniquement.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Belt {
    /// `[colonne][rangée]` ; colonne 0 = touche 1.
    pub columns: [[Option<Item>; 4]; 4],
}

impl Default for Belt {
    fn default() -> Self {
        Self::new()
    }
}

impl Belt {
    /// Vide.
    pub fn new() -> Belt {
        Belt { columns: Default::default() }
    }

    /// Place une potion : colonne de la touche, première rangée libre ;
    /// `None` si ceinture pleine (colonnes saturées) ou item sans code.
    pub fn store(&mut self, item: Item) -> Option<(usize, usize)> {
        if item.code.is_empty() {
            return None;
        }
        let (col, row) = self.first_free()?;
        self.columns[col][row] = Some(item);
        Some((col, row))
    }

    /// Première cellule libre (balayage RANGÉE puis colonne, ordre fixe) —
    /// comportement ceinture D2 : les 4 touches se remplissent d'abord (une
    /// potion par colonne, rangée 0), puis l'empilement monte colonne par
    /// colonne. Un remplissage colonne-d'abord entasserait les 4 premières
    /// potions sur la touche 1.
    pub fn first_free(&self) -> Option<(usize, usize)> {
        for r in 0..4 {
            for c in 0..4 {
                if self.columns[c][r].is_none() {
                    return Some((c, r));
                }
            }
        }
        None
    }

    /// Prend la potion d'une touche (colonne 0-3, bas de pile D2 = rangée 3
    /// consommée en premier ? D2 consomme la PILE la plus extérieure ;
    /// MVP : première non vide de la colonne, rangée 0 en premier — écart
    /// documenté).
    pub fn take_key(&mut self, key: usize) -> Option<Item> {
        let col = key.checked_sub(1)?;
        (col < 4).then_some(())?;
        for r in 0..4 {
            if let Some(it) = self.columns[col][r].take() {
                return Some(it);
            }
        }
        None
    }
}

/// Stats dérivées de l'équipement (AC/dégâts réels + affixes, DoD §5).
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DerivedStats {
    /// Défense totale (bases roulées + ac% + ac).
    pub defense: u32,
    /// Dégâts min cumulés (armes + dmg-min + dmg%).
    pub dmg_min: u32,
    /// Dégâts max cumulés.
    pub dmg_max: u32,
    /// Bonus force (str).
    pub strength: i32,
    /// Bonus dextérité (dex).
    pub dexterity: i32,
    /// Bonus vitalité (vit).
    pub vitality: i32,
    /// Bonus énergie (enr).
    pub energy: i32,
    /// Bonus vie (hp).
    pub life: i32,
    /// Bonus mana (mana).
    pub mana: i32,
    /// Résistances feu/froid/foudre/poison + all.
    pub res_fire: i32,
    pub res_cold: i32,
    pub res_ltng: i32,
    pub res_pois: i32,
    pub res_all: i32,
    /// Magic Find / Gold Find (mag%/gold%).
    pub magic_find: i32,
    pub gold_find: i32,
    /// Toucher (att/att%).
    pub attack_rating: i32,
}

/// Agrège l'équipement : AC de base ROLLÉ (déterministe via `rng`), dégâts
/// d'arme cumulés deux mains (MVP, écart documenté : D2 n'active qu'une
/// main), puis tous les bonus typés des props.
pub fn derive_equipped(eq: &Equipment, rng: &mut Rng) -> DerivedStats {
    let mut d = DerivedStats::default();
    let mut base_ac = 0u32;
    let mut base_min = 0u32;
    let mut base_max = 0u32;
    for slot in &eq.slots {
        let Some(item) = slot else { continue };
        // Bases réelles : AC roulé des armures, dégâts des armes.
        base_ac += item.rolled_ac(rng);
        if item.base.max_dam > 0 {
            base_min += item.base.min_dam;
            base_max += item.base.max_dam;
        }
        // Bonus simples (les % ac/dmg et les dégâts plats passent par les
        // cumuls ci-dessous : ils s'appliquent aux bases, convention D2).
        for p in &item.props {
            let Some(v) = p.typed else { continue };
            let value = p.value;
            match v {
                StatProp::AcPct(_) | StatProp::EdamPct(_) | StatProp::DmgMin(_)
                | StatProp::DmgMax(_) => {}
                _ => apply(&mut d, &v, value),
            }
        }
    }
    // Cumuls des pourcentages et plats (toutes pièces confondues).
    let mut ac_pct = 0i32;
    let mut edam_pct = 0i32;
    let mut flat_min = 0i32;
    let mut flat_max = 0i32;
    for slot in &eq.slots {
        let Some(item) = slot else { continue };
        ac_pct += item.prop_sum(prop_pick_acpct);
        edam_pct += item.prop_sum(prop_pick_edam);
        flat_min += item.prop_sum(prop_pick_dmgmin);
        flat_max += item.prop_sum(prop_pick_dmgmax);
    }
    // Défense = bases roulées × (1 + ac%/100) [ac plat déjà compté].
    d.defense += scale_u32(base_ac, ac_pct);
    // Dégâts = (bases + plats) × (1 + dmg%/100) — le plat s'ajoute avant le
    // % (ordre D2 : min/max de base, puis bonus plats, puis ED).
    d.dmg_min = scale_u32(base_min.saturating_add(flat_min.max(0) as u32), edam_pct);
    d.dmg_max = scale_u32(base_max.saturating_add(flat_max.max(0) as u32), edam_pct)
        .max(d.dmg_min);
    d
}

fn prop_pick_acpct(p: &StatProp) -> Option<i32> {
    if let StatProp::AcPct(v) = p { Some(*v) } else { None }
}
fn prop_pick_edam(p: &StatProp) -> Option<i32> {
    if let StatProp::EdamPct(v) = p { Some(*v) } else { None }
}
fn prop_pick_dmgmin(p: &StatProp) -> Option<i32> {
    if let StatProp::DmgMin(v) = p { Some(*v) } else { None }
}
fn prop_pick_dmgmax(p: &StatProp) -> Option<i32> {
    if let StatProp::DmgMax(v) = p { Some(*v) } else { None }
}

/// `base × (100 + pct)/100` arrondi vers le bas (D2 floor).
fn scale_u32(base: u32, pct: i32) -> u32 {
    ((base as i64 * (100 + pct as i64).max(0)) / 100) as u32
}

/// Applique un bonus typé simple aux stats dérivées.
fn apply(d: &mut DerivedStats, s: &StatProp, _value: i32) {
    match s {
        StatProp::Str(v) => d.strength += v,
        StatProp::Dex(v) => d.dexterity += v,
        StatProp::Vit(v) => d.vitality += v,
        StatProp::Enr(v) => d.energy += v,
        StatProp::Life(v) => d.life += v,
        StatProp::Mana(v) => d.mana += v,
        StatProp::ResFire(v) => d.res_fire += v,
        StatProp::ResCold(v) => d.res_cold += v,
        StatProp::ResLightning(v) => d.res_ltng += v,
        StatProp::ResPoison(v) => d.res_pois += v,
        StatProp::ResAll(v) => d.res_all += v,
        StatProp::MagicFind(v) => d.magic_find += v,
        StatProp::GoldFind(v) => d.gold_find += v,
        StatProp::AttackRating(v) => d.attack_rating += v,
        StatProp::AttackRatingPct(v) => d.attack_rating += v,
        // Défense plate (code `ac` des tables, ex. affixes « Sturdy » voisins
        // d'« ac% ») : ajoutée telle quelle, hors partie scalée ac%.
        StatProp::Ac(v) => d.defense = (d.defense as i64 + *v as i64).max(0) as u32,
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Tests — tailles réelles v1.12, canFit, équipement, ceinture, stats dérivées.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_props::{PropDescriptor, RolledProp};

    fn prop(code: &str, v: i32) -> RolledProp {
        RolledProp {
            desc: PropDescriptor { code: code.into(), param: String::new(), min: v, max: v },
            value: v,
            typed: crate::item_props::interpret(
                &PropDescriptor { code: code.into(), param: String::new(), min: v, max: v },
                v,
            ),
        }
    }

    fn item(code: &str, w: u32, h: u32) -> Item {
        Item { code: code.into(), w, h, ..Default::default() }
    }

    /// Grille 10×4 (inventory.txt) : placement, collision, hors-bords,
    /// échange via `ignore`, première place libre.
    #[test]
    fn grid_can_fit_behaviors() {
        let mut g = InventoryGrid::inventory();
        assert_eq!((g.w, g.h), (10, 4));
        assert!(g.can_fit(2, 3, 0, 0, None), "item 2×3 en (0,0)");
        g.place(0, 2, 3, 0, 0);
        assert!(!g.can_fit(1, 1, 1, 2, None), "cellule occupée");
        assert!(g.can_fit(2, 3, 0, 0, Some(0)), "son propre rectangle ignoré");
        assert!(!g.can_fit(3, 3, 9, 0, None), "hors bord X");
        assert!(!g.can_fit(3, 3, 0, 3, None), "hors bord Y");
        // Épée 1×4 ? (bâton 2×4) : il reste (2,0) → 8 colonnes libres.
        let spot = g.find_first_fit(2, 4, None).expect("2×4 après hax");
        assert_eq!(spot, (2, 0));
        g.place(1, 2, 4, spot.0, spot.1);
        assert!(g.find_first_fit(10, 4, None).is_none(), "plus de 10×4 libre");
    }

    /// Tailles portées par le modèle (`BaseInfo` de loot.rs porte les vraies
    /// tailles v1.12 — test croisé là-bas) ; ici le contrat de taille du
    /// modèle : tout Item connaît son X×Y et sa position de grille.
    #[test]
    fn item_carries_grid_size() {
        let mut it = item("hax", 1, 3);
        assert_eq!((it.w, it.h), (1, 3));
        it.grid = Some((0, 0));
        assert_eq!(it.grid, Some((0, 0)));
    }

    /// Équipement : mapping type → slot (helm→Head, boot→Legs, glov→RA,
    /// shie→Shield, arme→RightHand d'abord), échange retourne l'ancien.
    #[test]
    fn equipment_slots_and_swap() {
        let mut eq = Equipment::new();
        let mut cap = item("cap", 2, 2);
        cap.base.name = "Cap".into();
        let slot = Equipment::slot_for_type("helm").unwrap();
        assert_eq!(slot, EquipSlot::Head);
        eq.slots[EquipSlot::Head.index()] = Some(cap);
        // bouclier
        assert_eq!(Equipment::slot_for_type("shie"), Some(EquipSlot::Shield));
        // arme : RightHand d'abord
        assert_eq!(Equipment::slot_for_type("axe"), Some(EquipSlot::RightHand));
        let mut axe = item("hax", 1, 3);
        axe.base = BaseInfo {
            name: "Hand Axe".into(),
            wtype: "axe".into(),
            min_dam: 3,
            max_dam: 6,
            ..Default::default()
        };
        assert!(eq.equip(axe.clone()).is_ok());
        assert_eq!(eq.slots[EquipSlot::RightHand.index()].as_ref().unwrap().code, "hax");
        // ceinture (géar) non assignable au MVP
        assert_eq!(Equipment::slot_for_type("belt"), None);
        // amulette/anneau : restant F6
        assert_eq!(Equipment::slot_for_type("amul"), None);
        assert_eq!(Equipment::slot_for_type("ring"), None);
    }

    /// Ceinture 4×4 : 16 potions max, touches par colonne, rejet non-potion
    /// impossible (caller-enforced via kind) et pile par colonne.
    #[test]
    fn belt_four_by_four() {
        let mut b = Belt::new();
        for i in 0..16 {
            let mut p = item("hp1", 1, 1);
            p.base.name = format!("pot{i}");
            assert!(b.store(p).is_some(), "place {i}");
        }
        let mut extra = item("hp1", 1, 1);
        extra.base.name = "overflow".into();
        assert!(b.store(extra).is_none(), "ceinture pleine");
        let t0 = b.take_key(1).expect("touche 1");
        assert_eq!(t0.base.name, "pot0");
        let t1 = b.take_key(1).expect("touche 1 re-remplie");
        assert_eq!(t1.base.name, "pot4");
        assert!(b.take_key(4).is_some());
        assert!(b.take_key(5).is_none(), "touche 5 inexistante");
    }

    /// Stats dérivées : AC roulé de la base réelle × ac% + ac plat ;
    /// dégâts d'arme × dmg% + dmg-min/max plats (descripteurs).
    #[test]
    fn derived_stats_ac_and_damage() {
        let mut buckler = item("buc", 2, 2);
        buckler.base = BaseInfo {
            name: "Buckler".into(),
            wtype: "shie".into(),
            min_ac: 4,
            max_ac: 6,
            ..Default::default()
        };
        buckler.props = vec![prop("ac%", 30), prop("ac", 5)];
        let mut axe = item("hax", 1, 3);
        axe.base = BaseInfo {
            name: "Hand Axe".into(),
            wtype: "axe".into(),
            min_dam: 3,
            max_dam: 6,
            ..Default::default()
        };
        axe.props = vec![prop("dmg%", 50), prop("dmg-min", 2)];
        let mut eq = Equipment::new();
        eq.slots[EquipSlot::LeftHand.index()] = Some(axe);
        eq.slots[EquipSlot::Shield.index()] = Some(buckler);
        let mut rng = Rng::new(11);
        let d1 = derive_equipped(&eq, &mut rng);
        // AC : roll(4..6) ×1,30 (floor) + 5 plat → 10..12
        assert!((10..=12).contains(&d1.defense), "AC {}", d1.defense);
        // Dégâts : (base + plat) × (1 + dmg%/100) — min (3+2)×1,5 = 7,
        // max (6+0)×1,5 = 9.
        assert_eq!(d1.dmg_min, 7, "min = (3+2)×1,5");
        assert_eq!(d1.dmg_max, 9, "max = 6×1,5");
        // Déterminisme au seed.
        let mut rng2 = Rng::new(11);
        let d2 = derive_equipped(&eq, &mut rng2);
        assert_eq!(d1, d2);
    }

    /// Bonus typés agrégés : str/dex/res/MF depuis les props roulées.
    #[test]
    fn derived_stats_aggregate_typed_bonuses() {
        let mut ring_like = item("rin", 1, 1);
        ring_like.props = vec![
            prop("str", 5),
            prop("dex", 3),
            prop("res-all", 10),
            prop("mag%", 15),
        ];
        let mut eq = Equipment::new();
        eq.slots[EquipSlot::RightHand.index()] = Some(ring_like);
        let mut rng = Rng::new(3);
        let d = derive_equipped(&eq, &mut rng);
        assert_eq!(d.strength, 5);
        assert_eq!(d.dexterity, 3);
        assert_eq!(d.res_all, 10);
        assert_eq!(d.magic_find, 15);
    }
}
