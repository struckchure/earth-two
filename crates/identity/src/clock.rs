//! Time without `std::time::SystemTime`/`Instant`, which panic on
//! wasm32-unknown-unknown. Proofs carry wall-clock expiries in microseconds;
//! latency and ping scheduling use a monotonic millisecond clock.

use std::time::Duration;

/// Wall-clock microseconds since the Unix epoch, as Go's `UnixMicro`.
#[cfg(not(target_arch = "wasm32"))]
pub fn now_unix_micro() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_micros() as i64)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
pub fn now_unix_micro() -> i64 {
    (js_sys::Date::now() * 1000.0) as i64
}

/// A proof expiry one minute from now, the lifetime every Go caller uses.
pub fn expires_in_a_minute() -> i64 {
    now_unix_micro() + 60_000_000
}

/// A monotonic timestamp for elapsed-time measurement.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Moment(f64);

impl Moment {
    #[cfg(not(target_arch = "wasm32"))]
    pub fn now() -> Moment {
        use std::sync::OnceLock;
        static START: OnceLock<std::time::Instant> = OnceLock::new();
        let start = START.get_or_init(std::time::Instant::now);
        Moment(start.elapsed().as_secs_f64() * 1000.0)
    }

    #[cfg(target_arch = "wasm32")]
    pub fn now() -> Moment {
        let performance = js_sys::Reflect::get(&js_sys::global(), &"performance".into())
            .ok()
            .filter(|p| !p.is_undefined());
        match performance {
            Some(p) => Moment(
                js_sys::Reflect::get(&p, &"now".into())
                    .ok()
                    .and_then(|f| f.dyn_into::<js_sys::Function>().ok())
                    .and_then(|f| f.call0(&p).ok())
                    .and_then(|v| v.as_f64())
                    .unwrap_or_else(js_sys::Date::now),
            ),
            None => Moment(js_sys::Date::now()),
        }
    }

    pub fn elapsed(self) -> Duration {
        Duration::from_secs_f64(((Moment::now().0 - self.0) / 1000.0).max(0.0))
    }

    /// Time left until `deadline`, zero once it has passed.
    pub fn until(self, deadline: Moment) -> Duration {
        Duration::from_secs_f64(((deadline.0 - self.0) / 1000.0).max(0.0))
    }

    pub fn plus(self, d: Duration) -> Moment {
        Moment(self.0 + d.as_secs_f64() * 1000.0)
    }

    pub fn is_before(self, other: Moment) -> bool {
        self.0 < other.0
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::JsCast;
