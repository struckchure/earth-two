//! The terrain: the Red, [`WORLD_SIZE`] across, centred on Landfall, with
//! the other seats 8 to 15 km out across it (docs/settlement.md). It's
//! level under each seat and along the roads between them, and rolls into
//! dunes and ridges everywhere else, into canyons round the Quiet Book's
//! haven, and up into mountains at the world's edge. [`ground_height`] says
//! how high it is anywhere; everything else (what's drawn, what's walked
//! on, where the layout's pieces stand) comes from that.
//!
//! This is `game/terrain.go`'s height model, kept to the same constants,
//! seeds, hash and octaves so the two clients agree on the land. Float
//! operations follow the Go code's single-precision order; where Go goes
//! through `float64` (hypot, pow, floor) so does this.

use bevy_math::{Rect, Vec2, Vec3};

/// Metres across, centred on Landfall.
pub const WORLD_SIZE: f32 = 32768.0;
/// Detail chunks round the player: metres across, and cells a side.
pub const CHUNK_SIZE: f32 = 256.0;
pub const CHUNK_CELLS: usize = 64;
/// Tiles to the horizon, coarser.
pub const TILE_SIZE: f32 = 2048.0;
/// Chunks each way round the player that collide.
pub const COLLIDER_RADIUS: i32 = 1;
/// How far the tiles drop where chunks cover them.
pub const TILE_SINK: f32 = 8.0;
/// How far below 0 the ground's surface is: the deck's floors stand on it,
/// their tops at 0.
pub const GROUND_LEVEL: f32 = -0.05;
/// How much lower the drawn ground is under floors, so the two can't
/// flicker through each other (the collider isn't).
pub const UNDER_FLOORS: f32 = 0.5;

/// Where the player starts, and comes back to: on the Pads at the foot of
/// the drifter's ramp (`SPAWN` in tools/world/landfall.py).
pub const ARRIVAL: Vec3 = Vec3::new(9807.0, 0.0, -1770.5);

/// The seats, in the game's frame (X, and Z for Blender's -Y): where each
/// one's middle is. tools/world/landfall.py has the same (PADS_AT etc.).
pub const PADS_AT: Vec2 = Vec2::new(9800.0, -1800.0);
pub const HOLD_AT: Vec2 = Vec2::new(-1500.0, 12000.0);
pub const HAVEN_AT: Vec2 = Vec2::new(-6500.0, -6500.0);

/// A rectangle on the XZ plane by its corner and size, as raylib's.
const fn rect(x: f32, z: f32, width: f32, height: f32) -> Rect {
    Rect {
        min: Vec2::new(x, z),
        max: Vec2::new(x + width, z + height),
    }
}

/// Where the terrain is flat, from tools/world/landfall.py: under the seats.
const LEVEL: [Rect; 9] = [
    rect(-64.0, -66.0, 108.0, 120.0), // Landfall's dome, and a margin round it
    rect(-7.0, 44.0, 14.0, 52.0),     // the road south from its gate
    rect(9764.0, -1850.0, 60.0, 90.0), // the Pads
    // The Fringers' hold: its farms, salvage fields, camp, wind farm and road.
    rect(-1652.0, 12004.0, 54.0, 40.0),
    rect(-1402.0, 11998.0, 56.0, 54.0),
    rect(-1476.0, 12000.0, 32.0, 30.0),
    rect(-1607.0, 11842.0, 14.0, 146.0),
    rect(-1656.0, 11993.0, 312.0, 14.0),
    rect(-6528.0, -6528.0, 56.0, 56.0), // the haven's pocket
];

/// Where floors cover the ground: the Hull's deck at Landfall, and the
/// Pads' tiles out at the spaceport (HULL_* and PADS_* in landfall.py).
pub const FLOORED: [Rect; 2] = [
    rect(-40.0, -12.0, 48.0, 32.0),
    rect(9776.0, -1830.0, 44.0, 64.0),
];

/// A road: its points, how far either side of its middle it's level, and
/// whether it's painted (the wash to the haven isn't a road, only a way
/// through). tools/world/landfall.py has the same (ROADS).
pub struct Road {
    pub name: &'static str,
    pub points: &'static [Vec2],
    pub half: f32,
    pub paint: bool,
}

pub const ROADS: [Road; 4] = [
    Road {
        name: "caravan road",
        points: &[
            Vec2::new(0.0, 44.0),
            Vec2::new(0.0, 90.0),
            Vec2::new(2200.0, 260.0),
            Vec2::new(5600.0, -500.0),
            Vec2::new(8300.0, -1500.0),
            Vec2::new(9772.0, -1772.0),
        ],
        half: 5.0,
        paint: true,
    },
    Road {
        name: "Fringe track",
        points: &[
            Vec2::new(0.0, 90.0),
            Vec2::new(-400.0, 3200.0),
            Vec2::new(-1300.0, 7600.0),
            Vec2::new(-1300.0, 11000.0),
            Vec2::new(-1500.0, 12000.0),
        ],
        half: 3.0,
        paint: true,
    },
    Road {
        name: "hold road",
        points: &[Vec2::new(-1650.0, 12000.0), Vec2::new(-1350.0, 12000.0)],
        half: 4.0,
        paint: true,
    },
    Road {
        name: "haven wash",
        points: &[
            Vec2::new(-6500.0, -6500.0),
            Vec2::new(-6000.0, -6200.0),
            Vec2::new(-5400.0, -5600.0),
        ],
        half: 4.0,
        paint: false,
    },
];

/// How far (x, z) is from the nearest road, past its half width (0 or less
/// on it), and whether that road is painted.
pub fn road_distance(x: f32, z: f32) -> (f32, bool) {
    let mut best = f32::INFINITY;
    let mut painted = false;
    for road in &ROADS {
        for pair in road.points.windows(2) {
            let d = segment_distance(pair[0], pair[1], x, z) - road.half;
            if d < best {
                best = d;
                painted = road.paint;
            }
        }
    }
    (best, painted)
}

/// How far (x, z) is from the segment a to b.
fn segment_distance(a: Vec2, b: Vec2, x: f32, z: f32) -> f32 {
    let (dx, dz) = (b.x - a.x, b.y - a.y);
    let t = clamp01(((x - a.x) * dx + (z - a.y) * dz) / (dx * dx + dz * dz));
    hypot(x - a.x - t * dx, z - a.y - t * dz)
}

/// How far (x, z) is from the nearest level ground: 0 on it.
fn level_distance(x: f32, z: f32) -> f32 {
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

/// How high the ground is at (x, z), above [`GROUND_LEVEL`].
pub fn ground_height(x: f32, z: f32) -> f32 {
    levelled(x, z) + drift(x, z) + berm(x, z)
}

/// The land, levelled under the seats and along the roads.
fn levelled(x: f32, z: f32) -> f32 {
    let mut d = level_distance(x, z);
    if d == 0.0 {
        return 0.0;
    }
    // Level near the places above, the full terrain from 30 m or so off
    // them: their edges wander, so the level ground doesn't end in straight
    // embankments.
    d += (fbm(x / 28.0 + 3.1, z / 28.0 - 5.7) - 0.5) * 18.0;
    let full = smoothstep(3.0, 30.0, d);
    if full == 0.0 {
        return 0.0;
    }
    full * relief(x, z).max(-2.0)
}

/// The dome's walls (DOME_* in landfall.py), and how high and how far out
/// the sand banks against them.
const DOME_WALLS: Rect = rect(-54.0, -56.0, 88.0, 100.0);
pub const DRIFT_HIGH: f32 = 1.4;
const DRIFT_REACH: f32 = 10.0;

/// The sand blown up against the outside of the dome's walls: highest at
/// the wall, thinning out over `DRIFT_REACH`, more in some places than
/// others, and none across the road through the gate.
pub fn drift(x: f32, z: f32) -> f32 {
    let d = rect_distance(&DOME_WALLS, x, z);
    if d <= 0.2 || d >= DRIFT_REACH {
        return 0.0;
    }
    let (rd, _) = road_distance(x, z);
    let k = 1.0 - smoothstep(0.2, DRIFT_REACH, d);
    DRIFT_HIGH
        * k
        * (k as f64).sqrt() as f32
        * (0.4 + 1.2 * fbm(x / 9.0 + 31.0, z / 9.0 - 13.0))
        * smoothstep(1.0, 5.0, rd)
}

/// The Fringers' hold sits in a bowl in the dunes: berms `BERM_HIGH` high
/// round its fields and camp, `BERM_IN` to `BERM_OUT` metres out from them,
/// but where its roads come in.
const HOLD_GROUND: Rect = rect(-1656.0, 11842.0, 312.0, 210.0);
const BERM_HIGH: f32 = 6.0;
const BERM_IN: f32 = 10.0;
const BERM_OUT: f32 = 75.0;

/// The bank of the hold's bowl at (x, z).
fn berm(x: f32, z: f32) -> f32 {
    let d = rect_distance(&HOLD_GROUND, x, z);
    if d <= BERM_IN || d >= BERM_OUT {
        return 0.0;
    }
    let (rd, _) = road_distance(x, z);
    let mid = (BERM_IN + BERM_OUT) / 2.0;
    let k = smoothstep(BERM_IN, mid - 10.0, d) * smoothstep(BERM_OUT, mid + 5.0, d);
    BERM_HIGH * k * (0.6 + 0.8 * fbm(x / 40.0 + 5.0, z / 40.0 + 9.0)) * smoothstep(4.0, 22.0, rd)
}

/// The land before it's levelled: dunes, long dunes, ridges, canyons round
/// the haven, and mountains at the world's edge.
fn relief(x: f32, z: f32) -> f32 {
    let dunes = (fbm(x / 45.0, z / 45.0) - 0.5) * 9.0;
    let long = (fbm(x / 420.0 + 11.0, z / 260.0 - 4.0) - 0.5) * 26.0;
    let ridges = 3.5
        * powf(
            1.0 - (2.0 * fbm(x / 110.0 + 7.3, z / 160.0 + 1.9) - 1.0).abs(),
            2.0,
        );
    let canyons = canyon(x, z)
        * 40.0
        * powf(
            1.0 - (2.0 * fbm(x / 170.0 - 2.2, z / 170.0 + 8.1) - 1.0).abs(),
            1.5,
        );
    let rim = 140.0 * smoothstep(13500.0, 16200.0, x.abs().max(z.abs()));
    dunes + long + ridges + canyons + rim
}

/// How much (x, z) is in the canyon country round the haven: fully within
/// 900 m of it, not at all past 1.7 km.
pub fn canyon(x: f32, z: f32) -> f32 {
    smoothstep(1700.0, 900.0, hypot(x - HAVEN_AT.x, z - HAVEN_AT.y))
}

/// How high the ground is drawn at (x, z): [`ground_height`], but lower
/// under the floors that cover it.
pub fn drawn_height(x: f32, z: f32) -> f32 {
    let h = ground_height(x, z);
    for f in &FLOORED {
        if x > f.min.x && x < f.max.x && z > f.min.y && z < f.max.y {
            return h - UNDER_FLOORS;
        }
    }
    h
}

/// How high what's walked on is at (x, z), above [`GROUND_LEVEL`]: the
/// ground, but up at the floors' tops (0) where floors cover it. The floors
/// have no colliders of their own, and without this feet sink through them
/// to the ground under.
pub fn walk_height(x: f32, z: f32) -> f32 {
    let h = ground_height(x, z);
    for f in &FLOORED {
        if x >= f.min.x && x <= f.max.x && z >= f.min.y && z <= f.max.y {
            return h - GROUND_LEVEL;
        }
    }
    h
}

/// The ground's paint at (x, z), 8-bit sRGB as the Go client mixes it: red
/// soil, lighter and darker in drifts; packed dirt on the roads; darker rock
/// in the canyons.
pub fn ground_colour(x: f32, z: f32) -> [u8; 3] {
    // Dusty red, not orange: the low sun warms it enough.
    let mut soil = mix_colour([134, 76, 56], [174, 108, 80], fbm(x / 34.0, z / 34.0));
    soil = mix_colour(
        soil,
        [194, 138, 102],
        0.35 * smoothstep(0.55, 0.8, fbm(x / 9.0 + 5.0, z / 90.0)),
    );
    let c = canyon(x, z);
    if c > 0.0 {
        soil = mix_colour(
            soil,
            [106, 60, 48],
            c * smoothstep(0.45, 0.7, fbm(x / 60.0 - 9.0, z / 60.0 + 3.0)),
        );
    }
    // Where it's steep, in the canyons and the mountains, the rock shows
    // through in its beds: bands by height, wandering a little.
    if c > 0.0 || x.abs().max(z.abs()) > 13000.0 {
        let hx = ground_height(x + 2.0, z) - ground_height(x - 2.0, z);
        let hz = ground_height(x, z + 2.0) - ground_height(x, z - 2.0);
        let steep = smoothstep(0.35, 0.75, hypot(hx, hz) / 4.0);
        if steep > 0.0 {
            let h = ground_height(x, z) + 4.0 * fbm(x / 60.0, z / 60.0);
            let bed = smoothstep(-0.3, 0.3, ((h * 0.75) as f64).sin() as f32);
            let rock = mix_colour([98, 50, 40], [176, 108, 76], bed);
            soil = mix_colour(soil, rock, steep);
        }
    }
    // The sand banked against the dome, paler.
    let dr = drift(x, z);
    if dr > 0.0 {
        soil = mix_colour(soil, [200, 146, 108], (dr / DRIFT_HIGH * 1.4).min(1.0));
    }
    // The roads: the same red packed darker, their verges ragged where the
    // sand's blown over them.
    let (mut d, painted) = road_distance(x, z);
    if painted && d < 4.0 {
        d += 2.4 * (fbm(x / 4.0 + 11.0, z / 4.0 - 7.0) - 0.5);
        return mix_colour(soil, [104, 70, 60], 0.85 * smoothstep(2.5, -1.0, d));
    }
    soil
}

/// `game.mixColour`: each channel moved `t` of the way, truncated.
fn mix_colour(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let m = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t) as u8;
    [m(a[0], b[0]), m(a[1], b[1]), m(a[2], b[2])]
}

/// How far (x, z) is from r: 0 inside it.
fn rect_distance(r: &Rect, x: f32, z: f32) -> f32 {
    let dx = (r.min.x - x).max(0.0).max(x - r.max.x);
    let dz = (r.min.y - z).max(0.0).max(z - r.max.y);
    hypot(dx, dz)
}

pub fn smoothstep(a: f32, b: f32, v: f32) -> f32 {
    let t = clamp01((v - a) / (b - a));
    t * t * (3.0 - 2.0 * t)
}

fn clamp01(v: f32) -> f32 {
    v.clamp(0.0, 1.0)
}

/// Go's `float32(math.Hypot(float64(a), float64(b)))`.
fn hypot(a: f32, b: f32) -> f32 {
    (a as f64).hypot(b as f64) as f32
}

/// Go's `float32(math.Pow(float64(a), float64(b)))`.
fn powf(a: f32, b: f32) -> f32 {
    (a as f64).powf(b as f64) as f32
}

/// Fractal value noise at (x, z): four octaves, 0 to 1.
pub fn fbm(mut x: f32, mut z: f32) -> f32 {
    let (mut sum, mut amp, mut total) = (0.0f32, 1.0f32, 0.0f32);
    for seed in 0..4u32 {
        sum += amp * value_noise(x, z, seed);
        total += amp;
        x *= 2.03;
        z *= 2.03;
        amp *= 0.5;
    }
    sum / total
}

/// Smooth noise, 0 to 1, from a lattice of hashed values.
pub fn value_noise(x: f32, z: f32, seed: u32) -> f32 {
    let (x0, z0) = ((x as f64).floor() as f32, (z as f64).floor() as f32);
    let (fx, fz) = (x - x0, z - z0);
    let (sx, sz) = (fx * fx * (3.0 - 2.0 * fx), fz * fz * (3.0 - 2.0 * fz));
    let at = |i: f32, j: f32| lattice(i as i32, j as i32, seed);
    let top = at(x0, z0) + (at(x0 + 1.0, z0) - at(x0, z0)) * sx;
    let bottom = at(x0, z0 + 1.0) + (at(x0 + 1.0, z0 + 1.0) - at(x0, z0 + 1.0)) * sx;
    top + (bottom - top) * sz
}

/// A fixed pseudo-random value, 0 to 1, for a lattice point.
pub fn lattice(i: i32, j: i32, seed: u32) -> f32 {
    let mut h = (i as u32).wrapping_mul(0x27d4eb2d)
        ^ (j as u32).wrapping_mul(0x165667b1)
        ^ seed.wrapping_mul(0x9e3779b9);
    h ^= h >> 15;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    (h & 0xffffff) as f32 / 0xffffff as f32
}

/// The middle of chunk (ci, cj).
pub fn chunk_centre(ci: i32, cj: i32) -> Vec2 {
    Vec2::new(
        (ci as f32 + 0.5) * CHUNK_SIZE,
        (cj as f32 + 0.5) * CHUNK_SIZE,
    )
}

/// The chunk (x, z) is in.
pub fn chunk_of(x: f32, z: f32) -> (i32, i32) {
    (
        ((x / CHUNK_SIZE) as f64).floor() as i32,
        ((z / CHUNK_SIZE) as f64).floor() as i32,
    )
}

/// The ground's normal at (x, z), from `height`, `e` apart.
pub fn terrain_normal(height: impl Fn(f32, f32) -> f32, x: f32, z: f32, e: f32) -> Vec3 {
    let dx = height(x + e, z) - height(x - e, z);
    let dz = height(x, z + e) - height(x, z - e);
    Vec3::new(-dx, 2.0 * e, -dz).normalize()
}

/// A square of terrain `size` across, `cells` a side, round (cx, cz): its
/// vertices (from `height`, at [`GROUND_LEVEL`]) and triangles, row by row
/// along X then down Z, facing up.
pub fn grid(
    cx: f32,
    cz: f32,
    size: f32,
    cells: usize,
    height: impl Fn(f32, f32) -> f32,
) -> (Vec<Vec3>, Vec<u32>) {
    let n = cells + 1;
    let step = size / cells as f32;
    let mut vertices = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let (x, z) = (
                cx - size / 2.0 + i as f32 * step,
                cz - size / 2.0 + j as f32 * step,
            );
            vertices.push(Vec3::new(x, GROUND_LEVEL + height(x, z), z));
        }
    }
    let mut indices = Vec::with_capacity(cells * cells * 6);
    for j in 0..cells {
        for i in 0..cells {
            let (a, b) = ((j * n + i) as u32, ((j + 1) * n + i) as u32);
            indices.extend_from_slice(&[a, b, a + 1, a + 1, b, b + 1]);
        }
    }
    (vertices, indices)
}

/// A grid point's height and normal.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sample {
    pub height: f32,
    pub normal: Vec3,
}

/// Heights and normals over the grid round (cx, cz), evaluating `height`
/// once per point: one extra row on each edge supplies the normals'
/// neighbouring heights.
pub fn sample(
    cx: f32,
    cz: f32,
    size: f32,
    cells: usize,
    height: impl Fn(f32, f32) -> f32,
) -> Vec<Sample> {
    let n = cells + 1;
    let step = size / cells as f32;
    let stride = n + 2;
    let mut heights = vec![0.0f32; stride * stride];
    for j in 0..stride {
        for i in 0..stride {
            let x = cx - size / 2.0 + (i as f32 - 1.0) * step;
            let z = cz - size / 2.0 + (j as f32 - 1.0) * step;
            heights[j * stride + i] = height(x, z);
        }
    }
    let mut out = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let p = (j + 1) * stride + i + 1;
            let dx = heights[p + 1] - heights[p - 1];
            let dz = heights[p + stride] - heights[p - stride];
            out.push(Sample {
                height: heights[p],
                normal: Vec3::new(-dx, 2.0 * step, -dz).normalize(),
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_match_height_and_normal() {
        let calls = std::cell::Cell::new(0);
        let height = |x: f32, z: f32| {
            calls.set(calls.get() + 1);
            x * 0.2 - z * 0.3
        };
        const CELLS: usize = 4;
        let samples = sample(9800.0, -1800.0, 256.0, CELLS, height);
        assert_eq!(calls.get(), (CELLS + 3) * (CELLS + 3));
        for j in 0..=CELLS {
            for i in 0..=CELLS {
                let (x, z) = (
                    (9800 - 128 + i * 64) as f32,
                    (-1800 - 128 + j as i32 * 64) as f32,
                );
                let s = samples[j * (CELLS + 1) + i];
                assert_eq!(s.height, height(x, z));
                let want = terrain_normal(height, x, z, 64.0);
                assert!(s.normal.distance(want) < 1e-6 && (s.normal.length() - 1.0).abs() < 1e-6);
            }
        }
        // The real nonlinear terrain too, at the arrival and canyon country.
        for at in [Vec2::new(ARRIVAL.x, ARRIVAL.z), HAVEN_AT] {
            let (ci, cj) = chunk_of(at.x, at.y);
            let center = chunk_centre(ci, cj);
            let grid = sample(center.x, center.y, CHUNK_SIZE, CHUNK_CELLS, drawn_height);
            for [i, j] in [
                [0, 0],
                [CHUNK_CELLS, CHUNK_CELLS],
                [CHUNK_CELLS / 2, CHUNK_CELLS / 2],
                [7, 19],
            ] {
                let step = CHUNK_SIZE / CHUNK_CELLS as f32;
                let x = center.x - CHUNK_SIZE / 2.0 + i as f32 * step;
                let z = center.y - CHUNK_SIZE / 2.0 + j as f32 * step;
                let s = grid[j * (CHUNK_CELLS + 1) + i];
                assert_eq!(s.height, drawn_height(x, z));
                assert!(s.normal.distance(terrain_normal(drawn_height, x, z, step)) < 1e-6);
            }
        }
    }

    #[test]
    fn level_where_it_matters() {
        for (name, x, z) in [
            ("the Hull", -16.0, 4.0),
            ("Charter Row", -4.0, -36.0),
            ("outside the gate", 0.0, 60.0),
            ("the arrival", ARRIVAL.x, ARRIVAL.z),
            ("the Pads", PADS_AT.x, PADS_AT.y),
            ("the caravan road", 3900.0, -120.0),
            ("the Fringe track", -850.0, 5400.0),
            ("the hold", HOLD_AT.x, HOLD_AT.y),
            ("the farms", -1625.0, 12023.0),
            ("the salvage fields", -1375.0, 12020.0),
            ("the wind farm", -1600.0, 11900.0),
            ("the haven", HAVEN_AT.x, HAVEN_AT.y),
            ("the wash out of the haven", -6000.0, -6200.0),
        ] {
            assert_eq!(ground_height(x, z), 0.0, "{name}: want level ground");
        }
    }

    #[test]
    fn rises_to_the_edge() {
        let mut high = 0.0f32;
        for p in [
            Vec2::new(16000.0, 0.0),
            Vec2::new(-16000.0, 3000.0),
            Vec2::new(2000.0, -16000.0),
            Vec2::new(-6000.0, 16000.0),
        ] {
            let h = ground_height(p.x, p.y);
            high = high.max(h);
            assert!(
                h >= 80.0,
                "at the world's edge {p}, ground {h}, want mountains"
            );
        }
        assert!(high <= 200.0, "mountains {high} m high, want under 200");
    }

    #[test]
    fn seats_are_far_apart() {
        let seats = [
            ("Landfall", Vec2::ZERO),
            ("the Pads", PADS_AT),
            ("the hold", HOLD_AT),
            ("the haven", HAVEN_AT),
        ];
        for (a, pa) in seats {
            for (b, pb) in seats {
                if a < b {
                    assert!(pa.distance(pb) >= 8000.0, "{a} and {b} are too close");
                }
            }
        }
        for (name, p) in &seats[1..] {
            assert!(p.length() <= 15000.0, "{name} is too far from Landfall");
        }
    }

    #[test]
    fn hash_is_the_go_lattice() {
        // Go's lattice(0, 0, 0) is 0; the rest pin the hash's constants.
        assert_eq!(lattice(0, 0, 0), 0.0);
        let a = lattice(3, -7, 2);
        assert!((0.0..=1.0).contains(&a));
        assert_ne!(a, lattice(3, -7, 3));
        assert_ne!(a, lattice(-7, 3, 2));
        assert_eq!(a, lattice(3, -7, 2));
    }

    #[test]
    fn floors_lift_the_walk_and_sink_the_drawing() {
        let (x, z) = (-16.0, 4.0);
        assert_eq!(walk_height(x, z), -GROUND_LEVEL);
        assert_eq!(drawn_height(x, z), -UNDER_FLOORS);
        assert_eq!(walk_height(3900.0, -120.0), 0.0);
    }
}
