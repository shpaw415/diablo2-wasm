//! DCC direction frame header and cell re-tiling.

use crate::cell::DccCell;
use crate::direction::DccDirection;
use crate::error::DccError;
use crate::bitmuncher::BitMuncher;

/// Port of OpenDiablo2 `DCCDirectionFrame` (dcc_direction_frame.go).
#[derive(Debug, Clone, Default)]
pub struct DccDirectionFrame {
    /// Frame box: `left = x_offset`, `top = y_offset - height + 1`.
    /// Width/height are the frame pixel size; `left/top` are positions
    /// relative to the direction box origin.
    pub box_left: i32,
    pub box_top: i32,
    pub box_width: i32,
    pub box_height: i32,
    pub cells: Vec<DccCell>,
    pub pixel_data: Vec<u8>,
    pub width: i32,
    pub height: i32,
    pub x_offset: i32,
    pub y_offset: i32,
    pub number_of_optional_bytes: i32,
    pub number_of_coded_bytes: i32,
    pub horizontal_cell_count: i32,
    pub vertical_cell_count: i32,
    pub frame_is_bottom_up: bool,
}

impl DccDirectionFrame {
    /// Reads one frame header (dcc_direction_frame.go `CreateDCCDirectionFrame`).
    pub fn parse(
        bits: &mut BitMuncher,
        direction: &DccDirection,
    ) -> Result<DccDirectionFrame, DccError> {
        let mut f = DccDirectionFrame::default();

        let _variable0 = bits.get_bits(direction.variable0_bits as usize);

        f.width = bits.get_bits(direction.width_bits as usize) as i32;
        f.height = bits.get_bits(direction.height_bits as usize) as i32;
        f.x_offset = bits.get_signed_bits(direction.x_offset_bits as usize);
        f.y_offset = bits.get_signed_bits(direction.y_offset_bits as usize);
        f.number_of_optional_bytes = bits.get_bits(direction.optional_data_bits as usize) as i32;
        f.number_of_coded_bytes = bits.get_bits(direction.coded_bytes_bits as usize) as i32;
        f.frame_is_bottom_up = bits.get_bit() == 1;

        if f.frame_is_bottom_up {
            return Err(DccError::BottomUpFrame);
        }

        f.box_left = f.x_offset;
        f.box_top = f.y_offset - f.height + 1;
        f.box_width = f.width;
        f.box_height = f.height;

        Ok(f)
    }

    /// Re-tiles the frame onto the direction's 4-px grid, honoring the
    /// direction-box alignment of the first column/row
    /// (dcc_direction_frame.go `recalculateCells`).
    pub fn recalculate_cells(&mut self, direction: &DccDirection) {
        self.recalculate_cells_boxed(direction.box_left, direction.box_top);
    }

    /// Same, taking only the direction box origin it actually depends on
    /// (lets callers re-tile frames without aliasing the direction).
    pub fn recalculate_cells_boxed(&mut self, dir_box_left: i32, dir_box_top: i32) {
        // Width of the first column. Go `%` truncates toward zero as does
        // Rust `%` on i32, so the port is exact (left/top deltas can be
        // negative here).
        let w = 4 - ((self.box_left - dir_box_left) % 4);

        self.horizontal_cell_count = if (self.width - w) <= 1 {
            1
        } else {
            let tmp = self.width - w - 1;
            let mut c = 2 + tmp / 4;
            if tmp % 4 == 0 {
                c -= 1;
            }
            c
        };

        let h = 4 - ((self.box_top - dir_box_top) % 4);

        self.vertical_cell_count = if (self.height - h) <= 1 {
            1
        } else {
            let tmp = self.height - h - 1;
            let mut c = 2 + tmp / 4;
            if tmp % 4 == 0 {
                c -= 1;
            }
            c
        };

        let hcc = self.horizontal_cell_count as usize;
        let vcc = self.vertical_cell_count as usize;

        let mut cell_widths = vec![0i32; hcc];
        if hcc == 1 {
            cell_widths[0] = self.width;
        } else {
            cell_widths[0] = w;
            for cw in cell_widths.iter_mut().take(hcc - 1).skip(1) {
                *cw = 4;
            }
            cell_widths[hcc - 1] = self.width - w - (4 * (hcc as i32 - 2));
        }

        let mut cell_heights = vec![0i32; vcc];
        if vcc == 1 {
            cell_heights[0] = self.height;
        } else {
            cell_heights[0] = h;
            for ch in cell_heights.iter_mut().take(vcc - 1).skip(1) {
                *ch = 4;
            }
            cell_heights[vcc - 1] = self.height - h - (4 * (vcc as i32 - 2));
        }

        let mut cells = Vec::with_capacity(hcc * vcc);
        let mut offset_y = self.box_top - dir_box_top;
        for y in 0..vcc {
            let mut offset_x = self.box_left - dir_box_left;
            for x in 0..hcc {
                cells.push(DccCell {
                    width: cell_widths[x],
                    height: cell_heights[y],
                    x_offset: offset_x,
                    y_offset: offset_y,
                    ..Default::default()
                });
                offset_x += cell_widths[x];
            }
            offset_y += cell_heights[y];
        }
        self.cells = cells;
    }
}
