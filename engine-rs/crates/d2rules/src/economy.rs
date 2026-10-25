//! `economy` — ÉCONOMIE & INVENTAIRE v1.12 (étape 11c : or, stash 6×4,
//! durabilité, vente/achat, auto-pickup).
//!
//! Port des règles D2 réelles, PUR (zéro IO, zéro wasm) et additif sur
//! l'arbre : les données viennent des mêmes tables embarquées que
//! [`crate::vendor`] (patch_weapons/armor/misc.txt : `cost`, `durability`),
//! la géométrie du stash de `inventory.txt` (page « Bank Page » 6×4, brief
//! 11c) et la grille partagée [`crate::inventory::InventoryGrid`] (canFit).
//!
//! # OR (sources : Basin Wiki « Gold », wiki fan officieux — le site
//! officiel Arreat Summit est hors ligne ; règles v1.10+ vérifiées D2R)
//! * **Plafond porté** = `10 000 × niveau` (formule D2 réelle) :
//!   [`carry_cap`] ; tout gain au-delà est refusé ([`gain_carry`] renvoie
//!   la part `refused` qui reste au sol).
//! * **Or du stash** (séparé de la bourse) : dépôt/retrait via
//!   [`Stash::deposit_gold`]/[`Stash::withdraw_gold`] ; plafond du stash
//!   [`stash_gold_cap`] = **2 500 000** sur les trois difficultés (étape 19b,
//!   interop .d2s : le README de l'oracle d2s-file-generator accepte
//!   « stashed gold 0..2 500 000 » et le vrai .d2s javazon du corpus d2d2s
//!   porte `goldbank = 2 500 000` — fait de format ; remplace la table
//!   fan-wiki 300k/500k/800k, non officielle).
//! * **Achat** = débit RÉEL de la bourse + réserve du vendeur crédité
//!   ([`purchase`], miroir pur de `TradeSession::buy` de `vendor.rs`).
//!
//! # STASH 6×4
//! [`Stash`] porte l'or + une grille [`InventoryGrid`] 6×4 partagée
//! (mêmes `can_fit`/`find_first_fit` que l'inventaire) + les items
//! [`EquipItem`] (riches : tailles `invwidth/invheight` réelles,
//! durabilité). Dépôt/retrait par première place libre (balayage fixe,
//! déterministe). Sérialisable : champ de save additif `gold_stash`
//! (`#[serde(default)]`, save v3 inchangée — branché dans `save.rs`).
//!
//! # DURABILITÉ (tables réelles : colonnes `durability`/`nodurability`)
//! * Arme : −1 par coup PORTÉ ([`wear`], constantes [`WEAR_PER_HIT`]/
//!   [`WEAR_PER_BLOW`] = 1) ; armure portée : −1 par coup REÇU.
//!   Simplification documentée : le retail use une pièce aléatoire par
//!   coup (Basin « Durability », wiki fan) — ici chaque pièce portée vieillit
//!   d'1 point par coup concerné.
//! * Brisé (`durability == 0`) = **non utilisable, plus de dégâts**
//!   ([`weapon_damage`] renvoie (0,0), [`item_effective_damage`] idem 2H).
//! * Réparation chez le forgeron (Charsi) : [`repair_cost`] =
//!   `floor(cost × (max − cur) / max)` — formule linéaire ASSUMÉE et
//!   documentée (le brief 11c le permet) : le facteur exact du retail
//!   n'est pas public (wikis non fiables : Arreat Summit « Repair » /
//!   fandom). Réparable via [`repair`] (garde sur l'or du héros).
//! * Valeurs réelles : [`durability_of`] parse les colonnes des 3 tables
//!   (armes col `durability`, armures col `durability`, misc : 0 —
//!   `nodurability`).
//!
//! # VENTE & AUTO-PICKUP
//! * Vente = `cost / 4` plancher ([`sell_price`]) — formule officielle D2
//!   (marchands payent 25 % de la valeur ; Basin Wiki « Shops », wiki fan
//!   cité ; même marge que `vendor.rs::sell_price`).
//! * Auto-pickup : l'or au sol dans la zone de ramassage
//!   (`PICKUP_RANGE`, 2 tuiles) est ramassé au passage (en mouvement),
//!   tant que la bourse est sous le plafond — [`auto_pickup`] (pur) et
//!   `world_auto_pickup` (branché en fin d'`update_hero`, monde réel).
//!
//! # Branchement moteur (contrat documenté, comme `vendor.rs`)
//! * Or cap + auto-pickup : branchés dans `world.rs` (Pickup + tick).
//! * Stash : état `World::gold_stash` + save additive `gold_stash` ;
//!   API [`World::stash`] / [`World::stash_deposit_gold`] /
//!   [`World::stash_withdraw_gold`] ; panneau démo `demo/stash-ui.js`.
//! * Durabilité : règles pures prêtes ; le portage World (usure au coup)
//!   attend l'intégration de l'équipement 7c (`equipment.rs` n'est pas
//!   branché sur le World — cf. choix J1 dans `corpse.rs`) : l'usure
//!   s'appliquera sur les [`EquipItem`] portés au branchement.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::difficulty::Difficulty;
use crate::equipment::EquipItem;
use crate::inventory::InventoryGrid;
use crate::loot::tsv_rows;
use crate::world::{dist, PICKUP_RANGE, World};
use crate::{GameEvent, ItemKind};

// ---------------------------------------------------------------------------
// OR — plafonds (formule D2 réelle : 10 000 × niveau, Basin « Gold »)
// ---------------------------------------------------------------------------

/// Or porté max PAR NIVEAU de personnage (D2 réel : `10 000 × clvl`).
pub const GOLD_PER_LEVEL: u32 = 10_000;

/// Plafond d'or PORTÉ au niveau `level` (formule D2 : 10 000 × clvl).
pub fn carry_cap(level: u32) -> u32 {
    level.saturating_mul(GOLD_PER_LEVEL)
}

/// Part de gain acceptée / refusée par le plafond de bourse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GoldGain {
    /// Or effectivement gagné (crédité).
    pub gained: u32,
    /// Or refusé (reste au sol) — bourse au plafond.
    pub refused: u32,
}

/// Crédite `amount` sur la bourse DANS LA LIMITE du plafond 10 000 × niveau.
pub fn gain_carry(carried: u32, level: u32, amount: u32) -> GoldGain {
    let room = carry_cap(level).saturating_sub(carried);
    let gained = amount.min(room);
    GoldGain { gained, refused: amount - gained }
}

/// Résultat d'un balayage d'auto-pickup : total gagné + piles restantes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoPickup {
    /// Total crédité (somme des gains).
    pub gained: u32,
    /// Piles restant au sol (même ordre ; tronquées si bourse pleine).
    pub piles: Vec<u32>,
}

/// Ramassage automatique de piles d'or (ordre fixe) sous plafond : chaque
/// pile est prise dans la limite de la place restante, le reliquat reste.
pub fn auto_pickup(carried: u32, level: u32, piles: &[u32]) -> AutoPickup {
    let mut gold = carried;
    let mut gained = 0;
    let mut left = Vec::with_capacity(piles.len());
    for &p in piles {
        let g = gain_carry(gold, level, p);
        gold += g.gained;
        gained += g.gained;
        left.push(g.refused);
    }
    AutoPickup { gained, piles: left }
}

/// Plafond d'or du STASH — **2 500 000** sur les trois difficultés (étape
/// 19b, interop .d2s) : fait de FORMAT confirmé deux fois (README de
/// l'oracle d2s-file-generator « stashed gold 0..2 500 000 » ; vrai .d2s
/// javazon du corpus d2d2s : `goldbank = 2 500 000`). Remplace la table
/// fan-wiki 300k/500k/800k (source non officielle) — le paramètre
/// `diff` reste dans la signature pour l'API existante (UI/game) mais ne
/// différencie plus les difficultés.
pub const STASH_GOLD_CAP: u32 = 2_500_000;

/// Plafond d'or du STASH (constante unique [`STASH_GOLD_CAP`] — étape 19b).
pub fn stash_gold_cap(_diff: Difficulty) -> u32 {
    STASH_GOLD_CAP
}

// ---------------------------------------------------------------------------
// ACHAT / VENTE (marges officielles D2)
// ---------------------------------------------------------------------------

/// Diviseur de vente D2 : le marchand paye 1/4 de la valeur (plancher).
pub const SELL_DIVISOR: u32 = 4;

/// Prix de VENTE officiel D2 : `cost / 4` (plancher) — Basin Wiki « Shops »
/// (« merchants pay 25 % », wiki fan cité) ; même marge que `vendor.rs`.
pub fn sell_price(cost: u32) -> u32 {
    cost / SELL_DIVISOR
}

/// Résultat d'un achat : bourse débitée, réserve vendeur créditée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Purchase {
    /// Nouvelle bourse du héros.
    pub hero_gold: u32,
    /// Nouvelle réserve du vendeur.
    pub vendor_gold: u32,
}

/// Erreurs d'économie (API pure — jamais d'effet de bord partiel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EconError {
    /// Or insuffisant (besoin, possédé).
    NotEnoughGold { need: u32, have: u32 },
    /// Retrait refusé : la bourse est au plafond 10 000 × niveau.
    CarryGoldFull { cap: u32 },
    /// Dépôt refusé : le stash est à son plafond de difficulté.
    StashGoldFull { cap: u32 },
    /// Grille 6×4 pleine (aucune place pour cet item).
    StashFull,
    /// L'item ne tient JAMAIS dans la page (plus large/haute que 6×4).
    ItemTooBig { w: u32, h: u32 },
    /// Index de stash invalide (slot vide ou hors bornes).
    BadSlot(usize),
    /// Rien à réparer (pièce pleine ou indestructible).
    NothingToRepair,
    /// 17c : pièce ÉTHÉRÉE — JAMAIS réparable (D2 réel ; les recettes
    /// cubemain de réparation L139-140 exigent `noe`).
    EtherealNotRepairable,
}

/// ACHAT D2 : débit RÉEL de la bourse + réserve du vendeur créditée
/// (miroir pur de `TradeSession::buy` — `vendor.rs`). Rien ne bouge en
/// cas de refus.
pub fn purchase(price: u32, hero_gold: u32, vendor_gold: u32) -> Result<Purchase, EconError> {
    if hero_gold < price {
        return Err(EconError::NotEnoughGold { need: price, have: hero_gold });
    }
    Ok(Purchase { hero_gold: hero_gold - price, vendor_gold: vendor_gold.saturating_add(price) })
}

// ---------------------------------------------------------------------------
// DURABILITÉ (tables réelles : patch_weapons/armor/misc.txt)
// ---------------------------------------------------------------------------

/// Usure de l'arme par coup PORTÉ (D2 réel : 1 point/coup — Basin
/// « Durability », wiki fan ; simplification multi-pièces documentée).
pub const WEAR_PER_HIT: u32 = 1;
/// Usure d'une pièce portée par coup REÇU (simplification documentée).
pub const WEAR_PER_BLOW: u32 = 1;

/// Colonnes des tables réelles v1.12 (0-based) — `durability` :
/// weapons.txt (code 3, dur 25, nodur 26), armor.txt (code 17, dur 11,
/// nodur 12), misc.txt (code 13, pas de colonne durabilité ⇒ 0).
const WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
const ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");

/// Table code → durabilité max (parse une fois, `OnceLock` std — zéro IO).
fn durability_table() -> &'static HashMap<String, u32> {
    static TABLE: OnceLock<HashMap<String, u32>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut out: HashMap<String, u32> = HashMap::new();
        let mut load = |txt: &str, code_c: usize, dur_c: Option<usize>, nodur_c: Option<usize>| {
            let rows = tsv_rows(txt);
            for r in rows.iter().skip(1) {
                let cell = |i: usize| r.get(i).map(|s| s.trim()).unwrap_or("");
                let code = cell(code_c);
                if code.is_empty() || out.contains_key(code) {
                    continue; // ordre weapons → armor → misc, premier gagne
                }
                let nodur = nodur_c.map(|i| cell(i) == "1").unwrap_or(false);
                let dur = if nodur { 0 } else { dur_c.and_then(|i| cell(i).parse().ok()).unwrap_or(0) };
                out.insert(code.to_string(), dur);
            }
        };
        load(WEAPONS_TXT, 3, Some(25), Some(26));
        load(ARMOR_TXT, 17, Some(11), Some(12));
        load(MISC_TXT, 13, None, Some(10));
        out
    })
}

/// Durabilité MAX réelle d'un code (`durability` des tables ; 0 = aucune —
/// misc/potions, indestructible ou code inconnu).
pub fn durability_of(code: &str) -> u32 {
    durability_table().get(code).copied().unwrap_or(0)
}

/// Usure : `cur − amount` saturé à 0 (0 = brisé).
pub fn wear(cur: u32, amount: u32) -> u32 {
    cur.saturating_sub(amount)
}

/// Une pièce à 0 de durabilité est BRISÉE (non utilisable).
pub fn is_broken(cur: u32) -> bool {
    cur == 0
}

/// Dégâts effectifs d'une arme : (0, 0) si brisée — « pas de dégâts »
/// (brief 11c) ; sinon la paire demandée.
pub fn weapon_damage(cur: u32, min: u32, max: u32) -> (u32, u32) {
    if is_broken(cur) { (0, 0) } else { (min, max) }
}

/// Coût de réparation : `floor(cost × (max − cur) / max)` — formule
/// linéaire ASSUMÉE et documentée (en-tête du module : wikis non fiables).
/// Pièce pleine ou indestructible (`max == 0`) ⇒ 0.
pub fn repair_cost(cost: u32, cur: u32, max: u32) -> u32 {
    if max == 0 || cur >= max {
        return 0;
    }
    (cost as u64 * (max - cur) as u64 / max as u64) as u32
}

/// Résultat d'une réparation : coût payé + durabilité restaurée.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Repair {
    /// Coût débité.
    pub cost: u32,
    /// Durabilité après réparation (= max).
    pub durability: u32,
}

/// Répare (cur → max) si l'or du héros suffit ; rien ne bouge sinon.
pub fn repair(cost: u32, cur: u32, max: u32, hero_gold: u32) -> Result<Repair, EconError> {
    let price = repair_cost(cost, cur, max);
    if price == 0 {
        return Err(EconError::NothingToRepair);
    }
    if hero_gold < price {
        return Err(EconError::NotEnoughGold { need: price, have: hero_gold });
    }
    Ok(Repair { cost: price, durability: max })
}

/// 17c — refus du forgeron pour une pièce ÉTHÉRÉE : JAMAIS réparable
/// (D2 réel ; les recettes cubemain de réparation L139-140 exigent `noe`,
/// cohérent). Message fr prêt pour l'UI ; la RECHARGE des charges reste
/// autorisée (`item_special::recharge`, sans garde).
pub const MSG_ETHEREAL_NOT_REPAIRABLE: &str =
    "Cet objet éthéré ne peut pas être réparé.";

/// Garde de réparation : `Err(EtherealNotRepairable)` si `ethereal`,
/// `Ok(())` sinon (chaîner ensuite avec [`repair`]).
pub fn repair_ethereal_guard(ethereal: bool) -> Result<(), EconError> {
    if ethereal {
        Err(EconError::EtherealNotRepairable)
    } else {
        Ok(())
    }
}

// --- Aides sur les pièces portées ([`EquipItem`], branchement 7c futur) ----

/// Coup PORTÉ par une arme : −1 de durabilité (brisée = reste à 0).
pub fn take_hit(item: &mut EquipItem) {
    if item.max_durability > 0 {
        item.durability = wear(item.durability, WEAR_PER_HIT);
    }
}

/// Coup REÇU par une pièce portée (armure/bouclier…) : −1 de durabilité.
pub fn take_blow(item: &mut EquipItem) {
    if item.max_durability > 0 {
        item.durability = wear(item.durability, WEAR_PER_BLOW);
    }
}

/// Pièce brisée ? (0 de durabilité, ou indestructible = jamais brisée).
pub fn item_broken(item: &EquipItem) -> bool {
    item.max_durability > 0 && is_broken(item.durability)
}

/// Dégâts effectifs d'une arme portée (mode 2 mains respecté) : (0,0) si
/// brisée — « item brisé = non utilisable (pas de dégâts) », brief 11c.
pub fn item_effective_damage(item: &EquipItem) -> (u32, u32) {
    if item_broken(item) {
        return (0, 0);
    }
    if item.two_handed {
        (item.twoh_min_dam, item.twoh_max_dam)
    } else {
        (item.min_dam, item.max_dam)
    }
}

/// Coût de réparation d'une pièce chez le forgeron (`cost` = valeur réelle
/// de la base, tables v1.12 — [`crate::vendor::VendorCatalog::cost_of`]).
pub fn item_repair_cost(cost: u32, item: &EquipItem) -> u32 {
    repair_cost(cost, item.durability, item.max_durability)
}

// ---------------------------------------------------------------------------
// STASH 6×4 (inventory.txt « Bank Page » — grille partagée InventoryGrid)
// ---------------------------------------------------------------------------

/// Largeur de la page de stash (`inventory.txt` Bank Page, brief 11c).
pub const STASH_W: usize = 6;
/// Hauteur de la page de stash.
pub const STASH_H: usize = 4;

/// Stash du héros : or séparé de la bourse + grille 6×4 d'items riches.
/// Sérialisable (save additive `gold_stash`, champ `#[serde(default)]`).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Stash {
    /// Or DÉPOSÉ (séparé de l'or porté — `HeroStats.gold`).
    pub gold: u32,
    /// Grille partagée 6×4 (`inventory.rs::InventoryGrid` — canFit).
    pub grid: InventoryGrid,
    /// Items déposés (index stables : `None` = slot retiré) — la grille
    /// porte ces mêmes indices dans `occupied`.
    pub items: Vec<Option<EquipItem>>,
}

impl Default for Stash {
    fn default() -> Stash {
        Stash::new()
    }
}

impl Stash {
    /// Stash vide (grille 6×4 neuve).
    pub fn new() -> Stash {
        Stash { gold: 0, grid: InventoryGrid::new(STASH_W, STASH_H), items: Vec::new() }
    }

    /// DÉPÔT d'or : `amount` quitte la bourse (garde `hero_gold`), entre
    /// dans le stash DANS LA LIMITE du plafond de difficulté. Renvoie les
    /// nouveaux soldes (bourse, stash) ; rien ne bouge en cas d'erreur.
    pub fn deposit_gold(
        &mut self,
        amount: u32,
        hero_gold: u32,
        _level: u32,
        diff: Difficulty,
    ) -> Result<(u32, u32), EconError> {
        if hero_gold < amount {
            return Err(EconError::NotEnoughGold { need: amount, have: hero_gold });
        }
        let cap = stash_gold_cap(diff);
        let new_stash = self.gold + amount; // u32 : cap 2.5M, pas d'overflow
        if new_stash > cap {
            return Err(EconError::StashGoldFull { cap });
        }
        self.gold = new_stash;
        Ok((hero_gold - amount, new_stash))
    }

    /// RETRAIT d'or : `amount` quitte le stash (garde), rejoint la bourse
    /// DANS LA LIMITE du plafond porté 10 000 × niveau. Rien ne bouge sinon.
    pub fn withdraw_gold(
        &mut self,
        amount: u32,
        hero_gold: u32,
        level: u32,
    ) -> Result<(u32, u32), EconError> {
        if self.gold < amount {
            return Err(EconError::NotEnoughGold { need: amount, have: self.gold });
        }
        let cap = carry_cap(level);
        if hero_gold + amount > cap {
            return Err(EconError::CarryGoldFull { cap });
        }
        self.gold -= amount;
        Ok((hero_gold + amount, self.gold))
    }

    /// Cet item tient-il quelque part dans la page 6×4 ? (grille partagée)
    pub fn can_fit(&self, item: &EquipItem) -> bool {
        item.inv_w > 0
            && item.inv_h > 0
            && item.inv_w as usize <= STASH_W
            && item.inv_h as usize <= STASH_H
            && self.grid.find_first_fit(item.inv_w, item.inv_h, None).is_some()
    }

    /// DÉPÔT d'item : première place libre (balayage fixe) ; renvoie
    /// l'index du slot. Rien ne bouge en cas d'erreur.
    pub fn deposit_item(&mut self, item: EquipItem) -> Result<usize, EconError> {
        if item.inv_w == 0
            || item.inv_h == 0
            || item.inv_w as usize > STASH_W
            || item.inv_h as usize > STASH_H
        {
            return Err(EconError::ItemTooBig { w: item.inv_w, h: item.inv_h });
        }
        let Some((x, y)) = self.grid.find_first_fit(item.inv_w, item.inv_h, None) else {
            return Err(EconError::StashFull);
        };
        let idx = self.items.len();
        self.grid.place(idx, item.inv_w, item.inv_h, x, y);
        self.items.push(Some(item));
        Ok(idx)
    }

    /// RETRAIT d'item : libère la grille, renvoie la pièce. Le slot reste
    /// marqué `None` (indices stables — la grille les référence).
    pub fn withdraw_item(&mut self, index: usize) -> Result<EquipItem, EconError> {
        let slot = self.items.get_mut(index).ok_or(EconError::BadSlot(index))?;
        let item = slot.take().ok_or(EconError::BadSlot(index))?;
        self.grid.remove(index);
        Ok(item)
    }

    /// Cellules libres restantes (diagnostic/tests/UI).
    pub fn free_cells(&self) -> usize {
        self.grid.occupied.iter().filter(|c| c.is_none()).count()
    }
}

// ---------------------------------------------------------------------------
// Branchement World (contrat 11c) — bloc additif, pattern corpse.rs
// ---------------------------------------------------------------------------

impl World {
    /// Stash (lecture — UI/panneau démo).
    pub fn stash(&self) -> &Stash {
        &self.gold_stash
    }

    /// Stash (écriture — intégrateur/UI).
    pub fn stash_mut(&mut self) -> &mut Stash {
        &mut self.gold_stash
    }

    /// Dépose `amount` d'or porté dans le stash (plafond de difficulté).
    /// Renvoie l'or stash après dépôt ; rien ne bouge en cas d'erreur.
    pub fn stash_deposit_gold(&mut self, amount: u32) -> Result<u32, EconError> {
        let (hero, stash) =
            self.gold_stash.deposit_gold(amount, self.hero.stats.gold, self.hero.stats.level, self.difficulty)?;
        self.hero.stats.gold = hero;
        Ok(stash)
    }

    /// Retire `amount` d'or du stash vers la bourse (plafond 10 000 ×
    /// niveau). Renvoie l'or porté après retrait ; rien ne bouge sinon.
    pub fn stash_withdraw_gold(&mut self, amount: u32) -> Result<u32, EconError> {
        let (hero, stash) =
            self.gold_stash.withdraw_gold(amount, self.hero.stats.gold, self.hero.stats.level)?;
        self.hero.stats.gold = hero;
        Ok(stash)
    }
}

/// Auto-pickup MONDE (fin d'`update_hero`, héros en mouvement) : piles
/// d'or dans [`PICKUP_RANGE`] ramassées dans l'ordre du sol, sous plafond ;
/// le reliquat reste au sol (bourse pleine). Déterministe (ordre d'index).
pub(crate) fn world_auto_pickup(w: &mut World, ev: &mut Vec<GameEvent>) {
    let (hx, hy) = (w.hero_ent.x, w.hero_ent.y);
    let mut kept: Vec<crate::ItemOnGround> = Vec::with_capacity(w.items.len());
    for mut it in w.items.drain(..) {
        if it.def.kind == ItemKind::Gold && dist(it.x - hx, it.y - hy) <= PICKUP_RANGE {
            let g = gain_carry(w.hero.stats.gold, w.hero.stats.level, it.def.amount);
            if g.gained > 0 {
                w.hero.stats.gold += g.gained;
                ev.push(GameEvent::GoldGained { amount: g.gained });
            }
            it.def.amount = g.refused;
            if g.refused > 0 {
                kept.push(it); // bourse pleine : le reliquat reste au sol
            }
            continue;
        }
        kept.push(it);
    }
    w.items = kept;
}

// ---------------------------------------------------------------------------
// Tests — goldens tables réelles v1.12, tout déterministe.
// ---------------------------------------------------------------------------
#[cfg(test)]
mod tests {
    use super::*;
    use crate::vendor::VendorCatalog;
    use crate::ItemOnGround;

    /// Manifest réel sans monstres (pattern corpse.rs) — monde de test.
    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    fn mk_world() -> World {
        let mut m: serde_json::Value = serde_json::from_str(MANIFEST_JSON).unwrap();
        m["monsters"] = serde_json::json!([]);
        World::with_seed(open_grid(24, 24), m, 0x0E11_C0DE)
    }

    fn equip(code: &str, w: u32, h: u32, max_dur: u32) -> EquipItem {
        EquipItem {
            code: code.into(),
            name: code.into(),
            wtype: "tors".into(),
            inv_w: w,
            inv_h: h,
            max_durability: max_dur,
            durability: max_dur,
            ..Default::default()
        }
    }

    // -- OR : plafonds -------------------------------------------------------

    /// Formule D2 réelle : cap porté = 10 000 × niveau (golden 1/7/30) ;
    /// gain partiel sous plafond (refusé = reliquat au sol) ; gain plein
    /// si la place suffit.
    #[test]
    fn gold_cap_carry_10k_per_level() {
        assert_eq!(carry_cap(1), 10_000);
        assert_eq!(carry_cap(7), 70_000);
        assert_eq!(carry_cap(30), 300_000);
        // 7 000 + 5 000 au niveau 1 : 3 000 gagnés, 2 000 refusés (au sol).
        let g = gain_carry(7_000, 1, 5_000);
        assert_eq!(g, GoldGain { gained: 3_000, refused: 2_000 });
        // Bourse pleine : tout est refusé.
        assert_eq!(gain_carry(10_000, 1, 1), GoldGain { gained: 0, refused: 1 });
        // Place suffisante : tout passe.
        assert_eq!(gain_carry(0, 2, 20_000), GoldGain { gained: 20_000, refused: 0 });
    }

    /// Auto-pickup pur : piles dans l'ordre fixe, reliquats tronqués au
    /// plafond, `gained` = total crédité.
    #[test]
    fn auto_pickup_zone_respects_cap() {
        // carried 7 000, cap 10 000 : pile 5 000 → 3 000 (2 000 restent),
        // pile 8 000 → 0 (intacte).
        let ap = auto_pickup(7_000, 1, &[5_000, 8_000]);
        assert_eq!(ap.gained, 3_000);
        assert_eq!(ap.piles, vec![2_000, 8_000]);
        // Aucune pile : rien.
        assert_eq!(auto_pickup(1, 1, &[]), AutoPickup { gained: 0, piles: vec![] });
    }

    /// Stash or : dépôt/retrait, plafond interop UNIQUE 2 500 000 (étape
    /// 19b — fait de format .d2s) et plafond porté au retrait — erreurs
    /// exactes, rien ne bouge.
    #[test]
    fn stash_gold_deposit_withdraw_caps() {
        assert_eq!(STASH_GOLD_CAP, 2_500_000);
        assert_eq!(stash_gold_cap(Difficulty::Normal), 2_500_000);
        assert_eq!(stash_gold_cap(Difficulty::Nightmare), 2_500_000);
        assert_eq!(stash_gold_cap(Difficulty::Hell), 2_500_000);

        let mut s = Stash::new();
        // Dépôt simple : bourse −100, stash +100.
        assert_eq!(s.deposit_gold(100, 1_000, 1, Difficulty::Normal).unwrap(), (900, 100));
        // Pas assez porté : refus, état intact.
        assert_eq!(
            s.deposit_gold(5_000, 900, 1, Difficulty::Normal),
            Err(EconError::NotEnoughGold { need: 5_000, have: 900 })
        );
        assert_eq!(s.gold, 100);
        // Plafond 2 500 000 atteint : refus StashGoldFull.
        s.gold = 2_499_000;
        assert_eq!(
            s.deposit_gold(2_000, 3_000_000, 1, Difficulty::Normal),
            Err(EconError::StashGoldFull { cap: 2_500_000 })
        );
        assert_eq!(s.deposit_gold(1_000, 3_000_000, 1, Difficulty::Hell).unwrap(), (2_999_000, 2_500_000));
        // Retrait au-delà du cap porté (niveau 1 = 10 000) : refus.
        assert_eq!(
            s.withdraw_gold(20_000, 0, 1),
            Err(EconError::CarryGoldFull { cap: 10_000 })
        );
        // Cap porté à haut niveau aussi (30 = 300 000), stash large.
        assert_eq!(
            s.withdraw_gold(400_000, 0, 30),
            Err(EconError::CarryGoldFull { cap: 300_000 })
        );
        // Retrait au-delà du stash : refus (cap porté niveau 90 = 900 000).
        assert_eq!(
            s.withdraw_gold(2_600_000, 0, 90),
            Err(EconError::NotEnoughGold { need: 2_600_000, have: 2_500_000 })
        );
        // Retrait normal : stash −50 000, bourse +50 000 (cap porté 60).
        assert_eq!(s.withdraw_gold(50_000, 0, 60).unwrap(), (50_000, 2_450_000));
    }

    // -- VENTE / ACHAT (marges officielles, objets réels) --------------------

    /// Vente officielle = cost/4 plancher, sur des objets RÉELS des tables
    /// (misc hp1 30 ⇒ 7, cap 64 ⇒ 16, stu 2385 ⇒ 596) ; achat = débit
    /// réel + réserve vendeur créditée, refus si bourse insuffisante.
    #[test]
    fn sell_and_purchase_official_margins_real_items() {
        let cat = VendorCatalog::build().expect("tables v1.12 valides");
        assert_eq!(cat.cost_of("hp1"), Some(30));
        assert_eq!(sell_price(30), 7, "hp1 : 30/4 plancher");
        assert_eq!(cat.cost_of("cap"), Some(64));
        assert_eq!(sell_price(64), 16, "cap : 64/4");
        assert_eq!(cat.cost_of("stu"), Some(2385));
        assert_eq!(sell_price(2385), 596, "Studded Leather : 2385/4 plancher");
        // Achat : débit réel + réserve vendeur (hp1 = 30).
        let p = purchase(30, 100, 500).unwrap();
        assert_eq!(p, Purchase { hero_gold: 70, vendor_gold: 530 });
        assert_eq!(
            purchase(30, 29, 500),
            Err(EconError::NotEnoughGold { need: 30, have: 29 })
        );
    }

    // -- DURABILITÉ (valeurs réelles des tables) ------------------------------

    /// Golden tables : hax 28, cap 12, stu 32, hp1 0 (misc sans
    /// durabilité) ; usure → bris → plus de dégâts ; réparation formule.
    #[test]
    fn durability_real_tables_wear_break_repair() {
        assert_eq!(durability_of("hax"), 28, "Hand Axe : col durability weapons.txt");
        assert_eq!(durability_of("cap"), 12, "Cap : col durability armor.txt");
        assert_eq!(durability_of("stu"), 32, "Studded Leather : armor.txt");
        assert_eq!(durability_of("hp1"), 0, "potions : pas de durabilité");
        assert_eq!(durability_of("zzz"), 0, "code inconnu : 0");

        // Arme réelle : 28 → 26 (2 coups portés) → brisée à 28 coups.
        let mut axe = equip("hax", 1, 3, durability_of("hax"));
        take_hit(&mut axe);
        take_hit(&mut axe);
        assert_eq!(axe.durability, 26);
        assert!(!item_broken(&axe));
        for _ in 0..26 {
            take_hit(&mut axe);
        }
        assert_eq!(axe.durability, 0);
        assert!(item_broken(&axe), "28 usures ⇒ brisée");
        assert_eq!(item_effective_damage(&axe), (0, 0), "brisée = pas de dégâts");

        // Arme saine : dégâts de base ; 2 mains : paire 2H ; brisée 2H : (0,0).
        let mut sword = equip("2hs", 2, 3, durability_of("2hs"));
        sword.min_dam = 2;
        sword.max_dam = 7;
        sword.twoh_min_dam = 3;
        sword.twoh_max_dam = 10;
        sword.two_handed = true;
        assert_eq!(item_effective_damage(&sword), (3, 10));
        take_blow(&mut sword); // un coup reçu use aussi (pièce portée)
        assert_eq!(sword.durability, durability_of("2hs") - 1);
        sword.durability = 0;
        assert_eq!(item_effective_damage(&sword), (0, 0), "2 mains brisée = (0,0)");

        // Indestructible (max 0) : l'usure ne s'applique jamais.
        let mut ghost = equip("nodur", 1, 1, 0);
        take_hit(&mut ghost);
        take_blow(&mut ghost);
        assert!(!item_broken(&ghost));

        // Réparation : cap (cost 64, max 12) à 3/12 ⇒ 64 × 9/12 = 48 ;
        // payée : durabilité pleine, or débité ; pleine ⇒ NothingToRepair ;
        // or insuffisant ⇒ refus (rien ne bouge).
        assert_eq!(repair_cost(64, 3, 12), 48);
        assert_eq!(repair_cost(64, 12, 12), 0);
        assert_eq!(repair_cost(64, 15, 12), 0, "surchargée : rien à faire");
        assert_eq!(repair_cost(64, 3, 0), 0, "indestructible");
        let r = repair(64, 3, 12, 100).unwrap();
        assert_eq!(r, Repair { cost: 48, durability: 12 });
        assert_eq!(repair(64, 12, 12, 100), Err(EconError::NothingToRepair));
        assert_eq!(repair(64, 3, 12, 47), Err(EconError::NotEnoughGold { need: 48, have: 47 }));
    }

    // -- 17c : ÉTHÉRÉ non réparable (garde additive) -------------------------

    /// Garde 17c : l'éthéré est REFUSÉ chez le forgeron (message fr prêt),
    /// une pièce normale passe et se répare comme avant.
    #[test]
    fn ethereal_never_repairable_guard() {
        assert_eq!(repair_ethereal_guard(true), Err(EconError::EtherealNotRepairable));
        assert!(!MSG_ETHEREAL_NOT_REPAIRABLE.is_empty());
        assert_eq!(repair_ethereal_guard(false), Ok(()));
        // Chaînage : garde OK ⇒ réparation normale inchangée.
        let guarded = repair_ethereal_guard(false).and_then(|_| repair(64, 3, 12, 100));
        assert_eq!(guarded.unwrap().durability, 12);
    }

    // -- STASH 6×4 : grille partagée canFit ----------------------------------

    /// Géométrie 6×4 (Bank Page) : dépôt première place libre, collision,
    /// item trop grand, stash plein, retrait libère la place (canFit).
    #[test]
    fn stash_grid_can_fit_deposit_withdraw() {
        let mut s = Stash::new();
        assert_eq!((s.grid.w, s.grid.h), (6, 4), "Bank Page 6×4");
        assert_eq!(s.free_cells(), 24);

        // Item 2×3 en (0,0) ; un 2×3 passe à côté (2,0) mais pas en (1,0).
        let a = equip("cap", 2, 3, 12);
        assert!(s.can_fit(&a));
        let ia = s.deposit_item(a).unwrap();
        assert_eq!(ia, 0);
        let b = equip("stu", 2, 3, 32);
        assert!(s.can_fit(&b));
        assert_eq!(s.deposit_item(b.clone()).unwrap(), 1);
        // Grille : 2 items 2×3 côte à côte rangée 0..3 ; plus de 3×3 d'un bloc.
        assert!(!s.can_fit(&equip("big", 3, 3, 10)), "plus de place pour un 3×3");
        // Item plus large que la page : jamais.
        assert_eq!(
            s.deposit_item(equip("huge", 7, 1, 0)),
            Err(EconError::ItemTooBig { w: 7, h: 1 })
        );
        // Retrait de l'item 0 : sa zone (0,0)-(1,2) se libère (canFit 2×3
        // à nouveau) ; un 3×3 reste bloqué par l'item 1 (colonnes 2-3).
        let back = s.withdraw_item(0).unwrap();
        assert_eq!(back.code, "cap");
        assert!(s.can_fit(&equip("cap2", 2, 3, 12)), "retrait libère la zone (0,0)");
        assert!(!s.can_fit(&equip("big", 3, 3, 10)), "3×3 encore bloqué par stu");
        let back_b = s.withdraw_item(1).unwrap();
        assert_eq!(back_b.code, "stu");
        assert!(s.can_fit(&equip("big", 3, 3, 10)), "deux retraits ⇒ bloc 3×3 libre");
        // Slot déjà vide / hors bornes : BadSlot.
        assert_eq!(s.withdraw_item(0), Err(EconError::BadSlot(0)));
        assert_eq!(s.withdraw_item(99), Err(EconError::BadSlot(99)));
        // Remplissage : page vide ⇒ 24 items 1×1, le 25e non.
        for _ in 0..24 {
            assert!(s.deposit_item(equip("rin", 1, 1, 0)).is_ok());
        }
        assert_eq!(s.free_cells(), 0, "page pleine");
        assert_eq!(s.deposit_item(equip("rin", 1, 1, 0)), Err(EconError::StashFull));
    }

    // -- Branchement World : stash + save additive + auto-pickup --------------

    /// World : dépôt/retrait d'or via l'API World (plafonds respectés) ;
    /// le stash traverse la save (champ additif `gold_stash`) et une save
    /// v3 SANS champ charge à un stash vide (rétro-compat).
    #[test]
    fn world_stash_save_roundtrip_and_retro_compat() {
        let mut w = mk_world();
        w.hero.stats.gold = 5_000;
        assert_eq!(w.stash_deposit_gold(1_234).unwrap(), 1_234, "stash après dépôt");
        assert_eq!(w.hero.stats.gold, 3_766, "bourse débitée réellement");
        // Un item riche dans le stash.
        let idx = w.stash_mut().deposit_item(equip("hax", 1, 3, 28)).unwrap();
        assert_eq!(idx, 0);

        let json = w.save();
        let back = World::load(&json).expect("save valide");
        assert_eq!(back.stash().gold, 1_234, "or du stash persisté");
        assert_eq!(back.stash().items.len(), 1);
        assert_eq!(back.hero_stats().gold, 3_766);
        let mut reloaded = back;
        assert_eq!(reloaded.stash_mut().withdraw_item(0).unwrap().code, "hax");

        // Rétro-compat : save sans `gold_stash` ⇒ stash vide 6×4.
        let mut v: serde_json::Value = serde_json::from_str(&json).unwrap();
        v.as_object_mut().unwrap().remove("gold_stash");
        let old = World::load(&v.to_string()).expect("save v3 sans stash");
        assert_eq!(old.stash(), &Stash::new());

        // API retrait : plafond porté (niveau 1 = 10 000).
        let mut w2 = mk_world();
        w2.hero.stats.gold = 9_900;
        w2.stash_mut().gold = 500;
        assert_eq!(w2.stash_withdraw_gold(500), Err(EconError::CarryGoldFull { cap: 10_000 }));
        assert_eq!(w2.stash_withdraw_gold(50).unwrap(), 450, "stash après retrait");
        assert_eq!(w2.hero_stats().gold, 9_950);
    }

    /// Pickup monde : l'or respecte le plafond porté (le reliquat reste au
    /// sol) ; l'auto-pickup ne se déclenche qu'en MOUVEMENT (immobile : rien).
    #[test]
    fn world_pickup_caps_and_auto_pickup_needs_movement() {
        let mut w = mk_world();
        w.hero.stats.level = 1;
        w.hero.stats.gold = 9_900;
        let (hx, hy) = (w.hero_ent.x, w.hero_ent.y);
        w.items.push(ItemOnGround {
            def: crate::ItemDef::gold(500),
            x: hx + 1.0,
            y: hy,
        });
        // Immobile (ticks sans input) : PAS d'auto-pickup.
        for _ in 0..10 {
            w.tick(50, None);
        }
        assert_eq!(w.hero_stats().gold, 9_900, "immobile : l'or reste au sol");
        assert_eq!(w.items().len(), 1);

        // Pickup explicite : cap niveau 1 = 10 000 ⇒ 100 gagnés, 400 au sol.
        let ev = w.tick(50, Some(crate::PlayerInput::Pickup));
        assert!(ev.iter().any(|e| matches!(e, GameEvent::GoldGained { amount: 100 })));
        assert_eq!(w.hero_stats().gold, 10_000);
        assert_eq!(w.items().len(), 1, "le reliquat reste au sol");
        assert_eq!(w.items()[0].def.amount, 400);

        // Auto-pickup en mouvement : marche sur la pile restante — mais la
        // bourse est pleine : rien n'est pris (l'or attend la place).
        let far = (hx + 3.0, hy);
        let ev = w.tick(0, Some(crate::PlayerInput::MoveTo {
            x: far.0.floor() as i32,
            y: far.1.floor() as i32,
        }));
        let _ = ev;
        for _ in 0..40 {
            w.tick(50, None);
        }
        assert_eq!(w.hero_stats().gold, 10_000, "bourse pleine : l'or reste au sol");
        assert_eq!(w.items().len(), 1, "pile intacte (400)");
    }

    /// Auto-pickup heureux : héros sous le cap, marche AVEC de l'or en zone
    /// ⇒ GoldGained émis par le tick, pile retirée du sol.
    #[test]
    fn world_auto_pickup_gains_while_walking_under_cap() {
        let mut w = mk_world();
        w.hero.stats.level = 1;
        let (hx, hy) = (w.hero_ent.x, w.hero_ent.y);
        w.items.push(ItemOnGround { def: crate::ItemDef::gold(250), x: hx + 2.0, y: hy });
        let ev = w.tick(0, Some(crate::PlayerInput::MoveTo {
            x: hx as i32 + 4,
            y: hy as i32,
        }));
        let _ = ev;
        let mut gained = 0;
        for _ in 0..40 {
            for e in w.tick(50, None) {
                if let GameEvent::GoldGained { amount } = e {
                    gained += amount;
                }
            }
        }
        assert_eq!(gained, 250, "l'or de la zone est ramassé en marchant");
        assert_eq!(w.hero_stats().gold, 250);
        assert!(w.items().is_empty(), "pile retirée du sol");
    }
}
