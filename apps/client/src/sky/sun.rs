//! The sky: where the Red is. It's TRAPPIST-1e, the fourth of seven planets
//! round TRAPPIST-1, a red dwarf 40.7 light-years away in Aquarius. The
//! star is small and cool but so close that it's 2.2° across from here,
//! and orange-red. The other six planets are so near that several show
//! discs the size of the Moon, and wander along the ecliptic, the band the
//! sun sits on. They're placed from their real orbits (Agol et al. 2021) at
//! the real time, and drawn ten times their size: there to say this isn't
//! Earth. They keep their sizes to each other, and the sun is its real size.
//!
//! The Red turns once a real day here (docs/decisions.md): the sun rises in
//! the east about 05:00, Landfall time (UTC), crosses the south no higher
//! than `SUN_HIGH` at noon, sets in the west about 19:00, and is down
//! through the night, lowest at midnight.

use super::{
    colour::{Rgba, clamp01, mix_colour, powf, rgb},
    time::UnixTime,
};
use bevy::math::Vec3;
use earth_two_world::terrain::smoothstep;
use std::f64::consts::PI;

/// A planet of TRAPPIST-1: its letter, how far it orbits (AU), how long it
/// takes (days), its radius (Earth radii), and where it was in its orbit
/// (degrees from the Red's, as the sun sees it) at `sky_epoch`.
#[derive(Clone, Copy, Debug)]
pub struct Exoplanet {
    pub name: &'static str,
    pub a: f64,
    pub p: f64,
    pub r: f64,
    pub start_deg: f64,
}

const fn planet(name: &'static str, a: f64, p: f64, r: f64, start_deg: f64) -> Exoplanet {
    Exoplanet {
        name,
        a,
        p,
        r,
        start_deg,
    }
}

/// TRAPPIST-1e.
pub const THE_RED: Exoplanet = planet("e", 0.02925, 6.101, 0.920, 0.0);

/// Its neighbours. Where they start is ours to say: nobody knows.
pub const NEIGHBOURS: [Exoplanet; 6] = [
    planet("b", 0.01154, 1.511, 1.116, 200.0), // too hot for water: Venus-like
    planet("c", 0.01580, 2.422, 1.097, 290.0),
    planet("d", 0.02227, 4.049, 0.788, 110.0), // maybe an ocean world
    planet("f", 0.03849, 9.208, 1.045, 70.0),  // past the snow line: ice, maybe water
    planet("g", 0.04683, 12.352, 1.129, 150.0),
    planet("h", 0.06189, 18.773, 0.755, 250.0),
];

/// When the neighbours were where `start_deg` says.
pub fn sky_epoch() -> UnixTime {
    UnixTime::utc(2026, 10, 5, 0, 0, 0)
}

pub const AU_KM: f64 = 1.496e8;
pub const EARTH_KM: f64 = 6371.0;
/// TRAPPIST-1's radius.
pub const SUN_KM: f64 = 0.1192 * 696000.0;
/// How much bigger the planets are drawn than they look.
pub const PLANET_SCALE: f64 = 10.0;
// The dome, and the sun and planets in it, are beyond the horizon (the
// world's edge is 16 km off at most) and inside the camera's far plane.
pub const SKY_RADIUS: f32 = 19800.0;
pub const SUN_DISTANCE: f32 = 19600.0;
pub const BODY_DISTANCE: f32 = 19300.0;

pub const SUN_HIGH: f64 = 30.0 * PI / 180.0;
/// Raises the sun's path above the horizon's middle: the days are longer
/// than the nights.
pub const SUN_LIFT: f64 = 6.0 * PI / 180.0;

/// How high the ecliptic rises above the horizon: the planets wander no
/// higher, where the camera can look.
pub const ECLIPTIC_TILT: f64 = 32.0 * PI / 180.0;

/// Scales sunlight: past white on what faces the sun, so the low sun reads
/// as bright against the shadows.
pub const SUN_BRIGHTNESS: f32 = 1.3;

// The sky's colours: overhead, and at the horizon, thin dusty air lit by a
// red sun (docs/look-and-feel.md: pale ochre by day, violet at dusk).
pub const SKY_TOP: Rgba = rgb(190, 136, 98);
pub const SKY_HORIZON: Rgba = rgb(246, 204, 150);
// The glare round the sun, and the light it gives: a red dwarf's, warm
// orange.
pub const GLARE_COLOUR: Rgba = rgb(255, 238, 200);
pub const SUNLIGHT: Rgba = rgb(255, 182, 128);
// At dusk: violet overhead, rose at the horizon, the glare and the light
// deep orange, and the fill from the sky violet.
pub const DUSK_TOP: Rgba = rgb(92, 70, 124);
pub const DUSK_HORIZON: Rgba = rgb(226, 144, 126);
pub const DUSK_GLARE: Rgba = rgb(255, 168, 104);
pub const DUSK_SUN: Rgba = rgb(255, 116, 64);
pub const DUSK_FILL: Rgba = rgb(150, 118, 206);
// At night: the sky near black with a deep blue at the horizon, and the
// faint, cold light off the neighbour planets.
pub const NIGHT_TOP: Rgba = rgb(8, 8, 18);
pub const NIGHT_HORIZON: Rgba = rgb(30, 28, 52);
pub const NIGHT_FILL: Rgba = rgb(84, 90, 146);
pub const PLANET_LIGHT: Rgba = rgb(120, 128, 190);

/// The way to the sun at `t`.
pub fn sun_at(t: UnixTime) -> Vec3 {
    let h = t.hour_of();
    let turn = 2.0 * PI * (h - 6.0) / 24.0; // from the east (+X) through the south (+Z) at noon
    let up = SUN_LIFT + (SUN_HIGH - SUN_LIFT) * turn.sin();
    let across = Vec3::new(turn.cos() as f32, 0.0, turn.sin() as f32);
    across * up.cos() as f32 + Vec3::new(0.0, up.sin() as f32, 0.0)
}

/// How much of dusk's (or dawn's) colour there is with the sun the way
/// `dir`: none with it well up, all of it as it nears the horizon.
pub fn dusk_at(dir: Vec3) -> f32 {
    smoothstep(
        (12.0 * PI / 180.0) as f32,
        (2.0 * PI / 180.0) as f32,
        f64::from(dir.y).asin() as f32,
    )
}

/// How far into night it is with the sun the way `dir`: none until it's
/// set, all of it once it's well down.
pub fn night_at(dir: Vec3) -> f32 {
    smoothstep(
        (-PI / 180.0) as f32,
        (-10.0 * PI / 180.0) as f32,
        f64::from(dir.y).asin() as f32,
    )
}

/// The way the scene's light comes from, with the sun the way `sun` and
/// `night` of the way into night: the sun's, until it's down, and then the
/// planets', from above where the sun went (so they show it lit on the side
/// towards it), never from under the ground.
pub fn light_from(sun: Vec3, night: f32) -> Vec3 {
    let flat = Vec3::new(sun.x, 0.0, sun.z).normalize_or_zero();
    let mut up = f64::from(sun.y)
        .asin()
        .max(25.0 * PI / 180.0 * f64::from(night));
    up = up.max(3.0 * PI / 180.0);
    flat * up.cos() as f32 + Vec3::new(0.0, up.sin() as f32, 0.0)
}

/// The band the sun and planets move on, as two directions in it: the sun,
/// `s`, and `v` a quarter of the way round from it. It runs through the sun
/// and rises no higher than `ECLIPTIC_TILT`.
pub fn ecliptic(sun: Vec3) -> (Vec3, Vec3) {
    let s = sun;
    // Its normal leans ECLIPTIC_TILT off the vertical, the way that keeps
    // the sun on it.
    let flat = Vec3::new(s.x, 0.0, s.z).normalize_or_zero();
    let side = Vec3::new(-flat.z, 0.0, flat.x);
    let cos_phi = -f64::from(s.y).asin().tan() / ECLIPTIC_TILT.tan();
    let phi = cos_phi.clamp(-1.0, 1.0).acos();
    let k = flat * phi.cos() as f32 + side * phi.sin() as f32;
    let n = Vec3::Y * ECLIPTIC_TILT.cos() as f32 + k * ECLIPTIC_TILT.sin() as f32;
    (s, n.cross(s).normalize_or_zero())
}

/// Where a neighbour is in the sky `days` after `sky_epoch`, with the sun
/// the way `sun`: the way to it, and its angular radius (radians), drawn
/// size and all.
pub fn neighbour_at(n: &Exoplanet, sun: Vec3, days: f64) -> (Vec3, f64) {
    // In the Red's frame, which turns with it: the sun is off along -X.
    let turn = n.start_deg * PI / 180.0 + 2.0 * PI * days * (1.0 / n.p - 1.0 / THE_RED.p);
    let (rx, ry) = (n.a * turn.cos() - THE_RED.a, n.a * turn.sin());
    let dist = rx.hypot(ry) * AU_KM;
    let (s, v) = ecliptic(sun);
    let dir = (s * (-rx) as f32 + v * ry as f32).normalize_or_zero();
    (dir, (n.r * EARTH_KM / dist).min(1.0).asin() * PLANET_SCALE)
}

/// The sun's angular radius, in radians.
pub fn sun_radius() -> f64 {
    (SUN_KM / (THE_RED.a * AU_KM)).asin()
}

/// How many days it is at `now` since `sky_epoch`, on the real clock: the
/// world runs in real time (docs/decisions.md).
pub fn sky_days(now: UnixTime) -> f64 {
    now.since(sky_epoch()) as f64 / 1e9 / 3600.0 / 24.0
}

/// The sky, without the sun, at elevation `up` (radians), `dusk` of the
/// way into dusk's colours and `night` into night's: overhead down to the
/// horizon, and the horizon's colour below.
pub fn sky_colour(up: f64, dusk: f32, night: f32) -> Rgba {
    let t = clamp01((1.0 - up / (PI / 2.0)) as f32);
    let (mut top, mut horizon) = (
        mix_colour(SKY_TOP, DUSK_TOP, dusk),
        mix_colour(SKY_HORIZON, DUSK_HORIZON, dusk),
    );
    top = mix_colour(top, NIGHT_TOP, night);
    horizon = mix_colour(horizon, NIGHT_HORIZON, night);
    mix_colour(top, horizon, powf(t, 2.2))
}

/// How much of the sun's glare there is off from it by `off` (radians):
/// dense close in, thinning out, and a wide warmth round it.
pub fn glare(off: f64) -> f32 {
    let deg = off * 180.0 / PI;
    clamp01(
        (0.95 * (-deg / 1.8).exp() + 0.5 * (-deg / 7.0).exp() + 0.18 * (-deg / 30.0).exp()) as f32,
    )
}
