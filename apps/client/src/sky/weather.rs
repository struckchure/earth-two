//! The weather: dust, the Red's only weather (docs/look-and-feel.md). It's
//! mostly clear. Now and then a dusty spell comes through, the air thick
//! and brown and the far off gone; once in a while a dust storm, the light
//! brown and close: the haze draws in to a couple of hundred metres, the
//! sky and the sun go behind the dust, the sun dims and the shadows with
//! it, and dust blows across the view. Under the dome the air's filtered,
//! so the blown dust is mostly kept out, though the haze still closes in.
//!
//! The weather comes by the real clock, as everything in the shared world
//! does (docs/shared-world.md), so everyone has the same: each
//! WEATHER_SLOT may bring one spell (SPELLS), at a time, for a length and
//! as thick as its hash says. How thick it is, 0 to 1, is the weather's
//! `storm`. EARTH_TWO_STORM, from 0 to 1, holds the weather that thick
//! instead, for looking at it, as the test panel's Weather switch does.

use super::{
    colour::{Rgba, clamp01, mix_colour, rgb},
    daylight::Daylight,
    sun::{
        BODY_DISTANCE, DUSK_FILL, DUSK_HORIZON, DUSK_SUN, NIGHT_FILL, NIGHT_HORIZON, PLANET_LIGHT,
        SKY_HORIZON, SUN_BRIGHTNESS, SUNLIGHT,
    },
    time::{HOUR, MINUTE, UnixTime, seconds},
};
use bevy::prelude::*;
use earth_two_world::terrain::{lattice, smoothstep};

pub const WEATHER_SLOT: i64 = 3 * HOUR;
/// How far the haze reaches (as `Haze::distance`) in the hardest storm.
pub const STORM_REACH: f32 = 70.0;
/// How quickly the weather follows a change it's told to make at once (the
/// forced storm's), the share 1 - e^-STORM_EASE a second.
pub const STORM_EASE: f32 = 1.5;
/// How thick the air is for it to be dusty, and a storm.
pub const DUSTY_FROM: f32 = 0.15;
pub const STORM_FROM: f32 = 0.6;

/// A spell of weather a slot may bring: what it's called, how likely it
/// is, how long it lasts (nanoseconds), how thick it gets at its height,
/// and how long it takes to blow up and to die down.
#[derive(Clone, Copy, Debug)]
pub struct Spell {
    pub name: &'static str,
    pub chance: f32,
    pub shortest: i64,
    pub longest: i64,
    pub thinnest: f32,
    pub thickest: f32,
    pub rise: i64,
}

/// The slots' weather, by how likely: the rest of the time it's clear.
pub const SPELLS: [Spell; 2] = [
    Spell {
        name: "Dust storm",
        chance: 0.06,
        shortest: 30 * MINUTE,
        longest: 90 * MINUTE,
        thinnest: 0.75,
        thickest: 1.0,
        rise: 5 * MINUTE,
    },
    Spell {
        name: "Dusty",
        chance: 0.2,
        shortest: 60 * MINUTE,
        longest: 150 * MINUTE,
        thinnest: 0.28,
        thickest: 0.45,
        rise: 12 * MINUTE,
    },
];

// What a storm turns things towards: the dust's brown, for the haze and
// the sky; the light through it, for the sun and the fill.
pub const STORM_DUST: Rgba = rgb(150, 96, 64);
pub const STORM_SUN: Rgba = rgb(222, 150, 104);
pub const STORM_FILL: Rgba = rgb(196, 138, 100);
/// A storm by night: the dust dark, the haze it makes near black.
pub const NIGHT_DUST: Rgba = rgb(40, 30, 28);

/// A resource: the dust in the air as the shaders draw it. `distance` is
/// how far the haze goes a share 1 - 1/e of the way; `end` where what's
/// past it (the sky, the sun and the planets) begins; `veil` how much of
/// the dust hides what's past the end, 0 in clear air. Starts as the Go
/// shading plugin's haze; `blow` moves it with the hour and the weather.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Haze {
    pub color: Rgba,
    pub distance: f32,
    pub end: f32,
    pub veil: f32,
}

impl Default for Haze {
    fn default() -> Self {
        Self {
            color: SKY_HORIZON,
            distance: 1600.0,
            end: BODY_DISTANCE - 200.0,
            veil: 0.0,
        }
    }
}

/// A resource: the fill from the dusty sky, dimmer than the sun and
/// violet (the Go `render.AmbientLight`, in the shader's own scale).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct Ambient {
    pub color: Rgba,
    pub brightness: f32,
}

impl Default for Ambient {
    fn default() -> Self {
        Self {
            color: rgb(206, 186, 222),
            brightness: 0.26,
        }
    }
}

/// A resource: the sun's light this frame, its colour and brightness (the
/// Go `render.DirectionalLight`, in the shader's own scale, past white on
/// what faces the sun).
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct SunLight {
    pub color: Rgba,
    pub brightness: f32,
}

impl Default for SunLight {
    fn default() -> Self {
        Self {
            color: SUNLIGHT,
            brightness: SUN_BRIGHTNESS,
        }
    }
}

/// The clear day's haze, fill and sun, that the hour and the weather are
/// made from.
#[derive(Clone, Copy, Debug)]
struct Clear {
    haze: Haze,
    ambient: Ambient,
    sun: Rgba,
    brightest: f32,
}

/// A resource: how thick the dust is, 0 to 1 (a storm from STORM_FROM), a
/// storm held (EARTH_TWO_STORM, or -1), what it was all made from (the
/// clear day's haze, sun and fill), and the clock the blown dust and the
/// stars' twinkle are drawn by.
#[derive(Resource, Clone, Copy, Debug)]
pub struct Weather {
    pub storm: f32,
    pub forced: f32,
    pub t: f32,
    clear: Option<Clear>,
}

impl Default for Weather {
    fn default() -> Self {
        Self {
            storm: 0.0,
            forced: -1.0,
            t: 0.0,
            clear: None,
        }
    }
}

impl Weather {
    /// The weather, held as thick as EARTH_TWO_STORM (0 to 1) if it's set.
    pub fn from_env() -> Self {
        Self::from_setting(std::env::var("EARTH_TWO_STORM").ok().as_deref())
    }

    /// The weather as a setting of EARTH_TWO_STORM has it.
    pub fn from_setting(setting: Option<&str>) -> Self {
        let mut w = Self::default();
        if let Some(v) = setting.and_then(|v| v.trim().parse::<f64>().ok()) {
            w.forced = clamp01(v as f32);
            w.storm = w.forced;
        }
        w
    }

    /// Holds the weather this thick (or lets it go, below 0): the test
    /// panel's Weather switch.
    pub fn hold(&mut self, storm: f32) {
        self.forced = if storm < 0.0 { -1.0 } else { clamp01(storm) };
    }
}

/// How thick the air is at `t`, 0 to 1, by the schedule.
pub fn storm_at(t: UnixTime) -> f32 {
    let slot = t.0 / WEATHER_SLOT;
    // A spell late in the slot before may still be blowing.
    for n in [slot, slot - 1] {
        let Some((sp, start, length, peak)) = spell_in(n) else {
            continue;
        };
        let (since, until) = (t.since(start), start.after(length).since(t));
        if since < 0 || until < 0 {
            continue;
        }
        let rise = seconds(sp.rise) as f32;
        return peak
            * smoothstep(0.0, rise, seconds(since) as f32)
            * smoothstep(0.0, rise, seconds(until) as f32);
    }
    0.0
}

/// The spell of weather in a slot, if it has one: which, when it starts,
/// how long it lasts (nanoseconds) and how thick it gets.
pub fn spell_in(slot: i64) -> Option<(&'static Spell, UnixTime, i64, f32)> {
    let h = |k: u32| lattice(slot as i32, (slot >> 31) as i32, 0x5701 + k);
    let mut pick = h(0);
    for s in &SPELLS {
        if pick < s.chance {
            let length = s.shortest + (h(1) * (s.longest - s.shortest) as f32) as i64;
            let start =
                UnixTime(slot * WEATHER_SLOT).after((h(2) * (WEATHER_SLOT - length) as f32) as i64);
            return Some((
                s,
                start,
                length,
                s.thinnest + (s.thickest - s.thinnest) * h(3),
            ));
        }
        pick -= s.chance;
    }
    None
}

/// What the weather's called, as thick as `storm` is.
pub fn conditions(storm: f32) -> &'static str {
    if storm >= STORM_FROM {
        "Dust storm"
    } else if storm >= DUSTY_FROM {
        "Dusty"
    } else {
        "Clear"
    }
}

/// The wind the dust blows on: from the east (from +X).
pub fn dust_wind() -> Vec3 {
    Vec3::new(-1.0, 0.0, 0.25)
}

/// The dome's walls on the ground, as the terrain has them: inside, the
/// air's filtered.
const DOME_WALLS: (f32, f32, f32, f32) = (-54.0, -56.0, 88.0, 100.0);

/// How much of the blown dust reaches the camera at `at`: all of it
/// outside, little under the dome, whose glass keeps it out.
pub fn outdoors(at: Vec3) -> f32 {
    let (x, z, w, h) = DOME_WALLS;
    let inside = at.x >= x && at.x <= x + w && at.z >= z && at.z <= z + h;
    if at.y < 30.0 && inside { 0.12 } else { 1.0 }
}

/// Sets the weather by the clock (or the storm held), and the haze, the
/// sun and the fill by it, on the light of the hour (daylight.rs).
pub fn blow(
    mut w: ResMut<Weather>,
    day: Res<Daylight>,
    mut haze: ResMut<Haze>,
    mut ambient: ResMut<Ambient>,
    mut sun: ResMut<SunLight>,
    time: Res<Time>,
) {
    let want = if w.forced >= 0.0 {
        w.forced
    } else {
        storm_at(UnixTime::now())
    };
    blow_to(
        &mut w,
        &day,
        &mut haze,
        &mut ambient,
        &mut sun,
        want,
        time.delta_secs(),
    );
}

/// `blow`, towards the storm `want`, over `dt` seconds.
pub fn blow_to(
    w: &mut Weather,
    day: &Daylight,
    hz: &mut Haze,
    am: &mut Ambient,
    sun: &mut SunLight,
    want: f32,
    dt: f32,
) {
    let (dusk, night) = (day.dusk, day.night);
    w.t += dt;
    let c = *w.clear.get_or_insert(Clear {
        haze: *hz,
        ambient: *am,
        sun: SUNLIGHT,
        brightest: SUN_BRIGHTNESS,
    });
    // The schedule moves slowly enough as it is; a held storm comes in
    // over a second or two.
    w.storm += (want - w.storm) * (1.0 - f64::from(-STORM_EASE * dt).exp()) as f32;
    let s = w.storm;

    // The clear air at this hour: at dusk the haze takes the horizon's
    // rose, the fill goes violet (and stronger: the sky's the light then),
    // and the low sun deep orange and dimmer. At night it's dark: the haze
    // the night's horizon, a faint cold fill, and only the planets' light.
    let clear_haze = mix_colour(
        mix_colour(c.haze.color, DUSK_HORIZON, dusk),
        NIGHT_HORIZON,
        night,
    );
    let clear_fill = mix_colour(
        mix_colour(c.ambient.color, DUSK_FILL, dusk),
        NIGHT_FILL,
        night,
    );
    let fill = c.ambient.brightness * (1.0 + 0.35 * dusk) * (1.0 - 0.2 * night);
    let clear_sun = mix_colour(mix_colour(c.sun, DUSK_SUN, dusk), PLANET_LIGHT, night);
    let bright = c.brightest * (1.0 - 0.4 * dusk) * (1.0 - 0.75 * night);
    // And the storm on it, its dust as dark as the hour.
    let dust = mix_colour(STORM_DUST, NIGHT_DUST, night);
    hz.color = mix_colour(clear_haze, dust, s);
    // Drawn in towards STORM_REACH, by the same share of the way each step.
    hz.distance =
        c.haze.distance * f64::from(STORM_REACH / c.haze.distance).powf(f64::from(s)) as f32;
    hz.veil = 0.97 * smoothstep(0.0, 0.8, s);
    am.color = mix_colour(clear_fill, mix_colour(STORM_FILL, NIGHT_FILL, night), s);
    am.brightness = fill * (1.0 + 0.7 * s);
    sun.color = mix_colour(clear_sun, STORM_SUN, s);
    sun.brightness = bright * (1.0 - 0.8 * s);
}
