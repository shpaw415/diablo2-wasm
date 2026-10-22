//! Minimal PNG writer — color type 2 (truecolor), filter 0 rows, stored
//! deflate blocks. No dependencies; keeps the render tool dep-free.

fn crc32_table() -> [u32; 256] {
    let mut t = [0u32; 256];
    for (n, e) in t.iter_mut().enumerate() {
        let mut c = n as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *e = c;
    }
    t
}

fn crc32(data: &[u8]) -> u32 {
    let t = crc32_table();
    let mut c = 0xFFFF_FFFFu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFF_FFFF
}

fn adler32(data: &[u8]) -> u32 {
    let mut a: u32 = 1;
    let mut b: u32 = 0;
    for &byte in data {
        a = (a + byte as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}

fn push_be32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_be_bytes());
}

fn push_chunk(out: &mut Vec<u8>, tag: &[u8; 4], data: &[u8]) {
    push_be32(out, data.len() as u32);
    out.extend_from_slice(tag);
    out.extend_from_slice(data);
    let mut crc_input = Vec::with_capacity(4 + data.len());
    crc_input.extend_from_slice(tag);
    crc_input.extend_from_slice(data);
    push_be32(out, crc32(&crc_input));
}

/// Stored-deflate (BTYPE 00) of `raw`.
fn stored_deflate(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len() + raw.len() / 65535 * 5 + 16);
    out.push(0x78);
    out.push(0x01);
    let mut chunks = raw.chunks(65535).peekable();
    if raw.is_empty() {
        out.push(0x01);
        push_be32(&mut out, 0);
        push_be32(&mut out, 0xFFFF_FFFF);
    }
    while let Some(chunk) = chunks.next() {
        let last = if chunks.peek().is_none() { 1u8 } else { 0u8 };
        out.push(last);
        let len = chunk.len() as u16;
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(chunk);
    }
    out.extend_from_slice(&adler32(raw).to_be_bytes());
    out
}

/// Write an 8-bit RGB PNG.
pub fn write_png_rgb(path: &str, w: u32, h: u32, rgb: &[u8]) -> Result<(), String> {
    if rgb.len() != (w as usize) * (h as usize) * 3 {
        return Err(format!(
            "rgb buffer {} does not match {}x{}x3",
            rgb.len(),
            w,
            h
        ));
    }

    // Raw scanlines with filter byte 0.
    let mut raw = Vec::with_capacity((w * h * 3 + h) as usize);
    for y in 0..h {
        raw.push(0u8);
        let start = (y * w * 3) as usize;
        raw.extend_from_slice(&rgb[start..start + (w * 3) as usize]);
    }
    write_png(path, w, h, 2, raw)
}

/// Write an 8-bit RGBA PNG (alpha channel, for transparent sprites).
pub fn write_png_rgba(path: &str, w: u32, h: u32, rgba: &[u8]) -> Result<(), String> {
    if rgba.len() != (w as usize) * (h as usize) * 4 {
        return Err(format!(
            "rgba buffer {} does not match {}x{}x4",
            rgba.len(),
            w,
            h
        ));
    }

    let mut raw = Vec::with_capacity((w * h * 4 + h) as usize);
    for y in 0..h {
        raw.push(0u8);
        let start = (y * w * 4) as usize;
        raw.extend_from_slice(&rgba[start..start + (w * 4) as usize]);
    }
    write_png(path, w, h, 6, raw)
}

fn write_png(path: &str, w: u32, h: u32, color_type: u8, raw: Vec<u8>) -> Result<(), String> {
    let mut png = Vec::new();
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);

    let mut ihdr = Vec::with_capacity(13);
    push_be32(&mut ihdr, w);
    push_be32(&mut ihdr, h);
    ihdr.push(8); // bit depth
    ihdr.push(color_type); // 2 = truecolor, 6 = truecolor + alpha
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    push_chunk(&mut png, b"IHDR", &ihdr);
    push_chunk(&mut png, b"IDAT", &stored_deflate(&raw));
    push_chunk(&mut png, b"IEND", &[]);

    std::fs::write(path, &png).map_err(|e| e.to_string())
}
