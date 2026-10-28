//! Image loading: TIFF, JPEG, BMP, PNG, GIF through the `image` crate plus a PCX decoder.

use std::path::Path;

use image::RgbImage;

pub fn load(path: &Path) -> Result<RgbImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if is_pcx(&bytes) {
        return decode_pcx(&bytes);
    }
    image::load_from_memory(&bytes).map(|i| i.to_rgb8()).map_err(|e| e.to_string())
}

fn is_pcx(b: &[u8]) -> bool {
    b.len() > 128 && b[0] == 0x0A && b[1] <= 5 && b[2] == 1
}

pub fn decode_pcx(b: &[u8]) -> Result<RgbImage, String> {
    let u16le = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]) as usize;
    let bpp = b[3] as usize;
    let (x0, y0, x1, y1) = (u16le(4), u16le(6), u16le(8), u16le(10));
    let planes = b[65] as usize;
    let bpl = u16le(66);
    if x1 < x0 || y1 < y0 {
        return Err("Corrupt PCX file.".into());
    }
    let (w, h) = (x1 - x0 + 1, y1 - y0 + 1);
    let supported = matches!((bpp, planes), (1, 1) | (1, 4) | (4, 1) | (8, 1) | (8, 3));
    if !supported || bpl * 8 < w * bpp {
        return Err("This PCX file is of unsupported format.\nConvert it to 2, 256 or 16m colors.".into());
    }

    // RLE-decode all scanlines.
    let line_len = bpl * planes;
    let mut raw = vec![0u8; line_len * h];
    let mut src = 128usize;
    let mut dst = 0usize;
    while dst < raw.len() && src < b.len() {
        let c = b[src];
        src += 1;
        if c & 0xC0 == 0xC0 {
            let n = (c & 0x3F) as usize;
            let v = *b.get(src).ok_or("Corrupt PCX file.")?;
            src += 1;
            let end = (dst + n).min(raw.len());
            raw[dst..end].fill(v);
            dst = end;
        } else {
            raw[dst] = c;
            dst += 1;
        }
    }

    let ega: Vec<[u8; 3]> = (0..16).map(|i| [b[16 + i * 3], b[17 + i * 3], b[18 + i * 3]]).collect();
    let vga: Option<Vec<[u8; 3]>> = (b.len() >= 769 && b[b.len() - 769] == 0x0C).then(|| {
        let p = &b[b.len() - 768..];
        (0..256).map(|i| [p[i * 3], p[i * 3 + 1], p[i * 3 + 2]]).collect()
    });

    let mut img = RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        let line = &raw[y * line_len..(y + 1) * line_len];
        for x in 0..w {
            let rgb = match (bpp, planes) {
                (1, 1) => {
                    let bit = (line[x / 8] >> (7 - x % 8)) & 1;
                    if bit == 1 { [255, 255, 255] } else { [0, 0, 0] }
                }
                (1, 4) => {
                    let mut idx = 0;
                    for p in 0..4 {
                        idx |= ((line[p * bpl + x / 8] >> (7 - x % 8)) & 1) << p;
                    }
                    ega[idx as usize]
                }
                (4, 1) => {
                    let byte = line[x / 2];
                    let idx = if x % 2 == 0 { byte >> 4 } else { byte & 15 };
                    ega[idx as usize]
                }
                (8, 1) => {
                    let idx = line[x] as usize;
                    match &vga {
                        Some(p) => p[idx],
                        None => [idx as u8; 3],
                    }
                }
                _ => [line[x], line[bpl + x], line[2 * bpl + x]],
            };
            img.put_pixel(x as u32, y as u32, image::Rgb(rgb));
        }
    }
    Ok(img)
}
