//! Ships sharing the system: pirates, police and traders.
//! Anarchy fields pirates and does not fine you for firing. A government that
//! keeps police records a fine only when a witness can still tell a station.
//! Two missed shots in sight of police are a warning. The next shot alerts the
//! nearest station. Killing the only police ship before that warning leaves
//! nobody to make the report. The fine stays with that government.

const WITNESS_KM: f64 = 80.0;

use super::{steer_toward, FlightMode, FrameState, Game, Screen, Where};
use crate::font::money;
use frontier_sim::flight::{FlightModel, ShipStats};
use frontier_sim::frontier::{apply_shield_hit, laser_damage_tons};
use glam::{DMat3, DVec3};

pub(super) struct Npc {
    #[allow(dead_code)]
    pub id: String,
    pub name: String,
    pub role: &'static str,
    pub state: FrameState,
    pub hull: f64,
    pub shield: f64,
    pub max_shield: f64,
    pub legal: bool,
    pub bounty_cr: i64,
    pub alive: bool,
    cooldown: f64,
    laser_mw: f64,
    main_g: f64,
}

impl Game {
    pub(super) fn ensure_traffic(&mut self) {
        if self.spawned_traffic || !matches!(self.wh, Where::Flying) {
            return;
        }
        let wanted = self.wanted_here();
        let roles: &[&str] = match self.system().government.as_str() {
            "Anarchy" => &["pirate", "pirate", "trader", "trader"],
            "Democracy" | "Corporate" => {
                if wanted {
                    &["police", "trader", "trader", "pirate"]
                } else {
                    &["police", "trader", "trader"]
                }
            }
            _ => &["pirate", "trader", "police"],
        };
        for (i, role) in roles.iter().enumerate() {
            let dist = if *role == "pirate" { 22.0 + i as f64 * 6.0 } else { 40.0 + i as f64 * 12.0 };
            self.spawn_ship(role, dist);
        }
        self.spawned_traffic = true;
    }

    pub(super) fn spawn_raid(&mut self) {
        if !matches!(self.wh, Where::Flying) {
            self.msg("Cheat: launch first");
            return;
        }
        for i in 0..4 {
            self.spawn_ship("pirate", 16.0 + i as f64 * 4.0);
        }
        self.msg("Cheat: four pirates inbound");
    }

    pub(super) fn wipe_pirates(&mut self) {
        let mut n = 0;
        let mut pay = 0i64;
        for npc in &mut self.traffic {
            if npc.alive && npc.role == "pirate" {
                npc.alive = false;
                n += 1;
                pay += npc.bounty_cr;
            }
        }
        self.traffic.retain(|s| s.alive);
        self.cash += pay * 10;
        self.msg(format!("Cheat: {n} pirate{} removed", if n == 1 { "" } else { "s" }));
    }

    fn spawn_ship(&mut self, role: &'static str, dist: f64) {
        let seq = self.traffic_seq;
        self.traffic_seq = self.traffic_seq.wrapping_add(1);
        let (name, hull, shield, laser, main_g, bounty, legal) = match role {
            "pirate" => ("Pirate Eagle", 40.0, 0.0, 2.0, 20.0, 2000, false),
            "police" => ("Police Viper", 80.0, 20.0, 4.0, 18.0, 0, true),
            _ => ("Trader", 60.0, 8.0, 0.0, 8.0, 0, true),
        };
        let up = self.ship.position.try_normalize().unwrap_or(DVec3::Y);
        let tangent = up.cross(DVec3::X).try_normalize().unwrap_or(DVec3::Z);
        let bit = up.cross(tangent);
        let ang = (seq as f64) * 2.399;
        let offset = tangent * (dist * ang.cos()) + bit * (dist * ang.sin()) + up * 8.0;
        let mut state = FrameState::new(self.ship.parent_id.clone());
        state.position = self.ship.position + offset;
        let to_player = -offset;
        if let Some(fwd) = to_player.try_normalize() {
            let right = DVec3::Y.cross(fwd).try_normalize().unwrap_or(DVec3::X);
            let up = fwd.cross(right);
            state.orientation = DMat3::from_cols(right, up, fwd);
        }
        self.traffic.push(Npc {
            id: format!("{role}-{seq}"),
            name: name.into(),
            role,
            state,
            hull,
            shield,
            max_shield: shield,
            legal,
            bounty_cr: bounty,
            alive: true,
            cooldown: 0.0,
            laser_mw: laser,
            main_g,
        });
    }

    pub(super) fn step_traffic(&mut self, dt: f64) {
        if dt <= 0.0 || !matches!(self.wh, Where::Flying) {
            return;
        }
        self.laser_wait = (self.laser_wait - dt).max(0.0);
        let n = self.traffic.len();
        for i in 0..n {
            if self.screen != Screen::Play {
                return;
            }
            self.step_one(i, dt);
        }
        self.traffic.retain(|s| s.alive);
    }

    fn step_one(&mut self, i: usize, dt: f64) {
        if !self.traffic[i].alive {
            return;
        }
        let days = self.stardate.days;
        let me = self.frames.ship_world_pos(&self.ship, days);
        let them = self.frames.ship_world_pos(&self.traffic[i].state, days);
        let delta = me - them;
        let dist = delta.length();
        let (pitch, yaw, roll, align) = steer_toward(self.traffic[i].state.orientation, delta);
        let role = self.traffic[i].role;
        let hostile = role == "pirate" || (role == "police" && self.wanted_here());
        let speed = if !hostile {
            1.2
        } else if dist > 8.0 {
            4.0
        } else {
            0.6
        };
        let mut stats = ShipStats::default();
        stats.fuel_t = 50.0;
        stats.max_fuel_t = 50.0;
        stats.main_g = self.traffic[i].main_g;
        stats.retro_g = stats.main_g * 0.4;
        stats.lateral_g = stats.main_g * 0.2;
        let mut fm = FlightModel::new(stats);
        fm.controls.mode = FlightMode::Manual;
        fm.controls.pitch = pitch;
        fm.controls.yaw = yaw;
        fm.controls.roll = roll;
        fm.controls.set_speed = speed;
        let stepped = fm.step(&self.traffic[i].state, dt, DVec3::ZERO, DVec3::ZERO);
        self.traffic[i].state = stepped;
        self.traffic[i].cooldown = (self.traffic[i].cooldown - dt).max(0.0);
        let range = if role == "police" { 18.0 } else { 25.0 };
        if hostile && dist < range && align > 0.85 && self.traffic[i].cooldown <= 0.0 && self.traffic[i].laser_mw > 0.0 {
            let mw = self.traffic[i].laser_mw;
            self.traffic[i].cooldown = if role == "police" { 0.7 } else { 0.9 };
            let dmg = laser_damage_tons(mw, false, false);
            self.hurt_player(dmg);
        }
    }

    pub(super) fn fire_laser(&mut self) {
        if self.laser_wait > 0.0 || !matches!(self.wh, Where::Flying) {
            return;
        }
        if self.fm.stats.energy < 8.0 || self.fm.stats.laser_power <= 0.0 {
            return;
        }
        self.fm.stats.energy -= 8.0;
        self.laser_wait = 0.45;
        let days = self.stardate.days;
        let me = self.frames.ship_world_pos(&self.ship, days);
        let fwd = self.ship.orientation.col(2);
        let mut best: Option<(usize, f64)> = None;
        for (i, npc) in self.traffic.iter().enumerate() {
            if !npc.alive {
                continue;
            }
            let rel = self.frames.ship_world_pos(&npc.state, days) - me;
            let dist = rel.length();
            if !(0.05..30.0).contains(&dist) {
                continue;
            }
            let align = fwd.dot(rel / dist);
            if align > 0.9 && best.map_or(true, |(_, a)| align > a) {
                best = Some((i, align));
            }
        }
        let Some((i, _)) = best else {
            self.note_miss();
            return;
        };
        let dmg = laser_damage_tons(self.fm.stats.laser_power, self.fm.stats.beam_laser, self.fm.stats.mining_laser);
        self.damage_npc(i, dmg);
    }

    /// A shot that hit nothing. Two of those, with police close enough to see,
    /// are a warning. The next one calls the nearest station.
    fn note_miss(&mut self) {
        if !self.enforces_law() || !self.police_in_sight(None) {
            return;
        }
        if self.police_warned {
            if !self.station_told {
                self.alert_station(500);
            }
            return;
        }
        self.police_misses = self.police_misses.saturating_add(1);
        if self.police_misses >= 2 {
            self.police_warned = true;
            self.msg("Police warning");
        }
    }

    fn damage_npc(&mut self, i: usize, dmg: f64) {
        let (dead, role, lawful, name, bounty) = {
            let npc = &mut self.traffic[i];
            let (shield, through) = apply_shield_hit(npc.shield, npc.max_shield, dmg);
            npc.shield = shield;
            npc.hull -= through;
            let dead = npc.hull <= 0.0;
            if dead {
                npc.alive = false;
            }
            (dead, npc.role, npc.legal, npc.name.clone(), npc.bounty_cr)
        };
        if role == "pirate" || !lawful {
            if dead {
                self.kills += 1;
                self.cash += bounty * 10;
                self.msg(format!("{name}  {}", money(bounty * 10)));
            }
            return;
        }
        // Another patrol can report a kill. The ship under fire cannot, unless
        // it already gave a warning and this is the shot after that warning.
        let other = self.police_in_sight(Some(i));
        if dead {
            self.kills += 1;
            if self.enforces_law() && (other || self.police_warned) {
                self.alert_station(5000);
            } else {
                self.msg(name);
            }
            return;
        }
        if self.enforces_law() && self.police_warned && !self.station_told {
            self.alert_station(500);
        }
    }

    pub(super) fn enforces_law(&self) -> bool {
        self.system().government != "Anarchy"
    }

    pub(super) fn fine_here(&self) -> i64 {
        self.fines.get(&self.system().government).copied().unwrap_or(0)
    }

    pub(super) fn wanted_here(&self) -> bool {
        self.fine_here() > 0
    }

    pub(super) fn add_fine(&mut self, amount: i64) {
        if amount <= 0 || !self.enforces_law() {
            return;
        }
        let gov = self.system().government.clone();
        *self.fines.entry(gov).or_insert(0) += amount;
    }

    /// `except` is a ship that can no longer talk: it is the one just destroyed.
    fn police_in_sight(&self, except: Option<usize>) -> bool {
        let days = self.stardate.days;
        let me = self.frames.ship_world_pos(&self.ship, days);
        self.traffic.iter().enumerate().any(|(i, n)| {
            Some(i) != except && n.alive && n.role == "police" && {
                let d = (self.frames.ship_world_pos(&n.state, days) - me).length();
                d <= WITNESS_KM
            }
        })
    }

    fn alert_station(&mut self, amount: i64) {
        if !self.enforces_law() {
            return;
        }
        let station = self.nearest_port_name();
        self.add_fine(amount);
        self.station_told = true;
        self.police_warned = true;
        self.msg(format!("{station} alerted  {}", money(amount * 10)));
    }

    fn nearest_port_name(&self) -> String {
        let days = self.stardate.days;
        let me = self.frames.ship_world_pos(&self.ship, days);
        let mut best: Option<(f64, String)> = None;
        let mut consider = |id: &str, name: &str| {
            let d = (me - self.frames.world_position(id, days)).length();
            if best.as_ref().map_or(true, |(bd, _)| d < *bd) {
                best = Some((d, name.to_string()));
            }
        };
        for st in &self.system().stations {
            consider(&st.id, &st.name);
        }
        for p in &self.system().planets {
            for sp in &p.starports {
                consider(&sp.id, &sp.name);
            }
        }
        best.map(|(_, n)| n).unwrap_or_else(|| self.system().name.clone())
    }

    fn hurt_player(&mut self, dmg: f64) {
        if self.god_mode || self.screen != Screen::Play {
            return;
        }
        let (shield, through) = apply_shield_hit(self.fm.stats.shield, self.fm.stats.max_shield, dmg);
        self.fm.stats.shield = shield;
        self.fm.stats.hull = (self.fm.stats.hull - through).max(0.0);
        self.fitted.hull = self.fm.stats.hull;
        if self.fm.stats.hull <= 0.0 {
            self.destroyed("Hull destroyed.");
        }
    }
}
