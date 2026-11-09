//! Bulletin board, video link and Federal HQ. Advert wording, the suggested
//! responses and the replies are those of the DOS original at Sirocco Station.

use super::station::{more_arrows, responses, Hit, Page};
use super::Game;
use crate::fb::{Canvas, Rect};
use crate::font::{credits, money};
use crate::palette;
use crate::screens;
use frontier_sim::market::Market;
use frontier_sim::{Stardate, StarSystem};
use serde_json::Value;

#[derive(Clone, Debug)]
pub enum AdKind {
    /// `style` 0 "WANTED: passage...", 1 "SHIP REQUIRED: ... small group",
    /// 2 "SHIP REQUIRED: ... on a ship for a group".
    Passenger { group: i32, style: u8 },
    Package,
    Goods { good: String, name: String, price: i64 },
    Missing { person: String },
    Military,
}

#[derive(Clone, Debug)]
pub struct Ad {
    pub kind: AdKind,
    pub client: String,
    pub dest: String,
    pub sector: (i32, i32),
    /// Whole credits.
    pub pay: i64,
    pub why: String,
    pub problems: String,
    pub portrait: usize,
    pub gone: bool,
}

#[derive(Clone, Debug)]
pub struct Mission {
    pub what: String,
    pub dest: String,
    pub sector: (i32, i32),
    pub deadline: String,
    pub pay: i64,
}

/// A job on the passenger and small items roster.
#[derive(Clone, Debug)]
pub struct Contract {
    pub client: String,
    pub group: String,
    pub dest: String,
    /// Credits still to be paid on arrival.
    pub pay: i64,
    pub passengers: i32,
}

/// Video-link state for the advert being answered.
#[derive(Clone, Debug, Default)]
pub struct Talk {
    pub agreed: bool,
    pub half: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Reply {
    Ok,
    HowMany,
    Why,
    Problems,
    MakeRoom,
    Repeat,
    Permit,
    More,
    Half,
    All,
    NoRoom,
    NotGoing,
    HangUp,
}

const PASSENGER_REPLIES: [(Reply, &str); 13] = [
    (Reply::Ok, "\"OK - agreed.\""),
    (Reply::HowMany, "\"How many of you are there?\""),
    (Reply::Why, "\"Why so much money?\""),
    (Reply::Problems, "\"Will there be any problems?\""),
    (Reply::MakeRoom, "\"Hold on while I make room\" then hang up."),
    (Reply::Repeat, "\"Could you repeat your original request\""),
    (Reply::Permit, "\"Do I need a permit, and if so can I have one?\""),
    (Reply::More, "\"I want more money\""),
    (Reply::Half, "\"I want half the money now\""),
    (Reply::All, "\"I want all the money now\""),
    (Reply::NoRoom, "\"I haven't enough room in my ship\" then hang up."),
    (Reply::NotGoing, "\"Sorry, I'm not going that way\" then hang up."),
    (Reply::HangUp, "Hang up."),
];

/// The original's start-of-game state at one port, from `data/observed.json`.
pub struct Observed {
    pub port: String,
    raw: Value,
    pub ships: Vec<String>,
    pub upgrades: Vec<String>,
}

fn strs(v: &Value, key: &str) -> Vec<String> {
    v[key].as_array().map(|a| a.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn mix(a: u64, b: u64) -> u64 {
    let mut h = a ^ b.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    h ^= h >> 31;
    h = h.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    h ^ (h >> 27)
}

impl Observed {
    pub fn load() -> Result<Self, String> {
        let path = frontier_sim::paths::data_dir().join("observed.json");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        let raw: Value = serde_json::from_str(&text).map_err(|e| format!("observed.json: {e}"))?;
        Ok(Self {
            port: raw["port"].as_str().unwrap_or_default().to_string(),
            ships: strs(&raw, "ships"),
            upgrades: strs(&raw, "upgrades"),
            raw,
        })
    }

    /// A registration read off the original. Empty when that ship was not opened.
    pub fn registration(&self, i: usize) -> String {
        self.raw["registrations"].get(i).and_then(|v| v.as_str()).unwrap_or("").to_string()
    }

    fn sector(&self, system: &str) -> (i32, i32) {
        let v = &self.raw["sectors"][system];
        let at = if v.is_array() { v } else { &v["at"] };
        (at[0].as_i64().unwrap_or(0) as i32, at[1].as_i64().unwrap_or(0) as i32)
    }

    fn pick(&self, key: &str, h: u64) -> String {
        let v = strs(&self.raw, key);
        if v.is_empty() {
            String::new()
        } else {
            v[(h % v.len() as u64) as usize].clone()
        }
    }

    fn name(&self, h: u64) -> String {
        format!("{} {}", self.pick("first_names", h), self.pick("surnames", h >> 17))
    }

    fn goods_ad(&self, market: &Market, sys: &StarSystem, day: i64, good: &str, client: String, portrait: usize) -> Option<Ad> {
        let g = market.goods_by_id.get(good)?;
        let (price, _) = market.quote(sys, day, g);
        Some(Ad {
            kind: AdKind::Goods { good: good.into(), name: g.name.clone(), price: price as i64 * 2 },
            client,
            dest: String::new(),
            sector: (0, 0),
            pay: 0,
            why: String::new(),
            problems: String::new(),
            portrait,
            gone: false,
        })
    }

    /// The twelve adverts and two Federal missions seen at Sirocco on day one.
    pub fn ads(&self, market: &Market, sys: &StarSystem, day: i64) -> (Vec<Ad>, Vec<Mission>) {
        let mut ads = Vec::new();
        for (i, a) in self.raw["ads"].as_array().into_iter().flatten().enumerate() {
            let h = mix(0x51c0, i as u64);
            let client = a["client"].as_str().map(String::from).unwrap_or_else(|| self.name(h));
            let s = |k: &str, alt: &str| a[k].as_str().map(String::from).unwrap_or_else(|| self.pick(alt, h >> 7));
            let dest = a["dest"].as_str().unwrap_or_default().to_string();
            let pay = a["pay"].as_i64().unwrap_or(0);
            let portrait = match client.as_str() {
                "Betty Morris" => 1,
                "Andrea Peterson" => 2,
                "Sarah Davies" => 3,
                _ => (h % 5) as usize,
            };
            let base = Ad {
                kind: AdKind::Military,
                client: client.clone(),
                sector: self.sector(&dest),
                dest,
                pay,
                why: String::new(),
                problems: String::new(),
                portrait,
                gone: false,
            };
            let ad = match a["kind"].as_str().unwrap_or_default() {
                "passenger" => Ad {
                    kind: AdKind::Passenger {
                        group: a["group"].as_i64().unwrap_or(2) as i32,
                        style: a["style"].as_u64().unwrap_or(0) as u8,
                    },
                    why: s("why", "reasons"),
                    problems: a["problems"].as_str().unwrap_or("No.").to_string(),
                    ..base
                },
                "package" => Ad {
                    kind: AdKind::Package,
                    why: s("why", "package_reasons"),
                    problems: s("problems", "package_problems"),
                    ..base
                },
                "missing" => Ad {
                    kind: AdKind::Missing { person: a["person"].as_str().unwrap_or_default().to_string() },
                    portrait: 4,
                    ..base
                },
                "goods" => {
                    let good = a["good"].as_str().unwrap_or_default();
                    match self.goods_ad(market, sys, day, good, client, portrait) {
                        Some(ad) => ad,
                        None => continue,
                    }
                }
                _ => base,
            };
            ads.push(ad);
        }
        let missions = self.raw["missions"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|m| {
                let dest = m["dest"].as_str().unwrap_or_default().to_string();
                Mission {
                    what: m["what"].as_str().unwrap_or("documents").to_string(),
                    sector: self.sector(&dest),
                    dest,
                    deadline: date(day as f64 + m["days"].as_f64().unwrap_or(14.0)),
                    pay: m["pay"].as_i64().unwrap_or(200),
                }
            })
            .collect();
        (ads, missions)
    }

    /// Adverts for any other port or day, built from the same templates.
    pub fn random_ads(&self, market: &Market, sys: &StarSystem, day: i64, port: &str) -> (Vec<Ad>, Vec<Mission>) {
        let seed = port.bytes().fold(day as u64, |h, b| mix(h, b as u64));
        let dests: Vec<String> = self.raw["sectors"]
            .as_object()
            .map(|o| o.keys().filter(|k| **k != sys.name).cloned().collect())
            .unwrap_or_default();
        if dests.is_empty() {
            return (vec![], vec![]);
        }
        let n = 6 + (seed % 7) as usize;
        let mut ads = Vec::new();
        for i in 0..n {
            let h = mix(seed, i as u64 + 1);
            let dest = dests[(h % dests.len() as u64) as usize].clone();
            let pay = 300 + (h >> 8) % 120 * 10;
            let base = Ad {
                kind: AdKind::Package,
                client: self.name(h >> 3),
                sector: self.sector(&dest),
                dest,
                pay: pay as i64,
                why: self.pick("package_reasons", h >> 11),
                problems: self.pick("package_problems", h >> 13),
                portrait: (h >> 5) as usize % 5,
                gone: false,
            };
            let ad = match (h >> 20) % 10 {
                0..=4 => {
                    let style = ((h >> 24) % 3) as u8;
                    let group = if style == 2 { 4 + (h >> 26) % 4 } else { 2 + (h >> 26) % 2 } as i32;
                    Ad {
                        kind: AdKind::Passenger { group, style },
                        why: self.pick("reasons", h >> 15),
                        problems: "No.".into(),
                        ..base
                    }
                }
                5 | 6 => base,
                7 | 8 => {
                    let goods: Vec<&String> = market.goods.iter().filter(|g| !g.disposal).map(|g| &g.id).collect();
                    let good = goods[((h >> 28) % goods.len() as u64) as usize].clone();
                    match self.goods_ad(market, sys, day, &good, base.client.clone(), base.portrait) {
                        Some(ad) => ad,
                        None => continue,
                    }
                }
                _ => Ad { kind: AdKind::Missing { person: self.name(h >> 30) }, pay: 100 + (h >> 9) as i64 % 10 * 10, ..base },
            };
            ads.push(ad);
        }
        if frontier_sim::market::system_faction(sys) == "Federation" && !ads.is_empty() {
            ads.insert((seed % ads.len().max(1) as u64) as usize, Ad { kind: AdKind::Military, ..ads[0].clone() });
        }
        let missions = (0..2)
            .map(|i| {
                let h = mix(seed ^ 0xfed, i);
                let dest = dests[(h % dests.len() as u64) as usize].clone();
                Mission {
                    what: if h & 1 == 0 { "documents".into() } else { "messages".into() },
                    sector: self.sector(&dest),
                    dest,
                    deadline: date(day as f64 + 14.0 + (h >> 4) as f64 % 14.0),
                    pay: 200 + (h >> 9) as i64 % 20 * 10,
                }
            })
            .collect();
        (ads, missions)
    }
}

/// `23-Jan-3200` for a day number.
fn date(days: f64) -> String {
    Stardate::new(days).format().split(' ').nth(1).unwrap_or_default().to_string()
}

fn ad_text(ad: &Ad) -> String {
    let pay = credits(ad.pay);
    let d = &ad.dest;
    match &ad.kind {
        AdKind::Passenger { style: 0, .. } => format!("WANTED: passage for a small group to the {d} system. Will pay {pay}."),
        AdKind::Passenger { style: 1, .. } => {
            format!("SHIP REQUIRED: Payment of {pay} for passage for a small group to the {d} system.")
        }
        AdKind::Passenger { .. } => {
            format!("SHIP REQUIRED: Payment of {pay} for passage on a ship for a group to the {d} system.")
        }
        AdKind::Package => format!("SHIP REQUIRED: Payment of {pay} for a ship to take a small package to the {d} system."),
        AdKind::Goods { name, price, .. } => format!("WANTED: {name}. Will pay {} per tonne.", money(*price)),
        AdKind::Missing { person } => {
            format!("MISSING PERSON: {person} last seen in the {d} system. {pay} paid for useful information.")
        }
        AdKind::Military => "FEDERAL MILITARY: We want YOU for adventures unlimited.".into(),
    }
}

impl Game {
    pub(super) fn ad_intro(&self, i: usize) -> String {
        let Some(ad) = self.port.as_ref().and_then(|p| p.ads.get(i)) else { return String::new() };
        let (sx, sy) = ad.sector;
        let (c, d, pay) = (&ad.client, &ad.dest, credits(ad.pay));
        match &ad.kind {
            AdKind::Passenger { style, .. } => {
                let who = if *style == 2 { "a group" } else { "a small group" };
                format!("I'm {c} and I need passage for {who} to the {d} system [{sx},{sy}]. I will pay {pay}.")
            }
            AdKind::Package => {
                format!("I'm {c} and I need a ship to take a small package to the {d} system [{sx},{sy}]. I will pay {pay}.")
            }
            AdKind::Goods { name, price, .. } => {
                format!("I'm {c}. I need {name} urgently, and I'll pay twice the market price ({}) for them.", money(*price))
            }
            AdKind::Missing { person } => {
                format!("Hello. I'm looking for {person} and I need some help with my search. Can you help me?")
            }
            AdKind::Military => String::new(),
        }
    }

    pub(super) fn open_ad(&mut self, visible: usize) {
        let Some(p) = &self.port else { return };
        let Some(i) = p.ads.iter().enumerate().filter(|(_, a)| !a.gone).nth(visible).map(|(i, _)| i) else { return };
        if matches!(p.ads[i].kind, AdKind::Military) {
            self.open_page(Page::FederalHq);
        } else {
            self.open_page(Page::Video(i));
        }
    }

    fn reply_list(&self, ad: &Ad) -> Vec<Reply> {
        if self.talk.agreed {
            return vec![Reply::HangUp];
        }
        PASSENGER_REPLIES
            .iter()
            .map(|r| r.0)
            .filter(|r| !(matches!(ad.kind, AdKind::Package) && *r == Reply::HowMany))
            .collect()
    }

    fn answer(&mut self, question: &str, reply: String) {
        self.say(vec![(question.to_string(), palette::CON_ORANGE), (reply, palette::CON_GREEN)]);
    }

    pub(super) fn video_reply(&mut self, i: usize, r: usize) {
        let Some(ad) = self.port.as_ref().and_then(|p| p.ads.get(i)).cloned() else { return };
        match &ad.kind {
            AdKind::Passenger { .. } | AdKind::Package => self.job_reply(i, &ad, r),
            AdKind::Goods { good, name, price } => {
                if r != 0 {
                    self.open_page(Page::Bulletin);
                    return;
                }
                let have = self.fitted.cargo.get(good).copied().unwrap_or(0);
                if have <= 0 {
                    self.say(vec![(format!("You have no {name} to sell."), palette::CON_GREEN)]);
                } else {
                    self.cash += price;
                    if let Some(n) = self.fitted.cargo.get_mut(good) {
                        *n -= 1;
                        if *n == 0 {
                            self.fitted.cargo.remove(good);
                        }
                    }
                    // Wording not in the manual. Not confirmed.
                    self.say(vec![("Thank you.".into(), palette::CON_GREEN)]);
                }
            }
            AdKind::Missing { person } => {
                let reply = match r {
                    0 => format!("\"{person} is in a cabin on my ship\""),
                    1 => format!("\"{person} has an ad. on this bulletin board\""),
                    2 => format!("\"I will look out for {person}\""),
                    _ => {
                        self.open_page(Page::Bulletin);
                        return;
                    }
                };
                let answer = match r {
                    2 => "That would be appreciated, but I will only pay for solid information.",
                    // Wording not in the manual. Not confirmed.
                    _ => "I don't believe you.",
                };
                self.answer(&reply, answer.into());
            }
            AdKind::Military => {}
        }
    }

    fn job_reply(&mut self, i: usize, ad: &Ad, r: usize) {
        let list = self.reply_list(ad);
        let Some(&reply) = list.get(r) else { return };
        let q = PASSENGER_REPLIES.iter().find(|p| p.0 == reply).map_or("", |p| p.1);
        let q = q.split(" then").next().unwrap_or(q).to_string();
        let group = match ad.kind {
            AdKind::Passenger { group, .. } => group,
            _ => 0,
        };
        let pay = credits(ad.pay);
        match reply {
            Reply::Ok => {
                let free = self.hold().cabins_free;
                if group > free {
                    self.answer(&q, "You do not have enough cabin space available.".into());
                    return;
                }
                let paid = if self.talk.half { ad.pay / 2 } else { 0 };
                self.cash += paid * 10;
                self.contracts.push(Contract {
                    client: ad.client.clone(),
                    group: if group > 0 { group.to_string() } else { "Parcel".into() },
                    dest: ad.dest.clone(),
                    pay: ad.pay - paid,
                    passengers: group,
                });
                self.talk.agreed = true;
                if let Some(p) = self.port.as_mut() {
                    p.ads[i].gone = true;
                }
                let due = credits(ad.pay - paid);
                self.answer(&q, format!("Excellent. {due} will be paid on arrival at the {} system.", ad.dest));
            }
            Reply::HowMany => self.answer(&q, format!("There are {group} of us.")),
            Reply::Why => self.answer(&q, ad.why.clone()),
            Reply::Problems => self.answer(&q, ad.problems.clone()),
            Reply::Repeat => {
                let intro = self.ad_intro(i);
                self.answer(&q, intro);
            }
            Reply::Permit => self.answer(&q, "Don't be silly- you don't need a permit.".into()),
            Reply::More | Reply::Half | Reply::All if self.talk.half => self.answer(&q, "We've agreed on payment.".into()),
            Reply::More => self.answer(&q, format!("{pay} is all I can pay.")),
            Reply::Half => {
                self.talk.half = true;
                self.answer(&q, "OK. I'll pay half once you agree, and half on arrival.".into());
            }
            Reply::All => self.answer(&q, "I'll pay half once you agree, and no more.".into()),
            Reply::MakeRoom | Reply::NoRoom | Reply::NotGoing | Reply::HangUp => self.open_page(Page::Bulletin),
        }
    }

    pub(super) fn federal_reply(&mut self, r: usize) {
        let n = self.port.as_ref().map_or(0, |p| p.missions.len());
        if r >= n {
            self.open_page(Page::Bulletin);
            return;
        }
        let Some(m) = self.port.as_mut().map(|p| p.missions.remove(r)) else { return };
        self.contracts.push(Contract {
            client: "Federal Military".into(),
            group: "Docs.".into(),
            dest: m.dest.clone(),
            pay: m.pay,
            passengers: 0,
        });
        self.say(vec![(format!("Deliver the {} to the {} system by midnight on {}.", m.what, m.dest, m.deadline), palette::CON_GREEN)]);
    }

    pub(super) fn draw_bulletin(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>) {
        let a = &self.a;
        let Some(p) = &self.port else { return };
        let ads: Vec<&Ad> = p.ads.iter().filter(|a| !a.gone).collect();
        for (row, ad) in ads.iter().skip(self.scroll).take(4).enumerate() {
            let y = 15 + 35 * row as i32;
            let b = screens::button(cv, 125, y - 1);
            for (k, l) in screens::wrap(a, &ad_text(ad), 178).iter().take(3).enumerate() {
                screens::text(cv, a, l, 140, y + 10 * k as i32, palette::CREAM);
            }
            hits.push((Rect::new(b.x, b.y, 380, b.h), Hit::Ad(row)));
        }
        more_arrows(cv, a, self.scroll, ads.len(), 4, hits);
    }

    pub(super) fn draw_video(&self, cv: &mut Canvas, i: usize, hits: &mut Vec<(Rect, Hit)>) {
        let a = &self.a;
        let Some(ad) = self.port.as_ref().and_then(|p| p.ads.get(i)) else { return };
        let items: Vec<Vec<String>> = match &ad.kind {
            AdKind::Passenger { .. } | AdKind::Package => self
                .reply_list(ad)
                .iter()
                .map(|r| vec![PASSENGER_REPLIES.iter().find(|p| p.0 == *r).map_or("", |p| p.1).to_string()])
                .collect(),
            AdKind::Goods { good, name, price } => {
                let have = self.fitted.cargo.get(good).copied().unwrap_or(0);
                vec![
                    vec![format!("Sell 1 tonne of {name} for {}", money(*price)), format!("[{have} tonnes in cargo bay]")],
                    vec!["Hang up.".into()],
                ]
            }
            AdKind::Missing { person } => vec![
                vec![format!("{person} is in a cabin on my ship")],
                vec![format!("{person} has an ad. on this bulletin board")],
                vec![format!("I will look out for {person}")],
                vec!["Hang up.".into()],
            ],
            AdKind::Military => vec![],
        };
        responses(cv, a, &items, 10, hits);
    }

    pub(super) fn draw_federal(&self, cv: &mut Canvas, hits: &mut Vec<(Rect, Hit)>) {
        let a = &self.a;
        let Some(p) = &self.port else { return };
        let mut items: Vec<Vec<String>> = p
            .missions
            .iter()
            .map(|m| {
                let t = format!(
                    "Deliver some {} to the {} system [{},{}] by midnight on {}. You will be paid {}.",
                    m.what,
                    m.dest,
                    m.sector.0,
                    m.sector.1,
                    m.deadline,
                    credits(m.pay)
                );
                screens::wrap(a, &t, 178)
            })
            .collect();
        items.push(vec!["Hang up.".into()]);
        responses(cv, a, &items, 35, hits);
    }
}
