//! Table réelle `assets/patch_missiles.txt` (v1.12, 684 lignes) — extraite
//! des colonnes `Missile/Id/Vel/Range/HitClass/EType/CelFile/AnimLen`, puis
//! EType complété par `assets/patch_skills.txt` (colonne `EType` du skill
//! porteur pour firebolt/icebolt/chargedbolt/magicarrow/firearrow/icearrow).
//!
//! Embarquée en compile-time (`include_str!`) : zéro IO runtime, comme le
//! manifest réel des tests `d2rules`. Dégâts :
//! * carriers `Weapon` (arrow, bolt, javelin, throwaxe, firearrow, icearrow) :
//!   la flèche transporte les dégâts de l'arme (règle D2 ; colonnes `SrcDmg`
//!   vides dans missiles.txt) — résolus par l'appelant au tir.
//! * carriers `Skill` : dégâts officiels de la compétence niveau 1
//!   (firebolt 3–8 feu, icebolt 3–7 froid, chargedbolt 2–6 foudre,
//!   magicarrow +1 magique) — les colonnes `MinDam/MaxDam` de
//!   `patch_skills.txt` sont vides en v1.12 (calculées par le code interne
//!   des niveaux ; valeurs canoniques de l'arbre de compétences).

/// JSON de la table réelle (10 définitions MVP).
pub(crate) const REAL_MISSILES_JSON: &str = include_str!("../data/missiles_real.json");

use crate::{DamageType, DmgSrc, MissileDef};

/// Parse la table ; `Err` = structure inattendue (pas de panic).
pub(crate) fn parse_table_err(v: &serde_json::Value) -> Result<Vec<MissileDef>, String> {
    let arr = v
        .get("missiles")
        .and_then(|m| m.as_array())
        .ok_or_else(|| "missiles: clé 'missiles' (tableau) absente".to_string())?;
    let mut out = Vec::with_capacity(arr.len());
    for m in arr {
        let id = m
            .get("id")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "missiles: entrée sans 'id'".to_string())?
            .to_ascii_lowercase();
        let def = MissileDef {
            id,
            vel: num(m, "vel", 24.0),
            range: num(m, "range", 40.0),
            hit_class: m.get("hitClass").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            elem: m.get("elem").and_then(|x| x.as_str()).map(DamageType::parse).unwrap_or_default(),
            dmg_min: m.get("dmgMin").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            dmg_max: m.get("dmgMax").and_then(|x| x.as_u64()).unwrap_or(0) as u32,
            dmg_src: match m.get("dmgSrc").and_then(|x| x.as_str()) {
                Some("weapon") => DmgSrc::Weapon,
                _ => DmgSrc::Skill,
            },
            cel_file: m.get("celFile").and_then(|x| x.as_str()).unwrap_or("").to_string(),
            anim_len: m.get("animLen").and_then(|x| x.as_u64()).unwrap_or(1).max(1) as u32,
        };
        out.push(def);
    }
    Ok(out)
}

fn num(m: &serde_json::Value, key: &str, d: f32) -> f32 {
    m.get(key).and_then(|x| x.as_f64()).map(|x| x as f32).unwrap_or(d)
}
