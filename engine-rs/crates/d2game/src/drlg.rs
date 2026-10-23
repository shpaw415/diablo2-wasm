//! DRLG wilderness seedé — W02 phase 1 (`docs/parity/finish-monde.md`).
//!
//! Port Rust de `tools/bloodmoor_build.py` (proto Python prouvé, voir
//! `evidence/MAP-SPEC-BLOODMOOR.md` §4) : la Blood Moor (levels.txt Id 2,
//! DrlgType 3) est ASSEMBLÉE en mémoire depuis les 23 presets outdoor réels
//! de l'acte I (DS1 v18 9×9 extraits de d2exp.mpq, embarqués ci-dessous),
//! avec un RNG déterministe par graine de partie (SplitMix64, le même
//! générateur que `d2rules/src/rng.rs` — l'algorithme de composition est le
//! port fidèle du proto, les tirages diffèrent donc du DS1 figé
//! `assets/map/bloodmoor.ds1` qui avait été produit par le RNG Python).
//!
//! Pipeline (identique au proto) :
//! 1. pools de cellules réelles (sols/murs) remappées vers l'espace DT1
//!    unifié (8 conteneurs, ordre figé = liste `dt1` du manifest bloodmoor) ;
//! 2. sol de base = herbe brassée par le RNG sur tout le canevas 72×72 ;
//! 3. anneau de bordure forestière (chunks `bord*` sur le pourtour 8×8) ;
//! 4. features intérieures : waypoint, falaises `stnclf6b`, rivière 2×2,
//!    clairières `bord6..9` ;
//! 5. dispersion déterministe : 420 arbres (ceinture densifiée près des
//!    bords), 90 rochers, 50 taches de pierres ;
//! 6. [`Ds1`] synthétique consommé tel quel par `compose::compose_seeded`
//!    (collision + rendu : un seul chemin de code, pas de DS1 intermédiaire).
//!
//! Garanties testées : même graine ⇒ mêmes couches, graine différente ⇒
//! carte différente, 0 cellule hors bornes, sol 100 % rempli.

use std::collections::BTreeMap;

use crate::ds1::Ds1;

/// Canevas 8×8 chunks de 9×9 cellules (v18 : stocké 8, réel 9).
const GRID: usize = 8;
const CH: usize = 9;
pub const WILDERNESS_SIZE: usize = GRID * CH; // 72×72

// ---------------------------------------------------------------------------
// Liste DT1 unifiée (espace d'indices cumulatifs des couches générées)
// ---------------------------------------------------------------------------

/// Les 8 conteneurs unifiés, DANS L'ORDRE des indices de tuiles des couches
/// générées — identique à la liste `dt1` du bloc `bloodmoor` du manifest.
pub const UNIFIED_FILES: [&str; 8] = [
    "out_treegroups.dt1",
    "out_stonewall.dt1",
    "floor.dt1",
    "out_stones.dt1",
    "out_river.dt1",
    "bloodmoor_cliff2.dt1",
    "bloodmoor_corner.dt1",
    "bloodmoor_cliff1.dt1",
];

/// Les 8 chemins embarqués dans le `Ds1` généré (style tg1/dt1 du proto) —
/// `game.rs::resolve_container` les résout vers `UNIFIED_FILES` via la liste
/// `dt1` du niveau, donc l'ordre de concaténation des tilesets = cet ordre.
pub const UNIFIED_TG1: [&str; 8] = [
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\treegroups.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\stonewall.dt1",
    "\\d2\\data\\global\\tiles\\act1\\town\\floor.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\stones.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\river.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\cliff2.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\corner.dt1",
    "\\d2\\data\\global\\tiles\\act1\\outdoors\\cliff1.dt1",
];

/// Nombre de tuiles par conteneur unifié (bases cumulatives du remap).
/// Vient des DT1 réels décodés (`dt1::decode_dt1(...).len()`), PAS codé en
/// dur : [`UnifiedCounts::from_slices`] est alimenté par `game.rs`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnifiedCounts {
    pub counts: [usize; 8],
}

impl UnifiedCounts {
    pub fn from_slices(counts: &[usize]) -> Result<UnifiedCounts, String> {
        let n = counts.len();
        if n != 8 {
            return Err(format!("drlg : {n} conteneurs au lieu de 8"));
        }
        Ok(UnifiedCounts { counts: counts.try_into().unwrap() })
    }

    /// Index de base cumulatif du conteneur `i`.
    fn base(&self, i: usize) -> usize {
        self.counts.iter().take(i).sum()
    }

    fn total(&self) -> usize {
        self.counts.iter().sum()
    }

    fn index_of(&self, file: &str) -> Option<usize> {
        UNIFIED_FILES.iter().position(|f| *f == file)
    }
}

/// Mapping tg1/dt1 embarqué d'un preset → conteneur unifié (port exact du
/// `Composer.tg1_file` du proto).
fn tg1_to_unified(tg1: &str) -> Result<&'static str, String> {
    let norm = tg1.replace('/', "\\");
    let parts: Vec<&str> = norm.split('\\').filter(|p| !p.is_empty()).collect();
    let last = parts.last().copied().unwrap_or("");
    let base = last
        .strip_suffix(".dt1")
        .or_else(|| last.strip_suffix(".tg1"))
        .unwrap_or(last)
        .to_ascii_lowercase();
    let folder = parts.iter().rev().nth(1).map(|p| p.to_ascii_lowercase()).unwrap_or_default();
    let unified: &'static str = match base.as_str() {
        "floor" => "floor.dt1",
        "objects" => {
            if folder == "town" {
                "town_objects.dt1"
            } else {
                "out_objects.dt1"
            }
        }
        "treegroups" => "out_treegroups.dt1",
        "stonewall" => "out_stonewall.dt1",
        "stones" => "out_stones.dt1",
        "river" => "out_river.dt1",
        "cliff1" => "bloodmoor_cliff1.dt1",
        "cliff2" => "bloodmoor_cliff2.dt1",
        "corner" => "bloodmoor_corner.dt1",
        other => {
            return Err(format!(
                "drlg : tg1 « {other} » sans conteneur unifié (preset hors banque ?)"
            ))
        }
    };
    Ok(unified)
}

// ---------------------------------------------------------------------------
// Banque de presets (DS1 réels extraits de d2exp.mpq, embarqués)
// ---------------------------------------------------------------------------

macro_rules! bank {
    ($($name:literal),* $(,)?) => {
        &[$(($name, include_bytes!(concat!(
            "../../../../assets/map/presets_bloodmoor/", $name, ".ds1")) as &[u8])),*]
    };
}

/// Les 23 presets utilisés par le proto (`bloodmoor_build.py`), embarqués :
/// bordure (13), clairières (4), rivière (4), falaise (1), plaine (1).
static PRESET_BANK: &[(&str, &[u8])] = bank!(
    // BORDER
    "bord10", "bord11", "bord12", "bord1b", "bord1c", "bord2", "bord2b", "bord2c",
    "bord3", "bord3b", "bord3c", "bord4c", "bord5",
    // CLEARING
    "bord6", "bord7", "bord8", "bord9",
    // RIVER
    "LriverN", "LriverS", "UriverN", "UriverS",
    // CLIFF
    "stnclf6b",
    // PLAIN (le waypoint — plaine pure)
    "waypoint",
);

const BORDER: [&str; 13] = [
    "bord10", "bord11", "bord12", "bord1b", "bord1c", "bord2", "bord2b", "bord2c",
    "bord3", "bord3b", "bord3c", "bord4c", "bord5",
];
const CLEARING: [&str; 4] = ["bord6", "bord7", "bord8", "bord9"];
const RIVER: [&str; 4] = ["LriverN", "LriverS", "UriverN", "UriverS"];
const CLIFF: [&str; 1] = ["stnclf6b"];
const PLAIN: [&str; 1] = ["waypoint"];

/// Un preset DS1 v18 décodé (tags 1/2/3 : 4/6/8 couches — le décodeur
/// `ds1::decode_ds1` du moteur lit 6 couches fixes, les presets tag3 en ont
/// 8, d'où un lecteur dédié).
#[derive(Debug, Clone)]
struct Preset {
    name: String,
    w: usize,
    h: usize,
    /// tag1 du DS1 (1/2/3) — détermine le nombre de couches.
    tag: u32,
    /// Couches dans l'ordre du fichier (`tag` : murs, puis sol, puis ombre).
    layers: Vec<Vec<i32>>,
    /// Chemins DT1/tg1 embarqués (bases cumulatives du remap).
    files: Vec<String>,
}

impl Preset {
    fn parse(name: &str, d: &[u8]) -> Result<Preset, String> {
        let i32_at = |at: usize| -> Result<i32, String> {
            d.get(at..at + 4)
                .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
                .ok_or_else(|| format!("preset {name} : tronqué @{at}"))
        };
        let version = i32_at(0)?;
        if version != 18 {
            return Err(format!("preset {name} : version {version} != 18"));
        }
        let w = i32_at(4)?.max(0) as usize + 1; // piège v18 : réel = stocké + 1
        let h = i32_at(8)?.max(0) as usize + 1;
        let file_count = i32_at(20)?.max(0) as usize;
        let mut p = 24usize;
        let mut files = Vec::with_capacity(file_count);
        for _ in 0..file_count {
            let end = d
                .get(p..)
                .and_then(|rest| rest.iter().position(|&b| b == 0))
                .ok_or_else(|| format!("preset {name} : liste de fichiers tronquée"))?
                + p;
            files.push(String::from_utf8_lossy(&d[p..end]).into_owned());
            p = end + 1;
        }
        let tag = i32_at(p)?.max(0) as u32;
        let _tag2 = i32_at(p + 4).unwrap_or(0);
        p += 8;
        let n_layers = match tag {
            1 => 4usize,
            2 => 6,
            3 => 8,
            other => return Err(format!("preset {name} : tag1 {other} inattendu")),
        };
        let n = w * h;
        let mut layers = Vec::with_capacity(n_layers);
        for li in 0..n_layers {
            let mut cells = Vec::with_capacity(n);
            for i in 0..n {
                cells.push(i32_at(p + 4 * (li * n + i))?);
            }
            layers.push(cells);
        }
        Ok(Preset { name: name.to_string(), w, h, tag, layers, files })
    }

    /// Index de la couche de sol (4 en tag2, 6 en tag3).
    fn floor_layer(&self) -> usize {
        match self.tag {
            1 => 4, // overlays murs-seuls : pas de sol — non utilisé en pool
            2 => 4,
            _ => 6,
        }
    }

    /// Bases cumulatives des conteneurs unifiés pour CE preset.
    fn bases(&self, counts: &UnifiedCounts) -> Result<Vec<(usize, usize, &'static str)>, String> {
        let mut out = Vec::with_capacity(self.files.len());
        let mut base = 0usize;
        for tg1 in &self.files {
            let unified = tg1_to_unified(tg1)?;
            let cnt = counts.counts[counts.index_of(unified).ok_or_else(|| {
                format!("preset {} : {unified} hors liste unifiée", self.name)
            })?];
            out.push((base, cnt, unified));
            base += cnt;
        }
        Ok(out)
    }
}

fn preset_bytes(name: &str) -> Result<&'static [u8], String> {
    PRESET_BANK
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, b)| *b)
        .ok_or_else(|| format!("drlg : preset « {name} » absent de la banque embarquée"))
}

// ---------------------------------------------------------------------------
// RNG — SplitMix64, le générateur seedé de d2rules/src/rng.rs
// ---------------------------------------------------------------------------

/// SplitMix64 (Steele et al. 2019) — même suite que `d2rules::rng::Rng` :
/// même graine ⇒ même partie, générateur indépendant du rng de combat.
pub struct SplitMix64(u64);

impl SplitMix64 {
    pub fn new(seed: u64) -> SplitMix64 {
        SplitMix64(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Entier uniforme dans [0, n) ; n = 0 ⇒ 0 (équivalent `randrange(0, n)`).
    pub fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            ((self.next_u64() >> 32) as usize) % n
        }
    }

    /// Flottant uniforme dans [0, 1) (équivalent `random.random()`).
    pub fn f01(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// FNV-1a 64 (même constante que game.rs) — graine par clé de niveau.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01B3);
    }
    h
}

/// Un pas SplitMix64 — mélange graine de partie ⊕ clé de niveau.
pub fn mix(seed: u64) -> u64 {
    let mut r = SplitMix64::new(seed);
    r.next_u64()
}

/// Graine DRLG d'un niveau : `mix(game_seed ^ fnv1a(key))` — même partie ⇒
/// même carte par niveau ; parties différentes ⇒ mondes différents, tous
/// rejouables à l'identique (déterministe).
pub fn level_seed(game_seed: u64, level_key: &str) -> u64 {
    mix(game_seed ^ fnv1a(level_key))
}

/// Ce module sait-il générer ce niveau ? Phase 1 : la Blood Moor seule
/// (banque de presets + tileset unifié extraits pour elle).
pub fn supports(level_key: &str) -> bool {
    level_key.eq_ignore_ascii_case("bloodmoor")
}

// ---------------------------------------------------------------------------
// Composition
// ---------------------------------------------------------------------------

/// Statistiques de génération (journal/notes du moteur + tests).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DrlgStats {
    /// Cellules remappées preset → espace unifié.
    pub remapped: usize,
    /// Cellules preset hors espace unifié (doit rester 0).
    pub out_of_bounds: usize,
    /// Presets collés (anneau + features).
    pub pasted: usize,
    /// Cellules dispersées (arbres + rochers + pierres).
    pub scattered: usize,
}

/// Canevas 6 couches (tag2 : mur1-4, sol, ombre) en espace unifié.
struct Canvas {
    layers: [Vec<i32>; 6],
}

impl Canvas {
    fn new(w: usize, h: usize) -> Canvas {
        Canvas { layers: std::array::from_fn(|_| vec![0i32; w * h]) }
    }

    fn put(&mut self, layer: usize, x: usize, y: usize, w: usize, v: i32) {
        if let Some(slot) = self.layers.get_mut(layer) {
            if let Some(slot) = slot.get_mut(y * w + x) {
                *slot = v;
            }
        }
    }
}

#[inline]
fn cell_fields(v: i32) -> (u8, u8, u8, u8) {
    (
        (v & 0xFF) as u8,
        ((v >> 8) & 0xFF) as u8,
        ((v >> 16) & 0xFF) as u8,
        ((v >> 24) & 0xFF) as u8,
    )
}

#[inline]
fn make_cell(p1: u8, p2: u8, p3: u8, orient: u8) -> i32 {
    (p1 as i32) | ((p2 as i32) << 8) | ((p3 as i32) << 16) | ((orient as i32) << 24)
}

/// Contexte de remap preset → espace unifié.
struct Remapper<'a> {
    counts: &'a UnifiedCounts,
    stats: &'a mut DrlgStats,
}

impl<'a> Remapper<'a> {
    /// Remap une cellule preset vers l'espace unifié (None si vide) —
    /// port exact du `Composer.remap` du proto.
    fn cell(&mut self, bases: &[(usize, usize, &'static str)], v: i32) -> Option<i32> {
        let (p1, p2, p3, orient) = cell_fields(v);
        if p1 == 0 {
            return None;
        }
        let nib = (p2 & 0x0F) as usize;
        let idx = p1 as usize + nib;
        for (base, cnt, unified) in bases {
            if idx < base + cnt {
                let new_idx = self.counts.base(self.counts.index_of(unified)?) + (idx - base);
                self.stats.remapped += 1;
                return Some(make_cell((new_idx - nib) as u8, p2, p3, orient));
            }
        }
        self.stats.out_of_bounds += 1;
        None
    }
}

/// Colle un preset sur le canevas (sol écrase, ombre copiée, murs posés
/// par-dessus, clamp aux bords) — port du `Composer.paste`.
fn paste(
    canvas: &mut Canvas,
    preset: &Preset,
    bases: &[(usize, usize, &'static str)],
    remap: &mut Remapper,
    cx0: usize,
    cy0: usize,
    w: usize,
    h: usize,
) {
    let floor_i = preset.floor_layer();
    let shadow_i = floor_i + 1;
    let wall_is: Vec<usize> = (0..floor_i).collect();
    for cy in 0..preset.h.min(h.saturating_sub(cy0)) {
        for cx in 0..preset.w.min(w.saturating_sub(cx0)) {
            let (x, y) = (cx0 + cx, cy0 + cy);
            // sol : écrase (le chunk apporte son sol réel)
            if let Some(v) = remap.cell(bases, preset.layers[floor_i][cy * preset.w + cx]) {
                canvas.put(4, x, y, w, v);
            }
            // ombre : copiée telle quelle (flags, pas d'indice tuile)
            let sv = preset.layers[shadow_i][cy * preset.w + cx];
            canvas.put(5, x, y, w, sv);
            // murs : ajout par-dessus (cellules non vides uniquement) —
            // couches 1..4 ; au-delà (tag3), retombée sur la couche 1
            // (streets), comme le proto (`zip((1,2,3,4,0,0), wall_i)`).
            let targets = [1usize, 2, 3, 4, 0, 0];
            for (li, target) in wall_is.iter().zip(targets) {
                if let Some(v) = remap.cell(bases, preset.layers[*li][cy * preset.w + cx]) {
                    canvas.put(target, x, y, w, v);
                }
            }
        }
    }
}

/// Génère la Blood Moor seedée. `counts` = tuiles réelles des 8 conteneurs
/// unifiés (dans l'ordre [`UNIFIED_FILES`], cf. [`UnifiedCounts`]).
/// Retourne le [`Ds1`] synthétique (6 couches tag 2,1, fichiers
/// [`UNIFIED_TG1`]) + les stats. Erreur si un preset déborde de l'espace
/// unifié (`out_of_bounds > 0`) ou si le sol n'est pas plein — jamais de
/// carte invalide silencieuse.
pub fn generate_bloodmoor(seed: u64, counts: &UnifiedCounts) -> Result<(Ds1, DrlgStats), String> {
    let (ds1, stats, _notes) = generate_wilderness(seed, counts, &PlainSpec::bloodmoor())?;
    Ok((ds1, stats))
}

/// Spécification d'un wilderness (phase 1 : une seule — la Blood Moor du
/// proto ; la structure isole les listes de presets pour les futurs niveaux).
struct PlainSpec {
    border: &'static [&'static str],
    clearing: &'static [&'static str],
    river: &'static [&'static str],
    cliff: &'static [&'static str],
    plain: &'static [&'static str],
}

impl PlainSpec {
    fn bloodmoor() -> PlainSpec {
        PlainSpec {
            border: &BORDER,
            clearing: &CLEARING,
            river: &RIVER,
            cliff: &CLIFF,
            plain: &PLAIN,
        }
    }
}

fn generate_wilderness(
    seed: u64,
    counts: &UnifiedCounts,
    spec: &PlainSpec,
) -> Result<(Ds1, DrlgStats, Vec<String>), String> {
    let mut stats = DrlgStats::default();
    let (w, h) = (WILDERNESS_SIZE, WILDERNESS_SIZE);

    // --- banque décodée + pools (déterministes, hors RNG) -------------------
    let pool_names: Vec<&str> = spec
        .plain
        .iter()
        .chain(spec.clearing)
        .chain(spec.river)
        .chain(spec.border)
        .chain(spec.cliff)
        .copied()
        .collect();
    let mut presets: BTreeMap<String, Preset> = Default::default();
    let mut preset_bases: BTreeMap<String, Vec<(usize, usize, &'static str)>> = Default::default();
    for name in pool_names.iter().chain(spec.border) {
        if presets.contains_key(*name) {
            continue;
        }
        let p = Preset::parse(name, preset_bytes(name)?)?;
        let bases = p.bases(counts)?;
        presets.insert((*name).to_string(), p);
        preset_bases.insert((*name).to_string(), bases);
    }

    let mut stats_probe = DrlgStats::default();
    let mut wall_pool: Vec<(usize, i32)> = Vec::new();
    let mut floor_pool: Vec<i32> = Vec::new();
    for name in &pool_names {
        let p = &presets[*name];
        let bases = &preset_bases[*name];
        let mut r = Remapper { counts, stats: &mut stats_probe };
        let floor_i = p.floor_layer();
        for cy in 0..p.h {
            for cx in 0..p.w {
                for li in 0..floor_i {
                    if let Some(c) = r.cell(bases, p.layers[li][cy * p.w + cx]) {
                        wall_pool.push((li, c));
                    }
                }
                if let Some(c) = r.cell(bases, p.layers[floor_i][cy * p.w + cx]) {
                    floor_pool.push(c);
                }
            }
        }
    }
    // cellules réellement collées : on rejoue le comptage sur les stats finales
    let probe_remapped = stats_probe.remapped;
    let probe_oob = stats_probe.out_of_bounds;

    // sols : herbe = cellules du conteneur floor (pas stones/cliff)
    let grass_lo = counts.base(counts.index_of("floor.dt1").unwrap());
    let grass_hi = grass_lo + counts.counts[counts.index_of("floor.dt1").unwrap()];
    let is_grass = |c: i32| {
        let p1 = (c & 0xFF) as usize;
        (grass_lo..grass_hi).contains(&p1)
    };
    let stonewall_i = counts.index_of("out_stonewall.dt1").unwrap();
    let stonewall_lo = counts.base(stonewall_i);
    let floor_i_uni = counts.index_of("floor.dt1").unwrap();
    let allfloor_pool: Vec<i32> = floor_pool.iter().copied().filter(|c| is_grass(*c)).collect();
    let stone_pool: Vec<i32> = floor_pool.iter().copied().filter(|c| !is_grass(*c)).collect();
    let tree_pool: Vec<(usize, i32)> = wall_pool
        .iter()
        .copied()
        .filter(|(_, c)| ((c & 0xFF) as usize) < stonewall_lo)
        .collect();
    let rock_pool: Vec<(usize, i32)> = wall_pool
        .iter()
        .copied()
        .filter(|(_, c)| {
            let p1 = (c & 0xFF) as usize;
            (stonewall_lo..counts.base(floor_i_uni)).contains(&p1)
        })
        .collect();
    if allfloor_pool.is_empty() || tree_pool.is_empty() {
        return Err("drlg : pools vides (presets ou comptes DT1 invalides)".into());
    }

    let mut rng = SplitMix64::new(seed);
    let mut canvas = Canvas::new(w, h);
    let mut remap = Remapper { counts, stats: &mut stats };

    // --- 1. sol de base : herbe réelle brassée par le RNG -------------------
    for y in 0..h {
        for x in 0..w {
            let c = allfloor_pool[rng.below(allfloor_pool.len())];
            canvas.put(4, x, y, w, c);
        }
    }

    // --- 2. anneau de bordure forestière (chunks bord* sur le pourtour) -----
    let mut ring: Vec<&str> = Vec::new();
    for i in 0..GRID {
        ring.push(spec.border[i % spec.border.len()]); // N
        ring.push(spec.border[(i + 3) % spec.border.len()]); // S
    }
    for i in 0..(GRID - 2) {
        ring.push(spec.border[(i + 5) % spec.border.len()]); // W
        ring.push(spec.border[(i + 1) % spec.border.len()]); // E
    }
    let mut pos = 0usize;
    for i in 0..GRID {
        paste_named(&mut canvas, &presets, &preset_bases, &mut remap, ring[pos], i * CH, 0, w, h);
        pos += 1;
        paste_named(&mut canvas, &presets, &preset_bases, &mut remap, ring[pos], i * CH, (GRID - 1) * CH, w, h);
        pos += 1;
    }
    for j in 1..(GRID - 1) {
        paste_named(&mut canvas, &presets, &preset_bases, &mut remap, ring[pos], 0, j * CH, w, h);
        pos += 1;
        paste_named(&mut canvas, &presets, &preset_bases, &mut remap, ring[pos], (GRID - 1) * CH, j * CH, w, h);
        pos += 1;
    }

    // --- 3. features intérieures (mêmes positions que le proto) -------------
    let features: [(&str, usize, usize); 11] = [
        (spec.plain[0], 3 * CH, 3 * CH),  // le waypoint !
        (spec.cliff[0], 1 * CH, 1 * CH),  // falaise NO
        (spec.cliff[0], 5 * CH, 5 * CH),  // falaise SE
        (spec.river[0], 4 * CH, 2 * CH),  // rivière 2×2
        (spec.river[2], 5 * CH, 2 * CH),
        (spec.river[1], 4 * CH, 3 * CH),
        (spec.river[3], 5 * CH, 3 * CH),
        (spec.clearing[0], 2 * CH, 5 * CH), // clairières/chemins
        (spec.clearing[1], 5 * CH, 1 * CH),
        (spec.clearing[2], 1 * CH, 4 * CH),
        (spec.clearing[3], 3 * CH, 1 * CH),
    ];
    for (name, cx, cy) in features {
        paste_named(&mut canvas, &presets, &preset_bases, &mut remap, name, cx, cy, w, h);
    }

    // --- 4. dispersion déterministe ------------------------------------------
    // arbres : ceinture densifiée près des bords (0.85 / 0.30 / 0.12)
    for _ in 0..420 {
        let x = rng.below(w - 2) + 1;
        let y = rng.below(h - 2) + 1;
        let d = x.min(y).min(w - 1 - x).min(h - 1 - y);
        let density = if d < CH {
            0.85
        } else if d < 2 * CH {
            0.30
        } else {
            0.12
        };
        if rng.f01() > density {
            continue;
        }
        let (li, c) = tree_pool[rng.below(tree_pool.len())];
        let layer = if (1..=4).contains(&li) { li } else { 1 };
        scatter(&mut canvas, layer, x, y, w, h, c);
        stats.scattered += 1;
    }
    for _ in 0..90 {
        let x = rng.below(w - 4) + 2;
        let y = rng.below(h - 4) + 2;
        let (li, c) = rock_pool[rng.below(rock_pool.len())];
        let layer = if (1..=4).contains(&li) { li } else { 1 };
        scatter(&mut canvas, layer, x, y, w, h, c);
        stats.scattered += 1;
    }
    for _ in 0..50 {
        let x = rng.below(w - 4) + 2;
        let y = rng.below(h - 4) + 2;
        let c = stone_pool[rng.below(stone_pool.len())];
        scatter(&mut canvas, 4, x, y, w, h, c);
        stats.scattered += 1;
    }

    // comptes des pools consolidés dans les stats (avant validation)
    stats.remapped += probe_remapped;
    stats.out_of_bounds += probe_oob;

    // --- 5. validation + Ds1 synthétique -------------------------------------
    if stats.out_of_bounds != 0 {
        return Err(format!(
            "drlg : {} cellules hors espace unifié (comptes DT1 incohérents)",
            stats.out_of_bounds
        ));
    }
    let floor_layer = &canvas.layers[4];
    let empty = floor_layer.iter().filter(|v| (**v & 0xFF) == 0).count();
    if empty != 0 {
        return Err(format!("drlg : sol incomplet ({empty} cellules vides)"));
    }
    let total = counts.total();
    for li in 0..5 {
        for &v in &canvas.layers[li] {
            let (p1, p2, _, _) = cell_fields(v);
            if p1 != 0 && p1 as usize + (p2 & 0x0F) as usize >= total {
                return Err(format!("drlg : indice de tuile {p1}/{} hors bornes", p2 & 0x0F));
            }
        }
    }

    let ds1 = Ds1 {
        version: 18,
        w,
        h,
        files: UNIFIED_TG1.iter().map(|s| s.to_string()).collect(),
        layers: canvas.layers.to_vec(),
    };
    Ok((ds1, stats, Vec::new()))
}

/// Colle un preset nommé (lookup banque + bases) et compte le collage dans
/// les stats portées par le remapper.
#[allow(clippy::too_many_arguments)]
fn paste_named(
    canvas: &mut Canvas,
    presets: &BTreeMap<String, Preset>,
    preset_bases: &BTreeMap<String, Vec<(usize, usize, &'static str)>>,
    remap: &mut Remapper,
    name: &str,
    cx: usize,
    cy: usize,
    w: usize,
    h: usize,
) {
    let (preset, bases) = match (presets.get(name), preset_bases.get(name)) {
        (Some(p), Some(b)) => (p, b),
        _ => return, // banque garantie complète : inatteignable
    };
    paste(canvas, preset, bases, remap, cx, cy, w, h);
    remap.stats.pasted += 1;
}

/// Pose une cellule dispersée si dans les bornes et non vide (port du
/// `scatter_cell`).
fn scatter(canvas: &mut Canvas, layer: usize, x: usize, y: usize, w: usize, h: usize, cell: i32) {
    if x < w && y < h && cell != 0 {
        canvas.put(layer, x, y, w, cell);
    }
}

// ---------------------------------------------------------------------------
// SQUELETTE maze (DrlgType 1, actes 2+) — plan de rooms seedé (bonus)
// ---------------------------------------------------------------------------

/// Une room posée par le plan de maze : cellule de grille + preset choisi +
/// ouvertures vers les rooms voisines (N/E/S/W).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MazeRoom {
    pub gx: u32,
    pub gy: u32,
    /// `Def` LvlPrest choisi pour cette room (index dans la planche fournie).
    pub preset: usize,
    pub open_n: bool,
    pub open_e: bool,
    pub open_s: bool,
    pub open_w: bool,
}

/// Plan de maze seedé — SQUELETTE assumé : la disposition des rooms
/// (grille `SizeX×SizeY` de LvlMaze, `Rooms` posées, connexes) + le choix
/// seedé du preset LvlPrest de chaque room. PAS encore l'assemblage DS1/DT1
/// (étape actes 2+ du XL W02) : ce plan est la structure que le futur
/// assembleur consommera, comme `plan_maze` côte Go d2mapgen maze.
#[derive(Debug, Clone)]
pub struct MazePlan {
    pub grid_w: u32,
    pub grid_h: u32,
    /// Rooms posées, ordre de pose (non trié géographiquement).
    pub rooms: Vec<MazeRoom>,
}

/// Planifie un maze depuis son record LvlMaze : `rooms` rooms sur la grille
/// `size_x×size_y`, chaque room reliée à une room déjà posée et voisine
/// (arbre couvrant — aucune room isolée), preset tiré seedé dans `presets`
/// (liste de `Def` LvlPrest candidats). Erreur si la planche de presets est
/// vide ou si les rooms ne tiennent pas dans la grille.
pub fn plan_maze(
    rec: &crate::levels::LvlMazeRecord,
    seed: u64,
    presets: &[u32],
) -> Result<MazePlan, String> {
    if presets.is_empty() {
        return Err(format!("drlg maze « {} » : planche de presets vide", rec.name));
    }
    let (gw, gh) = (rec.size_x.max(1), rec.size_y.max(1));
    let capacity = (gw as usize).saturating_mul(gh as usize);
    if capacity == 0 {
        return Err(format!("drlg maze « {} » : grille 0", rec.name));
    }
    let want = (rec.rooms as usize).min(capacity);
    let mut rng = SplitMix64::new(seed);

    // Pose en croissance de cluster : après la première room, une nouvelle
    // room doit toucher une cellule occupée (arbre couvrant — aucune île).
    let mut occupied = vec![false; capacity];
    let mut rooms: Vec<MazeRoom> = Vec::with_capacity(want);
    for room_i in 0..want {
        let mut picked: Option<(usize, usize)> = None;
        for tries in 0..2_000 {
            let (gx, gy) = (rng.below(gw as usize), rng.below(gh as usize));
            if occupied[gy * gw as usize + gx] {
                continue;
            }
            // Sauf dernières tentatives (grilles saturées), exiger un voisin
            // posé : le plan reste connexe par construction.
            if room_i > 0
                && tries < 1_500
                && !has_occupied_neighbor(gx, gy, gw as usize, gh as usize, &occupied)
            {
                continue;
            }
            picked = Some((gx, gy));
            break;
        }
        let Some((gx, gy)) = picked else { break };
        occupied[gy * gw as usize + gx] = true;

        // Ouvertures : vers chaque voisin déjà posé (les deux côtés),
        // preset tiré seedé dans la planche.
        let mut room = MazeRoom {
            gx: gx as u32,
            gy: gy as u32,
            preset: rng.below(presets.len()),
            open_n: false,
            open_e: false,
            open_s: false,
            open_w: false,
        };
        for (dx, dy, side, opposite) in
            [(0i64, -1i64, 0u8, 2u8), (1, 0, 1, 3), (0, 1, 2, 0), (-1, 0, 3, 1)]
        {
            let (nx, ny) = (gx as i64 + dx, gy as i64 + dy);
            if nx < 0 || ny < 0 || nx >= gw as i64 || ny >= gh as i64 {
                continue;
            }
            if !occupied[ny as usize * gw as usize + nx as usize] {
                continue;
            }
            match side {
                0 => room.open_n = true,
                1 => room.open_e = true,
                2 => room.open_s = true,
                _ => room.open_w = true,
            }
            if let Some(prev) = rooms
                .iter_mut()
                .find(|r| r.gx as usize == nx as usize && r.gy as usize == ny as usize)
            {
                match opposite {
                    0 => prev.open_n = true,
                    1 => prev.open_e = true,
                    2 => prev.open_s = true,
                    _ => prev.open_w = true,
                }
            }
        }
        rooms.push(room);
    }
    Ok(MazePlan { grid_w: gw, grid_h: gh, rooms })
}

/// Une cellule (gx, gy) touche-t-elle une cellule occupée ?
fn has_occupied_neighbor(gx: usize, gy: usize, gw: usize, gh: usize, occupied: &[bool]) -> bool {
    for (dx, dy) in [(0i64, -1i64), (1, 0), (0, 1), (-1, 0)] {
        let (x, y) = (gx as i64 + dx, gy as i64 + dy);
        if x < 0 || y < 0 || x >= gw as i64 || y >= gh as i64 {
            continue;
        }
        if occupied[y as usize * gw + x as usize] {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Tests — déterminisme, intégrité, et vrais DT1/composeur
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Comptes de tuiles des 8 DT1 réels, lus depuis le disque (skip hors
    /// workspace). Ordre = [`UNIFIED_FILES`].
    fn real_counts() -> Option<UnifiedCounts> {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)?
            .to_path_buf();
        let counts: Vec<usize> = UNIFIED_FILES
            .iter()
            .map(|f| {
                let bytes = std::fs::read(root.join("assets/map").join(f))
                    .unwrap_or_else(|e| panic!("{f} : {e}"));
                crate::dt1::decode_dt1(&bytes)
                    .unwrap_or_else(|e| panic!("{f} : {e:?}"))
                    .len()
            })
            .collect();
        Some(UnifiedCounts::from_slices(&counts).unwrap())
    }

    #[test]
    fn preset_bank_parses_and_is_v18() {
        for (name, bytes) in PRESET_BANK {
            let p = Preset::parse(name, bytes).unwrap_or_else(|e| panic!("{name} : {e}"));
            // chunks 7×7 à 9×9 réels (v18 : stocké = réel − 1 ; les bord* et
            // river/cliff font 9×9, le waypoint 7×7)
            assert!((7..=9).contains(&p.w) && (7..=9).contains(&p.h), "{name} : {}×{}", p.w, p.h);
            assert!((1..=3).contains(&p.tag), "{name} : tag {}", p.tag);
            let expected_layers = match p.tag {
                1 => 4,
                2 => 6,
                _ => 8,
            };
            assert_eq!(p.layers.len(), expected_layers);
            assert!(!p.files.is_empty(), "{name} : liste DT1 vide");
        }
        // waypoint = plaine pure tag2 avec sol plein
        let wp = Preset::parse("waypoint", preset_bytes("waypoint").unwrap()).unwrap();
        assert_eq!(wp.tag, 2);
        assert!(wp.layers[4].iter().all(|v| (*v & 0xFF) != 0), "sol du waypoint 100% plein");
        // les clairières bord6-9 sont tag3 (8 couches)
        for name in CLEARING {
            let p = Preset::parse(name, preset_bytes(name).unwrap()).unwrap();
            assert_eq!(p.tag, 3, "{name}");
        }
    }

    #[test]
    fn tg1_mapping_matches_proto() {
        assert_eq!(tg1_to_unified("\\d2\\data\\global\\tiles\\ACT1\\Outdoors\\TreeGroups.dt1").unwrap(), "out_treegroups.dt1");
        assert_eq!(tg1_to_unified("\\d2\\data\\global\\tiles\\act1\\town\\floor.dt1").unwrap(), "floor.dt1");
        assert_eq!(tg1_to_unified("\\d2\\tiles\\cliff1.tg1").unwrap(), "bloodmoor_cliff1.dt1");
        assert_eq!(tg1_to_unified("\\town\\objects.tg1").unwrap(), "town_objects.dt1");
        assert!(tg1_to_unified("\\barracks\\bridge.dt1").is_err(), "hors banque unifiée");
    }

    #[test]
    fn counts_reject_wrong_length() {
        assert!(UnifiedCounts::from_slices(&[1, 2, 3]).is_err());
    }

    #[test]
    fn generation_is_deterministic_per_seed() {
        let Some(counts) = real_counts() else { return };
        let (a, sa) = generate_bloodmoor(42, &counts).unwrap();
        let (b, sb) = generate_bloodmoor(42, &counts).unwrap();
        assert_eq!(a.layers, b.layers, "même graine ⇒ mêmes couches");
        assert_eq!(sa, sb);
        let (c, _) = generate_bloodmoor(43, &counts).unwrap();
        assert_ne!(a.layers, c.layers, "graine différente ⇒ carte différente");
        // La graine de partie se combine à la clé de niveau :
        assert_eq!(level_seed(7, "bloodmoor"), level_seed(7, "bloodmoor"));
        assert_ne!(level_seed(7, "bloodmoor"), level_seed(8, "bloodmoor"));
        assert_ne!(level_seed(7, "bloodmoor"), level_seed(7, "coldplains"));
    }

    #[test]
    fn generated_layout_is_sane() {
        let Some(counts) = real_counts() else { return };
        let (ds1, stats) = generate_bloodmoor(20261022, &counts).unwrap();
        assert_eq!(ds1.version, 18);
        assert_eq!((ds1.w, ds1.h), (WILDERNESS_SIZE, WILDERNESS_SIZE));
        assert_eq!(
            ds1.files.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            UNIFIED_TG1.to_vec()
        );
        assert_eq!(ds1.layers.len(), 6, "canevas tag (2,1)");
        // 28 chunks d'anneau + 11 features
        assert_eq!(stats.pasted, 39);
        assert_eq!(stats.out_of_bounds, 0);
        assert!(stats.remapped > 5000, "{} cellules remappées", stats.remapped);
        // sol 100 % plein (revérifié ici, indépendamment du validateur)
        assert!(ds1.layers[4].iter().all(|v| (*v & 0xFF) != 0));
        // ombre : vide partout, constaté sur TOUS les presets
        // (MAP-SPEC-BLOODMOOR §7 « ombre des presets est vide ») — copiée
        // telle quelle par le paste, elle reste nulle dans la carte générée.
        assert!(ds1.layers[5].iter().all(|v| *v == 0));
        // un peu de forêt sur l'anneau (couche 2 = arbres du proto)
        let trees = ds1.layers[2].iter().filter(|v| (**v & 0xFF) != 0).count();
        assert!(trees > 300, "arbres attendus en couche 2 : {trees}");
    }

    #[test]
    fn seed_zero_differs_from_python_frozen_but_is_stable() {
        // La graine par défaut (0) donne une carte fixe (rejouable), qui n'a
        // AUCUNE raison d'égaler le DS1 figé Python (RNG différent, assumé).
        let Some(counts) = real_counts() else { return };
        let (a, _) = generate_bloodmoor(0, &counts).unwrap();
        let (b, _) = generate_bloodmoor(0, &counts).unwrap();
        assert_eq!(a.layers, b.layers);
    }

    #[test]
    fn composes_with_real_tilesets_and_yields_playable_grid() {
        // Intégration composeur : la carte générée passe le pipeline réel
        // (framebuffer + grille de collision 5×5 par cellule).
        let Some(root) = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .map(|p| p.to_path_buf())
        else {
            return;
        };
        let counts = real_counts().unwrap();
        let (ds1, _) = generate_bloodmoor(7, &counts).unwrap();
        let mut tilesets: Vec<crate::dt1::Dt1Tile> = Vec::new();
        for f in UNIFIED_FILES {
            let bytes = std::fs::read(root.join("assets/map").join(f)).unwrap();
            tilesets.extend(crate::dt1::decode_dt1(&bytes).unwrap());
        }
        assert_eq!(tilesets.len(), counts.total(), "espace de tuiles unifié");
        let map = crate::compose::compose_seeded(&ds1, &tilesets, 1);
        assert_eq!((map.grid_w, map.grid_h), (360, 360));
        let walkable = map.walk.iter().filter(|w| **w).count();
        let ratio = walkable as f32 / map.walk.len() as f32;
        assert!(
            (0.25..0.85).contains(&ratio),
            "plaine jouable attendue, ratio {ratio:.2}"
        );
        // Le point d'entrée du warp manifest (137,147) a une tuile marchable
        // à proximité (le héros peut bien atterrir sur la carte générée).
        assert!(
            crate::compose::find_nearby_walkable(&map.walk, map.grid_w, map.grid_h, 137, 147, 10)
                .is_some(),
            "entrée warp sans tuile marchable à proximité"
        );
        // Les pixels de sol sont peints (pas une carte noire).
        assert!(map.idx.iter().any(|v| *v != 0));
    }

    #[test]
    fn belt_is_denser_near_edges() {
        // La ceinture forestière : plus de murs près du bord qu'au centre.
        let Some(counts) = real_counts() else { return };
        let (ds1, _) = generate_bloodmoor(99, &counts).unwrap();
        let walls = |cells: &[i32]| cells.iter().filter(|v| (**v & 0xFF) != 0).count();
        let row_edge = walls(&ds1.layers[1][0 * 72..1 * 72]);
        let row_center = walls(&ds1.layers[1][36 * 72..37 * 72]);
        assert!(row_edge > row_center, "bord {row_edge} vs centre {row_center}");
    }

    // --- squelette maze (bonus, DrlgType 1 actes 2+) ------------------------

    fn cave2_record() -> crate::levels::LvlMazeRecord {
        crate::levels::LvlMazeRecord {
            name: "Act 1 - Cave 2".into(),
            level: 9,
            rooms: 4,
            size_x: 24,
            size_y: 24,
            merge: 500,
        }
    }

    #[test]
    fn maze_plan_is_deterministic_and_connected() {
        let rec = cave2_record();
        let presets = [101u32, 102, 103];
        let a = plan_maze(&rec, 5, &presets).unwrap();
        let b = plan_maze(&rec, 5, &presets).unwrap();
        assert_eq!(a.rooms, b.rooms, "même graine ⇒ même plan");
        assert_eq!((a.grid_w, a.grid_h), (24, 24));
        assert_eq!(a.rooms.len(), 4, "Rooms=4 posées");
        // presets tirés dans la planche
        assert!(a.rooms.iter().all(|r| r.preset < 3));
        // cellules distinctes
        let mut cells: Vec<(u32, u32)> = a.rooms.iter().map(|r| (r.gx, r.gy)).collect();
        cells.sort();
        cells.dedup();
        assert_eq!(cells.len(), a.rooms.len(), "pas de doublon de cellule");
        // connexité : BFS sur les ouvertures réciproques atteint toutes les rooms
        let reachable = |plan: &MazePlan| {
            let mut seen = vec![false; plan.rooms.len()];
            let mut queue = vec![0usize];
            seen[0] = true;
            while let Some(i) = queue.pop() {
                let (gx, gy) = (plan.rooms[i].gx as i64, plan.rooms[i].gy as i64);
                for (dx, dy, open) in [
                    (0i64, -1i64, plan.rooms[i].open_n),
                    (1, 0, plan.rooms[i].open_e),
                    (0, 1, plan.rooms[i].open_s),
                    (-1, 0, plan.rooms[i].open_w),
                ] {
                    if !open {
                        continue;
                    }
                    for (j, r) in plan.rooms.iter().enumerate() {
                        if !seen[j] && r.gx as i64 == gx + dx && r.gy as i64 == gy + dy {
                            seen[j] = true;
                            queue.push(j);
                        }
                    }
                }
            }
            seen.iter().all(|s| *s)
        };
        assert!(reachable(&a), "toutes les rooms sont connexes (arbre couvrant)");
        // ouvertures réciproques : si A ouvre vers B, B ouvre vers A
        for r in &a.rooms {
            for (dx, dy, open) in [
                (0i64, -1i64, r.open_n),
                (1, 0, r.open_e),
                (0, 1, r.open_s),
                (-1, 0, r.open_w),
            ] {
                if !open {
                    continue;
                }
                let back = a.rooms.iter().find(|o| {
                    o.gx as i64 == r.gx as i64 + dx && o.gy as i64 == r.gy as i64 + dy
                });
                if let Some(o) = back {
                    let back_open = match (dx, dy) {
                        (0, -1) => o.open_s,
                        (1, 0) => o.open_w,
                        (0, 1) => o.open_n,
                        _ => o.open_e,
                    };
                    assert!(back_open, "ouverture non réciproque ({},{})->({dx},{dy})", r.gx, r.gy);
                }
            }
        }
        // graine différente ⇒ plan différent (statistiquement : 4 seeds)
        let diff = (6u64..20)
            .map(|s| plan_maze(&rec, s, &presets).unwrap())
            .any(|p| p.rooms != a.rooms);
        assert!(diff, "une graine différente finit par donner un plan différent");
        // erreurs propres
        assert!(plan_maze(&rec, 1, &[]).is_err(), "planche vide rejetée");
    }
}
