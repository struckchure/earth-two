//! The night sky. The stars are points, not a picture of them: a fixed
//! catalogue of STAR_COUNT, most faint and a few bright, coloured by how
//! hot they are (blue-white, white, yellow, orange, the odd red), thicker
//! along the Milky Way, drawn a pixel each with the brightest given a faint
//! sparkle. They wheel round the Red's pole through the night as it turns,
//! twinkle (more low down, through more air), fade into the dust near the
//! horizon, and go behind any dust in the air. The Milky Way itself, a
//! faint band with dark lanes of dust down it and a brighter core, and the
//! airglow low round the horizon, are painted into the sky (skytex.rs),
//! turned the same way.

use super::{
    colour::{Rgba, rgb},
    time::UnixTime,
};
use bevy::math::{Quat, Vec3};
use earth_two_world::terrain::{fbm, lattice, smoothstep};
use std::{f64::consts::PI, sync::LazyLock};

pub const STAR_COUNT: usize = 9000;
/// More, faint, along the Milky Way.
pub const STAR_BAND: usize = 5000;
pub const STARS_RADIUS: f32 = 19700.0;
/// The Red's pole is due north (-Z), POLE_UP above the horizon (as
/// Landfall's latitude would put it); the sky turns once a day about it.
pub const POLE_UP: f64 = 38.0 * PI / 180.0;

/// On the desktop every star's drawn, each a small cross. In the browser
/// each line drawn costs, so only the brightest are drawn, each a single
/// dot (Go's stars_budget.go and stars_budget_js.go).
#[cfg(not(target_arch = "wasm32"))]
pub const STAR_BUDGET: usize = 1 << 30;
#[cfg(not(target_arch = "wasm32"))]
pub const STARS_CROSSED: bool = true;
#[cfg(target_arch = "wasm32")]
pub const STAR_BUDGET: usize = 2500;
#[cfg(target_arch = "wasm32")]
pub const STARS_CROSSED: bool = false;

/// One of the catalogue's: the way to it in the sky's own frame (turned to
/// where it is now by `sky_turn`), how bright it is (0 to 1), its colour,
/// and how it twinkles.
#[derive(Clone, Copy, Debug)]
pub struct Star {
    pub dir: Vec3,
    pub bright: f32,
    pub colour: Rgba,
    pub phase: f32,
}

/// The pole the sky's own frame turns about.
pub fn sky_pole() -> Vec3 {
    Vec3::new(0.0, POLE_UP.sin() as f32, -(POLE_UP.cos() as f32))
}

/// The Milky Way's plane (its normal), in the sky's own frame.
pub fn galaxy_pole() -> Vec3 {
    Vec3::new(0.35, 0.3, 0.89).normalize()
}

/// The Milky Way's core, in the sky's own frame.
pub fn galaxy_core() -> Vec3 {
    galaxy_pole().cross(Vec3::Y).normalize()
}

/// The stars by how hot they are: the share of each, and its colour.
pub const STAR_CLASSES: [(f32, Rgba); 5] = [
    (0.14, rgb(178, 200, 255)), // hot, blue-white
    (0.4, rgb(236, 238, 255)),  // white
    (0.27, rgb(255, 240, 210)), // yellow
    (0.15, rgb(255, 204, 150)), // orange
    (0.04, rgb(255, 156, 120)), // red
];

static STARS: LazyLock<Vec<Star>> = LazyLock::new(catalogue);

/// The catalogue: the same on every run, brightest first (so a budget,
/// STAR_BUDGET, keeps the ones that show).
pub fn stars() -> &'static [Star] {
    &STARS
}

fn catalogue() -> Vec<Star> {
    let mut out = Vec::with_capacity(STAR_COUNT + STAR_BAND);
    let h = |i: usize, k: u32| lattice(i as i32, k as i32, 0x5ea7);
    let (pole, core) = (galaxy_pole(), galaxy_core());
    for i in 0..STAR_COUNT + STAR_BAND {
        let dir = if i < STAR_COUNT {
            // Even over the sphere.
            let (z, a) = (2.0 * h(i, 1) - 1.0, 2.0 * PI * f64::from(h(i, 2)));
            let r = f64::from(1.0 - z * z).sqrt() as f32;
            Vec3::new(r * a.cos() as f32, r * a.sin() as f32, z)
        } else {
            // Close about the Milky Way's plane.
            let a = 2.0 * PI * f64::from(h(i, 1));
            let off = (h(i, 2) - 0.5) * 0.25;
            let across = pole.cross(core).normalize_or_zero();
            let inward = core * a.cos() as f32 + across * a.sin() as f32;
            (inward + pole * off).normalize_or_zero()
        };
        // Most faint: brightness falls away as a power, as magnitudes do.
        let mut b = f64::from(h(i, 3)).powf(6.0) as f32;
        if i >= STAR_COUNT {
            b *= 0.4;
        }
        let mut pick = h(i, 4);
        let mut colour = STAR_CLASSES[1].1;
        for (share, c) in STAR_CLASSES {
            if pick < share {
                colour = c;
                break;
            }
            pick -= share;
        }
        out.push(Star {
            dir,
            bright: 0.22 + 0.78 * b,
            colour,
            phase: (2.0 * PI * f64::from(h(i, 5))) as f32,
        });
    }
    out.sort_by(|a, b| b.bright.total_cmp(&a.bright));
    out
}

/// How far the sky's turned about its pole at `t`: once a day.
pub fn sky_turn(t: UnixTime) -> f32 {
    (2.0 * PI * t.hour_of() / 24.0) as f32
}

/// Turns a way in the sky's own frame to where it is in the sky, turned by
/// `turn`.
pub fn sky_frame(turn: f32) -> Quat {
    Quat::from_axis_angle(sky_pole(), -turn)
}

/// How bright the Milky Way is the way `dir` (in the sky's own frame), 0 to
/// about 1: a band along its plane, brighter and wider towards its core,
/// mottled, with dark lanes of dust down its middle.
pub fn milky_way(dir: Vec3) -> f32 {
    let off = dir.dot(galaxy_pole());
    let core = (1.0 + dir.dot(galaxy_core())) / 2.0;
    let width = 0.11 + 0.12 * core * core;
    let band = f64::from(-(off * off) / (width * width)).exp() as f32;
    let mottle = 0.55
        + 0.45
            * fbm(
                dir.x * 5.0 + dir.z * 3.0 + 11.0,
                dir.y * 5.0 - dir.z * 2.0 + 7.0,
            );
    let lane = 1.0
        - 0.75
            * smoothstep(0.55, 0.8, fbm(dir.x * 9.0 - 3.0, dir.y * 9.0 + dir.z * 6.0))
            * f64::from(-(off * off) / (width * width * 0.12)).exp() as f32;
    band * mottle * lane * (0.35 + 0.65 * core * core)
}

/// The night's own light the way `dir` in the sky: the Milky Way, turned
/// by `turn`, and the airglow low round the horizon, added to the sky
/// colour `c` as `night` of the way into night.
pub fn night_sky(c: Rgba, dir: Vec3, turn: f32, night: f32) -> Rgba {
    if night <= 0.0 {
        return c;
    }
    let up = f64::from(dir.y).asin() as f32;
    let own = sky_frame(turn).inverse() * dir;
    // Dimmed through the dust low down.
    let clear_air = smoothstep(0.0, 0.35, up);
    let mw = milky_way(own) * clear_air * night;
    let glow = night * smoothstep(0.3, 0.0, up) * smoothstep(-0.05, 0.02, up);
    let add = |v: u8, by: f32| (f32::from(v) + by).min(255.0) as u8;
    rgb(
        add(c.r, 92.0 * mw + 26.0 * glow),
        add(c.g, 86.0 * mw + 14.0 * glow),
        add(c.b, 108.0 * mw + 12.0 * glow),
    )
}

/// A star as it's drawn this frame: the way to it, its colour with how
/// bright it shows as its alpha, and the half-length of its cross.
#[derive(Clone, Copy, Debug)]
pub struct StarDot {
    pub dir: Vec3,
    pub colour: Rgba,
    pub size: f32,
    pub bright: f32,
}

/// A dot's size: about a pixel, at their distance.
pub const STAR_PX: f32 = STARS_RADIUS * 0.00055;

/// The stars that show at `turn`, as many as it's `night` and the air's
/// clear (`clear_air`), twinkling at time `t`, within `budget`: what
/// `draw_stars` draws. Empty with no night to see them by.
pub fn visible_stars(night: f32, clear_air: f32, turn: f32, t: f32, budget: usize) -> Vec<StarDot> {
    let k = night * clear_air;
    if night < 0.01 || k < 0.01 {
        return Vec::new();
    }
    let frame = sky_frame(turn);
    let mut out = Vec::new();
    for s in stars().iter().take(budget) {
        let dir = frame * s.dir;
        if dir.y < -0.02 {
            continue;
        }
        let low = smoothstep(0.02, 0.3, dir.y);
        let twinkle = 1.0
            - (0.12 + 0.3 * (1.0 - low))
                * (0.5 + 0.5 * f64::from(t * (2.0 + 3.0 * s.phase) + s.phase * 7.0).sin() as f32);
        let a = k * (0.3 + 0.7 * s.bright) * twinkle * (0.15 + 0.85 * low);
        if a < 0.03 {
            continue;
        }
        out.push(StarDot {
            dir,
            colour: s.colour.with_alpha((255.0 * a.min(1.0)) as u8),
            size: STAR_PX * (0.7 + 1.8 * s.bright * s.bright),
            bright: s.bright,
        });
    }
    out
}
