//! The Frontier typeface (assets/Frontier.ttf), rendered crisp like the Amiga.

use crate::fb::Canvas;
use crate::palette::{Rgb, BLACK};
use fontdue::{Font as FdFont, FontSettings, Metrics};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;

pub const SMALL: f32 = 11.0;
/// All the original's lettering: capitals 7 DOS rows tall at 2x, which puts
/// "Which service do you require?" at the original's 97 DOS pixels wide.
pub const CONSOLE: f32 = 20.0;

struct Glyph {
    m: Metrics,
    bmp: Vec<u8>,
}

/// The original's credit sign, which Frontier.ttf lacks: a C with a stroke through
/// it, 4 DOS pixels wide, from the cap line to one row below the baseline.
const CENT: [&str; 9] = ["..#.", "####", "#.#.", "#.#.", "#.#.", "#.#.", "#.#.", "####", "..#."];

pub struct Font {
    font: FdFont,
    cache: RefCell<HashMap<(char, u32), Glyph>>,
}

/// Money as the original shows it, in tenths of a credit: `¢100.0`, `¢-4.3`.
pub fn money(tenths: i64) -> String {
    let sign = if tenths < 0 { "-" } else { "" };
    let t = tenths.abs();
    format!("¢{sign}{}.{}", t / 10, t % 10)
}

/// Whole credits, as the bulletin board and shipyard print them: `¢1200`.
pub fn credits(c: i64) -> String {
    format!("¢{c}")
}

impl Font {
    pub fn load(path: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let font = FdFont::from_bytes(bytes, FontSettings::default())?;
        Ok(Self { font, cache: RefCell::new(HashMap::new()) })
    }

    fn with_glyph<R>(&self, c: char, size: f32, f: impl FnOnce(&Glyph) -> R) -> R {
        let key = (c, size.to_bits());
        let mut cache = self.cache.borrow_mut();
        let g = cache.entry(key).or_insert_with(|| {
            if c == '¢' && self.font.lookup_glyph_index('¢') == 0 {
                return self.cent_glyph(size);
            }
            let (m, bmp) = self.font.rasterize(c, size);
            Glyph { m, bmp }
        });
        f(g)
    }

    /// Build the credit sign at the size of the font's digits.
    fn cent_glyph(&self, size: f32) -> Glyph {
        let (zm, _) = self.font.rasterize('0', size);
        let px = ((zm.height as f32 / 8.0).round() as usize).max(1);
        let (w, h) = (4 * px, CENT.len() * px);
        let mut bmp = vec![0u8; w * h];
        for (r, row) in CENT.iter().enumerate() {
            for (c, ch) in row.chars().enumerate() {
                if ch == '#' {
                    for dy in 0..px {
                        for dx in 0..px {
                            bmp[(r * px + dy) * w + c * px + dx] = 255;
                        }
                    }
                }
            }
        }
        let m = Metrics {
            xmin: zm.xmin,
            ymin: zm.ymin - px as i32,
            width: w,
            height: h,
            advance_width: (5 * px) as f32,
            ..zm
        };
        Glyph { m, bmp }
    }

    pub fn ascent(&self, size: f32) -> i32 {
        self.font
            .horizontal_line_metrics(size)
            .map(|m| m.ascent.round() as i32)
            .unwrap_or(size as i32)
    }

    /// From the `y` passed to [`Font::draw`] down to the top of the capitals.
    pub fn cap_offset(&self, size: f32) -> i32 {
        let h = self.with_glyph('0', size, |g| g.m.height as i32 + g.m.ymin);
        self.ascent(size) - h
    }

    pub fn width(&self, text: &str, size: f32) -> i32 {
        text.chars()
            .map(|c| self.with_glyph(c, size, |g| g.m.advance_width.round() as i32))
            .sum()
    }

    /// Draw text with its top-left at (x, y). Returns the advance width.
    pub fn draw(&self, cv: &mut Canvas, text: &str, x: i32, y: i32, size: f32, color: Rgb) -> i32 {
        let base = y + self.ascent(size);
        let mut pen = x;
        for c in text.chars() {
            pen += self.with_glyph(c, size, |g| {
                let gx = pen + g.m.xmin;
                let gy = base - g.m.height as i32 - g.m.ymin;
                for row in 0..g.m.height {
                    for col in 0..g.m.width {
                        // Hard threshold: the original has no anti-aliasing.
                        if g.bmp[row * g.m.width + col] >= 110 {
                            cv.put(gx + col as i32, gy + row as i32, color);
                        }
                    }
                }
                g.m.advance_width.round() as i32
            });
        }
        pen - x
    }

    /// Station-screen text: colour with a black drop shadow one DOS pixel down and right.
    pub fn draw_shadow(&self, cv: &mut Canvas, text: &str, x: i32, y: i32, size: f32, color: Rgb) -> i32 {
        self.draw(cv, text, x + 2, y + 2, size, BLACK);
        self.draw(cv, text, x, y, size, color)
    }

    pub fn draw_centered(&self, cv: &mut Canvas, text: &str, cx: i32, y: i32, size: f32, color: Rgb) {
        let w = self.width(text, size);
        self.draw(cv, text, cx - w / 2, y, size, color);
    }
    /// Word-wrap to `max_w` pixels.
    pub fn wrap(&self, text: &str, size: f32, max_w: i32) -> Vec<String> {
        let mut lines = Vec::new();
        let mut cur = String::new();
        for word in text.split_whitespace() {
            let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if self.width(&trial, size) <= max_w || cur.is_empty() {
                cur = trial;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        lines
    }
}
