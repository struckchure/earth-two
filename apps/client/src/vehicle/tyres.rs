//! A parked vehicle's tyres as something to bump into. Port of
//! `vehicle/tyres.go`.

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Drivable, sim::VehicleState};

/// A parked vehicle's tyre: a static cylinder where the wheel is. A
/// vehicle's own body is only its chassis, which clears the ground and so
/// its wheels, for it to ride on them; while it's parked, its tyres stop
/// the player walking through them, and other vehicles driving into them.
/// They go when it's driven, or the wheels' casts for the ground would
/// find them.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tyre {
    pub of: Entity,
}

/// Gives each parked vehicle its tyres, where its wheels came to rest, and
/// takes them away while it's driven.
pub fn tyres(
    mut commands: Commands,
    mut cars: Query<(Entity, &mut Drivable, &RigidBody, &Transform, &VehicleState)>,
    existing: Query<(Entity, &Tyre)>,
) {
    for (e, mut d, rb, root, st) in &mut cars {
        let parked = rb.is_static();
        if parked && !d.tyred {
            let axle = root.rotation * Quat::from_axis_angle(Vec3::Z, std::f32::consts::FRAC_PI_2);
            for (i, w) in d.spec.wheels.iter().enumerate() {
                let at = st
                    .wheels
                    .get(i)
                    .map(|s| s.transform.translation)
                    .unwrap_or(Vec3::from(w.at));
                let centre = root.translation + root.rotation * at;
                commands.spawn((
                    Name::new("tyre"),
                    Tyre { of: e },
                    Transform::from_translation(centre).with_rotation(axle),
                    RigidBody::Static,
                    Collider::cylinder(w.radius, w.width),
                ));
            }
            d.tyred = true;
        } else if !parked && d.tyred {
            for (t, tyre) in &existing {
                if tyre.of == e {
                    commands.entity(t).despawn();
                }
            }
            d.tyred = false;
        }
    }
}
