//! DCC direction — one facing of a DCC, with its frames and cell coding.

use crate::bitmuncher::BitMuncher;
use crate::cell::DccCell;
use crate::error::DccError;
use crate::frame::DccDirectionFrame;
use crate::pixel_buffer::PixelBufferEntry;

const BASE_MIN: i32 = 100_000;
const BASE_MAX: i32 = -100_000;
const CELLS_PER_ROW: i32 = 4;

/// Go `crazyBitTable`: 4-bit field -> real bit width.
const CRAZY_BIT_TABLE: [u32; 16] = [0, 1, 2, 4, 6, 8, 10, 12, 14, 16, 20, 24, 26, 28, 30, 32];

/// Go `pixelMaskLookup`: 4-bit pixel mask -> number of pixel values coded.
const PIXEL_MASK_LOOKUP: [usize; 16] = [
    0, 1, 1, 2, 1, 2, 2, 3, 1, 2, 2, 3, 2, 3, 3, 4,
];

/// Port of OpenDiablo2 `DCCDirection` (dcc_direction.go).
///
/// After `parse`, every frame owns a `pixel_data` buffer of
/// `box_width * box_height` palette indices, 0 = transparent, with the
/// frame's box sub-rectangle painted (frame boxes live in the direction
/// box coordinate space).
#[derive(Debug, Clone)]
pub struct DccDirection {
    pub out_size_coded: u32,
    pub compression_flags: u32,
    pub variable0_bits: u32,
    pub width_bits: u32,
    pub height_bits: u32,
    pub x_offset_bits: u32,
    pub y_offset_bits: u32,
    pub optional_data_bits: u32,
    pub coded_bytes_bits: u32,
    pub equal_cells_bitstream_size: usize,
    pub pixel_mask_bitstream_size: usize,
    pub encoding_type_bitstream_size: usize,
    pub raw_pixel_codes_bitstream_size: usize,
    pub frames: Vec<DccDirectionFrame>,
    pub palette_entries: [u8; 256],
    pub box_left: i32,
    pub box_top: i32,
    pub box_width: i32,
    pub box_height: i32,
}

impl Default for DccDirection {
    fn default() -> Self {
        DccDirection {
            out_size_coded: 0,
            compression_flags: 0,
            variable0_bits: 0,
            width_bits: 0,
            height_bits: 0,
            x_offset_bits: 0,
            y_offset_bits: 0,
            optional_data_bits: 0,
            coded_bytes_bits: 0,
            equal_cells_bitstream_size: 0,
            pixel_mask_bitstream_size: 0,
            encoding_type_bitstream_size: 0,
            raw_pixel_codes_bitstream_size: 0,
            frames: Vec::new(),
            palette_entries: [0; 256],
            box_left: 0,
            box_top: 0,
            box_width: 0,
            box_height: 0,
        }
    }
}

impl DccDirection {
    /// Reads and fully decodes one direction starting at the current bit
    /// position of `bm` (dcc_direction.go `CreateDCCDirection`).
    pub fn parse(
        bm: &mut BitMuncher,
        frames_per_direction: usize,
    ) -> Result<DccDirection, DccError> {
        let mut dir = DccDirection::default();

        dir.out_size_coded = bm.get_uint32();
        dir.compression_flags = bm.get_bits(2);
        dir.variable0_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];
        dir.width_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];
        dir.height_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];
        dir.x_offset_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];
        dir.y_offset_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];
        dir.optional_data_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];
        dir.coded_bytes_bits = CRAZY_BIT_TABLE[bm.get_bits(4) as usize];

        let mut min_x = BASE_MIN;
        let mut min_y = BASE_MIN;
        let mut max_x = BASE_MAX;
        let mut max_y = BASE_MAX;

        for _ in 0..frames_per_direction {
            let frame = DccDirectionFrame::parse(bm, &dir)?;
            min_x = min_x.min(frame.box_left);
            min_y = min_y.min(frame.box_top);
            max_x = max_x.max(frame.box_left + frame.box_width);
            max_y = max_y.max(frame.box_top + frame.box_height);
            dir.frames.push(frame);
        }

        dir.box_left = min_x;
        dir.box_top = min_y;
        dir.box_width = max_x - min_x;
        dir.box_height = max_y - min_y;

        if dir.optional_data_bits > 0 {
            return Err(DccError::OptionalBytesUnsupported);
        }

        if dir.compression_flags & 0x2 > 0 {
            dir.equal_cells_bitstream_size = bm.get_bits(20) as usize;
        }
        dir.pixel_mask_bitstream_size = bm.get_bits(20) as usize;
        if dir.compression_flags & 0x1 > 0 {
            dir.encoding_type_bitstream_size = bm.get_bits(20) as usize;
            dir.raw_pixel_codes_bitstream_size = bm.get_bits(20) as usize;
        }

        // Palette presence bitmap -> compact list of used entries.
        let mut count = 0usize;
        for i in 0..256u32 {
            if bm.get_bit() != 0 {
                dir.palette_entries[count] = i as u8;
                count += 1;
            }
        }

        // HERE BE GIANTS (dcc_direction.go): the five sub-bitstreams all
        // start at the *current bit offset*, then the main stream skips
        // past each one. Copying the muncher preserves the intra-byte bit
        // position, so a 20-byte stream starting at byte 3 bit 6 ends at
        // byte 23 bit 6.
        let mut equal_cells_stream = bm.fork();
        bm.skip_bits(dir.equal_cells_bitstream_size);

        let mut pixel_mask_stream = bm.fork();
        bm.skip_bits(dir.pixel_mask_bitstream_size);

        let mut encoding_type_stream = bm.fork();
        bm.skip_bits(dir.encoding_type_bitstream_size);

        let mut raw_pixel_codes_stream = bm.fork();
        bm.skip_bits(dir.raw_pixel_codes_bitstream_size);

        let mut pixel_code_stream = bm.fork();

        // Direction-level buffer cells (4-px grid over the direction box).
        let buffer_cells = dir.calculate_cells();

        // Per-frame cells re-tiled onto the direction grid; the re-tile
        // also fills each frame's cell counts (dcc_direction.go calls
        // recalculateCells on the live frames before decoding).
        let frames_cells: Vec<Vec<DccCell>> = dir
            .frames
            .iter_mut()
            .map(|f| {
                f.recalculate_cells_boxed(dir.box_left, dir.box_top);
                f.cells.clone()
            })
            .collect();

        let pixel_buffer = dir.fill_pixel_buffer(
            &mut pixel_code_stream,
            &mut equal_cells_stream,
            &mut pixel_mask_stream,
            &mut encoding_type_stream,
            &mut raw_pixel_codes_stream,
            &frames_cells,
        );

        dir.generate_frames(&mut pixel_code_stream, &pixel_buffer, &buffer_cells);

        // Sanity: each sub-bitstream must have consumed exactly its size.
        let streams: [(&'static str, &BitMuncher, usize); 4] = [
            ("equalCells", &equal_cells_stream, dir.equal_cells_bitstream_size),
            ("pixelMask", &pixel_mask_stream, dir.pixel_mask_bitstream_size),
            ("encodingType", &encoding_type_stream, dir.encoding_type_bitstream_size),
            ("rawPixelCodes", &raw_pixel_codes_stream, dir.raw_pixel_codes_bitstream_size),
        ];
        for (name, s, expected) in streams {
            if s.bits_read() != expected {
                return Err(DccError::BitstreamSizeMismatch {
                    stream: name,
                    expected,
                    read: s.bits_read(),
                });
            }
        }

        // Consume the pixel-code/displacement stream so a subsequent
        // direction starts at the right bit (Go `bm.SkipBits(pcd.BitsRead())`).
        bm.skip_bits(pixel_code_stream.bits_read());

        Ok(dir)
    }

    /// Direction-level 4-px grid cells (dcc_direction.go `calculateCells`).
    fn calculate_cells(&self) -> Vec<DccCell> {
        let hcc = (1 + (self.box_width - 1) / CELLS_PER_ROW) as usize;
        let vcc = (1 + (self.box_height - 1) / CELLS_PER_ROW) as usize;

        let mut cell_widths = vec![0i32; hcc];
        if hcc == 1 {
            cell_widths[0] = self.box_width;
        } else {
            for cw in cell_widths.iter_mut().take(hcc - 1) {
                *cw = 4;
            }
            cell_widths[hcc - 1] = self.box_width - (4 * (hcc as i32 - 1));
        }

        let mut cell_heights = vec![0i32; vcc];
        if vcc == 1 {
            cell_heights[0] = self.box_height;
        } else {
            for ch in cell_heights.iter_mut().take(vcc - 1) {
                *ch = 4;
            }
            cell_heights[vcc - 1] = self.box_height - (4 * (vcc as i32 - 1));
        }

        let mut cells = Vec::with_capacity(hcc * vcc);
        for y in 0..vcc {
            let mut x_offset = 0;
            let y_offset = (y as i32) * CELLS_PER_ROW;
            for x in 0..hcc {
                cells.push(DccCell {
                    width: cell_widths[x],
                    height: cell_heights[y],
                    x_offset,
                    y_offset,
                    ..Default::default()
                });
                x_offset += 4;
            }
        }
        cells
    }

    #[allow(clippy::too_many_arguments)]
    fn fill_pixel_buffer(
        &self,
        pcd: &mut BitMuncher,
        ec: &mut BitMuncher,
        pm: &mut BitMuncher,
        et: &mut BitMuncher,
        rp: &mut BitMuncher,
        frames_cells: &[Vec<DccCell>],
    ) -> Vec<PixelBufferEntry> {
        // Go sums the per-frame (re-tiled) cell counts, then takes the
        // product of the two sums — an over-allocation, ported as-is.
        let mut max_cell_x = 0usize;
        let mut max_cell_y = 0usize;
        for frame in &self.frames {
            max_cell_x += frame.horizontal_cell_count.max(0) as usize;
            max_cell_y += frame.vertical_cell_count.max(0) as usize;
        }

        let mut pixel_buffer = vec![
            PixelBufferEntry {
                frame: -1,
                frame_cell_index: -1,
                ..Default::default()
            };
            max_cell_x * max_cell_y
        ];

        let dir_hcc = (1 + (self.box_width - 1) / CELLS_PER_ROW).max(1) as usize;
        let mut cell_buffer = vec![-1i32; dir_hcc * (1 + (self.box_height - 1) / CELLS_PER_ROW).max(1) as usize];

        let mut pb_index: i64 = -1;
        let mut last_pixel: u32;

        for (frame_index, frame) in self.frames.iter().enumerate() {
            let origin_cell_x = (frame.box_left - self.box_left) / CELLS_PER_ROW;
            let origin_cell_y = (frame.box_top - self.box_top) / CELLS_PER_ROW;
            let frame_hcc = frame.horizontal_cell_count.max(1) as usize;

            for cell_y in 0..frame.vertical_cell_count.max(0) as usize {
                let current_cell_y = cell_y as i32 + origin_cell_y;
                for cell_x in 0..frame_hcc {
                    let current_cell =
                        (origin_cell_x as usize) + cell_x + (current_cell_y as usize) * dir_hcc;

                    let mut next_cell = false;
                    let pixel_mask: u32;

                    if cell_buffer[current_cell] >= 0 {
                        let tmp = if self.equal_cells_bitstream_size > 0 {
                            ec.get_bit() as usize
                        } else {
                            0
                        };
                        if tmp == 0 {
                            pixel_mask = pm.get_bits(4);
                        } else {
                            next_cell = true;
                            pixel_mask = 0;
                        }
                    } else {
                        pixel_mask = 0x0F;
                    }

                    if next_cell {
                        continue;
                    }

                    // Decode the pixel stack for this buffer cell.
                    let mut pixel_stack = [0u32; 4];
                    last_pixel = 0;
                    let number_of_pixel_bits = PIXEL_MASK_LOOKUP[pixel_mask as usize];
                    let encoding_type = if number_of_pixel_bits != 0
                        && self.encoding_type_bitstream_size > 0
                    {
                        et.get_bit()
                    } else {
                        0
                    };

                    let mut decoded_pixel = 0usize;
                    for slot in pixel_stack.iter_mut().take(number_of_pixel_bits) {
                        if encoding_type != 0 {
                            *slot = rp.get_bits(8);
                        } else {
                            *slot = last_pixel;
                            let mut displacement = pcd.get_bits(4);
                            *slot = last_pixel.wrapping_add(displacement);
                            while displacement == 15 {
                                displacement = pcd.get_bits(4);
                                *slot = slot.wrapping_add(displacement);
                            }
                        }

                        if *slot == last_pixel {
                            *slot = 0;
                            break;
                        }
                        last_pixel = *slot;
                        decoded_pixel += 1;
                    }

                    let old_values: Option<[u8; 4]> =
                        if cell_buffer[current_cell] >= 0 {
                            Some(pixel_buffer[cell_buffer[current_cell] as usize].value)
                        } else {
                            None
                        };

                    pb_index += 1;
                    let mut cur_idx: i64 = decoded_pixel as i64 - 1;
                    let mut value = [0u8; 4];
                    for (i, v) in value.iter_mut().enumerate() {
                        if pixel_mask & (1 << i) != 0 {
                            if cur_idx >= 0 {
                                *v = pixel_stack[cur_idx as usize] as u8;
                                cur_idx -= 1;
                            } else {
                                *v = 0;
                            }
                        } else {
                            *v = old_values.map_or(0, |ov| ov[i]);
                        }
                    }

                    let entry = &mut pixel_buffer[pb_index as usize];
                    entry.value = value;
                    entry.frame = frame_index as i32;
                    entry.frame_cell_index =
                        (cell_x + cell_y * frame_hcc) as i32;
                    cell_buffer[current_cell] = pb_index as i32;
                }
            }
        }

        // Convert stack indices into real palette entries.
        let n = (pb_index + 1).max(0) as usize;
        for entry in pixel_buffer.iter_mut().take(n) {
            for v in entry.value.iter_mut() {
                *v = self.palette_entries[*v as usize];
            }
        }

        pixel_buffer
    }

    fn generate_frames(
        &mut self,
        pcd: &mut BitMuncher,
        pixel_buffer: &[PixelBufferEntry],
        buffer_cells: &[DccCell],
    ) {
        let bw = self.box_width as usize;
        let bh = self.box_height as usize;
        let dir_hcc = (1 + (self.box_width - 1) / CELLS_PER_ROW).max(1) as usize;
        let mut dir_pixels = vec![0u8; bw * bh];

        // Mutable last_* state for the direction-level buffer cells.
        let mut buffer_cells: Vec<DccCell> = buffer_cells.to_vec();
        let mut pb_idx = 0usize;

        // Take frame cells out so frames can be written while iterating
        // (Go frees frame.Cells at the end of the loop too).
        let frames_cells: Vec<Vec<DccCell>> = self
            .frames
            .iter_mut()
            .map(|f| std::mem::take(&mut f.cells))
            .collect();

        for (frame_index, cells) in frames_cells.iter().enumerate() {
            let mut frame_pixels = vec![0u8; bw * bh];

            for (c, cell) in cells.iter().enumerate() {
                let cell_x = (cell.x_offset / CELLS_PER_ROW) as usize;
                let cell_y = (cell.y_offset / CELLS_PER_ROW) as usize;
                let cell_index = cell_x + cell_y * dir_hcc;
                let buffer_cell = &mut buffer_cells[cell_index];
                let pbe = pixel_buffer[pb_idx];

                if pbe.frame != frame_index as i32 || pbe.frame_cell_index != c as i32 {
                    // Equal-cell bit set: copy the previous frame cell or
                    // clear this one.
                    if cell.width != buffer_cell.last_width
                        || cell.height != buffer_cell.last_height
                    {
                        for y in 0..cell.height {
                            for x in 0..cell.width {
                                let idx = (x + cell.x_offset) as usize
                                    + ((y + cell.y_offset) as usize) * bw;
                                dir_pixels[idx] = 0;
                            }
                        }
                    } else {
                        // Copy old cell (last_*) into the new position,
                        // direction scratch -> direction scratch -> frame.
                        for fy in 0..cell.height {
                            for fx in 0..cell.width {
                                let src = (fx + buffer_cell.last_x_offset) as usize
                                    + ((fy + buffer_cell.last_y_offset) as usize) * bw;
                                let dst = (fx + cell.x_offset) as usize
                                    + ((fy + cell.y_offset) as usize) * bw;
                                dir_pixels[dst] = dir_pixels[src];
                            }
                        }
                        for fy in 0..cell.height {
                            for fx in 0..cell.width {
                                let src = (cell.x_offset + fx) as usize
                                    + ((cell.y_offset + fy) as usize) * bw;
                                frame_pixels[src] = dir_pixels[src];
                            }
                        }
                    }
                } else {

                    if pbe.value[0] == pbe.value[1] {
                        for y in 0..cell.height {
                            for x in 0..cell.width {
                                let idx = (x + cell.x_offset) as usize
                                    + ((y + cell.y_offset) as usize) * bw;
                                dir_pixels[idx] = pbe.value[0];
                            }
                        }
                    } else {
                        let bits_to_read = if pbe.value[1] != pbe.value[2] { 2 } else { 1 };
                        for y in 0..cell.height {
                            for x in 0..cell.width {
                                let palette_index = pcd.get_bits(bits_to_read) as usize;
                                let idx = (x + cell.x_offset) as usize
                                    + ((y + cell.y_offset) as usize) * bw;
                                dir_pixels[idx] = pbe.value[palette_index];
                            }
                        }
                    }

                    // Copy the painted cell into the frame.
                    for fy in 0..cell.height {
                        for fx in 0..cell.width {
                            let src = (fx + cell.x_offset) as usize
                                + ((fy + cell.y_offset) as usize) * bw;
                            frame_pixels[src] = dir_pixels[src];
                        }
                    }
                    pb_idx += 1;
                }

                buffer_cell.last_width = cell.width;
                buffer_cell.last_height = cell.height;
                buffer_cell.last_x_offset = cell.x_offset;
                buffer_cell.last_y_offset = cell.y_offset;
            }

            self.frames[frame_index].pixel_data = frame_pixels;
        }
    }
}
