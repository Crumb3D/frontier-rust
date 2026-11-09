//! Admin menu from the Python build. Backquote or F12 opens it.

use super::{bulletin::Contract, FlightMode, FrameState, Game, Panel, Where, H, W};
use frontier_sim::frontier::{combat_rank, COMBAT_RANKS};
use crate::fb::Rect;
use crate::font::{self, SMALL};
use crate::palette;
use frontier_sim::FittedShip;
use glam::DVec3;
use std::collections::BTreeMap;
use winit::keyboard::KeyCode;

impl Game {
    fn cheat_rows(&self) -> Vec<(&'static str, String)> {
        vec![
            ("freebuy", format!("Free purchases: {}", on(self.free_purchases))),
            ("rubbish", "Put 1t rubbish in the hold".into()),
            ("cr100k", "+100,000 credits".into()),
            ("cr1m", "+1,000,000 credits".into()),
            ("fuel", "Fill tanks and +30t hydrogen".into()),
            ("milfuel", "+10t military fuel".into()),
            ("minerals", "+5t minerals".into()),
            ("gems", "+10t gem stones".into()),
            ("repair", "Full repair".into()),
            ("god", format!("God mode: {}", on(self.god_mode))),
            ("infuel", format!("Infinite fuel: {}", on(self.infinite_fuel))),
            ("legal", "Clear fines and go legal".into()),
            ("wanted", "Become wanted (10,000 Cr fine)".into()),
            ("clearance", "Grant launch clearance".into()),
            ("kit", "Fit scoop, cabin, shields".into()),
            ("minelaser", "Fit a mining laser on the nose".into()),
            ("passenger", "Put a passenger in a cabin".into()),
            ("contract", "Add a test delivery contract".into()),
            ("rank", "Advance to the next combat rank".into()),
            ("elite", "Set combat rating to ELITE".into()),
            ("jump", "Jump range 50 ly".into()),
            ("teleport", "Teleport to nearest station".into()),
            ("panther", "Give a fitted Panther Clipper".into()),
            ("raid", "Spawn a pirate raid".into()),
            ("wipe", "Destroy every pirate".into()),
            ("month", "Advance the stardate 30 days".into()),
            ("close", "Close".into()),
        ]
    }

    pub(super) fn cheat_key(&mut self, key: KeyCode) {
        let n = self.cheat_rows().len();
        let half = n.div_ceil(2);
        match key {
            KeyCode::Escape | KeyCode::Backquote | KeyCode::F12 => self.cheats_open = false,
            KeyCode::ArrowUp | KeyCode::KeyW => self.cheat_sel = (self.cheat_sel + n - 1) % n,
            KeyCode::ArrowDown | KeyCode::KeyS => self.cheat_sel = (self.cheat_sel + 1) % n,
            KeyCode::ArrowLeft | KeyCode::KeyA => self.cheat_sel = (self.cheat_sel + n - half) % n,
            KeyCode::ArrowRight | KeyCode::KeyD => self.cheat_sel = (self.cheat_sel + half) % n,
            KeyCode::Enter | KeyCode::Space => self.apply_cheat(self.cheat_sel),
            _ => {}
        }
    }

    pub(super) fn apply_cheat(&mut self, i: usize) {
        let Some(id) = self.cheat_rows().get(i).map(|r| r.0) else { return };
        self.cheat_sel = i;
        match id {
            "close" => self.cheats_open = false,
            "freebuy" => {
                self.free_purchases = !self.free_purchases;
                self.msg(format!(
                    "Cheat: purchases {}",
                    if self.free_purchases { "free (sales still pay)" } else { "cost credits again" }
                ));
            }
            "rubbish" => {
                add_hold(&mut self.fitted, "rubbish", 1);
                self.msg("Cheat: +1t rubbish in the hold");
            }
            "cr100k" => {
                self.cash += 100_000 * 10;
                self.msg("Cheat: +100,000 credits");
            }
            "cr1m" => {
                self.cash += 1_000_000 * 10;
                self.msg("Cheat: +1,000,000 credits");
            }
            "fuel" => {
                let max = self.yard.stats(&self.fitted).max_fuel_t.max(1.0);
                self.fitted.fuel_t = max;
                add_hold(&mut self.fitted, "hydrogen_fuel", 30);
                self.refresh_ship();
                self.msg("Cheat: tanks full, +30t hydrogen in the hold");
            }
            "milfuel" => {
                add_hold(&mut self.fitted, "military_fuel", 10);
                self.msg("Cheat: +10t military fuel");
            }
            "minerals" => {
                add_hold(&mut self.fitted, "minerals", 5);
                self.msg("Cheat: +5t minerals");
            }
            "gems" => {
                add_hold(&mut self.fitted, "gem_stones", 10);
                self.msg("Cheat: +10t gem stones");
            }
            "repair" => {
                self.fitted.hull = self.fitted.max_hull;
                self.refresh_ship();
                self.msg("Cheat: ship fully repaired");
            }
            "god" => {
                self.god_mode = !self.god_mode;
                self.msg(format!("Cheat: god mode {}", on(self.god_mode)));
            }
            "legal" => {
                self.fines.clear();
                self.msg("Cheat: record wiped clean");
            }
            "wanted" => {
                let gov = self.system().government.clone();
                let due = self.fine_here().max(10_000);
                self.fines.insert(gov.clone(), due);
                self.msg(format!("Cheat: you are now wanted in {gov} with a {due} Cr fine"));
            }
            "infuel" => {
                self.infinite_fuel = !self.infinite_fuel;
                self.msg(format!("Cheat: infinite fuel {}", on(self.infinite_fuel)));
            }
            "clearance" => {
                self.launch_clearance = true;
                self.msg("Cheat: launch clearance granted");
            }
            "kit" => {
                for eid in ["fuel_scoop", "passenger_cabin", "shield_gen", "atmo_shield", "autopilot", "cloud_analyser"] {
                    if self.yard.equipment.contains_key(eid) && !self.fitted.has(eid) {
                        self.fitted.equipment.push(eid.to_string());
                    }
                }
                self.refresh_ship();
                self.msg("Cheat: test fittings installed");
            }
            "minelaser" => self.cheat_mining_laser(),
            "passenger" => self.cheat_passenger(),
            "contract" => self.cheat_contract(),
            "rank" => self.cheat_rank(),
            "elite" => {
                self.kills = 6000;
                self.msg(format!("Cheat: combat rating {}", combat_rank(self.kills)));
            }
            "jump" => {
                self.cheat_jump_ly = Some(50.0);
                self.msg("Cheat: hyperspace range 50 ly");
            }
            "teleport" => self.cheat_teleport(),
            "panther" => self.cheat_panther(),
            "raid" => self.spawn_raid(),
            "wipe" => self.wipe_pirates(),
            "month" => {
                self.stardate.days += 30.0;
                self.msg(format!("Cheat: stardate now {}", self.stardate.format()));
            }
            _ => {}
        }
    }

    fn cheat_teleport(&mut self) {
        if matches!(self.wh, Where::Docked { .. }) {
            self.msg("Cheat: undock first");
            return;
        }
        let days = self.stardate.days;
        let world = self.frames.ship_world_pos(&self.ship, days);
        let mut best: Option<(f64, String, String)> = None;
        for st in &self.system().stations {
            let d = (world - self.frames.world_position(&st.id, days)).length();
            if best.as_ref().map_or(true, |(bd, _, _)| d < *bd) {
                best = Some((d, st.id.clone(), st.name.clone()));
            }
        }
        if best.is_none() {
            if let Some((id, body)) = self.frames.bodies.iter().find(|(_, b)| b.is_starport) {
                best = Some((0.0, id.clone(), body.name.clone()));
            }
        }
        let Some((_, id, name)) = best else {
            self.msg("Cheat: no station in this system");
            return;
        };
        self.ship = FrameState::new(id);
        self.ship.position = DVec3::new(0.0, 0.0, -25.0);
        self.wh = Where::Flying;
        self.panel = Panel::View;
        self.gear_down = true;
        self.fm.controls.mode = FlightMode::Manual;
        self.fm.controls.set_speed = 0.0;
        self.fm.controls.thrust = 0.0;
        self.msg(format!("Cheat: 25 km from {name}"));
    }

    fn cheat_mining_laser(&mut self) {
        if let Some(old) = self.fitted.guns.get("front").cloned() {
            self.fitted.equipment.retain(|e| e != &old);
        }
        self.fitted.guns.insert("front".into(), "mining_laser".into());
        if !self.fitted.equipment.iter().any(|e| e == "mining_laser") {
            self.fitted.equipment.push("mining_laser".into());
        }
        self.refresh_ship();
        self.msg("Cheat: mining laser on the front mount");
    }

    fn cheat_passenger(&mut self) {
        if self.yard.equipment.contains_key("passenger_cabin") && !self.fitted.has("passenger_cabin") {
            self.fitted.equipment.push("passenger_cabin".into());
        }
        let dest = self.other_system_name();
        self.contracts.push(Contract {
            client: "Passenger".into(),
            group: "1 passenger".into(),
            dest: dest.clone(),
            pay: 500,
            passengers: 1,
        });
        self.refresh_ship();
        self.msg(format!("Cheat: passenger for {dest}"));
    }

    fn cheat_contract(&mut self) {
        *self.fitted.cargo.entry("computers".into()).or_insert(0) += 1;
        let dest = self.other_system_name();
        self.contracts.push(Contract {
            client: "Delivery".into(),
            group: "1t computers".into(),
            dest: dest.clone(),
            pay: 500,
            passengers: 0,
        });
        self.msg(format!("Cheat: contract to {dest}, 1t computers aboard"));
    }

    fn cheat_rank(&mut self) {
        let nxt = COMBAT_RANKS.iter().find(|(need, _)| *need > self.kills).map(|(need, _)| *need);
        match nxt {
            None => self.msg("Cheat: already ELITE"),
            Some(need) => {
                self.kills = need;
                self.msg(format!("Cheat: combat rating {} ({})", combat_rank(self.kills), self.kills));
            }
        }
    }

    fn other_system_name(&self) -> String {
        self.galaxy
            .systems
            .keys()
            .find(|id| id.as_str() != self.sys_id)
            .map(|id| self.galaxy.get(id).name.clone())
            .unwrap_or_else(|| "Sol".into())
    }

    fn cheat_panther(&mut self) {
        self.fitted = FittedShip {
            ship_id: "panther".into(),
            equipment: vec![
                "drive_class8".into(),
                "shield_gen".into(),
                "shield_gen".into(),
                "autopilot".into(),
                "fuel_scoop".into(),
                "atmo_shield".into(),
                "mining_laser".into(),
            ],
            guns: BTreeMap::from([("front".into(), "mining_laser".into())]),
            missile_load: vec![],
            cargo: BTreeMap::from([("hydrogen_fuel".into(), 30), ("gem_stones".into(), 20)]),
            bonus_capacity: 0.0,
            fuel_t: 10.0,
            hull: 500.0,
            max_hull: 500.0,
        };
        self.cheat_jump_ly = Some(50.0);
        self.refresh_ship();
        self.msg("Cheat: Panther Clipper delivered, jump 50 ly");
    }

    fn refresh_ship(&mut self) {
        let controls = self.fm.controls.clone();
        self.fm = frontier_sim::FlightModel::new(self.yard.stats(&self.fitted));
        self.fm.controls = controls;
    }

    pub(super) fn draw_cheats(&mut self, cv: &mut crate::fb::Canvas) {
        cv.reset_clip();
        cv.clear([6, 4, 12]);
        let font = &self.a.font;
        font.draw_centered(cv, "CHEAT CONSOLE", W / 2, 16, SMALL, palette::CON_GREEN);
        let sub = format!(
            "{}    Jump {:.1} ly    H2 {}t    {}    {}",
            font::money(self.cash),
            self.flight_stats().jump_range_ly,
            self.fitted.hydrogen_t(),
            combat_rank(self.kills),
            if self.wanted_here() { "WANTED" } else { "Legal" }
        );
        font.draw_centered(cv, &sub, W / 2, 34, SMALL, palette::CON_ORANGE);
        let rows = self.cheat_rows();
        let n = rows.len();
        self.cheat_sel = self.cheat_sel.min(n - 1);
        let half = n.div_ceil(2);
        for (i, (_, label)) in rows.iter().enumerate() {
            let col = i / half;
            let row = i % half;
            let x = 24 + col as i32 * 310;
            let y = 58 + row as i32 * 18;
            if i == self.cheat_sel {
                cv.fill_rect(Rect::new(x - 4, y - 1, 292, 16), [36, 90, 48]);
            }
            let prefix = if i == self.cheat_sel { "> " } else { "  " };
            let colr = if i == self.cheat_sel { palette::CON_GREEN } else { palette::HUD_WHITE };
            font.draw(cv, &format!("{prefix}{label}"), x, y, SMALL, colr);
            self.hits.push((Rect::new(x - 4, y - 1, 292, 16), super::Action::Cheat(i)));
        }
        font.draw_centered(
            cv,
            "` or Esc close    Arrows select    Enter apply",
            W / 2,
            H - 22,
            SMALL,
            [140, 140, 150],
        );
        if !self.message.is_empty() {
            font.draw_centered(cv, &self.message_text(), W / 2, H - 40, SMALL, palette::CON_GREEN);
        }
    }
}

fn on(v: bool) -> &'static str {
    if v { "ON" } else { "OFF" }
}

fn add_hold(ship: &mut FittedShip, id: &str, n: i32) {
    *ship.cargo.entry(id.to_string()).or_insert(0) += n;
}
