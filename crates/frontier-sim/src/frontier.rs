//! Documented Frontier: Elite II formulas (FAQ VII-C, XI-B, XI-F).

use std::collections::{HashMap, HashSet, VecDeque};

pub const SHIELD_TONS: f64 = 16.0;
pub const PULSE_TONS_PER_MW: f64 = 12.5;
pub const SHIELD_RECHARGE: f64 = 0.02;
pub const BOOSTER_RECHARGE: f64 = 0.03;

/// FAQ XI-C equipment masses (tonnes).
pub fn faq_mass_table() -> HashMap<&'static str, f64> {
    [
        ("mine_dummy", 1.0),
        ("mine_proximity", 1.0),
        ("energy_bomb", 4.0),
        ("missile_homing", 1.0),
        ("missile_smart", 1.0),
        ("missile_navy", 1.0),
        ("mining_laser", 10.0),
        ("pulse_1mw", 1.0),
        ("pulse_5mw", 3.0),
        ("beam_1mw", 5.0),
        ("beam_4mw", 20.0),
        ("beam_20mw", 75.0),
        ("beam_100mw", 200.0),
        ("laser_cooler", 1.0),
        ("plasma_small", 500.0),
        ("plasma_large", 900.0),
        ("ecm", 2.0),
        ("ecm_naval", 2.0),
        ("life_support", 1.0),
        ("passenger_cabin", 5.0),
        ("autopilot", 1.0),
        ("auto_refueller", 1.0),
        ("radar_mapper", 1.0),
        ("cloud_analyser", 1.0),
        ("shield_gen", 4.0),
        ("energy_booster", 5.0),
        ("escape_capsule", 5.0),
        ("atmo_shield", 1.0),
        ("hull_repair", 40.0),
        ("fuel_scoop", 6.0),
        ("cargo_scoop", 2.0),
        ("mining_machine", 30.0),
        ("drive_ip", 4.0),
        ("drive_class1", 10.0),
        ("drive_class2", 25.0),
        ("drive_class3", 45.0),
        ("drive_class4", 80.0),
        ("drive_class5", 150.0),
        ("drive_class6", 250.0),
        ("drive_class7", 400.0),
        ("drive_class8", 600.0),
        ("drive_mil1", 6.0),
        ("drive_mil2", 12.0),
        ("drive_mil3", 24.0),
    ]
    .into_iter()
    .collect()
}

/// Alias used by callers that expect `FAQ_MASS_T`.
pub fn faq_mass(id: &str) -> Option<f64> {
    faq_mass_table().get(id).copied()
}

pub const FAQ_MASS_T: &[(&str, f64)] = &[
    ("mine_dummy", 1.0),
    ("drive_class1", 10.0),
    ("drive_class8", 600.0),
    ("shield_gen", 4.0),
    ("pulse_1mw", 1.0),
];

pub const COMBAT_RANKS: &[(i32, &str)] = &[
    (0, "Harmless"),
    (4, "Mostly Harmless"),
    (8, "Poor"),
    (16, "Below Average"),
    (32, "Average"),
    (64, "Above Average"),
    (128, "Competent"),
    (1000, "Dangerous"),
    (3000, "Deadly"),
    (6000, "ELITE"),
];

fn digits(s: &str) -> u32 {
    s.chars()
        .filter(|c| c.is_ascii_digit())
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

fn civil_h(n: u32) -> f64 {
    if n == 8 {
        9.0
    } else {
        n as f64
    }
}

/// Hyperdrive rating h. Zero for an interplanetary drive.
pub fn drive_h(drive_id: Option<&str>) -> f64 {
    let Some(id) = drive_id else {
        return 0.0;
    };
    if id.is_empty() || id == "interplanetary" || id == "drive_ip" {
        return 0.0;
    }
    if id.starts_with("military_") || id.starts_with("drive_mil") {
        return digits(id) as f64;
    }
    if id.starts_with("class_") || id.starts_with("drive_class") {
        return civil_h(digits(id));
    }
    0.0
}

/// d_max = 200 * h^2 / m.
pub fn max_range_ly(mass_t: f64, h: f64) -> f64 {
    if h <= 0.0 || mass_t <= 0.0 {
        0.0
    } else {
        200.0 * (h * h) / mass_t
    }
}

/// c = ceil(d * m / 200). At least 1 t for any real jump.
pub fn jump_fuel_t(distance_ly: f64, mass_t: f64) -> i32 {
    if distance_ly <= 0.0 || mass_t <= 0.0 {
        0
    } else {
        ((distance_ly * mass_t / 200.0 - 1e-9).ceil() as i32).max(1)
    }
}

/// t = 168/200 * d * m / h^2 hours.
pub fn jump_hours(distance_ly: f64, mass_t: f64, h: f64) -> f64 {
    if h <= 0.0 || mass_t <= 0.0 {
        0.0
    } else {
        (168.0 / 200.0) * distance_ly * mass_t / (h * h)
    }
}

pub fn combat_rank(kills: i32) -> &'static str {
    let mut name = "Harmless";
    for &(need, label) in COMBAT_RANKS {
        if kills >= need {
            name = label;
        }
    }
    name
}

pub fn apply_shield_hit(shield_tons: f64, max_tons: f64, damage_tons: f64) -> (f64, f64) {
    if max_tons <= 0.0 || shield_tons <= 0.0 {
        return (0.0, damage_tons);
    }
    let absorbed = shield_tons.min(damage_tons);
    (shield_tons - absorbed, damage_tons - absorbed)
}

pub fn laser_damage_tons(megawatts: f64, beam: bool, mining: bool) -> f64 {
    if mining {
        return 1000.0;
    }
    let per_shot = PULSE_TONS_PER_MW * megawatts;
    if beam {
        per_shot * 0.8
    } else {
        per_shot
    }
}

/// Shortest hop-count path where each jump is inside `range_of(system)`.
pub fn plan_route<F, N>(
    start: &str,
    goal: &str,
    range_of: F,
    neighbors: N,
    max_hops: usize,
) -> Option<Vec<String>>
where
    F: Fn(&str) -> f64,
    N: Fn(&str, f64) -> Vec<(String, f64)>,
{
    if start == goal {
        return Some(vec![start.to_string()]);
    }
    let mut frontier: VecDeque<(String, Vec<String>)> =
        VecDeque::from([(start.to_string(), vec![start.to_string()])]);
    let mut seen = HashSet::from([start.to_string()]);
    while let Some((cur, path)) = frontier.pop_front() {
        if path.len() > max_hops {
            continue;
        }
        let hop = range_of(&cur);
        for (nid, dist) in neighbors(&cur, hop.max(0.1)) {
            if seen.contains(&nid) || dist > hop + 1e-6 {
                continue;
            }
            let mut nxt = path.clone();
            nxt.push(nid.clone());
            if nid == goal {
                return Some(nxt);
            }
            seen.insert(nid.clone());
            frontier.push_back((nid, nxt));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn class1_eagle_laden_range() {
        let r = max_range_ly(25.0, 1.0);
        assert!((r - 8.0).abs() < 1e-9);
    }

    /// The Constrictor's View screen, at its 120 t laden mass: 6.66, 15.00, 26.66.
    /// The original truncates. Class 1 and classes 5 to 7 read 0.00 on that screen.
    #[test]
    fn constrictor_view_ranges() {
        let show = |h| (max_range_ly(120.0, h) * 100.0).floor() / 100.0;
        assert!((show(2.0) - 6.66).abs() < 1e-9);
        assert!((show(3.0) - 15.0).abs() < 1e-9);
        assert!((show(4.0) - 26.66).abs() < 1e-9);
    }

    #[test]
    fn class8_is_h9() {
        assert!((drive_h(Some("drive_class8")) - 9.0).abs() < 1e-9);
    }
}
