//! Game state and rules for the vertical slice, following the FE2 manual:
//! start landed at Sirocco Starport on Merlin at 12:00 on 1-Jan-3200, F4 Launch
//! Request, F7 vertical take-off, fly with gravity, then land (clearance, gear F9)
//! or dock at Merlin Orbital (clearance, slow approach into the slot).

mod bulletin;
mod cheat;
mod map;
mod station;
mod traffic;

use crate::assets::Assets;
use crate::console::{self, ConsoleState, ScanContact, CON_H};
use crate::music::Music;
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
use std::collections::{BTreeMap, HashMap, HashSet};
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
    Map,
}

/// F2 cycles these two. F1 leaves the chart and goes back to the view.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum MapPage {
    Galaxy,
    System,
    /// F6 on the galactic chart: the selected system's data.
    Data,
}

#[derive(Clone, Debug)]
pub enum Where {
    /// Sitting on the ground. `local` and `heading` are in the planet's body axes.
    Landed { planet: String, port: Option<String>, local: DVec3, heading: DVec3 },
    Docked { station: String },
    Flying,
}

#[derive(Clone, Debug)]
pub enum Action {
    Key(KeyCode),
    Vcr(usize),
    Comms(usize),
    Title(usize),
    Station(station::Hit),
    /// A star on the galactic chart. Its id is the hyperspace destination.
    Map(String),
    /// A body on the current-system chart.
    Body(String),
    /// A row on the admin cheat menu.
    Cheat(usize),
    /// A hyperspace cloud in the view.
    Cloud(usize),
    /// A body on the system data page.
    Info(String),
    /// A checkbox on the pause options screen.
    Pref(u8),
}

/// The star-streak tunnel. The jump itself is applied when this finishes.
struct JumpAnim {
    t: f64,
    dest: String,
    fuel: i32,
    hours: f64,
}

const JUMP_SECS: f64 = 3.2;
/// How close the Hyperspace Cloud Analyser has to be to read a remnant.
const CLOUD_RANGE_KM: f64 = 80.0;
/// The exit disturbance stays visible for two days.
const CLOUD_LIFE_DAYS: f64 = 2.0;

/// The options screen the original opens when the game is paused.
#[derive(Clone)]
struct Prefs {
    detail: u8,
    sound: bool,
    engine: bool,
    station_sfx: bool,
    wind: bool,
    beeps: bool,
    music: bool,
    music_hyper: bool,
    music_dock: bool,
    music_battle: bool,
    music_continuous: bool,
    dust: bool,
    stars: bool,
    lock_external: bool,
    reverse_lr: bool,
    reverse_ud: bool,
    elite_controls: bool,
    tunnels: bool,
    ship_tunnels: bool,
    extensions: bool,
    track: u8,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            detail: 1,
            sound: true,
            engine: true,
            station_sfx: true,
            wind: true,
            beeps: true,
            music: true,
            music_hyper: true,
            music_dock: true,
            music_battle: false,
            music_continuous: false,
            dust: true,
            stars: true,
            lock_external: false,
            reverse_lr: false,
            reverse_ud: false,
            elite_controls: false,
            tunnels: true,
            ship_tunnels: false,
            extensions: true,
            track: 6,
        }
    }
}

/// Residual disturbance left where a ship came out of hyperspace.
struct Residue {
    system: String,
    /// Position in the star's frame, km.
    pos: DVec3,
    /// System the ship jumped from.
    origin: String,
    /// Stardate of the arrival.
    arrived: f64,
    mass_t: f64,
    expires: f64,
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
    /// F10 in the view: 0 both, 1 names, 2 tunnel, 3 neither.
    hud_mode: u8,
    show_fps: bool,
    fps: f64,
    fps_acc: f64,
    fps_n: u32,
    pub rear: bool,
    /// Scanner-well text: paragraphs in their own colours.
    pub message: Vec<(String, Rgb)>,
    message_t: f64,
    message_face: bool,
    held: HashSet<KeyCode>,
    pub mouse: (i32, i32),
    rmb: bool,
    lmb: bool,
    /// Where the left button went down, so a drag can be told from a click.
    drag_from: (i32, i32),
    dragging: bool,
    mouse_acc: (f64, f64),
    /// Time rate to restore when the pause is lifted.
    vcr_was: usize,
    prefs: Prefs,
    stars: Vec<(DVec3, u8)>,
    station_mesh: Mesh,
    port_mesh: Mesh,
    pub quit: bool,
    pub hits: Vec<(Rect, Action)>,
    /// Heading of each starport's layout about its local vertical, radians from north.
    port_yaw: HashMap<String, f64>,
    /// Hyperspace destination. The system kept in the centre of the galactic chart.
    dest: String,
    map_page: MapPage,
    map_yaw: f64,
    map_pitch: f64,
    map_zoom: f64,
    map_stalks: bool,
    map_grid: bool,
    map_names: bool,
    map_tools: bool,
    /// Seconds until the next arrow-key step, so a held key walks the chart.
    map_repeat: f64,
    /// Light-years the galactic chart is centred on. Separate from the target so
    /// a click can mark a system without sliding the grid under it.
    cam_at: DVec3,
    /// F10 on a chart: the next click places the target.
    arm_target: bool,
    /// Time is stopped. The options screen is not up yet.
    paused: bool,
    /// The options screen is showing.
    pause_menu: bool,
    /// System data page: 0 overview, 1 economy, 2 political.
    data_page: u8,
    /// Body picked on the system data diagram.
    data_body: Option<String>,
    sys_yaw: f64,
    sys_pitch: f64,
    sys_zoom: f64,
    sys_orbits: bool,
    sys_names: bool,
    /// Body the current-system chart is centred on.
    focus: String,
    /// Planet, moon, station or starport locked for the autopilot. The star is not one.
    ap_body: Option<String>,
    /// Playing the hyperspace tunnel. None while in normal flight.
    jump: Option<JumpAnim>,
    /// Hyperspace clouds, kept per system. A jump adds one at the exit.
    clouds: Vec<Residue>,
    /// Remnant the commander clicked. None reads the nearest one in range.
    cloud_sel: Option<usize>,
    cheats_open: bool,
    cheat_sel: usize,
    free_purchases: bool,
    infinite_fuel: bool,
    /// Replaces the drive's rated range while set.
    cheat_jump_ly: Option<f64>,
    kills: i32,
    /// Fines in whole credits, kept per government. A democracy fine is still
    /// due in the next democracy, and is not collected under a different government.
    fines: BTreeMap<String, i64>,
    /// Shots that missed while police could see them. Two brings a warning.
    police_misses: u8,
    police_warned: bool,
    /// The nearest station has already been told about this incident.
    station_told: bool,
    god_mode: bool,
    traffic: Vec<traffic::Npc>,
    spawned_traffic: bool,
    laser_wait: f64,
    traffic_seq: u32,
    music: Music,
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
        let cam_at = galaxy.get("ross154").position_ly;
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
            hud_mode: 0,
            show_fps: false,
            fps: 0.0,
            fps_acc: 0.0,
            fps_n: 0,
            rear: false,
            message: Vec::new(),
            message_t: 0.0,
            message_face: false,
            held: HashSet::new(),
            mouse: (0, 0),
            rmb: false,
            lmb: false,
            drag_from: (0, 0),
            dragging: false,
            mouse_acc: (0.0, 0.0),
            vcr_was: 1,
            prefs: Prefs::default(),
            stars: render3d::starfield(0x5eed_f2, 900),
            station_mesh: meshes::station(1.0),
            port_mesh: meshes::starport(7),
            quit: false,
            hits: Vec::new(),
            port_yaw: HashMap::new(),
            dest: "ross154".into(),
            map_page: MapPage::Galaxy,
            map_yaw: 0.4,
            map_pitch: -0.95,
            map_zoom: 14.0,
            map_stalks: true,
            map_grid: true,
            map_names: true,
            map_tools: false,
            map_repeat: 0.0,
            sys_yaw: 0.35,
            sys_pitch: -0.8,
            sys_zoom: 1.0,
            sys_orbits: true,
            sys_names: true,
            focus: "star".into(),
            ap_body: None,
            jump: None,
            clouds: Vec::new(),
            cloud_sel: None,
            cheats_open: false,
            cheat_sel: 0,
            free_purchases: false,
            infinite_fuel: false,
            cheat_jump_ly: None,
            kills: 0,
            fines: BTreeMap::new(),
            police_misses: 0,
            police_warned: false,
            station_told: false,
            god_mode: false,
            traffic: Vec::new(),
            spawned_traffic: false,
            laser_wait: 0.0,
            traffic_seq: 0,
            cam_at,
            arm_target: false,
            paused: false,
            pause_menu: false,
            data_page: 0,
            data_body: None,
            music: Music::new(),
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
        self.hud_mode = 0;
        self.rear = false;
        self.panel = Panel::View;
        self.dest = "ross154".into();
        self.cam_at = self.galaxy.get("ross154").position_ly;
        self.arm_target = false;
        self.paused = false;
        self.pause_menu = false;
        self.data_page = 0;
        self.data_body = None;
        self.map_page = MapPage::Galaxy;
        self.map_tools = false;
        self.focus = "star".into();
        self.ap_body = None;
        self.jump = None;
        self.clouds.clear();
        self.cloud_sel = None;
        self.cheats_open = false;
        self.cheat_sel = 0;
        self.free_purchases = false;
        self.infinite_fuel = false;
        self.cheat_jump_ly = None;
        self.kills = 0;
        self.fines.clear();
        self.police_misses = 0;
        self.police_warned = false;
        self.station_told = false;
        self.god_mode = false;
        self.traffic.clear();
        self.spawned_traffic = false;
        self.laser_wait = 0.0;
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
            Screen::Play if key == KeyCode::F11 => self.show_fps = !self.show_fps,
            Screen::Play if self.cheats_open => self.cheat_key(key),
            Screen::Play if self.pause_menu => {
                if matches!(key, KeyCode::Escape | KeyCode::F1) {
                    self.resume();
                }
            }
            Screen::Play if self.paused => {
                if key == KeyCode::Escape {
                    self.pause_menu = true;
                }
            }
            Screen::Play if self.jump.is_some() => {
                if matches!(key, KeyCode::Space | KeyCode::Enter | KeyCode::Escape) {
                    self.finish_jump();
                }
            }
            Screen::Play => self.play_key(key, shift),
        }
    }

    pub fn key_up(&mut self, key: KeyCode) {
        self.held.remove(&key);
    }

    pub fn mouse_move(&mut self, x: i32, y: i32) {
        let dx = (x - self.mouse.0) as f64;
        let dy = (y - self.mouse.1) as f64;
        if self.lmb && self.panel == Panel::Map && self.map_page == MapPage::Galaxy && !self.paused {
            if (x - self.drag_from.0).abs() + (y - self.drag_from.1).abs() > 5 {
                self.dragging = true;
            }
            if self.dragging {
                self.pan_chart(dx, dy);
            }
        } else if self.rmb {
            self.mouse_acc.0 += dx;
            self.mouse_acc.1 += dy;
        }
        self.mouse = (x, y);
    }

    pub fn mouse_button(&mut self, left: bool, down: bool) {
        if !left {
            self.rmb = down;
            return;
        }
        if !down {
            let was_drag = self.dragging;
            let on_chart = self.lmb && self.panel == Panel::Map && self.map_page == MapPage::Galaxy;
            self.lmb = false;
            self.dragging = false;
            if on_chart && !was_drag && !self.paused {
                self.click_at(self.mouse);
            }
            return;
        }
        if self.panel == Panel::Map && self.map_page == MapPage::Galaxy && self.mouse.1 < VIEW_H && !self.paused
        {
            self.lmb = true;
            self.drag_from = self.mouse;
            self.dragging = false;
            return;
        }
        self.click_at(self.mouse);
    }

    fn click_at(&mut self, at: (i32, i32)) {
        if self.paused && !self.pause_menu {
            self.pause_menu = true;
            return;
        }
        let hit = self.hits.iter().rev().find(|(r, _)| r.contains(at.0, at.1)).map(|h| h.1.clone());
        match hit {
            Some(Action::Key(k)) => self.key_down(k, false),
            Some(Action::Vcr(0)) => self.pause_icon(),
            Some(Action::Vcr(i)) => self.set_vcr(i),
            Some(Action::Pref(i)) => self.toggle_pref(i),
            Some(Action::Comms(i)) => self.comms_pick(i),
            Some(Action::Title(i)) => self.title_pick(i),
            Some(Action::Station(h)) => self.station_hit(h),
            Some(Action::Map(id)) => {
                if !self.arm_target {
                    self.cam_at = self.galaxy.get(&id).position_ly;
                }
                self.dest = id;
                self.arm_target = false;
            }
            Some(Action::Body(id)) => {
                self.focus = id.clone();
                // The star can be centred, but the autopilot will not fly into it.
                self.ap_body = if id == "star" { None } else { Some(id) };
            }
            Some(Action::Cheat(i)) => self.apply_cheat(i),
            Some(Action::Cloud(i)) => self.cloud_sel = Some(i),
            Some(Action::Info(id)) => self.data_body = Some(id),
            None => {
                if self.panel == Panel::View && self.mouse.1 < VIEW_H {
                    self.cloud_sel = None;
                }
                if self.panel == Panel::Map && self.map_page == MapPage::Data && self.mouse.1 < VIEW_H {
                    self.data_body = None;
                }
            }
        }
    }

    /// First Escape or pause-icon click stops time. The next one opens the options screen.
    fn escape_pressed(&mut self) {
        self.hold_pause();
    }

    fn pause_icon(&mut self) {
        if self.paused {
            self.pause_menu = true;
        } else {
            self.hold_pause();
        }
    }

    fn hold_pause(&mut self) {
        if self.paused {
            return;
        }
        self.vcr_was = self.vcr;
        self.vcr = 0;
        self.paused = true;
    }

    fn resume(&mut self) {
        self.paused = false;
        self.pause_menu = false;
        self.vcr = if self.vcr_was == 0 { 1 } else { self.vcr_was };
    }

    fn show_names(&self) -> bool {
        self.hud_mode == 0 || self.hud_mode == 1
    }

    fn show_tunnel(&self) -> bool {
        self.prefs.tunnels && self.ap_body.is_some() && (self.hud_mode == 0 || self.hud_mode == 2)
    }

    /// Drag the galactic chart. Screen motion follows the grid, so the stars stay under the cursor.
    fn pan_chart(&mut self, dx: f64, dy: f64) {
        let sens = 1.0 / self.map_zoom.max(0.2);
        let (sy, cy) = self.map_yaw.sin_cos();
        let (_, cp) = self.map_pitch.sin_cos();
        self.cam_at.x += (-dx * cy + dy * sy * cp) * sens;
        self.cam_at.z += (-dx * sy - dy * cy * cp) * sens;
    }

    fn toggle_pref(&mut self, i: u8) {
        let p = &mut self.prefs;
        match i {
            0..=3 => p.detail = i,
            4 => p.sound = !p.sound,
            5 => p.engine = !p.engine,
            6 => p.station_sfx = !p.station_sfx,
            7 => p.wind = !p.wind,
            8 => p.beeps = !p.beeps,
            9 => p.music = !p.music,
            10 => p.music_hyper = !p.music_hyper,
            11 => p.music_dock = !p.music_dock,
            12 => p.music_battle = !p.music_battle,
            13 => p.music_continuous = !p.music_continuous,
            14 => p.dust = !p.dust,
            15 => p.stars = !p.stars,
            16 => p.lock_external = !p.lock_external,
            17 => p.reverse_lr = !p.reverse_lr,
            18 => p.reverse_ud = !p.reverse_ud,
            19 => p.elite_controls = !p.elite_controls,
            20 => p.tunnels = !p.tunnels,
            21 => p.ship_tunnels = !p.ship_tunnels,
            22 => p.extensions = !p.extensions,
            23..=32 => p.track = i - 23,
            _ => {}
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

    /// Pause toggles. Any other button selects that Stardreamer rate:
    /// 1x, 10x, 100x, 1000x, 10000x.
    fn set_vcr(&mut self, i: usize) {
        let i = i.min(5);
        self.vcr = if i == 0 {
            if self.vcr == 0 { 1 } else { 0 }
        } else {
            i
        };
    }

    /// Shift+F1 .. Shift+F5, as the manual's time control: 1x through 10000x.
    fn set_stardreamer(&mut self, tier: usize) {
        self.vcr = (tier + 1).min(5);
    }

    /// Wheel up zooms in on whichever chart is open.
    pub fn wheel(&mut self, lines: f64) {
        if self.screen != Screen::Play || self.panel != Panel::Map || lines.abs() < 1e-4 {
            return;
        }
        let factor = 1.2_f64.powf(lines);
        match self.map_page {
            MapPage::Galaxy | MapPage::Data => self.map_zoom = (self.map_zoom * factor).clamp(1.5, 64.0),
            MapPage::System => self.sys_zoom = (self.sys_zoom * factor).clamp(0.15, 80.0),
        }
    }

    pub fn in_dock(&self) -> bool {
        matches!(self.wh, Where::Docked { .. } | Where::Landed { port: Some(_), .. })
    }

    fn play_key(&mut self, key: KeyCode, shift: bool) {
        if shift {
            let tier = match key {
                KeyCode::F1 => Some(0),
                KeyCode::F2 => Some(1),
                KeyCode::F3 => Some(2),
                KeyCode::F4 => Some(3),
                KeyCode::F5 => Some(4),
                _ => None,
            };
            if let Some(t) = tier {
                self.set_stardreamer(t);
                return;
            }
        }
        if matches!(key, KeyCode::Backquote | KeyCode::F12) {
            self.cheats_open = true;
            self.mouse_acc = (0.0, 0.0);
            return;
        }
        match key {
            KeyCode::Escape => self.escape_pressed(),
            KeyCode::F1 => {
                if self.panel == Panel::View {
                    self.rear = !self.rear;
                } else if self.message_t.is_infinite() {
                    self.clear_msg();
                }
                self.map_tools = false;
                self.panel = Panel::View;
            }
            KeyCode::F2 => {
                self.map_tools = false;
                match (self.panel, self.map_page) {
                    (Panel::Map, MapPage::Galaxy) => self.map_page = MapPage::System,
                    (Panel::Map, MapPage::System) => self.map_page = MapPage::Galaxy,
                    _ => {
                        self.map_page = MapPage::Galaxy;
                        self.panel = Panel::Map;
                    }
                }
            }
            KeyCode::F6 if self.panel == Panel::Map && self.map_page == MapPage::Data => {
                self.map_page = MapPage::Galaxy;
            }
            KeyCode::F6 => {
                self.panel = Panel::Map;
                self.map_page = MapPage::Data;
                self.data_page = 0;
                self.data_body = None;
            }
            KeyCode::F7 if self.panel == Panel::Map && self.map_page == MapPage::Data => {
                self.data_page = 1;
            }
            KeyCode::F8 if self.panel == Panel::Map && self.map_page == MapPage::Data => {
                self.data_page = 2;
            }
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
            KeyCode::F7 if self.panel == Panel::Map => self.map_fn(7),
            KeyCode::F7 => self.flight_control(),
            // Tools keep F8. Once a system is targeted, F8 is the hyperspace control
            // on the chart as well as the view. Otherwise the chart zooms.
            KeyCode::F8 if self.panel == Panel::Map && self.map_tools => self.map_fn(8),
            KeyCode::F8 if self.hyperspace_selected() => self.try_jump(),
            KeyCode::F8 if self.panel == Panel::Map => self.map_fn(8),
            KeyCode::F8 if self.panel == Panel::View => self.try_jump(),
            KeyCode::F9 if self.panel == Panel::Map && !self.map_tools => self.map_tools = true,
            KeyCode::F9 if self.panel == Panel::Map => {}
            KeyCode::F9 => {
                if matches!(self.wh, Where::Flying) {
                    self.gear_down = !self.gear_down;
                    self.msg(if self.gear_down { "Undercarriage lowered" } else { "Undercarriage raised" });
                }
            }
            KeyCode::F10 if self.panel == Panel::Map => self.map_fn(10),
            KeyCode::F10 => self.hud_mode = (self.hud_mode + 1) % 4,
            KeyCode::KeyH => self.try_jump(),
            KeyCode::Space if matches!(self.wh, Where::Flying) => self.fire_laser(),
            KeyCode::KeyC if self.panel == Panel::Map && self.map_page == MapPage::Galaxy => {
                self.dest = self.sys_id.clone();
                self.cam_at = self.galaxy.get(&self.dest).position_ly;
            }
            KeyCode::ArrowUp if self.panel == Panel::Map && self.map_page == MapPage::Galaxy => {
                self.step_dest(DVec3::Z);
                self.map_repeat = 0.32;
            }
            KeyCode::ArrowDown if self.panel == Panel::Map && self.map_page == MapPage::Galaxy => {
                self.step_dest(-DVec3::Z);
                self.map_repeat = 0.32;
            }
            KeyCode::ArrowRight if self.panel == Panel::Map && self.map_page == MapPage::Galaxy => {
                self.step_dest(DVec3::X);
                self.map_repeat = 0.32;
            }
            KeyCode::ArrowLeft if self.panel == Panel::Map && self.map_page == MapPage::Galaxy => {
                self.step_dest(-DVec3::X);
                self.map_repeat = 0.32;
            }
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

    /// Drive stats, with the admin range override applied.
    fn flight_stats(&self) -> frontier_sim::flight::ShipStats {
        let mut stats = self.yard.stats(&self.fitted);
        if let Some(ly) = self.cheat_jump_ly {
            stats.jump_range_ly = ly;
        }
        stats
    }

    /// A different system is targeted, a drive is fitted, and the ship is in space.
    /// The icon stays hidden on the ground and in a docking bay.
    fn hyperspace_selected(&self) -> bool {
        matches!(self.wh, Where::Flying)
            && self.dest != self.sys_id
            && self.flight_stats().drive_h > 0.0
    }

    /// The manual puts Autopilot in the F7 cycle only with the unit fitted and a target locked.
    fn offer_autopilot(&self) -> bool {
        matches!(self.wh, Where::Flying)
            && self.fitted.has("autopilot")
            && (self.ap_body.is_some() || self.dest != self.sys_id)
    }

    /// F7, following the icon: take off when landed. In flight the cycle is
    /// Manual, Engines Off, and Autopilot once a target is locked.
    fn flight_control(&mut self) {
        match self.wh.clone() {
            Where::Landed { port, .. } => {
                if port.is_some() && !self.launch_clearance {
                    self.msg("Launch clearance has not been granted.");
                    if self.enforces_law() {
                        self.add_fine(400);
                    }
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
                let offer = self.offer_autopilot();
                let prev = self.fm.controls.mode;
                let next = match prev {
                    FlightMode::Autopilot => FlightMode::EnginesOff,
                    FlightMode::EnginesOff => FlightMode::Manual,
                    FlightMode::Manual | FlightMode::TakeOff if offer => FlightMode::Autopilot,
                    FlightMode::Manual | FlightMode::TakeOff => FlightMode::EnginesOff,
                };
                if prev == FlightMode::EnginesOff {
                    let rf = self.reference(self.stardate.days);
                    let v = self.ship.velocity - rf.vel;
                    self.fm.controls.set_speed = v.dot(self.ship.orientation.col(2));
                }
                // A galactic target has no place to fly to. Autopilot makes the jump.
                if next == FlightMode::Autopilot && self.ap_body.is_none() {
                    self.try_jump();
                    if self.dest != self.sys_id {
                        self.fm.controls.mode = FlightMode::Manual;
                    }
                    return;
                }
                if next == FlightMode::EnginesOff {
                    self.fm.controls.thrust = 0.0;
                }
                self.fm.controls.mode = next;
            }
        }
    }

    /// Point the nose at the locked body and cruise, holding short of the surface.
    fn steer_autopilot(&mut self) {
        let Some(id) = self.ap_body.clone() else {
            self.fm.controls.mode = FlightMode::Manual;
            return;
        };
        let Some(body) = self.frames.bodies.get(&id).cloned() else {
            self.ap_body = None;
            self.fm.controls.mode = FlightMode::Manual;
            return;
        };
        let days = self.stardate.days;
        let me = self.frames.ship_world_pos(&self.ship, days);
        let place = self.frames.world_position(&id, days);
        let (aim, hold) = if body.is_starport {
            let up = body
                .parent_id
                .as_deref()
                .map(|p| place - self.frames.world_position(p, days))
                .and_then(|v| v.try_normalize())
                .unwrap_or(DVec3::Y);
            (place + up * 20.0, 2.0)
        } else if body.is_station {
            (place, (body.radius_km * 4.0).max(8.0))
        } else {
            (place, body.radius_km + 80.0)
        };
        let delta = aim - me;
        let dist = delta.length();
        if dist < hold {
            let c = &mut self.fm.controls;
            c.pitch = 0.0;
            c.yaw = 0.0;
            c.roll = 0.0;
            c.set_speed = 0.0;
            return;
        }
        let (pitch, yaw, roll, align) = steer_toward(self.ship.orientation, delta);
        let cruise = (dist * 0.002).clamp(0.05, 8.0);
        let c = &mut self.fm.controls;
        c.pitch = pitch;
        c.yaw = yaw;
        c.roll = roll;
        c.set_speed = if align > 0.85 { cruise } else { 0.0 };
        if (body.is_station || body.is_starport) && dist < 80.0 {
            self.gear_down = true;
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
        self.note_frame(dt);
        self.anim_t += dt;
        self.sync_music();
        match self.screen {
            Screen::Title | Screen::Destroyed => {
                self.title_t += dt;
                return;
            }
            Screen::Play => {}
        }
        if self.cheats_open || self.paused {
            self.mouse_acc = (0.0, 0.0);
            return;
        }
        if self.jump.is_some() {
            let done = {
                let jump = self.jump.as_mut().unwrap();
                jump.t += dt;
                jump.t >= JUMP_SECS
            };
            if done {
                self.finish_jump();
            }
            return;
        }
        self.clouds.retain(|c| self.stardate.days <= c.expires);
        if self.cloud_sel.is_some_and(|i| i >= self.clouds.len()) {
            self.cloud_sel = None;
        }
        if self.message_t > 0.0 {
            self.message_t -= dt;
            if self.message_t <= 0.0 {
                self.clear_msg();
            }
        }
        if self.panel == Panel::Map {
            let (mx, my) = std::mem::take(&mut self.mouse_acc);
            if self.map_page == MapPage::Galaxy {
                self.map_yaw += mx * 0.008;
                self.map_pitch = (self.map_pitch + my * 0.008).clamp(-1.4, 1.4);
                self.walk_chart(dt);
            } else {
                self.sys_yaw += mx * 0.008;
                self.sys_pitch = (self.sys_pitch + my * 0.008).clamp(-1.4, 1.4);
            }
        }
        self.read_controls(dt);
        if self.fm.controls.mode == FlightMode::Autopilot && matches!(self.wh, Where::Flying) {
            self.steer_autopilot();
        }

        // The on-board computer returns to real time on the deck or in a station's slot.
        // The chart keeps whatever rate was selected, so the orrery can be run forward.
        if self.panel != Panel::Map && matches!(self.wh, Where::Flying) && self.vcr > 1 {
            let days = self.stardate.days;
            let world = self.frames.ship_world_pos(&self.ship, days);
            let near_station = self.nearest_station(world, days).is_some_and(|s| s.2 < 6.0);
            if near_station || self.altitude_km() < 0.4 {
                self.vcr = 1;
            }
        }
        let sim_dt = dt * VCR_FACTORS[self.vcr];
        if sim_dt <= 0.0 {
            return;
        }
        match self.wh {
            Where::Flying => {
                self.ensure_traffic();
                let steps = ((sim_dt / 0.1).ceil() as usize).clamp(1, 4000);
                let h = sim_dt / steps as f64;
                for _ in 0..steps {
                    self.stardate.days += h / 86400.0;
                    if !self.step_flight(h) {
                        break;
                    }
                    self.step_traffic(h);
                    if self.screen != Screen::Play {
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
        let my = if self.prefs.reverse_ud { -my } else { my };
        let mx = if self.prefs.reverse_lr { -mx } else { mx };
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

    fn draw_pause(&mut self, cv: &mut Canvas) {
        cv.clear([40, 40, 40]);
        let p = self.prefs.clone();
        let opt = crate::font::SMALL;
        self.a.font.draw(cv, "Level", 16, 10, opt, palette::HUD_WHITE);
        self.a.font.draw(cv, "Shape Detail:", 78, 10, opt, palette::CON_GREEN);
        let levels = ["Low", "Medium", "High", "Very High"];
        for (i, name) in levels.iter().enumerate() {
            self.pref_box(cv, 16, 36 + i as i32 * 22, p.detail == i as u8, i as u8);
            self.a.font.draw(cv, name, 36, 34 + i as i32 * 22, opt, palette::HUD_WHITE);
        }
        self.a.font.draw(cv, "Sounds:", 250, 10, opt, palette::CON_GREEN);
        let sounds = [
            (4u8, "Sound effects on", p.sound),
            (5, "Engine sound", p.engine),
            (6, "Space station", p.station_sfx),
            (7, "Wind noise", p.wind),
            (8, "Icon beeps", p.beeps),
        ];
        for (i, (id, name, on)) in sounds.iter().enumerate() {
            self.pref_box(cv, 250, 36 + i as i32 * 22, *on, *id);
            self.a.font.draw(cv, name, 270, 34 + i as i32 * 22, opt, palette::HUD_WHITE);
        }
        self.a.font.draw(cv, "Music:", 470, 10, opt, palette::CON_GREEN);
        let music = [
            (9u8, "Music on", p.music),
            (10, "Music after hyperspace", p.music_hyper),
            (11, "Docking music", p.music_dock),
            (12, "Music in battles", p.music_battle),
            (13, "Continuous music", p.music_continuous),
        ];
        for (i, (id, name, on)) in music.iter().enumerate() {
            self.pref_box(cv, 470, 36 + i as i32 * 22, *on, *id);
            self.a.font.draw(cv, name, 490, 34 + i as i32 * 22, opt, palette::HUD_WHITE);
        }
        self.a.font.draw(cv, "Other Options:", 16, 168, opt, palette::CON_GREEN);
        let other = [
            (14u8, "Space dust, clouds etc.", p.dust),
            (15, "Background stars", p.stars),
            (16, "Lock external view to ship", p.lock_external),
            (17, "Reverse left-right controls", p.reverse_lr),
            (18, "Reverse up-down controls", p.reverse_ud),
            (19, "Elite control method in space", p.elite_controls),
            (20, "Show targeting tunnels", p.tunnels),
            (21, "Allow targeting tunnels for ships", p.ship_tunnels),
            (22, "Use filename extensions", p.extensions),
        ];
        for (i, (id, name, on)) in other.iter().enumerate() {
            self.pref_box(cv, 16, 194 + i as i32 * 20, *on, *id);
            self.a.font.draw(cv, name, 36, 192 + i as i32 * 20, opt, palette::HUD_WHITE);
        }
        self.a.font.draw(cv, "Choose Music From:", 300, 168, opt, palette::CON_GREEN);
        let tracks = [
            "Greig - In the Hall of the Mountain King",
            "Lowe - Frontier Main Theme",
            "Mussorgsky - Baba Yaga",
            "Mussorgsky - Night on the Bare Mountain",
            "Lowe - Frontier Second Theme",
            "Wagner - Ride of the Valkyries",
            "Strauss - Blue Danube",
            "Mussorgsky - Great Gates of Kiev",
        ];
        for (i, name) in tracks.iter().enumerate() {
            self.pref_box(cv, 300, 194 + i as i32 * 20, p.track == i as u8, 23 + i as u8);
            self.a.font.draw(cv, name, 320, 192 + i as i32 * 20, opt, palette::HUD_WHITE);
        }
    }

    fn pref_box(&mut self, cv: &mut Canvas, x: i32, y: i32, on: bool, id: u8) {
        let r = Rect::new(x, y, 12, 12);
        cv.rect(r, palette::BLACK);
        if on {
            cv.fill_rect(Rect::new(x + 2, y + 2, 8, 8), palette::ROSTER_RED);
        } else {
            cv.fill_rect(Rect::new(x + 2, y + 2, 8, 8), palette::GREY_102);
        }
        self.hits.push((Rect::new(x - 2, y - 2, 16, 16), Action::Pref(id)));
    }

    fn note_frame(&mut self, dt: f64) {
        if !(0.0..1.0).contains(&dt) {
            return;
        }
        self.fps_acc += dt;
        self.fps_n += 1;
        if self.fps_acc >= 0.3 {
            self.fps = self.fps_n as f64 / self.fps_acc;
            self.fps_acc = 0.0;
            self.fps_n = 0;
        }
    }

    fn paint_fps(&self, cv: &mut Canvas) {
        if !self.show_fps {
            return;
        }
        let text = format!("FPS {:.0}", self.fps);
        self.a.font.draw(cv, &text, 4, 4, crate::font::SMALL, palette::CON_GREEN);
    }

    pub fn draw(&mut self, cv: &mut Canvas) {
        self.draw_frame(cv);
        self.paint_fps(cv);
    }

    fn draw_frame(&mut self, cv: &mut Canvas) {
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
        if self.pause_menu {
            self.draw_pause(cv);
            return;
        }
        if self.cheats_open {
            self.draw_cheats(cv);
            return;
        }
        if self.jump.is_some() {
            let t = self.jump.as_ref().map(|j| j.t).unwrap_or(0.0);
            draw_hyperspace(cv, t / JUMP_SECS);
            self.draw_console(cv);
            return;
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
            Panel::Map => self.draw_map(cv),
            Panel::Inventory => {
                let hold = self.hold();
                match self.inv_page {
                    0 => screens::equipment_status(cv, &self.a, &self.fitted, &self.yard, &hold, self.anim_t),
                    1 => screens::profile(
                        cv,
                        &self.a,
                        self.anim_t,
                        frontier_sim::frontier::combat_rank(self.kills),
                        !self.wanted_here(),
                    ),
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

    /// The remnant the fitted analyser is reading: the one clicked, else the nearest in range.
    fn cloud_index(&self) -> Option<usize> {
        if !self.fitted.has("cloud_analyser") {
            return None;
        }
        let days = self.stardate.days;
        let here = self.sys_id.clone();
        let me = self.frames.ship_world_pos(&self.ship, days);
        let star = self.frames.world_position("star", days);
        let near = |c: &Residue| {
            if c.system != here || days > c.expires {
                return None;
            }
            let d = (star + c.pos - me).length();
            (d < CLOUD_RANGE_KM).then_some(d)
        };
        if let Some(i) = self.cloud_sel {
            if self.clouds.get(i).and_then(near).is_some() {
                return Some(i);
            }
        }
        self.clouds
            .iter()
            .enumerate()
            .filter_map(|(i, c)| near(c).map(|d| (d, i)))
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap())
            .map(|(_, i)| i)
    }

    /// Top-right of the view: where the ship came from, and when it arrived.
    fn draw_cloud_readout(&self, cv: &mut Canvas) {
        let Some(c) = self.cloud_index().and_then(|i| self.clouds.get(i)) else {
            return;
        };
        let lines = [
            c.origin.clone(),
            Stardate::new(c.arrived).format(),
            format!("{:.0}t", c.mass_t),
        ];
        for (i, line) in lines.iter().enumerate() {
            let w = self.a.font.width(line, crate::font::SMALL);
            self.a.font.draw(
                cv,
                line,
                W - 8 - w,
                8 + i as i32 * 16,
                crate::font::SMALL,
                palette::HUD_WHITE,
            );
        }
    }

    fn draw_view(&mut self, cv: &mut Canvas) {
        let view = View::new(Rect::new(0, 0, W, VIEW_H));
        cv.set_clip(view.rect);
        let days = self.stardate.days;
        let rot = self.camera();
        let cam_world = self.frames.ship_world_pos(&self.ship, days);
        let star_w = self.frames.world_position("star", days);
        let here = self.sys_id.clone();
        let residues: Vec<(usize, DVec3)> = self
            .clouds
            .iter()
            .enumerate()
            .filter(|(_, c)| c.system == here && days <= c.expires)
            .map(|(i, c)| (i, star_w + c.pos))
            .collect();
        let sys = self.system();
        let star_rel = star_w - cam_world;

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
        if self.prefs.stars && (sky_fade < 0.6 || day < 0.3) {
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
        for (_, w) in &residues {
            bodies.push(SphereBody {
                center: *w - cam_world,
                radius: 18.0,
                color: [255, 214, 255],
                sea: None,
                kind: BodyKind::Star,
                seed: 1,
                spin: DMat3::IDENTITY,
                rings: None,
                near: None,
            });
        }
        render3d::draw_spheres(cv, &view, &rot, &mut bodies, star_rel);

        let mut ship_meshes = Vec::new();
        for npc in self.traffic.iter().filter(|n| n.alive) {
            let pos = self.frames.ship_world_pos(&npc.state, days) - cam_world;
            if pos.length() < 250.0 {
                let colour = match npc.role {
                    "pirate" => [[180, 40, 40], [140, 30, 30], [80, 16, 16]],
                    "police" => [[40, 80, 180], [30, 60, 140], [16, 30, 80]],
                    _ => [[160, 160, 160], [110, 110, 110], [60, 60, 60]],
                };
                ship_meshes.push((meshes::eagle_in(colour), pos, npc.state.orientation * 0.04));
            }
        }

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
        for (mesh, pos, orient) in &ship_meshes {
            instances.push(Instance { mesh, pos: *pos, orient: *orient });
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

        if self.show_names() {
            self.draw_labels(cv, &view, &rot, cam_world, &occluders);
        }
        if self.show_tunnel() {
            self.draw_tunnel(cv, &view, &rot, cam_world);
        }
        for (i, w) in &residues {
            let cam = render3d::to_cam(&rot, *w - cam_world);
            if let Some((x, y)) = view.project(cam) {
                let (x, y) = (x as i32, y as i32);
                if view.rect.contains(x, y) {
                    self.hits.push((Rect::new(x - 10, y - 10, 20, 20), Action::Cloud(*i)));
                    if self.show_names() {
                        self.a.font.draw(cv, "Hyperspace cloud", x + 8, y - 6, crate::font::SMALL, palette::HUD_WHITE);
                    }
                }
            }
        }
        self.draw_cloud_readout(cv);
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

    /// Nested squares along the path to the locked station or planet, with range and ETA.
    fn draw_tunnel(&self, cv: &mut Canvas, view: &View, rot: &DMat3, cam_world: DVec3) {
        let Some(id) = self.ap_body.clone() else { return };
        let days = self.stardate.days;
        let rel = self.frames.world_position(&id, days) - cam_world;
        let dist = rel.length();
        if dist < 0.05 {
            return;
        }
        let dir = rel / dist;
        let cam = render3d::to_cam(rot, rel);
        let range = if dist >= 15_000_000.0 {
            format!("{:.2} AU", dist / 149_597_870.7)
        } else if dist >= 100.0 {
            format!("{:.0} km", dist)
        } else {
            format!("{:.1} km", dist)
        };
        let closing = self.frames.ship_world_vel(&self.ship, days).dot(dir);
        let eta = if closing > 0.02 {
            let secs = dist / closing;
            if secs < 90.0 {
                format!("ETA {secs:.0}s")
            } else if secs < 3600.0 {
                format!("ETA {:.0}m", secs / 60.0)
            } else if secs < 86_400.0 {
                format!("ETA {}:{:02}", (secs / 3600.0) as i32, ((secs % 3600.0) / 60.0) as i32)
            } else {
                format!("ETA {:.1}d", secs / 86_400.0)
            }
        } else {
            "ETA --".into()
        };
        if cam.z <= 0.05 {
            self.a.font.draw_centered(cv, "Target Behind", W / 2, 80, CONSOLE, palette::CON_GREEN);
            self.a.font.draw_centered(cv, &range, W / 2, 104, CONSOLE, palette::CON_GREEN);
            return;
        }
        let right = dir.cross(rot.col(1)).try_normalize().unwrap_or(rot.col(0));
        let up = right.cross(dir);
        let green = palette::CON_GREEN;
        for i in 1..=7 {
            let d = dist * (i as f64 / 8.0);
            let half = (d * 0.11).clamp(0.15, dist * 0.2);
            let c = dir * d;
            let corners = [
                c + (right + up) * half,
                c + (-right + up) * half,
                c + (-right - up) * half,
                c + (right - up) * half,
            ];
            let pts: Vec<Option<(i32, i32)>> = corners
                .iter()
                .map(|p| {
                    view.project(render3d::to_cam(rot, *p)).map(|(x, y)| (x as i32, y as i32))
                })
                .collect();
            if pts.iter().all(Option::is_some) {
                for k in 0..4 {
                    let a = pts[k].unwrap();
                    let b = pts[(k + 1) % 4].unwrap();
                    cv.line(a.0, a.1, b.0, b.1, green);
                }
            }
        }
        if let Some((x, y)) = view.project(cam) {
            let (x, y) = (x as i32, y as i32);
            let inside = view.rect.contains(x, y);
            let (tx, ty) = if inside {
                (x, y)
            } else {
                let cx = view.rect.x + view.rect.w / 2;
                let cy = view.rect.y + view.rect.h / 2;
                let (dx, dy) = ((x - cx) as f64, (y - cy) as f64);
                let scale = (view.rect.w as f64 * 0.45) / dx.abs().max(dy.abs()).max(1.0);
                (cx + (dx * scale) as i32, cy + (dy * scale) as i32)
            };
            if !inside {
                cv.line(W / 2, VIEW_H / 2, tx, ty, green);
            }
            self.a.font.draw(cv, &range, tx + 10, ty - 8, CONSOLE, green);
            self.a.font.draw(cv, &eta, tx + 10, ty + 12, CONSOLE, green);
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
                FlightMode::Autopilot => "console_icons/f7_engines_off.png",
                FlightMode::Manual | FlightMode::TakeOff if self.offer_autopilot() => {
                    "console_icons/f7_autopilot.png"
                }
                FlightMode::Manual | FlightMode::TakeOff => "console_icons/f7_engines_off.png",
            }),
        };
        let f9 = match &self.wh {
            Where::Docked { .. } => "F9 Empty.png",
            _ if self.gear_down => "F9 Landing Gear Up.png",
            _ => "F9 Landing Down.png",
        };
        let f10 = if self.show_names() { "F10 Text Off.png" } else { "F10 Text On.png" };
        // Off the 3D view the original leaves F7-F10 as blank plates.
        let hyper = self.hyperspace_selected().then_some("console_icons/f8_hyperspace.png");
        let right = match self.panel {
            Panel::View => [f7, hyper, Some(f9), Some(f10)],
            Panel::Map if self.map_page == MapPage::Galaxy && self.map_tools => [
                Some("F7 Settings.png"),
                Some("F8 Settings.png"),
                Some("F9 Settings.png"),
                Some("F10 Settings.png"),
            ],
            Panel::Map if self.map_page == MapPage::Galaxy => [
                Some("F7 Galatic Map.png"),
                Some(hyper.unwrap_or("F8 Galactic Map.png")),
                Some("F9 Galactic Map.png"),
                Some("F10 Galactic Map.png"),
            ],
            Panel::Map if self.map_tools => [
                Some("F7 Empty.png"),
                Some("F8 Settings.png"),
                Some("F9 Empty.png"),
                Some("F10 Settings.png"),
            ],
            Panel::Map => [
                Some("F7 Map.png"),
                Some(hyper.unwrap_or("F8 Map.png")),
                Some("F9 Map.png"),
                Some("F10 Map.png"),
            ],
            _ => [Some("F7 Empty.png"), Some("F8 Empty.png"), Some("F9 Empty.png"), Some("F10 Empty.png")],
        };
        let upper = match &self.wh {
            Where::Landed { .. } => "Landed",
            Where::Docked { .. } => "Docked",
            Where::Flying => match self.fm.controls.mode {
                FlightMode::TakeOff => "Take Off",
                FlightMode::Manual => "Manual Control",
                FlightMode::EnginesOff => "Engines Off",
                FlightMode::Autopilot => "Autopilot",
            },
        };
        let lower = match self.panel {
            Panel::View if self.rear => "Rear View",
            Panel::View => "Front View",
            Panel::Comms => "Comms Link",
            Panel::Inventory => "Inventory",
            Panel::Map if self.map_page == MapPage::System => "System Map",
            Panel::Map if self.map_page == MapPage::Data => "System Data",
            Panel::Map => "Galactic Map",
        };

        let mut contacts = Vec::new();
        let days = self.stardate.days;
        let world = self.frames.ship_world_pos(&self.ship, days);
        let o = self.ship.orientation;
        for st in &self.system().stations {
            let rel = self.frames.world_position(&st.id, days) - world;
            contacts.push(ScanContact { rel: o.transpose() * rel, color: palette::HUD_WHITE });
        }
        for npc in self.traffic.iter().filter(|n| n.alive) {
            let rel = self.frames.ship_world_pos(&npc.state, days) - world;
            let color = match npc.role {
                "pirate" => palette::ROSTER_RED,
                "police" => [80, 140, 255],
                _ => palette::CON_GREEN,
            };
            contacts.push(ScanContact { rel: o.transpose() * rel, color });
        }
        (contacts, right, upper, lower)
    }

    fn draw_console(&mut self, cv: &mut Canvas) {
        let (contacts, right, upper, lower) = self.console_state();
        // F3 shows the page it will turn to next.
        let next_inv = if self.panel == Panel::Inventory { (self.inv_page + 1) % 3 } else { 0 };
        let f2 = match (self.panel, self.map_page) {
            (Panel::Map, MapPage::Galaxy) | (Panel::Map, MapPage::Data) => "F2 Map.png",
            (Panel::Map, MapPage::System) => "F2 galactic Map.png",
            _ => "F2 galactic Map.png",
        };
        let left = [
            if self.rear { "F1 Rear View.png" } else { "F1 Front View.png" },
            f2,
            ["F3 Ship Status.png", "F3 Captain Status.png", "F3 Cargo Hold.png"][next_inv],
            "F4 Comms.png",
        ];
        let show_scanner = self.panel != Panel::Map && (self.panel != Panel::Comms || !self.in_dock());
        let face = if self.message_face { self.a.portraits.get("msg_station") } else { None };
        let map_lines = (self.panel == Panel::Map).then(|| self.map_lines());
        let message = map_lines.as_deref().unwrap_or(&self.message);
        let st = ConsoleState {
            stardate: self.stardate.format(),
            vcr_active: self.vcr,
            left,
            right,
            upper,
            lower,
            message,
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

/// Pitch, yaw, roll and how squarely the nose faces `desired` (world axes).
fn steer_toward(orientation: DMat3, desired: DVec3) -> (f64, f64, f64, f64) {
    let n = desired.length();
    if n < 1e-6 {
        return (0.0, 0.0, 0.0, 1.0);
    }
    let body = orientation.transpose() * (desired / n);
    let (bx, by, bz) = (body.x, body.y, body.z);
    let yaw_err = bx.atan2(bz);
    let mut pitch_err = by.atan2(if bz > -0.05 { bx.hypot(bz) } else { bz.max(0.01) });
    if bz < 0.0 {
        pitch_err *= (bz + 1.0).max(0.0);
    }
    let gain = 0.85;
    let mut yaw = (yaw_err * gain).clamp(-1.0, 1.0);
    let mut pitch = (pitch_err * gain).clamp(-1.0, 1.0);
    if yaw_err.abs() < 0.03 {
        yaw = 0.0;
    }
    if pitch_err.abs() < 0.03 {
        pitch = 0.0;
    }
    let roll = if bz > 0.85 {
        let up_err = (orientation.transpose() * DVec3::Y).x;
        (-up_err * 0.4).clamp(-0.25, 0.25)
    } else {
        0.0
    };
    (pitch, yaw, roll, bz.clamp(0.0, 1.0))
}

/// Star streaks rushing out of a tunnel of squares, then a white flash.
fn draw_hyperspace(cv: &mut Canvas, u: f64) {
    let u = u.clamp(0.0, 1.0);
    let (w, h) = (W, VIEW_H);
    let (cx, cy) = (w / 2, h / 2);
    cv.set_clip(Rect::new(0, 0, w, h));
    cv.fill_rect(Rect::new(0, 0, w, h), [4, 8, 28]);
    let streak = (u * 2.2).min(1.0);
    let tunnel = ((u - 0.15) / 0.55).clamp(0.0, 1.0);
    let flash = ((u - 0.78) / 0.22).clamp(0.0, 1.0);
    let mut seed = 0xFE2A11u32;
    let mut rnd = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) as f64 / 16_777_216.0
    };
    for _ in 0..90 {
        let ang = rnd() * std::f64::consts::TAU;
        let r0 = 4.0 + rnd() * 30.0;
        let speed = 180.0 + rnd() * 420.0;
        let jitter = rnd();
        let r1 = r0 + streak * speed * (0.35 + rnd());
        let r2 = r1 + 8.0 + streak * 40.0 * rnd();
        let col = [(180.0 + 75.0 * rnd()) as u8, (180.0 + 75.0 * jitter) as u8, 255];
        let (s, c) = ang.sin_cos();
        if r1 < w.max(h) as f64 {
            cv.line(
                (cx as f64 + c * r1) as i32,
                (cy as f64 + s * r1 * 0.72) as i32,
                (cx as f64 + c * r2) as i32,
                (cy as f64 + s * r2 * 0.72) as i32,
                col,
            );
        }
    }
    if tunnel > 0.0 {
        for i in 0..14 {
            let phase = (i as f64 / 14.0 + tunnel * 2.8).fract();
            let scale = 0.04 + phase * phase * 1.35;
            let half_w = (w as f64 * 0.48 * scale) as i32;
            let half_h = (h as f64 * 0.42 * scale) as i32;
            if half_w < 2 || half_h < 2 {
                continue;
            }
            let boost = (40.0 + 160.0 * (1.0 - phase)) as i32;
            let col = [(40 + boost).min(255) as u8, (120 + boost / 2).min(255) as u8, 255];
            cv.rect(Rect::new(cx - half_w, cy - half_h, half_w * 2, half_h * 2), col);
            let tick = (half_w / 10).max(2);
            for (ox, oy) in [(-1, -1), (1, -1), (-1, 1), (1, 1)] {
                let px = cx + ox * half_w;
                let py = cy + oy * half_h;
                cv.line(px, py, px - ox * tick, py, col);
                cv.line(px, py, px, py - oy * tick, col);
            }
        }
    }
    if flash > 0.35 {
        let k = if flash < 0.5 { flash * 2.0 } else { (1.0 - flash) * 2.0 };
        let v = (255.0 * k) as u8;
        cv.fill_rect(Rect::new(0, 0, w, h), [v, v, v]);
    }
    cv.reset_clip();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shift_f3_runs_the_clock_at_one_hundred() {
        let mut game = Game::new(crate::assets::Assets::load().expect("assets")).expect("game");
        game.new_game();
        let t0 = game.stardate.days;
        game.key_down(KeyCode::F3, true);
        game.update(10.0);
        let advanced = game.stardate.days - t0;
        assert!(
            advanced > 0.01 && advanced < 0.013,
            "clock moved {advanced} days, expected about 0.0116 at 100x"
        );
    }

    #[test]
    fn a_locked_system_offers_hyperspace_and_autopilot() {
        let mut game = Game::new(crate::assets::Assets::load().expect("assets")).expect("game");
        game.new_game();
        game.dest = "epsilon_eri".into();
        let (_, right, _, _) = game.console_state();
        assert_eq!(right[1], None, "hyperspace stays hidden on the pad");

        game.wh = Where::Flying;
        let (_, right, _, _) = game.console_state();
        assert_eq!(right[1], Some("console_icons/f8_hyperspace.png"));
        assert_eq!(right[0], Some("console_icons/f7_autopilot.png"));

        game.cheat_jump_ly = Some(80.0);
        game.fitted.cargo.insert("hydrogen_fuel".into(), 30);
        game.flight_control();
        assert!(game.jump.is_some(), "the jump plays the tunnel before arriving");
        assert_eq!(game.sys_id, "ross154");
        game.update(JUMP_SECS);
        assert_eq!(game.sys_id, "epsilon_eri");
        assert!(game.jump.is_none());
        assert!(matches!(game.fm.controls.mode, FlightMode::Manual));

        let day = game.stardate.days;
        game.key_down(KeyCode::Escape, false);
        assert!(game.paused);
        assert!(!game.pause_menu, "the first Escape only stops time");
        game.mouse_button(true, true);
        assert!(game.pause_menu);
        assert_eq!(game.screen, Screen::Play);
        game.update(5.0);
        assert!((game.stardate.days - day).abs() < 1e-9);
        game.key_down(KeyCode::F1, false);
        assert!(!game.paused);

        game.key_down(KeyCode::F6, false);
        assert_eq!(game.map_page, MapPage::Data);
        assert!(game.galaxy.systems.len() > 400);
        assert!(game.galaxy.systems.contains_key("achenar"));

        game.paused = false;
        game.panel = Panel::Map;
        game.map_page = MapPage::System;
        let station = game.system().stations[0].id.clone();
        game.hits.push((Rect::new(0, 0, 8, 8), Action::Body(station.clone())));
        game.mouse = (2, 2);
        game.mouse_button(true, true);
        assert_eq!(game.ap_body.as_deref(), Some(station.as_str()));
        assert_eq!(game.clouds.len(), 1, "the jump leaves residue at the exit");
        assert_eq!(game.clouds[0].origin, "Ross 154");
        assert_eq!(game.clouds[0].system, "epsilon_eri");
        assert!((game.clouds[0].arrived - game.stardate.days).abs() < 1e-6);
        assert!(game.cloud_index().is_none(), "the analyser has to be fitted");
        game.fitted.equipment.push("cloud_analyser".into());
        let read = game.cloud_index().expect("the exit cloud is in range");
        assert_eq!(game.clouds[read].origin, "Ross 154");
    }

    #[test]
    fn democracy_keeps_pirates_away_until_you_are_wanted() {
        let mut game = Game::new(crate::assets::Assets::load().expect("assets")).expect("game");
        game.new_game();
        game.wh = Where::Flying;
        game.ensure_traffic();
        assert!(game.traffic.iter().any(|s| s.role == "police"));
        assert!(game.traffic.iter().all(|s| s.role != "pirate"));
        game.add_fine(10_000);
        game.traffic.clear();
        game.spawned_traffic = false;
        game.ensure_traffic();
        assert!(game.traffic.iter().any(|s| s.role == "pirate"));
        let before = game.kills;
        game.fire_laser();
        assert!(game.kills >= before);
    }

    #[test]
    fn a_fine_needs_a_witness_and_stays_with_that_government() {
        let mut game = Game::new(crate::assets::Assets::load().expect("assets")).expect("game");
        game.new_game();
        game.wh = Where::Flying;
        game.ensure_traffic();
        game.traffic.retain(|s| s.role == "police");
        let police = game.traffic.iter().position(|s| s.role == "police").expect("police");
        let days = game.stardate.days;
        let me = game.frames.ship_world_pos(&game.ship, days);
        let them = game.frames.ship_world_pos(&game.traffic[police].state, days);
        let away = (me - them).normalize();
        let right = DVec3::Y.cross(away).try_normalize().unwrap_or(DVec3::X);
        game.ship.orientation = DMat3::from_cols(right, away.cross(right), away);

        let shoot = |game: &mut Game| {
            game.laser_wait = 0.0;
            game.fm.stats.energy = 100.0;
            game.fire_laser();
        };
        shoot(&mut game);
        assert_eq!(game.fine_here(), 0);
        assert!(!game.police_warned);
        shoot(&mut game);
        assert!(game.police_warned, "two misses in sight of police are a warning");
        assert_eq!(game.fine_here(), 0, "the warning itself is not a fine");
        shoot(&mut game);
        assert_eq!(game.fine_here(), 500, "the next shot alerts the station");

        // Destroying the only police before a warning leaves no witness.
        game.fines.clear();
        game.police_misses = 0;
        game.police_warned = false;
        game.station_told = false;
        game.traffic.retain(|s| s.role == "police");
        let only = game.traffic.iter().position(|s| s.role == "police").unwrap();
        game.traffic[only].hull = 1.0;
        game.traffic[only].shield = 0.0;
        game.traffic[only].state.position = game.ship.position + DVec3::Z * 10.0;
        let them = game.frames.ship_world_pos(&game.traffic[only].state, game.stardate.days);
        let me = game.frames.ship_world_pos(&game.ship, game.stardate.days);
        let at = (them - me).normalize();
        let right = DVec3::Y.cross(at).try_normalize().unwrap_or(DVec3::X);
        game.ship.orientation = DMat3::from_cols(right, at.cross(right), at);
        shoot(&mut game);
        assert!(game.traffic.iter().all(|s| s.role != "police" || !s.alive));
        assert_eq!(game.fine_here(), 0);

        // The democracy fine is not payable, or even outstanding, under anarchy.
        game.add_fine(10_000);
        assert_eq!(game.fine_here(), 10_000);
        game.galaxy.systems.get_mut("ross154").unwrap().government = "Anarchy".into();
        assert_eq!(game.fine_here(), 0);
        assert!(!game.wanted_here());
    }
}
