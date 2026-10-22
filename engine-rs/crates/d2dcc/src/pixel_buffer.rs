//! Pixel buffer entry — up to 4 palette values shared by a buffer cell
//! across frames.

/// Port of OpenDiablo2 `DCCPixelBufferEntry` (dcc_pixel_buffer_entry.go).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PixelBufferEntry {
    pub value: [u8; 4],
    pub frame: i32,
    pub frame_cell_index: i32,
}
