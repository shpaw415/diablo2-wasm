//! FIX-CAMPES — lecteur du schéma RÉEL des DS1 v1.12 (v18, section
//! `(numWalls, numFloors)`), tel que validé sur TownN1.ds1 (rapport
//! `docs/rapports-d2w-fix/RAPPORT-FIX-cartes.md`, preuves chiffrées
//! reproduites par `tests/town1_camp.rs`).
//!
//! Le décodeur historique ([`crate::ds1`]) lit 6 flux « à plat » avec la
//! sémantique du prototype (L0 rues, L1 arbres, L4 sol…) : sur la ville 1
//! réelle, le flux d'*orientation* était rendu comme des indices de tuiles
//! d'arbres — la fausse forêt généralisée. Le schéma réel, après la liste
//! de fichiers, porte `numWalls=2` puis `numFloors=1` et les 6 flux
//! `[wall1, orient1, wall2, orient2, floor1..N, shadow1]` avec des cellules
//! bit-packées :
//!
//! ```text
//! prop   = bits 0-7    (départ de variation, `prop % candidats`)
//! seq    = bits 8-13
//! unk1   = bits 14-19
//! style  = bits 20-25
//! unk2   = bits 26-30
//! hidden = bit 31
//! ```
//!
//! Les flux d'orientation portent le code de kind du mur (1-15) dans
//! `prop` : 1/2 palissades, 10 porte-warp, 12 feu/props, 14 arbres,
//! 15 tentes. Le flux d'ombre n'est jamais dessiné (il sert au rendu
//! lumière du jeu original). Piège v18 : `w`/`h` stockés sont `(réel - 1)`.

use crate::ds1::Ds1Error;

/// Une cellule bit-packée décodée (champs inutiles au rendu ignorés).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RealCell {
    pub prop: u8,
    pub seq: u8,
    pub style: u8,
    pub hidden: bool,
}

/// Layout réel TownN1 : 2 paires (mur, orientation) + 1 sol + 1 ombre.
#[derive(Debug, Clone)]
pub struct Ds1Real {
    /// Cell width (`stored + 1`).
    pub w: usize,
    /// Cell height (`stored + 1`).
    pub h: usize,
    /// Les chemins `.tg1` du DS1 — définissent l'ordre de concaténation des
    /// conteneurs DT1 (résolution par [`crate::game::resolve_container`]).
    pub files: Vec<String>,
    pub wall1: Vec<RealCell>,
    /// Code de kind par cellule (0 = pas de mur).
    pub orient1: Vec<u8>,
    pub wall2: Vec<RealCell>,
    pub orient2: Vec<u8>,
    pub floor: Vec<RealCell>,
    /// Jamais dessiné (rendu lumière du jeu original) — gardé pour tests.
    pub shadow: Vec<RealCell>,
}

#[inline]
fn unpack(v: i32) -> RealCell {
    RealCell {
        prop: (v & 0xFF) as u8,
        seq: ((v >> 8) & 0x3F) as u8,
        style: ((v >> 20) & 0x3F) as u8,
        hidden: (v >> 31) & 1 == 1,
    }
}

fn i32_at(d: &[u8], at: usize) -> Result<i32, Ds1Error> {
    d.get(at..at + 4)
        .map(|b| i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .ok_or(Ds1Error::TooShort)
}

/// Décode le layout réel. Erreur si la section `(numWalls, numFloors)` ne
/// vaut pas `(2, 1)` — seuls les niveaux du schéma complet sont concernés ;
/// tout autre DS1 reste au chemin historique.
pub fn decode_ds1_real(d: &[u8]) -> Result<Ds1Real, Ds1Error> {
    let version = i32_at(d, 0)?;
    if version != 18 {
        return Err(Ds1Error::UnsupportedVersion(version));
    }
    let stored_w = i32_at(d, 4)?.max(0) as usize;
    let stored_h = i32_at(d, 8)?.max(0) as usize;
    let (w, h) = (stored_w + 1, stored_h + 1);
    let file_count = i32_at(d, 20)?.max(0) as usize;
    let mut off = 24usize;
    let mut files = Vec::with_capacity(file_count);
    for _ in 0..file_count {
        let end = d
            .get(off..)
            .and_then(|rest| rest.iter().position(|&b| b == 0))
            .map(|p| off + p)
            .ok_or(Ds1Error::TooShort)?;
        files.push(String::from_utf8_lossy(&d[off..end]).into_owned());
        off = end + 1;
    }
    let (num_walls, num_floors) = (i32_at(d, off)?, i32_at(d, off + 4)?);
    if num_walls != 2 || num_floors != 1 {
        return Err(Ds1Error::UnsupportedVersion(version));
    }
    off += 8;
    let n = w * h;
    let mut stream = |at: usize| -> Result<Vec<RealCell>, Ds1Error> {
        let mut cells = Vec::with_capacity(n);
        for i in 0..n {
            cells.push(unpack(i32_at(d, at + 4 * i)?));
        }
        Ok(cells)
    };
    let mut orients = |at: usize| -> Result<Vec<u8>, Ds1Error> {
        let mut o = Vec::with_capacity(n);
        for i in 0..n {
            o.push((i32_at(d, at + 4 * i)? & 0xFF) as u8);
        }
        Ok(o)
    };
    let wall1 = stream(off)?;
    let orient1 = orients(off + 4 * n)?;
    let wall2 = stream(off + 8 * n)?;
    let orient2 = orients(off + 12 * n)?;
    let floor = stream(off + 16 * n)?;
    let shadow = stream(off + 20 * n)?;
    Ok(Ds1Real { w, h, files, wall1, orient1, wall2, orient2, floor, shadow })
}
