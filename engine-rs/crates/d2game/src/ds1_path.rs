//! DS1 full-layout decoder with the **PATH section** (NPC patrols) —
//! additive companion to [`crate::ds1`], which stays the validated render
//! path (its 6-stream shortcut only covers the 2-wall town layout).
//!
//! Port of the RE campaign `d2w-re/evidence-formats/ds1-path` (8 real DS1
//! v1.12 samples, all v18, byte-verified against the OpenDiablo2 reference
//! `ds1.go` / `ds1_version.go`). Layout from byte 0:
//!
//! ```text
//! version i32 (this port: > 14, samples are 18)
//! w-1 i32, h-1 i32        — real grid = (w+1) × (h+1), the v18 trap
//! act i32 (v>=8), substitutionType i32 (v>=10)
//! [8 unknown bytes (9<=v<=13)]
//! fileCount u32 + fileCount NUL-terminated .tg1 paths
//! numWalls i32 (v>=4), numFloors i32 (v>=16)
//! streams of w*h i32: per wall → wall stream + orientation stream,
//!                     then the floor streams, then 1 shadow stream,
//!                     +1 substitution stream if substType ∈ {1, 2}
//! objectCount u32 + objects × (type, id, x, y, flags) i32
//! [substitutions (v>=12 ∧ substType ∈ {1,2}): unknown2 u32 (v>=18),
//!  count u32 + groups × (x, y, w, h, unknown) i32]
//! ===== PATH section (v>14) =====
//! npcCount u32                                  — NO magic on v15+
//! per NPC: numPaths u32, x i32, y i32 (the carrier object position)
//! per point: x i32, y i32 [, action i32 when v>15]
//! ```
//!
//! Everything is little-endian dwords without padding: the parse must end
//! exactly at EOF ([`Ds1Path::consumed_to_eof`], 8/8 samples in the
//! evidence). Golden tests: `tests/ds1_path_golden.rs` (townW1 5 NPC /
//! 22 points, lutN 31 NPC / 116 points, vs the decoded JSON evidence).

/// Full DS1 parse: grid, streams with kinds, objects and NPC paths.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ds1Path {
    pub version: i32,
    /// Real cell width (`stored + 1` — the v18 trap, as in [`crate::ds1`]).
    pub w: usize,
    /// Real cell height (`stored + 1`).
    pub h: usize,
    /// Present from v8.
    pub act: Option<i32>,
    /// Present from v10 (0 on all 8 evidence samples).
    pub subst_type: Option<i32>,
    /// The `.tg1` paths in file order — tileset concatenation order.
    pub files: Vec<String>,
    /// Cell streams in file order, each `w * h` packed cells.
    pub layers: Vec<Ds1Layer>,
    /// The placed objects (type 1 = dynamic object / NPC carrier).
    pub objects: Vec<Ds1Object>,
    /// The PATH section: one entry per NPC patrol.
    pub npcs: Vec<NpcPath>,
    /// Byte offset of the PATH section (`None` before v15).
    pub path_section_offset: Option<usize>,
    /// `true` when the parse ended exactly at EOF — the strongest layout
    /// check (evidence §5.1: 0 trailing byte on 8/8 samples).
    pub consumed_to_eof: bool,
}

/// One cell stream, with the role the v18 layer schema gives it
/// (walls first as wall+orientation pairs, then floors, then shadow).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ds1Layer {
    pub kind: LayerKind,
    /// `w * h` packed cells, row-major (`y * w + x`).
    pub cells: Vec<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerKind {
    Wall,
    /// The orientation stream paired with the preceding [`LayerKind::Wall`].
    Orientation,
    Floor,
    /// Always exactly one stream, after the floors.
    Shadow,
    /// Extra stream when `substType ∈ {1, 2}` (never seen on the samples).
    Substitution,
}

/// A placed object — `(type, id, x, y, flags)`, all i32 on disk. `type 1`
/// is a dynamic object (unit / NPC), `id` the monster id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ds1Object {
    pub kind: i32,
    pub id: i32,
    pub x: i32,
    pub y: i32,
    pub flags: i32,
}

/// One waypoint of a patrol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathPoint {
    pub x: i32,
    pub y: i32,
    /// Action code at the point (observed: 1..=4) — only read for v>15
    /// (`specifiesNPCActions`); `None` on v15.
    pub action: Option<i32>,
}

/// One NPC entry of the PATH section: the carrier object position plus
/// its waypoints. The object link is by exact `(x, y)` equality against
/// the object table (OpenDiablo2 ds1.go:421-429) — [`NpcPath::object_index`]
/// is `None` when orphaned (the points are still read, unlike OD2 which
/// skips them; the format has no object index, the position IS the key).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NpcPath {
    pub x: i32,
    pub y: i32,
    pub paths: Vec<PathPoint>,
    /// Index into [`Ds1Path::objects`] when the carrier object exists.
    pub object_index: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ds1PathError {
    TooShort,
    /// This port covers the v15+ layout (PATH section); the evidence
    /// samples are all v18.
    UnsupportedVersion(i32),
}

struct Cursor<'a> {
    d: &'a [u8],
    p: usize,
}

impl<'a> Cursor<'a> {
    fn i32(&mut self) -> Result<i32, Ds1PathError> {
        let b = self.d.get(self.p..self.p + 4).ok_or(Ds1PathError::TooShort)?;
        self.p += 4;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u32(&mut self) -> Result<u32, Ds1PathError> {
        let b = self.d.get(self.p..self.p + 4).ok_or(Ds1PathError::TooShort)?;
        self.p += 4;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn skip(&mut self, n: usize) -> Result<(), Ds1PathError> {
        if self.p + n > self.d.len() {
            return Err(Ds1PathError::TooShort);
        }
        self.p += n;
        Ok(())
    }

    fn nul_str(&mut self) -> Result<String, Ds1PathError> {
        let rest = self.d.get(self.p..).ok_or(Ds1PathError::TooShort)?;
        let len = rest.iter().position(|&b| b == 0).ok_or(Ds1PathError::TooShort)?;
        let s = String::from_utf8_lossy(&rest[..len]).into_owned();
        self.p += len + 1; // consume the NUL
        Ok(s)
    }
}

/// Decode a full DS1 (v15+ layout, samples v18) including objects and the
/// PATH section.
pub fn decode_ds1_paths(d: &[u8]) -> Result<Ds1Path, Ds1PathError> {
    let mut c = Cursor { d, p: 0 };
    let version = c.i32()?;
    if version <= 14 {
        return Err(Ds1PathError::UnsupportedVersion(version));
    }
    let w = c.i32()?.max(0) as usize + 1; // v18 trap: real = stored + 1
    let h = c.i32()?.max(0) as usize + 1;
    let act = (version >= 8).then(|| c.i32()).transpose()?;
    let subst_type = (version >= 10).then(|| c.i32()).transpose()?;
    if (9..=13).contains(&version) {
        c.skip(8)?; // unknown1
    }

    // --- file list (NUL-terminated .tg1 paths) ---------------------------
    let file_count = c.u32()? as usize;
    let mut files = Vec::with_capacity(file_count.min(1 << 12));
    for _ in 0..file_count {
        files.push(c.nul_str()?);
    }

    // --- cell streams ----------------------------------------------------
    let n_walls = if version >= 4 { c.i32()?.max(0) as usize } else { 0 };
    let n_floors = if version >= 16 { c.i32()?.max(0) as usize } else { 0 };
    let has_subst = version >= 12 && matches!(subst_type, Some(1) | Some(2));
    let n_streams = n_walls * 2 + n_floors + 1 + usize::from(has_subst);
    let n = w * h;
    let mut layers = Vec::with_capacity(n_streams);
    for s in 0..n_streams {
        let kind = if s < n_walls * 2 {
            if s % 2 == 0 { LayerKind::Wall } else { LayerKind::Orientation }
        } else if s < n_walls * 2 + n_floors {
            LayerKind::Floor
        } else if s == n_walls * 2 + n_floors {
            LayerKind::Shadow
        } else {
            LayerKind::Substitution
        };
        let mut cells = Vec::with_capacity(n);
        for _ in 0..n {
            cells.push(c.i32()?);
        }
        layers.push(Ds1Layer { kind, cells });
    }

    // --- objects ---------------------------------------------------------
    let object_count = c.u32()? as usize;
    let mut objects = Vec::with_capacity(object_count.min(1 << 16));
    for _ in 0..object_count {
        objects.push(Ds1Object {
            kind: c.i32()?,
            id: c.i32()?,
            x: c.i32()?,
            y: c.i32()?,
            flags: c.i32()?,
        });
    }

    // --- substitutions (substType 1/2 only; never on the samples) --------
    if has_subst {
        if version >= 18 {
            c.u32()?; // unknown2
        }
        let groups = c.u32()? as usize;
        c.skip(groups.saturating_mul(5 * 4))?;
    }

    // ===== PATH section (v>14, no magic) =================================
    let path_section_offset = c.p;
    let has_actions = version > 15;
    let npc_count = c.u32()?;
    let mut npcs = Vec::with_capacity((npc_count as usize).min(1 << 16));
    for _ in 0..npc_count {
        let num_paths = c.u32()? as usize;
        let x = c.i32()?;
        let y = c.i32()?;
        let object_index =
            objects.iter().position(|o| o.x == x && o.y == y); // ds1.go:421-429
        let mut paths = Vec::with_capacity(num_paths.min(1 << 16));
        for _ in 0..num_paths {
            let px = c.i32()?;
            let py = c.i32()?;
            let action = if has_actions { Some(c.i32()?) } else { None };
            paths.push(PathPoint { x: px, y: py, action });
        }
        npcs.push(NpcPath { x, y, paths, object_index });
    }

    Ok(Ds1Path {
        version,
        w,
        h,
        act,
        subst_type,
        files,
        layers,
        objects,
        npcs,
        path_section_offset: Some(path_section_offset),
        consumed_to_eof: c.p == d.len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_versions_are_rejected() {
        assert_eq!(
            decode_ds1_paths(&[14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            Err(Ds1PathError::UnsupportedVersion(14))
        );
        assert_eq!(decode_ds1_paths(&[]), Err(Ds1PathError::TooShort));
    }

    /// A minimal v18-shaped file (1×1 grid, 0 walls/floors + shadow,
    /// no objects, one NPC with one action point) parsed to the exact EOF.
    #[test]
    fn minimal_v18_layout_roundtrip() {
        let mut d = Vec::new();
        let put = |d: &mut Vec<u8>, v: i32| d.extend_from_slice(&v.to_le_bytes());
        put(&mut d, 18); // version
        put(&mut d, 0); // w-1 → w = 1
        put(&mut d, 0); // h-1 → h = 1
        put(&mut d, 0); // act
        put(&mut d, 0); // substType
        put(&mut d, 0); // fileCount
        put(&mut d, 0); // numWalls
        put(&mut d, 0); // numFloors
        put(&mut d, 0); // shadow stream (1 cell)
        put(&mut d, 0); // objectCount
        put(&mut d, 1); // npcCount
        put(&mut d, 1); // numPaths
        put(&mut d, 7); // npc.x
        put(&mut d, 9); // npc.y
        put(&mut d, 3); // point.x
        put(&mut d, 4); // point.y
        put(&mut d, 2); // point.action (v18 > 15)
        let ds1 = decode_ds1_paths(&d).unwrap();
        assert_eq!(ds1.w, 1);
        assert_eq!(ds1.h, 1);
        assert_eq!(ds1.layers.len(), 1);
        assert_eq!(ds1.layers[0].kind, LayerKind::Shadow);
        assert!(ds1.consumed_to_eof);
        assert_eq!(ds1.path_section_offset, Some(40));
        assert_eq!(ds1.objects, vec![]);
        assert_eq!(
            ds1.npcs,
            vec![NpcPath {
                x: 7,
                y: 9,
                paths: vec![PathPoint { x: 3, y: 4, action: Some(2) }],
                object_index: None,
            }]
        );
    }

    /// The carrier object link: an object at the NPC position attaches.
    #[test]
    fn npc_attaches_to_object_by_exact_position() {
        let mut d = Vec::new();
        let put = |d: &mut Vec<u8>, v: i32| d.extend_from_slice(&v.to_le_bytes());
        put(&mut d, 18);
        for _ in 0..3 {
            put(&mut d, 0); // w-1, h-1, act
        }
        put(&mut d, 0); // substType
        put(&mut d, 0); // fileCount
        put(&mut d, 0); // numWalls
        put(&mut d, 0); // numFloors
        put(&mut d, 0); // shadow
        put(&mut d, 2); // objectCount
        put(&mut d, 1); // obj0 type=1
        put(&mut d, 7); // obj0 id=7
        put(&mut d, 56); // obj0 x
        put(&mut d, 100); // obj0 y
        put(&mut d, 0); // obj0 flags
        put(&mut d, 1); // obj1 (elsewhere)
        put(&mut d, 9);
        put(&mut d, 1);
        put(&mut d, 1);
        put(&mut d, 0);
        put(&mut d, 1); // npcCount
        put(&mut d, 0); // numPaths
        put(&mut d, 56); // npc at the object position
        put(&mut d, 100);
        let ds1 = decode_ds1_paths(&d).unwrap();
        assert_eq!(ds1.npcs[0].object_index, Some(0));
        assert_eq!((ds1.objects[0].kind, ds1.objects[0].id), (1, 7));
    }
}
