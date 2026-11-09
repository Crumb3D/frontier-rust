//! Commodity markets. Prices are kept in tenths of a credit, as the original shows
//! them (`¢44.3`). The catalogue in `data/market.json` was read off the DOS
//! original at Sirocco on 1-Jan-3200, so Ross 154 on day 0 reproduces that screen
//! exactly; other systems and days scale it by economy and a slow daily drift.

use crate::galaxy::StarSystem;
use crate::paths::data_dir;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};

#[derive(Clone, Debug, Deserialize)]
pub struct Good {
    pub id: String,
    pub name: String,
    /// Tenths of a credit per tonne at the reference market.
    pub price: i32,
    /// Tonnes in stock at the reference market.
    pub stock: i32,
    #[serde(default)]
    pub needs_life_support: bool,
    #[serde(default)]
    pub disposal: bool,
    #[serde(default)]
    pub illegal_in: Vec<String>,
    #[serde(default)]
    pub is_drive_fuel: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct Reference {
    economy: String,
    tech_level: i32,
}

#[derive(Clone, Debug, Deserialize)]
struct Catalogue {
    reference: Reference,
    goods: Vec<Good>,
}

#[derive(Clone, Debug)]
pub struct MarketListing {
    pub good: Good,
    /// Tenths of a credit per tonne; buying and selling use the same price.
    pub price: i32,
    pub stock: i32,
}

pub fn system_faction(system: &StarSystem) -> &'static str {
    match system.allegiance.as_str() {
        "Federation" => return "Federation",
        "Empire" => return "Empire",
        "Independent" => return "Independent",
        _ => {}
    }
    match system.government.as_str() {
        "Democracy" | "Corporate" | "Communist" => "Federation",
        "Dictatorship" | "Feudal" => "Empire",
        _ => "Independent",
    }
}

/// Relative price shift by economy: negative where a good is produced locally.
fn economy_bias(economy: &str, good: &str) -> f64 {
    let table: &[(&str, f64)] = match economy {
        "Agricultural" => &[
            ("grain", -0.4),
            ("fruit_veg", -0.35),
            ("animal_meat", -0.3),
            ("fertilizer", -0.2),
            ("farm_machinery", 0.35),
            ("computers", 0.3),
            ("industrial_parts", 0.25),
            ("medicines", 0.15),
            ("luxury_goods", 0.2),
        ],
        "Industrial" => &[
            ("industrial_parts", -0.35),
            ("heavy_plastics", -0.3),
            ("metal_alloys", -0.25),
            ("computers", -0.15),
            ("farm_machinery", -0.1),
            ("minerals", 0.3),
            ("grain", 0.2),
            ("water", 0.15),
            ("luxury_goods", 0.1),
        ],
        "High-Tech" => &[
            ("computers", -0.4),
            ("robots", -0.3),
            ("medicines", -0.2),
            ("industrial_parts", -0.15),
            ("luxury_goods", -0.1),
            ("minerals", 0.25),
            ("precious_metals", 0.15),
            ("grain", 0.2),
            ("military_fuel", -0.1),
        ],
        "Extraction" | "Mining" => &[
            ("minerals", -0.45),
            ("precious_metals", -0.3),
            ("gem_stones", -0.25),
            ("metal_alloys", -0.2),
            ("radioactives", -0.15),
            ("farm_machinery", 0.2),
            ("air_processors", 0.25),
            ("liquid_oxygen", 0.2),
            ("water", 0.15),
            ("grain", 0.3),
        ],
        "Tourist" => &[
            ("luxury_goods", -0.25),
            ("liquor", -0.2),
            ("animal_skins", -0.15),
            ("medicines", 0.1),
            ("computers", 0.15),
            ("narcotics", 0.1),
        ],
        "Colony" => &[
            ("grain", -0.15),
            ("water", -0.2),
            ("liquid_oxygen", -0.15),
            ("synthetic_meat", -0.1),
            ("farm_machinery", 0.3),
            ("industrial_parts", 0.25),
            ("medicines", 0.2),
            ("air_processors", 0.15),
            ("computers", 0.2),
        ],
        _ => &[],
    };
    table.iter().find(|(g, _)| *g == good).map_or(0.0, |(_, b)| *b)
}

fn str_hash(s: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(16777619);
    }
    h
}

/// A slow wobble that is exactly zero on day 0.
fn drift(seed: u32, day: i64, rate: f64, amp: f64) -> f64 {
    let ph = (seed % 6283) as f64 / 1000.0;
    amp * ((day as f64 * rate + ph).sin() - ph.sin())
}

pub struct Market {
    pub goods: Vec<Good>,
    pub goods_by_id: HashMap<String, Good>,
    reference: Reference,
}

impl Market {
    pub fn load() -> Result<Self, String> {
        let path = data_dir().join("market.json");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let cat: Catalogue = serde_json::from_str(&text).map_err(|e| format!("parse market.json: {e}"))?;
        let goods_by_id = cat.goods.iter().map(|g| (g.id.clone(), g.clone())).collect();
        Ok(Self { goods: cat.goods, goods_by_id, reference: cat.reference })
    }

    pub fn is_legal(&self, system: &StarSystem, good: &Good) -> bool {
        let faction = system_faction(system);
        !good.illegal_in.iter().any(|f| f == faction)
    }

    /// Price and stock of one good in `system` on `day` (days since 1-Jan-3200),
    /// whether or not it may be traded openly there.
    pub fn quote(&self, system: &StarSystem, day: i64, good: &Good) -> (i32, i32) {
        let refb = economy_bias(&self.reference.economy, &good.id);
        let b = economy_bias(&system.economy, &good.id);
        let tech = 1.0 - (system.tech_level - self.reference.tech_level) as f64 * 0.01;
        let seed = system.seed ^ str_hash(&good.id);
        let p = good.price as f64 * (1.0 + b) / (1.0 + refb) * tech * (1.0 + drift(seed, day, 0.61, 0.04));
        let s = good.stock as f64 * (1.0 - 2.0 * (b - refb)).max(0.1) * (1.0 + drift(seed >> 3, day, 0.43, 0.25));
        let price = p.round() as i32;
        let price = if good.disposal { price.min(-1) } else { price.max(1) };
        (price, s.round().max(0.0) as i32)
    }

    /// The stockmarket screen: legal goods in the original's order.
    pub fn listings(&self, system: &StarSystem, day: i64) -> Vec<MarketListing> {
        self.goods
            .iter()
            .filter(|g| self.is_legal(system, g))
            .map(|g| {
                let (price, stock) = self.quote(system, day, g);
                MarketListing { good: g.clone(), price, stock }
            })
            .collect()
    }

    pub fn price_map(&self, system: &StarSystem, day: i64) -> BTreeMap<String, i32> {
        self.listings(system, day).into_iter().map(|l| (l.good.id, l.price)).collect()
    }

    pub fn can_carry(&self, good_id: &str, has_life_support: bool) -> Result<(), String> {
        let Some(g) = self.goods_by_id.get(good_id) else {
            return Err("Unknown good".into());
        };
        if g.needs_life_support && !has_life_support {
            return Err("Requires cargo bay life support".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::galaxy::Galaxy;

    #[test]
    fn ross154_prices_stable() {
        let g = Galaxy::load().unwrap();
        let m = Market::load().unwrap();
        let sys = g.get("ross154");
        assert_eq!(m.price_map(sys, 3), m.price_map(sys, 3));
        assert_ne!(m.price_map(sys, 0), m.price_map(sys, 9));
    }

    #[test]
    fn goods_catalog() {
        let m = Market::load().unwrap();
        for id in ["hydrogen_fuel", "slaves", "radioactives", "military_fuel"] {
            assert!(m.goods_by_id.contains_key(id), "missing {id}");
        }
        assert_eq!(m.goods.len(), 31);
    }

    /// The DOS original's Sirocco Station Stockmarket at 12:00 1-Jan-3200.
    #[test]
    fn sirocco_day_one() {
        let g = Galaxy::load().unwrap();
        let m = Market::load().unwrap();
        let l = m.listings(g.get("ross154"), 0);
        assert_eq!(l.len(), 25);
        let row = |i: usize| (l[i].good.name.as_str(), l[i].price, l[i].stock);
        assert_eq!(row(0), ("Water", 10, 4678));
        assert_eq!(row(3), ("Fruit and Veg.", 1264, 0));
        assert_eq!(row(14), ("Gem Stones", 30987, 1));
        assert_eq!(row(23), ("Radioactives", -43, 226));
        assert_eq!(row(24), ("Rubbish", -7, 907));
    }
}
