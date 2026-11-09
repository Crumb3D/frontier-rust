//! Full-view screens and the pieces the station pages are built from. Positions are
//! in DOS pixels (the original's 320x200), measured on DOSBox captures of the
//! original, and doubled when drawn.

use crate::assets::Assets;
use crate::fb::{Canvas, Rect};
use crate::font::{self, CONSOLE};
use crate::game::{VIEW_H, W};
use crate::meshes;
use crate::palette::{self, Rgb};
use crate::render3d::{self, Instance, View};
use frontier_sim::{FittedShip, Shipyard};
use glam::{DMat3, DVec3};
use image::RgbaImage;

/// Text with its capitals' top-left at DOS (x, y), with the one-pixel black shadow.
pub fn text(cv: &mut Canvas, a: &Assets, s: &str, x: i32, y: i32, col: Rgb) -> i32 {
    let f = &a.font;
    f.draw_shadow(cv, s, x * 2, y * 2 - f.cap_offset(CONSOLE), CONSOLE, col) / 2
}

pub fn text_right(cv: &mut Canvas, a: &Assets, s: &str, right: i32, y: i32, col: Rgb) {
    let w = a.font.width(s, CONSOLE);
    let f = &a.font;
    f.draw_shadow(cv, s, right * 2 - w, y * 2 - f.cap_offset(CONSOLE), CONSOLE, col);
}

pub fn text_width(a: &Assets, s: &str) -> i32 {
    a.font.width(s, CONSOLE) / 2
}

/// Word-wrap to `w` DOS pixels.
pub fn wrap(a: &Assets, s: &str, w: i32) -> Vec<String> {
    a.font.wrap(s, CONSOLE, w * 2)
}

fn dos_px(cv: &mut Canvas, x: i32, y: i32, c: Rgb) {
    cv.fill_rect(Rect::new(x * 2, y * 2, 2, 2), c);
}

/// The grey square button: an 8x8 face at DOS (x, y) with a two-pixel black shadow.
/// Returns its clickable area in screen pixels.
pub fn button(cv: &mut Canvas, x: i32, y: i32) -> Rect {
    let [hi, lit, face, dark] = palette::BTN;
    for dy in 0..10 {
        for dx in 0..10 {
            let c = match (dx, dy) {
                (0, 0) => Some(hi),
                (0..=7, 0) | (0, 0..=7) => Some(lit),
                (1..=7, 7) | (7, 1..=6) => Some(dark),
                (1..=6, 1..=6) => Some(face),
                (8, 1) => Some(palette::BLACK),
                (8..=9, 2..=9) | (1..=9, 8) | (2..=9, 9) => Some(palette::BLACK),
                _ => None,
            };
            if let Some(c) = c {
                dos_px(cv, x + dx, y + dy, c);
            }
        }
    }
    Rect::new(x * 2, y * 2, 20, 20)
}

/// The "More..." arrow, 10 DOS pixels wide, pointing down or up, at DOS (x, y).
pub fn arrow(cv: &mut Canvas, x: i32, y: i32, down: bool) -> Rect {
    const DOWN: [&str; 10] = [
        "..LLLLLL...",
        "..Lggggk...",
        "..Lggggk...",
        "..Lggggk...",
        "LLLggggkLL.",
        "kggggggggk.",
        ".kggggggkk.",
        "..kggggkk..",
        "...kggkk...",
        "....kkk....",
    ];
    for (r, row) in DOWN.iter().enumerate() {
        let ry = if down { r as i32 } else { 9 - r as i32 };
        for (c, ch) in row.chars().enumerate() {
            let col = match ch {
                'L' => palette::BTN[1],
                'g' => palette::ARROW_FACE,
                'k' => palette::BLACK,
                _ => continue,
            };
            dos_px(cv, x + c as i32, y + ry, col);
        }
    }
    Rect::new(x * 2, y * 2, 22, 20)
}

/// Lilac station page: header at the top left and the caller's face in a black box.
pub fn station_frame(cv: &mut Canvas, a: &Assets, header: &str, face: Option<&RgbaImage>) {
    let area = Rect::new(0, 0, W, VIEW_H);
    cv.set_clip(area);
    cv.fill_rect(area, palette::STATION_BG);
    text(cv, a, header, 4, 6, palette::CREAM);
    let bx = Rect::new(2, 44, 236, 230);
    cv.fill_rect(bx, palette::BLACK);
    if let Some(img) = face {
        cv.blit_scaled(img, bx, None);
    }
}

pub struct Hold {
    pub free: i32,
    pub used: i32,
    pub cabins_free: i32,
    pub cabins_used: i32,
    pub cash: i64,
}

/// "Remaining Used / Cargo Space / Cabins / Cash", lilac and cream on the station
/// pages (x0 = 0), all white on the status pages (x0 = 189).
pub fn hold_block(cv: &mut Canvas, a: &Assets, x0: i32, h: &Hold, label: Rgb, value: Rgb) {
    text(cv, a, "Remaining", x0 + 20, 138, label);
    text(cv, a, "Used", x0 + 59, 138, label);
    text(cv, a, "Cargo Space", x0 + 1, 148, label);
    text(cv, a, &h.free.to_string(), x0 + 44, 148, value);
    text(cv, a, &h.used.to_string(), x0 + 62, 148, value);
    text(cv, a, "Cash", x0 + 82, 148, label);
    text(cv, a, "Cabins", x0 + 1, 158, label);
    text(cv, a, &h.cabins_free.to_string(), x0 + 44, 158, value);
    text(cv, a, &h.cabins_used.to_string(), x0 + 62, 158, value);
    text_right(cv, a, &font::money(h.cash), x0 + 97, 158, value);
}

/// Title: the intro runs behind the start-position menu, as in the DOS original.
pub fn title(cv: &mut Canvas, a: &Assets, frame: Option<&RgbaImage>, t: f64) -> Vec<(Rect, usize)> {
    cv.reset_clip();
    cv.clear(palette::BLACK);
    let sky = Rect::new(0, 0, W, 336);
    match frame {
        Some(img) => {
            // The frames are letterboxed 522x360; rows 21-292 hold the picture.
            let crop = image::imageops::crop_imm(img, 0, 21, img.width(), 272).to_image();
            cv.blit_scaled(&crop, sky, None);
        }
        None => {
            cv.fill_rect(sky, palette::TITLE_NAVY);
            for (d, b) in render3d::starfield(0x7171e, 160) {
                let x = ((d.x * 0.5 + 0.5) * W as f64 + t * 4.0) as i32 % W;
                let y = ((d.y * 0.5 + 0.5) * 336.0) as i32;
                cv.put(x, y, [b, b, b]);
            }
        }
    }
    if let Some(logo) = &a.title_logo {
        cv.blit_scaled(logo, Rect::new(0, 336, W, 64), None);
    }
    let white = palette::WHITE;
    text(cv, a, "Select Start Position:", 40, 20, white);
    let items = [
        "1. Ross 154 System (Recommended Start Position)",
        "2. Mars in the Sol System",
        "3. Lave System",
        "4. Last Saved Position",
        "5. Select Position from disc",
    ];
    let mut hits = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let y = 40 + 20 * i as i32;
        for k in 0..8 {
            for (x, yy) in [(90 + k, y), (90 + k, y + 7), (90, y + k), (97, y + k)] {
                dos_px(cv, x, yy, white);
            }
        }
        for k in 1..7 {
            for (x, yy) in [(91 + k, y + 1), (91, y + k), (91 + k, y + 8), (98, y + 1 + k)] {
                dos_px(cv, x, yy, palette::BLACK);
            }
        }
        let w = text(cv, a, item, 101, y, white);
        hits.push((Rect::new(180, y * 2 - 4, (w + 12) * 2, 22), i));
    }
    let copy = "Copyright © D.J.Braben 1993, All rights reserved.
                Rust Port by Crumb3D 2026";
    text(cv, a, copy, 160 - text_width(a, copy) / 2, 156, white);
    hits
}

/// In flight, F4 lists the traffic control channels in range over the view.
pub fn flight_comms(cv: &mut Canvas, a: &Assets, options: &[String]) -> Vec<(Rect, usize)> {
    let f = &a.font;
    let lh = 20;
    let n = options.len().max(1) as i32;
    let panel = Rect::new(40, 20, W - 80, 16 + n * lh);
    cv.fill_rect(panel, palette::BLACK);
    cv.rect(panel, palette::GREY_102);
    let mut hits = Vec::new();
    if options.is_empty() {
        f.draw(cv, "No traffic control in range.", panel.x + 10, panel.y + 8, CONSOLE, palette::CON_GREEN);
    }
    for (i, o) in options.iter().enumerate() {
        let y = panel.y + 8 + i as i32 * lh;
        f.draw(cv, &format!("{}. {o}", i + 1), panel.x + 10, y, CONSOLE, palette::CON_GREEN);
        hits.push((Rect::new(panel.x, y - 2, panel.w, lh), i));
    }
    hits
}

/// The ship turning in the black box at the right of the F3 pages, in that
/// page's colours (hull, sides, belly).
fn ship_box(cv: &mut Canvas, t: f64, colours: [Rgb; 3]) {
    ship_in(cv, Rect::new(378, 4, 260, 276), t, colours);
}

/// The same spinning ship, drawn into `bay` (framebuffer pixels). The station's
/// View page uses the portrait box; the F3 pages use the box on the right.
pub fn ship_in(cv: &mut Canvas, bay: Rect, t: f64, colours: [Rgb; 3]) {
    let area = Rect::new(0, 0, W, VIEW_H);
    cv.fill_rect(bay, palette::BLACK);
    let view = View::new(bay);
    cv.set_clip(bay);
    let eagle = meshes::eagle_in(colours);
    // Tilted so the spin shows the top, the side and the three-quarter view,
    // which is how the original's ship box turns.
    let orient = DMat3::from_rotation_y(t * 0.55) * DMat3::from_rotation_x(-1.05);
    let inst = Instance { mesh: &eagle, pos: DVec3::new(0.0, -0.05, 1.85), orient };
    render3d::draw_meshes(cv, &view, &DMat3::IDENTITY, &[inst], DVec3::new(-20.0, 30.0, -10.0), &[]);
    cv.set_clip(area);
}

/// F3 first page: SHIP EQUIPMENT STATUS.
pub fn equipment_status(cv: &mut Canvas, a: &Assets, fitted: &FittedShip, yard: &Shipyard, hold: &Hold, t: f64) {
    let area = Rect::new(0, 0, W, VIEW_H);
    cv.set_clip(area);
    cv.fill_rect(area, palette::STATUS_BG);
    let w = palette::WHITE;
    let stats = yard.stats(fitted);
    let name_of = |id: &str| yard.equipment.get(id).map(|e| e.name.clone()).unwrap_or_else(|| id.to_string());
    text(cv, a, &format!("SHIP EQUIPMENT STATUS - {}", stats.name), 8, 5, w);

    let drive = fitted
        .equipment
        .iter()
        .find(|e| yard.equipment.get(*e).is_some_and(|d| d.slot == "drive"))
        .map(|e| name_of(e));
    let rated = yard.ships.get(&fitted.ship_id).map_or(stats.jump_range_ly, |s| s.jump_ly_base);
    text(cv, a, "Drive System:", 5, 15, w);
    match drive {
        Some(d) => text(cv, a, &format!("{d} (max.{rated:.2} ly)"), 68, 15, w),
        None => text(cv, a, "None", 68, 15, w),
    };
    text(cv, a, "Hull Condition:", 5, 25, w);
    let hull = 100.0 * fitted.hull / fitted.max_hull.max(1.0);
    text(cv, a, &format!("{hull:.1}%"), 68, 25, w);
    let current = if fitted.hydrogen_t() > 0 { stats.jump_range_ly.min(rated) } else { 0.0 };
    text(cv, a, &format!("(current {current:.2} ly)"), 130, 25, w);
    let shields = fitted.equipment.iter().filter(|e| *e == "shield_gen").count();
    text(cv, a, "Shield Generators:", 5, 35, w);
    text(cv, a, &format!("{shields} unit(s)"), 68, 35, w);

    let mut y = 47;
    let mut seen = Vec::new();
    for id in &fitted.equipment {
        let Some(e) = yard.equipment.get(id) else { continue };
        if matches!(e.slot.as_str(), "drive" | "gun" | "missile") || seen.contains(id) {
            continue;
        }
        seen.push(id.clone());
        let n = fitted.equipment.iter().filter(|x| *x == id).count();
        let label = if n > 1 { format!("{} x{n}", e.name) } else { e.name.clone() };
        text(cv, a, &label, 5, y, w);
        y += 9;
    }
    if let Some(gun) = fitted.guns.get("front") {
        text(cv, a, "Front Gun Mounting:", 5, 125, w);
        text(cv, a, &name_of(gun), 90, 125, w);
    }
    ship_box(cv, t, [[93, 158, 158], [77, 142, 142], palette::STATUS_BG]);
    hold_block(cv, a, 189, hold, w, w);
    cv.reset_clip();
}

/// F3 second page: the commander's profile, on grey.
pub fn profile(cv: &mut Canvas, a: &Assets, t: f64, rank: &str, legal: bool) {
    let area = Rect::new(0, 0, W, VIEW_H);
    cv.set_clip(area);
    cv.fill_rect(area, palette::PROFILE_BG);
    let w = palette::WHITE;
    text(cv, a, "PROFILE FOR COMMANDER JAMESON", 30, 5, w);
    text(cv, a, "MILITARY STANDING", 2, 20, w);
    text(cv, a, "Federal Rank:", 2, 30, w);
    text(cv, a, "None", 52, 30, w);
    text(cv, a, "Imperial Title:", 2, 40, w);
    text(cv, a, "Outsider", 52, 40, w);
    text(cv, a, "Elite Rating:", 2, 50, w);
    text(cv, a, rank, 52, 50, w);
    text(cv, a, "LEGAL STATUS", 114, 20, w);
    let standing = if legal { "Clean" } else { "Offender" };
    for (i, who) in ["Federal:", "Imperial:", "Interpol:"].iter().enumerate() {
        let y = 30 + 10 * i as i32;
        text(cv, a, who, 114, y, w);
        text(cv, a, standing, 150, y, w);
    }
    text(cv, a, "FEDERAL CRIMINAL RECORD", 2, 65, w);
    text(cv, a, "MEDALS AND PERMITS", 2, 123, w);
    ship_box(cv, t, [[158, 158, 158], [125, 125, 125], palette::PROFILE_BG]);
    cv.reset_clip();
}

pub struct RosterRow {
    pub client: String,
    pub group: String,
    pub dest: String,
    pub pay: i64,
}

/// F3 third page: PASSENGER AND SMALL ITEMS ROSTER, on brown.
pub fn roster(cv: &mut Canvas, a: &Assets, rows: &[RosterRow], hold: &Hold, t: f64) {
    let area = Rect::new(0, 0, W, VIEW_H);
    cv.set_clip(area);
    cv.fill_rect(area, palette::ROSTER_BG);
    let w = palette::WHITE;
    let red = palette::ROSTER_RED;
    text(cv, a, "PASSENGER AND SMALL ITEMS ROSTER", 30, 5, w);
    text(cv, a, "Client", 6, 25, red);
    text(cv, a, "Group", 72, 25, red);
    text(cv, a, "Destination", 100, 25, red);
    text(cv, a, "To pay", 163, 25, red);
    for (i, r) in rows.iter().take(10).enumerate() {
        let y = 35 + 10 * i as i32;
        text(cv, a, &r.client, 2, y, w);
        text(cv, a, &r.group, 72, y, w);
        text(cv, a, &r.dest, 100, y, w);
        text(cv, a, &font::credits(r.pay), 163, y, w);
    }
    ship_box(cv, t, [[77, 142, 77], [60, 125, 60], palette::ROSTER_BG]);
    hold_block(cv, a, 189, hold, w, w);
    cv.reset_clip();
}
