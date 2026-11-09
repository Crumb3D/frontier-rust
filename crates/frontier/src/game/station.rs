//! Starport services over F4: the station menu, stockmarket, shipyard and police,
//! laid out and worded after the DOS original's screens at Sirocco Station.

use super::bulletin::{Ad, Mission};
use super::{Game, Panel, Where};
use crate::fb::{Canvas, Rect};
use crate::font::{credits, money};
use crate::palette;
use crate::screens::{self, Hold};
use frontier_sim::market::MarketListing;
use frontier_sim::{FittedShip, StarSystem};
use std::collections::BTreeMap;

/// Rows on the stockmarket and upgrade lists.
const LIST_ROWS: usize = 14;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Services,
    Stock,
    Bulletin,
    Video(usize),
    FederalHq,
    Shipyard,
    Upgrades,
    Repairs,
    Ships,
    ShipView(usize),
    Police,
}

#[derive(Clone, Copy, Debug)]
pub enum Hit {
    Option(usize),
    Buy(usize),
    Sell(usize),
    Scroll(i32),
    Back,
    Ad(usize),
    Reply(usize),
}

/// What a starport offers today.
pub struct Port {
    pub id: String,
    pub day: i64,
    pub market: Vec<MarketListing>,
    pub ads: Vec<Ad>,
    pub missions: Vec<Mission>,
    pub ships: Vec<String>,
    pub upgrades: Vec<String>,
}

pub const SERVICES: [(&str, &[&str]); 4] = [
    ("Launch Request", &["[Request launch permission from traffic control]"]),
    ("Shipyard", &["[Ships refitted, repaired, bought and sold]", "[Also contact with local Police]"]),
    ("Bulletin Board", &["[Advertisements for goods and services]"]),
    ("Stockmarket", &["[Direct trading on the local market]"]),
];

const SHIPYARD: [(&str, &[&str]); 4] = [
    ("Upgrades", &["[Ship equipment fitted while you wait]"]),
    ("Repairs and Servicing", &["[in our fully automated workshop]"]),
    ("New and Reconditioned Ships", &["[Ships part exchanged]"]),
    ("Contact Local Police", &["[For paying fines, etc]"]),
];

fn hash(a: &str, n: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ n.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for b in a.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100_0000_01b3);
    }
    h ^ (h >> 29)
}

impl Game {
    pub(super) fn port_id(&self) -> Option<String> {
        match &self.wh {
            Where::Landed { port: Some(p), .. } => Some(p.clone()),
            Where::Docked { station } => Some(station.clone()),
            _ => None,
        }
    }

    pub(super) fn day(&self) -> i64 {
        self.stardate.days.floor() as i64
    }

    /// The station's market, adverts and shipyard, made once per port per day.
    pub(super) fn ensure_port(&mut self) {
        let Some(id) = self.port_id() else { return };
        let day = self.day();
        if self.port.as_ref().is_some_and(|p| p.id == id && p.day == day) {
            return;
        }
        let sys = self.galaxy.get(&self.sys_id).clone();
        let market = self.market.listings(&sys, day);
        let observed = self.observed.port == id && day == 0;
        let (ads, missions) = if observed {
            self.observed.ads(&self.market, &sys, day)
        } else {
            self.observed.random_ads(&self.market, &sys, day, &id)
        };
        let ships = if observed { self.observed.ships.clone() } else { self.random_ships(&sys, day, &id) };
        let upgrades = if observed { self.observed.upgrades.clone() } else { self.random_upgrades(&sys) };
        self.port = Some(Port { id, day, market, ads, missions, ships, upgrades });
    }

    fn random_ships(&self, sys: &StarSystem, day: i64, id: &str) -> Vec<String> {
        let faction = frontier_sim::market::system_faction(sys);
        let mut pool: Vec<&String> = self
            .yard
            .ships
            .values()
            .filter(|s| s.tech_min <= sys.tech_level && s.allegiance.iter().any(|a| a == faction) && s.id != "eagle_mk1")
            .map(|s| &s.id)
            .collect();
        pool.sort();
        if pool.is_empty() {
            return vec![];
        }
        let h = hash(id, day as u64);
        let n = 2 + (h % 4) as usize;
        (0..n).map(|i| pool[(hash(id, h ^ i as u64) % pool.len() as u64) as usize].clone()).collect()
    }

    fn random_upgrades(&self, sys: &StarSystem) -> Vec<String> {
        let mut v: Vec<&frontier_sim::EquipDef> = self
            .yard
            .equipment
            .values()
            .filter(|e| e.tech_min <= sys.tech_level && !matches!(e.category.as_str(), "special"))
            .collect();
        v.sort_by_key(|e| (e.price, e.id.clone()));
        v.into_iter().take(LIST_ROWS).map(|e| e.id.clone()).collect()
    }

    pub(super) fn port_title(&self) -> String {
        self.place_name()
    }

    pub(super) fn hold(&self) -> Hold {
        let cabins = self.fitted.equipment.iter().filter(|e| *e == "passenger_cabin").count() as i32;
        let used = self.cabins_used();
        Hold {
            free: self.yard.free_cargo(&self.fitted).round() as i32,
            used: self.fitted.cargo_mass().round() as i32,
            cabins_free: cabins - used,
            cabins_used: used,
            cash: self.cash,
        }
    }

    pub(super) fn cabins_used(&self) -> i32 {
        self.contracts.iter().map(|c| c.passengers).sum()
    }

    /// Open a station page and put its prompt in the console.
    pub(super) fn open_page(&mut self, page: Page) {
        self.ensure_port();
        self.page = page;
        self.scroll = 0;
        let prompt: &[&str] = match page {
            Page::Services | Page::Shipyard => &["Which service do you require?"],
            Page::Stock => &["Choose Buy or Sell to trade one unit."],
            Page::Bulletin => &["Please choose advertisement. Direct link will be made with advertiser."],
            Page::Upgrades => &["Choose Fit or Remove for required item."],
            Page::Repairs => {
                &["Select desired repair or service. Remember servicing AT LEAST once a year is a wise precaution."]
            }
            Page::Ships => &[
                "Select Buy to part exchange your ship for a ship with no extra equipment, or View to see the ship with suggested options fitted.",
            ],
            Page::Police if self.fine_here() > 0 => &[],
            Page::Police => &["We have no business with you at the moment."],
            Page::FederalHq => &[
                "Listed above are mercenary missions on offer appropriate to your current security and combat ratings with us.",
            ],
            Page::Video(_) | Page::ShipView(_) => &[],
        };
        if matches!(page, Page::Police) && self.fine_here() > 0 {
            self.say(vec![(
                format!("Outstanding fine {}.", money(self.fine_here() * 10)),
                palette::CON_GREEN,
            )]);
        } else if !prompt.is_empty() {
            self.say(prompt.iter().map(|s| (s.to_string(), palette::CON_GREEN)).collect());
        }
        if let Page::Video(i) = page {
            self.talk = Default::default();
            let intro = self.ad_intro(i);
            self.say(vec![(intro, palette::CON_GREEN)]);
        }
    }

    pub(super) fn station_key_option(&mut self, i: usize) {
        match self.page {
            Page::Services | Page::Shipyard => self.station_hit(Hit::Option(i)),
            _ => {}
        }
    }

    pub(super) fn station_hit(&mut self, h: Hit) {
        match (self.page, h) {
            (Page::Services, Hit::Option(i)) => match i {
                0 => self.launch_request(),
                1 => self.open_page(Page::Shipyard),
                2 => self.open_page(Page::Bulletin),
                3 => self.open_page(Page::Stock),
                _ => {}
            },
            (Page::Shipyard, Hit::Option(i)) => match i {
                0 => self.open_page(Page::Upgrades),
                1 => self.open_page(Page::Repairs),
                2 => self.open_page(Page::Ships),
                3 => self.open_page(Page::Police),
                _ => {}
            },
            (Page::Stock, Hit::Buy(i)) => self.stock_buy(self.scroll + i),
            (Page::Stock, Hit::Sell(i)) => self.stock_sell(self.scroll + i),
            (Page::Upgrades, Hit::Buy(i)) => self.fit(i),
            (Page::Upgrades, Hit::Sell(i)) => self.remove(i),
            (Page::Repairs, Hit::Option(i)) => self.repair(i),
            (Page::Ships, Hit::Buy(i)) => self.buy_ship(i),
            (Page::Ships, Hit::Sell(i)) => self.page = Page::ShipView(i),
            (Page::ShipView(_), Hit::Back) => self.page = Page::Ships,
            (Page::Upgrades | Page::Repairs | Page::Ships, Hit::Back) => self.open_page(Page::Shipyard),
            (Page::Police, Hit::Reply(0)) if self.fine_here() > 0 => self.pay_fine(),
            (Page::Police, Hit::Reply(_)) => self.open_page(Page::Shipyard),
            (Page::Bulletin, Hit::Ad(i)) => self.open_ad(self.scroll + i),
            (Page::Video(ad), Hit::Reply(r)) => self.video_reply(ad, r),
            (Page::FederalHq, Hit::Reply(r)) => self.federal_reply(r),
            (_, Hit::Scroll(d)) => {
                let len = self.list_len();
                let page_rows = if self.page == Page::Bulletin { 4 } else { LIST_ROWS };
                let max = len.saturating_sub(page_rows);
                self.scroll = (self.scroll as i32 + d).clamp(0, max as i32) as usize;
            }
            _ => {}
        }
    }

    fn list_len(&self) -> usize {
        let Some(p) = &self.port else { return 0 };
        match self.page {
            Page::Stock => p.market.len(),
            Page::Bulletin => p.ads.iter().filter(|a| !a.gone).count(),
            _ => 0,
        }
    }

    fn pay_fine(&mut self) {
        let due = self.fine_here();
        let bill = due * 10;
        if bill <= 0 {
            self.open_page(Page::Police);
            return;
        }
        if !self.free_purchases && self.cash < bill {
            self.say(vec![(format!("Outstanding fine {}.", money(bill)), palette::CON_GREEN)]);
            return;
        }
        if !self.free_purchases {
            self.cash -= bill;
        }
        self.fines.remove(&self.system().government.clone());
        self.open_page(Page::Police);
    }

    fn launch_request(&mut self) {
        self.launch_clearance = true;
        self.panel = Panel::View;
        self.rear = false;
        let who = self.port_title();
        self.msg_from(&who, "Clearance granted. Seal airlocks and prepare for launching.");
    }

    // ------------------------------------------------------------ stockmarket

    fn stock_buy(&mut self, i: usize) {
        let Some(l) = self.port.as_ref().and_then(|p| p.market.get(i)).cloned() else { return };
        let name = l.good.name.clone();
        let free = self.yard.free_cargo(&self.fitted);
        let price = if self.free_purchases { 0 } else { l.price as i64 };
        let text = if l.stock <= 0 {
            format!("There is no {name} for sale.")
        } else if self.cash < price {
            format!("You cannot afford any {name}.")
        } else if free < 1.0 {
            "You have no room in your cargo bay.".to_string()
        } else if l.good.needs_life_support && !self.fitted.has("life_support") {
            "You need Cargo Bay Life Support to carry that.".to_string()
        } else {
            self.cash -= price;
            *self.fitted.cargo.entry(l.good.id.clone()).or_insert(0) += 1;
            if let Some(p) = self.port.as_mut() {
                p.market[i].stock -= 1;
            }
            format!("Buying {name}.")
        };
        self.say(vec![(text, palette::CON_GREEN)]);
    }

    fn stock_sell(&mut self, i: usize) {
        let Some(l) = self.port.as_ref().and_then(|p| p.market.get(i)).cloned() else { return };
        let name = l.good.name.clone();
        let have = self.fitted.cargo.get(&l.good.id).copied().unwrap_or(0);
        let text = if have <= 0 {
            format!("You have no {name} to sell.")
        } else if self.cash + (l.price as i64) < 0 {
            format!("You cannot afford to dispose of {name}.")
        } else {
            self.cash += l.price as i64;
            take_cargo(&mut self.fitted, &l.good.id);
            if let Some(p) = self.port.as_mut() {
                p.market[i].stock += 1;
            }
            format!("Selling {name}.")
        };
        self.say(vec![(text, palette::CON_GREEN)]);
    }

    // ------------------------------------------------------------ shipyard

    fn upgrade_id(&self, i: usize) -> Option<String> {
        self.port.as_ref().and_then(|p| p.upgrades.get(i)).cloned()
    }

    fn fit(&mut self, i: usize) {
        let Some(id) = self.upgrade_id(i) else { return };
        let e = self.yard.equipment[&id].clone();
        let hull = self.yard.ships[&self.fitted.ship_id].clone();
        let stackable = e.raw_bool("stackable") || e.slot == "missile";
        let price = if self.free_purchases { 0 } else { e.price as i64 * 10 };
        let text = if self.cash < price {
            format!("You cannot afford a new {}.", e.name)
        } else if e.slot == "missile" && self.fitted.missile_load.len() as i32 >= hull.missiles {
            "You have no empty missile positions.".to_string()
        } else if e.slot == "gun" && hull.gun_mounts.iter().all(|m| self.fitted.guns.contains_key(m)) {
            "You have no empty gun mountings.".to_string()
        } else if e.slot == "drive" && self.fitted.equipment.iter().any(|x| self.yard.equipment[x].slot == "drive") {
            "You must remove your present drive first.".to_string()
        } else if !stackable && e.slot != "gun" && self.fitted.has(&id) {
            format!("You already have a {}.", e.name)
        } else if self.yard.free_cargo(&self.fitted) < e.mass_t {
            format!("There is no room for a new {}.", e.name)
        } else {
            self.cash -= price;
            self.fitted.equipment.push(id.clone());
            if e.slot == "missile" {
                self.fitted.missile_load.push(id.clone());
            }
            if e.slot == "gun" {
                if let Some(m) = hull.gun_mounts.iter().find(|m| !self.fitted.guns.contains_key(*m)) {
                    self.fitted.guns.insert(m.clone(), id.clone());
                }
            }
            self.refit();
            // Wording not in the manual. Not confirmed.
            format!("Fitting {}.", e.name)
        };
        self.say(vec![(text, palette::CON_GREEN)]);
    }

    fn remove(&mut self, i: usize) {
        let Some(id) = self.upgrade_id(i) else { return };
        let e = self.yard.equipment[&id].clone();
        let Some(pos) = self.fitted.equipment.iter().position(|x| *x == id) else {
            self.say(vec![(format!("You have no {} to remove.", e.name), palette::CON_GREEN)]);
            return;
        };
        self.fitted.equipment.remove(pos);
        if let Some(p) = self.fitted.missile_load.iter().position(|x| *x == id) {
            self.fitted.missile_load.remove(p);
        }
        if let Some(m) = self.fitted.guns.iter().find(|(_, g)| **g == id).map(|(m, _)| m.clone()) {
            self.fitted.guns.remove(&m);
        }
        self.cash += e.removal as i64 * 10;
        self.refit();
        self.say(vec![(format!("Removing {}.", e.name), palette::CON_GREEN)]);
    }

    fn refit(&mut self) {
        let controls = self.fm.controls.clone();
        self.fm = frontier_sim::FlightModel::new(self.yard.stats(&self.fitted));
        self.fm.controls = controls;
    }

    /// Deluxe, drive-only and equipment-only service; 8% and 5% of the hull price
    /// (¢3040 and ¢1900 for the Eagle).
    fn repair_list(&self) -> Vec<(String, i64)> {
        let price = self.yard.ships[&self.fitted.ship_id].price as i64;
        let last = |d: Option<i64>| match d {
            Some(day) => frontier_sim::Stardate::new(day as f64).format().split(' ').nth(1).unwrap_or("").to_string(),
            None => "31-Dec-3199".to_string(),
        };
        vec![
            ("Deluxe service (drive and equipment)".into(), price * 8 / 100),
            (format!("Drive Only service (last: {})", last(self.serviced.0)), price * 5 / 100),
            (format!("Equipment Only service (last: {})", last(self.serviced.1)), price * 5 / 100),
        ]
    }

    fn repair(&mut self, i: usize) {
        let list = self.repair_list();
        let Some((_, cost)) = list.get(i).cloned() else { return };
        let bill = if self.free_purchases { 0 } else { cost * 10 };
        let text = if self.cash < bill {
            "You cannot afford that service.".to_string()
        } else {
            self.cash -= bill;
            let day = self.day();
            if i != 2 {
                self.serviced.0 = Some(day);
            }
            if i != 1 {
                self.serviced.1 = Some(day);
            }
            self.fitted.hull = self.fitted.max_hull;
            // Wording not in the manual. Not confirmed.
            "Service completed.".to_string()
        };
        self.say(vec![(text, palette::CON_GREEN)]);
    }

    /// The part-exchange value of the present hull: 95% of its list price.
    fn trade_in(&self) -> i64 {
        self.yard.ships[&self.fitted.ship_id].price as i64 * 95 / 100
    }

    fn buy_ship(&mut self, i: usize) {
        let Some(sid) = self.port.as_ref().and_then(|p| p.ships.get(i)).cloned() else { return };
        let s = self.yard.ships[&sid].clone();
        let cost = s.price as i64 - self.trade_in();
        let bill = if self.free_purchases && cost > 0 { 0 } else { cost * 10 };
        if self.cash < bill {
            self.say(vec![(format!("You cannot afford a new {}.", s.name), palette::CON_GREEN)]);
            return;
        }
        self.cash -= bill;
        let drive = self
            .yard
            .equipment
            .values()
            .find(|e| e.slot == "drive" && e.raw_str("drive_id") == Some(s.drive.as_str()))
            .map(|e| e.id.clone());
        let cargo: BTreeMap<String, i32> = std::mem::take(&mut self.fitted.cargo);
        self.fitted = FittedShip {
            ship_id: sid.clone(),
            equipment: drive.into_iter().collect(),
            guns: BTreeMap::new(),
            missile_load: vec![],
            cargo: BTreeMap::new(),
            bonus_capacity: 0.0,
            fuel_t: 1.0,
            hull: s.mass_t * 16.0,
            max_hull: s.mass_t * 16.0,
        };
        for (g, n) in cargo {
            let room = self.yard.free_cargo(&self.fitted).floor() as i32;
            if room > 0 {
                self.fitted.cargo.insert(g, n.min(room));
            }
        }
        self.refit();
        if let Some(p) = self.port.as_mut() {
            p.ships.remove(i);
        }
        self.say(vec![(format!("Your new {} is ready.", s.name), palette::CON_GREEN)]);
    }

    // ------------------------------------------------------------ drawing

    fn face(&self, kind: &str, n: usize) -> Option<&image::RgbaImage> {
        let pool = self.a.portrait_pool(kind);
        (!pool.is_empty()).then(|| pool[(n + self.port_visits) % pool.len()])
    }

    pub(super) fn draw_station(&mut self, cv: &mut Canvas) {
        self.ensure_port();
        let place = self.port_title();
        let (title, face) = match self.page {
            Page::Services => (format!("{place} Services"), self.face("station", 0)),
            Page::Stock => (format!("{place} Stockmarket"), self.face("civ", 0)),
            Page::Bulletin => (format!("{place} Bulletin Board"), self.face("station", 1)),
            Page::Video(i) => {
                let n = self.port.as_ref().map_or(0, |p| p.ads[i].portrait);
                (format!("{place} Video Link"), self.face("civ", n))
            }
            Page::FederalHq => (format!("{place} Federal HQ"), self.face("military", 0)),
            Page::Shipyard => (format!("{place} Shipyard"), self.face("shipyard", 0)),
            Page::Upgrades => (format!("{place} Shipyard"), self.face("shipyard", 1)),
            Page::Repairs => (format!("{place} Shipyard"), self.face("shipyard", 2)),
            Page::Ships => (format!("{place} Shipyard"), self.face("shipyard", 3)),
            // The original replaces the clerk with the ship being viewed.
            Page::ShipView(_) => (format!("{place} Shipyard"), None),
            Page::Police => (format!("{place} Shipyard"), self.face("police", 0)),
        };
        let face = face.cloned();
        screens::station_frame(cv, &self.a, &title, face.as_ref());
        let hold = self.hold();
        screens::hold_block(cv, &self.a, 0, &hold, palette::LILAC, palette::CREAM);
        let mut hits = Vec::new();
        match self.page {
            Page::Services => menu(cv, &self.a, &SERVICES, &mut hits),
            Page::Shipyard => menu(cv, &self.a, &SHIPYARD, &mut hits),
            Page::Stock => self.draw_stock(cv, &mut hits),
            Page::Upgrades => self.draw_upgrades(cv, &mut hits),
            Page::Repairs => self.draw_repairs(cv, &mut hits),
            Page::Ships => self.draw_ships(cv, &mut hits),
            Page::ShipView(i) => self.draw_ship_view(cv, i, &mut hits),
            Page::Police if self.fine_here() > 0 => {
                let rows = [
                    vec![format!("Pay fine {}", money(self.fine_here() * 10))],
                    vec!["Hang up.".to_string()],
                ];
                responses(cv, &self.a, &rows, 10, &mut hits);
            }
            Page::Police => responses(cv, &self.a, &[vec!["Hang up.".to_string()]], 10, &mut hits),
            Page::Bulletin => self.draw_bulletin(cv, &mut hits),
            Page::Video(i) => self.draw_video(cv, i, &mut hits),
            Page::FederalHq => self.draw_federal(cv, &mut hits),
        }
        cv.reset_clip();
        self.hits.extend(hits.into_iter().map(|(r, h)| (r, super::Action::Station(h))));
    }

    fn draw_stock(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>) {
        let a = &self.a;
        let li = palette::LILAC;
        for (s, x) in [("Buy", 120), ("Sell", 136), ("Item", 151), ("Price per t", 204), ("Stock", 250), ("Cargo", 284)] {
            screens::text(cv, a, s, x, 2, li);
        }
        let Some(p) = &self.port else { return };
        for (row, l) in p.market.iter().skip(self.scroll).take(LIST_ROWS).enumerate() {
            let y = 15 + 10 * row as i32;
            hits.push((screens::button(cv, 125, y - 1), Hit::Buy(row)));
            hits.push((screens::button(cv, 137, y - 1), Hit::Sell(row)));
            let c = palette::CREAM;
            screens::text(cv, a, &l.good.name, 152, y, c);
            screens::text(cv, a, &money(l.price as i64), 214, y, c);
            let stock = if l.stock > 0 { format!("{}t", l.stock) } else { "-".into() };
            screens::text(cv, a, &stock, 254, y, c);
            let have = self.fitted.cargo.get(&l.good.id).copied().unwrap_or(0);
            let cargo = if have > 0 { format!("{have}t") } else { "-".into() };
            screens::text(cv, a, &cargo, 290, y, c);
        }
        more_arrows(cv, a, self.scroll, p.market.len(), LIST_ROWS, hits);
    }

    fn draw_upgrades(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>) {
        let a = &self.a;
        let li = palette::LILAC;
        for (s, x) in [("Fit", 122), ("Remove", 133), ("Item", 164), ("¢ to Fit", 226), ("¢ for Removal", 248), ("Wt", 302)] {
            screens::text(cv, a, s, x, 2, li);
        }
        let Some(p) = &self.port else { return };
        for (row, id) in p.upgrades.iter().take(LIST_ROWS).enumerate() {
            let Some(e) = self.yard.equipment.get(id) else { continue };
            let y = 15 + 10 * row as i32;
            hits.push((screens::button(cv, 125, y - 1), Hit::Buy(row)));
            hits.push((screens::button(cv, 137, y - 1), Hit::Sell(row)));
            let c = palette::CREAM;
            screens::text(cv, a, &e.name, 152, y, c);
            screens::text(cv, a, &e.price.to_string(), 242, y, c);
            screens::text(cv, a, &e.removal.to_string(), 272, y, c);
            screens::text(cv, a, &format!("{}t", e.mass_t), 302, y, c);
        }
        go_back(cv, a, hits);
    }

    fn draw_repairs(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>) {
        let a = &self.a;
        screens::text(cv, a, "Repair", 120, 2, palette::LILAC);
        screens::text(cv, a, "Item", 149, 2, palette::LILAC);
        screens::text(cv, a, "Cost", 292, 2, palette::LILAC);
        for (row, (name, cost)) in self.repair_list().iter().enumerate() {
            let y = 15 + 10 * row as i32;
            hits.push((screens::button(cv, 125, y - 1), Hit::Option(row)));
            screens::text(cv, a, name, 140, y, palette::CREAM);
            screens::text_right(cv, a, &credits(*cost), 311, y, palette::CREAM);
        }
        go_back(cv, a, hits);
    }

    fn ship_headers(&self, cv: &mut Canvas) {
        let li = palette::LILAC;
        for (s, x) in [("Buy", 122), ("View", 137), ("Ship", 158), ("Cost", 236), ("Pt.Exchg", 268), ("Cap.", 302)] {
            screens::text(cv, &self.a, s, x, 2, li);
        }
    }

    /// One ship for sale. View (the Sell hit) opens its specification.
    fn ship_offer(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>, index: usize, y: i32) {
        let Some(sid) = self.port.as_ref().and_then(|p| p.ships.get(index)) else { return };
        let Some(s) = self.yard.ships.get(sid) else { return };
        let trade = self.trade_in();
        let a = &self.a;
        let c = palette::CREAM;
        hits.push((screens::button(cv, 125, y - 1), Hit::Buy(index)));
        hits.push((screens::button(cv, 137, y - 1), Hit::Sell(index)));
        screens::text(cv, a, &s.name, 152, y, c);
        screens::text(cv, a, &credits(s.price as i64), 234, y, c);
        screens::text(cv, a, &credits(s.price as i64 - trade), 268, y, c);
        screens::text(cv, a, &format!("{}t", s.capacity_t as i32), 302, y, c);
    }

    fn draw_ships(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>) {
        self.ship_headers(cv);
        let n = self.port.as_ref().map_or(0, |p| p.ships.len());
        for row in 0..n.min(LIST_ROWS) {
            self.ship_offer(cv, hits, self.scroll + row, 15 + 10 * row as i32);
        }
        go_back(cv, &self.a, hits);
    }

    /// Manual, Section Four: View adds mountings, crew and hyperspace range.
    /// The rows and the Constrictor's figures are the original's View screen.
    fn draw_ship_view(&self, cv: &mut Canvas, i: usize, hits: &mut Vec<(Rect, Hit)>) {
        let Some(s) = self.port.as_ref().and_then(|p| p.ships.get(i)).and_then(|id| self.yard.ships.get(id)).cloned() else {
            return;
        };
        // The hull in this box is still the Eagle mesh. The original draws the
        // ship on offer; the Constrictor's own shape is not built yet.
        screens::ship_in(cv, Rect::new(2, 44, 236, 230), self.anim_t, [[77, 142, 77], [60, 125, 60], [44, 109, 44]]);
        self.ship_headers(cv);
        self.ship_offer(cv, hits, i, 15);
        let a = &self.a;
        let c = palette::CREAM;
        let laden = s.mass_t + s.capacity_t;
        let drive = self
            .yard
            .equipment
            .values()
            .find(|e| e.slot == "drive" && e.raw_str("drive_id") == Some(s.drive.as_str()))
            .map(|e| e.name.clone())
            .unwrap_or_else(|| s.drive.clone());
        let reg = self.registration(i);
        let rows = [
            ("Mass (Fully laden)", format!("{}t", laden as i32)),
            ("Internal Capacity (no drive)", format!("{}t", s.capacity_t as i32)),
            ("Gun Mountings", s.gun_mounts.len().to_string()),
            ("Missile Pylons", s.missiles.to_string()),
            ("Crew", s.crew.to_string()),
            ("Main Thruster Acceleration", format!("{:.1} Earth g", s.thrust_g)),
            ("Retro Thruster Acceleration", format!("{:.1} Earth g", s.retro_g)),
            ("Drive Fitted", drive),
            ("Registration Code", reg),
        ];
        for (k, (label, value)) in rows.iter().enumerate() {
            let y = 25 + 10 * k as i32;
            screens::text(cv, a, label, 140, y, c);
            screens::text_right(cv, a, value, 316, y, c);
        }
        screens::text(cv, a, "Hyperspace Ranges:", 140, 115, c);
        let class_of = |id: &str| id.chars().filter(|ch| ch.is_ascii_digit()).collect::<String>().parse::<i32>().unwrap_or(0);
        let (lo, hi) = if s.drive_min > 0 && s.drive_max >= s.drive_min {
            (s.drive_min, s.drive_max)
        } else {
            let n = class_of(&s.drive);
            (n, n)
        };
        for class in 1..=7 {
            let x = 148 + (class - 1) * 24;
            screens::text(cv, a, &format!("Class {class}"), x, 125, c);
            let ly = if class < lo || class > hi {
                0.0
            } else {
                (frontier_sim::frontier::max_range_ly(laden, class as f64) * 100.0).floor() / 100.0
            };
            screens::text_right(cv, a, &format!("{ly:.2}"), x + 22, 135, c);
        }
        go_back(cv, a, hits);
    }

    /// Registration of a ship on the Sirocco lot. Only VR-039 was read off the
    /// original's View screen; the other three were not opened.
    fn registration(&self, i: usize) -> String {
        let Some(p) = &self.port else { return String::new() };
        if p.id != self.observed.port || self.day() != 0 {
            return String::new();
        }
        self.observed.registration(i)
    }
}

fn take_cargo(f: &mut FittedShip, id: &str) {
    if let Some(n) = f.cargo.get_mut(id) {
        *n -= 1;
        if *n <= 0 {
            f.cargo.remove(id);
        }
    }
}

/// Four options with bracketed descriptions, 35 DOS rows apart.
fn menu(cv: &mut Canvas, a: &crate::assets::Assets, items: &[(&str, &[&str])], hits: &mut Vec<(Rect, Hit)>) {
    for (i, (name, desc)) in items.iter().enumerate() {
        let y = 15 + 35 * i as i32;
        let b = screens::button(cv, 125, y - 1);
        let w = screens::text(cv, a, name, 140, y, palette::CREAM);
        for (k, d) in desc.iter().enumerate() {
            screens::text(cv, a, d, 142, y + 10 * (k as i32 + 1), palette::CREAM);
        }
        hits.push((Rect::new(b.x, b.y, (w + 18) * 2, b.h), Hit::Option(i)));
    }
}

/// "Suggested Responses:" and a column of buttons; each response may wrap onto
/// several rows. `gap` is the extra spacing between responses in DOS rows.
pub(super) fn responses(
    cv: &mut Canvas,
    a: &crate::assets::Assets,
    items: &[Vec<String>],
    step: i32,
    hits: &mut Vec<(Rect, Hit)>,
) {
    screens::text(cv, a, "Suggested Responses:", 120, 2, palette::LILAC);
    let mut y = 15;
    for (i, lines) in items.iter().enumerate() {
        let b = screens::button(cv, 125, y - 1);
        let mut w = 0;
        for (k, l) in lines.iter().enumerate() {
            w = w.max(screens::text(cv, a, l, 140, y + 10 * k as i32, palette::CREAM));
        }
        hits.push((Rect::new(b.x, b.y, (w + 18) * 2, b.h), Hit::Reply(i)));
        y += step.max(10 * lines.len() as i32);
    }
}

/// Down and up arrows with "More..." under a scrolling list.
pub(super) fn more_arrows(
    cv: &mut Canvas,
    a: &crate::assets::Assets,
    top: usize,
    len: usize,
    rows: usize,
    hits: &mut Vec<(Rect, Hit)>,
) {
    let down = top + rows < len;
    let up = top > 0;
    if down {
        hits.push((screens::arrow(cv, 124, 156, true), Hit::Scroll(1)));
    }
    if up {
        hits.push((screens::arrow(cv, 136, 156, false), Hit::Scroll(-1)));
    }
    if up || down {
        screens::text(cv, a, "More...", 152, 157, palette::CREAM);
    }
}

fn go_back(cv: &mut Canvas, a: &crate::assets::Assets, hits: &mut Vec<(Rect, Hit)>) {
    let b = screens::button(cv, 201, 157);
    let w = screens::text(cv, a, "Go Back...", 210, 157, palette::WHITE);
    hits.push((Rect::new(b.x, b.y, (w + 12) * 2, b.h), Hit::Back));
}
