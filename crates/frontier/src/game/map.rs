//! Galactic Map and Current System Map.
//!
//! Manual, Section Two: arrow keys put a system in the centre and it turns
//! green (that is the hyperspace destination). The system you are in is pink.
//! Sector lines are every 8 ly, coordinates relative to Sol, east and north
//! positive. Right-drag tilts the grid. A pink circle around the current
//! system is the drive's range. "Out of Range" sits by the system name, and
//! the fuel that would be used is at the bottom left, with "Insufficient Fuel"
//! when the hold is short.

use super::{Game, MapPage, W, VIEW_H};
use crate::fb::{Canvas, Rect};
use crate::font::CONSOLE;
use crate::palette::{self, Rgb};
use frontier_sim::kepler::{position_at, AU_KM};
use glam::DVec3;

const PINK: Rgb = [236, 108, 172];
const GRID: Rgb = [0, 72, 0];
const SECTOR: Rgb = [0, 176, 0];
const STALK: Rgb = [168, 168, 168];
const ORBIT: Rgb = [72, 120, 72];

impl Game {
    pub(super) fn draw_map(&mut self, cv: &mut Canvas) {
        cv.set_clip(Rect::new(0, 0, W, VIEW_H));
        cv.fill_rect(Rect::new(0, 0, W, VIEW_H), palette::BLACK);
        match self.map_page {
            MapPage::Galaxy => self.draw_galaxy(cv),
            MapPage::System => self.draw_orrery(cv),
            MapPage::Data => self.draw_data(cv),
        }
        cv.reset_clip();
    }

    fn draw_galaxy(&mut self, cv: &mut Canvas) {
        let cam = self.cam_at;
        let here = self.galaxy.get(&self.sys_id).position_ly;
        let (yaw, pitch, scale) = (self.map_yaw, self.map_pitch, self.map_zoom);
        let origin = |p: DVec3| project(p - cam, yaw, pitch, scale);
        let (cx, cy) = (W as f64 / 2.0, VIEW_H as f64 / 2.0);

        if self.map_grid {
            let span = ((W as f64 / scale) * 0.85).ceil() as i32 + 2;
            let span = span.clamp(6, 48);
            let x0 = cam.x.floor() as i32;
            let z0 = cam.z.floor() as i32;
            for pass in 0..2 {
                for gx in (x0 - span)..=(x0 + span) {
                    let sector = gx.rem_euclid(8) == 0;
                    if sector != (pass == 1) {
                        continue;
                    }
                    let a = origin(DVec3::new(gx as f64, 0.0, (z0 - span) as f64));
                    let b = origin(DVec3::new(gx as f64, 0.0, (z0 + span) as f64));
                    cv.line(px(cx, a.0), px(cy, a.1), px(cx, b.0), px(cy, b.1), if sector { SECTOR } else { GRID });
                }
                for gz in (z0 - span)..=(z0 + span) {
                    let sector = gz.rem_euclid(8) == 0;
                    if sector != (pass == 1) {
                        continue;
                    }
                    let a = origin(DVec3::new((x0 - span) as f64, 0.0, gz as f64));
                    let b = origin(DVec3::new((x0 + span) as f64, 0.0, gz as f64));
                    cv.line(px(cx, a.0), px(cy, a.1), px(cx, b.0), px(cy, b.1), if sector { SECTOR } else { GRID });
                }
            }
        }

        let stats = self.flight_stats();
        if stats.jump_range_ly > 0.0 {
            let n = 96;
            let mut prev = None;
            for i in 0..=n {
                let a = i as f64 / n as f64 * std::f64::consts::TAU;
                let p = DVec3::new(
                    here.x + stats.jump_range_ly * a.cos(),
                    0.0,
                    here.z + stats.jump_range_ly * a.sin(),
                );
                let q = origin(p);
                let pt = (px(cx, q.0), px(cy, q.1));
                if let Some((x0, y0)) = prev {
                    cv.line(x0, y0, pt.0, pt.1, PINK);
                }
                prev = Some(pt);
            }
        }

        let mut stars: Vec<(String, DVec3, Rgb, String)> = self
            .galaxy
            .systems
            .iter()
            .map(|(id, sys)| (id.clone(), sys.position_ly, sys.star_color(), sys.name.clone()))
            .collect();
        stars.sort_by(|a, b| {
            let da = (a.1 - cam).length();
            let db = (b.1 - cam).length();
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut labelled: Vec<(i32, i32, String, Rgb)> = Vec::new();
        let mut star_hits: Vec<(Rect, String)> = Vec::new();
        let mut target_at: Option<(i32, i32)> = None;
        for (id, pos, color, name) in &stars {
            let tip = origin(*pos);
            let foot = origin(DVec3::new(pos.x, 0.0, pos.z));
            let (tx, ty) = (px(cx, tip.0), px(cy, tip.1));
            let (fx, fy) = (px(cx, foot.0), px(cy, foot.1));
            if self.map_stalks {
                cv.line(fx, fy, tx, ty, STALK);
            }
            let selected = id == &self.dest;
            let current = id == &self.sys_id;
            let c = if selected {
                palette::CON_GREEN
            } else if current {
                PINK
            } else {
                *color
            };
            disc(cv, tx, ty, if selected || current { 3 } else { 2 }, c);
            if selected {
                lock_box(cv, tx, ty, 12, palette::HUD_WHITE);
                target_at = Some((tx, ty));
            }
            if tx >= -20 && ty >= -20 && tx < W + 20 && ty < VIEW_H + 20 {
                star_hits.push((Rect::new(tx - 24, ty - 24, 72, 48), id.clone()));
            }
            let near = tx > 4 && tx < W - 4 && ty > 4 && ty < VIEW_H - 4;
            if !selected && near && (current || self.map_names) {
                let col = if current { PINK } else { palette::CON_GREEN };
                labelled.push((tx + 6, ty - 16, name.clone(), col));
            }
        }

        for (r, id) in star_hits.into_iter().rev() {
            self.hits.push((r, super::Action::Map(id)));
        }

        let font = &self.a.font;
        let name = self.galaxy.get(&self.dest).name.clone();
        let nw = font.width(&name, CONSOLE);
        let (nx, ny) = match target_at {
            Some((x, y)) => (x + 16, y - 18),
            None => (W / 2 - nw / 2, VIEW_H / 2 - 36),
        };
        let mut taken = vec![Rect::new(nx - 2, ny - 2, nw + 4, 18)];
        font.draw(cv, &name, nx, ny, CONSOLE, palette::CON_GREEN);
        if self.dest != self.sys_id {
            let dist = self.galaxy.distance_ly(&self.sys_id, &self.dest);
            if dist > stats.jump_range_ly + 1e-6 {
                let ow = font.width("Out of Range", CONSOLE);
                font.draw(cv, "Out of Range", nx + nw + 10, ny, CONSOLE, palette::CON_GREEN);
                taken.push(Rect::new(nx + nw + 8, ny - 2, ow + 4, 18));
            }
            let fuel = frontier_sim::frontier::jump_fuel_t(dist, stats.mass_t);
            font.draw(cv, &format!("{fuel}t"), 8, VIEW_H - 22, CONSOLE, palette::CON_GREEN);
            if !self.infinite_fuel && self.fitted.hydrogen_t() < fuel {
                font.draw(cv, "Insufficient Fuel", 8, VIEW_H - 44, CONSOLE, palette::CON_GREEN);
            }
        }
        for (x, y, text, col) in labelled {
            let w = font.width(&text, CONSOLE);
            let r = Rect::new(x, y, w, 16);
            if taken.iter().any(|o| overlaps(*o, r)) {
                continue;
            }
            font.draw(cv, &text, x, y, CONSOLE, col);
            taken.push(r);
        }

        if self.arm_target {
            lock_box(cv, self.mouse.0, self.mouse.1, 8, palette::HUD_WHITE);
        }

        let sx = (cam.x / 8.0).floor() as i32;
        let sz = (cam.z / 8.0).floor() as i32;
        let sector = format!("{sx},{sz}");
        let sw = font.width(&sector, CONSOLE);
        font.draw(cv, &sector, W - sw - 8, VIEW_H - 22, CONSOLE, palette::CON_GREEN);
    }

    fn draw_orrery(&mut self, cv: &mut Canvas) {
        let days = self.stardate.days;
        let focus = self.body_world(&self.focus, days);
        let max_km = self.system_span_km();
        let scale = self.sys_zoom * 150.0 / max_km;
        let (yaw, pitch) = (self.sys_yaw, self.sys_pitch);
        let (cx, cy) = (W as f64 / 2.0, VIEW_H as f64 / 2.0);
        let at = |p: DVec3| {
            let q = project(p - focus, yaw, pitch, scale);
            (px(cx, q.0), px(cy, q.1))
        };

        if self.sys_orbits {
            let elems: Vec<_> = self
                .system()
                .planets
                .iter()
                .filter(|p| p.parent_id.is_none())
                .map(|p| p.elements.clone())
                .collect();
            let moons: Vec<(String, f64)> = self
                .system()
                .planets
                .iter()
                .filter_map(|p| p.parent_id.clone().map(|id| (id, p.moon_orbit_km)))
                .filter(|(_, r)| *r > 0.0)
                .collect();
            let stations: Vec<(String, f64)> = self
                .system()
                .stations
                .iter()
                .map(|st| (st.parent_planet_id.clone(), st.orbit_radius_km))
                .collect();
            let star = self.frames.world_position("star", days);
            for e in &elems {
                let period = e.period().max(1.0);
                let mut prev = None;
                for i in 0..=64 {
                    let pt = at(star + position_at(e, period * i as f64 / 64.0));
                    if let Some((x0, y0)) = prev {
                        cv.line(x0, y0, pt.0, pt.1, ORBIT);
                    }
                    prev = Some(pt);
                }
            }
            for (parent, radius) in moons.into_iter().chain(stations) {
                circle(cv, self.frames.world_position(&parent, days), radius, &at);
            }
        }

        let star = at(self.frames.world_position("star", days));
        let star_color = self.system().star_color();
        let star_name = self.system().name.clone();
        disc(cv, star.0, star.1, 4, star_color);
        self.hits.push((Rect::new(star.0 - 8, star.1 - 8, 16, 16), super::Action::Body("star".into())));
        if self.sys_names {
            self.a.font.draw(cv, &star_name, star.0 + 6, star.1 - 16, CONSOLE, palette::CREAM);
        }

        let planets: Vec<(String, String, Rgb)> = self
            .system()
            .planets
            .iter()
            .map(|p| (p.id.clone(), p.name.clone(), p.color))
            .collect();
        let mut spots: Vec<(i32, i32)> = vec![(star.0, star.1)];
        let mut taken = Vec::new();
        if self.sys_names {
            let w = self.a.font.width(&star_name, CONSOLE);
            taken.push(Rect::new(star.0 + 6, star.1 - 16, w, 16));
        }
        for (id, name, color) in planets {
            let pt = at(self.frames.world_position(&id, days));
            disc(cv, pt.0, pt.1, 3, color);
            self.mark_body(cv, &id, &name, pt, palette::CREAM, &mut spots, &mut taken);
        }
        let stations: Vec<(String, String, String)> = self
            .system()
            .stations
            .iter()
            .map(|s| (s.id.clone(), s.name.clone(), s.parent_planet_id.clone()))
            .collect();
        let ports: Vec<(String, String, String)> = self
            .system()
            .planets
            .iter()
            .flat_map(|p| p.starports.iter().map(|sp| (sp.id.clone(), sp.name.clone(), p.id.clone())))
            .collect();
        for (id, name, parent) in stations.into_iter().chain(ports) {
            let mut pt = at(self.frames.world_position(&id, days));
            let parent_pt = at(self.frames.world_position(&parent, days));
            // A surface starport sits on its planet. Nudge the mark so it can be clicked.
            if (pt.0 - parent_pt.0).abs() < 8 && (pt.1 - parent_pt.1).abs() < 8 {
                pt.0 += 12;
                pt.1 -= 10;
            }
            disc(cv, pt.0, pt.1, 2, palette::HUD_WHITE);
            self.mark_body(cv, &id, &name, pt, palette::HUD_WHITE, &mut spots, &mut taken);
        }
    }

    /// Click target for a chart body. A station or starport becomes the autopilot lock.
    fn mark_body(
        &mut self,
        cv: &mut Canvas,
        id: &str,
        name: &str,
        pt: (i32, i32),
        color: Rgb,
        spots: &mut Vec<(i32, i32)>,
        taken: &mut Vec<Rect>,
    ) {
        let label_w = if self.sys_names { self.a.font.width(name, CONSOLE) } else { 0 };
        self.hits.push((
            Rect::new(pt.0 - 8, pt.1 - 16, 20 + label_w, 30),
            super::Action::Body(id.to_string()),
        ));
        if self.ap_body.as_deref() == Some(id) {
            lock_box(cv, pt.0, pt.1, 8, palette::HUD_WHITE);
        }
        let clear = spots.iter().all(|(x, y)| (x - pt.0).abs() > 10 || (y - pt.1).abs() > 10);
        spots.push((pt.0, pt.1));
        if self.sys_names && clear {
            self.put_label(cv, name, pt.0 + 6, pt.1 - 14, color, taken);
        }
    }

    /// F6. The selected system's diagram, then economy or political pages.
    fn draw_data(&mut self, cv: &mut Canvas) {
        let sys_id = self.dest.clone();
        let sys = self.galaxy.get(&sys_id);
        let mut marks: Vec<(String, String, Rgb, i32)> = vec![(
            "star".into(),
            sys.name.clone(),
            sys.star_color(),
            11,
        )];
        let mut planets: Vec<_> = sys.planets.iter().collect();
        planets.sort_by(|a, b| a.elements.semi_major_au.partial_cmp(&b.elements.semi_major_au).unwrap());
        for p in &planets {
            let r = if p.is_gas_giant { 7 } else { 4 };
            marks.push((p.id.clone(), p.name.clone(), p.color, r));
            for st in sys.stations.iter().filter(|s| s.parent_planet_id == p.id) {
                marks.push((st.id.clone(), st.name.clone(), palette::HUD_WHITE, 2));
            }
            for sp in &p.starports {
                marks.push((sp.id.clone(), sp.name.clone(), palette::HUD_WHITE, 2));
            }
        }
        let n = marks.len().max(1) as i32;
        let gap = ((W - 48) / n).clamp(16, 52);
        let y = 64;
        if self.data_page == 0 {
            for (i, (id, name, color, r)) in marks.iter().enumerate() {
                let x = 28 + i as i32 * gap;
                disc(cv, x, y, *r, *color);
                self.hits.push((Rect::new(x - 10, y - 12, 24, 28), super::Action::Info(id.clone())));
                if self.data_body.as_deref() == Some(id.as_str()) {
                    lock_box(cv, x, y, r + 4, palette::HUD_WHITE);
                    self.a.font.draw(cv, name, x + r + 4, y - 18, CONSOLE, palette::CREAM);
                }
            }
            let count = (sys.planets.len() + sys.stations.len()).max(1);
            self.a.font.draw(
                cv,
                &format!("Stable system with {count} major bodies."),
                8,
                108,
                CONSOLE,
                palette::HUD_WHITE,
            );
            let text = if let Some(id) = self.data_body.clone() {
                self.body_facts(&sys_id, &id)
            } else {
                system_blurb(&sys_id).unwrap_or("").to_string()
            };
            let mut line_y = 136;
            for para in text.split('\n') {
                for line in self.a.font.wrap(para, CONSOLE, W - 16) {
                    self.a.font.draw(cv, &line, 8, line_y, CONSOLE, palette::HUD_WHITE);
                    line_y += 22;
                    if line_y > VIEW_H - 28 {
                        break;
                    }
                }
            }
        } else if self.data_page == 1 {
            self.data_heading(cv, "Economy");
            let lines = [
                format!("Economy  {}", sys.economy),
                format!("Tech level  {}", sys.tech_level),
            ];
            for (i, line) in lines.iter().enumerate() {
                self.a.font.draw(cv, line, 8, 48 + i as i32 * 24, CONSOLE, palette::HUD_WHITE);
            }
        } else {
            self.data_heading(cv, "Political");
            let sector = format!("{},{}", (sys.position_ly.x / 8.0).floor() as i32, (sys.position_ly.z / 8.0).floor() as i32);
            let lines = [
                format!("Government  {}", sys.government),
                format!("Allegiance  {}", sys.allegiance),
                format!("Sector  {sector}"),
            ];
            for (i, line) in lines.iter().enumerate() {
                self.a.font.draw(cv, line, 8, 48 + i as i32 * 24, CONSOLE, palette::HUD_WHITE);
            }
        }
        let footer = if sys_id == "sol" {
            "Historic Federal capital. Administration and Tourism."
        } else {
            ""
        };
        if !footer.is_empty() {
            self.a.font.draw(cv, footer, 8, VIEW_H - 44, CONSOLE, palette::HUD_WHITE);
        }
        let loaded = "Loaded Galactic Map";
        let lw = self.a.font.width(loaded, CONSOLE);
        self.a.font.draw(cv, loaded, W - lw - 8, VIEW_H - 22, CONSOLE, palette::CON_ORANGE);
    }

    fn data_heading(&self, cv: &mut Canvas, title: &str) {
        self.a.font.draw(cv, title, 8, 12, CONSOLE, palette::CON_GREEN);
    }

    fn body_facts(&self, sys_id: &str, id: &str) -> String {
        let sys = self.galaxy.get(sys_id);
        if id == "star" {
            return format!("{}\n{}", sys.name, star_phrase(&sys.spectral));
        }
        if let Some(p) = sys.planet_by_id(id) {
            let e = &p.elements;
            let period = e.period();
            let ports: Vec<_> = p.starports.iter().map(|s| s.name.as_str()).collect();
            let port_line = if ports.is_empty() {
                String::new()
            } else {
                format!("\nStarports  {}", ports.join(", "))
            };
            return format!(
                "{}\nOrbital period  {period:.1} days\nOrbital radius  {:.2} AU\nEccentricity  {:.3}\nInclination  {:.1}°{port_line}",
                p.name, e.semi_major_au, e.eccentricity, e.inclination_deg
            );
        }
        if let Some(st) = sys.stations.iter().find(|s| s.id == id) {
            let parent = sys.planet_by_id(&st.parent_planet_id).map(|p| p.name.as_str()).unwrap_or("its planet");
            return format!("{}\nOrbits {parent}", st.name);
        }
        self.body_label(id)
    }

    fn put_label(&self, cv: &mut Canvas, text: &str, x: i32, y: i32, color: Rgb, taken: &mut Vec<Rect>) {
        let w = self.a.font.width(text, CONSOLE);
        let r = Rect::new(x, y, w, 16);
        if taken.iter().any(|o| overlaps(*o, r)) {
            return;
        }
        self.a.font.draw(cv, text, x, y, CONSOLE, color);
        taken.push(r);
    }

    fn body_world(&self, id: &str, days: f64) -> DVec3 {
        if id == "star" || self.system().planet_by_id(id).is_some() || self.system().stations.iter().any(|s| s.id == id)
        {
            self.frames.world_position(id, days)
        } else {
            DVec3::ZERO
        }
    }

    fn system_span_km(&self) -> f64 {
        let mut max_km: f64 = 5.0e6;
        for p in &self.system().planets {
            if p.parent_id.is_none() {
                max_km = max_km.max(p.elements.semi_major_au * AU_KM);
            }
        }
        max_km
    }

    pub(super) fn body_label(&self, id: &str) -> String {
        if id == "star" {
            return self.system().name.clone();
        }
        if let Some(p) = self.system().planet_by_id(id) {
            return p.name.clone();
        }
        self.system()
            .stations
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_default()
    }

    pub(super) fn walk_chart(&mut self, dt: f64) {
        let dir = if self.held.contains(&winit::keyboard::KeyCode::ArrowUp) {
            Some(DVec3::Z)
        } else if self.held.contains(&winit::keyboard::KeyCode::ArrowDown) {
            Some(-DVec3::Z)
        } else if self.held.contains(&winit::keyboard::KeyCode::ArrowRight) {
            Some(DVec3::X)
        } else if self.held.contains(&winit::keyboard::KeyCode::ArrowLeft) {
            Some(-DVec3::X)
        } else {
            None
        };
        let Some(dir) = dir else {
            self.map_repeat = 0.0;
            return;
        };
        self.map_repeat -= dt;
        if self.map_repeat <= 0.0 {
            self.step_dest(dir);
            self.map_repeat = 0.16;
        }
    }

    pub(super) fn map_fn(&mut self, key: u8) {
        let galaxy = self.map_page == MapPage::Galaxy;
        match key {
            7 if galaxy && self.map_tools => self.map_stalks = !self.map_stalks,
            7 if galaxy => self.map_zoom = (self.map_zoom * 1.25).clamp(1.5, 64.0),
            8 if galaxy && self.map_tools => self.map_grid = !self.map_grid,
            8 if galaxy => self.map_zoom = (self.map_zoom / 1.25).clamp(1.5, 64.0),
            10 if self.map_tools && galaxy => self.map_names = !self.map_names,
            10 if self.map_tools => self.sys_names = !self.sys_names,
            10 => self.arm_target = !self.arm_target,
            7 => self.sys_zoom = (self.sys_zoom * 1.35).clamp(0.15, 80.0),
            8 if self.map_tools => self.sys_orbits = !self.sys_orbits,
            8 => self.sys_zoom = (self.sys_zoom / 1.35).clamp(0.15, 80.0),
            _ => {}
        }
    }

    /// F8 from the view, or H from anywhere, and only while flying.
    /// The manual's two refusals are "Out of Range" and "Insufficient Fuel".
    pub(super) fn try_jump(&mut self) {
        if self.jump.is_some() || !matches!(self.wh, super::Where::Flying) || self.dest == self.sys_id {
            return;
        }
        let dist = self.galaxy.distance_ly(&self.sys_id, &self.dest);
        let stats = self.flight_stats();
        if stats.drive_h <= 0.0 || dist > stats.jump_range_ly + 1e-6 {
            self.msg("Out of Range");
            return;
        }
        let fuel = frontier_sim::frontier::jump_fuel_t(dist, stats.mass_t);
        if !self.infinite_fuel && self.fitted.hydrogen_t() < fuel {
            self.msg("Insufficient Fuel");
            return;
        }
        let hours = frontier_sim::frontier::jump_hours(dist, stats.mass_t, stats.drive_h);
        self.jump = Some(super::JumpAnim { t: 0.0, dest: self.dest.clone(), fuel, hours });
    }

    pub(super) fn finish_jump(&mut self) {
        let Some(jump) = self.jump.take() else { return };
        if !self.infinite_fuel {
            if let Some(n) = self.fitted.cargo.get_mut("hydrogen_fuel") {
                *n -= jump.fuel;
                if *n <= 0 {
                    self.fitted.cargo.remove("hydrogen_fuel");
                }
            }
        }
        let from = self.galaxy.get(&self.sys_id).name.clone();
        self.stardate.days += jump.hours / 24.0;
        let mass = self.flight_stats().mass_t;
        self.arrive(&jump.dest, from, mass);
    }

    fn arrive(&mut self, id: &str, from: String, mass_t: f64) {
        self.sys_id = id.to_string();
        self.frames = frontier_sim::build_frame_system(self.galaxy.get(id));
        let far = self.system_span_km() * 1.15;
        self.ship = frontier_sim::FrameState::new("star");
        self.ship.position = DVec3::new(0.0, far * 0.15, far);
        let fwd = (-self.ship.position).normalize();
        let right = DVec3::Y.cross(fwd).try_normalize().unwrap_or(DVec3::X);
        let up = fwd.cross(right);
        self.ship.orientation = glam::DMat3::from_cols(right, up, fwd);
        // The ship faces the star, so the exit cloud sits behind the tail.
        let away = self.ship.position.try_normalize().unwrap_or(glam::DVec3::Z);
        self.clouds.push(super::Residue {
            system: id.to_string(),
            pos: self.ship.position + away * 40.0,
            origin: from,
            arrived: self.stardate.days,
            mass_t,
            expires: self.stardate.days + super::CLOUD_LIFE_DAYS,
        });
        self.cloud_sel = None;
        self.wh = super::Where::Flying;
        self.panel = super::Panel::View;
        self.gear_down = false;
        self.launch_clearance = false;
        self.dock_clearance = None;
        self.port = None;
        self.focus = "star".into();
        self.ap_body = None;
        self.traffic.clear();
        self.spawned_traffic = false;
        self.police_misses = 0;
        self.police_warned = false;
        self.station_told = false;
        self.fm.controls.mode = frontier_sim::flight::FlightMode::Manual;
        self.fm.controls.set_speed = 0.0;
        self.fm.controls.thrust = 0.0;
        self.fm.stats = self.yard.stats(&self.fitted);
        let name = self.galaxy.get(id).name.clone();
        self.msg(name);
    }

    pub(super) fn sync_music(&mut self) {
        let key = match self.screen {
            super::Screen::Title | super::Screen::Destroyed => "intro",
            super::Screen::Play if self.panel == super::Panel::Map => "bare",
            super::Screen::Play if self.in_dock() => "docked",
            super::Screen::Play => "space",
        };
        self.music.set(key);
        self.music.gain(if self.prefs.music { 0.9 } else { 0.0 });
    }

    /// Nearest system in `dir` (a unit vector in the galactic plane) becomes
    /// the one in the centre.
    pub(super) fn step_dest(&mut self, dir: DVec3) {
        let here = self.galaxy.get(&self.dest).position_ly;
        let mut best: Option<(f64, String)> = None;
        let mut wider: Option<(f64, String)> = None;
        for (id, sys) in &self.galaxy.systems {
            if id == &self.dest {
                continue;
            }
            let flat = DVec3::new(sys.position_ly.x - here.x, 0.0, sys.position_ly.z - here.z);
            let len = flat.length();
            if len < 0.05 {
                continue;
            }
            let dot = flat.normalize().dot(dir);
            if dot <= 0.05 {
                continue;
            }
            let score = len / dot;
            if wider.as_ref().map_or(true, |(s, _)| score < *s) {
                wider = Some((score, id.clone()));
            }
            if dot >= 0.45 && best.as_ref().map_or(true, |(s, _)| score < *s) {
                best = Some((score, id.clone()));
            }
        }
        if let Some((_, id)) = best.or(wider) {
            self.cam_at = self.galaxy.get(&id).position_ly;
            self.dest = id;
            self.arm_target = false;
        }
    }

    pub(super) fn map_lines(&self) -> Vec<(String, Rgb)> {
        if self.map_page == MapPage::Data {
            let sys = self.galaxy.get(&self.dest);
            let dist = self.galaxy.distance_ly(&self.sys_id, &self.dest);
            return vec![
                (sys.name.clone(), palette::CON_GREEN),
                (format!("Dist. {dist:.2} light years"), palette::HUD_WHITE),
                (star_phrase(&sys.spectral), palette::CON_GREEN),
            ];
        }
        if self.map_page == MapPage::System {
            return vec![(self.body_label(&self.focus), palette::CON_GREEN)];
        }
        let name = self.galaxy.get(&self.dest).name.clone();
        let mut lines = vec![(name, palette::CON_GREEN)];
        if self.dest != self.sys_id {
            let dist = self.galaxy.distance_ly(&self.sys_id, &self.dest);
            lines.push((format!("{dist:.2} ly"), palette::CON_ORANGE));
            let stats = self.flight_stats();
            if dist > stats.jump_range_ly + 1e-6 {
                lines.push(("Out of Range".into(), palette::CON_GREEN));
            }
            if !self.infinite_fuel && self.fitted.hydrogen_t() < frontier_sim::frontier::jump_fuel_t(dist, stats.mass_t) {
                lines.push(("Insufficient Fuel".into(), palette::CON_GREEN));
            }
        }
        lines
    }
}

fn star_phrase(spectral: &str) -> String {
    let letter = spectral.chars().next().unwrap_or('G');
    let kind = match letter {
        'O' | 'B' => "blue",
        'A' => "white",
        'F' => "yellow-white",
        'G' => "yellow",
        'K' => "orange",
        'M' => "red",
        _ => "yellow",
    };
    format!("Type {letter} {kind} star")
}

/// Wording taken from the original Sol data screen.
fn system_blurb(id: &str) -> Option<&'static str> {
    match id {
        "sol" => Some(
            "Historic system famous as both the birthplace of Humanity and as the political capital of the Federation. A very expensive and prestigious system to live in, and a popular tourist venue. Most rich humans will visit Earth once in their lives. All the major corporations have their headquarters on Mars, terraformed in 2286, which is the main centre for administration.",
        ),
        _ => None,
    }
}

fn project(p: DVec3, yaw: f64, pitch: f64, scale: f64) -> (f64, f64) {
    let (sy, cy) = yaw.sin_cos();
    let x1 = p.x * cy + p.z * sy;
    let z1 = -p.x * sy + p.z * cy;
    let (sp, cp) = pitch.sin_cos();
    let y2 = p.y * cp - z1 * sp;
    let z2 = p.y * sp + z1 * cp;
    let s = scale / (1.0 + z2 * scale * 0.0015).max(0.3);
    (x1 * s, -y2 * s)
}

fn overlaps(a: Rect, b: Rect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

fn lock_box(cv: &mut Canvas, x: i32, y: i32, r: i32, c: Rgb) {
    cv.rect(Rect::new(x - r, y - r, r * 2 + 1, r * 2 + 1), c);
    cv.rect(Rect::new(x - r + 1, y - r + 1, r * 2 - 1, r * 2 - 1), c);
}

fn px(origin: f64, d: f64) -> i32 {
    (origin + d).round() as i32
}

fn disc(cv: &mut Canvas, x: i32, y: i32, r: i32, c: Rgb) {
    for dy in -r..=r {
        for dx in -r..=r {
            if dx * dx + dy * dy <= r * r {
                cv.put(x + dx, y + dy, c);
            }
        }
    }
}

fn circle(cv: &mut Canvas, centre: DVec3, radius: f64, at: &impl Fn(DVec3) -> (i32, i32)) {
    let mut prev = None;
    for i in 0..=48 {
        let a = i as f64 / 48.0 * std::f64::consts::TAU;
        let pt = at(centre + DVec3::new(radius * a.cos(), 0.0, radius * a.sin()));
        if let Some((x0, y0)) = prev {
            cv.line(x0, y0, pt.0, pt.1, ORBIT);
        }
        prev = Some(pt);
    }
}
