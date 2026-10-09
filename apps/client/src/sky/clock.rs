//! The world's clock: Landfall time (UTC, as the world runs on the real
//! clock), or the sun held at an hour for looking at it (EARTH_TWO_HOUR,
//! and the test panel's Time of day switch); and what the time of day's
//! called, and the day as a strip for the HUD's clock (night dark, day
//! light, dusk and dawn between).

use super::{
    colour::{Rgba, mix_colour, rgb},
    sun::{dusk_at, night_at, sun_at},
    time::{HOUR, UnixTime},
};
use bevy::prelude::*;

/// A resource: the hour the sun's held at, or -1 to follow the clock.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Clock {
    pub hour: f32,
}

impl Default for Clock {
    fn default() -> Self {
        Self { hour: -1.0 }
    }
}

impl Clock {
    /// The clock as EARTH_TWO_HOUR (0 to 24) has it, or following the real
    /// clock.
    pub fn from_env() -> Self {
        Self::from_setting(std::env::var("EARTH_TWO_HOUR").ok().as_deref())
    }

    /// The clock as a setting of EARTH_TWO_HOUR has it.
    pub fn from_setting(setting: Option<&str>) -> Self {
        let hour = setting
            .and_then(|v| v.trim().parse::<f64>().ok())
            .map_or(-1.0, |v| (v.max(0.0) % 24.0) as f32);
        Self { hour }
    }

    /// The sun held at `hour`.
    pub fn held(hour: f32) -> Self {
        Self { hour }
    }

    /// When the sun's at: the clock's time, or the held hour today.
    pub fn now(&self) -> UnixTime {
        self.at(UnixTime::now())
    }

    /// When the sun's at, with the real clock at `real`.
    pub fn at(&self, real: UnixTime) -> UnixTime {
        if self.hour < 0.0 {
            return real;
        }
        real.day_start()
            .after((f64::from(self.hour) * HOUR as f64) as i64)
    }
}

/// The test panel's settings: the clock, or the sun held in the day, at
/// dusk, in the night, or at dawn.
pub const DAYLIGHT_HOURS: [(&str, f32); 5] = [
    ("Real clock", -1.0),
    ("Day", 11.0),
    ("Dusk", 18.5),
    ("Night", 0.0),
    ("Dawn", 5.5),
];

/// What the time of day's called, with the sun the way `sun` at `hour`.
pub fn part_of_day(sun: Vec3, hour: f64) -> &'static str {
    if night_at(sun) >= 0.5 {
        "Night"
    } else if dusk_at(sun) >= 0.35 && hour < 12.0 {
        "Dawn"
    } else if dusk_at(sun) >= 0.35 {
        "Dusk"
    } else {
        "Day"
    }
}

/// The day as the HUD's strip shows it: each hour's colour, from midnight.
pub fn day_strip() -> [Rgba; 24] {
    let day = UnixTime::utc(2026, 10, 6, 0, 0, 0);
    let mut out = [Rgba::default(); 24];
    for (h, c) in out.iter_mut().enumerate() {
        let sun = sun_at(day.add_hours(h as f64 + 0.5));
        let dusk = mix_colour(rgb(232, 196, 140), rgb(214, 120, 120), dusk_at(sun));
        *c = mix_colour(dusk, rgb(44, 42, 74), night_at(sun));
    }
    out
}
