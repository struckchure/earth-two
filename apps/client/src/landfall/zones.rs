//! The light zones (`game/zones.go`, and `lightZones` in `game.go`): where
//! the light isn't the open air's, in the game's frame, from
//! tools/world/landfall.py's DOME_* and HULL_*. Under the dome the sun
//! comes through filtered and warm; inside the Hull it's the sodium work
//! lamps' warm fill. How far into a zone a point is blends the toon shader
//! (shading/toon.go), so what's heard and what's lit change in the same
//! place.

use bevy::prelude::*;

use super::{ground::mix_colour, maps::Color};

/// `shading.Zone`: a box of different light, blended `blend` metres in
/// from its faces. The shading port owns how it reaches the shader; this
/// is the data and the blend the rest of the game asks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    pub min: Vec3,
    pub max: Vec3,
    pub blend: f32,
    pub ambient: Color,
    pub brightness: f32,
    pub sun: Color,
}

pub const LIGHT_ZONES: [Zone; 2] = [
    Zone {
        min: Vec3::new(-54.0, -5.0, -56.0),
        max: Vec3::new(34.0, 30.0, 44.0),
        blend: 3.0,
        ambient: [232, 200, 170, 255],
        brightness: 0.32,
        sun: [255, 232, 205, 255],
    },
    Zone {
        min: Vec3::new(-40.0, -5.0, -12.0),
        max: Vec3::new(8.0, 11.0, 20.0),
        blend: 1.5,
        ambient: [255, 186, 130, 255],
        brightness: 0.8,
        sun: [255, 240, 220, 255],
    },
];

/// The sun's colour at noon (`sunlight` in sky.go).
pub const SUNLIGHT: Color = [255, 182, 128, 255];

fn smoothstep(a: f32, b: f32, v: f32) -> f32 {
    earth_two_world::terrain::smoothstep(a, b, v)
}

/// `zoneWeight`: how far into zone z the point p is, 0 to 1.
pub fn zone_weight(z: &Zone, p: Vec3) -> f32 {
    let in_ = |lo: f32, hi: f32, v: f32| {
        smoothstep(lo, lo + z.blend, v) * (1.0 - smoothstep(hi - z.blend, hi, v))
    };
    in_(z.min.x, z.max.x, p.x) * in_(z.min.y, z.max.y, p.y) * in_(z.min.z, z.max.z, p.z)
}

/// How much a point is in each of the light zones: the open air, under
/// the dome, inside the Hull. They add up to 1.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Whereabouts {
    pub outside: f32,
    pub dome: f32,
    pub hull: f32,
}

/// `placesAt`: how much p is outside, under the dome and in the Hull, the
/// later zone in [`LIGHT_ZONES`] winning over the earlier, as in the
/// shader.
pub fn places_at(p: Vec3) -> Whereabouts {
    let mut dome = zone_weight(&LIGHT_ZONES[0], p);
    let hull = zone_weight(&LIGHT_ZONES[1], p);
    dome *= 1.0 - hull;
    Whereabouts {
        outside: 1.0 - dome - hull,
        dome,
        hull,
    }
}

/// `lightAt`: the colour of the light at p, for what's drawn unlit there
/// (the dust): the sun's outside, the zones' fill inside them.
pub fn light_at(p: Vec3) -> Color {
    let mut c = SUNLIGHT;
    for z in &LIGHT_ZONES {
        c = mix_colour(c, z.ambient, zone_weight(z, p));
    }
    c
}
