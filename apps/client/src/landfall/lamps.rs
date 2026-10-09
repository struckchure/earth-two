//! The fixed lamps (`game/lamps.go`): the same light definitions and
//! placements `setup` uses, including terrain height and rotation. The Go
//! renderer compiled them into its shader before startup, so camera
//! movement cannot switch lamps; the shading port reads them from the
//! [`FixedLamps`] resource.

use bevy::prelude::*;
use earth_two_world::kit::{self, Kit, Layout, Placement, quarter_turns};

use super::terrain::stand_on;

/// `shading.Lamp`: a point light where it stands in the world, its colour
/// 0 to 1, the game's intensity scale, and its reach in metres.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FixedLamp {
    pub at: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

/// The lamps of the layout Landfall loaded, in placement order.
#[derive(Resource, Debug, Clone, Default)]
pub struct FixedLamps(pub Vec<FixedLamp>);

/// `fixedLamps` on a kit and layout already read: the lights of every
/// placed piece that has them, but the vehicles', each where the piece
/// stands on the terrain.
pub fn fixed_lamps_of(k: &Kit, placed: &[Placement]) -> Result<Vec<FixedLamp>, kit::Error> {
    let mut lamps = Vec::new();
    for p in placed {
        let piece = k.piece(&p.piece)?;
        if piece.lights.is_empty() || piece.vehicle.is_some() {
            continue;
        }
        let at = p.at() + Vec3::new(0.0, stand_on(k, p), 0.0);
        let turn = quarter_turns(p.turns);
        for l in &piece.lights {
            let light = l.in_frame(at, turn);
            lamps.push(FixedLamp {
                at: light.at,
                color: light.color,
                intensity: light.intensity,
                range: light.range,
            });
        }
    }
    Ok(lamps)
}

/// `fixedLamps`: reads `world/world.json` and `world/landfall.json` under
/// `root` and returns their lamps.
pub fn fixed_lamps(root: &str) -> Result<Vec<FixedLamp>, kit::Error> {
    let k = Kit::load(root, "world/world.json")?;
    let placed = Layout::load(root, "world/landfall.json")?;
    fixed_lamps_of(&k, &placed.pieces)
}
