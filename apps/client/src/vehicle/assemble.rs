//! Turns a parked piece into a drivable vehicle: `vehicle.Spawn` in Go. The
//! world module places a vehicle piece as a Static root with its chassis
//! boxes and wheel models as children ([`ParkedVehicle`]); this replaces the
//! boxes with Go's single convex chassis and adds the driving components.

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

/// Inertia of the same convex hull used by Go, shifted to the authored COM.
/// Summing overlapping boxes changes both the mass distribution and handling.
pub fn chassis_inertia(spec: &Spec, mass: f32, com: Vec3) -> Mat3 {
    let hull = Collider::convex_hull(&spec::corners(spec)).expect("validated chassis hull");
    let props = hull.mass_properties(1.0);
    let frame = Mat3::from_quat(props.local_inertial_frame);
    let local = frame * Mat3::from_diagonal(props.principal_angular_inertia) * frame.transpose();
    let d = props.center_of_mass - com;
    let shift = Mat3::IDENTITY * d.length_squared() - Mat3::from_cols(d * d.x, d * d.y, d * d.z);
    local * (mass / props.mass) + shift * mass
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
        let Some(hull) = Collider::convex_hull(&spec::corners(spec)) else {
            commands
                .entity(root)
                .insert(Undrivable(format!("vehicle {name}: invalid chassis hull")));
            continue;
        };
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
                LinearDamping(0.05),
                AngularDamping(0.05),
                SweptCcd::default(),
                CollisionEventsEnabled,
                TransformInterpolation,
                NoTransformEasing,
            ),
        ));
        commands.entity(root).with_children(|c| {
            c.spawn((
                Name::new("chassis"),
                PieceCollider,
                Transform::IDENTITY,
                hull,
                Friction::new(0.4),
            ));
        });
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
                commands.entity(child).remove::<Collider>();
            }
        }
        commands
            .entity(root)
            .with_children(|c| spawn_headlamps(c, spec));
        const { assert!(GRAVITY > 0.0) };
    }
}
