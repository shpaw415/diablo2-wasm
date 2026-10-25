//! FIX-HEROS — preuve du rendu des jambes (couche LG) du héros.
//!
//! Cause racine (RAPPORT-FIX.md) : LG n'existe pas dans l'enum COF
//! (0=HD…14=S8, evidence/DCC-SPEC.md §7.3) — le compositeur n'empilait que
//! les couches déclarées par le COF, donc les DCC LG du manifest n'étaient
//! jamais dessinés (« allure buste », héros sans jambes en jeu).
//!
//! Ce test :
//! 1. rend une planche NU/WL/RN/A1 du barbare (4 facings) avec LG empilé
//!    en bas de pile → `docs/fix-heros-apres.png` (l'AVANT est archivé
//!    dans `docs/fix-heros-avant.png` — planche proof_sprite_plate du buste) ;
//! 2. PROUVE l'apport de LG : la composition avec LG est strictement plus
//!    haute que la même composition sans la couche LG, et le quart bas du
//!    sprite (jambes/pieds) contient des pixels peints.
//!
//! Auto-skip si les assets du workspace sont absents.

mod common;

use d2game::png_mini;
use d2dcc::{Cof, Dcc};
use d2game::sprite::SpriteSet;
use serde_json::Value;

/// Construit le set d'un mode héros depuis un bloc manifest {cof, layers}.
fn build_set(root: &str, blk: &Value, with_lg: bool) -> SpriteSet {
    let cof_path = blk["cof"].as_str().unwrap();
    let cof = Cof::parse(&std::fs::read(format!("{root}/{cof_path}")).unwrap()).unwrap();
    let mut set = SpriteSet::new(cof);
    for (tok, p) in blk["layers"].as_object().unwrap() {
        if !with_lg && tok == "LG" {
            continue; // variante « buste » : l'état AVANT du bug
        }
        let bytes = std::fs::read(format!("{root}/{}", p.as_str().unwrap())).unwrap();
        set.add_layer(tok, Dcc::decode(&bytes).unwrap());
    }
    // FIX-HEROS : convention fichier (comme game.rs build_sprite_set) —
    // les blocs manifest régénérés peuvent omettre LG ; le DCC
    // <code><mode>LG.dcc fait foi s'il existe (stem « BANUHTH » → BANULG).
    if with_lg && !set.has_layer("LG") {
        let cof_path = blk["cof"].as_str().unwrap();
        let base = cof_path.rsplit('/').next().unwrap_or(cof_path);
        let stem: Vec<char> = base.split('.').next().unwrap_or(base).chars().collect();
        if stem.len() >= 4 {
            let lg = format!(
                "{root}/assets/dcc/{}{}LG.dcc",
                &stem[..2].iter().collect::<String>(),
                &stem[2..4].iter().collect::<String>()
            );
            if let Ok(bytes) = std::fs::read(&lg) {
                set.add_layer("LG", Dcc::decode(&bytes).unwrap());
            }
        }
    }
    set
}

/// Pixels non transparents du quart bas du canvas (zone jambes/pieds).
fn painted_in_lower_quarter(sf: &d2game::sprite::SpriteFrame) -> usize {
    let y0 = sf.height * 3 / 4;
    let mut n = 0;
    for y in y0..sf.height {
        for x in 0..sf.width {
            let si = ((y * sf.width + x) * 4) as usize;
            if sf.rgba[si + 3] != 0 {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn hero_has_legs() {
    let Some(root) = common::ws_root() else { return };
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(format!("{root}/assets/manifest.json")).unwrap(),
    )
    .unwrap();
    let raw_pal =
        std::fs::read(format!("{root}/{}", manifest.pointer("/level/palette").unwrap().as_str().unwrap()))
            .unwrap();
    let pal = d2game::sprite::sprite_palette(&raw_pal);
    let ba = &manifest["heroes"]["BA"];

    // --- 2. preuve d'apport de LG (NU = l'état debout du jeu) -------------
    let full = build_set(&root, &ba["modes"]["NU"], true);
    let bust = build_set(&root, &ba["modes"]["NU"], false);
    let sf_full = full.compose(0, 0, &pal).expect("compose NU avec LG");
    let sf_bust = bust.compose(0, 0, &pal).expect("compose NU sans LG");
    println!(
        "NU dir0 f2 : avec LG = {}x{}, buste (AVANT) = {}x{}",
        sf_full.width, sf_full.height, sf_bust.width, sf_bust.height
    );
    assert!(
        sf_full.height > sf_bust.height,
        "LG doit allonger le sprite : {} vs {} (buste)",
        sf_full.height,
        sf_bust.height
    );
    let legs = painted_in_lower_quarter(&sf_full);
    assert!(legs > 200, "le quart bas (jambes) doit être peint, {legs} px seulement");
    // le buste seul n'a (presque) rien sous la ligne de torse
    let bust_lower = painted_in_lower_quarter(&sf_bust);
    assert!(
        legs > bust_lower * 2,
        "LG doit peindre massivement le bas : {legs} px (avec) vs {bust_lower} px (buste)"
    );

    // idem en course (RN — jambes extraites de balglitrnhth.dcc cette vague)
    let rn = build_set(&root, &ba["modes"]["RN"], true);
    let rn_bust = build_set(&root, &ba["modes"]["RN"], false);
    let sf_rn = rn.compose(0, 2, &pal).expect("compose RN avec LG");
    let sf_rn_bust = rn_bust.compose(0, 2, &pal).expect("compose RN sans LG");
    assert!(
        sf_rn.height > sf_rn_bust.height,
        "LG doit allonger RN aussi : {} vs {}",
        sf_rn.height,
        sf_rn_bust.height
    );

    // --- 1. planche APRÈS : NU/WL/RN/A1 × facings, frame 2 ----------------
    let modes =
        [("NU", "/modes/NU"), ("WL", "/modes/WL"), ("RN", "/modes/RN"), ("A1", "/modes/A1")];
    let facings = [0u8, 4, 8, 12];
    let (cw, ch) = (180usize, 200usize);
    let mut rgba = vec![0u8; cw * facings.len() * ch * modes.len() * 4];
    for (y, x) in (0..ch * modes.len()).flat_map(|y| (0..cw * facings.len()).map(move |x| (y, x))) {
        let o = (y * cw * facings.len() + x) * 4;
        let v = if (x / 16 + y / 16) % 2 == 0 { 70 } else { 55 };
        rgba[o] = v;
        rgba[o + 1] = v;
        rgba[o + 2] = v + 8;
        rgba[o + 3] = 255;
    }
    for (mi, (tag, ptr)) in modes.iter().enumerate() {
        let set = build_set(&root, ba.pointer(ptr).unwrap(), true);
        for (fi, facing) in facings.iter().enumerate() {
            let Some(sf) = set.compose(*facing, 2, &pal) else { continue };
            if fi == 0 {
                println!("{tag}: compose dir0 f2 = {}x{}", sf.width, sf.height);
            }
            let bx = fi * cw + (cw - sf.width as usize) / 2;
            let by = mi * ch + ch - sf.height as usize; // pieds au bas de cellule
            for y in 0..sf.height as usize {
                for x in 0..sf.width as usize {
                    let si = (y * sf.width as usize + x) * 4;
                    if sf.rgba[si + 3] == 0 {
                        continue;
                    }
                    let (dx, dy) = (bx + x, by + y);
                    if dx >= cw * facings.len() || dy >= ch * modes.len() {
                        continue;
                    }
                    let o = (dy * cw * facings.len() + dx) * 4;
                    rgba[o..o + 4].copy_from_slice(&sf.rgba[si..si + 4]);
                }
            }
        }
    }
    let out = format!("{}/docs/fix-heros-apres.png", root);
    png_mini::write_png_rgba(&out, (cw * facings.len()) as u32, (ch * modes.len()) as u32, &rgba)
        .unwrap();
    println!("wrote {out}");
}
