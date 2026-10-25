//! Lecture tolérante de `assets/manifest.json` (JSON strict à 4 clés racine).
//! Champs absents ⇒ valeurs par défaut cohérentes niveau 1 (jamais de panic).

use serde_json::Value;

/// Gabarit héros (colonnes `patch_charstats.txt`, ligne Barbarian).
/// 3b : `Clone` — les coéquipiers copient leur gabarit de classe.
#[derive(Clone)]
pub(crate) struct HeroTpl {
    /// Vie niveau 1 (`hpadd` + `vit` = 55).
    pub life: u32,
    /// Mana niveau 1 (`int` + 5 = 15).
    pub mana: u32,
    /// Endurance niveau 1 (EXP-H2, col `stamina`, BA = 92).
    pub stamina: u32,
    /// Vitesse de marche en tuiles/s (`WalkVelocity` = 6 yards/s).
    pub walk_speed: f32,
    /// Vitesse de course en tuiles/s (`RunVelocity` = 9 yards/s ; absent ⇒
    /// walk × 1,5 comme le brief moteur).
    pub run_speed: f32,
    /// Force (`str` = 30).
    pub strength: u32,
    /// Dextérité (`dex` = 20).
    pub dexterity: u32,
    /// Facteur de toucher (`ToHitFactor` = 20).
    pub to_hit_factor: u32,
    /// Vie gagnée par niveau (`LifePerLevel` 8/4 = +2).
    pub life_per_level: u32,
    /// Mana gagné par niveau (`ManaPerLevel` 4/4 = +1).
    pub mana_per_level: u32,
    /// Dégâts min arme de départ (`item1` = hax, `mindam` = 3).
    pub weapon_min: u32,
    /// Dégâts max arme de départ (`maxdam` = 6).
    pub weapon_max: u32,
    /// AC min bouclier de départ (`item2` = buc, `minac` = 4).
    pub shield_ac_min: u32,
    /// AC max bouclier (`maxac` = 6).
    pub shield_ac_max: u32,
}

/// Gabarit monstre (colonnes `patch_monstats.txt`, valeurs brutes non scalées).
#[derive(Clone)]
pub(crate) struct MonTpl {
    /// Code table (`FA`, `ZM`, `SI`).
    pub code: String,
    /// Nom affichable (réservé HUD/labels, non lu par les règles MVP).
    #[allow(dead_code)]
    pub name: String,
    /// Niveau (`Level` = 1 au MVP).
    pub level: u32,
    /// PV min bruts (`minHP`).
    pub hp_min: u32,
    /// PV max bruts (`maxHP`).
    pub hp_max: u32,
    /// Dégâts min bruts (`A1MinD`).
    pub dmg_min: u32,
    /// Dégâts max bruts (`A1MaxD`).
    pub dmg_max: u32,
    /// XP brute (`Exp`).
    pub exp: u32,
    /// Vitesse marche tuiles/s (`Velocity`).
    pub speed: f32,
    /// Classe de trésor drop (`stats.treasureClass`, ex. "Act 1 H2H A") ;
    /// vide ⇒ repli MVP dans `roll_drop`.
    pub treasure_class: String,
}

fn u32_at(v: &Value, ptr: &str, d: u32) -> u32 {
    v.pointer(ptr).and_then(|x| x.as_u64()).map(|x| x as u32).unwrap_or(d)
}

fn f32_at(v: &Value, ptr: &str, d: f32) -> f32 {
    v.pointer(ptr).and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(d)
}

fn str_at(v: &Value, ptr: &str, d: &str) -> String {
    v.pointer(ptr).and_then(|x| x.as_str()).unwrap_or(d).to_string()
}

/// Résout le code héros EFFECTIF : `heroes[CODE]` si l'objet `heroes` existe et
/// contient le code (insensible à la casse), sinon le bloc unique `hero`
/// (rétro-compat manifest à héros seul) — sa classe `hero.class`, défaut "BA".
pub(crate) fn resolve_hero(v: &Value, requested: &str) -> String {
    let up = requested.trim().to_ascii_uppercase();
    if let Some(heroes) = v.get("heroes").and_then(|h| h.as_object()) {
        if heroes.contains_key(&up) {
            return up;
        }
    }
    v.pointer("/hero/class")
        .and_then(|c| c.as_str())
        .unwrap_or("BA")
        .to_string()
}

/// 3b-1 : comme [`parse_hero_code`] mais renvoie `None` si la classe
/// résolue est inconnue du manifest — connu = `heroes[CODE]` existe, ou le
/// manifest n'a PAS d'objet `heroes` (le bloc unique `hero` fait alors foi
/// pour l'unique classe du monde). Utilisé par `add_player`/`add_cohero_near`
/// (refus propre, sans muter l'état).
pub(crate) fn parse_hero_code_checked(v: &Value, resolved: &str) -> Option<HeroTpl> {
    match v.get("heroes").and_then(|h| h.as_object()) {
        Some(heroes) if heroes.contains_key(resolved) => Some(parse_hero_code(v, resolved)),
        Some(_) => None, // objet heroes présent mais code absent : classe inconnue
        None => Some(parse_hero_code(v, resolved)), // monde à héros unique
    }
}

/// Parse les stats du héros `code` : `heroes[code]/stats` si présent, sinon le
/// bloc `hero/stats` (rétro-compat). Champs absents ⇒ défauts barbare niv 1.
pub(crate) fn parse_hero_code(v: &Value, code: &str) -> HeroTpl {    let has = v
        .get("heroes")
        .and_then(|h| h.as_object())
        .is_some_and(|h| h.contains_key(code));
    let base = if has { format!("/heroes/{code}/stats") } else { "/hero/stats".into() };
    HeroTpl {
        life: u32_at(v, &format!("{base}/life"), 55),
        mana: u32_at(v, &format!("{base}/mana"), 15),
        stamina: u32_at(v, &format!("{base}/stamina"), 92),
        walk_speed: f32_at(v, &format!("{base}/walkSpeed"), 6.0),
        run_speed: {
            let walk = f32_at(v, &format!("{base}/walkSpeed"), 6.0);
            f32_at(v, &format!("{base}/runSpeed"), walk * 1.5)
        },
        strength: u32_at(v, &format!("{base}/strength"), 30),
        dexterity: u32_at(v, &format!("{base}/dexterity"), 20),
        to_hit_factor: u32_at(v, &format!("{base}/toHitFactor"), 20),
        life_per_level: u32_at(v, &format!("{base}/lifePerLevel"), 2),
        mana_per_level: u32_at(v, &format!("{base}/manaPerLevel"), 1),
        weapon_min: u32_at(v, &format!("{base}/startWeapon/dmgMin"), 3),
        weapon_max: u32_at(v, &format!("{base}/startWeapon/dmgMax"), 6),
        shield_ac_min: u32_at(v, &format!("{base}/startShield/acMin"), 4),
        shield_ac_max: u32_at(v, &format!("{base}/startShield/acMax"), 6),
    }
}

/// Parse le tableau `monsters` du manifest (vide ⇒ aucun monstre auto-spawné).
pub(crate) fn parse_monsters(v: &Value) -> Vec<MonTpl> {
    let mut out = Vec::new();
    if let Some(arr) = v.pointer("/monsters").and_then(|m| m.as_array()) {
        for m in arr {
            out.push(MonTpl {
                code: str_at(m, "/code", "???"),
                name: str_at(m, "/name", "???"),
                level: u32_at(m, "/stats/level", 1).max(1),
                hp_min: u32_at(m, "/stats/hpMin", 21),
                hp_max: u32_at(m, "/stats/hp", 61).max(u32_at(m, "/stats/hpMin", 21)),
                dmg_min: u32_at(m, "/stats/dmgMin", 51),
                dmg_max: u32_at(m, "/stats/dmgMax", 101).max(u32_at(m, "/stats/dmgMin", 51)),
                exp: u32_at(m, "/stats/exp", 61),
                speed: f32_at(m, "/stats/speed", 3.0),
                treasure_class: str_at(m, "/stats/treasureClass", ""),
            });
        }
    }
    out
}
