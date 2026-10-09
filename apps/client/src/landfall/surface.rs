//! What's underfoot (`game/surface.go`), for how a step sounds and whether
//! it raises dust: the rugs, the paving, the floors, the roads, the dome's
//! ground, the canyon rock, and the sand everywhere else; and the
//! soundscape the cues need: where the paving and rugs are, what's making
//! a noise where, and where the bell is.

use std::collections::HashMap;

use bevy::prelude::*;
use earth_two_world::{
    kit::{Kit, Placement},
    terrain::{FLOORED, canyon, road_distance},
};

use super::{
    ground::{DOME_WALLS, abs, rect_distance},
    terrain::stand_on,
};

/// What's underfoot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Footing {
    /// The Red's dunes, and the banks against the dome.
    #[default]
    Sand,
    /// The dome's beds and lawns, and the packed dirt of the roads.
    Soil,
    /// Concrete and the Hull's deck: Charter Row, the Pads.
    Paving,
    /// Kit underfoot: catwalks, stairs, deck slabs, plinths.
    Grating,
    /// A rug on the floor.
    Rug,
    /// The canyon country.
    Rock,
}

impl Footing {
    /// The sound of a step on it, and how it's pitched.
    pub fn step(self) -> (&'static str, f32) {
        match self {
            Footing::Soil => ("step_soil", 1.0),
            Footing::Paving => ("step_paving", 1.0),
            Footing::Grating => ("step_grating", 1.0),
            Footing::Rug => ("step_rug", 1.0),
            Footing::Rock => ("step_paving", 0.85),
            Footing::Sand => ("step_sand", 1.0),
        }
    }

    /// Whether it's loose enough to kick up dust.
    pub fn loose(self) -> bool {
        matches!(self, Footing::Sand | Footing::Soil)
    }
}

/// The size of Charter Row's paving tiles, and the grid they're found on.
pub const PAVING_CELL: f32 = 2.0;
/// How far from its middle a rug is underfoot.
pub const RUG_REACH: f32 = 1.2;

/// The pieces that make a noise, and the loop each makes.
pub const SOURCE_LOOPS: [(&str, &str); 9] = [
    ("generator", "generator"),
    ("pump_unit", "generator"),
    ("air_scrubber", "fans"),
    ("air_fan", "fans"),
    ("hull_vent", "fans"),
    ("fountain", "fountain"),
    ("market_stall", "market"),
    ("food_stall", "market"),
    ("parts_stall", "market"),
];

/// `soundscape`, a resource: what the cues need to know about the world
/// that the world doesn't say.
#[derive(Resource, Debug, Clone, Default)]
pub struct Soundscape {
    /// The paving tiles' middles, by the [`PAVING_CELL`] they're in.
    pub paved: HashMap<(i32, i32), Vec<Vec2>>,
    pub rugs: Vec<Vec3>,
    /// The places each loop is heard from, by the loop's name: the
    /// machines' hums, the market's chatter, the fountain.
    pub sources: HashMap<String, Vec<Vec3>>,
    pub bell: Option<Vec3>,
}

pub fn paving_of(x: f32, z: f32) -> (i32, i32) {
    (
        ((x / PAVING_CELL) as f64).floor() as i32,
        ((z / PAVING_CELL) as f64).floor() as i32,
    )
}

impl Soundscape {
    /// `newSoundscape`.
    pub fn new(k: &Kit, placed: &[Placement]) -> Soundscape {
        let mut s = Soundscape::default();
        for p in placed {
            let mut at = p.at();
            match p.piece.as_str() {
                "charter_paving" => {
                    s.paved
                        .entry(paving_of(at.x, at.z))
                        .or_default()
                        .push(Vec2::new(at.x, at.z));
                    continue;
                }
                "rug" | "rug_runner" => {
                    s.rugs.push(at);
                    continue;
                }
                "floor_bell" => {
                    at.y += stand_on(k, p);
                    s.bell = Some(at + Vec3::new(0.0, 2.0, 0.0));
                }
                _ => {}
            }
            if let Some((_, l)) = SOURCE_LOOPS.iter().find(|(piece, _)| *piece == p.piece) {
                at.y += stand_on(k, p) + 1.0;
                s.sources.entry(l.to_string()).or_default().push(at);
            }
        }
        s
    }

    /// `onPaving`: whether (x, z) is on a paving tile: within half a tile
    /// of one's middle, in its cell or one next to it.
    pub fn on_paving(&self, x: f32, z: f32) -> bool {
        let c = paving_of(x, z);
        for dx in -1..=1 {
            for dz in -1..=1 {
                if let Some(middles) = self.paved.get(&(c.0 + dx, c.1 + dz)) {
                    for m in middles {
                        if abs(m.x - x) <= PAVING_CELL / 2.0 && abs(m.y - z) <= PAVING_CELL / 2.0 {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// `nearest`: the source of `l` nearest p, and how far it is.
    pub fn nearest(&self, l: &str, p: Vec3) -> Option<(Vec3, f32)> {
        let mut best = None;
        let mut far = f32::INFINITY;
        for at in self.sources.get(l).map(Vec::as_slice).unwrap_or(&[]) {
            let d = at.distance(p);
            if d < far {
                best = Some(*at);
                far = d;
            }
        }
        best.map(|at| (at, far))
    }

    /// `surfaceAt`: what's underfoot at `feet`: `on_kit` is whether what's
    /// under them is a piece of kit (a ray down hit one of the world's
    /// colliders, not the ground).
    pub fn surface_at(&self, feet: Vec3, on_kit: bool) -> Footing {
        let (x, z) = (feet.x, feet.z);
        for r in &self.rugs {
            let (dx, dz) = (r.x - x, r.z - z);
            if dx * dx + dz * dz < RUG_REACH * RUG_REACH && abs(r.y - feet.y) < 0.5 {
                return Footing::Rug;
            }
        }
        if on_kit {
            return Footing::Grating;
        }
        for f in &FLOORED {
            if x >= f.min.x && x <= f.max.x && z >= f.min.y && z <= f.max.y {
                return Footing::Paving;
            }
        }
        if self.on_paving(x, z) {
            return Footing::Paving;
        }
        if road_distance(x, z).0 <= 0.0 {
            return Footing::Soil;
        }
        if rect_distance(&DOME_WALLS, x, z) == 0.0 {
            return Footing::Soil;
        }
        if canyon(x, z) > 0.5 {
            return Footing::Rock;
        }
        Footing::Sand
    }
}
