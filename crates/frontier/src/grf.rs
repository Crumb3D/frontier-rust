//! DOS Frontier PIC*.GRF stills: 768-byte 6-bit VGA palette + PackBits pixels.
//! Port of render/grf.py. Layout from watsonmw/fe2-intro: 320x168 view + 32-row cockpit.

use image::RgbaImage;
use std::path::Path;

pub const VIEW_W: u32 = 320;
pub const SCREEN_H: u32 = 200;

/// Apple / TIFF PackBits.
pub fn packbits_decode(data: &[u8], limit: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(limit);
    let mut i = 0;
    while i < data.len() && out.len() < limit {
        let ctrl = data[i];
        i += 1;
        if ctrl <= 127 {
            let cnt = ctrl as usize + 1;
            let end = (i + cnt).min(data.len());
            out.extend_from_slice(&data[i..end]);
            i += cnt;
        } else if ctrl == 128 {
            continue;
        } else {
            let cnt = 257 - ctrl as usize;
            if i >= data.len() {
                break;
            }
            out.extend(std::iter::repeat(data[i]).take(cnt));
            i += 1;
        }
    }
    out.truncate(limit);
    out
}

pub fn load_pic(path: &Path) -> Result<RgbaImage, String> {
    let raw = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
    if raw.len() < 768 + 16 {
        return Err(format!("GRF too small: {}", path.display()));
    }
    let pal: Vec<[u8; 3]> = (0..256)
        .map(|i| [raw[i * 3] * 4, raw[i * 3 + 1] * 4, raw[i * 3 + 2] * 4])
        .collect();
    let n = (VIEW_W * SCREEN_H) as usize;
    let mut pixels = packbits_decode(&raw[768..], n);
    pixels.resize(n, 0);
    let mut img = RgbaImage::new(VIEW_W, SCREEN_H);
    for (i, &idx) in pixels.iter().enumerate() {
        let c = pal[idx as usize];
        img.put_pixel(i as u32 % VIEW_W, i as u32 / VIEW_W, image::Rgba([c[0], c[1], c[2], 255]));
    }
    Ok(img)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packbits_literal_and_run() {
        // 2 literals, then a run of 3 x 0xAA
        let data = [1, 0x10, 0x20, 254, 0xAA];
        assert_eq!(packbits_decode(&data, 16), vec![0x10, 0x20, 0xAA, 0xAA, 0xAA]);
    }
}
