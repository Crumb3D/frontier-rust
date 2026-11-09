//! Stardate clock and time compression.

pub const SECONDS_PER_DAY: f64 = 86400.0;

pub const COMPRESSION_LEVELS: &[i32] = &[
    1, 10, 100, 1000, 10000, 2, 4, 8, 16, 32, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384,
    32768, 65536, 100000,
];

pub const STARDREAMER_FACTORS: &[i32] = &[1, 10, 100, 1000, 10000];

/// Game clock starting 1 January 3200.
#[derive(Clone, Debug, Default)]
pub struct Stardate {
    pub days: f64,
}

impl Stardate {
    pub fn new(days: f64) -> Self {
        Self { days }
    }

    /// Advance by wall-time seconds at compression. Returns sim seconds.
    pub fn advance(&mut self, real_dt: f64, compression: i32) -> f64 {
        let sim_dt = real_dt * compression as f64;
        self.days += sim_dt / SECONDS_PER_DAY;
        sim_dt
    }

    pub fn total_seconds(&self) -> f64 {
        self.days * SECONDS_PER_DAY
    }

    /// The console clock, e.g. `12:00:05 1-Jan-3200` (Gregorian calendar).
    pub fn format(&self) -> String {
        let secs = (self.days.max(0.0) * SECONDS_PER_DAY + 1e-6) as i64;
        let mut day_no = secs / 86400;
        let sod = secs % 86400;
        let (hour, minute, second) = (sod / 3600, sod / 60 % 60, sod % 60);
        let mut year = 3200;
        loop {
            let len = if is_leap(year) { 366 } else { 365 };
            if day_no < len {
                break;
            }
            day_no -= len;
            year += 1;
        }
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        let mut month = 0;
        loop {
            let len = month_len(year, month);
            if day_no < len || month == 11 {
                break;
            }
            day_no -= len;
            month += 1;
        }
        format!("{hour}:{minute:02}:{second:02} {}-{}-{year}", day_no + 1, MONTHS[month])
    }
}

fn is_leap(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

fn month_len(y: i64, m: usize) -> i64 {
    match m {
        1 if is_leap(y) => 29,
        1 => 28,
        3 | 5 | 8 | 10 => 30,
        _ => 31,
    }
}

/// Manages compression level with auto-drop near threats/bodies.
#[derive(Clone, Debug, Default)]
pub struct TimeCompression {
    pub index: usize,
    pub force_realtime: bool,
}

impl TimeCompression {
    pub fn factor(&self) -> i32 {
        if self.force_realtime {
            1
        } else {
            COMPRESSION_LEVELS[self.index]
        }
    }

    pub fn selected_factor(&self) -> i32 {
        COMPRESSION_LEVELS[self.index]
    }

    pub fn faster(&mut self) {
        if self.index + 1 < COMPRESSION_LEVELS.len() {
            self.index += 1;
        }
    }

    pub fn slower(&mut self) {
        if self.index > 0 {
            self.index -= 1;
        }
    }

    pub fn set_index(&mut self, index: usize) {
        self.index = index.min(COMPRESSION_LEVELS.len() - 1);
    }

    pub fn set_vcr(&mut self, vcr_slot: i32) {
        let target = match vcr_slot {
            1 => 10,
            2 => 100,
            3 => 1000,
            4 => 10000,
            _ => return,
        };
        if let Some(i) = COMPRESSION_LEVELS.iter().position(|&f| f == target) {
            self.index = i;
        }
    }

    pub fn set_stardreamer(&mut self, tier: i32) {
        let tier = tier.clamp(0, 4) as usize;
        let factor = STARDREAMER_FACTORS[tier];
        if let Some(i) = COMPRESSION_LEVELS.iter().position(|&f| f == factor) {
            self.index = i;
        }
    }

    pub fn vcr_slot(&self) -> i32 {
        let f = self.selected_factor();
        if f <= 10 {
            1
        } else if f <= 100 {
            2
        } else if f <= 1000 {
            3
        } else {
            4
        }
    }

    pub fn reset(&mut self) {
        self.index = 0;
    }

    pub fn update_auto(&mut self, near_body: bool, in_combat: bool) {
        self.force_realtime = near_body || in_combat;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stardate_format_contains_3200() {
        let s = Stardate::new(0.0);
        assert!(s.format().contains("3200"));
    }

    #[test]
    fn stardate_matches_original_opening() {
        let s = Stardate::new(0.5 + 5.0 / SECONDS_PER_DAY);
        assert_eq!(s.format(), "12:00:05 1-Jan-3200");
        // 3200 is a leap year: day 59 is 29 Feb.
        assert_eq!(Stardate::new(59.0).format(), "0:00:00 29-Feb-3200");
        assert_eq!(Stardate::new(366.25).format(), "6:00:00 1-Jan-3201");
    }
}
