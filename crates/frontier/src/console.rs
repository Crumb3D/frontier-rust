//! The bottom console, built from the DOS original's own console (rows 168-199 of
//! PIC002.GRF, doubled to 640x64). Everything that changes with game state is
//! painted over it at the cells measured on that still.

use crate::assets::Assets;
use crate::fb::{Canvas, Rect};
use crate::font::CONSOLE;
use crate::palette::{self, Rgb};
use glam::DVec3;

/// Console height on the 640x400 logical screen (32 DOS rows).
pub const CON_H: i32 = 64;

/// Stardreamer buttons in DOS pixels (x0, x1), rows 179-185:
/// pause, x1, x10, x100, x1000, x10000.
pub const VCR_X: [(i32, i32); 6] = [(1, 9), (10, 21), (22, 29), (31, 39), (41, 49), (51, 59)];
const VCR_Y: (i32, i32) = (179, 185);
/// Scanner ellipse in DOS pixels.
const SCAN_C: (i32, i32) = (160, 184);
const SCAN_R: (i32, i32) = (83, 12);
pub const SCAN_RANGE_KM: f64 = 50.0;

/// DOS pixel to logical screen, relative to the console top.
fn d2s(x: i32, y: i32, top: i32) -> (i32, i32) {
    (x * 2, top + (y - 168) * 2)
}

pub fn vcr_rect(i: usize, top: i32) -> Rect {
    let (x0, x1) = VCR_X[i];
    let (sx, sy) = d2s(x0, VCR_Y.0, top);
    Rect::new(sx, sy, (x1 - x0 + 1) * 2, (VCR_Y.1 - VCR_Y.0 + 1) * 2)
}

/// F1-F4 (left, `i` 0..4) and F7-F10 (right, `i` 4..8) icon cells.
pub fn key_rect(i: usize, top: i32) -> Rect {
    let x0 = if i < 4 { 1 + 16 * i as i32 } else { 257 + 16 * (i as i32 - 4) };
    let (sx, sy) = d2s(x0, 188, top);
    Rect::new(sx, sy, 15 * 2, 11 * 2)
}

/// Unlit Stardreamer face from lit: the DOS palette swaps 224->128 and 128->96.
pub fn unlight(c: Rgb) -> Rgb {
    match c {
        [128, 128, 128] => [96, 96, 96],
        [224, 224, 224] => [128, 128, 128],
        _ => c,
    }
}

/// What the console should show this frame.
pub struct ConsoleState<'a> {
    pub stardate: String,
    /// 0 pause, 1 = x1 ... 5 = x10000.
    pub vcr_active: usize,
    pub left: [&'a str; 4],
    pub right: [Option<&'a str>; 4],
    pub upper: &'a str,
    pub lower: &'a str,
    /// Paragraphs for the scanner well, each in its own colour.
    pub message: &'a [(String, Rgb)],
    /// Caller's face beside the text, as for "Message from Sirocco Station:".
    pub face: Option<&'a image::RgbaImage>,
    pub scanner: Option<Vec<ScanContact>>,
}

/// Text in the scanner well: left-aligned at DOS x 89 (116 beside a face), cap
/// tops on DOS rows 170/180/190, or 180 for a single line.
fn draw_message(cv: &mut Canvas, assets: &Assets, top: i32, paras: &[(String, Rgb)], face: Option<&image::RgbaImage>) {
    let font = &assets.font;
    let (x_dos, right_dos) = if face.is_some() { (116, 237) } else { (89, 237) };
    if let Some(img) = face {
        let (fx, fy) = d2s(88, 169, top);
        cv.blit_scaled(img, Rect::new(fx, fy, img.width() as i32 * 2, img.height() as i32 * 2), None);
    }
    let mut lines: Vec<(String, Rgb)> = Vec::new();
    for (text, col) in paras {
        for l in font.wrap(text, CONSOLE, (right_dos - x_dos) * 2) {
            lines.push((l, *col));
        }
    }
    let first = if lines.len() == 1 { 180 } else { 170 };
    for (i, (l, col)) in lines.iter().take(3).enumerate() {
        let (x, y) = d2s(x_dos, first + 10 * i as i32, top);
        font.draw(cv, l, x, y - font.cap_offset(CONSOLE), CONSOLE, *col);
    }
}

pub struct ScanContact {
    /// Ship-relative: x right, y up, z forward (km).
    pub rel: DVec3,
    pub color: Rgb,
}

const VCR_ICONS: [&str; 6] = [
    "pause.png",
    "StarDreamer Normal Speed.png",
    "StarDreamer speed 2.png",
    "StarDreamer speed 3.png",
    "StarDreamer speed 4.png",
    "StarDreamer speed 5.png",
];

/// Selected Stardreamer key: the grey of the glyph becomes white.
fn vcr_lit(c: Rgb) -> Rgb {
    let (r, g, b) = (c[0] as i16, c[1] as i16, c[2] as i16);
    if (r - g).abs() < 28 && (g - b).abs() < 28 && (50..210).contains(&c[0]) {
        [255, 255, 255]
    } else {
        c
    }
}

pub fn draw(cv: &mut Canvas, assets: &Assets, st: &ConsoleState) {
    let top = cv.h - CON_H;
    cv.blit(&assets.console, 0, top);

    let active = st.vcr_active.min(5);
    for (i, name) in VCR_ICONS.iter().enumerate() {
        let Some(img) = assets.icons.get(*name) else { continue };
        let recolor: Option<&dyn Fn(Rgb) -> Rgb> = if i == active { Some(&vcr_lit) } else { None };
        cv.blit_scaled(img, vcr_rect(i, top), recolor);
    }

    for i in 0..4 {
        if let Some(img) = assets.icons.get(st.left[i]) {
            cv.blit_scaled(img, key_rect(i, top), None);
        }
        if let Some(img) = st.right[i].and_then(|n| assets.icons.get(n)) {
            cv.blit_scaled(img, key_rect(4 + i, top), None);
        }
    }

    let font = &assets.font;
    font.draw(cv, &st.stardate, 2, top + 2, CONSOLE, palette::CON_ORANGE);
    font.draw(cv, st.upper, 532, top + 2, CONSOLE, palette::CON_ORANGE);
    font.draw(cv, st.lower, 532, top + 22, CONSOLE, palette::CON_ORANGE);

    if !st.message.is_empty() || st.scanner.is_none() {
        for (i, &m) in assets.well_mask.iter().enumerate() {
            if m {
                cv.put(i as i32 % cv.w, top + i as i32 / cv.w, palette::BLACK);
            }
        }
        draw_message(cv, assets, top, st.message, st.face);
    } else if let Some(contacts) = &st.scanner {
        draw_contacts(cv, top, contacts);
    }
}

/// Contacts on the DOS scanner dish: a dot on the plane and a stalk up or down.
fn draw_contacts(cv: &mut Canvas, top: i32, contacts: &[ScanContact]) {
    let (cx, cy) = d2s(SCAN_C.0, SCAN_C.1, top);
    let (rx, ry) = (SCAN_R.0 as f64 * 2.0, SCAN_R.1 as f64 * 2.0);
    for c in contacts {
        let d = c.rel.length();
        if d > SCAN_RANGE_KM || d < 1e-4 {
            continue;
        }
        // Compress range so near and far contacts both read on the small dish.
        let k = (d / SCAN_RANGE_KM).sqrt() / d;
        let p = c.rel * k;
        let sx = cx + (p.x * rx) as i32;
        let sy = cy - (p.z * ry) as i32;
        let stalk = ((p.y * ry * 1.6) as i32).clamp(-22, 22);
        cv.vline(sx, sy, sy - stalk, c.color);
        cv.fill_rect(Rect::new(sx - 1, sy - stalk - 1, 3, 2), c.color);
    }
}
