//! Landfall time: the real clock, UTC, as nanoseconds since 1970 (what Go's
//! `time.Time.UnixNano` gives), so the sun, the stars and the weather come
//! out the same for everyone, to the nanosecond, as the Go client has them.

pub const SECOND: i64 = 1_000_000_000;
pub const MINUTE: i64 = 60 * SECOND;
pub const HOUR: i64 = 60 * MINUTE;
pub const DAY: i64 = 24 * HOUR;

#[cfg(not(target_arch = "wasm32"))]
use std::time::{SystemTime, UNIX_EPOCH};
#[cfg(target_arch = "wasm32")]
use web_time::{SystemTime, UNIX_EPOCH};

/// A moment, as nanoseconds since 1970 UTC.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixTime(pub i64);

impl UnixTime {
    /// The real clock, now.
    pub fn now() -> Self {
        let since = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        Self(since.as_nanos() as i64)
    }

    /// A calendar moment, UTC (Go's `time.Date(..., time.UTC)`).
    pub fn utc(year: i64, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> Self {
        let days = days_from_civil(year, month, day);
        Self(
            days * DAY
                + i64::from(hour) * HOUR
                + i64::from(minute) * MINUTE
                + i64::from(second) * SECOND,
        )
    }

    /// `self` moved on by `nanos` (back, if negative).
    pub fn after(self, nanos: i64) -> Self {
        Self(self.0 + nanos)
    }

    /// `self` moved on by whole minutes, for stepping through a schedule.
    pub fn add_minutes(self, minutes: i64) -> Self {
        self.after(minutes * MINUTE)
    }

    /// `self` moved on by `hours`, with their fraction (Go's
    /// `Add(time.Duration(h * float64(time.Hour)))`).
    pub fn add_hours(self, hours: f64) -> Self {
        self.after((hours * HOUR as f64) as i64)
    }

    /// How long since `earlier`, in nanoseconds (Go's `Sub`).
    pub fn since(self, earlier: UnixTime) -> i64 {
        self.0 - earlier.0
    }

    /// Whole seconds since 1970, rounded down.
    pub fn unix_secs(self) -> i64 {
        self.0.div_euclid(SECOND)
    }

    /// Midnight at the start of this day, UTC.
    pub fn day_start(self) -> Self {
        Self(self.0.div_euclid(DAY) * DAY)
    }

    /// The hour, minute and second of the day, UTC.
    pub fn clock(self) -> (u32, u32, u32) {
        let sod = self.unix_secs().rem_euclid(24 * 3600);
        (
            (sod / 3600) as u32,
            ((sod % 3600) / 60) as u32,
            (sod % 60) as u32,
        )
    }

    /// The hour of the day, Landfall time (UTC), with its fraction: whole
    /// seconds, as Go's `Hour() + Minute()/60 + Second()/3600` has it.
    pub fn hour_of(self) -> f64 {
        let (h, m, s) = self.clock();
        f64::from(h) + f64::from(m) / 60.0 + f64::from(s) / 3600.0
    }

    /// The year, month and day, UTC.
    pub fn date(self) -> (i64, u32, u32) {
        civil_from_days(self.0.div_euclid(DAY))
    }

    /// The day of the year, from 1 (Go's `YearDay`).
    pub fn year_day(self) -> u32 {
        let (year, _, _) = self.date();
        (self.0.div_euclid(DAY) - days_from_civil(year, 1, 1) + 1) as u32
    }
}

/// A duration's seconds with their fraction (Go's `Duration.Seconds`).
pub fn seconds(nanos: i64) -> f64 {
    let sec = nanos / SECOND;
    let nsec = nanos % SECOND;
    sec as f64 + nsec as f64 / 1e9
}

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's
/// algorithm).
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let m = i64::from(month);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + i64::from(day) - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

/// The date of a day counted from 1970-01-01.
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = z.div_euclid(146097);
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calendar_round_trips() {
        let t = UnixTime::utc(2026, 10, 6, 14, 30, 7);
        assert_eq!(t.date(), (2026, 10, 6));
        assert_eq!(t.clock(), (14, 30, 7));
        assert_eq!(t.year_day(), 279);
        assert_eq!(UnixTime::utc(1970, 1, 1, 0, 0, 0).0, 0);
        assert_eq!(UnixTime::utc(2000, 3, 1, 0, 0, 0).unix_secs(), 951_868_800);
        assert_eq!(t.day_start(), UnixTime::utc(2026, 10, 6, 0, 0, 0));
    }
}
