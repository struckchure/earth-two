//! Turns a parked piece into a drivable vehicle: `vehicle.Spawn` in Go. The
//! world module places a vehicle piece as a Static root with its chassis
//! boxes and wheel models as children ([`ParkedVehicle`]); this adds what
//! makes it drive, from its Spec in the kit, without touching that module.

use std::collections::BTreeMap;

use avian3d::prelude::*;
use bevy::prelude::*;
use earth_two_world::kit::Kit;

use super::{
    Drivable, Seats, WheelOf,
    handling::{GRAVITY, handling},
    headlamps::spawn_headlamps,
    sim::{VehicleInput, VehicleState},
    spec::{self, Spec, center_of_mass, quat},
};
use crate::world::{KitAsset, ParkedVehicle, PieceCollider, PieceModel, Placed, Wheel};

/// The vehicle specs by piece name, from every kit loaded; the game may
/// also insert its own.
#[derive(Resource, Debug, Default)]
pub struct VehicleKit {
    pub specs: BTreeMap<String, Spec>,
}

impl VehicleKit {
    pub fn insert(&mut self, piece: &str, spec: Spec) {
        self.specs.insert(piece.to_string(), spec);
    }

    /// Takes every vehicle piece's spec from `kit`.
    pub fn add_kit(&mut self, kit: &Kit) {
        for (name, piece) in &kit.pieces {
            if let Some(v) = &piece.vehicle {
                self.specs.insert(name.clone(), v.clone());
            }
        }
    }
}

/// A parked vehicle's spec, on its root, for one spawned straight from a
/// spec rather than a placed piece.
#[derive(Component, Debug, Clone)]
pub struct ParkedSpec(pub Spec);

/// Why a parked piece couldn't be made drivable.
#[derive(Component, Debug, Clone)]
pub struct Undrivable(pub String);

/// Collects the vehicle specs of every kit that loads.
pub fn collect_kit_specs(
    mut events: MessageReader<AssetEvent<KitAsset>>,
    kits: Res<Assets<KitAsset>>,
    mut vehicles: ResMut<VehicleKit>,
) {
    for event in events.read() {
        if let AssetEvent::Added { id } | AssetEvent::Modified { id } = event
            && let Some(kit) = kits.get(*id)
        {
            vehicles.add_kit(&kit.0);
        }
    }
}

/// Parks a drivable vehicle as the world module would place its piece:
/// `name` built as `spec`, at `at` turned by `turn`, Static until someone
/// gets in. The models are left to the viewer.
pub fn spawn_parked(
    commands: &mut Commands,
    name: &str,
    spec: &Spec,
    at: Vec3,
    turn: Quat,
) -> Entity {
    let root = commands
        .spawn((
            Name::new(name.to_string()),
            Placed {
                piece: name.to_string(),
                index: 0,
            },
            PieceModel::default(),
            ParkedVehicle {
                handling: spec.handling.clone(),
            },
            ParkedSpec(spec.clone()),
            Transform::from_translation(at).with_rotation(turn),
            RigidBody::Static,
        ))
        .id();
    commands.entity(root).with_children(|c| {
        for b in &spec.chassis {
            let [x, y, z] = b.size;
            c.spawn((
                Name::new("collider"),
                PieceCollider,
                Transform::from_translation(Vec3::from(b.center)).with_rotation(quat(b.rotation)),
                Collider::cuboid(x, y, z),
            ));
        }
        for w in &spec.wheels {
            c.spawn((
                Name::new(w.piece.clone()),
                Wheel,
                PieceModel::default(),
                spec::parked_wheel(w),
            ));
        }
    });
    root
}

/// The chassis boxes' inertia about `com`, at one density, scaled to
/// `mass`: what Jolt gave the convex hull.
pub fn chassis_inertia(spec: &Spec, mass: f32, com: Vec3) -> Mat3 {
    let volume: f32 = spec
        .chassis
        .iter()
        .map(|b| b.size[0] * b.size[1] * b.size[2])
        .sum();
    let mut total = Mat3::ZERO;
    for b in &spec.chassis {
        let [sx, sy, sz] = b.size;
        let m = if volume > 0.0 {
            mass * sx * sy * sz / volume
        } else {
            mass / spec.chassis.len() as f32
        };
        let local = Mat3::from_diagonal(
            Vec3::new(sy * sy + sz * sz, sx * sx + sz * sz, sx * sx + sy * sy) * (m / 12.0),
        );
        let r = Mat3::from_quat(quat(b.rotation));
        let d = Vec3::from(b.center) - com;
        let shift =
            Mat3::IDENTITY * d.length_squared() - Mat3::from_cols(d * d.x, d * d.y, d * d.z);
        total += r * local * r.transpose() + shift * m;
    }
    total
}

/// Makes each parked piece with a known spec drivable.
#[allow(clippy::type_complexity)]
pub fn assemble(
    mut commands: Commands,
    parked: Query<
        (
            Entity,
            &Placed,
            &ParkedVehicle,
            &Transform,
            Option<&ParkedSpec>,
            Option<&Children>,
        ),
        (Without<Drivable>, Without<Undrivable>),
    >,
    wheels: Query<(), With<Wheel>>,
    colliders: Query<(), With<PieceCollider>>,
    kit: Res<VehicleKit>,
) {
    for (root, placed, parked_vehicle, tr, own, children) in &parked {
        let Some(spec) = own.map(|s| &s.0).or_else(|| kit.specs.get(&placed.piece)) else {
            continue; // its kit hasn't loaded yet
        };
        let name = &placed.piece;
        let handling_name = if parked_vehicle.handling.is_empty() {
            &spec.handling
        } else {
            &parked_vehicle.handling
        };
        let Some(h) = handling(handling_name) else {
            let why = format!("vehicle {name}: no handling {handling_name:?}");
            error!("{why}");
            commands.entity(root).insert(Undrivable(why));
            continue;
        };
        if spec.wheels.is_empty() || spec.chassis.is_empty() || spec.seats.is_empty() {
            let why = format!("vehicle {name} needs wheels, a chassis and a seat");
            error!("{why}");
            commands.entity(root).insert(Undrivable(why));
            continue;
        }
        let com = center_of_mass(spec);
        let vehicle = super::handling::build(spec, &h, com);
        let inertia = chassis_inertia(spec, h.mass, com);
        let home = (tr.translation, tr.rotation);
        commands.entity(root).insert((
            Drivable::new(name, spec.clone(), h.clone(), home),
            super::drive::Controls::default(),
            Seats(spec.seats.clone()),
            vehicle,
            VehicleInput {
                hand_brake: 1.0,
                ..default()
            },
            VehicleState::default(),
            (
                Mass(h.mass),
                NoAutoMass,
                CenterOfMass(com),
                NoAutoCenterOfMass,
                AngularInertia::from_tensor(AngularInertiaTensor::from_mat3_unchecked(inertia)),
                NoAutoAngularInertia,
                Friction::new(0.4),
                SweptCcd::default(),
                CollisionEventsEnabled,
                TransformInterpolation,
                NoTransformEasing,
            ),
        ));
        let mut index = 0;
        for child in children
            .map(|c| c.iter().collect::<Vec<_>>())
            .unwrap_or_default()
        {
            if wheels.contains(child) {
                commands.entity(child).insert(WheelOf { index });
                index += 1;
            }
            if colliders.contains(child) {
                commands.entity(child).insert(Friction::new(0.4));
            }
        }
        commands
            .entity(root)
            .with_children(|c| spawn_headlamps(c, spec));
        const { assert!(GRAVITY > 0.0) };
    }
}
