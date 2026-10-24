//! Overlays d'états + sprites de missiles — étape 15c (PLAN-FIDELITE,
//! arbre `d2w-sprites`). Data-driven depuis les tables v1.12 réelles
//! `overlay.txt` (294 rec : nom d'overlay → fichier de sprite) et
//! `states.txt` (183 rec : état → overlays + teinte), embarquées EN L'ÉTAT.
//!
//! # Ce qui est branché sur les états ACTIFS du moteur
//! * **poison** — `d2rules::combat_mods` pose `Dot` poison sur les monstres
//!   (`world.rs` 12c). La table `states.txt` ligne `poison` ne référence
//!   AUCUN overlay : l'état retail est un **colorshift** (colonne
//!   `colorshift`=104, lumière 128,255,128) ⇒ teinte verte de l'entité via
//!   [`shift_palette`] (palshift simple, cf. mission).
//! * **chill/froid** — idem : ligne `cold` (`colorshift`=108, lumière
//!   150,215,255) ⇒ teinte bleue. La ligne `freeze` porte la même teinte.
//! * **buffs de sanctuaires** — `d2rules::shrines::ShrineBuff` (codes 6-15
//!   de `shrines.txt`) ↔ lignes `shrine_*` de `states.txt` (ids 128-137),
//!   chacune référencant son overlay DC animé (`shrine_armor`,
//!   `shrine_shimmer00`…) : rendu additif (`Trans`=3) au-dessus de l'entité
//!   (`PreDraw`=0), ancré pieds.
//! * **auras (à venir)** — lignes `resistfire/resistcold/resistlight/
//!   resistall` → overlays `aura_*` (20 frames, `PreDraw`=1 = SOUS l'entité,
//!   teinte RGB de la table : feu orangé 255,193,103…).
//!
//! # Piège v1.12 PROUVÉ (sonde hash 4 MPQ, 2026-10-24)
//! L'installeur stocke ces sprites en **DCC** dans `d2data.mpq`
//! (`data\global\overlays\ShrineArmor.dcc`, `data\global\missiles\
//! Firebolt.dcc`) — les `.dc6` de même nom sont absents de
//! patch_d2/d2exp/d2data/d2char. Le décodage passe donc par `d2dcc` (déjà
//! dépendance du crate), palette `pal_act1_pl2.dat` (display RGB, `bgr=false`
//! — cf. `sprite::sprite_palette`).
//!
//! # Conventions de rendu (miroir du moteur)
//! * **fps** = colonne `AnimRate` (8 sanctuaires, 16 auras) — simplification
//!   documentée ; le clamp [`frame_at`] boucle sur `Frames` de la table
//!   (15/20) sans dépasser les frames du DCC (17/20 décodées).
//! * **missiles** — frame bouclée à 25 fps (tick D2) sur les frames du DCC
//!   (Arrow 32 dirs × 1, Firebolt 16 × 5, ChargedBolt 16 × 10…) ; direction
//!   = [`dir64_from`] (16 pas, MÊME convention que `d2rules::world::
//!   facing_from` — prouvé égal aux 8 points cardinaux par golden) puis
//!   `dir64_to_dcc` (tables Necrolis, identiques aux entités).
//! * **ancre** — overlays : bas-centre de la box de direction sur les pieds
//!   `(sx, sy)` ; missiles : centre de la box sur le point de trajectoire.
//!
//! Zéro IO : texte TSV et octets DCC arrivent en `&str`/`&[u8]` (embed des
//! tests, asset provider runtime). Zéro panique : décodages en `Result`/`Option`.

use d2dcc::render::{composite_indices, indices_to_rgba, Rect};
use d2dcc::render::Palette;
use d2dcc::{dir64_to_dcc, Dcc};
use d2rules::tsv::Tsv;
use std::collections::HashMap;

/// Tables v1.12 réelles embarquées (copies `d2w-re/tables/items-ui`, MAJ
/// extracteur 15c). LECTURE SEULE à la source — ces includes sont les copies
/// validées de l'arbre (`assets/overlay.txt`, `assets/states.txt`).
pub const OVERLAY_TSV: &str = include_str!("../../../../assets/overlay.txt");
pub const STATES_TSV: &str = include_str!("../../../../assets/states.txt");

/// État poison (monstres empoisonnés, DoT 12c) — teinte verte, zéro DC.
pub const STATE_POISON: &str = "poison";
/// État froid (monstres refroidis, `chill_ms`) — teinte bleue, zéro DC.
pub const STATE_CHILL: &str = "cold";
/// État gel complet (`freeze`) — même teinte que `cold` dans la table.
pub const STATE_FREEZE: &str = "freeze";

/// Une ligne de `overlay.txt` (champs utiles au rendu ; `v1.12`, CRLF Excel).
#[derive(Debug, Clone, PartialEq)]
pub struct OverlayDef {
    /// Nom table (clé référencée par `states.txt`, ex. `shrine_armor`).
    pub name: String,
    /// Fichier sprite v1.12 (`data\global\overlays\<file>` — DCC ici).
    pub file: String,
    /// Colonne `Frames` (durée d'un cycle joué par le jeu).
    pub frames: u32,
    /// Colonne `PreDraw` : `true` = dessiné SOUS l'entité (derrière).
    pub pre_draw: bool,
    /// Colonne `Trans` : 3 = additif (tous les overlays d'états du moteur).
    pub trans: u32,
    /// Colonnes `Red/Green/Blue` (/255) — teinte multiplicative du glow.
    pub rgb: [f32; 3],
    /// Colonne `AnimRate` — fps de lecture (simplification documentée).
    pub anim_rate: u32,
}

/// Une ligne de `states.txt` (champs utiles : overlays + teinte).
#[derive(Debug, Clone, PartialEq)]
pub struct StateDef {
    /// Nom de l'état (`poison`, `cold`, `shrine_armor`…).
    pub name: String,
    /// Colonnes `overlay1-4` résolues (vides et `null` retirés).
    pub overlays: Vec<String>,
    /// Colonne `colorshift` (104 poison, 108 froid — 0 = aucun).
    pub colorshift: i64,
    /// Colonnes `light-r/g/b` (/255) — la teinte du colorshift.
    pub light: Option<[f32; 3]>,
}

/// Tables parsées : `overlay.txt` + `states.txt` (parse lenient `d2rules::tsv`).
#[derive(Debug, Clone, Default)]
pub struct OverlayTable {
    overlays: HashMap<String, OverlayDef>,
    states: HashMap<String, StateDef>,
}

impl OverlayTable {
    /// Parse les deux tables. Erreur seulement si un en-tête manque
    /// (tables embarquées validées par golden — jamais attendu).
    pub fn parse(overlay_tsv: &str, states_tsv: &str) -> Result<OverlayTable, d2rules::tsv::TsvError> {
        let ov = Tsv::parse(overlay_tsv)?;
        let mut overlays = HashMap::new();
        for r in 0..ov.len() {
            let rec = ov.rec(r).expect("index < len");
            let name = rec.s("overlay");
            if name.is_empty() {
                continue;
            }
            overlays.insert(
                name.to_string(),
                OverlayDef {
                    name: name.to_string(),
                    file: rec.s("Filename").to_string(),
                    frames: rec.u("Frames"),
                    pre_draw: rec.i("PreDraw") != 0,
                    trans: rec.u("Trans"),
                    rgb: [
                        rec.i("Red").clamp(0, 255) as f32 / 255.0,
                        rec.i("Green").clamp(0, 255) as f32 / 255.0,
                        rec.i("Blue").clamp(0, 255) as f32 / 255.0,
                    ],
                    anim_rate: rec.u("AnimRate").max(1),
                },
            );
        }
        let st = Tsv::parse(states_tsv)?;
        let mut states = HashMap::new();
        for r in 0..st.len() {
            let rec = st.rec(r).expect("index < len");
            let name = rec.s("state");
            if name.is_empty() {
                continue;
            }
            let ovs = [rec.s("overlay1"), rec.s("overlay2"), rec.s("overlay3"), rec.s("overlay4")]
                .into_iter()
                .filter(|s| !s.is_empty() && *s != "null")
                .map(|s| s.to_string())
                .collect();
            let (lr, lg, lb) = (rec.i("light-r"), rec.i("light-g"), rec.i("light-b"));
            states.insert(
                name.to_string(),
                StateDef {
                    name: name.to_string(),
                    overlays: ovs,
                    colorshift: rec.i("colorshift") as i64,
                    light: if lr == 0 && lg == 0 && lb == 0 {
                        None
                    } else {
                        Some([
                            lr.clamp(0, 255) as f32 / 255.0,
                            lg.clamp(0, 255) as f32 / 255.0,
                            lb.clamp(0, 255) as f32 / 255.0,
                        ])
                    },
                },
            );
        }
        Ok(OverlayTable { overlays, states })
    }

    /// Tables embarquées v1.12 (le cas normal).
    pub fn embedded() -> Result<OverlayTable, d2rules::tsv::TsvError> {
        Self::parse(OVERLAY_TSV, STATES_TSV)
    }

    /// Définition d'overlay par nom table.
    pub fn overlay(&self, name: &str) -> Option<&OverlayDef> {
        self.overlays.get(name)
    }

    /// Définition d'état par nom.
    pub fn state(&self, name: &str) -> Option<&StateDef> {
        self.states.get(name)
    }

    /// Overlays d'un état, résolus dans l'ordre `overlay1..4` (les lignes
    /// sans overlay — poison, froid — renvoient une tranche vide : l'état
    /// est une TEINTE, cf. [`OverlayTable::state_tint`]).
    pub fn state_overlays(&self, state: &str) -> Vec<&OverlayDef> {
        self.states
            .get(state)
            .map(|s| s.overlays.iter().filter_map(|n| self.overlays.get(n)).collect())
            .unwrap_or_default()
    }

    /// Teinte d'un état colorshift-only ([`STATE_POISON`] vert,
    /// [`STATE_CHILL`]/[`STATE_FREEZE`] bleu) — `None` si l'état n'en porte
    /// pas (les états à overlays DC, sanctuaires/auras).
    pub fn state_tint(&self, state: &str) -> Option<[f32; 3]> {
        self.states.get(state).and_then(|s| s.light)
    }

    /// Nombre d'états/overlays parsés (diagnostics + goldens).
    pub fn len(&self) -> (usize, usize) {
        (self.states.len(), self.overlays.len())
    }
}

/// États `shrine_*` de `states.txt` (ids 128-137) pour les codes buff de
/// `shrines.txt` ACTIFS côté moteur (`ShrineBuff`, codes 6-15). Le code 1
/// (Recharge) est instantané — pas de buff, pas d'overlay.
pub fn state_for_shrine(code: u8) -> Option<&'static str> {
    match code {
        6 => Some("shrine_armor"),
        7 => Some("shrine_combat"),
        8 => Some("shrine_resist_fire"),
        9 => Some("shrine_resist_cold"),
        10 => Some("shrine_resist_lightning"),
        11 => Some("shrine_resist_poison"),
        12 => Some("shrine_skill"),
        13 => Some("shrine_mana_regen"),
        14 => Some("shrine_stamina"),
        15 => Some("shrine_experience"),
        _ => None,
    }
}

/// Frame d'une boucle : `t` ms à `fps` images/s, clampé au nombre de frames
/// jouables (table `Frames` bridée aux frames réellement décodées).
pub fn frame_at(t_ms: u64, fps: u32, frames: usize) -> usize {
    if frames == 0 {
        return 0;
    }
    let fps = fps.max(1) as u64;
    ((t_ms / 1000 * fps + (t_ms % 1000) * fps / 1000) as usize) % frames
}

/// Frame d'un missile en vol : boucle 25 fps (tick D2) sur les frames DCC.
pub fn missile_frame_at(t_ms: u64, frames: usize) -> usize {
    frame_at(t_ms, 25, frames)
}

/// Direction D2 64 pas (16 UTILISÉS, pas de 22,5°) depuis un vecteur écran
/// `(dx, dy)` (y vers le bas) — MÊME convention que
/// `d2rules::world::facing_from` (`oct = round((deg-90)/45)`, sud = 0) :
/// `round((deg-90)/22,5) × 2` coïncide avec `facing_from × 4` aux 8 points
/// cardinaux (golden `dir64_matches_world_facing`).
pub fn dir64_from(dx: f32, dy: f32) -> usize {
    if dx == 0.0 && dy == 0.0 {
        return 0;
    }
    let deg = dy.atan2(dx).to_degrees();
    let step = ((deg - 90.0) / 22.5).round() as i32;
    step.rem_euclid(16) as usize * 2
}

/// Slot DCC de la direction d'un missile à `ndirs` directions (tables
/// Necrolis `dir64_to_dcc`, identiques aux entités).
pub fn missile_dir_slot(dx: f32, dy: f32, ndirs: usize) -> usize {
    dir64_to_dcc(dir64_from(dx, dy), ndirs)
}

/// Canvas d'une frame d'un DCC mono-couche : la box de direction ENTIÈRE
/// (toutes les frames alignées — zéro jitter entre frames d'un cycle), RGBA
/// display via `pal` (index 0 = transparent). Ordre : `dirs[direction]`.
pub fn frame_rgba(
    dcc: &Dcc,
    direction: usize,
    frame: usize,
    pal: &Palette,
) -> Option<(usize, usize, Vec<u8>)> {
    let dir = dcc.directions.get(direction)?;
    let frame = frame.min(dcc.frames_per_direction.saturating_sub(1));
    let rect = Rect {
        left: dir.box_left,
        top: dir.box_top,
        width: dir.box_width,
        height: dir.box_height,
    };
    if rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    let indices = composite_indices(std::slice::from_ref(&dir), direction, frame, rect);
    let rgba = indices_to_rgba(&indices, pal);
    Some((rect.width as usize, rect.height as usize, rgba))
}

/// Frame + box d'une direction (pour les anchors : bas-centre de box pour
/// les overlays, centre de box pour les missiles). `frames_union` de la
/// frame jouée — même géométrie que [`frame_rgba`], en entier + offsets.
pub fn frame_box(dcc: &Dcc, direction: usize) -> Option<(i32, i32, usize, usize)> {
    let dir = dcc.directions.get(direction)?;
    Some((dir.box_left, dir.box_top, dir.box_width.max(0) as usize, dir.box_height.max(0) as usize))
}

/// Blit ADDITIF (`Trans`=3) d'un canvas RGBA dans le framebuffer du jeu —
/// `dst += src × tint` (clamp 255), pixels `alpha=0` ignorés. L'ORDRE
/// d'appel porte le `PreDraw` de la table : `pre_draw=true` (auras) se blit
/// AVANT l'entité (sous), `false` (sanctuaires) APRÈS (au-dessus).
pub fn blit_additive(
    fb: &mut [u8],
    vw: usize,
    vh: usize,
    w: usize,
    h: usize,
    rgba: &[u8],
    cx: f32,
    cy: f32,
    anchor_feet: bool,
    tint: Option<[f32; 3]>,
) {
    let dx0 = (cx - w as f32 / 2.0).round() as i32;
    // feet : bas de l'image sur cy (entité au sol) ; centre : missile en vol.
    let dy0 = if anchor_feet { (cy - h as f32).round() } else { (cy - h as f32 / 2.0).round() } as i32;
    for y in 0..h {
        let fy = dy0 + y as i32;
        if fy < 0 || fy as usize >= vh {
            continue;
        }
        for x in 0..w {
            let fx = dx0 + x as i32;
            if fx < 0 || fx as usize >= vw {
                continue;
            }
            let si = (y * w + x) * 4;
            if rgba[si + 3] == 0 {
                continue;
            }
            let di = (fy as usize * vw + fx as usize) * 4;
            let [tr, tg, tb] = tint.unwrap_or([1.0, 1.0, 1.0]);
            for (c, t) in (0..3).zip([tr, tg, tb]) {
                let add = (rgba[si + c] as f32 * t).round() as u32;
                let v = (fb[di + c] as u32 + add).min(255) as u8;
                fb[di + c] = v;
            }
        }
    }
}

/// Palshift simple d'état : palette display multipliée par la teinte de la
/// table (`poison` → vert 128,255,128 ; `cold`/`freeze` → bleu
/// 150,215,255). L'entité composée avec CETTE palette porte la teinte
/// retail du colorshift (la table ne référence aucun DC pour ces états).
pub fn shift_palette(pal: &Palette, tint: [f32; 3]) -> Palette {
    let mut rgb = pal.rgb;
    for c in rgb.iter_mut() {
        for k in 0..3 {
            c[k] = ((c[k] as f32 * tint[k]).round() as u32).min(255) as u8;
        }
    }
    Palette { rgb }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Palette display acte 1 (PL2, RGB direct — cf. `sprite::sprite_palette`).
    fn pal() -> Palette {
        static RAW: &[u8] = include_bytes!("../../../../assets/map/pal_act1_pl2.dat");
        Palette::load(RAW, false)
    }

    // Sprites DCC v1.12 réels extraits (extract_overlays_15c.py) — d2data.mpq.
    macro_rules! dcc {
        ($dir:literal, $file:literal) => {
            include_bytes!(concat!("../../../../assets/", $dir, "/", $file, ".dcc"))
        };
    }

    fn overlay_dcc(file: &str) -> &'static [u8] {
        match file {
            "ShrineArmor" => dcc!("overlays", "ShrineArmor"),
            "ShrineCombat" => dcc!("overlays", "ShrineCombat"),
            "ShrineResistFire" => dcc!("overlays", "ShrineResistFire"),
            "ShrineResistCold" => dcc!("overlays", "ShrineResistCold"),
            "ShrineResistLightning" => dcc!("overlays", "ShrineResistLightning"),
            "ShrineResistPoison" => dcc!("overlays", "ShrineResistPoison"),
            "ShrineSkill" => dcc!("overlays", "ShrineSkill"),
            "ShrineManaRegen" => dcc!("overlays", "ShrineManaRegen"),
            "ShrineStamina" => dcc!("overlays", "ShrineStamina"),
            "ShrineExperience" => dcc!("overlays", "ShrineExperience"),
            "ShrineShimmer00" => dcc!("overlays", "ShrineShimmer00"),
            "ShrineShimmer01" => dcc!("overlays", "ShrineShimmer01"),
            "AuraResistFireFront" => dcc!("overlays", "AuraResistFireFront"),
            "AuraResistColdBack" => dcc!("overlays", "AuraResistColdBack"),
            "AuraResistLightningBack" => dcc!("overlays", "AuraResistLightningBack"),
            "AuraResistAllFront" => dcc!("overlays", "AuraResistAllFront"),
            "AuraResistAllBack" => dcc!("overlays", "AuraResistAllBack"),
            other => panic!("overlay DCC non embarqué : {other}"),
        }
    }

    fn missile_dcc(file: &str) -> &'static [u8] {
        match file {
            "Arrow" => dcc!("missiles", "Arrow"),
            "Javelin" => dcc!("missiles", "Javelin"),
            "FireArrow" => dcc!("missiles", "FireArrow"),
            "SafeArrow" => dcc!("missiles", "SafeArrow"),
            "IceArrow" => dcc!("missiles", "IceArrow"),
            "XBowBolt" => dcc!("missiles", "XBowBolt"),
            "MissileHandAxe" => dcc!("missiles", "MissileHandAxe"),
            "ChargedBolt" => dcc!("missiles", "ChargedBolt"),
            "Firebolt" => dcc!("missiles", "Firebolt"),
            "Icebolt" => dcc!("missiles", "Icebolt"),
            other => panic!("missile DCC non embarqué : {other}"),
        }
    }

    fn table() -> OverlayTable {
        OverlayTable::embedded().expect("tables embarquées v1.12")
    }

    /// Tous les DCC de la mission décodent (overlays + missiles).
    fn decode_ok(bytes: &'static [u8]) -> Dcc {
        Dcc::decode(bytes).expect("DCC v1.12 réel")
    }

    // ------------------------------------------------------------------
    // Golden tables
    // ------------------------------------------------------------------

    #[test]
    fn tables_v112_completes() {
        let t = table();
        let (states, overlays) = t.len();
        assert_eq!(overlays, 294, "overlay.txt = 294 records");
        assert_eq!(states, 185, "states.txt = 185 records (186 lignes - en-tête)");
    }

    /// États ACTIFS teinte-only : poison vert 104, froid/gel bleu 108,
    /// AUCUN overlay DC (le rendu = palshift simple de l'entité).
    #[test]
    fn etats_tint_poison_et_froid() {
        let t = table();
        for (state, cshift, rgb) in [
            (STATE_POISON, 104, [128, 255, 128]),
            (STATE_CHILL, 108, [150, 215, 255]),
            (STATE_FREEZE, 108, [150, 215, 255]),
        ] {
            let s = t.state(state).unwrap_or_else(|| panic!("état {state}"));
            assert!(s.overlays.is_empty(), "{state} : aucun DC en retail");
            assert_eq!(s.colorshift, cshift, "{state} colorshift");
            let tint = t.state_tint(state).expect("teinte");
            for k in 0..3 {
                assert!((tint[k] * 255.0).round() as u32 - rgb[k] <= 1, "{state} light rgb");
            }
        }
    }

    /// Golden PAR ÉTAT ACTIF : état → overlay(s) → fichier + rendu.
    /// Ordre = col `overlay1` de states.txt, valeurs de overlay.txt.
    #[test]
    fn golden_etat_overlay_dc_par_buff_shrine() {
        let t = table();
        // states.txt réel : overlay1 = DC du sanctuaire, overlay2 = le
        // scintillement (shimmer00 ou 01 selon l'état).
        let goldens: [(u8, &str, &str, &str); 10] = [
            (6, "shrine_armor", "ShrineArmor", "ShrineShimmer00"),
            (7, "shrine_combat", "ShrineCombat", "ShrineShimmer00"),
            (8, "shrine_resist_fire", "ShrineResistFire", "ShrineShimmer00"),
            (9, "shrine_resist_cold", "ShrineResistCold", "ShrineShimmer01"),
            (10, "shrine_resist_lightning", "ShrineResistLightning", "ShrineShimmer01"),
            (11, "shrine_resist_poison", "ShrineResistPoison", "ShrineShimmer01"),
            (12, "shrine_skill", "ShrineSkill", "ShrineShimmer00"),
            (13, "shrine_mana_regen", "ShrineManaRegen", "ShrineShimmer01"),
            (14, "shrine_stamina", "ShrineStamina", "ShrineShimmer01"),
            (15, "shrine_experience", "ShrineExperience", "ShrineShimmer00"),
        ];
        for (code, state, file, shimmer) in goldens {
            let st = state_for_shrine(code).unwrap_or_else(|| panic!("code shrine {code}"));
            assert_eq!(st, state, "code {code} → ligne states.txt");
            let ovs = t.state_overlays(state);
            assert_eq!(ovs.len(), 2, "{state} : overlay1 + shimmer");
            assert_eq!(ovs[1].file, shimmer, "{state} shimmer");
            let o = ovs[0];
            assert_eq!(o.file, file, "{state} → fichier");
            assert_eq!(o.frames, 15, "{state} Frames table");
            assert_eq!(o.trans, 3, "{state} Trans additif");
            assert_eq!(o.pre_draw, false, "{state} PreDraw=0 (au-dessus de l'entité)");
            assert_eq!(o.rgb, [1.0, 1.0, 1.0], "{state} teinte table blanche");
            // Le DCC réel décode, 1 direction, ≥ Frames de la table, frame
            // non vide (indices peints).
            let dcc = decode_ok(overlay_dcc(file));
            assert_eq!(dcc.number_of_directions, 1, "{file} mono-direction");
            assert!(dcc.frames_per_direction >= o.frames as usize, "{file} frames");
            let (w, h, rgba) =
                frame_rgba(&dcc, 0, 0, &pal()).unwrap_or_else(|| panic!("{file} frame 0"));
            assert!(w > 0 && h > 0, "{file} box");
            assert!(rgba.chunks_exact(4).any(|p| p[3] != 0), "{file} frame peinte");
        }
    }

    /// Auras (états à venir) : overlays sous l'entité (PreDraw=1), 20 frames,
    /// teinte table (feu orangé…), DCC décodé.
    #[test]
    fn golden_auras_resist() {
        let t = table();
        // PreDraw réel de overlay.txt : resistfire (fichier Front !) = 1,
        // cold/light (Back) = 1, resistall front = 0 / back = 1.
        let goldens: [(&str, &str, [u8; 3], bool); 5] = [
            ("resistfire", "AuraResistFireFront", [255, 193, 103], true),
            ("resistcold", "AuraResistColdBack", [210, 210, 255], true),
            ("resistlight", "AuraResistLightningBack", [255, 255, 200], true),
            ("resistall", "AuraResistAllFront", [211, 148, 255], false),
            ("resistall", "AuraResistAllBack", [211, 148, 255], true),
        ];
        for (state, file, rgb, pre) in goldens {
            let ovs = t.state_overlays(state);
            assert!(ovs.iter().any(|o| o.file == file), "{state} porte {file}");
            let o = ovs.iter().find(|o| o.file == file).unwrap();
            assert_eq!(o.frames, 20, "{file} Frames");
            assert_eq!(o.pre_draw, pre, "{file} PreDraw table");
            assert_eq!(o.trans, 3, "{file} additif");
            for k in 0..3 {
                assert_eq!((o.rgb[k] * 255.0).round() as u32, rgb[k] as u32, "{file} rgb");
            }
            let dcc = decode_ok(overlay_dcc(file));
            assert_eq!(dcc.frames_per_direction, 20, "{file} frames DCC");
        }
    }

    /// Le scintillement du sanctuaire (objet) décode aussi (15 frames table).
    #[test]
    fn shimmer_sanctuaire_decode() {
        let t = table();
        let o = t.overlay("shrine_shimmer00").expect("shrine_shimmer00");
        assert_eq!(o.file, "ShrineShimmer00");
        let dcc = decode_ok(overlay_dcc("ShrineShimmer00"));
        assert!(dcc.frames_per_direction >= o.frames as usize);
    }

    // ------------------------------------------------------------------
    // Golden missiles
    // ------------------------------------------------------------------

    /// Les 10 missiles de la table d2missiles décodent NON VIDES, avec les
    /// dirs/frames DCC réels. Tirés aujourd'hui : `arrow` (repli mêlée/
    /// invocation — 6 classes + NE Raise Skeleton) et `firebolt` (SOR Fire
    /// Bolt, SEUL StartSkill à missile de patch_charstats.txt) ; les 8
    /// autres = missiles de skills du barre (Magic/Charged Bolt, Fire/Ice
    /// Arrow, Ice Bolt… ∩ table d2missiles).
    #[test]
    fn golden_missiles_decodes_non_vide() {
        // dirs/frames = DCC v1.12 réels (dccdump) — traits 32 dirs, bolts 16.
        let goldens: [(&str, &str, usize, usize); 10] = [
            ("arrow", "Arrow", 32, 1),
            ("javelin", "Javelin", 32, 1),
            ("firearrow", "FireArrow", 32, 8),
            ("magicarrow", "SafeArrow", 32, 8),
            ("icearrow", "IceArrow", 32, 8),
            ("bolt", "XBowBolt", 32, 1),
            ("throwaxe", "MissileHandAxe", 32, 8),
            ("chargedbolt", "ChargedBolt", 16, 10),
            ("firebolt", "Firebolt", 16, 5),
            ("icebolt", "Icebolt", 16, 6),
        ];
        for (_id, file, dirs, frames) in goldens {
            let dcc = decode_ok(missile_dcc(file));
            assert_eq!(dcc.number_of_directions, dirs, "{file} dirs DCC");
            assert_eq!(dcc.frames_per_direction, frames, "{file} frames DCC");
            // Toutes les directions de vol décodent non vides (sonde 0, S).
            for d in [0usize, 8, 16, 24] {
                let (w, h, rgba) = frame_rgba(&dcc, d % dirs, 0, &pal())
                    .unwrap_or_else(|| panic!("{file} dir {d}"));
                assert!(w > 0 && h > 0);
                assert!(rgba.chunks_exact(4).any(|p| p[3] != 0), "{file} dir {d} vide");
            }
        }
    }

    /// Direction missile : convention MONDE (facing_from) respectée aux 8
    /// points cardinaux, slot DCC par tables Necrolis (sud = slot 4 en 16/32).
    #[test]
    fn dir64_matches_world_facing() {
        // d2rules::world::facing_from : oct = round((deg-90)/45) (sud=0, ouest=2…)
        let compass: [(f32, f32, usize); 8] = [
            (0.0, 1.0, 0),   // sud
            (-1.0, 1.0, 1),  // sud-ouest
            (-1.0, 0.0, 2),  // ouest
            (-1.0, -1.0, 3), // nord-ouest
            (0.0, -1.0, 4),  // nord
            (1.0, -1.0, 5),  // nord-est
            (1.0, 0.0, 6),   // est
            (1.0, 1.0, 7),   // sud-est
        ];
        for (dx, dy, facing) in compass {
            // dir64_from doit coïncider avec facing_from × 4 (16 pas).
            assert_eq!(dir64_from(dx, dy), facing * 4, "cardinal ({dx},{dy})");
            assert_eq!(dir64_from(5.0 * dx, 5.0 * dy), facing * 4, "dir normalisée");
        }
        // Slots Necrolis : sud = 4 (16 dirs) — idem entités (dcc_render).
        assert_eq!(missile_dir_slot(0.0, 1.0, 16), 4);
        assert_eq!(missile_dir_slot(0.0, 1.0, 32), 4);
        // Firebolt (16 dirs) : vol vers l'est → dir64 24 → slot Necrolis 1
        // (DIR16[24]) ; en 32 dirs la table dir_lookup donne AUSSI 1 pour
        // l'est (vérifié contre dir_lookup.rs — ground truth).
        assert_eq!(missile_dir_slot(1.0, 0.0, 16), 1);
        assert_eq!(missile_dir_slot(1.0, 0.0, 32), 1);
    }

    /// Animation : boucles overlay (AnimRate fps) et missile (25 fps).
    #[test]
    fn frames_boucles() {
        // 8 fps shrine : frame 8 à 1000 ms, wrap 15 frames à 2000 ms.
        assert_eq!(frame_at(1000, 8, 15), 8);
        assert_eq!(frame_at(2000, 8, 15), 1, "cycle 15 f à 8 fps = 1875 ms");
        assert_eq!(frame_at(1875, 8, 15), 0);
        assert_eq!(frame_at(125, 8, 15), 1);
        // Missile 25 fps : firebolt 5 frames → cycle 200 ms.
        assert_eq!(missile_frame_at(0, 5), 0);
        assert_eq!(missile_frame_at(40, 5), 1);
        assert_eq!(missile_frame_at(200, 5), 0);
        assert_eq!(missile_frame_at(10_000, 1), 0, "flèche 1 frame reste 0");
    }

    /// Blit additif : somme clampée, transparence intouchée, teinte.
    #[test]
    fn blit_additif_somme_et_clamp() {
        let (vw, vh) = (8usize, 8usize);
        let mut fb = vec![0u8; vw * vh * 4];
        for px in fb.chunks_exact_mut(4) {
            px.copy_from_slice(&[10, 20, 30, 255]);
        }
        // Sprite 2x1 opaque (200, 100, 50) ancré pieds au centre (4, 4).
        let rgba = vec![200, 100, 50, 255, 0, 0, 0, 0];
        blit_additive(&mut fb, vw, vh, 2, 1, &rgba, 4.0, 4.0, true, None);
        // Pixel gauche = 10+200, 20+100, 30+50 clampés ; droit intact.
        let i = (3 * vw + 3) * 4;
        assert_eq!(&fb[i..i + 3], &[210, 120, 80]);
        assert_eq!(&fb[i + 4..i + 7], &[10, 20, 30]);
        // Clamp à 255 + teinte feu (255,193,103)/255.
        let mut fb2 = vec![255u8; 4 * 4 * 4];
        blit_additive(&mut fb2, 4, 4, 1, 1, &[10, 10, 10, 255], 2.0, 2.0, false, Some([1.0, 0.5, 0.0]));
        assert_eq!(&fb2[0..3], &[255, 255, 255], "clamp");
    }

    /// Palshift simple : chaque canal est multiplié par la teinte de la
    /// table (poison 128,255,128 → gris 128 = 64,128,64 vert ; froid
    /// 150,215,255 → bleu), teinte blanche = identité.
    #[test]
    fn palshift_poison_vert_froid_bleu() {
        let t = table();
        // Gris synthétique : la teinte colorshift apparaît exactement.
        let mut gray = Palette { rgb: [[40, 40, 40]; 256] };
        gray.rgb[200] = [128, 128, 128];
        let poison = shift_palette(&gray, t.state_tint(STATE_POISON).unwrap());
        let [pr, pg, pb] = poison.rgb[200];
        assert_eq!((pr, pg, pb), (64, 128, 64), "gris × poison 128,255,128");
        let cold = shift_palette(&gray, t.state_tint(STATE_CHILL).unwrap());
        let [cr, cg, cb] = cold.rgb[200];
        assert_eq!((cr, cg, cb), (75, 108, 128), "gris × froid 150,215,255");
        // Palette réelle : chaque entrée = round(canal × teinte) (le miroir
        // exact de la spec, re-dérivé indépendamment dans le test).
        let p = pal();
        for state in [STATE_POISON, STATE_CHILL] {
            let tint = t.state_tint(state).unwrap();
            let shifted = shift_palette(&p, tint);
            for (src, out) in p.rgb.iter().zip(shifted.rgb.iter()) {
                for k in 0..3 {
                    let want = ((src[k] as f32 * tint[k]).round() as u32).min(255) as u8;
                    assert_eq!(out[k], want, "{state} canal {k}");
                }
            }
        }
        // Teinte blanche = identité.
        let id = shift_palette(&p, [1.0, 1.0, 1.0]);
        assert_eq!(id, p);
    }
}
