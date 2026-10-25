//! Pont `.d2s` ⇄ save v3 (étape 6b) — format d'échange OPTIONNEL déclenché
//! par bouton dans la démo (« Exporter / Importer .d2s »).
//!
//! Le save JSON v3 ([`crate::World::save`]) reste LE format par défaut : ce
//! module ne change rien au contrat, il traduit seulement l'état du héros
//! vers/depuis un fichier de personnage Diablo II v1.12 réel (`.d2s`
//! version 0x60) via la crate sœur [`d2d2s`] (parse/écriture round-trip
//! prouvés sur ses vecteurs + checksum complet).
//!
//! # Export ([`World::export_d2s`]) — save v3 → `.d2s` v0x60 VALIDE
//! (checksum inclus) : nom (paramètre, assaini ASCII), classe, niveau,
//! stats de base (str/dex réelles du héros ; energy/vitalité des lignes
//! réelles `patch_charstats.txt` de la classe — le save v3 ne les porte
//! pas), vie/mana/endurance (courant + max, ×256), XP/or, points de skill,
//! compétences (slots réels skills.txt via `SkillTreeData`), waypoints
//! activés (bits retail 0..=38 de la difficulté courante ; le slot MVP 39
//! Blood Moor est HORS plage retail → non exporté), difficulté (entrée
//! active Normal/NM/Enfer) et inventaire → items basiques IDENTIFIÉS
//! (qualité normale, sans affixe) posés à leur case de grille 10×4. L'or
//! porté part dans la stat 14 (pas d'item `gld`). Export DÉTERMINISTE :
//! horodatage et ID anti-dupe à valeurs fixes (même état ⇒ mêmes octets).
//!
//! # Import ([`World::import_d2s`]) — `.d2s` → save v3
//! Mappe ce qui existe : classe (héros + gabarit manifest), niveau
//! (plafonné au MVP 10, XP réalignée au seuil), str/dex, vie/mana/
//! endurance (÷256, courant borné au max), or (stat 14), points de skill,
//! compétences (slots → codes + missiles de la table ; fichier sans
//! compétence de classe ⇒ skills de départ de la classe importée),
//! waypoints activés, difficulté (entrée active du header), items de la
//! liste caractère (équipés d'abord, puis inventaire stocké à LEUR case,
//! plafonnés aux 40 slots) réduits en [`crate::ItemDef`] basiques. Un
//! héros mort est relevé (vivant, anim neutre) ; la position, la zone, les
//! monstres et le reste du monde restent ceux du moment.
//!
//! # Ignoré pour l'instant (documenté — le save v3 ne le porte pas)
//! * nom du personnage (aucun champ dans la save v3) — renvoyé dans le
//!   rapport à titre informatif ;
//! * energy/vitalité du fichier (pas de colonnes `HeroStats`) ;
//! * points de stats non dépensés (stat 4) et or en coffre (stat 15) ;
//! * quêtes, introductions NPC, hotkeys, apparence, titre/progression,
//!   flags hardcore/ladder, seed de carte, acte courant (la zone reste
//!   celle du monde) ;
//! * mercenaire (en-tête + liste `jf`), golem de fer (`kf`), liste corpse
//!   (`JM` secondaire), items du coffre/cube/ceinture, gemmes serties ;
//! * qualité/affixes/mods/durabilité des items (basiques uniquement) ;
//! * slot MVP 39 des waypoints (hors plage retail 0..=38).
//!
//! Aucune IO : octets en entrée/sortie, l'écriture/lecture fichier vit
//! dans la page (bouton téléchargement / input file).

use super::world::{INV_COLS, World, max_level_of, xp_threshold};
use crate::skills_hero::class_code;
use d2d2s::{
    Class, Container, DifficultyEntry, Item, ItemExtended, ItemHeader, Location, Quality,
    QualityData, SaveFile, Skills, TypeData, WaypointBits,
};
use serde::Serialize;

// --- fiche base (ex-`loot::base_of`, refondu étape 7) -----------------------
// Tables v1.12 embarquées : nom affichable + bornes d'AC par code (`lea`,
// `hax`, `hp1`…). Lecture seule, TSV tolérant (CRLF/quotes).

const BASE_WEAPONS_TXT: &str = include_str!("../../../../assets/patch_weapons.txt");
const BASE_ARMOR_TXT: &str = include_str!("../../../../assets/patch_armor.txt");
const BASE_MISC_TXT: &str = include_str!("../../../../assets/patch_misc.txt");

/// Fiche base minimale pour l'export `.d2s` (nom + bornes AC).
struct BaseFiche {
    name: String,
    min_ac: u32,
    max_ac: u32,
}

fn base_fiche(code: &str) -> Option<BaseFiche> {
    for txt in [BASE_ARMOR_TXT, BASE_WEAPONS_TXT, BASE_MISC_TXT] {
        let Ok(t) = crate::tsv::Tsv::parse(txt) else { continue };
        for i in 0..t.len() {
            if t.cell(i, "code") != Some(code) {
                continue;
            }
            let name = t
                .cell(i, "name")
                .filter(|n| !n.is_empty())
                .unwrap_or(code)
                .to_string();
            let ac = |col: &str| -> u32 {
                t.cell(i, col)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0)
            };
            return Some(BaseFiche { name, min_ac: ac("minac"), max_ac: ac("maxac") });
        }
    }
    None
}


/// Lignes réelles `patch_charstats.txt` v1.12 (colonnes `class/str/dex/int/
/// vit`) — energy/vitalité de départ par classe, absentes du save v3.
const CHARSTATS: &str = include_str!("../../../../assets/patch_charstats.txt");

/// Rapport d'import — sérialisé en JSON pour la page (note HUD).
#[derive(Debug, Clone, Serialize)]
pub struct D2sImportReport {
    /// Nom lu dans le fichier (INFORMATIF — non mappé, pas de champ save v3).
    pub name: String,
    /// Code héros manifest effectif après import (`"BA"`, `"SOR"`…).
    pub class: String,
    /// La classe du fichier différait de celle du monde.
    pub class_changed: bool,
    /// Niveau appliqué (plafonné au MVP 10).
    pub level: u32,
    /// Force appliquée.
    pub strength: u32,
    /// Dextérité appliquée.
    pub dexterity: u32,
    /// Or appliqué (stat 14).
    pub gold: u32,
    /// Nombre de compétences mappées.
    pub skills: usize,
    /// Nombre de waypoints activés mappés.
    pub waypoints: usize,
    /// Difficulté appliquée (`normal`/`nightmare`/`hell`).
    pub difficulty: String,
    /// Items chargés dans l'inventaire (basiques).
    pub items_loaded: usize,
    /// Items ignorés (or, coffre/cube/ceinture, serties, au-delà de 40…).
    pub items_ignored: usize,
}

// ---------------------------------------------------------------------------
// Tables de correspondance (codes manifest ⇄ classes .d2s ⇄ charstats)
// ---------------------------------------------------------------------------

/// Code héros manifest (`heroes` de `assets/manifest.json`) pour une classe
/// `.d2s`. `Other` (valeur hors table) ⇒ `"BA"` (défaut du manifest).
fn hero_code_of_class(c: Class) -> &'static str {
    match c {
        Class::Amazon => "AM",
        Class::Sorceress => "SOR",
        Class::Necromancer => "NE",
        Class::Paladin => "PA",
        Class::Barbarian => "BA",
        Class::Druid => "DZ",
        Class::Assassin => "AS",
        Class::Other(_) => "BA",
    }
}

/// Classe `.d2s` pour un code héros manifest (via `class_code` ama/sor/…).
fn class_of_hero(hero_code: &str) -> Class {
    match class_code(hero_code).as_str() {
        "ama" => Class::Amazon,
        "sor" => Class::Sorceress,
        "nec" => Class::Necromancer,
        "pal" => Class::Paladin,
        "bar" => Class::Barbarian,
        "dru" => Class::Druid,
        "ass" => Class::Assassin,
        _ => Class::Barbarian,
    }
}

/// Nom de ligne `patch_charstats.txt` d'un code héros.
fn charstats_row_name(hero_code: &str) -> &'static str {
    match class_code(hero_code).as_str() {
        "ama" => "Amazon",
        "sor" => "Sorceress",
        "nec" => "Necromancer",
        "pal" => "Paladin",
        "bar" => "Barbarian",
        "dru" => "Druid",
        "ass" => "Assassin",
        _ => "Barbarian",
    }
}

/// `(energy, vitality)` RÉELS de départ de la classe (`patch_charstats.txt`,
/// colonnes `int`/`vit` de la première ligne portant le nom) — défauts
/// barbare (10/25) si la ligne est absente.
fn charstats_bases(hero_code: &str) -> (u32, u32) {
    let want = charstats_row_name(hero_code);
    for line in CHARSTATS.lines() {
        let mut cols = line.split('\t');
        let name = cols.next().unwrap_or("").trim();
        if !name.eq_ignore_ascii_case(want) {
            continue;
        }
        // colonnes 1-based : str=1, dex=2, int=3, vit=4 — on veut int/vit.
        let cell = |i: usize| cols.clone().nth(i - 1).and_then(|c| c.trim().parse().ok());
        if let (Some(e), Some(v)) = (cell(3), cell(4)) {
            return (e, v);
        }
    }
    (10, 25)
}

/// Nom `.d2s` assaini : ASCII alphanumérique, 15 caractères max (place du
/// 0 final), jamais vide (repli : nom de classe anglais).
fn sanitize_name(name: &str, class: Class) -> String {
    let clean: String = name.chars().filter(|c| c.is_ascii_alphanumeric()).take(15).collect();
    if clean.is_empty() { class.name().to_string() } else { clean }
}

/// Code 4 octets du type d'item (`"hax"` → `b"hax "`, padding espaces).
fn type_raw_of(code: &str) -> [u8; 4] {
    let mut raw = [b' '; 4];
    for (i, &b) in code.as_bytes().iter().take(4).enumerate() {
        raw[i] = b;
    }
    raw
}

/// Plafond de VALEUR d'une stat de base : largeur CSvBits (table réelle
/// embarquée d2d2s, repli 32 bits) ramenée par ValShift — ex. hp 21 bits
/// ÷256 ⇒ 8191, or 25 bits ⇒ 0x1FFFFFF. Remplace les bornes codées en dur
/// de l'étape 6b (identiques bit pour bit, mais dérivées de la table).
fn csv_bits_max(id: u16) -> u32 {
    let bits = u64::from(d2d2s::stats::csv_bits(id).unwrap_or(32));
    let shift = u32::from(d2d2s::stats::val_shift(id));
    (((1u64 << bits) - 1) >> shift) as u32
}

// ---------------------------------------------------------------------------
// Export — save v3 → .d2s v0x60
// ---------------------------------------------------------------------------

impl World {
    /// Exporte l'état du héros vers un fichier `.d2s` v1.12 (0x60) VALIDE
    /// (taille déclarée + checksum recalculés par `d2d2s`). `name` est le
    /// nom du personnage (assaini ASCII, 15 max — la save v3 ne porte pas
    /// de nom, la page passe le nom affiché du héros). `Err` = raison.
    pub fn export_d2s(&self, name: &str) -> Result<Vec<u8>, String> {
        let class = class_of_hero(&self.players[0].code);
        let mut save = SaveFile::default_v112();

        // --- en-tête : nom, classe, niveau, statut, difficulté -------------
        save.header.set_name(&sanitize_name(name, class));
        save.header.class = class;
        save.header.level = self.players[0].data.stats.level.clamp(1, 99) as u8;
        // Port LOD v1.12 : personnage d'expansion (acte V, mercenaires…).
        save.header.status.expansion = true;
        let d_idx = self.difficulty.index();
        // Progression « retail » : +5 par difficulté franchie (oracle
        // d2s-file-generator : 0 / 5 / 10 / 15). Le save v3 ne porte que la
        // difficulté COURANTE — approximation 5 × index (Normal 0, NM 5,
        // Enfer 10 ; le cas « Enfer terminé » retail 15 n'est pas
        // exprimable sans les quêtes, cf. docs/d2s-export-interop.md).
        save.header.progression = (5 * d_idx) as u8;
        // Horodatage à 0 : l'export est DÉTERMINISTE (même état ⇒ mêmes
        // octets). Retail écrit l'heure réelle — écart volontaire documenté.
        save.header.timestamp = 0;
        for (i, entry) in save.header.difficulty.iter_mut().enumerate() {
            *entry = DifficultyEntry { active: i == d_idx, act: 0, unknown: 0 };
        }

        // --- stats (flux 1.10+ PRÉFIXÉ « gf », zeros omis) -----------------
        // Layout « retail 1.14d » étape 19c (saves oracle, jeu réel) : la
        // section 765 commence par `67 66` = « gf » puis le flux standard
        // (ID 9 bits + CSvBits, 0x1FF final, align octet). Retail omet en
        // outre les stats À ZÉRO (statpts/newskills/xp/or à 0 n'apparaissent
        // pas dans ses fichiers) — seules les attrs 0–3, le bloc vie 6–11
        // et le niveau 12 sont écrits inconditionnellement.
        save.stats = d2d2s::Stats {
            format: d2d2s::StatsFormat::GfPrefixed110,
            entries: Vec::new(),
        };
        let s = &self.players[0].data.stats;
        let (energy, vitality) = charstats_bases(&self.players[0].code);
        let mut set_stat = |id: u16, v: u32, force: bool| {
            if force || v != 0 {
                save.stats.set(id, v.min(csv_bits_max(id)));
            }
        };
        set_stat(0, s.strength, true); // strength
        set_stat(1, energy, true); // energy (charstats)
        set_stat(2, s.dexterity, true); // dexterity
        set_stat(3, vitality, true); // vitality (charstats)
        set_stat(4, s.stat_points, false); // statpts (points de stats non dépensés — save v3 19b)
        set_stat(5, s.skill_points, false); // newskills
        // hp/mana/stamina stockés ×256 (ValShift 8) — 21 bits ⇒ max 8191.
        set_stat(6, s.life, true);
        set_stat(7, s.max_life, true);
        set_stat(8, s.mana, true);
        set_stat(9, s.max_mana, true);
        set_stat(10, s.stamina, true);
        set_stat(11, s.max_stamina, true);
        set_stat(12, s.level.min(127), true); // level
        set_stat(13, s.xp, false); // experience (32 bits)
        set_stat(14, s.gold, false); // gold (25 bits)
        set_stat(15, 0, false); // goldbank : pas de coffre dans le save v3

        // --- compétences : codes → slots réels skills.txt (offset classe) --
        let base = Skills::class_slot_base(class) as u32;
        let tree = crate::skills_tree::SkillTreeData::global();
        for skill in &self.players[0].data.skills {
            let Some(rec) = tree.get(&skill.code) else { continue };
            let slot = rec.id as i64 - base as i64;
            if (0..30).contains(&slot) {
                save.skills.set_level(slot as usize, skill.level.min(255) as u8);
            }
        }

        // --- waypoints activés (bits retail 0..=38 de la difficulté) ------
        // Retail (saves oracle) : le bit 0 (Rogue Encampment) est posé dans
        // les TROIS enregistrements de difficulté dès la création (le camp
        // est le point d'entrée de chacune) — on l'aligne sur le jeu.
        let mut bits = WaypointBits::default();
        for &slot in self.wp.active.iter() {
            if (slot as usize) < 39 {
                bits.set(slot as usize, true);
            } // slot 39 (MVP Blood Moor) : hors plage retail — non exporté
        }
        bits.set(0, true);
        if !save.waypoints.set_difficulty(d_idx, bits) {
            return Err("index de difficulté hors plage".into());
        }
        for i in 0..d2d2s::sections::DIFFICULTIES {
            if i != d_idx {
                let mut other = save.waypoints.difficulty(i).unwrap_or_default();
                other.set(0, true);
                save.waypoints.set_difficulty(i, other);
            }
        }

        // --- inventaire → items basiques identifiés ------------------------
        // L'or porté vit dans la stat 14 (ci-dessus) — pas d'item `gld`.
        let mut file_id = 0x00D2_0001_u32; // compteur déterministe (anti-dupe)
        for (i, slot) in self.players[0].data.inv.iter().enumerate() {
            let Some(def) = slot else { continue };
            if def.kind == crate::ItemKind::Gold {
                continue;
            }
            let code = def.code.as_str();
            let type_data = if d2d2s::codes::is_weapon(code) {
                TypeData::Weapon { max_durability: 10, durability: 10, sockets: None }
            } else if d2d2s::codes::is_armor(code) {
                // défense stockée : AC moyenne réelle de la base (tables
                // patch_weapons/armor/misc embarquées — ex-`loot::base_of`,
                // refondu étape 7 ; voir `base_fiche` plus bas), repli 0.
                let ac = base_fiche(code)
                    .map(|f| (f.min_ac + f.max_ac) / 2)
                    .unwrap_or(0);
                TypeData::Armor {
                    defense_stored: ac.min(1023) as u16,
                    max_durability: 12,
                    durability: 12,
                    sockets: None,
                }
            } else if d2d2s::codes::is_stackable(code) {
                TypeData::Stack { tome_unknown: None, quantity: 1 }
            } else {
                TypeData::None // potions `hp1`, misc…
            };
            save.character_items.items.push(Item {
                header: ItemHeader {
                    identified: true,
                    // Gear de départ retail (bit 33 « starter/newbie ») :
                    // mission 19c — items exportés comme l'équipement de
                    // départ du jeu, identifiés (cf. docs/d2s-export-interop.md).
                    starter: true,
                    location: Location::Stored,
                    container: Container::Inventory,
                    column: (i % INV_COLS) as u8,
                    row: (i / INV_COLS) as u8,
                    type_raw: type_raw_of(code),
                    ..ItemHeader::default()
                },
                extended: Some(ItemExtended {
                    file_id,
                    ilvl: s.level.min(127) as u8,
                    quality: Quality::Normal,
                    graphic: None,
                    class_specific: None,
                    quality_data: QualityData::None,
                    runeword: None,
                    personalized_name: None,
                    unknown_pre_type_bit: 0,
                    type_data,
                    set_lists: None,
                    mods: Vec::new(), // basique : aucun affixe
                }),
                ear: None,
                children: Vec::new(),
            });
            file_id += 1;
        }
        save.character_items.declared_count = save.character_items.items.len() as u16;
        // Squelette de listes « retail 1.14d » (saves oracle 19a/19c) : le
        // jeu écrit TOUJOURS les quatre listes, même vides —
        // `JM+count / JM+count (corpse) / jf NU (merc, sans count) / kf+flag`.
        save.corpse_items = Some(d2d2s::ItemList { declared_count: 0, items: Vec::new() });
        save.merc_items = Some(d2d2s::ItemList { declared_count: 0, items: Vec::new() });
        save.merc_bare = true;

        Ok(save.to_bytes())
    }

    // -----------------------------------------------------------------------
    // Import — .d2s → save v3
    // -----------------------------------------------------------------------

    /// Importe un fichier `.d2s` (octets) dans le monde : remplace l'état du
    /// héros par ce que porte le fichier (cf. doc du module : mappé/ignoré).
    /// Parse STRICT (signature + checksum, comme le jeu) — `Err(descriptif)`
    /// sinon, état inchangé.
    pub fn import_d2s(&mut self, data: &[u8]) -> Result<D2sImportReport, String> {
        let save = SaveFile::parse(data).map_err(|e| format!(".d2s refusé : {e}"))?;

        // --- classe (héros + gabarit manifest) -----------------------------
        let code = hero_code_of_class(save.header.class).to_string();
        let class_changed = code != self.players[0].code;
        self.players[0].code = code.clone();
        self.hero_tpl = super::data::parse_hero_code(&self.manifest, &code);

        // --- stats mappées (÷256 pour hp/mana/stamina) ---------------------
        let st = &save.stats;
        let max_life = st.value(7).unwrap_or(0).max(1);
        let life = st.value(6).unwrap_or(max_life).clamp(1, max_life);
        let max_mana = st.value(9).unwrap_or(0);
        let mana = st.value(8).unwrap_or(max_mana).min(max_mana);
        let max_stamina = st.value(11).unwrap_or(0);
        let stamina = st.value(10).unwrap_or(max_stamina).min(max_stamina);
        let strength = st.value(0).unwrap_or(0);
        let dexterity = st.value(2).unwrap_or(0);
        let gold = st.value(14).unwrap_or(0);
        let skill_points = st.value(5).unwrap_or(0);
        // Points de stats non dépensés (stat 4 — mappé depuis 19c : les
        // saves retail les portent, le save v3 a le champ `stat_points`).
        let stat_points = st.value(4).unwrap_or(0);
        // Niveau plafonné au `MaxLvl` de la table XP réelle (99 en v1.12,
        // `world::max_level_of`) ; XP réalignée au seuil du niveau appliqué
        // (cohérence HUD/arbre) — l'XP brut du fichier n'est pas conservable
        // au-delà du plafond.
        let max_level = max_level_of(&self.players[0].code);
        let level = (save.header.level.max(1) as u32).clamp(1, max_level);
        let xp = xp_threshold(&self.players[0].code, level);
        let xp_to_next = if level >= max_level {
            0
        } else {
            xp_threshold(&self.players[0].code, level + 1) - xp
        };
        // Stats dérivées au gabarit de la classe importée (formules
        // `init_hero`) ; vitesse/potion du gabarit.
        let tpl = &self.hero_tpl;

        {
            let stats = &mut self.players[0].data.stats;
            stats.level = level;
            stats.xp = xp;
            stats.xp_to_next = xp_to_next;
            stats.strength = strength;
            stats.dexterity = dexterity;
            stats.max_life = max_life;
            stats.life = life;
            stats.max_mana = max_mana;
            stats.mana = mana;
            stats.max_stamina = max_stamina;
            stats.stamina = stamina;
            stats.gold = gold;
            stats.skill_points = skill_points;
            stats.stat_points = stat_points;
            stats.defense = dexterity / 4 + (tpl.shield_ac_min + tpl.shield_ac_max) / 2;
            stats.to_hit = tpl.to_hit_factor + dexterity;
            stats.walk_speed = tpl.walk_speed;
            stats.potion_heal = super::world::POTION_HEAL;
        }
        self.players[0].data.stamina_milli = (self.players[0].data.stats.stamina as i64) * 1000; // EXP-H2

        // --- compétences : slots → codes (offset classe) -------------------
        let base = Skills::class_slot_base(save.header.class) as u32;
        let tree = crate::skills_tree::SkillTreeData::global();
        let mut skills: Vec<crate::HeroSkill> = Vec::new();
        for slot in 0..30usize {
            let points = save.skills.level(slot);
            if points == 0 {
                continue;
            }
            let want = base + slot as u32;
            if let Some(rec) = tree.records().iter().find(|r| r.id == want && !r.class.is_empty())
            {
                skills.push(crate::HeroSkill {
                    code: rec.code.clone(),
                    level: points as u32,
                    missile: rec.srv_missile.clone(),
                });
            }
        }
        // Fichier sans compétence de classe ⇒ skills de départ de la classe
        // importée (jamais un héros sans attaque ; coins : vecteurs minimaux).
        self.players[0].data.skills = if skills.is_empty() {
            crate::skills::starting_skills(&code)
        } else {
            skills
        };
        let n_skills = self.players[0].data.skills.len();
        self.players[0].data.active_skill = crate::skills::default_active(&code);
        if let Some(sk) = self.players[0].data.skills.get(self.players[0].data.active_skill) {
            let (c, l) = (sk.code.clone(), sk.level);
            self.players[0].data.stats.skill = c;
            self.players[0].data.stats.skill_level = l;
        }

        // --- waypoints (bits 0..=38 de la difficulté active) ----------------
        let d_idx = save
            .header
            .difficulty
            .iter()
            .position(|d| d.active)
            .unwrap_or(self.difficulty.index());
        let mut n_wp = 0usize;
        if let Some(bits) = save.waypoints.difficulty(d_idx) {
            for slot in 0..39u8 {
                if bits.is_set(slot as usize) && crate::waypoints::by_slot(slot).is_some() {
                    self.activate_waypoint(slot);
                    n_wp += 1;
                }
            }
            // La difficulté du fichier prime (elle porte les bits lus).
            self.difficulty = match d_idx {
                1 => crate::Difficulty::Nightmare,
                2 => crate::Difficulty::Hell,
                _ => crate::Difficulty::Normal,
            };
        }

        // --- items : équipés d'abord, puis inventaire stocké ---------------
        // Réduits en basiques ([`crate::ItemDef`]) ; positions de grille
        // conservées pour le stocké, casés au premier slot libre pour les
        // équipés. Hors 40 slots / or / autres conteneurs / gemmes serties /
        // merc-corpse-golem ⇒ ignorés (doc du module).
        let mut inv = vec![None; super::world::INV_SLOTS];
        let mut loaded = 0usize;
        let mut ignored = 0usize;
        let mut ordered: Vec<&Item> = save.character_items.items.iter().collect();
        ordered.sort_by_key(|it| (it.header.location != Location::Equipped) as u8);
        for it in ordered {
            if it.extended.is_none() {
                ignored += 1; // items compact/oreille : rien à réduire
                continue;
            }
            let code = it.type_code();
            if code == "gld" || code.is_empty() {
                ignored += 1; // or : déjà porté par la stat 14
                continue;
            }
            let keep = it.header.location == Location::Equipped
                || (it.header.location == Location::Stored
                    && it.header.container == Container::Inventory);
            if !keep {
                ignored += 1; // ceinture / coffre / cube / socket parent…
                continue;
            }
            // Kind : mêmes règles que l'ex-`loot::base_of` (repli code) ;
            // nom : table embarquée sinon code majuscule.
            let fiche = base_fiche(&code);
            let kind = if code.starts_with("hp") {
                crate::ItemKind::Potion
            } else if d2d2s::codes::is_weapon(&code) {
                crate::ItemKind::Weapon
            } else if d2d2s::codes::is_armor(&code) {
                crate::ItemKind::Armor
            } else {
                crate::ItemKind::Misc
            };
            let name = fiche
                .map(|f| f.name)
                .unwrap_or_else(|| code.to_ascii_uppercase());
            let def = crate::ItemDef { code, name, kind, amount: 0 };
            let target = if it.header.location == Location::Stored {
                let col = it.header.column as usize;
                let row = it.header.row as usize;
                (col < INV_COLS && row < super::world::INV_ROWS).then_some(row * INV_COLS + col)
            } else {
                None // équipé : premier slot libre (placé en ordre)
            };
            let placed = match target {
                Some(idx) if inv[idx].is_none() => {
                    inv[idx] = Some(def);
                    true
                }
                _ => match inv.iter().position(|s| s.is_none()) {
                    Some(free) => {
                        inv[free] = Some(def);
                        true
                    }
                    None => false, // inventaire plein
                },
            };
            if placed {
                loaded += 1;
            } else {
                ignored += 1;
            }
        }
        self.players[0].data.inv = inv;

        // --- héros vivant (un import relève) — position/monde inchangés ----
        self.players[0].data.dead_ms = 0;
        self.players[0].data.swing_ms = 0;
        self.players[0].data.path.clear();
        self.players[0].ent.anim = crate::AnimState::NU;
        self.players[0].ent.anim_frame = 0;

        Ok(D2sImportReport {
            name: save.header.name(),
            class: code,
            class_changed,
            level: self.players[0].data.stats.level,
            strength: self.players[0].data.stats.strength,
            dexterity: self.players[0].data.stats.dexterity,
            gold: self.players[0].data.stats.gold,
            skills: n_skills,
            waypoints: n_wp,
            difficulty: self.difficulty.as_str().to_string(),
            items_loaded: loaded,
            items_ignored: ignored,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests — checksum, vecteur réel, round-trip import→export→import stable
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use d2d2s::EquipSlot;
    use serde_json::json;

    /// Vecteur SYNTHÉTIQUE minimal de d2d2s (checksum valide, 9 stats,
    /// 0 item) — un vrai format v0x60, PAS produit par notre export.
    const VECTOR_MINIMAL: &[u8] = include_bytes!("../../d2d2s/tests/vectors/minimal_v0x60.d2s");

    fn world(hero: &str) -> World {
        // Manifest minimal portant les 3 classes utilisées par les tests —
        // stats par défaut (55 vie / 15 mana / 30 str / 20 dex).
        let manifest = json!({ "heroes": { "SOR": {}, "BA": {}, "PA": {} } });
        let grid = vec![vec![false; 20]; 20];
        World::with_seed_and_hero(grid, manifest, 42, hero)
    }

    fn tweak(w: &mut World) {
        w.players[0].data.stats.level = 3;
        w.players[0].data.stats.xp = xp_threshold("SOR", 3);
        w.players[0].data.stats.xp_to_next = xp_threshold("SOR", 4) - xp_threshold("SOR", 3);
        w.players[0].data.stats.strength = 40;
        w.players[0].data.stats.dexterity = 25;
        w.players[0].data.stats.life = 44;
        w.players[0].data.stats.max_life = 61;
        w.players[0].data.stats.mana = 9;
        w.players[0].data.stats.max_mana = 17;
        w.players[0].data.stats.gold = 123;
        w.players[0].data.stats.skill_points = 2;
        w.players[0].data.skills = vec![crate::HeroSkill {
            code: "Fire Bolt".into(),
            level: 2,
            missile: Some("firebolt".into()),
        }];
        w.players[0].data.active_skill = 0;
        w.activate_waypoint(0); // Rogue Encampment (retail)
        w.activate_waypoint(39); // Blood Moor (MVP — non exportable)
        w.players[0].data.inv[12] = Some(crate::ItemDef {
            code: "hp1".into(),
            name: "Lesser Healing Potion".into(),
            kind: crate::ItemKind::Potion,
            amount: 0,
        });
        w.players[0].data.inv[23] = Some(crate::ItemDef {
            code: "hax".into(),
            name: "Hand Axe".into(),
            kind: crate::ItemKind::Weapon,
            amount: 0,
        });
    }

    #[test]
    fn export_produces_valid_signature_checksum_and_content() {
        let mut w = world("SOR");
        tweak(&mut w);
        let bytes = w.export_d2s("Testchar").expect("export");
        // Signature + checksum STRICTS (comportement du jeu).
        let save = SaveFile::parse(&bytes).expect("checksum/signature valides");
        assert_eq!(save.header.name(), "Testchar");
        assert_eq!(save.header.class, Class::Sorceress);
        assert_eq!(save.header.level, 3);
        assert_eq!(save.stats.value(0), Some(40), "strength");
        assert_eq!(save.stats.value(2), Some(25), "dexterity");
        assert_eq!(save.stats.value(14), Some(123), "gold");
        assert_eq!(save.stats.value(7), Some(61), "maxhp");
        // Checksum explicite sur les octets (champ neutralisé dans le calcul).
        let stored = u32::from_le_bytes([bytes[0x0C], bytes[0x0D], bytes[0x0E], bytes[0x0F]]);
        assert_eq!(stored, d2d2s::compute_checksum(&bytes));
        // Skills : Fire Bolt (Id 36) → slot 0 de la sorcière (base 36).
        assert_eq!(save.skills.level(0), 2);
        // Waypoint slot 0 actif en Normal, slot MVP 39 absent.
        assert_eq!(save.waypoints.is_active(0, "Rogue Encampment"), Some(true));
        // Items : hp1 (case 12) + hax (case 23), identifiés, basiques.
        assert_eq!(save.character_items.items.len(), 2);
        let axe = save
            .character_items
            .items
            .iter()
            .find(|i| i.type_code() == "hax")
            .expect("hax exporté");
        assert!(axe.header.identified);
        assert_eq!(axe.header.column, 3);
        assert_eq!(axe.header.row, 2);
        assert!(axe.extended.as_ref().unwrap().mods.is_empty());
        // Difficulté : Normal actif.
        assert!(save.header.difficulty[0].active && !save.header.difficulty[1].active);
        // Déterminisme : deux exports successifs = mêmes octets.
        assert_eq!(w.export_d2s("Testchar").unwrap(), bytes);
    }

    #[test]
    fn export_sanitizes_names() {
        let w = world("SOR");
        let bytes = w.export_d2s("Jôké-Ø_Name!").unwrap();
        let save = SaveFile::parse(&bytes).unwrap();
        assert_eq!(save.header.name(), "JkName", "ASCII alphanumérique seul");
        // Nom vide → nom de classe anglais.
        let bytes = w.export_d2s("!!").unwrap();
        assert_eq!(SaveFile::parse(&bytes).unwrap().header.name(), "Sorceress");
    }

    #[test]
    fn import_maps_vector_minimal_and_documented_limits() {
        let mut w = world("SOR");
        let report = w.import_d2s(VECTOR_MINIMAL).expect("vecteur minimal importé");
        // Le vecteur : str 20, level 12, gold 1000, 0 item. Nom informatif
        // (non mappé), niveau plafonné au `MaxLvl` de la table (99 — le
        // vecteur 12 passe tel quel).
        assert_eq!(report.name, "Testchar");
        assert_eq!(w.players[0].data.stats.strength, 20);
        assert_eq!(w.players[0].data.stats.gold, 1000);
        assert_eq!(w.players[0].data.stats.level, max_level_of("SOR").min(12), "plafonné au MaxLvl");
        assert_eq!(report.items_loaded, 0);
        // Le rapport reflète la classe appliquée (cohérence header).
        let class = SaveFile::parse(VECTOR_MINIMAL).unwrap().header.class;
        assert_eq!(report.class, hero_code_of_class(class));
        // Le save JSON v3 reste consommable (contrat principal intact).
        let w2 = World::load(&w.save()).expect("save v3 valide après import");
        assert_eq!(w2.hero_stats().gold, 1000);
    }

    #[test]
    fn import_rejects_corrupt_files_without_touching_state() {
        let mut w = world("SOR");
        let before = w.save();
        assert!(w.import_d2s(b"courte").is_err());
        assert!(w.import_d2s(&[]).is_err());
        // Mauvais checksum : refus strict (le jeu fait de même).
        let mut w2 = world("SOR");
        let mut bytes = w2.export_d2s("X").unwrap();
        bytes[0x30] ^= 0xFF; // corrompt le timestamp → checksum invalide
        assert!(w2.import_d2s(&bytes).is_err(), "checksum invalide refusé");
        assert_eq!(w.save(), before, "état inchangé sur refus");
    }

    #[test]
    fn round_trip_import_export_import_is_stable() {
        // A → export b1 → B.import(b1) → b2 = B.export → C.import(b2)
        // : B et C portent EXACTEMENT le même état mappé, et l'export de B
        // redonne b1 octet pour octet (format d'échange stable).
        let mut a = world("SOR");
        tweak(&mut a);
        let b1 = a.export_d2s("Rondeau").unwrap();

        let mut b = world("SOR");
        let report = b.import_d2s(&b1).unwrap();
        assert_eq!(report.class, "SOR");
        assert!(!report.class_changed);
        assert_eq!(report.items_loaded, 2);
        assert_eq!(report.waypoints, 1, "slot 0 ; le 39 MVP est hors retail");

        // État mappé de B.
        let s = b.hero_stats();
        assert_eq!((s.level, s.strength, s.dexterity), (3, 40, 25));
        assert_eq!((s.max_life, s.life), (61, 44));
        assert_eq!((s.max_mana, s.mana), (17, 9));
        assert_eq!(s.gold, 123);
        assert_eq!(s.skill_points, 2);
        assert_eq!(s.xp, xp_threshold("SOR", 3));
        assert_eq!(b.active_waypoints(), &[0], "le slot 39 n'est pas exportable");
        assert_eq!(
            b.hero_skills(),
            &[crate::HeroSkill { code: "Fire Bolt".into(), level: 2, missile: Some("firebolt".into()) }]
        );
        assert_eq!(b.inventory()[12].as_ref().unwrap().code, "hp1");
        assert_eq!(b.inventory()[23].as_ref().unwrap().code, "hax");

        // B → export → C : même état mappé, octets identiques.
        let b2 = b.export_d2s("Rondeau").unwrap();
        assert_eq!(b2, b1, "import→export stable octet pour octet");
        let mut c = world("SOR");
        c.import_d2s(&b2).unwrap();
        assert_eq!(c.players[0].code(), b.players[0].code());
        assert_eq!(format!("{:?}", c.hero_stats()), format!("{:?}", b.hero_stats()));
        assert_eq!(c.active_waypoints(), b.active_waypoints());
        assert_eq!(c.hero_skills(), b.hero_skills());
        assert_eq!(c.inventory(), b.inventory());
        assert_eq!(c.difficulty(), b.difficulty());
    }

    #[test]
    fn import_changes_class_and_skills_from_real_slots() {
        // Un .d2s barbare (classe 4, skills slots décalés base 126) : la
        // classe ET le gabarit manifest suivent, les skills se mappent par
        // Id skills.txt.
        let mut src = world("BA");
        src.players[0].data.skills = vec![crate::HeroSkill { code: "Bash".into(), level: 4, missile: None }];
        src.players[0].data.stats.level = 5;
        src.players[0].data.stats.xp = xp_threshold("BA", 5);
        let bytes = src.export_d2s("Barbu").unwrap();

        let mut dst = world("SOR"); // sorcière à l'origine
        let report = dst.import_d2s(&bytes).unwrap();
        assert!(report.class_changed);
        assert_eq!(report.class, "BA");
        assert_eq!(dst.players[0].code(), "BA");
        assert_eq!(dst.hero_tpl.strength, 30, "gabarit barbare du manifest");
        assert_eq!(
            dst.hero_skills(),
            &[crate::HeroSkill { code: "Bash".into(), level: 4, missile: None }]
        );
        assert_eq!(dst.hero_stats().level, 5);
    }

    #[test]
    fn import_places_equipped_first_then_stored_at_grid_position() {
        // Construction directe d'un SaveFile : 1 équipé (arme main droite) +
        // 1 stocké (cap case 5,0) → l'équipé prend le premier slot libre,
        // le cap reste à sa case 5.
        let mut save = SaveFile::default_v112();
        save.header.set_name("Equiper");
        save.header.class = Class::Paladin;
        save.stats.set(0, 25);
        // TypeData COHÉRENTS avec les codes (le parseur d2d2s lit les champs
        // durability/defense selon la classification armor/weapons réelle) —
        // un vrai fichier est toujours construit ainsi.
        let mk = |code: [u8; 4], equip: bool| {
            let is_weapon = &code == b"hax ";
            Item {
                header: ItemHeader {
                    location: if equip { Location::Equipped } else { Location::Stored },
                    equip_slot: if equip { EquipSlot::WEAPON_RIGHT } else { EquipSlot(0) },
                    column: 5,
                    row: 0,
                    type_raw: code,
                    ..ItemHeader::default()
                },
                extended: Some(ItemExtended {
                    file_id: 7,
                    ilvl: 1,
                    quality: Quality::Normal,
                    graphic: None,
                    class_specific: None,
                    quality_data: QualityData::None,
                    runeword: None,
                    personalized_name: None,
                    unknown_pre_type_bit: 0,
                    type_data: if is_weapon {
                        TypeData::Weapon { max_durability: 10, durability: 10, sockets: None }
                    } else {
                        TypeData::Armor {
                            defense_stored: 8,
                            max_durability: 12,
                            durability: 12,
                            sockets: None,
                        }
                    },
                    set_lists: None,
                    mods: Vec::new(),
                }),
                ear: None,
                children: Vec::new(),
            }
        };
        save.character_items.items.push(mk(*b"hax ", true));
        save.character_items.items.push(mk(*b"cap ", false));
        save.character_items.declared_count = 2;
        let bytes = save.to_bytes();

        let mut w = world("SOR");
        let report = w.import_d2s(&bytes).unwrap();
        assert_eq!(report.items_loaded, 2);
        assert_eq!(report.class, "PA", "Paladin → code manifest PA");
        let inv = w.inventory();
        assert_eq!(inv[0].as_ref().unwrap().code, "hax", "équipé → slot libre 0");
        assert_eq!(inv[5].as_ref().unwrap().code, "cap", "stocké → sa case");
        assert_eq!(inv[5].as_ref().unwrap().kind, crate::ItemKind::Armor);
        assert!(
            inv[5].as_ref().unwrap().name.starts_with("Cap"),
            "nom réel de la table : {}",
            inv[5].as_ref().unwrap().name
        );
    }

    #[test]
    fn import_revives_dead_hero() {
        let bytes = world("SOR").export_d2s("Reviens").unwrap();
        let mut w2 = world("SOR");
        w2.players[0].data.dead_ms = 900;
        w2.players[0].ent.anim = crate::AnimState::DD;
        w2.import_d2s(&bytes).unwrap();
        assert_eq!(w2.hero().anim, crate::AnimState::NU, "héros relevé");
        assert_eq!(w2.hero_stats().life, w2.hero_stats().max_life.max(1));
    }
}
