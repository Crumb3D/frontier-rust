//! Loads the reference art: the DOS console, F-key icons, portraits, font, GRF stills.

use crate::console::{unlight, CON_H, VCR_X};
use crate::font::Font;
use crate::grf;
use crate::palette::Rgb;
use frontier_sim::paths::{assets_dir, original_frontier_dir};
use image::{Rgba, RgbaImage};
use std::collections::HashMap;

pub struct Assets {
    pub font: Font,
    /// The DOS console strip, 640x64, with its text and lit button cleared.
    pub console: RgbaImage,
    /// Pixels of `console` inside the scanner well (black or scanner red).
    pub well_mask: Vec<bool>,
    pub icons: HashMap<String, RgbaImage>,
    /// Faces cropped from the original's comms screens, by file stem
    /// (`station_1`, `shipyard_3`, `msg_station`, ...).
    pub portraits: HashMap<String, RgbaImage>,
    /// The FRONTIER logo strip from the bottom of the DOS title screen.
    pub title_logo: Option<RgbaImage>,
    /// Frames of the intro, which runs behind the title menu.
    pub intro_frames: Vec<std::path::PathBuf>,
}

impl Assets {
    /// Every face of one kind, e.g. `portrait_pool("shipyard")`.
    pub fn portrait_pool(&self, kind: &str) -> Vec<&RgbaImage> {
        let mut names: Vec<&String> = self.portraits.keys().filter(|k| k.starts_with(&format!("{kind}_"))).collect();
        names.sort();
        names.into_iter().map(|k| &self.portraits[k]).collect()
    }
}

fn load_rgba(path: &std::path::Path) -> Result<RgbaImage, String> {
    image::open(path)
        .map(|i| i.to_rgba8())
        .map_err(|e| format!("{}: {e}", path.display()))
}

/// Cut the console out of PIC002 (rows 168-199) and strip the per-frame state:
/// orange lettering becomes the row's stripe grey, the lit x1 button is unlit.
fn console_from_still(still: &RgbaImage) -> RgbaImage {
    let mut src = RgbaImage::new(320, 32);
    for y in 0..32u32 {
        for x in 0..320u32 {
            let p = *still.get_pixel(x, 168 + y);
            let dy = 168 + y as i32;
            let xi = x as i32;
            let mut c: Rgb = [p[0], p[1], p[2]];
            if c == crate::palette::CON_ORANGE && (xi < 80 || xi >= 250) {
                c = if (dy - 169) % 2 == 0 { [64, 64, 64] } else { [96, 96, 96] };
            }
            let (x0, x1) = VCR_X[1];
            if (179..=185).contains(&dy) && (x0..=x1).contains(&xi) {
                c = unlight(c);
            }
            src.put_pixel(x, y, Rgba([c[0], c[1], c[2], 255]));
        }
    }
    image::imageops::resize(&src, 640, CON_H as u32, image::imageops::FilterType::Nearest)
}

/// A plain grey console when the DOS files are missing.
fn fallback_console() -> RgbaImage {
    let mut img = RgbaImage::from_pixel(640, CON_H as u32, Rgba([128, 128, 128, 255]));
    for y in 4..CON_H as u32 - 4 {
        for x in 130..510 {
            img.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
    }
    img
}

fn well_mask(console: &RgbaImage) -> Vec<bool> {
    let (w, h) = (console.width(), console.height());
    let mut mask = vec![false; (w * h) as usize];
    // Black or scanner-red pixels mark the well; everything between the first and
    // last on a row (the grey scanner lines too) is blanked behind text.
    for y in 2..h - 2 {
        let inside = |x: u32| {
            let p = console.get_pixel(x, y);
            p[1] == 0 && p[2] == 0
        };
        let (Some(x0), Some(x1)) = ((124..510).find(|&x| inside(x)), (124..510).rev().find(|&x| inside(x))) else {
            continue;
        };
        for x in x0..=x1 {
            mask[(y * w + x) as usize] = true;
        }
    }
    mask
}

impl Assets {
    pub fn load() -> Result<Self, String> {
        let dir = assets_dir();
        let font = Font::load(&dir.join("Frontier.ttf"))?;

        let mut icons = HashMap::new();
        for (sub, prefix) in [("icons", ""), ("console_icons", "console_icons/")] {
            let Ok(rd) = std::fs::read_dir(dir.join(sub)) else { continue };
            for entry in rd.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("png") {
                    if let (Some(name), Ok(img)) =
                        (path.file_name().and_then(|n| n.to_str()), load_rgba(&path))
                    {
                        icons.insert(format!("{prefix}{name}"), img);
                    }
                }
            }
        }

        let mut portraits = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(dir.join("Images").join("portraits")) {
            for path in rd.flatten().map(|e| e.path()) {
                if let (Some(stem), Ok(img)) = (path.file_stem().and_then(|s| s.to_str()), load_rgba(&path)) {
                    portraits.insert(stem.to_string(), img);
                }
            }
        }
        let title_logo = load_rgba(&dir.join("Images").join("title_logo.png")).ok();
        let mut intro_frames: Vec<_> = std::fs::read_dir(dir.join("Images").join("intro_frames"))
            .map(|rd| rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "png")).collect())
            .unwrap_or_default();
        intro_frames.sort();

        let mut stills = Vec::new();
        let fdir = original_frontier_dir();
        for name in ["PIC000.GRF", "PIC001.GRF", "PIC002.GRF"] {
            match grf::load_pic(&fdir.join(name)) {
                Ok(img) => stills.push(img),
                Err(e) => log::warn!("{e}"),
            }
        }

        let console = match stills.get(2) {
            Some(pic002) => console_from_still(pic002),
            None => {
                log::warn!("PIC002.GRF missing; using a plain console");
                fallback_console()
            }
        };
        let well_mask = well_mask(&console);

        Ok(Self { font, console, well_mask, icons, portraits, title_logo, intro_frames })
    }
}
