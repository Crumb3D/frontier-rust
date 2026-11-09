//! Galaxy catalog, seeded systems, hand-tuned Ross 154 and Sol.

use crate::frame::{BodyRef, FrameSystem, WorldPos};
use crate::kepler::OrbitalElements;
use crate::paths::data_dir;
use glam::DVec3;
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use serde::Deserialize;
use std::collections::HashMap;
use std::f64::consts::PI;

pub const ECONOMIES: &[&str] = &[
    "Agricultural",
    "Industrial",
    "High-Tech",
    "Extraction",
    "Tourist",
    "Colony",
];
pub const GOVERNMENTS: &[&str] = &[
    "Democracy",
    "Corporate",
    "Dictatorship",
    "Anarchy",
    "Feudal",
    "Communist",
];

const PLANET_NAMES: &[&str] = &[
    "Merlin", "Avalon", "Tintagel", "Camelot", "Lyonesse", "Galahad", "Earth", "Mars", "Venus",
    "Mercury", "Jupiter", "Saturn", "New Caledonia", "Port Royal", "Stark", "Iota", "Helios",
    "Nova", "Eden", "Athena", "Orion", "Vega", "Rhea", "Titan",
];

#[derive(Clone, Debug)]
pub struct Starport {
    pub id: String,
    pub name: String,
    pub planet_id: String,
    pub lat_deg: f64,
    pub lon_deg: f64,
    pub pads: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct Planet {
    pub id: String,
    pub name: String,
    pub elements: OrbitalElements,
    pub radius_km: f64,
    pub has_atmosphere: bool,
    pub atmosphere_km: f64,
    pub has_starport: bool,
    pub color: [u8; 3],
    pub inhabited: bool,
    pub parent_id: Option<String>,
    pub is_gas_giant: bool,
    pub has_rings: bool,
    pub rotation_days: f64,
    pub moon_orbit_km: f64,
    pub moon_period_days: f64,
    pub moon_phase0: f64,
    pub starports: Vec<Starport>,
}

#[derive(Clone, Debug)]
pub struct Station {
    pub id: String,
    pub name: String,
    pub parent_planet_id: String,
    pub orbit_radius_km: f64,
    pub orbit_period_days: f64,
    pub radius_km: f64,
    pub phase0: f64,
}

#[derive(Clone, Debug)]
pub struct StarSystem {
    pub id: String,
    pub name: String,
    pub position_ly: DVec3,
    pub spectral: String,
    pub mass: f64,
    pub seed: u32,
    pub economy: String,
    pub government: String,
    pub tech_level: i32,
    pub allegiance: String,
    pub planets: Vec<Planet>,
    pub stations: Vec<Station>,
}

impl StarSystem {
    pub fn planet_by_id(&self, pid: &str) -> Option<&Planet> {
        self.planets.iter().find(|p| p.id == pid)
    }

    pub fn starport_by_id(&self, sid: &str) -> Option<&Starport> {
        for p in &self.planets {
            if let Some(sp) = p.starports.iter().find(|s| s.id == sid) {
                return Some(sp);
            }
        }
        None
    }

    pub fn star_color(&self) -> [u8; 3] {
        match self.spectral.chars().next().unwrap_or('G') {
            'O' => [155, 176, 255],
            'B' => [170, 191, 255],
            'A' => [202, 215, 255],
            'F' => [248, 247, 255],
            'G' => [255, 244, 234],
            'K' => [255, 210, 161],
            'M' => [255, 160, 100],
            _ => [255, 244, 234],
        }
    }
}

fn hash_seed(name: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for ch in name.bytes() {
        h ^= ch as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

pub fn surface_offset(lat_deg: f64, lon_deg: f64, radius_km: f64) -> DVec3 {
    let lat = lat_deg.to_radians();
    let lon = lon_deg.to_radians();
    let r = radius_km;
    DVec3::new(
        r * lat.cos() * lon.cos(),
        r * lat.sin(),
        r * lat.cos() * lon.sin(),
    )
}

fn hand_ross154() -> StarSystem {
    let aster = Planet {
        id: "ross154_aster".into(),
        name: "Aster".into(),
        elements: OrbitalElements {
            semi_major_au: 0.18,
            eccentricity: 0.03,
            inclination_deg: 1.5,
            mean_anomaly0_deg: 55.0,
            parent_mass_sol: 0.17,
            ..OrbitalElements::new(0.18)
        },
        radius_km: 52000.0,
        has_atmosphere: true,
        atmosphere_km: 800.0,
        has_starport: false,
        // Teal with red rings, as seen from Sirocco in the DOS opening still (PIC002).
        color: [44, 108, 108],
        inhabited: false,
        parent_id: None,
        is_gas_giant: true,
        has_rings: true,
        rotation_days: 1.0,
        moon_orbit_km: 0.0,
        moon_period_days: 1.0,
        moon_phase0: 0.0,
        starports: vec![],
    };
    let merlin = Planet {
        id: "ross154_merlin".into(),
        name: "Merlin".into(),
        elements: OrbitalElements {
            parent_mass_sol: 0.001,
            ..OrbitalElements::new(0.001)
        },
        radius_km: 2800.0,
        has_atmosphere: true,
        atmosphere_km: 55.0,
        has_starport: true,
        // Pink land with purple seas, from the DOS rear view after launch.
        color: [188, 108, 108],
        inhabited: true,
        parent_id: Some("ross154_aster".into()),
        is_gas_giant: false,
        has_rings: false,
        rotation_days: 1.0,
        moon_orbit_km: 185000.0,
        moon_period_days: 2.8,
        // With Sirocco at lon -113 this gives the opening sky at 12:00 1-Jan-3200:
        // daylight, with the sun behind the pad lighting Aster about 22 degrees up
        // (FE2 manual Fig 81, PIC002).
        moon_phase0: 117f64.to_radians(),
        starports: vec![Starport {
            id: "ross154_sirocco".into(),
            name: "Sirocco Station".into(),
            planet_id: "ross154_merlin".into(),
            lat_deg: 18.0,
            lon_deg: -113.0,
            pads: vec![
                "Pad 1".into(),
                "Pad 2".into(),
                "Pad 3".into(),
                "Pad 4".into(),
                "Pad 5".into(),
                "Hangar North".into(),
            ],
        }],
    };
    let avalon = Planet {
        id: "ross154_avalon".into(),
        name: "Avalon".into(),
        elements: OrbitalElements {
            eccentricity: 0.05,
            inclination_deg: 5.0,
            mean_anomaly0_deg: 120.0,
            parent_mass_sol: 0.17,
            ..OrbitalElements::new(0.55)
        },
        radius_km: 6000.0,
        has_atmosphere: false,
        atmosphere_km: 0.0,
        has_starport: false,
        color: [160, 120, 80],
        inhabited: false,
        parent_id: None,
        is_gas_giant: false,
        has_rings: false,
        rotation_days: 1.0,
        moon_orbit_km: 0.0,
        moon_period_days: 1.0,
        moon_phase0: 0.0,
        starports: vec![],
    };
    let station = Station {
        id: "ross154_station".into(),
        name: "Merlin Orbital".into(),
        parent_planet_id: "ross154_merlin".into(),
        orbit_radius_km: 6500.0,
        orbit_period_days: 0.35,
        radius_km: 0.8,
        phase0: 0.3,
    };
    StarSystem {
        id: "ross154".into(),
        name: "Ross 154".into(),
        position_ly: DVec3::new(2.94, -7.21, -5.65),
        spectral: "M3.5V".into(),
        mass: 0.17,
        seed: hash_seed("Ross 154"),
        economy: "Colony".into(),
        government: "Democracy".into(),
        tech_level: 5,
        allegiance: "Independent".into(),
        planets: vec![aster, merlin, avalon],
        stations: vec![station],
    }
}

fn hand_sol() -> StarSystem {
    let earth = Planet {
        id: "sol_earth".into(),
        name: "Earth".into(),
        elements: OrbitalElements {
            eccentricity: 0.0167,
            ..OrbitalElements::new(1.0)
        },
        radius_km: 6371.0,
        has_atmosphere: true,
        atmosphere_km: 100.0,
        has_starport: true,
        color: [40, 90, 180],
        inhabited: true,
        parent_id: None,
        is_gas_giant: false,
        has_rings: false,
        rotation_days: 1.0,
        moon_orbit_km: 0.0,
        moon_period_days: 1.0,
        moon_phase0: 0.0,
        starports: vec![Starport {
            id: "sol_london".into(),
            name: "London Starport".into(),
            planet_id: "sol_earth".into(),
            lat_deg: 51.5,
            lon_deg: -0.1,
            pads: (1..=6)
                .map(|n| format!("Pad {n}"))
                .chain(std::iter::once("Military Apron".into()))
                .collect(),
        }],
    };
    let mars = Planet {
        id: "sol_mars".into(),
        name: "Mars".into(),
        elements: OrbitalElements {
            eccentricity: 0.09,
            inclination_deg: 1.8,
            mean_anomaly0_deg: 80.0,
            ..OrbitalElements::new(1.52)
        },
        radius_km: 3390.0,
        has_atmosphere: true,
        atmosphere_km: 30.0,
        has_starport: true,
        color: [180, 90, 50],
        inhabited: true,
        parent_id: None,
        is_gas_giant: false,
        has_rings: false,
        rotation_days: 1.0,
        moon_orbit_km: 0.0,
        moon_period_days: 1.0,
        moon_phase0: 0.0,
        starports: vec![Starport {
            id: "sol_olympus".into(),
            name: "Olympus Starport".into(),
            planet_id: "sol_mars".into(),
            lat_deg: 18.0,
            lon_deg: -133.0,
            pads: vec!["Pad A".into(), "Pad B".into(), "Pad C".into(), "Pad D".into()],
        }],
    };
    let jupiter = Planet {
        id: "sol_jupiter".into(),
        name: "Jupiter".into(),
        elements: OrbitalElements {
            eccentricity: 0.05,
            inclination_deg: 1.3,
            mean_anomaly0_deg: 200.0,
            ..OrbitalElements::new(5.2)
        },
        radius_km: 69911.0,
        has_atmosphere: true,
        atmosphere_km: 2000.0,
        has_starport: false,
        color: [200, 160, 100],
        inhabited: false,
        parent_id: None,
        is_gas_giant: true,
        has_rings: false,
        rotation_days: 1.0,
        moon_orbit_km: 0.0,
        moon_period_days: 1.0,
        moon_phase0: 0.0,
        starports: vec![],
    };
    StarSystem {
        id: "sol".into(),
        name: "Sol".into(),
        position_ly: DVec3::ZERO,
        spectral: "G2V".into(),
        mass: 1.0,
        seed: hash_seed("Sol"),
        economy: "High-Tech".into(),
        government: "Democracy".into(),
        tech_level: 10,
        allegiance: "Federation".into(),
        planets: vec![earth, mars, jupiter],
        stations: vec![Station {
            id: "sol_earth_orbital".into(),
            name: "Gateway".into(),
            parent_planet_id: "sol_earth".into(),
            orbit_radius_km: 12000.0,
            orbit_period_days: 0.5,
            radius_km: 0.8,
            phase0: 1.0,
        }],
    }
}

fn hand_lave(star: &StarJson) -> StarSystem {
    let mut sys = seeded_system(star);
    sys.name = "Lave".into();
    sys
}

#[derive(Deserialize)]
struct StarJson {
    id: String,
    name: String,
    x: f64,
    y: f64,
    z: f64,
    spectral: String,
    mass: f64,
    #[serde(default)]
    allegiance: Option<String>,
    #[serde(default)]
    government: Option<String>,
    #[serde(default)]
    tech_level: Option<i32>,
    #[serde(default)]
    economy: Option<String>,
}

fn seeded_system(star: &StarJson) -> StarSystem {
    let seed = hash_seed(&star.name);
    let mut rng = StdRng::seed_from_u64(seed as u64);
    let n_planets = rng.gen_range(1..=5);
    let mut planets = Vec::new();
    let mut stations = Vec::new();
    for i in 0..n_planets {
        let a = 0.08 + i as f64 * 0.25 + rng.gen::<f64>() * 0.1;
        let name = PLANET_NAMES[((seed as usize) + i * 17) % PLANET_NAMES.len()];
        let pid = format!("{}_p{i}", star.id);
        let inhabited = rng.gen::<f64>() < 0.45;
        let has_atm = inhabited || rng.gen::<f64>() < 0.3;
        let gas = !inhabited && rng.gen::<f64>() < 0.25 && a > 0.3;
        let mut p = Planet {
            id: pid.clone(),
            name: name.into(),
            elements: OrbitalElements {
                eccentricity: rng.gen::<f64>() * 0.08,
                inclination_deg: rng.gen::<f64>() * 8.0,
                mean_anomaly0_deg: rng.gen::<f64>() * 360.0,
                long_asc_node_deg: rng.gen::<f64>() * 360.0,
                parent_mass_sol: star.mass,
                ..OrbitalElements::new(a)
            },
            radius_km: if gas {
                25000.0 + rng.gen::<f64>() * 40000.0
            } else {
                2000.0 + rng.gen::<f64>() * 8000.0
            },
            has_atmosphere: has_atm || gas,
            atmosphere_km: if gas {
                200.0 + rng.gen::<f64>() * 600.0
            } else if has_atm {
                50.0 + rng.gen::<f64>() * 80.0
            } else {
                0.0
            },
            has_starport: inhabited,
            color: if gas {
                [200, 160, 100]
            } else {
                [
                    (60.0 + rng.gen::<f64>() * 160.0) as u8,
                    (60.0 + rng.gen::<f64>() * 160.0) as u8,
                    (60.0 + rng.gen::<f64>() * 160.0) as u8,
                ]
            },
            inhabited,
            parent_id: None,
            is_gas_giant: gas,
            has_rings: false,
            rotation_days: 0.4 + rng.gen::<f64>() * 1.6,
            moon_orbit_km: 0.0,
            moon_period_days: 1.0,
            moon_phase0: 0.0,
            starports: vec![],
        };
        if inhabited {
            p.starports.push(Starport {
                id: format!("{pid}_port"),
                name: format!("{name} Starport"),
                planet_id: pid.clone(),
                lat_deg: rng.gen_range(-40.0..40.0),
                lon_deg: rng.gen_range(-180.0..180.0),
                pads: (1..=rng.gen_range(3..=6)).map(|n| format!("Pad {n}")).collect(),
            });
            if rng.gen::<f64>() < 0.7 {
                stations.push(Station {
                    id: format!("{pid}_stn"),
                    name: format!("{name} Orbital"),
                    parent_planet_id: pid,
                    orbit_radius_km: p.radius_km * 1.8 + 2000.0,
                    orbit_period_days: 0.3 + rng.gen::<f64>() * 0.5,
                    radius_km: 0.8,
                    phase0: rng.gen::<f64>() * PI * 2.0,
                });
            }
        }
        planets.push(p);
    }
    StarSystem {
        id: star.id.clone(),
        name: star.name.clone(),
        position_ly: DVec3::new(star.x, star.y, star.z),
        spectral: star.spectral.clone(),
        mass: star.mass,
        seed,
        economy: ECONOMIES[(seed as usize) % ECONOMIES.len()].into(),
        government: GOVERNMENTS[((seed as usize) / 7) % GOVERNMENTS.len()].into(),
        tech_level: 1 + (seed % 10) as i32,
        allegiance: "Independent".into(),
        planets,
        stations,
    }
}

pub struct Galaxy {
    pub systems: HashMap<String, StarSystem>,
}

impl Galaxy {
    pub fn load() -> Result<Self, String> {
        let path = data_dir().join("stars.json");
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("read {}: {e}", path.display()))?;
        let stars: Vec<StarJson> =
            serde_json::from_str(&text).map_err(|e| format!("parse stars.json: {e}"))?;
        let mut systems = HashMap::new();
        let mut hand = HashMap::from([
            ("ross154".to_string(), hand_ross154()),
            ("sol".to_string(), hand_sol()),
        ]);
        for s in &stars {
            let mut sys = if let Some(mut h) = hand.remove(&s.id) {
                h.position_ly = DVec3::new(s.x, s.y, s.z);
                h
            } else if s.id == "lave" {
                hand_lave(s)
            } else {
                seeded_system(s)
            };
            if let Some(ref e) = s.economy {
                sys.economy = e.clone();
            }
            if let Some(ref g) = s.government {
                sys.government = g.clone();
            }
            if let Some(t) = s.tech_level {
                sys.tech_level = t;
            }
            if let Some(ref a) = s.allegiance {
                sys.allegiance = a.clone();
            }
            systems.insert(s.id.clone(), sys);
        }
        Ok(Self { systems })
    }

    pub fn get(&self, system_id: &str) -> &StarSystem {
        &self.systems[system_id]
    }

    pub fn distance_ly(&self, a: &str, b: &str) -> f64 {
        (self.systems[a].position_ly - self.systems[b].position_ly).length()
    }

    pub fn neighbors(&self, system_id: &str, max_ly: f64) -> Vec<(String, f64)> {
        let mut out = Vec::new();
        for sid in self.systems.keys() {
            if sid == system_id {
                continue;
            }
            let d = self.distance_ly(system_id, sid);
            if d <= max_ly {
                out.push((sid.clone(), d));
            }
        }
        out.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
        out
    }
}

pub fn build_frame_system(system: &StarSystem) -> FrameSystem {
    let mut frames = FrameSystem::new();
    frames.add(BodyRef {
        id: "star".into(),
        name: system.name.clone(),
        radius_km: (100000.0 * system.mass).max(50000.0),
        world_pos: WorldPos::Fixed(DVec3::ZERO),
        parent_id: None,
        is_station: false,
        is_starport: false,
        atmosphere_km: 0.0,
        spin_period_days: 0.0,
        spin_phase: 0.0,
    });

    for p in &system.planets {
        if p.parent_id.is_some() {
            continue;
        }
        frames.add(BodyRef {
            id: p.id.clone(),
            name: p.name.clone(),
            radius_km: p.radius_km,
            world_pos: WorldPos::Orbit {
                elements: p.elements.clone(),
            },
            parent_id: Some("star".into()),
            is_station: false,
            is_starport: false,
            atmosphere_km: p.atmosphere_km,
            spin_period_days: p.rotation_days,
            spin_phase: 0.0,
        });
    }

    for p in &system.planets {
        let Some(ref parent_id) = p.parent_id else {
            continue;
        };
        frames.add(BodyRef {
            id: p.id.clone(),
            name: p.name.clone(),
            radius_km: p.radius_km,
            world_pos: WorldPos::CircleAround {
                parent_id: parent_id.clone(),
                radius_km: p.moon_orbit_km,
                period_days: p.moon_period_days,
                phase0: p.moon_phase0,
            },
            parent_id: Some(parent_id.clone()),
            is_station: false,
            is_starport: false,
            atmosphere_km: p.atmosphere_km,
            spin_period_days: p.rotation_days,
            spin_phase: 0.0,
        });
    }

    for p in &system.planets {
        for sp in &p.starports {
            let off = surface_offset(sp.lat_deg, sp.lon_deg, p.radius_km);
            frames.add(BodyRef {
                id: sp.id.clone(),
                name: sp.name.clone(),
                radius_km: 2.0,
                world_pos: WorldPos::Surface {
                    parent_id: p.id.clone(),
                    offset: off,
                    spin_period_days: p.rotation_days,
                },
                parent_id: Some(p.id.clone()),
                is_station: false,
                is_starport: true,
                atmosphere_km: 0.0,
                spin_period_days: 0.0,
                spin_phase: 0.0,
            });
        }
    }

    for st in &system.stations {
        frames.add(BodyRef {
            id: st.id.clone(),
            name: st.name.clone(),
            radius_km: st.radius_km,
            world_pos: WorldPos::CircleAround {
                parent_id: st.parent_planet_id.clone(),
                radius_km: st.orbit_radius_km,
                period_days: st.orbit_period_days,
                phase0: st.phase0,
            },
            parent_id: Some(st.parent_planet_id.clone()),
            is_station: true,
            is_starport: false,
            atmosphere_km: 0.0,
            spin_period_days: 0.0,
            spin_phase: 0.0,
        });
    }

    frames
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ross154_has_station() {
        let g = Galaxy::load().expect("galaxy");
        let sys = g.get("ross154");
        assert!(sys
            .planets
            .iter()
            .any(|p| p.name == "Aster" && p.is_gas_giant));
        let merlin = sys.planets.iter().find(|p| p.name == "Merlin").unwrap();
        assert_eq!(merlin.parent_id.as_deref(), Some("ross154_aster"));
        assert!(merlin
            .starports
            .iter()
            .any(|sp| sp.name == "Sirocco Station"));
        assert!(!sys.stations.is_empty());
        let frames = build_frame_system(sys);
        assert!(frames.bodies.contains_key("ross154_station"));
        assert!(frames.bodies.contains_key("ross154_sirocco"));
        assert!(frames.bodies.contains_key("ross154_aster"));
        let aster = frames.world_position("ross154_aster", 0.0);
        let merlin_pos = frames.world_position("ross154_merlin", 0.0);
        assert!((merlin_pos - aster).length() < 300000.0);
        assert!(merlin_pos.length() > 1e6);
    }
}
