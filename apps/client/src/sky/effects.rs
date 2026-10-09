//! The effects: dust kicked up by feet, landings, rolls and slides and
//! wheels, and sparks off metal hit hard. Each is a particle, a soft disc
//! facing the camera, drawn after the world (so behind what's in front of
//! it) far to near, so they blend over each other. They're all one mesh,
//! rebuilt each frame (render.rs).

use super::{
    colour::{Rgba, clamp01, rgb},
    time::UnixTime,
    weather::{Weather, dust_wind},
};
use bevy::prelude::*;
use earth_two_world::terrain::smoothstep;
use std::f64::consts::PI;

/// The most there can be at once; past it, new ones take the place of the
/// oldest.
pub const MAX_PARTICLES: usize = 2048;

/// Dust closer to the camera than NEAR_FADE isn't drawn, and is drawn in
/// full only from NEAR_CLEAR on.
pub const NEAR_FADE: f32 = 1.5;
pub const NEAR_CLEAR: f32 = 6.0;

/// Dust drifts on the wind: a breath of it on a clear day, the storm's in
/// a storm.
pub const CALM_WIND: f32 = 0.4;
pub const STORM_WIND: f32 = 6.0;

/// The dust's colour in white light: the Red's soil, paler as it's thrown
/// up into the air.
pub const DUST_COLOUR: Rgba = rgb(184, 128, 98);

/// One mote of dust or spark.
#[derive(Clone, Copy, Debug, Default)]
pub struct Particle {
    pub pos: Vec3,
    pub vel: Vec3,
    pub age: f32,
    pub life: f32,
    /// The ground it rose from: it's drawn no lower than it, so it doesn't
    /// cut into the ground in a straight line.
    pub floor: f32,
    /// Its radius, starting and ending.
    pub size0: f32,
    pub size1: f32,
    pub colour: Rgba,
    /// How quickly it slows (the share 1 - e^-drag a second), how fast it
    /// accelerates down (m/s², up if less than 0), and how much the wind
    /// carries it.
    pub drag: f32,
    pub fall: f32,
    pub blown: f32,
}

/// A small fixed-sequence random source (xorshift64*), standing in for
/// Go's `math/rand`: the effects' scatter needn't match the Go client's
/// draw for draw, only its spread.
#[derive(Clone, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn seeded(seed: u64) -> Self {
        Self(seed | 1)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    /// Uniform in [0, 1).
    pub fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    pub fn f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A random vector, each part within ±k.
    pub fn jitter(&mut self, k: f32) -> Vec3 {
        let mut r = || (2.0 * self.f32() - 1.0) * k;
        Vec3::new(r(), r(), r())
    }
}

/// A particle to draw, and how far it is ahead of the camera.
#[derive(Clone, Copy, Debug)]
struct DrawOrder {
    i: usize,
    depth: f32,
}

/// A resource: the particles, the wind they drift on (m/s), and how many
/// quads the mesh had last frame.
#[derive(Resource, Debug)]
pub struct Effects {
    pub ps: Vec<Particle>,
    pub wind: Vec3,
    pub drawn: usize,
    rng: Rng,
    order: Vec<DrawOrder>,
}

impl Default for Effects {
    fn default() -> Self {
        Self::seeded(UnixTime::now().0 as u64)
    }
}

/// `c` in light: the light's colour over it, partly, as dust in the air is
/// lit from all round (it has to read a little paler than the sand it's
/// raised from, not glow).
pub fn lit(c: Rgba, light: Rgba, alpha: f32) -> Rgba {
    let f = |a: u8, b: u8| (f32::from(a) * (0.55 + 0.45 * f32::from(b) / 255.0)).min(255.0) as u8;
    Rgba {
        r: f(c.r, light.r),
        g: f(c.g, light.g),
        b: f(c.b, light.b),
        a: (255.0 * clamp01(alpha)) as u8,
    }
}

impl Effects {
    /// Effects whose scatter repeats, for tests.
    pub fn seeded(seed: u64) -> Self {
        Self {
            ps: Vec::with_capacity(MAX_PARTICLES),
            wind: Vec3::ZERO,
            drawn: 0,
            rng: Rng::seeded(seed),
            order: Vec::new(),
        }
    }

    /// Adds `p`, in place of the oldest if there are already MAX_PARTICLES.
    pub fn add(&mut self, mut p: Particle) {
        if p.floor == 0.0 {
            p.floor = p.pos.y - 0.05;
        }
        if self.ps.len() < MAX_PARTICLES {
            self.ps.push(p);
            return;
        }
        let mut oldest = 0;
        for i in 0..self.ps.len() {
            if self.ps[i].age / self.ps[i].life > self.ps[oldest].age / self.ps[oldest].life {
                oldest = i;
            }
        }
        self.ps[oldest] = p;
    }

    /// A footstep's dust, at a foot moving at `vel` (`speed` on the ground).
    pub fn puff(&mut self, at: Vec3, vel: Vec3, speed: f32, light: Rgba) {
        let n = 2 + (speed / 2.0) as usize;
        for _ in 0..n {
            let mut v = vel * 0.15 + self.rng.jitter(0.35);
            v.y = 0.2 + 0.4 * self.rng.f32();
            let p = Particle {
                pos: at + self.rng.jitter(0.08),
                vel: v,
                life: 0.9 + 0.6 * self.rng.f32(),
                size0: 0.1,
                size1: 0.45 + 0.25 * clamp01(speed / 4.0),
                colour: lit(DUST_COLOUR, light, 0.4),
                drag: 2.5,
                fall: -0.05,
                blown: 0.6,
                ..default()
            };
            self.add(p);
        }
    }

    /// The dust thrown out round something landing hard at `at`: `n`
    /// motes, as far and as big as `strength`.
    pub fn ring(&mut self, at: Vec3, n: usize, strength: f32, light: Rgba) {
        for i in 0..n {
            let a = 2.0 * PI * (i as f64 + self.rng.f64()) / n as f64;
            let out = Vec3::new(a.cos() as f32, 0.0, a.sin() as f32);
            let mut v = out * ((1.0 + self.rng.f32()) * 1.4 * strength);
            v.y = 0.3 + 0.5 * self.rng.f32() * strength;
            let p = Particle {
                pos: at + Vec3::new(0.0, 0.05, 0.0),
                vel: v,
                life: 1.1 + 0.8 * self.rng.f32(),
                size0: 0.15,
                size1: 0.6 + 0.5 * strength,
                colour: lit(DUST_COLOUR, light, 0.45),
                drag: 2.2,
                fall: -0.05,
                blown: 0.7,
                ..default()
            };
            self.add(p);
        }
    }

    /// The dust a slide or a roll leaves, over `dt`.
    pub fn trail(&mut self, at: Vec3, vel: Vec3, dt: f32, light: Rgba) {
        let n = (28.0 * dt + self.rng.f32()) as usize;
        for _ in 0..n {
            let mut v = vel * -0.1 + self.rng.jitter(0.4);
            v.y = 0.2 + 0.5 * self.rng.f32();
            let p = Particle {
                pos: at + self.rng.jitter(0.15),
                vel: v,
                life: 1.0 + 0.6 * self.rng.f32(),
                size0: 0.15,
                size1: 0.7,
                colour: lit(DUST_COLOUR, light, 0.4),
                drag: 2.0,
                fall: -0.05,
                blown: 0.8,
                ..default()
            };
            self.add(p);
        }
    }

    /// One puff of the dust a wheel throws up, going `vel`, as hard as `k`
    /// (0 to 1).
    pub fn wheel(&mut self, at: Vec3, vel: Vec3, k: f32, light: Rgba) {
        let mut v = vel * -0.08 + self.rng.jitter(0.8);
        v.y = 0.4 + 1.2 * self.rng.f32() * k;
        let p = Particle {
            pos: at + self.rng.jitter(0.2),
            vel: v,
            life: 1.4 + 1.2 * self.rng.f32() * k,
            size0: 0.25,
            size1: 0.8 + 1.8 * k,
            colour: lit(DUST_COLOUR, light, 0.18 + 0.27 * k),
            drag: 1.6,
            fall: -0.08,
            blown: 1.0,
            ..default()
        };
        self.add(p);
    }

    /// `n` sparks off metal hit at `at`, thrown back out along `normal`.
    pub fn sparks(&mut self, at: Vec3, normal: Vec3, n: usize) {
        let out = -normal.normalize_or_zero();
        for _ in 0..n {
            let mut v = out * (2.0 + 4.0 * self.rng.f32()) + self.rng.jitter(3.0);
            v.y += 1.5 * self.rng.f32();
            let p = Particle {
                pos: at,
                vel: v,
                life: 0.25 + 0.35 * self.rng.f32(),
                size0: 0.05,
                size1: 0.015,
                colour: rgb(255, 214, 140),
                drag: 0.6,
                fall: 9.8,
                blown: 0.0,
                ..default()
            };
            self.add(p);
        }
    }

    /// Ages and moves the particles over `dt`, and drops those past their
    /// life.
    pub fn step(&mut self, dt: f32) {
        let wind = self.wind;
        self.ps.retain_mut(|p| {
            p.age += dt;
            if p.age >= p.life {
                return false;
            }
            let slow = f64::from(-p.drag * dt).exp() as f32;
            p.vel *= slow;
            // The wind carries it toward its own speed.
            p.vel += (wind - p.vel) * (p.blown * (1.0 - slow));
            p.vel.y -= p.fall * dt;
            p.pos += p.vel * dt;
            true
        });
    }

    /// Writes the particles as quads, far to near, into `verts` (four
    /// corners each) and `cols` (their colour, with how much shows as its
    /// alpha), and returns how many; the quads past them, left from a
    /// frame with more, are collapsed to nothing.
    pub fn quads(
        &mut self,
        eye: Vec3,
        ahead: Vec3,
        right: Vec3,
        up: Vec3,
        verts: &mut [[f32; 3]],
        cols: &mut [Rgba],
    ) -> usize {
        self.order.clear();
        for (i, p) in self.ps.iter().enumerate() {
            let d = (p.pos - eye).dot(ahead);
            if d < 0.2 {
                continue; // behind the camera, or in its face
            }
            self.order.push(DrawOrder { i, depth: d });
        }
        self.order.sort_by(|a, b| b.depth.total_cmp(&a.depth));
        for (q, o) in self.order.iter().enumerate() {
            let mut p = self.ps[o.i];
            let t = p.age / p.life;
            let s = p.size0 + (p.size1 - p.size0) * t;
            // Held up off the ground by its size: a mote down in it would
            // be cut off flat.
            p.pos.y = p.pos.y.max(p.floor + s * up.y.abs());
            let (r, u) = (right * s, up * s);
            let corners = [p.pos - r - u, p.pos + r - u, p.pos + r + u, p.pos - r + u];
            for (c, at) in corners.iter().enumerate() {
                verts[4 * q + c] = at.to_array();
            }
            // In quickly, out slowly.
            let mut fade = smoothstep(0.0, 0.12, t) * (1.0 - smoothstep(0.45, 1.0, t));
            // Thinned out close to the camera, so following a vehicle
            // through its own dust doesn't fill the view with it.
            fade *= smoothstep(NEAR_FADE, NEAR_CLEAR, o.depth);
            let col = p.colour.with_alpha((f32::from(p.colour.a) * fade) as u8);
            for c in 0..4 {
                cols[4 * q + c] = col;
            }
        }
        let n = self.order.len();
        for q in n..self.drawn.min(MAX_PARTICLES) {
            for c in 0..4 {
                verts[4 * q + c] = [0.0; 3];
            }
        }
        self.drawn = n;
        n
    }
}

/// Ages and moves the particles on the wind, as the weather has it.
pub fn move_effects(mut fx: ResMut<Effects>, w: Res<Weather>, time: Res<Time>) {
    fx.wind = dust_wind().normalize() * (CALM_WIND + (STORM_WIND - CALM_WIND) * w.storm);
    fx.step(time.delta_secs());
}
