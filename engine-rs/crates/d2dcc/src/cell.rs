//! DCC cell — a 4×4-ish tile of pixels inside a direction/frame.

/// Port of OpenDiablo2 `DCCCell` (dcc_cell.go).
///
/// `last_*` fields carry the previous position of this direction-level
/// buffer cell so equal-cell frames can copy the old pixels forward.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DccCell {
    pub width: i32,
    pub height: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub last_width: i32,
    pub last_height: i32,
    pub last_x_offset: i32,
    pub last_y_offset: i32,
}
