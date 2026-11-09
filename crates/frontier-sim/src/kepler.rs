//! Kepler orbital mechanics — solve positions from elements at time T.

use glam::DVec3;
use std::f64::consts::PI;

/// GM in km^3/s^2 for a Sol-mass star.
pub const GM_SOL: f64 = 1.327_124_400_18e11;
/// Astronomical unit in km.
pub const AU_KM: f64 = 1.495_978_707e8;

/// Classic Keplerian elements relative to a parent barycenter.
#[derive(Clone, Debug)]
pub struct OrbitalElements {
    pub semi_major_au: f64,
    pub eccentricity: f64,
    pub inclination_deg: f64,
    pub long_asc_node_deg: f64,
    pub arg_periapsis_deg: f64,
    pub mean_anomaly0_deg: f64,
    pub period_days: Option<f64>,
    pub parent_mass_sol: f64,
}

impl OrbitalElements {
    pub fn new(semi_major_au: f64) -> Self {
        Self {
            semi_major_au,
            eccentricity: 0.0,
            inclination_deg: 0.0,
            long_asc_node_deg: 0.0,
            arg_periapsis_deg: 0.0,
            mean_anomaly0_deg: 0.0,
            period_days: None,
            parent_mass_sol: 1.0,
        }
    }

    pub fn period(&self) -> f64 {
        if let Some(days) = self.period_days {
            return days;
        }
        let years = (self.semi_major_au.powi(3) / self.parent_mass_sol.max(1e-9)).sqrt();
        years * 365.25
    }
}

fn solve_kepler(mut m: f64, e: f64) -> f64 {
    m = ((m + PI) % (2.0 * PI)) - PI;
    if e < 1e-8 {
        return m;
    }
    let mut eccentric = if e < 0.8 { m } else { PI };
    for _ in 0..32 {
        let f = eccentric - e * eccentric.sin() - m;
        let fp = 1.0 - e * eccentric.cos();
        let d = f / fp;
        eccentric -= d;
        if d.abs() < 1e-10 {
            break;
        }
    }
    eccentric
}

/// Cartesian position (km) relative to parent at stardate day `days`.
pub fn position_at(elements: &OrbitalElements, days: f64) -> DVec3 {
    let a_km = elements.semi_major_au * AU_KM;
    let e = elements.eccentricity;
    let period = elements.period();
    let n = 2.0 * PI / period;
    let m0 = elements.mean_anomaly0_deg.to_radians();
    let mean = m0 + n * days;
    let eccentric = solve_kepler(mean, e);

    let cos_e = eccentric.cos();
    let sin_e = eccentric.sin();
    let x_p = a_km * (cos_e - e);
    let y_p = a_km * (1.0 - e * e).sqrt() * sin_e;
    let z_p = 0.0;

    let w = elements.arg_periapsis_deg.to_radians();
    let i = elements.inclination_deg.to_radians();
    let o = elements.long_asc_node_deg.to_radians();

    let (cos_w, sin_w) = (w.cos(), w.sin());
    let (cos_o, sin_o) = (o.cos(), o.sin());
    let (cos_i, sin_i) = (i.cos(), i.sin());

    let r11 = cos_o * cos_w - sin_o * sin_w * cos_i;
    let r12 = -cos_o * sin_w - sin_o * cos_w * cos_i;
    let r13 = sin_o * sin_i;
    let r21 = sin_o * cos_w + cos_o * sin_w * cos_i;
    let r22 = -sin_o * sin_w + cos_o * cos_w * cos_i;
    let r23 = -cos_o * sin_i;
    let r31 = sin_w * sin_i;
    let r32 = cos_w * sin_i;
    let r33 = cos_i;

    let (x, y, z) = (
        r11 * x_p + r12 * y_p + r13 * z_p,
        r21 * x_p + r22 * y_p + r23 * z_p,
        r31 * x_p + r32 * y_p + r33 * z_p,
    );
    // The elements use the astronomical Z-up ecliptic; the world is Y-up (moons,
    // stations and planet spin all turn about +Y), so rotate the ecliptic onto XZ.
    DVec3::new(x, z, -y)
}

/// Finite-difference velocity (km/s).
pub fn velocity_at(elements: &OrbitalElements, days: f64, dt: f64) -> DVec3 {
    let p0 = position_at(elements, days);
    let p1 = position_at(elements, days + dt);
    (p1 - p0) / (dt * 86400.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbit_closes() {
        let el = OrbitalElements {
            eccentricity: 0.0,
            parent_mass_sol: 1.0,
            ..OrbitalElements::new(1.0)
        };
        let t = el.period();
        let p0 = position_at(&el, 0.0);
        let p1 = position_at(&el, t);
        assert!(
            (p0 - p1).length() < 1.0,
            "orbit did not close, err={} km",
            (p0 - p1).length()
        );
    }

    #[test]
    fn eccentric_orbit_closes() {
        let el = OrbitalElements {
            eccentricity: 0.2,
            parent_mass_sol: 0.17,
            ..OrbitalElements::new(0.5)
        };
        let t = el.period();
        let p0 = position_at(&el, 10.0);
        let p1 = position_at(&el, 10.0 + t);
        assert!(
            (p0 - p1).length() < 10.0,
            "eccentric orbit err={}",
            (p0 - p1).length()
        );
    }
}
