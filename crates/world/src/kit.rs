//! The pieces the world is built from and where they go: the formats of
//! `assets/world/world.json` and the layout files (`hull_block.json`,
//! `landfall.json`), as `world/world.go` reads them. These are the
//! authoritative content inputs; the client composes BSN from them.

use std::{collections::BTreeMap, fs, io, path::Path};

use bevy_math::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// The pieces file: every piece that can be placed, by its stable name.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Kit {
    pub pieces: BTreeMap<String, Piece>,
}

/// One piece of the world (architecture, a prop, a carried item or a
/// vehicle), made standing on its origin in metres. A lamp has the light
/// it gives.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Piece {
    /// The model, relative to the asset root.
    pub model: String,
    /// "kit", "prop", "item", "vehicle" or "wheel" (a vehicle's).
    pub kind: String,
    /// The part of the world it's from, e.g. "The Fringe".
    pub category: String,
    /// The most triangles it may have.
    #[serde(default)]
    pub budget: u32,
    #[serde(default)]
    pub colliders: Vec<Collider>,
    #[serde(default)]
    pub ladders: Vec<Ladder>,
    #[serde(default)]
    pub lights: Vec<Light>,
    /// How it drives, for a vehicle that can be driven.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vehicle: Option<VehicleSpec>,
}

impl Piece {
    /// The kinds a piece may have.
    pub const KINDS: [&'static str; 5] = ["kit", "prop", "item", "vehicle", "wheel"];
}

/// A box the piece collides with: its middle, its full size along its own
/// axes and how it's turned.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Collider {
    pub center: [f32; 3],
    pub size: [f32; 3],
    /// x, y, z, w.
    pub rotation: [f32; 4],
}

impl Collider {
    /// Where the collider is, and how it's turned, on a piece at `at`
    /// turned by `turn`.
    pub fn in_frame(&self, at: Vec3, turn: Quat) -> (Vec3, Quat) {
        let rotation = Quat::from_array(self.rotation);
        (at + turn * Vec3::from(self.center), turn * rotation)
    }

    /// The box round the collider's corners on a piece at the origin.
    pub fn bounds(&self) -> (Vec3, Vec3) {
        let (center, rotation) = self.in_frame(Vec3::ZERO, Quat::IDENTITY);
        let half = Vec3::from(self.size) / 2.0;
        let mut lo = Vec3::INFINITY;
        let mut hi = Vec3::NEG_INFINITY;
        for corner in 0..8 {
            let sign = Vec3::new(
                if corner & 1 == 0 { -1.0 } else { 1.0 },
                if corner & 2 == 0 { -1.0 } else { 1.0 },
                if corner & 4 == 0 { -1.0 } else { 1.0 },
            );
            let p = center + rotation * (sign * half);
            lo = lo.min(p);
            hi = hi.max(p);
        }
        (lo, hi)
    }
}

/// A ladder in the piece's own frame.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ladder {
    pub bottom: [f32; 3],
    pub top: [f32; 3],
    pub facing: [f32; 3],
    pub bottom_exit: [f32; 3],
    pub top_exit: [f32; 3],
    pub width: f32,
}

/// A ladder placed in the world, as `character.Ladder` has it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedLadder {
    pub bottom: Vec3,
    pub top: Vec3,
    pub facing: Vec3,
    pub bottom_exit: Vec3,
    pub top_exit: Vec3,
    pub width: f32,
}

impl Ladder {
    /// The ladder on a piece at `at` turned by `turn`.
    pub fn in_frame(&self, at: Vec3, turn: Quat) -> PlacedLadder {
        let point = |v: [f32; 3]| at + turn * Vec3::from(v);
        PlacedLadder {
            bottom: point(self.bottom),
            top: point(self.top),
            facing: turn * Vec3::from(self.facing),
            bottom_exit: point(self.bottom_exit),
            top_exit: point(self.top_exit),
            width: self.width,
        }
    }
}

/// A point light in the piece's own frame: where it is, its colour (0 to
/// 1), how bright, and how far it reaches in metres.
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
pub struct Light {
    pub at: [f32; 3],
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

/// A light placed in the world. Its colour is clamped to 0 to 1; the Go
/// reference rounds it to 8 bits, see [`PlacedLight::color_bytes`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlacedLight {
    pub at: Vec3,
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

impl PlacedLight {
    /// The colour as the Go client's `render.PointLight` carries it.
    pub fn color_bytes(&self) -> [u8; 3] {
        self.color.map(|c| (c * 255.0).round() as u8)
    }
}

impl Light {
    /// The light, and where it is, on a piece at `at` turned by `turn`.
    pub fn in_frame(&self, at: Vec3, turn: Quat) -> PlacedLight {
        PlacedLight {
            at: at + turn * Vec3::from(self.at),
            color: self.color.map(|c| c.clamp(0.0, 1.0)),
            intensity: self.intensity,
            range: self.range,
        }
    }
}

/// How a vehicle drives: `vehicle.Spec` in the Go reference. Kept as data
/// here; the vehicle port decides what it does with it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VehicleSpec {
    /// Names its entry in the handlings.
    #[serde(default)]
    pub handling: String,
    /// Where its weight is, relative to its origin; zero puts it at the
    /// middle of the chassis.
    #[serde(default)]
    pub center_of_mass: [f32; 3],
    /// How far behind the chase camera follows; 0 picks a distance from
    /// its size.
    #[serde(default)]
    pub camera: f32,
    /// The boxes it collides with, which clear its wheels.
    #[serde(default)]
    pub chassis: Vec<Collider>,
    #[serde(default)]
    pub wheels: Vec<WheelSpec>,
    #[serde(default)]
    pub seats: Vec<SeatSpec>,
    #[serde(default)]
    pub headlamps: Vec<HeadlampSpec>,
}

/// One of a vehicle's wheels.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WheelSpec {
    /// The wheel's own piece, modelled centred on its axle, the axle along
    /// X, as a wheel on the vehicle's left (+X) side.
    pub piece: String,
    /// The wheel's centre as the vehicle is modelled, parked.
    pub at: [f32; 3],
    #[serde(default)]
    pub radius: f32,
    #[serde(default)]
    pub width: f32,
    /// On the +X side.
    #[serde(default)]
    pub left: bool,
    /// Turns with the steering.
    #[serde(default)]
    pub steer: bool,
    /// The engine turns it.
    #[serde(default)]
    pub drive: bool,
    /// The hand brake holds it.
    #[serde(default)]
    pub hand_brake: bool,
}

/// A seat: where the sitter's model origin goes, facing the vehicle's
/// front; "drive", "ride" or "inside"; and where they can stand on getting
/// out, tried in order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeatSpec {
    pub at: [f32; 3],
    #[serde(default)]
    pub pose: String,
    #[serde(default)]
    pub exits: Vec<[f32; 3]>,
    #[serde(default)]
    pub grips: Vec<[f32; 3]>,
    #[serde(default)]
    pub pegs: Vec<[f32; 3]>,
}

/// A headlamp's lens centre and radius, in metres.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HeadlampSpec {
    pub at: [f32; 3],
    #[serde(default)]
    pub radius: f32,
}

/// Where a piece goes: at its origin's position, turned `turns` quarter
/// turns anticlockwise seen from above.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Placement {
    pub piece: String,
    pub at: [f32; 3],
    #[serde(default)]
    pub turns: i32,
}

impl Placement {
    pub fn at(&self) -> Vec3 {
        Vec3::from(self.at)
    }

    /// A quarter turn is anticlockwise seen from above: what was along +X
    /// goes along -Z.
    pub fn turn(&self) -> Quat {
        quarter_turns(self.turns)
    }
}

/// The rotation of `turns` quarter turns about +Y.
pub fn quarter_turns(turns: i32) -> Quat {
    Quat::from_axis_angle(Vec3::Y, turns as f32 * std::f32::consts::FRAC_PI_2)
}

/// A layout file: the pieces it places, in order.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Layout {
    pub pieces: Vec<Placement>,
}

/// Why a kit or layout couldn't be read.
#[derive(Debug)]
pub enum Error {
    Io(String, io::Error),
    Json(String, serde_json::Error),
    /// A layout places a piece the kit doesn't have.
    NoPiece(String),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(path, err) => write!(f, "{path}: {err}"),
            Error::Json(path, err) => write!(f, "{path}: {err}"),
            Error::NoPiece(piece) => write!(f, "world: no piece {piece:?}"),
        }
    }
}

impl std::error::Error for Error {}

fn read_json<T: for<'de> Deserialize<'de>>(root: &Path, path: &str) -> Result<T, Error> {
    let full = root.join(path);
    let shown = full.display().to_string();
    let data = fs::read(&full).map_err(|err| Error::Io(shown.clone(), err))?;
    serde_json::from_slice(&data).map_err(|err| Error::Json(shown, err))
}

impl Kit {
    /// Reads the pieces file at `path` under `root`, without loading models.
    pub fn load(root: impl AsRef<Path>, path: &str) -> Result<Kit, Error> {
        read_json(root.as_ref(), path)
    }

    /// Parses a pieces file already in memory (the browser fetches it).
    pub fn parse(data: &[u8]) -> Result<Kit, serde_json::Error> {
        serde_json::from_slice(data)
    }

    pub fn piece(&self, name: &str) -> Result<&Piece, Error> {
        self.pieces
            .get(name)
            .ok_or_else(|| Error::NoPiece(name.to_string()))
    }
}

impl Layout {
    /// Reads a layout file (e.g. `world/hull_block.json`) under `root`.
    pub fn load(root: impl AsRef<Path>, path: &str) -> Result<Layout, Error> {
        read_json(root.as_ref(), path)
    }

    pub fn parse(data: &[u8]) -> Result<Layout, serde_json::Error> {
        serde_json::from_slice(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    #[test]
    fn quarter_turn_takes_x_to_minus_z() {
        let c = Collider {
            center: [1.0, 0.5, 0.0],
            size: [2.0, 1.0, 0.5],
            rotation: [0.0, 0.0, 0.0, 1.0],
        };
        let turn = quarter_turns(1);
        let (center, _) = c.in_frame(Vec3::new(10.0, 0.0, 0.0), turn);
        assert!(
            center.distance(Vec3::new(10.0, 0.5, -1.0)) < EPSILON,
            "{center}"
        );
        let ladder = Ladder {
            facing: [0.0, 0.0, -1.0],
            ..Default::default()
        }
        .in_frame(Vec3::ZERO, turn);
        assert!(
            ladder.facing.distance(Vec3::NEG_X) < EPSILON,
            "{}",
            ladder.facing
        );
    }

    #[test]
    fn light_turns_with_its_piece_and_keeps_its_colour() {
        let light = Light {
            at: [1.0, 2.5, 0.0],
            color: [1.0, 0.5, 0.0],
            intensity: 2.0,
            range: 6.0,
        }
        .in_frame(Vec3::new(10.0, 0.0, 0.0), quarter_turns(1));
        assert!(
            light.at.distance(Vec3::new(10.0, 2.5, -1.0)) < EPSILON,
            "{}",
            light.at
        );
        assert_eq!(light.color_bytes(), [255, 128, 0]);
        assert_eq!((light.intensity, light.range), (2.0, 6.0));
    }

    #[test]
    fn collider_bounds_follow_its_rotation() {
        let c = Collider {
            center: [0.0, 1.0, 0.0],
            size: [2.0, 1.0, 4.0],
            rotation: quarter_turns(1).to_array(),
        };
        let (lo, hi) = c.bounds();
        assert!(lo.distance(Vec3::new(-2.0, 0.5, -1.0)) < EPSILON, "{lo}");
        assert!(hi.distance(Vec3::new(2.0, 1.5, 1.0)) < EPSILON, "{hi}");
    }

    #[test]
    fn pieces_file_fields_round_trip() {
        let kit = Kit::parse(
            br#"{"pieces":{"lamp":{"model":"world/lamp.glb","kind":"prop","category":"The Hull",
            "budget":10,"colliders":[],"lights":[{"at":[0,1,0],"color":[1,0.5,0.1],"intensity":1.3,"range":8}],
            "sources":["ignored"]}}}"#,
        )
        .unwrap();
        let lamp = kit.piece("lamp").unwrap();
        assert_eq!(lamp.lights.len(), 1);
        assert!(lamp.vehicle.is_none());
        assert!(matches!(kit.piece("nothing"), Err(Error::NoPiece(_))));
        let layout =
            Layout::parse(br#"{"pieces":[{"piece":"lamp","at":[1,2,3],"turns":3}]}"#).unwrap();
        assert_eq!(layout.pieces[0].turn(), quarter_turns(3));
    }
}
