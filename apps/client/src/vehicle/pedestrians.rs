//! Pedestrians near a moving car. Port of `vehicle/pedestrians.go`: crowd
//! LOD may skip character updates, leaving a stationary capsule between
//! them. Near a moving car that capsule can stop the car before the
//! character controller has a chance to respond and move aside, so nearby
//! pedestrians are stepped every tick, then their LOD restored.

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Drivable, spec::chassis_radius};
use crate::character::CharacterController;

#[derive(Debug, Clone, Copy)]
pub struct MovingChassis {
    pub at: Vec3,
    pub velocity: Vec3,
    pub radius: f32,
}

/// The intervals overridden this step, to restore.
#[derive(Resource, Default)]
pub struct PedestrianPhysics {
    every: HashMap<Entity, u32>,
    cars: Vec<MovingChassis>,
}

pub fn prepare_pedestrians(
    mut people: Query<(Entity, &mut CharacterController, &Transform)>,
    cars: Query<(&Drivable, &RigidBody, &Position, &LinearVelocity)>,
    mut state: ResMut<PedestrianPhysics>,
    time: Res<Time>,
) {
    let s = &mut *state;
    s.cars.clear();
    for (car, body, pos, velocity) in &cars {
        if !body.is_dynamic() {
            continue;
        }
        s.cars.push(MovingChassis {
            at: pos.0,
            velocity: velocity.0,
            radius: chassis_radius(Some(&car.spec)),
        });
    }
    if s.cars.is_empty() {
        return;
    }
    let dt = time.delta_secs();
    for (e, mut cc, tr) in &mut people {
        if cc.every <= 1 {
            continue;
        }
        if s.cars
            .iter()
            .any(|car| pedestrian_in_reach(*car, tr.translation, &cc, dt))
        {
            s.every.insert(e, cc.every);
            cc.every = 1;
        }
    }
}

/// Whether a car could reach a pedestrian at `at` before its next update.
pub fn pedestrian_in_reach(
    car: MovingChassis,
    at: Vec3,
    cc: &CharacterController,
    dt: f32,
) -> bool {
    // Look ahead through the pedestrian's entire normal update interval,
    // plus one tick. This catches fast approaches and reversing too.
    let delta = car.velocity * (dt.max(0.0) * (cc.every + 1) as f32);
    let to = at - car.at;
    let mut t = 0.0;
    let length = delta.length_squared();
    if length > 0.0 {
        t = (to.dot(delta) / length).clamp(0.0, 1.0);
    }
    let separation = to - delta * t;
    // Two metres of clearance wakes the controller before actual contact.
    let reach = car.radius + (cc.height / 2.0).max(cc.radius) + 2.0;
    separation.length_squared() <= reach * reach
}

pub fn restore_pedestrian_steps(
    mut people: Query<&mut CharacterController>,
    mut state: ResMut<PedestrianPhysics>,
) {
    for (e, every) in state.every.drain() {
        if let Ok(mut cc) = people.get_mut(e) {
            cc.every = every;
        }
    }
}

/// Port of illusion's ContactBuffer::characterResponse. Only a vehicle/person
/// pair makes the vehicle effectively infinite-mass; walls, props and other
/// vehicles retain the normal two-body response. Apply this to prepared
/// constraints, including their effective masses, before any warm starting.
#[allow(clippy::type_complexity)]
pub fn character_response(
    mut graph: ResMut<avian3d::dynamics::solver::constraint_graph::ConstraintGraph>,
    bodies: Res<avian3d::dynamics::solver::solver_body::SolverBodies>,
    softness: Res<avian3d::dynamics::solver::ContactSoftnessCoefficients>,
    cars: Query<(), With<Drivable>>,
    people: Query<
        (),
        Or<(
            With<CharacterController>,
            With<crate::character::CharacterBody>,
        )>,
    >,
) {
    use avian3d::{
        dynamics::solver::contact::{ContactNormalPart, ContactTangentPart},
        math::SymmetricTensor,
    };
    for color in &mut graph.colors {
        for c in &mut color.contact_constraints {
            let (Some(a), Some(b)) = (
                bodies.get_entity(c.body_index1),
                bodies.get_entity(c.body_index2),
            ) else {
                continue;
            };
            let first = cars.contains(a) && people.contains(b);
            let second = cars.contains(b) && people.contains(a);
            if !first && !second {
                continue;
            }
            let person = if first { c.body_index2 } else { c.body_index1 };
            let Some(inertia) = bodies.get_inertia(person) else {
                continue;
            };
            let inv_mass = inertia.effective_inv_mass();
            let inv_inertia = inertia.effective_inv_angular_inertia();
            let (i1, i2) = if first {
                (SymmetricTensor::ZERO, inv_inertia)
            } else {
                (inv_inertia, SymmetricTensor::ZERO)
            };
            c.relative_dominance = if first { 1 } else { -1 };
            let tangents = [c.tangent1, (-c.normal).cross(c.tangent1)];
            for p in &mut c.points {
                p.normal_part = ContactNormalPart::generate(
                    inv_mass,
                    &i1,
                    &i2,
                    p.anchor1,
                    p.anchor2,
                    c.normal,
                    Some(p.normal_part.impulse),
                    softness.non_dynamic,
                );
                if let Some(tangent) = &mut p.tangent_part {
                    *tangent = ContactTangentPart::generate(
                        inv_mass,
                        &i1,
                        &i2,
                        p.anchor1,
                        p.anchor2,
                        tangents,
                        Some(tangent.impulse),
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_looks_ahead_along_the_car_s_motion() {
        let car = MovingChassis {
            at: Vec3::ZERO,
            velocity: Vec3::Z * 10.0,
            radius: 3.0,
        };
        let cc = CharacterController {
            radius: 0.3,
            height: 1.8,
            every: 8,
            ..default()
        };
        let dt = 1.0 / 60.0;
        assert!(pedestrian_in_reach(car, Vec3::new(4.0, 0.0, 0.0), &cc, dt));
        // Ahead, where the car gets in nine ticks.
        assert!(pedestrian_in_reach(car, Vec3::new(0.0, 0.0, 6.0), &cc, dt));
        assert!(!pedestrian_in_reach(
            car,
            Vec3::new(50.0, 0.0, 0.0),
            &cc,
            dt
        ));
        // Behind: not where it's going.
        assert!(!pedestrian_in_reach(
            car,
            Vec3::new(0.0, 0.0, -7.0),
            &cc,
            dt
        ));
    }
}
