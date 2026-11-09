//! Shipyard: buy ships, fit/remove equipment, capacity accounting.

use crate::flight::ShipStats;
use crate::frontier::{drive_h, max_range_ly, SHIELD_TONS};
use crate::paths::data_dir;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug)]
pub struct EquipDef {
    pub id: String,
    pub name: String,
    pub category: String,
    pub mass_t: f64,
    /// Credits to fit.
    pub price: i32,
    /// Credits paid back when the shipyard removes it.
    pub removal: i32,
    pub tech_min: i32,
    pub slot: String,
    /// Extra JSON fields (power, fire_rate, drive_id, ...).
    pub raw: serde_json::Map<String, Value>,
}

impl EquipDef {
    pub fn raw_f64(&self, key: &str, default: f64) -> f64 {
        self.raw.get(key).and_then(Value::as_f64).unwrap_or(default)
    }
    pub fn raw_str(&self, key: &str) -> Option<&str> {
        self.raw.get(key).and_then(Value::as_str)
    }
    pub fn raw_bool(&self, key: &str) -> bool {
        self.raw.get(key).and_then(Value::as_bool).unwrap_or(false)
    }
}

#[derive(Clone, Debug, Deserialize)]
pub struct ShipDef {
    pub id: String,
    pub name: String,
    pub allegiance: Vec<String>,
    pub mass_t: f64,
    pub capacity_t: f64,
    pub price: i32,
    pub crew: i32,
    pub missiles: i32,
    pub drive: String,
    pub jump_ly_base: f64,
    pub thrust_g: f64,
    pub retro_g: f64,
    pub gun_mounts: Vec<String>,
    pub scoop_capable: bool,
    pub tech_min: i32,
    /// Drive classes the hull can mount, inclusive. 0 means not read off a screen:
    /// the view then shows a range only for the drive the ship is sold with.
    #[serde(default)]
    pub drive_min: i32,
    #[serde(default)]
    pub drive_max: i32,
}

fn load_ships() -> Result<HashMap<String, ShipDef>, String> {
    let path = data_dir().join("ships.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let list: Vec<ShipDef> = serde_json::from_str(&text).map_err(|e| format!("ships.json: {e}"))?;
    Ok(list.into_iter().map(|s| (s.id.clone(), s)).collect())
}

fn load_equipment() -> Result<HashMap<String, EquipDef>, String> {
    let path = data_dir().join("equipment.json");
    let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
    let list: Vec<serde_json::Map<String, Value>> =
        serde_json::from_str(&text).map_err(|e| format!("equipment.json: {e}"))?;
    let mut out = HashMap::new();
    for mut d in list {
        let take_s = |d: &mut serde_json::Map<String, Value>, k: &str| -> String {
            d.remove(k).and_then(|v| v.as_str().map(String::from)).unwrap_or_default()
        };
        let id = take_s(&mut d, "id");
        let name = take_s(&mut d, "name");
        let category = take_s(&mut d, "category");
        let slot = take_s(&mut d, "slot");
        let mass_t = d.remove("mass_t").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let price = d.remove("price").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let removal = d.remove("removal").and_then(|v| v.as_i64()).map_or(price * 9 / 10, |v| v as i32);
        let tech_min = d.remove("tech_min").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        out.insert(
            id.clone(),
            EquipDef { id, name, category, mass_t, price, removal, tech_min, slot, raw: d },
        );
    }
    Ok(out)
}

/// Player-owned hull with fitted equipment and cargo.
#[derive(Clone, Debug)]
pub struct FittedShip {
    pub ship_id: String,
    pub equipment: Vec<String>,
    pub guns: BTreeMap<String, String>,
    pub missile_load: Vec<String>,
    pub cargo: BTreeMap<String, i32>,
    pub bonus_capacity: f64,
    pub fuel_t: f64,
    pub hull: f64,
    pub max_hull: f64,
}

impl FittedShip {
    pub fn equip_mass(&self, equip: &HashMap<String, EquipDef>) -> f64 {
        let mut total = 0.0;
        for eid in &self.equipment {
            let e = &equip[eid];
            if e.slot == "gun" && self.guns.values().any(|g| g == eid) {
                continue;
            }
            if e.slot == "missile" {
                continue;
            }
            total += e.mass_t;
        }
        for eid in self.guns.values() {
            total += equip[eid].mass_t;
        }
        for eid in &self.missile_load {
            total += equip[eid].mass_t;
        }
        total
    }

    pub fn cargo_mass(&self) -> f64 {
        self.cargo.values().map(|&v| v as f64).sum()
    }

    pub fn used_capacity(&self, equip: &HashMap<String, EquipDef>) -> f64 {
        self.equip_mass(equip) + self.cargo_mass() + self.fuel_t
    }

    pub fn free_cargo(&self, ships: &HashMap<String, ShipDef>, equip: &HashMap<String, EquipDef>) -> f64 {
        let cap = ships[&self.ship_id].capacity_t + self.bonus_capacity;
        (cap - self.used_capacity(equip)).max(0.0)
    }

    pub fn has(&self, equip_id: &str) -> bool {
        self.equipment.iter().any(|e| e == equip_id) || self.guns.values().any(|g| g == equip_id)
    }

    pub fn hydrogen_t(&self) -> i32 {
        self.cargo.get("hydrogen_fuel").copied().unwrap_or(0)
    }
}

/// Starting ship, as the DOS original's opening: Eagle with 1MW pulse laser, class 1
/// drive, two KL760 missiles, and 1t of Hydrogen Fuel in the hold (Cargo Space 3/1).
/// Equipment is in the order the status screen lists it.
pub fn default_eagle() -> FittedShip {
    FittedShip {
        ship_id: "eagle_mk1".into(),
        equipment: [
            "drive_class1",
            "pulse_1mw",
            "missile_homing",
            "missile_homing",
            "scanner",
            "autopilot",
            "atmo_shield",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        guns: BTreeMap::from([("front".into(), "pulse_1mw".into())]),
        missile_load: vec!["missile_homing".into(), "missile_homing".into()],
        cargo: BTreeMap::from([("hydrogen_fuel".into(), 1)]),
        bonus_capacity: 0.0,
        fuel_t: 1.0,
        hull: 80.0,
        max_hull: 80.0,
    }
}

/// Derive live flight/combat stats from hull + fittings.
pub fn fitted_to_stats(
    fitted: &FittedShip,
    ships: &HashMap<String, ShipDef>,
    equip: &HashMap<String, EquipDef>,
) -> ShipStats {
    let hull = &ships[&fitted.ship_id];

    let mut laser_power = 8.0;
    let mut fire_rate = 1.6;
    let mut beam = false;
    let mut mining = false;
    if let Some(front) = fitted.guns.get("front").or_else(|| fitted.guns.values().next()) {
        let g = &equip[front];
        laser_power = g.raw_f64("power", 8.0);
        fire_rate = g.raw_f64("fire_rate", 1.6);
        beam = g.raw_bool("beam");
        mining = g.raw_bool("mining");
    }
    let shield = fitted.equipment.iter().filter(|e| *e == "shield_gen").count() as f64 * SHIELD_TONS;

    let mut max_fuel = 1.0;
    let mut drive_id = "interplanetary".to_string();
    for eid in &fitted.equipment {
        let e = &equip[eid];
        if e.slot == "drive" {
            drive_id = e.raw_str("drive_id").unwrap_or("interplanetary").to_string();
            max_fuel = e.raw_f64("internal_fuel_t", 1.0);
        }
    }
    let total_mass =
        hull.mass_t + fitted.equip_mass(equip) + fitted.cargo_mass() + fitted.fuel_t.min(max_fuel);
    let h = drive_h(Some(&drive_id));
    let laden = hull.mass_t + hull.capacity_t;
    let formula = max_range_ly(total_mass, h);
    let formula_laden = max_range_ly(laden, h);
    let jump = if h > 0.0 && formula_laden > 0.0 && (formula_laden - hull.jump_ly_base).abs() > 0.2 {
        hull.jump_ly_base * laden / total_mass.max(1.0)
    } else {
        formula
    };

    let cargo_cap = (fitted.free_cargo(ships, equip) + fitted.cargo_mass()) as i32;

    ShipStats {
        name: hull.name.clone(),
        mass_t: total_mass,
        main_g: hull.thrust_g,
        retro_g: hull.retro_g,
        lateral_g: hull.thrust_g * 0.2,
        max_fuel_t: max_fuel,
        fuel_t: fitted.fuel_t.min(max_fuel),
        jump_range_ly: jump,
        hull: fitted.hull,
        max_hull: fitted.max_hull,
        shield,
        max_shield: shield,
        energy: 100.0,
        max_energy: 100.0,
        laser_power,
        has_missile: hull.missiles > 0,
        missiles: fitted.missile_load.len() as i32,
        has_docking_computer: fitted.has("docking_computer") || fitted.has("autopilot"),
        cargo_capacity: cargo_cap.max(0),
        cargo: fitted.cargo.iter().map(|(k, v)| (k.clone(), *v)).collect(),
        price: hull.price,
        energy_booster: fitted.has("energy_booster"),
        has_life_support: fitted.has("life_support"),
        has_fuel_scoop: fitted.has("fuel_scoop"),
        has_atmo_shield: fitted.has("atmo_shield"),
        fire_rate,
        beam_laser: beam,
        mining_laser: mining,
        drive_h: h,
    }
}

/// Kept for API parity with Python; flags are already set by [`fitted_to_stats`].
pub fn apply_equipment_flags(stats: ShipStats, _fitted: &FittedShip, _equip: &HashMap<String, EquipDef>) -> ShipStats {
    stats
}

pub struct Shipyard {
    pub ships: HashMap<String, ShipDef>,
    pub equipment: HashMap<String, EquipDef>,
}

impl Shipyard {
    pub fn load() -> Result<Self, String> {
        Ok(Self { ships: load_ships()?, equipment: load_equipment()? })
    }

    pub fn stats(&self, fitted: &FittedShip) -> ShipStats {
        fitted_to_stats(fitted, &self.ships, &self.equipment)
    }

    pub fn free_cargo(&self, fitted: &FittedShip) -> f64 {
        fitted.free_cargo(&self.ships, &self.equipment)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::Galaxy;

    #[test]
    fn full_ship_roster() {
        let y = Shipyard::load().unwrap();
        assert!(y.ships.len() >= 29);
        for id in ["eagle_mk1", "anaconda", "panther"] {
            assert!(y.ships.contains_key(id), "missing {id}");
        }
    }

    #[test]
    fn equipment_catalog() {
        let y = Shipyard::load().unwrap();
        for id in ["pulse_1mw", "drive_class1", "fuel_scoop", "shield_gen"] {
            assert!(y.equipment.contains_key(id), "missing {id}");
        }
        assert!(y.equipment.len() >= 40);
    }

    #[test]
    fn eagle_cannot_reach_sol() {
        let g = Galaxy::load().unwrap();
        let dist = g.distance_ly("ross154", "sol");
        let y = Shipyard::load().unwrap();
        let stats = y.stats(&default_eagle());
        assert!(stats.jump_range_ly < dist, "jump {} >= dist {}", stats.jump_range_ly, dist);
        assert!(stats.jump_range_ly > 0.0);
    }

    #[test]
    fn eagle_starts_with_3t_free_1t_used() {
        // DOS original at Sirocco: "Cargo Space 3 1", the 1t being Hydrogen Fuel.
        let y = Shipyard::load().unwrap();
        let e = default_eagle();
        assert!((y.free_cargo(&e) - 3.0).abs() < 0.01, "free cargo {}", y.free_cargo(&e));
        assert_eq!(e.cargo_mass(), 1.0);
    }

    #[test]
    fn sirocco_upgrade_prices() {
        let y = Shipyard::load().unwrap();
        let e = |id: &str| (y.equipment[id].price, y.equipment[id].removal, y.equipment[id].mass_t as i32);
        assert_eq!(e("missile_homing"), (60, 60, 1));
        assert_eq!(e("atmo_shield"), (240, 120, 1));
        assert_eq!(e("drive_class1"), (9600, 9000, 10));
        assert_eq!(e("passenger_cabin"), (1800, 1200, 5));
    }
}
