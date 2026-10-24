//! `consumables` — CONSOMMABLES COMPLETS v1.12 (étape 11b, campagne
//! diablo-2-wasm) : potions, tomes/parchemins, identification, ceinture 4×4.
//!
//! # Sources réelles (lecture seule)
//! * `assets/patch_misc.txt` (148 lignes, v1.12) — PARSÉ EN L'ÉTAT au
//!   premier accès (`include_str!`, même modèle que `loot.rs`/`vendor.rs`) :
//!   aucun doublon de données, la table embarquée EST le fichier réel. Les
//!   golden tests verrouillent les valeurs de contrôle par famille.
//! * `assets/patch_missiles.txt` (684 ids, 0..=683 vérifiés) : AUCUNE ligne
//!   missile « Town Portal » n'existe en v1.12 (le parchemin déclenche
//!   l'OBJET portail, pas un projectile) — le port réutilise donc le
//!   switch de niveau existant de d2game (wire documenté ci-dessous).
//! * `assets/patch_itemstatcost.txt` — ids des stats de buffs (golden test).
//! * TBL fr réelles (`tbl.rs::bundled_fra`) — noms affichables : la clé TBL
//!   d'un consommable est SON CODE (`hp1` → « Petite potion de santé »,
//!   `ibk` → « Livre d'identification »).
//!
//! # Portée réelle (codes traités)
//! * soins `hp1`-`hp5` : +30/+60/+100/+180/+320 pv (colonne `calc1` des
//!   lignes `hpregen` — « valeurs heal du misc » ; le retail draine sur
//!   `len` frames ≈ 8 s, le port applique instantané, écart documenté) ;
//! * mana `mp1`-`mp5` : +20/+40/+80/+150/+250 (colonne `calc1`
//!   `manarecovery`, même écart instantané) ;
//! * rejuv `rvs`/`rvl` : 35 %/100 % de la vie ET du mana MAX (colonnes
//!   `hitpoints`/`mana`, instantané au retail aussi) ;
//! * buffs temporisés via le système d'états existant (durée = `len` × 40 ms
//!   comme `shrines::D2_FRAME_MS`, 750 f = 30 s) : antidote `yps`
//!   (+50 poisonresist, +10 maxpoisonresist), dégel `wms` (+50 coldresist,
//!   +10 maxcoldresist), endurance `vps` (+5000 staminarecoverybonus) ;
//! * porte de ville `tsc` + tome `tbk` (20 charges) : effet `town_portal` —
//!   WIRE MINIMAL : le consommable porte le flag, c'est d2game qui appelle
//!   son `switch_level("town", entrée du camp)` EXISTANT (aucune nouvelle
//!   mécanique de portail) ;
//! * identification `isc` + tome `ibk` (20 charges) : règles PURES
//!   [`identify`]/[`identify_all`] sur le modèle riche `inventory::Item` —
//!   le flag « non identifié au drop » existe déjà dans `loot.rs::finish`
//!   (magic/rare/set/unique). HOOK DOCUMENTÉ : l'inventaire runtime du
//!   `World` porte des `ItemDef` (sans qualité), donc rien n'est
//!   « non identifié » en partie aujourd'hui ; le branchement isc/Cain se
//!   fera avec la migration inventaire riche (F6/F7) — les règles et les
//!   golden tests sont prêts. « Cain identifie tout » =
//!   [`identify_all`] (pur) ;
//! * clé `key` : référence mission — `useable=0` réellement, refus
//!   explicite ([`ConsumError::NotUsable`]).
//!
//! # Ceinture 4×4 (« touches 1-4 déjà là ? »)
//! Vérifié : AUCUN handler 1-4 côté hud/wire (`demo/game.js` ne câble que
//! Maj/E/I/P). Le modèle `inventory::Belt` existait sans usage runtime.
//! Ce module porte l'état additif [`ConsumState`] (ceinture `ItemDef` 4×4 +
//! buffs actifs) dans le `World` :
//! * [`World::belt_auto_store`] — « auto-ranger » : déplace les potions
//!   `autobelt=1` réelles (hp1-5, mp1-5, rvs, rvl — vérifié colonne
//!   `autobelt` ; `yps`/`wms`/`vps`/parchemins = 0) de l'inventaire vers la
//!   ceinture (balayage rangée puis colonne, ordre fixe) ;
//! * [`World::use_belt_key`] — touches 1..=4 (colonne 0 = touche 1, rangée
//!   0 consommée en premier) ;
//! * [`World::use_consumable`] — usage direct d'un slot d'inventaire
//!   (effets par code ci-dessus, tomes = charges `amount`).
//!
//! Les APIs sont ADDITIVES : le chemin historique `PlayerInput::UseItem`
//! (soin MVP 60) reste strictement inchangé — les deux coexistent, l'UI
//! branche la nouvelle (wire d2game `consumables_ui.rs`).
//!
//! Zéro IO, zéro RNG, zéro panique : table lenient (illisible ⇒ pool vide,
//! usages en erreur typée), tout est déterministe.

use crate::statlist::StatLine;
use crate::tsv::Tsv;
use crate::{GameEvent, ItemDef, ItemKind, World};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::OnceLock;

/// `assets/patch_misc.txt` v1.12 réel (148 lignes) — parse runtime, la table
/// embarquée EST le fichier (zéro copie, zéro dérive possible).
const MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");

/// 40 ms par frame D2 (25 ips) — même conversion des durées gameplay que
/// `shrines` (`len` de misc.txt : 750 f = 30 s de buff).
pub const D2_FRAME_MS: u64 = crate::shrines::D2_FRAME_MS;

/// Charges d'un tome NEUF (`tbk`/`ibk`). Réel : `maxstack=20` (et la mission
/// « tome = 20 parchemins ») ; l'écart `spawnstack=5` du drop MVP n'est pas
/// porté (les tomes entrent à 20 charges via [`item_def`]).
pub const BOOK_CHARGES: u32 = 20;

/// Ids `ItemStatCost.txt` v1.12 des stats de buffs — ABSENTS des constantes
/// `statlist` (vérifiés sur `assets/patch_itemstatcost.txt`, golden test).
/// `maxcoldresist` (id 44).
pub const MAXCOLDRESIST: i32 = 44;
/// `maxpoisonresist` (id 46).
pub const MAXPOISONRESIST: i32 = 46;
/// `staminarecoverybonus` (id 28).
pub const STAMINARECOVERYBONUS: i32 = 28;

/// Famille d'effet d'un consommable (dérivée des colonnes réelles
/// `stat1`/`state`/code — cf. [`ConsumableDef`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConsumableKind {
    /// Soins (`hp1`-`hp5`, stat1 = `hpregen`).
    Healing,
    /// Mana (`mp1`-`mp5`, stat1 = `manarecovery`).
    Mana,
    /// Rajeunissement (`rvs`/`rvl`, stat1/2 = `hitpoints`/`mana`).
    Rejuvenation,
    /// Buff temporisé (`yps`/`wms`/`vps`, colonne `state`).
    Buff(BuffState),
    /// Porte de ville (`tsc`/`tbk`) — flag pour le wire d2game.
    TownPortal,
    /// Identification (`isc`/`ibk`).
    Identify,
    /// Consommable suivi sans effet porté (sentinelle — aucun aujourd'hui).
    Other,
}

/// États temporisés de la colonne `state` de misc.txt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BuffState {
    /// `staminapot` — potion d'endurance (`vps`).
    Stamina,
    /// `antidote` — antidote (`yps`).
    Antidote,
    /// `thawing` — potion de dégel (`wms`).
    Thawing,
}

/// Un consommable réel (une ligne de `patch_misc.txt`, colonnes consommées).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumableDef {
    /// Code table (`hp1`, `rvl`, `tbk`…).
    pub code: String,
    /// Nom anglais (colonne `name`).
    pub name: String,
    /// Famille d'effet.
    pub kind: ConsumableKind,
    /// Puissance : pv (hp*) ou mana (mp*) — colonne `calc1` de `stat1`.
    pub power: u32,
    /// % de vie/mana max (`rvs`=35, `rvl`=100) — colonne `calc1` `hitpoints`.
    pub rejuv_pct: u32,
    /// Durée du buff en FRAMES (`len`, 0 = instantané).
    pub duration_frames: u32,
    /// Stats du buff `(id ItemStatCost, valeur)` — vides sinon.
    pub buff_stats: Vec<(i32, i64)>,
    /// Colonne `useable` (1 = buvable/utilisable).
    pub usable: bool,
    /// Colonne `belt` (1 = peut poser en ceinture).
    pub belt: bool,
    /// Colonne `autobelt` (1 = auto-ranger en ceinture au ramassage).
    pub autobelt: bool,
    /// Colonne `stackable` (1 = empilable : tomes/clés).
    pub stackable: bool,
    /// Colonne `maxstack` (20 pour les tomes, 12 pour la clé).
    pub maxstack: u32,
    /// Colonne `cost` (prix vendeur).
    pub cost: u32,
}

impl ConsumableDef {
    /// Tome de parchemins (`tbk`/`ibk`) ? — charges portées par
    /// [`ItemDef::amount`] (les parchemins unitaires ne sont pas empilables).
    pub fn is_book(&self) -> bool {
        (self.kind == ConsumableKind::TownPortal || self.kind == ConsumableKind::Identify)
            && self.stackable
    }

    /// Durée du buff en ms (frames × 40 — 0 si instantané).
    pub fn duration_ms(&self) -> u64 {
        self.duration_frames as u64 * D2_FRAME_MS
    }
}

/// Table parsée du fichier réel, une fois (lenient : illisible ⇒ vide).
fn table() -> &'static HashMap<String, ConsumableDef> {
    static T: OnceLock<HashMap<String, ConsumableDef>> = OnceLock::new();
    T.get_or_init(|| {
        let mut out = HashMap::new();
        let Ok(t) = Tsv::parse(MISC_TXT) else { return out };
        for i in 0..t.len() {
            let Some(r) = t.rec(i) else { continue };
            let code = r.s("code").trim().to_ascii_lowercase();
            if !is_tracked(&code) {
                continue;
            }
            let stat1 = r.s("stat1").trim().to_ascii_lowercase();
            let state = r.s("state").trim().to_ascii_lowercase();
            // Famille brute depuis les colonnes d'effet, puis les codes
            // parchemins/tomes (sans colonnes d'effet) par leur code réel.
            let mut kind = match stat1.as_str() {
                "hpregen" => ConsumableKind::Healing,
                "manarecovery" => ConsumableKind::Mana,
                "hitpoints" => ConsumableKind::Rejuvenation,
                _ => match state.as_str() {
                    "antidote" => ConsumableKind::Buff(BuffState::Antidote),
                    "thawing" => ConsumableKind::Buff(BuffState::Thawing),
                    "staminapot" => ConsumableKind::Buff(BuffState::Stamina),
                    _ => ConsumableKind::Other,
                },
            };
            let (power, rejuv_pct) = match kind {
                ConsumableKind::Healing | ConsumableKind::Mana => (r.u("calc1"), 0),
                ConsumableKind::Rejuvenation => (0, r.u("calc1")),
                _ => (0, 0),
            };
            kind = match code.as_str() {
                "tsc" | "tbk" => ConsumableKind::TownPortal,
                "isc" | "ibk" => ConsumableKind::Identify,
                _ => kind,
            };
            // Stats du buff : (stat1, calc1) puis (stat2, calc2) si présent.
            let mut buff_stats = Vec::new();
            if let ConsumableKind::Buff(_) = kind {
                match stat1.as_str() {
                    "poisonresist" => buff_stats.push((crate::statlist::POISONRESIST, r.i("calc1") as i64)),
                    "coldresist" => buff_stats.push((crate::statlist::COLDRESIST, r.i("calc1") as i64)),
                    "staminarecoverybonus" => {
                        buff_stats.push((STAMINARECOVERYBONUS, r.i("calc1") as i64))
                    }
                    _ => {}
                }
                match r.s("stat2").trim().to_ascii_lowercase().as_str() {
                    "maxpoisonresist" => buff_stats.push((MAXPOISONRESIST, r.i("calc2") as i64)),
                    "maxcoldresist" => buff_stats.push((MAXCOLDRESIST, r.i("calc2") as i64)),
                    _ => {}
                }
            }
            out.insert(
                code.clone(),
                ConsumableDef {
                    name: r.s("name").trim().to_string(),
                    kind,
                    power,
                    rejuv_pct,
                    duration_frames: r.u("len"),
                    buff_stats,
                    usable: r.s("useable").trim() == "1",
                    belt: r.s("belt").trim() == "1",
                    autobelt: r.s("autobelt").trim() == "1",
                    stackable: r.s("stackable").trim() == "1",
                    maxstack: r.u("maxstack"),
                    cost: r.u("cost"),
                    code,
                },
            );
        }
        out
    })
}

/// Codes traités par le module (portée mission 11b) — le parse ignore tout
/// le reste de misc.txt (gemmes, runes, quest items…).
fn is_tracked(code: &str) -> bool {
    matches!(
        code,
        "hp1" | "hp2" | "hp3" | "hp4" | "hp5"
            | "mp1" | "mp2" | "mp3" | "mp4" | "mp5"
            | "rvs" | "rvl" | "vps" | "yps" | "wms"
            | "tsc" | "isc" | "tbk" | "ibk" | "key"
    )
}

/// Consommable par code (`None` = non suivi).
pub fn by_code(code: &str) -> Option<&'static ConsumableDef> {
    table().get(code.trim().to_ascii_lowercase().as_str())
}

/// Nom affichable fr depuis les TBL réelles (clé = code) — repli : nom
/// anglais de la table, `None` si le code n'est ni dans les TBL ni dans
/// misc.txt. Vérifié réel : `hp1`→« Petite potion de santé », `ibk`→« Livre
/// d'identification », `key`→« Clé » (la clé du tome d'identification v1.12
/// est `ibk`, pas `isb` — la mission citait le code historique).
pub fn display_name(code: &str) -> Option<String> {
    match crate::tbl::TblSet::bundled_fra().get(code) {
        Some(n) => Some(n),
        None => by_code(code).map(|d| d.name.clone()),
    }
}

/// `ItemDef` d'un consommable : nom fr (TBL), [`ItemKind::Potion`] pour les
/// potions (hp/mp/rejuv/buffs — cohérent avec le contrat legacy), `Misc`
/// pour parchemins/tomes/clé. `amount` = charges du tome
/// ([`BOOK_CHARGES`]), 0 sinon.
pub fn item_def(code: &str) -> Option<ItemDef> {
    let def = by_code(code)?;
    let kind = match def.kind {
        ConsumableKind::Healing
        | ConsumableKind::Mana
        | ConsumableKind::Rejuvenation
        | ConsumableKind::Buff(_) => ItemKind::Potion,
        _ => ItemKind::Misc,
    };
    Some(ItemDef {
        code: def.code.clone(),
        name: display_name(&def.code).unwrap_or_else(|| def.name.clone()),
        kind,
        amount: if def.is_book() { BOOK_CHARGES } else { 0 },
    })
}

/// Lecture du stock d'un tome (`tbk`/`ibk`) — champ `amount` de l'objet
/// (0 = tome vide/inconnu ; les tomes construits par [`item_def`] naissent à
/// [`BOOK_CHARGES`]).
pub fn book_charges(def: &ItemDef) -> u32 {
    match by_code(&def.code) {
        Some(c) if c.is_book() => def.amount,
        _ => 0,
    }
}

// ---------------------------------------------------------------------------
// Identification (règles pures — modèle riche `inventory::Item`)
// ---------------------------------------------------------------------------

/// Cet item a-t-il BESOIN d'identification ? Règle D2 : tout ce qui porte
/// des propriétés cachées (magic/rare/set/unique) drop NON identifié
/// (`loot.rs::finish` pose déjà `identified=false`) ; normal/supérieur/
/// fissuré n'en ont jamais besoin.
pub fn needs_identification(item: &crate::inventory::Item) -> bool {
    !item.identified
        && matches!(
            item.quality,
            crate::item_quality::Quality::Magic
                | crate::item_quality::Quality::Rare
                | crate::item_quality::Quality::Set
                | crate::item_quality::Quality::Unique
        )
}

/// Identifie UN item — `true` si l'item a changé d'état (idempotent : un
/// item déjà identifié ou sans besoin renvoie `false`, sans erreur).
pub fn identify(item: &mut crate::inventory::Item) -> bool {
    if needs_identification(item) {
        item.identified = true;
        true
    } else {
        false
    }
}

/// Cain identifie TOUT : le nombre d'items identifiés par le passage
/// (déterministe, ordre du tableau).
pub fn identify_all(items: &mut [crate::inventory::Item]) -> usize {
    let mut n = 0;
    for it in items.iter_mut() {
        if identify(it) {
            n += 1;
        }
    }
    n
}

// ---------------------------------------------------------------------------
// État additif du World : ceinture 4×4 + buffs temporisés
// ---------------------------------------------------------------------------

/// État consommables du héros (additif, sauvegardé `#[serde(default)]`) :
/// la ceinture 4×4 (colonnes = touches 1-4) et les buffs temporisés actifs.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ConsumState {
    /// `[colonne][rangée]` — colonne 0 = touche 1, rangée 0 consommée en
    /// premier (même convention que `inventory::Belt`).
    pub belt: [[Option<ItemDef>; 4]; 4],
    /// Buffs temporisés actifs (max 3 : antidote/dégel/endurance coexistent).
    pub buffs: Vec<ConsumableBuff>,
}

impl ConsumState {
    /// Cellules libres restantes (16 max).
    pub fn free_slots(&self) -> usize {
        self.belt.iter().flatten().filter(|c| c.is_none()).count()
    }

    /// Buff actif d'un état donné (référence — HUD).
    pub fn buff(&self, state: BuffState) -> Option<&ConsumableBuff> {
        self.buffs.iter().find(|b| b.state == state)
    }
}

/// Un buff temporisé (même contrat que `shrines::ShrineBuff`, stats issues
/// des colonnes `stat/calc` de misc.txt au lieu d'Arg0/Arg1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsumableBuff {
    /// État (colonne `state` de misc.txt).
    pub state: BuffState,
    /// Durée restante en ms (décomptée par le tick du `World`).
    pub remaining_ms: u64,
    /// Contributions `ItemStatCost` (ids réels : 43/44, 45/46, 28).
    pub stats: Vec<(i32, i64)>,
}

impl ConsumableBuff {
    /// Construit le buff d'une potion temporisée (`len` frames × 40 ms).
    /// `None` si la définition n'est pas un buff temporisé.
    pub fn from_def(def: &ConsumableDef) -> Option<ConsumableBuff> {
        let ConsumableKind::Buff(state) = def.kind else { return None };
        if def.duration_frames == 0 {
            return None;
        }
        Some(ConsumableBuff {
            state,
            remaining_ms: def.duration_ms(),
            stats: def.buff_stats.clone(),
        })
    }

    /// Un pas de temps simulé ; `true` tant que le buff est actif.
    pub fn tick(&mut self, dt_ms: u32) -> bool {
        if self.remaining_ms > 0 {
            self.remaining_ms = self.remaining_ms.saturating_sub(dt_ms as u64);
        }
        self.remaining_ms > 0
    }

    /// Buff actif ?
    pub fn is_active(&self) -> bool {
        self.remaining_ms > 0
    }

    /// Contributions au socle `statlist` existant (même interface que les
    /// affixes et `ShrineBuff::stat_lines` : à inclure dans
    /// `StatList::from_sources` pour le combat — câblage au pas suivant).
    pub fn stat_lines(&self) -> Vec<StatLine> {
        self.stats.iter().map(|&(id, v)| StatLine::new(id, v)).collect()
    }
}

// ---------------------------------------------------------------------------
// Résultat / erreurs d'usage
// ---------------------------------------------------------------------------

/// Résultat d'un usage de consommable (contrat JSON UI).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConsumUse {
    /// Code consommé.
    pub code: String,
    /// Nom affiché fr (TBL).
    pub name: String,
    /// Famille (`healing`/`mana`/`rejuv`/`buff`/`town_portal`/`identify`).
    pub family: String,
    /// PV rendus (0 sinon).
    pub heal: u32,
    /// Mana rendu (0 sinon).
    pub mana: u32,
    /// Durée du buff appliqué en ms (0 sinon).
    pub buff_ms: u64,
    /// Porte de ville déclenchée (le wire d2game bascule vers le camp).
    pub town_portal: bool,
    /// Items identifiés (0 en MVP — hook inventaire riche, cf. module).
    pub identified: u32,
    /// Charges restantes du tome après usage (`None` = pas un tome).
    pub charges_left: Option<u32>,
}

/// Erreurs d'usage (affichables UI telles quelles).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsumError {
    /// Slot vide.
    SlotEmpty,
    /// Code non suivi par le module (ni potion ni parchemin/tome/clé).
    Unknown(String),
    /// Réellement non utilisable (colonne `useable=0` — ex. la `key`) ou
    /// effet hors périmètre.
    NotUsable(String),
    /// Parchemin/tome d'identification sans rien à identifier (hook
    /// inventaire riche à venir) — l'objet N'EST PAS consommé.
    NothingToIdentify,
    /// Tome à 0 charge.
    BookEmpty,
    /// Touche de ceinture hors 1..=4.
    BadBeltKey(usize),
}

impl std::fmt::Display for ConsumError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsumError::SlotEmpty => write!(f, "slot vide"),
            ConsumError::Unknown(c) => write!(f, "objet inconnu : {c}"),
            ConsumError::NotUsable(c) => write!(f, "objet non utilisable : {c}"),
            ConsumError::NothingToIdentify => {
                write!(f, "rien à identifier (inventaire riche à venir)")
            }
            ConsumError::BookEmpty => write!(f, "tome vide"),
            ConsumError::BadBeltKey(k) => write!(f, "touche de ceinture invalide : {k}"),
        }
    }
}

/// Applique l'EFFET d'un consommable sur le monde (vie/mana/buff/événement)
/// SANS consommer l'objet — les points d'entrée gèrent la consommation
/// (slot vidé, charges de tomes). Déterministe, sans RNG.
fn apply(w: &mut World, def: &ItemDef) -> Result<ConsumUse, ConsumError> {
    let Some(c) = by_code(&def.code) else {
        return Err(ConsumError::Unknown(def.code.clone()));
    };
    if !c.usable {
        return Err(ConsumError::NotUsable(def.code.clone()));
    }
    let name = display_name(&def.code).unwrap_or_else(|| def.name.clone());
    let mut use1 = ConsumUse {
        code: def.code.clone(),
        name,
        family: String::new(),
        heal: 0,
        mana: 0,
        buff_ms: 0,
        town_portal: false,
        identified: 0,
        charges_left: None,
    };
    match c.kind {
        ConsumableKind::Healing => {
            let s = &mut w.hero.stats;
            use1.heal = c.power.min(s.max_life.saturating_sub(s.life));
            s.life += use1.heal;
            use1.family = "healing".into();
            // Même événement que le chemin legacy (HUD/sons de la page).
            w.pending_events.push(GameEvent::PotionUsed { heal: use1.heal });
        }
        ConsumableKind::Mana => {
            let s = &mut w.hero.stats;
            use1.mana = c.power.min(s.max_mana.saturating_sub(s.mana));
            s.mana += use1.mana;
            use1.family = "mana".into();
        }
        ConsumableKind::Rejuvenation => {
            // floor(max × pct / 100), plafonné au manquant (D2 floor).
            let s = &mut w.hero.stats;
            let pct_life = (s.max_life as u64 * c.rejuv_pct as u64 / 100) as u32;
            let pct_mana = (s.max_mana as u64 * c.rejuv_pct as u64 / 100) as u32;
            use1.heal = pct_life.min(s.max_life.saturating_sub(s.life));
            use1.mana = pct_mana.min(s.max_mana.saturating_sub(s.mana));
            s.life += use1.heal;
            s.mana += use1.mana;
            use1.family = "rejuv".into();
            w.pending_events.push(GameEvent::PotionUsed { heal: use1.heal });
        }
        ConsumableKind::Buff(state) => {
            let Some(buff) = ConsumableBuff::from_def(c) else {
                return Err(ConsumError::NotUsable(def.code.clone()));
            };
            use1.buff_ms = buff.remaining_ms;
            // Re-boire RAFRAÎCHIT la durée (retail) ; les 3 états coexistent.
            let (remaining_ms, stats) = (buff.remaining_ms, buff.stats.clone());
            match w.consum.buffs.iter_mut().find(|b| b.state == state) {
                Some(b) => {
                    b.remaining_ms = remaining_ms;
                    b.stats = stats;
                }
                None => w.consum.buffs.push(buff),
            }
            use1.family = "buff".into();
        }
        ConsumableKind::TownPortal => {
            use1.town_portal = true;
            use1.family = "town_portal".into();
        }
        ConsumableKind::Identify => {
            // HOOK (documenté au module) : l'inventaire runtime est en
            // `ItemDef` sans qualité — rien n'est identifiable aujourd'hui.
            // L'objet n'est PAS consommé sans cible (pas de gaspillage).
            use1.family = "identify".into();
            return Err(ConsumError::NothingToIdentify);
        }
        ConsumableKind::Other => return Err(ConsumError::NotUsable(def.code.clone())),
    }
    Ok(use1)
}

/// Décrémente les charges d'un tome dans le slot `slot` après un usage
/// réussi ; `None` = pas un tome (l'appelant videra le slot). Retire
/// l'objet à 0 charge.
fn spend_book(w: &mut World, slot: usize, def: &ItemDef) -> Option<u32> {
    let c = by_code(&def.code)?;
    if !c.is_book() {
        return None;
    }
    let charges = book_charges(def);
    if charges <= 1 {
        w.hero.inv[slot] = None;
        Some(0)
    } else {
        w.hero.inv[slot] = Some(ItemDef { amount: charges - 1, ..def.clone() });
        Some(charges - 1)
    }
}

impl World {
    /// État consommables (ceinture + buffs) — lecture (HUD/tests).
    pub fn consumables(&self) -> &ConsumState {
        &self.consum
    }

    /// Utilise l'objet du slot d'inventaire `slot` comme CONSOMMABLE réel
    /// (effets par code, cf. module) — chemin ADDITIF, le `PlayerInput::
    /// UseItem` historique reste inchangé. Potion/parchemin : le slot est
    /// vidé ; tome : une charge est dépensée (`charges_left`), retiré à 0.
    /// En erreur (rien à identifier, tome vide, objet non utilisable) :
    /// état inchangé.
    pub fn use_consumable(&mut self, slot: usize) -> Result<ConsumUse, ConsumError> {
        let Some(def) = self.hero.inv.get(slot).cloned().flatten() else {
            return Err(ConsumError::SlotEmpty);
        };
        if book_charges(&def) == 0 && by_code(&def.code).is_some_and(|c| c.is_book()) {
            return Err(ConsumError::BookEmpty);
        }
        let use1 = apply(self, &def)?;
        match spend_book(self, slot, &def) {
            Some(left) => Ok(ConsumUse { charges_left: Some(left), ..use1 }),
            None => {
                self.hero.inv[slot] = None;
                Ok(use1)
            }
        }
    }

    /// Touche de ceinture `key` (1..=4) : boit/utilise le premier objet de
    /// la colonne (rangée 0 en premier, même convention que
    /// `inventory::Belt::take_key`). Les potions/scrolls sont consommés ;
    /// un tome posé en ceinture dépense une charge.
    pub fn use_belt_key(&mut self, key: usize) -> Result<ConsumUse, ConsumError> {
        if !(1..=4).contains(&key) {
            return Err(ConsumError::BadBeltKey(key));
        }
        let col = key - 1;
        let Some(row) = (0..4).find(|&r| self.consum.belt[col][r].is_some()) else {
            return Err(ConsumError::SlotEmpty);
        };
        let Some(def) = self.consum.belt[col][row].clone() else {
            return Err(ConsumError::SlotEmpty);
        };
        if by_code(&def.code).is_some_and(|c| c.is_book()) && book_charges(&def) == 0 {
            return Err(ConsumError::BookEmpty);
        }
        let use1 = apply(self, &def)?;
        let charges_left = if by_code(&def.code).is_some_and(|c| c.is_book()) {
            // Tome en ceinture (jamais via auto-ranger — col `belt=0` — mais
            // l'usage reste correct si un wire futur le place).
            let charges = book_charges(&def);
            if charges <= 1 {
                self.consum.belt[col][row] = None;
                0
            } else {
                self.consum.belt[col][row] =
                    Some(ItemDef { amount: charges - 1, ..def.clone() });
                charges - 1
            }
        } else {
            self.consum.belt[col][row] = None;
            return Ok(use1);
        };
        Ok(ConsumUse { charges_left: Some(charges_left), ..use1 })
    }

    /// « Auto-ranger » : déplace les consommables `autobelt=1` (hp1-5,
    /// mp1-5, rvs, rvl — colonne réelle) de l'inventaire vers la ceinture,
    /// ordre fixe (slots 0..40 × première cellule libre rangée puis
    /// colonne). Renvoie le nombre d'objets rangés. Idempotent.
    pub fn belt_auto_store(&mut self) -> usize {
        let mut moved = 0;
        for slot in 0..self.hero.inv.len() {
            let Some(def) = self.hero.inv[slot].clone() else { continue };
            let Some(c) = by_code(&def.code) else { continue };
            if !c.autobelt {
                continue;
            }
            let Some((col, row)) = (0..4)
                .flat_map(|r| (0..4).map(move |c| (c, r)))
                .find(|&(c, r)| self.consum.belt[c][r].is_none())
            else {
                break; // ceinture pleine : les potions restent en inventaire
            };
            self.consum.belt[col][row] = Some(def);
            self.hero.inv[slot] = None;
            moved += 1;
        }
        moved
    }
}

/// Tick additif (branché DANS `World::tick`, une ligne) : décompte des
/// buffs temporisés (temps simulé, zéro RNG).
pub(crate) fn tick_buffs(w: &mut World, dt_ms: u32) {
    w.consum.buffs.retain_mut(|b| b.tick(dt_ms));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::item_quality::Quality;
    use crate::PlayerInput;

    const MANIFEST_JSON: &str = include_str!("../../../../assets/manifest.json");

    fn open_grid(w: usize, h: usize) -> Vec<Vec<bool>> {
        vec![vec![false; w]; h]
    }

    fn world(seed: u64) -> World {
        World::with_seed(open_grid(56, 40), serde_json::from_str(MANIFEST_JSON).unwrap(), seed)
    }

    /// Avance le monde de `ms` (dt 50 ms) en consommant les événements.
    fn run(w: &mut World, ms: u32) -> Vec<GameEvent> {
        let mut ev = Vec::new();
        for _ in 0..(ms / 50) {
            ev.extend(w.tick(50, None));
        }
        ev
    }

    fn put(w: &mut World, slot: usize, code: &str) {
        w.debug_set_slot(slot, item_def(code));
    }

    // ---------------------- GOLDEN table (données réelles) --------------

    /// Golden PATCH_MISC.TXT : les 20 codes traités, valeurs de contrôle par
    /// famille (colonnes `cost`/`len`/`calc1`/`calc2`/`belt`/`autobelt`/
    /// `stackable`/`maxstack`/`useable`) — toute dérive de la table casse
    /// les tests, pas le runtime (table lenient).
    #[test]
    fn golden_table_matches_real_misc_txt() {
        // potions de vie : soins croissants = colonnes `calc1` (hpregen)
        let heal = [
            ("hp1", 30, 30u32),
            ("hp2", 75, 60),
            ("hp3", 125, 100),
            ("hp4", 250, 180),
            ("hp5", 500, 320),
        ];
        for (code, cost, power) in heal {
            let d = by_code(code).expect("code suivi");
            assert_eq!((d.cost, d.power), (cost, power), "{code}");
            assert_eq!(d.kind, ConsumableKind::Healing);
            assert!(d.usable && d.belt && d.autobelt, "{code} : belt+autobelt réels");
        }
        // potions de mana
        let mana = [
            ("mp1", 60, 20u32),
            ("mp2", 150, 40),
            ("mp3", 300, 80),
            ("mp4", 500, 150),
            ("mp5", 1000, 250),
        ];
        for (code, cost, power) in mana {
            let d = by_code(code).expect("code suivi");
            assert_eq!((d.cost, d.power), (cost, power), "{code}");
            assert_eq!(d.kind, ConsumableKind::Mana);
        }
        // rejuv 35 % / 100 % (colonnes hitpoints/mana)
        let rvs = by_code("rvs").unwrap();
        assert_eq!((rvs.cost, rvs.rejuv_pct), (400, 35));
        assert_eq!(rvs.kind, ConsumableKind::Rejuvenation);
        let rvl = by_code("rvl").unwrap();
        assert_eq!((rvl.cost, rvl.rejuv_pct), (1500, 100));
        // buffs : 750 frames (30 s) + stats réelles
        let yps = by_code("yps").unwrap();
        assert_eq!(yps.kind, ConsumableKind::Buff(BuffState::Antidote));
        assert_eq!(yps.duration_frames, 750);
        assert_eq!(yps.duration_ms(), 30_000);
        assert_eq!(
            yps.buff_stats,
            vec![(crate::statlist::POISONRESIST, 50), (MAXPOISONRESIST, 10)]
        );
        assert!(!yps.autobelt, "antidote : autobelt=0 réel");
        let wms = by_code("wms").unwrap();
        assert_eq!(wms.kind, ConsumableKind::Buff(BuffState::Thawing));
        assert_eq!(wms.buff_stats, vec![(crate::statlist::COLDRESIST, 50), (MAXCOLDRESIST, 10)]);
        let vps = by_code("vps").unwrap();
        assert_eq!(vps.kind, ConsumableKind::Buff(BuffState::Stamina));
        assert_eq!(vps.buff_stats, vec![(STAMINARECOVERYBONUS, 5000)]);
        // parchemins : belt=1, autobelt=0
        for code in ["tsc", "isc"] {
            let d = by_code(code).unwrap();
            assert!(d.belt && !d.autobelt && !d.stackable, "{code}");
        }
        assert_eq!(by_code("tsc").unwrap().cost, 100);
        assert_eq!(by_code("isc").unwrap().cost, 80);
        // tomes : empilables, maxstack 20 (charges), belt=0
        for (code, cost) in [("tbk", 250u32), ("ibk", 200)] {
            let d = by_code(code).unwrap();
            assert!(d.stackable && d.maxstack == 20 && !d.belt, "{code}");
            assert_eq!(d.cost, cost);
            assert!(d.is_book());
        }
        // clé : réellement useable=0 (mission « key… »)
        let key = by_code("key").unwrap();
        assert!(!key.usable && key.maxstack == 12 && key.cost == 45);
        // ids ItemStatCost des buffs (patch_itemstatcost.txt réel)
        let isc = crate::stats::StatCostTable::global();
        for (name, id) in [
            ("maxpoisonresist", MAXPOISONRESIST),
            ("maxcoldresist", MAXCOLDRESIST),
            ("staminarecoverybonus", STAMINARECOVERYBONUS),
            ("poisonresist", crate::statlist::POISONRESIST),
            ("coldresist", crate::statlist::COLDRESIST),
        ] {
            assert_eq!(isc.get(name).expect(name).id, id, "id {name}");
        }
    }

    // ---------------------- Famille : potions de vie ---------------------

    #[test]
    fn healing_potions_heal_by_real_table_values() {
        let mut w = world(7);
        w.clear_monsters();
        // vie max portée à 500 pour lire les valeurs PLEINES de la table
        // (le barbare niv 1 plafonne à 55 pv)
        w.hero.stats.max_life = 500;
        w.hero.stats.life = 0; // blessé au maximum : soin = valeur pleine
        put(&mut w, 0, "hp1");
        let u = w.use_consumable(0).expect("hp1");
        assert_eq!((u.family.as_str(), u.heal), ("healing", 30));
        assert_eq!(w.hero_stats().life, 30);
        // l'objet est consommé + PotionUsed en file (pattern NpcTalked)
        assert!(w.inventory()[0].is_none());
        let ev = w.tick(0, None);
        assert!(ev.iter().any(|e| matches!(e, GameEvent::PotionUsed { heal: 30 })));
        // plafond : pleine vie ⇒ heal 0, potion quand même bue (retail)
        w.hero.stats.life = w.hero.stats.max_life;
        put(&mut w, 1, "hp5");
        let u = w.use_consumable(1).unwrap();
        assert_eq!(u.heal, 0, "pleine vie");
        assert!(w.inventory()[1].is_none());
        // échelle croissante hp1<hp2<hp3<hp4<hp5 (valeurs table). Les valeurs
        // heal = colonnes réelles `stat1=hpregen, calc1` de patch_misc.txt
        // (30/60/100/180/320, identiques au tooltip `spelldesccalc`) — le
        // monde de test porte la vie max à 500 : le barbare niv 1 (55 pv)
        // plafonnerait sinon à son manquant, ce qui masque la table.
        let mut w2 = world(7);
        w2.clear_monsters();
        w2.hero.stats.max_life = 500;
        let vals: Vec<u32> = ["hp1", "hp2", "hp3", "hp4", "hp5"]
            .iter()
            .map(|&c| {
                w2.hero.stats.life = 0;
                put(&mut w2, 5, c);
                w2.use_consumable(5).unwrap().heal
            })
            .collect();
        assert_eq!(vals, vec![30, 60, 100, 180, 320], "soins croissants table");
    }

    // ---------------------- Famille : potions de mana --------------------

    #[test]
    fn mana_potions_restore_by_real_table_values() {
        let mut w = world(7);
        w.clear_monsters();
        // mana max porté à 300 pour voir les valeurs table entières
        w.hero.stats.max_mana = 300;
        w.hero.stats.mana = 0;
        put(&mut w, 2, "mp3");
        let u = w.use_consumable(2).expect("mp3");
        assert_eq!((u.family.as_str(), u.mana), ("mana", 80));
        assert_eq!(u.heal, 0, "pas de vie");
        assert_eq!(w.hero_stats().mana, 80);
        // plafond mana
        put(&mut w, 3, "mp5");
        let u = w.use_consumable(3).unwrap();
        assert_eq!(u.mana, 300 - 80);
        assert_eq!(w.hero_stats().mana, 300);
        // échelle croissante mp1..mp5
        let mut w2 = world(7);
        w2.clear_monsters();
        w2.hero.stats.max_mana = 300;
        let vals: Vec<u32> = ["mp1", "mp2", "mp3", "mp4", "mp5"]
            .iter()
            .map(|&c| {
                w2.hero.stats.mana = 0;
                put(&mut w2, 6, c);
                w2.use_consumable(6).unwrap().mana
            })
            .collect();
        assert_eq!(vals, vec![20, 40, 80, 150, 250]);
    }

    // ---------------------- Famille : rajeunissement ---------------------

    #[test]
    fn rejuv_potions_restore_percent_of_max() {
        let mut w = world(7);
        w.clear_monsters();
        let (max_life, max_mana) = (w.hero_stats().max_life, w.hero_stats().max_mana);
        w.hero.stats.life = 0;
        w.hero.stats.mana = 0;
        put(&mut w, 4, "rvs");
        let u = w.use_consumable(4).expect("rvs");
        let exp_heal = (max_life as u64 * 35 / 100) as u32;
        let exp_mana = (max_mana as u64 * 35 / 100) as u32;
        assert_eq!((u.family.as_str(), u.heal, u.mana), ("rejuv", exp_heal, exp_mana), "35 % vie+mana");
        assert_eq!(w.hero_stats().life, exp_heal);
        // rvl = 100 % (full)
        put(&mut w, 5, "rvl");
        w.hero.stats.life = 0;
        w.hero.stats.mana = 0;
        let u = w.use_consumable(5).expect("rvl");
        assert_eq!((u.heal, u.mana), (max_life, max_mana), "100 % vie+mana");
        assert_eq!((w.hero_stats().life, w.hero_stats().mana), (max_life, max_mana));
    }

    // ---------------------- Famille : buffs temporisés -------------------

    #[test]
    fn timed_buffs_use_existing_state_system() {
        let mut w = world(7);
        w.clear_monsters();
        put(&mut w, 0, "yps");
        let u = w.use_consumable(0).expect("yps");
        assert_eq!((u.family.as_str(), u.buff_ms), ("buff", 30_000), "750 f × 40 ms");
        // buff actif, contributions statlist aux ids réels
        let buff = w.consumables().buff(BuffState::Antidote).expect("buff actif").clone();
        assert!(buff.is_active());
        assert_eq!(
            buff.stat_lines(),
            vec![
                StatLine::new(crate::statlist::POISONRESIST, 50),
                StatLine::new(MAXPOISONRESIST, 10),
            ]
        );
        // expiration au temps simulé (tick branché dans World::tick)
        run(&mut w, 29_950);
        assert!(w.consumables().buff(BuffState::Antidote).is_some(), "actif à 50 ms de la fin");
        run(&mut w, 100);
        assert!(w.consumables().buff(BuffState::Antidote).is_none(), "expiré");
        // coexistence des 3 états + rafraîchissement au re-boire
        for code in ["yps", "wms", "vps"] {
            put(&mut w, 1, code);
            assert!(w.use_consumable(1).is_ok(), "{code}");
        }
        assert_eq!(w.consumables().buffs.len(), 3, "les 3 états coexistent");
        run(&mut w, 10_000); // reste 20 000 ms sur chaque
        put(&mut w, 2, "yps");
        w.use_consumable(2).unwrap(); // rafraîchit l'antidote à 30 000
        let b = w.consumables().buff(BuffState::Antidote).unwrap();
        assert_eq!(b.remaining_ms, 30_000, "re-boire = durée fraîche");
        assert_eq!(w.consumables().buffs.len(), 3);
    }

    // ---------------------- Famille : porte de ville + tomes -------------

    #[test]
    fn town_portal_scroll_and_book_charges() {
        let mut w = world(7);
        w.clear_monsters();
        // parchemin : consommé, flag pour le wire d2game (switch_level)
        put(&mut w, 0, "tsc");
        let u = w.use_consumable(0).expect("tsc");
        assert!(u.town_portal, "le parchemin porte le retour au camp");
        assert_eq!(u.family, "town_portal");
        assert!(w.inventory()[0].is_none(), "parchemin consommé");
        // tome : 20 charges, -1 par usage, retiré à 0 (lecture/écriture stock)
        put(&mut w, 1, "tbk");
        let def = w.inventory()[1].clone().unwrap();
        assert_eq!(book_charges(&def), BOOK_CHARGES, "tome neuf = 20 parchemins");
        assert_eq!(def.name, "Livre des portails des villes", "nom fr TBL");
        for expected in (1..BOOK_CHARGES).rev() {
            let u = w.use_consumable(1).expect("charge");
            assert_eq!(u.charges_left, Some(expected));
            assert!(u.town_portal);
        }
        let u = w.use_consumable(1).expect("dernière charge");
        assert_eq!(u.charges_left, Some(0));
        assert!(w.inventory()[1].is_none(), "tome vide retiré");
        // tome vide : refus net (pas d'effet sans parchemin)
        put(&mut w, 2, "tbk");
        w.debug_set_slot(2, Some(ItemDef { amount: 0, ..item_def("tbk").unwrap() }));
        assert_eq!(w.use_consumable(2), Err(ConsumError::BookEmpty));
    }

    // ---------------------- Famille : identification ---------------------

    #[test]
    fn identification_rules_per_quality() {
        let mut normal = crate::inventory::Item { code: "cap".into(), ..Default::default() };
        normal.quality = Quality::Normal;
        assert!(!needs_identification(&normal), "normal : jamais");
        let mut superior = crate::inventory::Item { code: "buc".into(), ..Default::default() };
        superior.quality = Quality::Superior;
        assert!(!needs_identification(&superior), "supérieur : jamais");
        for q in [Quality::Magic, Quality::Rare, Quality::Set, Quality::Unique] {
            let mut it = crate::inventory::Item { code: "hax".into(), ..Default::default() };
            it.quality = q;
            assert!(needs_identification(&it), "{q:?} drop non identifié (flag loot.rs)");
            assert!(identify(&mut it), "un isc l'identifie");
            assert!(it.identified);
            assert!(!identify(&mut it), "idempotent : déjà identifié");
        }
        // Cain identifie tout (pur) : compte exact — les items normaux
        // naissent identifiés (comme les drops `loot.rs`, flagged true).
        let mut bag: Vec<crate::inventory::Item> = Vec::new();
        for q in [Quality::Magic, Quality::Rare, Quality::Set, Quality::Unique, Quality::Normal] {
            let mut it = crate::inventory::Item { code: "hax".into(), ..Default::default() };
            it.quality = q;
            it.identified = q == Quality::Normal;
            bag.push(it);
        }
        assert_eq!(identify_all(&mut bag), 4, "Cain : les 4 magiques+ (normal ignoré)");
        assert!(bag.iter().all(|i| i.identified));
        // en partie : le hook runtime est documenté — isc refuse et reste
        let mut w = world(7);
        w.clear_monsters();
        put(&mut w, 0, "isc");
        assert_eq!(w.use_consumable(0), Err(ConsumError::NothingToIdentify));
        assert!(w.inventory()[0].is_some(), "pas de gaspillage sans cible");
    }

    // ---------------------- Ceinture 4×4 + touches 1-4 -------------------

    #[test]
    fn belt_auto_stores_autobelt_potions_only() {
        let mut w = world(7);
        w.clear_monsters();
        // autobelt=1 réel : hp1-5, mp1-5, rvs, rvl ; =0 : vps/yps/wms/tsc
        put(&mut w, 0, "hp1");
        put(&mut w, 1, "rvl");
        put(&mut w, 2, "yps"); // NON auto
        put(&mut w, 3, "tsc"); // NON auto
        let moved = w.belt_auto_store();
        assert_eq!(moved, 2, "seules les autobelt=1 sont rangées");
        assert_eq!(w.consumables().belt[0][0].as_ref().unwrap().code, "hp1", "touche 1");
        assert_eq!(w.consumables().belt[1][0].as_ref().unwrap().code, "rvl", "touche 2");
        assert_eq!(w.inventory()[2].as_ref().unwrap().code, "yps", "reste en inventaire");
        assert_eq!(w.inventory()[3].as_ref().unwrap().code, "tsc");
        // ceinture pleine à 16 : le surplus reste en inventaire
        for s in 10..40 {
            put(&mut w, s, "hp2");
        }
        let moved = w.belt_auto_store();
        assert_eq!(moved, 14, "16 - 2 occupés");
        assert_eq!(w.consumables().free_slots(), 0);
        let moved = w.belt_auto_store();
        assert_eq!(moved, 0, "pleine : idempotent, plus rien ne bouge");
        assert!(w.inventory().iter().any(|s| s.as_ref().is_some_and(|d| d.code == "hp2")));
    }

    #[test]
    fn belt_keys_1_to_4_drink_column_top_first() {
        let mut w = world(7);
        w.clear_monsters();
        // vie max 500 : le plafond 55 pv du barbare niv 1 masquerait les
        // valeurs table (hp3 = 100 > 55)
        w.hero.stats.max_life = 500;
        w.hero.stats.life = 0;
        put(&mut w, 0, "hp1");
        put(&mut w, 1, "hp3");
        w.belt_auto_store();
        // rangée 0 : hp1 touche 1, hp3 touche 2 (vie remise à 0 avant chaque
        // gorgée pour lire la valeur PLEINE de la table, pas le plafond 55 pv)
        let u = w.use_belt_key(1).expect("touche 1");
        assert_eq!(u.code, "hp1");
        assert_eq!(u.heal, 30);
        w.hero.stats.life = 0;
        let u = w.use_belt_key(2).expect("touche 2");
        assert_eq!(u.code, "hp3");
        assert_eq!(u.heal, 100);
        // pile : deux hp5 → rangée 0 des touches 1 puis 2 (libres)
        w.hero.stats.life = 0;
        w.debug_set_slot(10, item_def("hp5"));
        w.debug_set_slot(11, item_def("hp5"));
        w.belt_auto_store();
        let u = w.use_belt_key(1).expect("hp5 rangée 0");
        assert_eq!((u.code.as_str(), u.heal), ("hp5", 320));
        assert!(w.use_belt_key(1).is_err(), "colonne 1 vide");
        assert!(w.use_belt_key(2).is_ok(), "hp5 sur la touche 2");
        // touches hors bornes
        assert_eq!(w.use_belt_key(0), Err(ConsumError::BadBeltKey(0)));
        assert_eq!(w.use_belt_key(5), Err(ConsumError::BadBeltKey(5)));
    }

    // ---------------------- Erreurs + noms fr ----------------------------

    #[test]
    fn errors_and_french_names() {
        let mut w = world(7);
        w.clear_monsters();
        assert_eq!(w.use_consumable(9), Err(ConsumError::SlotEmpty), "slot vide");
        put(&mut w, 0, "key");
        assert!(matches!(w.use_consumable(0), Err(ConsumError::NotUsable(_))), "key useable=0");
        assert!(w.inventory()[0].is_some(), "refus : objet intact");
        w.debug_set_slot(
            1,
            Some(ItemDef { code: "abc".into(), name: "?".into(), kind: ItemKind::Misc, amount: 0 }),
        );
        assert_eq!(w.use_consumable(1), Err(ConsumError::Unknown("abc".into())));
        // noms fr réels (TBL, clé = code)
        assert_eq!(display_name("hp1").as_deref(), Some("Petite potion de santé"));
        assert_eq!(display_name("hp5").as_deref(), Some("Super potion de santé"));
        assert_eq!(display_name("rvl").as_deref(), Some("Potion de rajeunissement total"));
        assert_eq!(display_name("yps").as_deref(), Some("Antidote"));
        assert_eq!(display_name("tsc").as_deref(), Some("Parchemin de Porte de ville"));
        assert_eq!(display_name("ibk").as_deref(), Some("Livre d'identification"));
        assert_eq!(display_name("key").as_deref(), Some("Clé"));
        // constructeur : nom fr, kind cohérent, tome = 20 charges
        let hp = item_def("hp1").unwrap();
        assert_eq!((hp.name.as_str(), hp.kind), ("Petite potion de santé", ItemKind::Potion));
        let tbk = item_def("tbk").unwrap();
        assert_eq!((tbk.kind, tbk.amount), (ItemKind::Misc, BOOK_CHARGES));
        let isc = item_def("isc").unwrap();
        assert_eq!((isc.kind, isc.amount), (ItemKind::Misc, 0));
    }

    // ---------------------- Déterminisme ---------------------------------

    #[test]
    fn deterministic_no_rng_same_seed_same_outcome() {
        // Même graine + même séquence ⇒ état final identique (octet près) :
        // les consommables ne tirent JAMAIS au sort.
        fn script(seed: u64) -> String {
            let mut w = World::with_seed(
                vec![vec![false; 56]; 40],
                serde_json::from_str(MANIFEST_JSON).unwrap(),
                seed,
            );
            w.clear_monsters();
            w.hero.stats.life = 0;
            w.hero.stats.mana = 0;
            for (s, c) in [0usize, 1, 2, 3].iter().zip(["hp2", "mp2", "rvs", "yps"]) {
                put(&mut w, *s, c);
            }
            w.belt_auto_store();
            let _ = w.use_belt_key(1);
            let _ = w.use_belt_key(2);
            put(&mut w, 4, "rvl");
            let _ = w.use_consumable(4);
            run(&mut w, 1_000);
            w.save()
        }
        assert_eq!(script(0xD2), script(0xD2), "même graine ⇒ même save");
        // et le flux de drop en aval reste déterministe (aucun RNG consommé)
        let mut a = world(0xD2);
        let mut b = world(0xD2);
        for w in [&mut a, &mut b] {
            w.clear_monsters();
            put(w, 0, "hp1");
            w.belt_auto_store();
            let _ = w.use_belt_key(1);
        }
        a.spawn_monster("FA", 5, 5);
        b.spawn_monster("FA", 5, 5);
        let mut da = Vec::new();
        let mut db = Vec::new();
        for _ in 0..600 {
            da.extend(a.tick(50, Some(PlayerInput::Attack { x: 5, y: 5 })));
            db.extend(b.tick(50, Some(PlayerInput::Attack { x: 5, y: 5 })));
        }
        let drops = |ev: &[GameEvent]| -> Vec<String> {
            ev.iter()
                .filter_map(|e| match e {
                    GameEvent::ItemDropped { item, .. } => Some(item.code.clone()),
                    _ => None,
                })
                .collect()
        };
        assert_eq!(drops(&da), drops(&db), "aucun RNG consommé par les consommables");
    }
}
