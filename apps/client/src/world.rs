//! Places the world's pieces from the existing manifests. `world/world.json`
//! and the layout files stay the content inputs; this composes each
//! placement into a BSN assembly: the piece's root (its logical ID, model
//! path and placement transform) with its colliders, ladders, lights and
//! wheels as children. Systems own what the entities then do: Avian
//! collides with the colliders, the viewer attaches models and lights.
//!
//! Runtime entity IDs stay internal: [`Placed`] carries the piece name the
//! assets and accounts use.

use avian3d::prelude::*;
use bevy::{
    asset::{AssetLoader, LoadContext, io::Reader},
    prelude::*,
};
use earth_two_world::kit::{self, Kit, Layout, Piece, Placement};

/// `world/world.json` as an asset, so the browser fetches it like the Go
/// client's asset root.
#[derive(Asset, TypePath, Debug)]
pub struct KitAsset(pub Kit);

/// A layout file as an asset.
#[derive(Asset, TypePath, Debug)]
pub struct LayoutAsset(pub Layout);

#[derive(Debug)]
pub enum LoadError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Io(err) => write!(f, "reading the world file: {err}"),
            LoadError::Json(err) => write!(f, "parsing the world file: {err}"),
        }
    }
}

impl std::error::Error for LoadError {}

#[derive(Default, TypePath)]
pub struct KitLoader;

impl AssetLoader for KitLoader {
    type Asset = KitAsset;
    type Settings = ();
    type Error = LoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> Result<KitAsset, LoadError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(LoadError::Io)?;
        Kit::parse(&bytes).map(KitAsset).map_err(LoadError::Json)
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

#[derive(Default, TypePath)]
pub struct LayoutLoader;

impl AssetLoader for LayoutLoader {
    type Asset = LayoutAsset;
    type Settings = ();
    type Error = LoadError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _context: &mut LoadContext<'_>,
    ) -> Result<LayoutAsset, LoadError> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(LoadError::Io)?;
        Layout::parse(&bytes)
            .map(LayoutAsset)
            .map_err(LoadError::Json)
    }

    fn extensions(&self) -> &[&str] {
        &["json"]
    }
}

/// Asks for a layout to be placed once its kit and layout files load. The
/// entity is replaced by a [`LayoutRoot`] with the pieces as its children,
/// so despawning that root removes the whole layout.
#[derive(Component, Debug)]
pub struct SpawnLayout {
    pub kit: Handle<KitAsset>,
    pub layout: Handle<LayoutAsset>,
}

impl SpawnLayout {
    /// Loads `kit` and `layout` relative to the asset root, e.g.
    /// `"world/world.json"` and `"world/hull_block.json"`.
    pub fn load(assets: &AssetServer, kit: &str, layout: &str) -> SpawnLayout {
        SpawnLayout {
            kit: assets.load(kit.to_string()),
            layout: assets.load(layout.to_string()),
        }
    }
}

/// The root of a placed layout; its children are the pieces.
#[derive(Component, Debug, Default, Clone)]
pub struct LayoutRoot {
    pub layout: String,
}

/// A placed piece's root: the stable piece name from the manifest, and its
/// index in the layout.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct Placed {
    pub piece: String,
    pub index: usize,
}

/// A model to draw here, by its path relative to the asset root. The
/// viewer turns this into a loaded scene and adds visibility to the
/// hierarchy; headless builds have no renderer and leave it as data.
#[derive(Component, Debug, Default, Clone, PartialEq, Eq)]
pub struct PieceModel(pub String);

/// One of a piece's static collision boxes, in the piece's frame.
#[derive(Component, Debug, Default, Clone)]
pub struct PieceCollider;

/// A ladder, as `character.Ladder` has it, in world space.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Ladder {
    pub bottom: Vec3,
    pub top: Vec3,
    pub facing: Vec3,
    pub bottom_exit: Vec3,
    pub top_exit: Vec3,
    pub width: f32,
}

impl Default for Ladder {
    fn default() -> Self {
        Ladder {
            bottom: Vec3::ZERO,
            top: Vec3::ZERO,
            facing: Vec3::NEG_Z,
            bottom_exit: Vec3::ZERO,
            top_exit: Vec3::ZERO,
            width: 0.0,
        }
    }
}

impl From<kit::PlacedLadder> for Ladder {
    fn from(l: kit::PlacedLadder) -> Self {
        Ladder {
            bottom: l.bottom,
            top: l.top,
            facing: l.facing,
            bottom_exit: l.bottom_exit,
            top_exit: l.top_exit,
            width: l.width,
        }
    }
}

/// A piece's light as the manifest gives it: colour 0 to 1, the game's
/// intensity scale and reach in metres. The shading port decides how it
/// maps to a renderer light.
#[derive(Component, Debug, Default, Clone, Copy, PartialEq)]
pub struct Lamp {
    pub color: [f32; 3],
    pub intensity: f32,
    pub range: f32,
}

/// A vehicle parked where the layout put it. It is static until the vehicle
/// port; its chassis boxes collide and its wheels are drawn in place.
#[derive(Component, Debug, Default, Clone)]
pub struct ParkedVehicle {
    pub handling: String,
}

/// A wheel drawn on a parked vehicle.
#[derive(Component, Debug, Default, Clone)]
pub struct Wheel;

/// The scenes of every placement in `layout`, in order, each a piece root
/// with its parts as children. A placement naming a piece the kit lacks is
/// an error, as `world.Kit.Place` has it.
pub fn assembly(kit: &Kit, layout: &Layout) -> Result<Vec<Box<dyn Scene>>, kit::Error> {
    layout
        .pieces
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let piece = kit.piece(&placement.piece)?;
            Ok(piece_scene(kit, piece, placement, index))
        })
        .collect()
}

/// One placed piece, as the Go client's `Kit.Place` spawns it: its model
/// where it stands, a static box for each collider, its ladders and lights;
/// or a parked vehicle with its chassis and wheels.
pub fn piece_scene(
    kit: &Kit,
    piece: &Piece,
    placement: &Placement,
    index: usize,
) -> Box<dyn Scene> {
    let at = placement.at();
    let turn = placement.turn();
    let placed = Placed {
        piece: placement.piece.clone(),
        index,
    };
    let model = PieceModel(piece.model.clone());
    let name = Name::new(format!("{} {}", placement.piece, index));
    let transform = Transform::from_translation(at).with_rotation(turn);

    if let Some(vehicle) = &piece.vehicle {
        let chassis: Vec<_> = vehicle.chassis.iter().map(collider_scene).collect();
        let wheels: Vec<_> = vehicle
            .wheels
            .iter()
            .map(|wheel| {
                let model = PieceModel(
                    kit.pieces
                        .get(&wheel.piece)
                        .map(|p| p.model.clone())
                        .unwrap_or_default(),
                );
                // Wheels are modelled for the left side; the right are turned about.
                let rotation = if wheel.left {
                    Quat::IDENTITY
                } else {
                    Quat::from_rotation_y(std::f32::consts::PI)
                };
                let transform =
                    Transform::from_translation(Vec3::from(wheel.at)).with_rotation(rotation);
                let name = Name::new(wheel.piece.clone());
                bsn! {
                    name
                    Wheel
                    model
                    transform
                }
            })
            .collect();
        let parked = ParkedVehicle {
            handling: vehicle.handling.clone(),
        };
        return Box::new(bsn! {
            name
            placed
            model
            parked
            transform
            RigidBody::Static
            Children [ {chassis} -- {wheels} ]
        });
    }

    let colliders: Vec<_> = piece.colliders.iter().map(collider_scene).collect();
    let ladders: Vec<_> = piece
        .ladders
        .iter()
        .map(|ladder| {
            let ladder = Ladder::from(ladder.in_frame(at, turn));
            bsn! { Name::new("ladder") ladder }
        })
        .collect();
    let lamps: Vec<_> = piece
        .lights
        .iter()
        .map(|light| {
            let placed = light.in_frame(Vec3::ZERO, Quat::IDENTITY);
            let lamp = Lamp {
                color: placed.color,
                intensity: placed.intensity,
                range: placed.range,
            };
            let transform = Transform::from_translation(placed.at);
            bsn! { Name::new("lamp") lamp transform }
        })
        .collect();
    // A piece with nothing to collide with needs no body.
    let body: Box<dyn Scene> = if piece.colliders.is_empty() {
        Box::new(bsn! {})
    } else {
        Box::new(bsn! { RigidBody::Static })
    };
    Box::new(bsn! {
        name
        placed
        model
        transform
        @{body}
        Children [ {colliders} -- {ladders} -- {lamps} ]
    })
}

/// A static box in its piece's frame; Avian attaches it to the piece's body
/// and places it by the propagated transform, which is where
/// `Collider::in_frame` puts it.
fn collider_scene(box_: &kit::Collider) -> impl Scene {
    let transform = Transform::from_translation(Vec3::from(box_.center))
        .with_rotation(Quat::from_array(box_.rotation));
    let [x, y, z] = box_.size;
    bsn! {
        Name::new("collider")
        PieceCollider
        transform
        Collider::cuboid(x, y, z)
    }
}

/// Registers the manifest assets and places requested layouts.
pub struct WorldPlugin;

impl Plugin for WorldPlugin {
    fn build(&self, app: &mut App) {
        app.init_asset::<KitAsset>()
            .init_asset::<LayoutAsset>()
            .register_asset_loader(KitLoader)
            .register_asset_loader(LayoutLoader)
            .add_systems(Update, place_layouts);
    }
}

/// Places each requested layout once its files are loaded. Errors are
/// logged and the request removed, so a bad layout fails once, not every
/// frame.
fn place_layouts(
    mut commands: Commands,
    requests: Query<(Entity, &SpawnLayout)>,
    kits: Res<Assets<KitAsset>>,
    layouts: Res<Assets<LayoutAsset>>,
    assets: Res<AssetServer>,
) {
    for (entity, request) in &requests {
        let path = request
            .layout
            .path()
            .map(|p| p.to_string())
            .unwrap_or_default();
        let (Some(kit), Some(layout)) = (kits.get(&request.kit), layouts.get(&request.layout))
        else {
            if assets.load_state(&request.kit).is_failed()
                || assets.load_state(&request.layout).is_failed()
            {
                error!("world: could not load the kit or layout {path}");
                commands.entity(entity).despawn();
            }
            continue;
        };
        commands.entity(entity).despawn();
        match assembly(&kit.0, &layout.0) {
            Ok(pieces) => {
                let root = LayoutRoot {
                    layout: path.clone(),
                };
                commands.spawn_scene(bsn! {
                    Name::new(path)
                    root
                    Transform::default()
                    Children [ {pieces} ]
                });
            }
            Err(err) => error!("{err}"),
        }
    }
}
