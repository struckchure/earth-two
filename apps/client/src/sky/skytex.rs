//! The sky's surfaces: textures painted here, for a sphere, from 3D noise
//! on the sphere, so they don't seam where they wrap round. Nobody knows
//! what TRAPPIST-1's planets look like; these are guesses that go with what
//! they're thought to be (sun.rs).
//!
//! Go painted through raylib's sphere's own texture coordinates, which
//! don't wrap the usual way; Bevy's dome and bodies are our own sphere
//! mesh (render.rs) with a plain longitude-latitude wrap, so each pixel is
//! painted for the direction it faces (`pixel_dir`). What's painted at a
//! direction is the same.

use super::{
    colour::{Rgba, brighten, mix_colour, rgb},
    stars::night_sky,
    sun::{DUSK_GLARE, GLARE_COLOUR, glare, sky_colour},
};
use bevy::math::Vec3;
use earth_two_world::terrain::{lattice, smoothstep};
use std::f64::consts::PI;

/// Texture sizes.
pub const SKY_TEX_W: usize = 512;
pub const SKY_TEX_H: usize = 256;

/// How a body looks at a point on its unit sphere.
pub type Surface = fn(Vec3) -> Rgba;

/// The direction a pixel of a `w` by `h` longitude-latitude texture faces:
/// longitude from +X through +Z along a row, latitude from the top (+Y)
/// down.
pub fn pixel_dir(x: usize, y: usize, w: usize, h: usize) -> Vec3 {
    let (u, v) = ((x as f64 + 0.5) / w as f64, (y as f64 + 0.5) / h as f64);
    let (az, el) = (2.0 * PI * u, PI * (0.5 - v));
    Vec3::new(
        (el.cos() * az.cos()) as f32,
        el.sin() as f32,
        (el.cos() * az.sin()) as f32,
    )
}

/// Paints what `f` says each direction looks like into a `w` by `h` RGBA
/// texture.
pub fn paint_sphere(w: usize, h: usize, f: impl Fn(Vec3) -> Rgba) -> Vec<u8> {
    let mut pixels = vec![0u8; w * h * 4];
    for y in 0..h {
        for x in 0..w {
            let c = f(pixel_dir(x, y, w, h));
            let at = (y * w + x) * 4;
            pixels[at..at + 4].copy_from_slice(&[c.r, c.g, c.b, c.a]);
        }
    }
    pixels
}

/// The dome's pixels, with the sun the way `sun`, `dusk` of the way into
/// dusk's colours, `night` into night's and the stars' sky turned by
/// `turn`: the sky's colour by elevation, the sun's glare round where it
/// hangs, and by night the Milky Way and the airglow (stars.rs). Pure, so
/// it can be painted off the main thread.
pub fn paint_sky(sun: Vec3, dusk: f32, night: f32, turn: f32) -> Vec<u8> {
    paint_sphere(SKY_TEX_W, SKY_TEX_H, |dir| {
        sky_at(dir, sun, dusk, night, turn)
    })
}

/// The sky as `paint_sky` paints it, the way `dir`.
pub fn sky_at(dir: Vec3, sun: Vec3, dusk: f32, night: f32, turn: f32) -> Rgba {
    let g = mix_colour(GLARE_COLOUR, DUSK_GLARE, dusk);
    // The glare fades as the sun goes down, but for a glow round where it
    // set.
    let fade = 1.0 - 0.85 * night;
    let up = f64::from(dir.y).asin();
    let off = f64::from(dir.dot(sun)).clamp(-1.0, 1.0).acos();
    night_sky(
        mix_colour(sky_colour(up, dusk, night), g, glare(off) * fade),
        dir,
        turn,
        night,
    )
}

/// A neighbour's surface, by its letter.
pub fn surface(name: &str) -> Option<Surface> {
    Some(match name {
        "b" => surface_b,
        "c" => surface_c,
        "d" => surface_d,
        "f" => surface_f,
        "g" => surface_g,
        "h" => surface_h,
        _ => return None,
    })
}

// b: too hot for water, under thick cloud: Venus-like, banded and swirled.
fn surface_b(p: Vec3) -> Rgba {
    let warp = fbm3(p * 3.0, 11);
    let band = 0.5 + 0.5 * (f64::from(p.y + 0.35 * warp) * 13.0).sin();
    let c = mix_colour(rgb(206, 156, 124), rgb(240, 220, 178), band as f32);
    brighten(c, 0.85 + 0.3 * fbm3(p * 9.0, 12))
}

// c: bare rock, tan with dark plains and pocked with craters.
fn surface_c(p: Vec3) -> Rgba {
    let mut c = rgb(178, 142, 106);
    let plains = smoothstep(0.5, 0.62, fbm3(p * 2.5, 21));
    c = mix_colour(c, rgb(104, 82, 66), plains * 0.85);
    let pits = smoothstep(0.66, 0.72, fbm3(p * 14.0, 22));
    brighten(c, (1.0 - 0.35 * pits) * (0.85 + 0.3 * fbm3(p * 6.0, 23)))
}

// d: maybe an ocean world: deep water under swirls of cloud.
fn surface_d(p: Vec3) -> Rgba {
    let sea = mix_colour(rgb(22, 58, 112), rgb(44, 104, 160), fbm3(p * 3.0, 31));
    let warp = p * 4.0 + Vec3::new(2.0 * fbm3(p * 2.0, 32), 0.0, 0.0);
    let cloud = smoothstep(0.52, 0.66, fbm3(warp, 33));
    mix_colour(sea, rgb(244, 246, 250), cloud * 0.9)
}

// f: past the snow line: water, ice caps reaching down from the poles,
// thin streaks of cloud.
fn surface_f(p: Vec3) -> Rgba {
    let sea = mix_colour(rgb(30, 96, 120), rgb(54, 136, 150), fbm3(p * 3.0, 41));
    let edge = 0.74 + 0.16 * (fbm3(p * 5.0, 42) - 0.5);
    let ice = smoothstep(edge - 0.04, edge + 0.04, p.y.abs());
    let c = mix_colour(sea, rgb(226, 238, 244), ice);
    let streak = smoothstep(
        0.66,
        0.74,
        fbm3(Vec3::new(p.x * 2.0, p.y * 10.0, p.z * 2.0), 43),
    );
    mix_colour(c, rgb(240, 244, 248), streak * 0.5)
}

// g: all ice, cracked across.
fn surface_g(p: Vec3) -> Rgba {
    let c = mix_colour(rgb(176, 196, 220), rgb(222, 232, 242), fbm3(p * 3.0, 51));
    let ridge = 1.0 - (2.0 * fbm3(p * 5.0, 52) - 1.0).abs();
    let crack = smoothstep(0.9, 0.97, ridge);
    mix_colour(c, rgb(104, 122, 160), crack * 0.8)
}

// h: the outermost: grey frost with darker patches.
fn surface_h(p: Vec3) -> Rgba {
    let patches = smoothstep(0.48, 0.62, fbm3(p * 3.5, 61));
    let c = mix_colour(rgb(204, 206, 212), rgb(132, 130, 140), patches * 0.7);
    brighten(c, 0.9 + 0.2 * fbm3(p * 10.0, 62))
}

/// The sun's face, for a sphere turned with its top pole (+Y) to the
/// camera: from the middle of the disc out to its edge it darkens and
/// reddens, as a star's does, and it's grained all over with a few dark
/// spots.
pub fn sun_surface(p: Vec3) -> Rgba {
    // How far out from the middle of the disc: 0 at the pole facing the
    // camera, 1 at the edge.
    let mut out = f64::from((1.0 - p.y * p.y).max(0.0)).sqrt() as f32;
    if p.y < 0.0 {
        out = 1.0;
    }
    let mu = f64::from((1.0 - out * out).max(0.0)).sqrt() as f32;
    let face = f64::from(mu).powf(0.6) as f32;
    // A red dwarf: orange at the middle, deep red at the edge.
    let c = mix_colour(rgb(168, 38, 14), rgb(255, 168, 84), face);
    let grain = 0.9 + 0.2 * fbm3(p * 40.0, 71);
    let spots = smoothstep(0.73, 0.78, fbm3(p * 4.0, 72));
    brighten(c, grain * (1.0 - 0.45 * spots))
}

/// Fractal value noise at `p`: four octaves, 0 to 1.
pub fn fbm3(mut p: Vec3, seed: u32) -> f32 {
    let (mut sum, mut amp, mut total) = (0.0f32, 1.0f32, 0.0f32);
    for i in 0..4u32 {
        sum += amp * value_noise3(p, seed.wrapping_mul(7).wrapping_add(i));
        total += amp;
        p *= 2.03;
        amp *= 0.5;
    }
    sum / total
}

/// Smooth noise in 3D, 0 to 1.
pub fn value_noise3(p: Vec3, seed: u32) -> f32 {
    let (x0, y0, z0) = (
        f64::from(p.x).floor(),
        f64::from(p.y).floor(),
        f64::from(p.z).floor(),
    );
    let (fx, fy, fz) = (p.x - x0 as f32, p.y - y0 as f32, p.z - z0 as f32);
    let (sx, sy, sz) = (
        fx * fx * (3.0 - 2.0 * fx),
        fy * fy * (3.0 - 2.0 * fy),
        fz * fz * (3.0 - 2.0 * fz),
    );
    let at = |i: f64, j: f64, k: f64| {
        lattice(
            (i as i32).wrapping_add((k as i32).wrapping_mul(7919)),
            (j as i32).wrapping_sub((k as i32).wrapping_mul(104729)),
            seed,
        )
    };
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let bottom = lerp(
        lerp(at(x0, y0, z0), at(x0 + 1.0, y0, z0), sx),
        lerp(at(x0, y0 + 1.0, z0), at(x0 + 1.0, y0 + 1.0, z0), sx),
        sy,
    );
    let top = lerp(
        lerp(at(x0, y0, z0 + 1.0), at(x0 + 1.0, y0, z0 + 1.0), sx),
        lerp(
            at(x0, y0 + 1.0, z0 + 1.0),
            at(x0 + 1.0, y0 + 1.0, z0 + 1.0),
            sx,
        ),
        sy,
    );
    lerp(bottom, top, sz)
}
