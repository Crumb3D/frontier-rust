//! Game state and rules for the vertical slice, following the FE2 manual:
//! start landed at Sirocco Starport on Merlin at 12:00 on 1-Jan-3200, F4 Launch
//! Request, F7 vertical take-off, fly with gravity, then land (clearance, gear F9)
//! or dock at Merlin Orbital (clearance, slow approach into the slot).

mod bulletin;
mod station;

use crate::assets::Assets;
use crate::console::{self, ConsoleState, ScanContact, CON_H};
use crate::fb::{Canvas, Rect};
use crate::font::CONSOLE;
use crate::meshes::{self, Mesh};
use crate::palette::{self, Rgb};
use crate::render3d::{self, BodyKind, Instance, NearGround, Rings, SphereBody, View};
use crate::screens::{self, RosterRow};
use bulletin::{Contract, Observed, Talk};
use frontier_sim::flight::{FlightMode, FlightModel};
use frontier_sim::frame::spin_about_y;
use frontier_sim::galaxy::{surface_offset, Planet};
use frontier_sim::{
    build_frame_system, default_eagle, FittedShip, FrameState, FrameSystem, Galaxy, Market, Shipyard, Stardate,
    StarSystem,
};
use glam::{DMat3, DVec3};
use station::{Page, Port};
use std::collections::{HashMap, HashSet};
use winit::keyboard::KeyCode;

pub const W: i32 = 640;
pub const H: i32 = 400;
pub const VIEW_H: i32 = H - CON_H;

/// Height of the cockpit above the ground when sitting on the undercarriage, km.
const SHIP_H: f64 = 0.002;
/// Touch-down limits relative to the ground, km/s.
const SOFT_LANDING: f64 = 0.03;
const CRASH_SPEED: f64 = 0.06;
/// Docking: the slot mouth (fraction of station radius) and the safe entry speed.
const SLOT_R: f64 = 0.3;
const DOCK_SPEED: f64 = 0.1;
const STATION_SPIN_S: f64 = 60.0;
const VCR_FACTORS: [f64; 6] = [0.0, 1.0, 10.0, 100.0, 1000.0, 10000.0];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Title,
    Play,
    Destroyed,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Panel {
    View,
    Comms,
    Inventory,
}

#[derive(Clone, Debug)]
pub enum Where {
    /// Sitting on the ground. `local` and `heading` are in the planet's body axes.
    Landed { planet: String, port: Option<String>, local: DVec3, heading: DVec3 },
    Docked { station: String },
    Flying,
}

#[derive(Clone, Copy, Debug)]
pub enum Action {
    Key(KeyCode),
    Vcr(usize),
    Comms(usize),
    Title(usize),
    Station(station::Hit),
}

/// What speeds and altitude are measured against this frame.
struct Reference {
    name: String,
    /// Velocity of the reference in the ship's parent frame.
    vel: DVec3,
}

pub struct Game {
    pub a: Assets,
    galaxy: Galaxy,
    pub yard: Shipyard,
    market: Market,
    observed: Observed,
    pub screen: Screen,
    pub panel: Panel,
    pub title_t: f64,
    title_frame: Option<(usize, image::RgbaImage)>,
    pub stardate: Stardate,
    pub vcr: usize,
    pub sys_id: String,
    frames: FrameSystem,
    pub fitted: FittedShip,
    /// Tenths of a credit, as the original counts them.
    pub cash: i64,
    pub wh: Where,
    /// Station services page shown under F4 while docked or landed at a port.
    page: Page,
    scroll: usize,
    port: Option<Port>,
    port_visits: usize,
    talk: Talk,
    contracts: Vec<Contract>,
    /// Day of the last drive and equipment service; None is 31-Dec-3199.
    serviced: (Option<i64>, Option<i64>),
    /// F3 page: equipment status, commander profile, passenger roster.
    inv_page: usize,
    anim_t: f64,
    pub ship: FrameState,
    pub fm: FlightModel,
    pub launch_clearance: bool,
    pub dock_clearance: Option<String>,
    pub gear_down: bool,
    pub labels: bool,
    pub rear: bool,
    /// Scanner-well text: paragraphs in their own colours.
    pub message: Vec<(String, Rgb)>,
    message_t: f64,
    message_face: bool,
    held: HashSet<KeyCode>,
    pub mouse: (i32, i32),
    rmb: bool,
    mouse_acc: (f64, f64),
    stars: Vec<(DVec3, u8)>,
    station_mesh: Mesh,
    port_mesh: Mesh,
    pub quit: bool,
    pub hits: Vec<(Rect, Action)>,
    /// Heading of each starport's layout about its local vertical, radians from north.
    port_yaw: HashMap<String, f64>,
}

fn surface_g(p: &Planet) -> f64 {
    if p.is_gas_giant {
        0.0248 * p.radius_km / 69911.0
    } else {
        0.00981 * p.radius_km / 6371.0
    }
}

fn gm(p: &Planet) -> f64 {
    surface_g(p) * p.radius_km * p.radius_km
}

fn omega(p: &Planet) -> DVec3 {
    DVec3::Y * (std::f64::consts::TAU / (p.rotation_days.max(1e-3) * 86400.0))
}

/// Right, up, forward frame standing on a sphere at unit normal `n`, facing north.
fn surface_frame(n: DVec3) -> DMat3 {
    let east = n.cross(DVec3::Y).try_normalize().unwrap_or(DVec3::X);
    let north = east.cross(n);
    DMat3::from_cols(east, n, north)
}

impl Game {
    pub fn new(a: Assets) -> Result<Self, String> {
        let galaxy = Galaxy::load()?;
        let yard = Shipyard::load()?;
        let fitted = default_eagle();
        let fm = FlightModel::new(yard.stats(&fitted));
        let frames = build_frame_system(galaxy.get("ross154"));
        Ok(Self {
            a,
            galaxy,
            yard,
            market: Market::load()?,
            observed: Observed::load()?,
            screen: Screen::Title,
            panel: Panel::View,
            title_t: 0.0,
            title_frame: None,
            stardate: Stardate::new(0.5),
            vcr: 1,
            sys_id: "ross154".into(),
            frames,
            fitted,
            cash: 1000,
            wh: Where::Flying,
            page: Page::Services,
            scroll: 0,
            port: None,
            port_visits: 0,
            talk: Talk::default(),
            contracts: Vec::new(),
            serviced: (None, None),
            inv_page: 0,
            anim_t: 0.0,
            ship: FrameState::new("ross154_merlin"),
            fm,
            launch_clearance: false,
            dock_clearance: None,
            gear_down: true,
            labels: false,
            rear: false,
            message: Vec::new(),
            message_t: 0.0,
            message_face: false,
            held: HashSet::new(),
            mouse: (0, 0),
            rmb: false,
            mouse_acc: (0.0, 0.0),
            stars: render3d::starfield(0x5eed_f2, 900),
            station_mesh: meshes::station(1.0),
            port_mesh: meshes::starport(7),
            quit: false,
            hits: Vec::new(),
            port_yaw: HashMap::new(),
        })
    }

    pub fn system(&self) -> &StarSystem {
        self.galaxy.get(&self.sys_id)
    }

    /// New game: the original's opening, landed at Sirocco, 12:00 1-Jan-3200, with
    /// ¢100.0 in cash.
    pub fn new_game(&mut self) {
        self.sys_id = "ross154".into();
        self.frames = build_frame_system(self.galaxy.get(&self.sys_id));
        self.stardate = Stardate::new(0.5);
        self.vcr = 1;
        self.fitted = default_eagle();
        self.fm = FlightModel::new(self.yard.stats(&self.fitted));
        self.cash = 1000;
        self.port = None;
        self.port_visits = 0;
        self.contracts.clear();
        self.serviced = (None, None);
        self.page = Page::Services;
        self.inv_page = 0;
        self.launch_clearance = false;
        self.dock_clearance = None;
        self.gear_down = true;
        self.labels = false;
        self.rear = false;
        self.panel = Panel::View;
        self.message.clear();
        self.land_at_port("ross154_sirocco");
        self.face_opening_view();
        self.screen = Screen::Play;
    }

    /// Lay out Sirocco so that, parked on the pad, Aster hangs up and to the right
    /// as in the opening still.
    fn face_opening_view(&mut self) {
        let Where::Landed { port: Some(port), .. } = self.wh.clone() else { return };
        let days = self.stardate.days;
        let cam = self.frames.ship_world_pos(&self.ship, days);
        let d = (self.frames.world_position("ross154_aster", days) - cam).normalize();
        let o = self.ship.orientation;
        let az = d.dot(o.col(0)).atan2(d.dot(o.col(2)));
        *self.port_yaw.entry(port.clone()).or_insert(0.0) += az - 38f64.to_radians();
        self.land_at_port(&port);
    }

    fn land_at_port(&mut self, port_id: &str) {
        let sys = self.galaxy.get(&self.sys_id);
        let Some(sp) = sys.starport_by_id(port_id) else { return };
        let Some(p) = sys.planet_by_id(&sp.planet_id) else { return };
        let off = surface_offset(sp.lat_deg, sp.lon_deg, p.radius_km);
        let n = off.normalize();
        let (planet, radius) = (p.id.clone(), p.radius_km);
        self.wh = Where::Landed {
            planet,
            port: Some(port_id.to_string()),
            local: n * (radius + SHIP_H),
            heading: self.port_frame(port_id, n).col(2),
        };
        self.sync_landed();
    }

    /// A starport's layout frame at surface normal `n`: right, up, and the pad heading.
    fn port_frame(&self, port_id: &str, n: DVec3) -> DMat3 {
        surface_frame(n) * DMat3::from_rotation_y(self.port_yaw.get(port_id).copied().unwrap_or(0.0))
    }

    fn planet(&self, id: &str) -> Option<&Planet> {
        self.system().planet_by_id(id)
    }

    fn body_vel(&self, id: &str, days: f64) -> DVec3 {
        let p0 = self.frames.world_position(id, days);
        let p1 = self.frames.world_position(id, days + 1e-6);
        (p1 - p0) / (1e-6 * 86400.0)
    }

    pub fn station_orient(&self, station_id: &str, days: f64) -> DMat3 {
        let st = self.system().stations.iter().find(|s| s.id == station_id);
        let Some(st) = st else { return DMat3::IDENTITY };
        let rel = self.frames.world_position(&st.id, days) - self.frames.world_position(&st.parent_planet_id, days);
        // The slot (+Z) faces away from the planet.
        let z = rel.normalize_or_zero();
        let x = surface_frame(z).col(0);
        let y = z.cross(x);
        let a = std::f64::consts::TAU * (days * 86400.0 / STATION_SPIN_S).fract();
        let (c, s) = (a.cos(), a.sin());
        DMat3::from_cols(x * c + y * s, y * c - x * s, z)
    }

    /// Ship state follows the turning ground (or the station) while landed or docked.
    fn sync_landed(&mut self) {
        let days = self.stardate.days;
        match self.wh.clone() {
            Where::Landed { planet, local, heading, .. } => {
                let Some(p) = self.planet(&planet).cloned() else { return };
                let spin = spin_about_y(days, p.rotation_days);
                let pos = spin * local;
                let n = pos.normalize();
                let fwd = (spin * heading - n * n.dot(spin * heading)).normalize();
                let right = n.cross(fwd);
                self.ship = FrameState {
                    parent_id: planet.clone(),
                    position: pos,
                    velocity: omega(&p).cross(pos),
                    orientation: DMat3::from_cols(right, n, fwd),
                    angular_velocity: DVec3::ZERO,
                };
            }
            Where::Docked { station } => {
                let Some(st) = self.system().stations.iter().find(|s| s.id == station).cloned() else { return };
                let parent = st.parent_planet_id.clone();
                let o = self.station_orient(&station, days);
                let rel = self.frames.world_position(&st.id, days) - self.frames.world_position(&parent, days);
                let v = self.body_vel(&st.id, days) - self.body_vel(&parent, days);
                self.ship = FrameState {
                    parent_id: parent,
                    position: rel,
                    velocity: v,
                    orientation: DMat3::from_cols(-o.col(0), o.col(1), -o.col(2)),
                    angular_velocity: DVec3::ZERO,
                };
            }
            Where::Flying => {}
        }
    }

    /// A passing green message in the scanner well.
    pub fn msg(&mut self, text: impl Into<String>) {
        self.message = vec![(text.into(), palette::CON_GREEN)];
        self.message_t = 5.0;
        self.message_face = false;
    }

    /// Station-page text that stays until the page changes.
    fn say(&mut self, paras: Vec<(String, Rgb)>) {
        self.message = paras;
        self.message_t = f64::INFINITY;
        self.message_face = false;
    }

    /// "Message from Sirocco Station:" with the controller's face, as in the original.
    fn msg_from(&mut self, who: &str, text: &str) {
        self.message = vec![(format!("Message from {who}:"), palette::CON_ORANGE), (text.into(), palette::CON_GREEN)];
        self.message_t = 8.0;
        self.message_face = true;
    }

    fn clear_msg(&mut self) {
        self.message.clear();
        self.message_t = 0.0;
        self.message_face = false;
    }

    fn message_text(&self) -> String {
        self.message.iter().map(|m| m.0.as_str()).collect::<Vec<_>>().join(" ")
    }

    // ---------------------------------------------------------------- frames

    /// Pick the coordinate parent: the smallest planet/moon sphere of influence
    /// (20 radii) that contains the ship, otherwise the star.
    fn choose_parent(&self, world: DVec3, days: f64) -> String {
        let mut best: Option<(f64, String)> = None;
        for p in &self.system().planets {
            let d = (world - self.frames.world_position(&p.id, days)).length();
            let soi = p.radius_km * 20.0;
            if d < soi && best.as_ref().map_or(true, |(s, _)| soi < *s) {
                best = Some((soi, p.id.clone()));
            }
        }
        best.map(|(_, id)| id).unwrap_or_else(|| "star".into())
    }

    fn gravity(&self, parent: &str, pos: DVec3) -> DVec3 {
        let mu = match self.planet(parent) {
            Some(p) => gm(p),
            None => self.system().mass * 1.327e11,
        };
        let r2 = pos.length_squared().max(1.0);
        -pos.normalize_or_zero() * (mu / r2)
    }

    fn nearest_station(&self, world: DVec3, days: f64) -> Option<(String, String, f64, f64)> {
        self.system()
            .stations
            .iter()
            .map(|s| {
                let d = (world - self.frames.world_position(&s.id, days)).length();
                (s.id.clone(), s.name.clone(), d, s.radius_km)
            })
            .min_by(|a, b| a.2.partial_cmp(&b.2).unwrap())
    }

    fn reference(&self, days: f64) -> Reference {
        let world = self.frames.ship_world_pos(&self.ship, days);
        if let Some((sid, name, d, _)) = self.nearest_station(world, days) {
            if d < 30.0 {
                let vel = self.body_vel(&sid, days) - self.body_vel(&self.ship.parent_id, days);
                return Reference { name, vel };
            }
        }
        match self.planet(&self.ship.parent_id) {
            Some(p) => {
                let alt = self.ship.position.length() - p.radius_km;
                let vel = if alt < (p.atmosphere_km * 2.0).max(100.0) {
                    omega(p).cross(self.ship.position)
                } else {
                    DVec3::ZERO
                };
                Reference { name: p.name.clone(), vel }
            }
            None => Reference { name: self.system().name.clone(), vel: DVec3::ZERO },
        }
    }

    pub fn altitude_km(&self) -> f64 {
        match self.planet(&self.ship.parent_id) {
            Some(p) => self.ship.position.length() - p.radius_km - SHIP_H,
            None => self.ship.position.length(),
        }
    }

    // ---------------------------------------------------------------- input

    pub fn key_down(&mut self, key: KeyCode, shift: bool) {
        self.held.insert(key);
        match self.screen {
            Screen::Title => self.title_key(key),
            Screen::Destroyed => {
                if matches!(key, KeyCode::Enter | KeyCode::Space | KeyCode::Escape) {
                    self.screen = Screen::Title;
                }
            }
            Screen::Play => self.play_key(key, shift),
        }
    }

    pub fn key_up(&mut self, key: KeyCode) {
        self.held.remove(&key);
    }

    pub fn mouse_move(&mut self, x: i32, y: i32) {
        if self.rmb {
            self.mouse_acc.0 += (x - self.mouse.0) as f64;
            self.mouse_acc.1 += (y - self.mouse.1) as f64;
        }
        self.mouse = (x, y);
    }

    pub fn mouse_button(&mut self, left: bool, down: bool) {
        if !left {
            self.rmb = down;
            return;
        }
        if !down {
            return;
        }
        let hit = self.hits.iter().rev().find(|(r, _)| r.contains(self.mouse.0, self.mouse.1)).map(|h| h.1);
        match hit {
            Some(Action::Key(k)) => self.key_down(k, false),
            Some(Action::Vcr(i)) => self.set_vcr(i),
            Some(Action::Comms(i)) => self.comms_pick(i),
            Some(Action::Title(i)) => self.title_pick(i),
            Some(Action::Station(h)) => self.station_hit(h),
            None => {}
        }
    }

    fn title_key(&mut self, key: KeyCode) {
        match key {
            KeyCode::Digit1 | KeyCode::Numpad1 | KeyCode::Enter | KeyCode::Space => self.title_pick(0),
            KeyCode::Digit2 => self.title_pick(1),
            KeyCode::Digit3 => self.title_pick(2),
            KeyCode::Digit4 => self.title_pick(3),
            KeyCode::Digit5 => self.title_pick(4),
            KeyCode::Escape => self.quit = true,
            _ => {}
        }
    }

    /// Start positions: only Ross 154 exists in this build; the others are listed
    /// as on the original's menu but do nothing yet.
    fn title_pick(&mut self, i: usize) {
        if i == 0 {
            self.new_game();
        }
    }

    fn set_vcr(&mut self, i: usize) {
        self.vcr = i.min(5);
    }

    pub fn in_dock(&self) -> bool {
        matches!(self.wh, Where::Docked { .. } | Where::Landed { port: Some(_), .. })
    }

    fn play_key(&mut self, key: KeyCode, shift: bool) {
        if shift {
            let slot = match key {
                KeyCode::F1 => Some(0),
                KeyCode::F2 => Some(1),
                KeyCode::F3 => Some(2),
                KeyCode::F4 => Some(3),
                KeyCode::F5 => Some(4),
                KeyCode::F6 => Some(5),
                _ => None,
            };
            if let Some(s) = slot {
                self.set_vcr(s);
                return;
            }
        }
        match key {
            KeyCode::Escape => self.screen = Screen::Title,
            KeyCode::F1 => {
                if self.panel == Panel::View {
                    self.rear = !self.rear;
                } else if self.message_t.is_infinite() {
                    self.clear_msg();
                }
                self.panel = Panel::View;
            }
            KeyCode::F2 => self.msg("Galactic Map is not in this build yet."),
            KeyCode::F3 => {
                self.inv_page = if self.panel == Panel::Inventory { (self.inv_page + 1) % 3 } else { 0 };
                if self.message_t.is_infinite() {
                    self.clear_msg();
                }
                self.panel = Panel::Inventory;
            }
            // In dock F4 opens the services menu, and from any service page returns to it.
            KeyCode::F4 if self.in_dock() => {
                self.panel = Panel::Comms;
                self.open_page(Page::Services);
            }
            KeyCode::F4 => {
                self.panel = if self.panel == Panel::Comms { Panel::View } else { Panel::Comms };
            }
            KeyCode::F7 => self.flight_control(),
            KeyCode::F9 => {
                if matches!(self.wh, Where::Flying) {
                    self.gear_down = !self.gear_down;
                    self.msg(if self.gear_down { "Undercarriage lowered" } else { "Undercarriage raised" });
                }
            }
            KeyCode::F10 => self.labels = !self.labels,
            KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3 | KeyCode::Digit4 | KeyCode::Digit5
                if self.panel == Panel::Comms =>
            {
                let i = match key {
                    KeyCode::Digit1 => 0,
                    KeyCode::Digit2 => 1,
                    KeyCode::Digit3 => 2,
                    KeyCode::Digit4 => 3,
                    _ => 4,
                };
                if self.in_dock() {
                    self.station_key_option(i);
                } else {
                    self.comms_pick(i);
                }
            }
            KeyCode::Enter => {
                if let Where::Docked { .. } = self.wh {
                    if self.launch_clearance {
                        self.launch_from_station();
                    }
                }
            }
            _ => {}
        }
    }

    /// F7, following the icon: take off when landed, otherwise Manual <-> Engines Off.
    fn flight_control(&mut self) {
        match self.wh.clone() {
            Where::Landed { port, .. } => {
                if port.is_some() && !self.launch_clearance {
                    self.msg("Launch clearance has not been granted.");
                }
                self.launch_clearance = false;
                self.wh = Where::Flying;
                self.panel = Panel::View;
                self.fm.controls.mode = FlightMode::TakeOff;
                self.fm.controls.set_speed = 0.0;
                self.fm.controls.thrust = 0.0;
            }
            Where::Docked { .. } => {}
            Where::Flying => {
                self.fm.controls.mode = match self.fm.controls.mode {
                    FlightMode::Manual | FlightMode::TakeOff => FlightMode::EnginesOff,
                    FlightMode::EnginesOff => {
                        let rf = self.reference(self.stardate.days);
                        let v = self.ship.velocity - rf.vel;
                        self.fm.controls.set_speed = v.dot(self.ship.orientation.col(2));
                        FlightMode::Manual
                    }
                };
            }
        }
    }

    /// In flight, F4 offers clearance requests to traffic control in range.
    pub fn comms_options(&self) -> Vec<(String, String)> {
        match &self.wh {
            Where::Landed { .. } | Where::Docked { .. } => vec![],
            Where::Flying => {
                let days = self.stardate.days;
                let world = self.frames.ship_world_pos(&self.ship, days);
                let mut out = Vec::new();
                for st in &self.system().stations {
                    let d = (world - self.frames.world_position(&st.id, days)).length();
                    if d < 500.0 {
                        out.push((format!("Request docking permission - {}", st.name), st.id.clone()));
                    }
                }
                for p in &self.system().planets {
                    for sp in &p.starports {
                        let d = (world - self.frames.world_position(&sp.id, days)).length();
                        if d < 500.0 {
                            out.push((format!("Request landing permission - {}", sp.name), sp.id.clone()));
                        }
                    }
                }
                out
            }
        }
    }

    fn comms_pick(&mut self, i: usize) {
        let opts = self.comms_options();
        let Some((name, id)) = opts.get(i).cloned() else { return };
        self.dock_clearance = Some(id);
        let who = name.split(" - ").nth(1).unwrap_or("Traffic Control").to_string();
        self.msg(format!("{who}: permission granted."));
    }

    pub fn status_line(&self) -> String {
        let place = match &self.wh {
            Where::Landed { port: Some(p), .. } => format!("landed at {p}"),
            Where::Landed { planet, .. } => format!("landed on {planet}"),
            Where::Docked { station } => format!("docked at {station}"),
            Where::Flying => format!("flying, alt {:.3} km", self.altitude_km()),
        };
        format!("{:?}: {place}; cash {}; message \"{}\"", self.screen, crate::font::money(self.cash), self.message_text())
    }

    /// Test hook for `--shot`: open a station page as if clicked.
    pub fn shot_page(&mut self, name: &str) {
        self.panel = Panel::Comms;
        let page = match name {
            "stock" => Page::Stock,
            "bulletin" => Page::Bulletin,
            "shipyard" => Page::Shipyard,
            "upgrades" => Page::Upgrades,
            "repairs" => Page::Repairs,
            "ships" => Page::Ships,
            "police" => Page::Police,
            "federal" => Page::FederalHq,
            _ => Page::Services,
        };
        self.open_page(page);
    }

    /// Test hook for `--shot`: click a station control.
    pub fn shot_hit(&mut self, h: &str, i: usize) {
        let hit = match h {
            "buy" => station::Hit::Buy(i),
            "sell" => station::Hit::Sell(i),
            "ad" => station::Hit::Ad(i),
            "reply" => station::Hit::Reply(i),
            "scroll" => station::Hit::Scroll(i as i32),
            _ => station::Hit::Option(i),
        };
        self.station_hit(hit);
    }

    /// Elevation and azimuth (degrees, azimuth clockwise from the nose) of the star
    /// and each planet as seen from the ship.
    pub fn sky_report(&self) -> String {
        let days = self.stardate.days;
        let cam = self.frames.ship_world_pos(&self.ship, days);
        let o = self.ship.orientation;
        let up = self.ship.position.normalize_or_zero();
        let mut ids: Vec<(String, String)> = vec![("star".into(), "star".into())];
        ids.extend(self.system().planets.iter().map(|p| (p.id.clone(), p.name.clone())));
        ids.iter()
            .map(|(id, name)| {
                let d = (self.frames.world_position(id, days) - cam).normalize();
                let el = d.dot(up).asin().to_degrees();
                let az = d.dot(o.col(0)).atan2(d.dot(o.col(2))).to_degrees();
                format!("{name} el {el:.0} az {az:.0}")
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Test placement: flying `dist_km` out from the nearest station's slot, facing it.
    pub fn place_near_station(&mut self, dist_km: f64) {
        let days = self.stardate.days;
        let Some(st) = self.system().stations.first().cloned() else { return };
        let o = self.station_orient(&st.id, days);
        let parent = st.parent_planet_id.clone();
        let rel = self.frames.world_position(&st.id, days) - self.frames.world_position(&parent, days);
        self.ship = FrameState {
            parent_id: parent.clone(),
            position: rel + o.col(2) * dist_km,
            velocity: self.body_vel(&st.id, days) - self.body_vel(&parent, days),
            orientation: DMat3::from_cols(-o.col(0), o.col(1), -o.col(2)),
            angular_velocity: DVec3::ZERO,
        };
        self.wh = Where::Flying;
        self.panel = Panel::View;
        self.gear_down = false;
        self.fm.controls.mode = FlightMode::Manual;
        self.fm.controls.set_speed = 0.0;
    }

    /// Test placement: `alt_km` straight up from the pad, nose up, looking back at
    /// the planet in the rear view as after the original's climb out.
    pub fn place_above_port(&mut self, alt_km: f64) {
        let r = self.ship.position.length();
        let up = self.ship.position / r;
        let fwd = self.ship.orientation.col(2);
        self.ship.position = up * (r + alt_km);
        self.ship.orientation = DMat3::from_cols(up.cross(fwd), -fwd, up);
        self.wh = Where::Flying;
        self.panel = Panel::View;
        self.gear_down = false;
        self.rear = true;
        self.fm.controls.mode = FlightMode::Manual;
        self.fm.controls.set_speed = 0.0;
    }

    fn launch_from_station(&mut self) {
        let Where::Docked { station } = self.wh.clone() else { return };
        let days = self.stardate.days;
        let o = self.station_orient(&station, days);
        let r = self.system().stations.iter().find(|s| s.id == station).map_or(1.0, |s| s.radius_km);
        self.sync_landed();
        self.ship.position += o.col(2) * r * 1.4;
        self.ship.orientation = o;
        self.ship.velocity += o.col(2) * 0.05;
        self.fm.controls.mode = FlightMode::Manual;
        self.fm.controls.set_speed = 0.05;
        self.launch_clearance = false;
        self.wh = Where::Flying;
        self.panel = Panel::View;
        self.gear_down = false;
    }

    // ---------------------------------------------------------------- update

    pub fn update(&mut self, dt: f64) {
        self.anim_t += dt;
        match self.screen {
            Screen::Title | Screen::Destroyed => {
                self.title_t += dt;
                return;
            }
            Screen::Play => {}
        }
        if self.message_t > 0.0 {
            self.message_t -= dt;
            if self.message_t <= 0.0 {
                self.clear_msg();
            }
        }
        self.read_controls(dt);

        // The on-board computer returns to normal time near the ground or a station.
        if matches!(self.wh, Where::Flying) && self.vcr > 1 {
            let days = self.stardate.days;
            let world = self.frames.ship_world_pos(&self.ship, days);
            let near_station = self.nearest_station(world, days).is_some_and(|s| s.2 < 20.0);
            if near_station || self.altitude_km() < 5.0 {
                self.vcr = 1;
            }
        }
        let sim_dt = dt * VCR_FACTORS[self.vcr];
        if sim_dt <= 0.0 {
            return;
        }
        match self.wh {
            Where::Flying => {
                let steps = ((sim_dt / 0.1).ceil() as usize).clamp(1, 4000);
                let h = sim_dt / steps as f64;
                for _ in 0..steps {
                    self.stardate.days += h / 86400.0;
                    if !self.step_flight(h) {
                        break;
                    }
                }
                self.rebase();
            }
            _ => {
                self.stardate.days += sim_dt / 86400.0;
                self.sync_landed();
            }
        }
    }

    fn read_controls(&mut self, dt: f64) {
        let k = |c: KeyCode| self.held.contains(&c);
        let mut pitch = (k(KeyCode::KeyZ) as i32 - k(KeyCode::KeyA) as i32) as f64;
        let mut yaw = (k(KeyCode::Period) as i32 - k(KeyCode::Comma) as i32) as f64;
        let up = k(KeyCode::Enter) && !(self.panel == Panel::Comms && self.in_dock());
        let down = k(KeyCode::ShiftRight);
        let (mx, my) = std::mem::take(&mut self.mouse_acc);
        pitch += my * 0.06;
        yaw += mx * 0.06;
        let c = &mut self.fm.controls;
        c.pitch = pitch.clamp(-1.0, 1.0);
        c.yaw = yaw.clamp(-1.0, 1.0);
        c.roll = 0.0;
        if c.mode == FlightMode::EnginesOff {
            c.thrust = up as i32 as f64 - down as i32 as f64;
        } else {
            c.thrust = 0.0;
            let rate = (0.02 + c.set_speed.abs() * 0.8) * dt;
            if up {
                self.fm.adjust_set_speed(rate);
            }
            if down {
                self.fm.adjust_set_speed(-rate);
            }
        }
    }

    fn rebase(&mut self) {
        let days = self.stardate.days;
        let world = self.frames.ship_world_pos(&self.ship, days);
        let parent = self.choose_parent(world, days);
        if parent != self.ship.parent_id && self.frames.bodies.contains_key(&parent) {
            self.ship = self.frames.rebase(&self.ship, days, &parent);
        }
    }

    /// One physics step. Returns false when the ship has stopped flying.
    fn step_flight(&mut self, h: f64) -> bool {
        let days = self.stardate.days;
        let g = self.gravity(&self.ship.parent_id.clone(), self.ship.position);
        let rf = self.reference(days);
        self.ship = self.fm.step(&self.ship, h, g, rf.vel);

        if let Some(p) = self.planet(&self.ship.parent_id).cloned() {
            if self.ground_contact(&p) {
                return false;
            }
        }
        !self.station_contact(days)
    }

    fn ground_contact(&mut self, p: &Planet) -> bool {
        let pos = self.ship.position;
        let r = pos.length();
        if r - p.radius_km > SHIP_H {
            return false;
        }
        let n = pos / r;
        let ground_v = omega(p).cross(pos);
        let rel = self.ship.velocity - ground_v;
        let speed = rel.length();
        if !self.gear_down || speed > CRASH_SPEED {
            self.destroyed(if self.gear_down { "You hit the ground too fast." } else { "Crash landing - the undercarriage was up." });
            return true;
        }
        // A slow touch-down on the undercarriage settles level by itself.
        if speed < SOFT_LANDING {
            self.touch_down(p, n);
            return true;
        }
        // Bounce: back onto the surface, losing the inward motion.
        self.ship.position = n * (p.radius_km + SHIP_H);
        let inward = rel.dot(n).min(0.0);
        self.ship.velocity = ground_v + (rel - n * inward) * 0.5;
        false
    }

    fn touch_down(&mut self, p: &Planet, n: DVec3) {
        let days = self.stardate.days;
        let spin = spin_about_y(days, p.rotation_days);
        let mut port = None;
        let mut local_n = spin.transpose() * n;
        for sp in &p.starports {
            let off = surface_offset(sp.lat_deg, sp.lon_deg, p.radius_km);
            let dist = (off - local_n * p.radius_km).length();
            if dist < 0.5 {
                port = Some(sp.clone());
                local_n = off.normalize();
            }
        }
        let fwd = spin.transpose() * self.ship.orientation.col(2);
        let flat = fwd - local_n * local_n.dot(fwd);
        let heading = match &port {
            Some(sp) => self.port_frame(&sp.id, local_n).col(2),
            None if flat.length() > 0.1 => flat.normalize(),
            None => surface_frame(local_n).col(2),
        };
        let legal = port.as_ref().is_some_and(|sp| self.dock_clearance.as_deref() == Some(sp.id.as_str()));
        let text = match &port {
            Some(sp) if legal => format!("Landed at {}.", sp.name),
            Some(sp) => format!("Landed at {} without permission.", sp.name),
            None => format!("Landed on {}.", p.name),
        };
        self.wh = Where::Landed {
            planet: p.id.clone(),
            port: port.map(|sp| sp.id),
            local: local_n * (p.radius_km + SHIP_H),
            heading,
        };
        self.dock_clearance = None;
        self.port_visits += 1;
        self.fm.controls.set_speed = 0.0;
        self.fm.controls.mode = FlightMode::Manual;
        self.vcr = 1;
        self.sync_landed();
        self.msg(text);
    }

    fn station_contact(&mut self, days: f64) -> bool {
        let world = self.frames.ship_world_pos(&self.ship, days);
        let stations: Vec<_> = self.system().stations.iter().map(|s| (s.id.clone(), s.name.clone(), s.radius_km)).collect();
        for (sid, name, r) in stations {
            let rel = world - self.frames.world_position(&sid, days);
            if rel.length() > r * 1.2 {
                continue;
            }
            let o = self.station_orient(&sid, days);
            let local = o.transpose() * rel;
            let st_vel = self.body_vel(&sid, days) - self.body_vel(&self.ship.parent_id, days);
            let speed = (self.ship.velocity - st_vel).length();
            let in_mouth = local.z > 0.0 && (local.x * local.x + local.y * local.y).sqrt() < SLOT_R * r;
            if in_mouth && local.z < r * 1.02 {
                if self.dock_clearance.as_deref() == Some(sid.as_str()) && speed < DOCK_SPEED {
                    self.wh = Where::Docked { station: sid.clone() };
                    self.dock_clearance = None;
                    self.port_visits += 1;
                    self.vcr = 1;
                    self.sync_landed();
                    self.panel = Panel::Comms;
                    self.open_page(Page::Services);
                    return true;
                }
                if speed >= DOCK_SPEED {
                    self.destroyed("Hit the docking bay too fast.");
                    return true;
                }
                self.msg("Docking permission has not been granted.");
                self.push_out(rel, local, o, st_vel, r);
            } else if rel.length() < r * 0.95 && !in_mouth {
                if speed >= DOCK_SPEED {
                    self.destroyed(format!("Collided with {name}."));
                    return true;
                }
                self.push_out(rel, local, o, st_vel, r);
            }
        }
        false
    }

    fn push_out(&mut self, rel: DVec3, _local: DVec3, _o: DMat3, st_vel: DVec3, r: f64) {
        let n = rel.normalize_or_zero();
        self.ship.position += n * (r * 1.2 - rel.length()).max(0.0);
        self.ship.velocity = st_vel + n * 0.01;
    }

    fn destroyed(&mut self, why: impl Into<String>) {
        self.screen = Screen::Destroyed;
        self.title_t = 0.0;
        self.msg(why);
    }

    // ---------------------------------------------------------------- drawing

    pub fn draw(&mut self, cv: &mut Canvas) {
        self.hits.clear();
        match self.screen {
            Screen::Title => {
                self.load_title_frame();
                let frame = self.title_frame.as_ref().map(|f| &f.1);
                let hits = screens::title(cv, &self.a, frame, self.title_t);
                self.hits.extend(hits.into_iter().map(|(r, i)| (r, Action::Title(i))));
                return;
            }
            Screen::Destroyed => {
                cv.clear(palette::BLACK);
                let f = &self.a.font;
                f.draw_centered(cv, "Your ship has been destroyed", W / 2, 140, CONSOLE, palette::HUD_WHITE);
                f.draw_centered(cv, &self.message_text(), W / 2, 170, CONSOLE, palette::CON_GREEN);
                f.draw_centered(cv, "Press Enter", W / 2, 220, CONSOLE, palette::GREY_136);
                return;
            }
            Screen::Play => {}
        }
        cv.reset_clip();
        match self.panel {
            Panel::View => self.draw_view(cv),
            Panel::Comms if self.in_dock() => self.draw_station(cv),
            Panel::Comms => {
                self.draw_view(cv);
                let opts: Vec<String> = self.comms_options().into_iter().map(|o| o.0).collect();
                let rows = screens::flight_comms(cv, &self.a, &opts);
                self.hits.extend(rows.into_iter().map(|(r, i)| (r, Action::Comms(i))));
            }
            Panel::Inventory => {
                let hold = self.hold();
                match self.inv_page {
                    0 => screens::equipment_status(cv, &self.a, &self.fitted, &self.yard, &hold, self.anim_t),
                    1 => screens::profile(cv, &self.a, self.anim_t),
                    _ => {
                        let rows: Vec<RosterRow> = self
                            .contracts
                            .iter()
                            .map(|c| RosterRow { client: c.client.clone(), group: c.group.clone(), dest: c.dest.clone(), pay: c.pay })
                            .collect();
                        screens::roster(cv, &self.a, &rows, &hold, self.anim_t);
                    }
                }
            }
        }
        self.draw_console(cv);
    }

    /// The intro runs behind the title menu at 25 frames a second.
    fn load_title_frame(&mut self) {
        let n = self.a.intro_frames.len();
        if n == 0 {
            return;
        }
        let i = (self.title_t * 25.0) as usize % n;
        if self.title_frame.as_ref().is_some_and(|f| f.0 == i) {
            return;
        }
        if let Ok(img) = image::open(&self.a.intro_frames[i]) {
            self.title_frame = Some((i, img.to_rgba8()));
        }
    }

    fn place_name(&self) -> String {
        match &self.wh {
            Where::Landed { port: Some(id), .. } => {
                self.system().starport_by_id(id).map(|s| s.name.clone()).unwrap_or_default()
            }
            Where::Docked { station } => self
                .system()
                .stations
                .iter()
                .find(|s| &s.id == station)
                .map(|s| s.name.clone())
                .unwrap_or_default(),
            _ => String::new(),
        }
    }

    fn camera(&self) -> DMat3 {
        let o = self.ship.orientation;
        if self.rear {
            DMat3::from_cols(-o.col(0), o.col(1), -o.col(2))
        } else {
            o
        }
    }

    fn draw_view(&self, cv: &mut Canvas) {
        let view = View::new(Rect::new(0, 0, W, VIEW_H));
        cv.set_clip(view.rect);
        let days = self.stardate.days;
        let rot = self.camera();
        let cam_world = self.frames.ship_world_pos(&self.ship, days);
        let sys = self.system();
        let star_rel = self.frames.world_position("star", days) - cam_world;

        // Atmosphere: fade bands out with height, dim them at night.
        let mut sky_fade = 0.0;
        let mut up = DVec3::Y;
        let mut day = 1.0;
        if let Some(p) = self.planet(&self.ship.parent_id) {
            if p.has_atmosphere && !p.is_gas_giant {
                let r = self.ship.position.length();
                up = self.ship.position / r;
                let alt = r - p.radius_km;
                sky_fade = (1.0 - alt / p.atmosphere_km).clamp(0.0, 1.0);
                let sun_el = up.dot(star_rel.normalize());
                day = ((sun_el + 0.1) / 0.3).clamp(0.0, 1.0);
            }
        }
        if sky_fade > 0.0 {
            render3d::draw_sky(cv, &view, &rot, up, &palette::SKY_MERLIN, sky_fade * (0.2 + 0.8 * day));
        } else {
            cv.fill_rect(view.rect, palette::BLACK);
        }
        if sky_fade < 0.6 || day < 0.3 {
            render3d::draw_stars(cv, &view, &rot, &self.stars);
        }

        let mut bodies = vec![SphereBody {
            center: star_rel,
            radius: self.frames.bodies["star"].radius_km,
            color: sys.star_color(),
            sea: None,
            kind: BodyKind::Star,
            seed: sys.seed,
            spin: DMat3::IDENTITY,
            rings: None,
            near: None,
        }];
        let mut occluders = Vec::new();
        for p in &sys.planets {
            let center = self.frames.world_position(&p.id, days) - cam_world;
            let spin = spin_about_y(days, p.rotation_days).transpose();
            let rings = p.has_rings.then(|| Rings {
                // Seen from Sirocco at the start this makes a narrow upright ellipse
                // whose near arc crosses Aster's lower-left limb (PIC002).
                normal: DVec3::new(0.191, -0.793, 0.579).normalize(),
                inner: p.radius_km * 1.4,
                outer: p.radius_km * 1.75,
                colors: palette::RING_ASTER,
            });
            let near = (p.id == self.ship.parent_id && sky_fade > 0.0).then(|| NearGround {
                up,
                ground: palette::DOS_GROUND,
                haze: palette::HAZE_MERLIN,
                day,
            });
            occluders.push((center, p.radius_km));
            bodies.push(SphereBody {
                center,
                radius: p.radius_km,
                color: p.color,
                sea: (p.id == "ross154_merlin").then_some(palette::MERLIN_SEA),
                kind: if p.is_gas_giant { BodyKind::Gas } else { BodyKind::Rocky },
                seed: sys.seed ^ p.radius_km as u32,
                spin,
                rings,
                near,
            });
        }
        render3d::draw_spheres(cv, &view, &rot, &mut bodies, star_rel);

        let mut instances = Vec::new();
        let mut station_orients = Vec::new();
        for st in &sys.stations {
            let pos = self.frames.world_position(&st.id, days) - cam_world;
            if pos.length() < 400.0 {
                station_orients.push((pos, self.station_orient(&st.id, days) * st.radius_km));
            }
        }
        for (pos, orient) in &station_orients {
            instances.push(Instance { mesh: &self.station_mesh, pos: *pos, orient: *orient });
        }
        for p in &sys.planets {
            for sp in &p.starports {
                let pos = self.frames.world_position(&sp.id, days) - cam_world;
                if pos.length() < 60.0 {
                    let n = (self.frames.world_position(&sp.id, days) - self.frames.world_position(&p.id, days)).normalize();
                    instances.push(Instance { mesh: &self.port_mesh, pos, orient: self.port_frame(&sp.id, n) });
                }
            }
        }
        render3d::draw_meshes(cv, &view, &rot, &instances, star_rel, &occluders);

        if self.labels {
            self.draw_labels(cv, &view, &rot, cam_world, &occluders);
        }
        cv.reset_clip();
        if !self.rear {
            self.draw_sights(cv, &view, &rot);
        }
        self.draw_hud(cv);
    }

    fn draw_labels(&self, cv: &mut Canvas, view: &View, rot: &DMat3, cam_world: DVec3, _occ: &[(DVec3, f64)]) {
        let days = self.stardate.days;
        let mut items: Vec<(String, DVec3)> = Vec::new();
        for p in &self.system().planets {
            items.push((p.name.clone(), self.frames.world_position(&p.id, days)));
            for sp in &p.starports {
                items.push((sp.name.clone(), self.frames.world_position(&sp.id, days)));
            }
        }
        for st in &self.system().stations {
            items.push((st.name.clone(), self.frames.world_position(&st.id, days)));
        }
        for (name, w) in items {
            let c = render3d::to_cam(rot, w - cam_world);
            if let Some((x, y)) = view.project(c) {
                let (x, y) = (x as i32, y as i32);
                if view.rect.contains(x, y) {
                    cv.rect(Rect::new(x - 3, y - 3, 7, 7), palette::HUD_WHITE);
                    self.a.font.draw(cv, &name, x + 6, y - 6, CONSOLE, palette::HUD_WHITE);
                }
            }
        }
    }

    /// Gun-sight (the DOS pixel pattern, doubled) and the velocity-vector diagonal cross.
    fn draw_sights(&self, cv: &mut Canvas, view: &View, rot: &DMat3) {
        let (cx, cy) = (W / 2, VIEW_H / 2);
        let mut px = |dx: i32, dy: i32| cv.fill_rect(Rect::new(cx + dx * 2, cy + dy * 2, 2, 2), palette::HUD_WHITE);
        for k in 8..=15 {
            px(0, -k);
            px(0, k);
            px(-k, 0);
            px(k, 0);
        }
        for k in 0..5 {
            px(-14 + k, -14 + k);
            px(13 - k, -14 + k);
            px(-10 - k, 10 + k);
            px(9 + k, 10 + k);
        }
        let rf = self.reference(self.stardate.days);
        let v = self.ship.velocity - rf.vel;
        if v.length() > 0.002 {
            if let Some((x, y)) = view.project(render3d::to_cam(rot, v)) {
                let (x, y) = (x as i32, y as i32);
                for k in 2..6 {
                    for (sx, sy) in [(1, 1), (-1, 1), (1, -1), (-1, -1)] {
                        cv.fill_rect(Rect::new(x + sx * k * 2, y + sy * k * 2, 2, 2), palette::HUD_WHITE);
                    }
                }
            }
        }
    }

    fn draw_hud(&self, cv: &mut Canvas) {
        if matches!(self.wh, Where::Docked { .. }) {
            return;
        }
        let rf = self.reference(self.stardate.days);
        let actual = (self.ship.velocity - rf.vel).length() * 3600.0;
        let actual = if matches!(self.wh, Where::Landed { .. }) { 0.0 } else { actual };
        let set = self.fm.controls.set_speed * 3600.0;
        let alt_m = (self.altitude_km() * 1000.0).max(0.0);
        let alt = if alt_m < 100_000.0 { format!("Alt:{:.0} m", alt_m) } else { format!("Alt:{:.0} km", alt_m / 1000.0) };
        let f = &self.a.font;
        let y = 314;
        let speed = |cv: &mut Canvas, label: &str, v: f64, x: i32| {
            let w = f.draw_shadow(cv, &format!("{label}:{v:.1} kmh"), x, y, CONSOLE, palette::HUD_WHITE);
            f.draw_shadow(cv, "-1", x + w, y - 4, crate::font::SMALL, palette::HUD_WHITE);
        };
        let show_set = !matches!(self.fm.controls.mode, FlightMode::EnginesOff);
        speed(cv, "Actual", actual, 4);
        if show_set {
            speed(cv, "Set", set, 152);
        }
        f.draw_shadow(cv, &alt, 300, y, CONSOLE, palette::HUD_WHITE);
        f.draw_shadow(cv, &format!("Relative to:{}", rf.name), 432, y, CONSOLE, palette::HUD_WHITE);
    }

    fn console_state(&self) -> (Vec<ScanContact>, [Option<&'static str>; 4], &'static str, &'static str) {
        let landed_port = matches!(self.wh, Where::Landed { port: Some(_), .. });
        let f7 = match &self.wh {
            Where::Landed { .. } if landed_port && !self.launch_clearance => Some("F7 Take Off No Clearance.png"),
            Where::Landed { .. } => Some("console_icons/f7_launch.png"),
            Where::Docked { .. } => Some("F7 Empty.png"),
            Where::Flying => Some(match self.fm.controls.mode {
                FlightMode::EnginesOff => "console_icons/f7_manual.png",
                _ => "console_icons/f7_engines_off.png",
            }),
        };
        let f9 = match &self.wh {
            Where::Docked { .. } => "F9 Empty.png",
            _ if self.gear_down => "F9 Landing Gear Up.png",
            _ => "F9 Landing Down.png",
        };
        let f10 = if self.labels { "F10 Text Off.png" } else { "F10 Text On.png" };
        // Off the 3D view the original leaves F7-F10 as blank plates.
        let right = if self.panel == Panel::View {
            [f7, Some("F8 Empty.png"), Some(f9), Some(f10)]
        } else {
            [Some("F7 Empty.png"), Some("F8 Empty.png"), Some("F9 Empty.png"), Some("F10 Empty.png")]
        };
        let upper = match &self.wh {
            Where::Landed { .. } => "Landed",
            Where::Docked { .. } => "Docked",
            Where::Flying => match self.fm.controls.mode {
                FlightMode::TakeOff => "Take Off",
                FlightMode::Manual => "Manual Control",
                FlightMode::EnginesOff => "Engines Off",
            },
        };
        let lower = match self.panel {
            Panel::View if self.rear => "Rear View",
            Panel::View => "Front View",
            Panel::Comms => "Comms Link",
            Panel::Inventory => "Inventory",
        };

        let mut contacts = Vec::new();
        let days = self.stardate.days;
        let world = self.frames.ship_world_pos(&self.ship, days);
        let o = self.ship.orientation;
        for st in &self.system().stations {
            let rel = self.frames.world_position(&st.id, days) - world;
            contacts.push(ScanContact { rel: o.transpose() * rel, color: palette::HUD_WHITE });
        }
        (contacts, right, upper, lower)
    }

    fn draw_console(&mut self, cv: &mut Canvas) {
        let (contacts, right, upper, lower) = self.console_state();
        // F3 shows the page it will turn to next.
        let next_inv = if self.panel == Panel::Inventory { (self.inv_page + 1) % 3 } else { 0 };
        let left = [
            if self.rear { "F1 Rear View.png" } else { "F1 Front View.png" },
            "F2 galactic Map.png",
            ["F3 Ship Status.png", "F3 Captain Status.png", "F3 Cargo Hold.png"][next_inv],
            "F4 Comms.png",
        ];
        let show_scanner = self.panel != Panel::Comms || !self.in_dock();
        let face = if self.message_face { self.a.portraits.get("msg_station") } else { None };
        let st = ConsoleState {
            stardate: self.stardate.format(),
            vcr_active: self.vcr,
            left,
            right,
            upper,
            lower,
            message: &self.message,
            face,
            scanner: show_scanner.then_some(contacts),
        };
        console::draw(cv, &self.a, &st);
        let top = H - CON_H;
        let keys = [
            KeyCode::F1,
            KeyCode::F2,
            KeyCode::F3,
            KeyCode::F4,
            KeyCode::F7,
            KeyCode::F8,
            KeyCode::F9,
            KeyCode::F10,
        ];
        for (i, k) in keys.iter().enumerate() {
            self.hits.push((console::key_rect(i, top), Action::Key(*k)));
        }
        for i in 0..6 {
            self.hits.push((console::vcr_rect(i, top), Action::Vcr(i)));
        }
    }
}
