//! Flight model as the FE2 manual describes it (Section Two, Navigation and Flight).
//!
//! - Manual Control: fly-by-wire. Enter / Right Shift change the *set speed*; the
//!   computer fires main, retro and lateral thrusters to bring velocity to
//!   `forward * set_speed`, compensating gravity, with a lag set by thrust.
//! - Engines Off: ballistic. Gravity acts; Enter / Shift fire the main and retro
//!   thrusters directly.
//! - Take Off: vertical thrusters lift the ship slowly off the pad until forward
//!   thrust is used, then the ship drops into Manual Control.

use crate::frame::FrameState;
use crate::frontier::{BOOSTER_RECHARGE, SHIELD_RECHARGE};
use glam::{DMat3, DVec3};
use std::collections::HashMap;

/// 1 g in km/s^2.
pub const G_KMS2: f64 = 0.00981;

fn skew(v: DVec3) -> DMat3 {
    DMat3::from_cols(
        DVec3::new(0.0, v.z, -v.y),
        DVec3::new(-v.z, 0.0, v.x),
        DVec3::new(v.y, -v.x, 0.0),
    )
}

/// Integrate body angular rates into orientation (columns: right, up, forward).
pub fn rotate_local(orientation: DMat3, rates: DVec3, dt: f64) -> DMat3 {
    let w = rates.length();
    if w < 1e-12 || dt.abs() < 1e-12 {
        return orientation;
    }
    let k = skew(rates / w);
    let angle = w * dt;
    let r = DMat3::IDENTITY + k * angle.sin() + (k * k) * (1.0 - angle.cos());
    orthonormalize(orientation * r)
}

pub fn orthonormalize(m: DMat3) -> DMat3 {
    let f = m.col(2).normalize_or_zero();
    let r = m.col(1).cross(f).normalize_or_zero();
    let u = f.cross(r);
    DMat3::from_cols(r, u, f)
}

#[derive(Clone, Debug)]
pub struct ShipStats {
    pub name: String,
    pub mass_t: f64,
    /// Main thruster acceleration in g (manual, Appendix Five).
    pub main_g: f64,
    pub retro_g: f64,
    pub lateral_g: f64,
    pub max_fuel_t: f64,
    pub fuel_t: f64,
    pub jump_range_ly: f64,
    pub hull: f64,
    pub max_hull: f64,
    pub shield: f64,
    pub max_shield: f64,
    pub energy: f64,
    pub max_energy: f64,
    pub laser_power: f64,
    pub has_missile: bool,
    pub missiles: i32,
    pub has_docking_computer: bool,
    pub cargo_capacity: i32,
    pub cargo: HashMap<String, i32>,
    pub price: i32,
    pub energy_booster: bool,
    pub has_life_support: bool,
    pub has_fuel_scoop: bool,
    pub has_atmo_shield: bool,
    pub fire_rate: f64,
    pub beam_laser: bool,
    pub mining_laser: bool,
    pub drive_h: f64,
}

impl Default for ShipStats {
    fn default() -> Self {
        Self {
            name: "Eagle long range fighter".into(),
            mass_t: 25.0,
            main_g: 25.0,
            retro_g: 10.0,
            lateral_g: 5.0,
            max_fuel_t: 1.0,
            fuel_t: 1.0,
            jump_range_ly: 8.0,
            hull: 100.0,
            max_hull: 100.0,
            shield: 0.0,
            max_shield: 0.0,
            energy: 100.0,
            max_energy: 100.0,
            laser_power: 8.0,
            has_missile: true,
            missiles: 2,
            has_docking_computer: false,
            cargo_capacity: 4,
            cargo: HashMap::new(),
            price: 38000,
            energy_booster: false,
            has_life_support: false,
            has_fuel_scoop: false,
            has_atmo_shield: false,
            fire_rate: 1.6,
            beam_laser: false,
            mining_laser: false,
            drive_h: 1.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlightMode {
    Manual,
    EnginesOff,
    TakeOff,
}

#[derive(Clone, Debug)]
pub struct FlightControls {
    /// Turn commands, -1..1. Pitch +1 climbs (Z key), yaw +1 turns right (> key).
    pub pitch: f64,
    pub yaw: f64,
    pub roll: f64,
    /// Engines Off only: +1 main (Enter), -1 retro (Shift).
    pub thrust: f64,
    /// Requested speed along the nose, km/s.
    pub set_speed: f64,
    pub mode: FlightMode,
}

impl Default for FlightControls {
    fn default() -> Self {
        Self { pitch: 0.0, yaw: 0.0, roll: 0.0, thrust: 0.0, set_speed: 0.0, mode: FlightMode::Manual }
    }
}

pub struct FlightModel {
    pub stats: ShipStats,
    pub controls: FlightControls,
    pub cabin_temp: f64,
}

/// Turn rate in rad/s at full stick. Small ships turn quickly.
const MAX_TURN: f64 = 0.9;
/// Fly-by-wire response time constant, seconds.
const RESPONSE_S: f64 = 0.6;
/// Take-off climb acceleration net of gravity, km/s^2 ("can seem quite slow").
const TAKEOFF_LIFT: f64 = 0.0025;
/// Vertical speed the take-off thrusters settle at, km/s.
const TAKEOFF_CLIMB: f64 = 0.04;
const TAKEOFF_RESPONSE_S: f64 = 4.0;

impl FlightModel {
    pub fn new(stats: ShipStats) -> Self {
        Self { stats, controls: FlightControls::default(), cabin_temp: 20.0 }
    }

    pub fn forward(o: DMat3) -> DVec3 {
        o.col(2)
    }
    pub fn up(o: DMat3) -> DVec3 {
        o.col(1)
    }
    pub fn right(o: DMat3) -> DVec3 {
        o.col(0)
    }

    /// Thruster acceleration (km/s^2) the ship would like, before limits.
    fn wanted_accel(&self, s: &FrameState, gravity: DVec3, ref_vel: DVec3) -> DVec3 {
        let fwd = Self::forward(s.orientation);
        match self.controls.mode {
            FlightMode::EnginesOff => {
                let g = if self.controls.thrust >= 0.0 { self.stats.main_g } else { self.stats.retro_g };
                fwd * self.controls.thrust * g * G_KMS2
            }
            FlightMode::TakeOff => {
                let up = Self::up(s.orientation);
                let target = ref_vel + up * TAKEOFF_CLIMB;
                ((target - s.velocity) / TAKEOFF_RESPONSE_S).clamp_length_max(TAKEOFF_LIFT) - gravity
            }
            FlightMode::Manual => {
                let target = ref_vel + fwd * self.controls.set_speed;
                (target - s.velocity) / RESPONSE_S - gravity
            }
        }
    }

    /// Clamp a body-frame thrust request to the ship's thrusters.
    fn limit(&self, o: DMat3, a: DVec3) -> DVec3 {
        let local = o.transpose() * a;
        let lat = self.stats.lateral_g * G_KMS2;
        let z = local.z.clamp(-self.stats.retro_g * G_KMS2, self.stats.main_g * G_KMS2);
        let x = local.x.clamp(-lat, lat);
        let y = local.y.clamp(-lat, lat);
        o * DVec3::new(x, y, z)
    }

    /// Advance one step. `gravity` is the field at the ship in km/s^2 and `ref_vel` the
    /// velocity of the body speeds are measured against (e.g. the turning ground), both
    /// in the parent frame.
    pub fn step(&mut self, state: &FrameState, dt: f64, gravity: DVec3, ref_vel: DVec3) -> FrameState {
        if dt <= 0.0 {
            return state.clone();
        }
        let mut s = state.clone();
        let c = &self.controls;
        // Body rates: +X climbs the nose, +Y turns right, +Z rolls.
        s.angular_velocity = DVec3::new(-c.pitch, c.yaw, -c.roll) * MAX_TURN;
        s.orientation = rotate_local(s.orientation, s.angular_velocity, dt);

        if self.controls.mode == FlightMode::TakeOff && self.controls.set_speed.abs() > 1e-6 {
            self.controls.mode = FlightMode::Manual;
        }
        let thrust = if self.stats.fuel_t > 0.0 {
            self.limit(s.orientation, self.wanted_accel(&s, gravity, ref_vel))
        } else {
            DVec3::ZERO
        };

        let max_a = self.stats.main_g * G_KMS2;
        let burn = thrust.length() / max_a.max(1e-9) * 0.00002 * dt;
        self.stats.fuel_t = (self.stats.fuel_t - burn).max(0.0);

        let accel = thrust + gravity;
        s.position += s.velocity * dt + 0.5 * accel * dt * dt;
        s.velocity += accel * dt;

        self.stats.energy = (self.stats.energy + 5.0 * dt).min(self.stats.max_energy);
        let rate = if self.stats.energy_booster { BOOSTER_RECHARGE } else { SHIELD_RECHARGE };
        if self.stats.max_shield > 0.0 {
            self.stats.shield = (self.stats.shield + self.stats.max_shield * rate * dt).min(self.stats.max_shield);
        }
        s
    }

    /// Enter / Right Shift in Manual Control.
    pub fn adjust_set_speed(&mut self, delta: f64) {
        self.controls.set_speed = (self.controls.set_speed + delta).clamp(-5.0, 1000.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ship() -> (FlightModel, FrameState) {
        (FlightModel::new(ShipStats::default()), FrameState::new("p"))
    }

    #[test]
    fn manual_hovers_against_gravity() {
        let (mut f, mut s) = ship();
        let g = DVec3::new(0.0, -0.004, 0.0);
        for _ in 0..600 {
            s = f.step(&s, 0.05, g, DVec3::ZERO);
        }
        assert!(s.velocity.length() < 1e-4, "drift {:?}", s.velocity);
    }

    #[test]
    fn engines_off_falls() {
        let (mut f, mut s) = ship();
        f.controls.mode = FlightMode::EnginesOff;
        let g = DVec3::new(0.0, -0.004, 0.0);
        for _ in 0..100 {
            s = f.step(&s, 0.1, g, DVec3::ZERO);
        }
        assert!((s.velocity.y + 0.04).abs() < 1e-9);
    }

    #[test]
    fn manual_reaches_set_speed() {
        let (mut f, mut s) = ship();
        f.controls.set_speed = 1.0;
        for _ in 0..400 {
            s = f.step(&s, 0.05, DVec3::ZERO, DVec3::ZERO);
        }
        assert!((s.velocity.z - 1.0).abs() < 1e-3, "v {:?}", s.velocity);
    }

    #[test]
    fn takeoff_climbs_then_manual() {
        let (mut f, mut s) = ship();
        f.controls.mode = FlightMode::TakeOff;
        let g = DVec3::new(0.0, -0.004, 0.0);
        for _ in 0..50 {
            s = f.step(&s, 0.1, g, DVec3::ZERO);
        }
        assert!(s.velocity.y > 0.0);
        f.adjust_set_speed(0.1);
        f.step(&s, 0.1, g, DVec3::ZERO);
        assert_eq!(f.controls.mode, FlightMode::Manual);
    }
}
