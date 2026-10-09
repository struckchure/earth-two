//! Ground maths the rest of the subsystem shares: raylib's rectangle on the
//! XZ plane, and the level-ground distances `earth_two_world::terrain`
//! keeps to itself (`levelDistance`, `rectDistance`, the `LEVEL` rects
//! and `domeWalls` in `game/terrain.go`), mirrored here with the same
//! numbers so the scatter and the surface can ask them.

use bevy::math::{Quat, Vec2, Vec3};
use earth_two_world::{kit::Collider, terrain::road_distance};

/// A rectangle by its corner and size, as raylib's: X across, Y for the
/// game's Z when it's on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rectangle {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rectangle {
    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Rectangle {
        Rectangle {
            x,
            y,
            width,
            height,
        }
    }

    /// Whether p is in it, edges included.
    pub fn contains(&self, p: Vec2) -> bool {
        p.x >= self.x && p.x <= self.x + self.width && p.y >= self.y && p.y <= self.y + self.height
    }

    /// Whether the two overlap, as `rl.CheckCollisionRecs`.
    pub fn overlaps(&self, o: &Rectangle) -> bool {
        self.x < o.x + o.width
            && self.x + self.width > o.x
            && self.y < o.y + o.height
            && self.y + self.height > o.y
    }
}

/// Where the terrain is flat, from tools/world/landfall.py: under the
/// seats (`LEVEL` in `earth_two_world::terrain`).
const LEVEL: [Rectangle; 9] = [
    Rectangle::new(-64.0, -66.0, 108.0, 120.0),
    Rectangle::new(-7.0, 44.0, 14.0, 52.0),
    Rectangle::new(9764.0, -1850.0, 60.0, 90.0),
    Rectangle::new(-1652.0, 12004.0, 54.0, 40.0),
    Rectangle::new(-1402.0, 11998.0, 56.0, 54.0),
    Rectangle::new(-1476.0, 12000.0, 32.0, 30.0),
    Rectangle::new(-1607.0, 11842.0, 14.0, 146.0),
    Rectangle::new(-1656.0, 11993.0, 312.0, 14.0),
    Rectangle::new(-6528.0, -6528.0, 56.0, 56.0),
];

/// The dome's walls (DOME_* in landfall.py).
pub const DOME_WALLS: Rectangle = Rectangle::new(-54.0, -56.0, 88.0, 100.0);

/// How far (x, z) is from r: 0 inside it.
pub fn rect_distance(r: &Rectangle, x: f32, z: f32) -> f32 {
    let dx = (r.x - x).max(0.0).max(x - (r.x + r.width));
    let dz = (r.y - z).max(0.0).max(z - (r.y + r.height));
    hypot(dx, dz)
}

/// How far (x, z) is from the nearest level ground: 0 on it.
pub fn level_distance(x: f32, z: f32) -> f32 {
    let mut d = f32::INFINITY;
    for r in &LEVEL {
        d = d.min(rect_distance(r, x, z));
    }
    let (rd, _) = road_distance(x, z);
    if rd < d {
        d = rd.max(0.0);
    }
    d
}

/// Go's `float32(math.Hypot(float64(a), float64(b)))`.
pub fn hypot(a: f32, b: f32) -> f32 {
    (a as f64).hypot(b as f64) as f32
}

/// Go's `float32(math.Abs(float64(v)))`.
pub fn abs(v: f32) -> f32 {
    v.abs()
}

/// `footprint`: the rectangle on the ground a collider of a piece at `at`,
/// turned by `turn`, covers, and how high it reaches.
pub fn footprint(c: &Collider, at: Vec3, turn: Quat) -> (Rectangle, f32) {
    let (center, rot) = c.in_frame(at, turn);
    let mut lo = Vec2::INFINITY;
    let mut hi = Vec2::NEG_INFINITY;
    let mut top = f32::NEG_INFINITY;
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                let corner = Vec3::new(
                    sx * c.size[0] / 2.0,
                    sy * c.size[1] / 2.0,
                    sz * c.size[2] / 2.0,
                );
                let p = center + rot * corner;
                lo = Vec2::new(lo.x.min(p.x), lo.y.min(p.z));
                hi = Vec2::new(hi.x.max(p.x), hi.y.max(p.z));
                top = top.max(p.y);
            }
        }
    }
    (Rectangle::new(lo.x, lo.y, hi.x - lo.x, hi.y - lo.y), top)
}

/// `game.mixColour`: each channel moved `t` of the way, truncated; opaque.
pub fn mix_colour(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2]), 255]
}
