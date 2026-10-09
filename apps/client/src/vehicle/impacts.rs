//! Vehicles running people over. Port of `vehicle/impacts.go`: a sweep of
//! the standing capsule against the chassis predicts the hit before the
//! solver sees an upright infinite-mass character; contact events catch
//! fallen bodies and other vehicles.

use std::collections::HashMap;

use avian3d::prelude::*;
use bevy::prelude::*;

use super::{Drivable, drive::Driving, drive::Prompt, spec::chassis_radius};
use crate::character::{self, CharacterController, Health, LifeState};

/// Gameplay thresholds in metres per second. Defaults: critical at 10
/// km/h, instant death at 30 km/h of closing speed.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct ImpactRules {
    pub critical_speed: f32,
    pub lethal_speed: f32,
}

impl Default for ImpactRules {
    fn default() -> Self {
        ImpactRules {
            critical_speed: 10.0 / 3.6,
            lethal_speed: 30.0 / 3.6,
        }
    }
}

impl ImpactRules {
    pub fn condition(&self, speed: f32) -> LifeState {
        if speed >= self.lethal_speed {
            LifeState::Dead
        } else if speed >= self.critical_speed {
            LifeState::Critical
        } else {
            LifeState::Healthy
        }
    }
}

/// How bad a state is, for never making one better by a lesser hit.
fn rank(state: LifeState) -> u8 {
    match state {
        LifeState::Healthy => 0,
        LifeState::Critical => 1,
        LifeState::Dead => 2,
    }
}

#[derive(Debug, Clone, Copy)]
struct ImpactCar {
    at: Vec3,
    velocity: Vec3,
    angular: Vec3,
    driver: Option<Entity>,
    radius: f32,
}

impl ImpactCar {
    fn velocity_at(&self, at: Vec3) -> Vec3 {
        self.velocity + self.angular.cross(at - self.at)
    }
}

#[derive(Debug, Clone, Copy)]
struct VehicleImpact {
    vehicle: Entity,
    speed: f32,
    velocity: Vec3,
    at: Transform,
}

/// What the step's vehicles and people were doing before the solver, for
/// the contact fallback.
#[derive(Resource, Default)]
pub struct ImpactMemory {
    cars: HashMap<Entity, ImpactCar>,
    people: HashMap<Entity, Vec3>,
    hits: HashMap<Entity, VehicleImpact>,
}

/// The people a vehicle can hit.
#[derive(bevy::ecs::system::SystemParam)]
pub struct ImpactPeople<'w, 's> {
    pub roots: Query<'w, 's, (Entity, &'static Transform), With<character::Character>>,
    pub controllers: Query<'w, 's, &'static CharacterController>,
    pub health: Query<'w, 's, &'static Health>,
    pub velocities: Query<'w, 's, &'static LinearVelocity>,
}

/// Predicts actual capsule/chassis contact before the solver sees an
/// upright infinite-mass character: a shape sweep, not a proximity damage
/// radius.
#[allow(clippy::too_many_arguments)]
pub fn run_over(
    mut commands: Commands,
    cars: Query<(
        Entity,
        &Drivable,
        &RigidBody,
        &Position,
        &LinearVelocity,
        &AngularVelocity,
    )>,
    people: ImpactPeople,
    physics: character::CharacterPhysics,
    rules: Res<ImpactRules>,
    mut state: ResMut<ImpactMemory>,
    driving: Res<Driving>,
    mut prompt: ResMut<Prompt>,
    physics_time: Res<Time<Physics>>,
    time: Res<Time>,
) {
    let s = &mut *state;
    s.cars.clear();
    s.people.clear();
    s.hits.clear();
    if physics_time.is_paused() {
        return;
    }
    for (e, car, body, pos, velocity, angular) in &cars {
        if body.is_dynamic() {
            s.cars.insert(
                e,
                ImpactCar {
                    at: pos.0,
                    velocity: velocity.0,
                    angular: angular.0,
                    driver: car.driver,
                    radius: chassis_radius(Some(&car.spec)),
                },
            );
        }
    }
    if s.cars.is_empty() {
        return;
    }
    let drivers: Vec<Entity> = s.cars.values().filter_map(|c| c.driver).collect();
    let dt = time.delta_secs();
    for (e, tr) in &people.roots {
        let health = people.health.get(e).copied().unwrap_or_default();
        if health.state == LifeState::Dead || drivers.contains(&e) {
            continue;
        }
        let cc = people.controllers.get(e).ok();
        let velocity = match cc {
            Some(cc) => cc.velocity,
            None => people.velocities.get(e).map(|v| v.0).unwrap_or(Vec3::ZERO),
        };
        s.people.insert(e, velocity);
        // Fallen bodies use genuine contact events below.
        let Some(cc) = cc else {
            continue;
        };
        let radius = if cc.radius <= 0.0 { 0.3 } else { cc.radius };
        let height = cc.height.max(2.0 * radius + 0.01);
        for (&vehicle, car) in &s.cars {
            if car.driver == Some(e) {
                continue;
            }
            let motion = car.velocity_at(tr.translation);
            if motion.length() < rules.critical_speed {
                continue;
            }
            let relative = motion - velocity;
            let distance = relative.length();
            if distance < rules.critical_speed {
                continue;
            }
            // A cheap bound avoids shape queries across the whole world.
            let reach = car.radius + height / 2.0 + distance * dt + 0.05;
            if car.at.distance_squared(tr.translation) > reach * reach {
                continue;
            }
            let delta = relative * -(dt + 0.04 / distance.max(0.001));
            // Raised by a small skin so the standing floor doesn't mask the
            // chassis.
            let origin = tr.translation + Vec3::Y * 0.025;
            let Some(hit) = physics.sweep_capsule_excluding(origin, delta, radius, height, e)
            else {
                continue;
            };
            let hit_body = physics.body_of(hit.entity);
            if hit_body != vehicle {
                continue;
            }
            let speed = relative.dot(hit.normal).max(0.0);
            if rules.condition(speed) == LifeState::Healthy {
                continue;
            }
            if s.hits.get(&e).is_some_and(|old| old.speed >= speed) {
                continue;
            }
            s.hits.insert(
                e,
                VehicleImpact {
                    vehicle,
                    speed,
                    velocity: motion,
                    at: *tr,
                },
            );
        }
    }
    let hits: Vec<(Entity, VehicleImpact)> = s.hits.iter().map(|(e, h)| (*e, *h)).collect();
    for (e, hit) in hits {
        apply_vehicle_impact(
            &mut commands,
            &people,
            &rules,
            &driving,
            &mut prompt,
            e,
            hit,
        );
    }
}

fn apply_vehicle_impact(
    commands: &mut Commands,
    people: &ImpactPeople,
    rules: &ImpactRules,
    driving: &Driving,
    prompt: &mut Prompt,
    e: Entity,
    hit: VehicleImpact,
) {
    let status = rules.condition(hit.speed);
    if status == LifeState::Healthy {
        return;
    }
    if people
        .health
        .get(e)
        .is_ok_and(|h| rank(h.state) >= rank(status))
    {
        return;
    }
    let health = Health {
        state: status,
        impact_speed: hit.speed,
        vehicle: Some(hit.vehicle),
    };
    if driving.vehicle == Some(hit.vehicle) {
        prompt.note(if status == LifeState::Dead {
            "Fatal vehicle impact"
        } else {
            "Character critically injured"
        });
    }
    if people.controllers.contains(e) {
        // Carry some impact momentum into the finite-mass fallen body.
        let mut velocity = hit.velocity * 0.6;
        velocity.y += (hit.speed * 0.15).min(3.0);
        character::knock_down(&mut commands.entity(e), hit.at, health, velocity);
    } else {
        commands.entity(e).insert(health);
    }
}

/// The contact fallback, also for another vehicle running over a critical
/// character. Speeds come from before the solver, not its post-impact
/// result.
#[allow(clippy::too_many_arguments)]
pub fn impact_contacts(
    mut commands: Commands,
    mut events: MessageReader<CollisionStart>,
    collisions: Collisions,
    people: ImpactPeople,
    state: Res<ImpactMemory>,
    rules: Res<ImpactRules>,
    driving: Res<Driving>,
    mut prompt: ResMut<Prompt>,
) {
    let s = &*state;
    for event in events.read() {
        let (a, b) = (
            event.body1.unwrap_or(event.collider1),
            event.body2.unwrap_or(event.collider2),
        );
        let Some(pair) = collisions.get(event.collider1, event.collider2) else {
            continue;
        };
        let Some(manifold) = pair.manifolds.first() else {
            continue;
        };
        let Some(point) = manifold.points.first() else {
            continue;
        };
        let (vehicle, actor, normal) = if s.cars.contains_key(&a) {
            (a, b, manifold.normal)
        } else if s.cars.contains_key(&b) {
            (b, a, -manifold.normal)
        } else {
            continue;
        };
        let car = s.cars[&vehicle];
        if car.driver == Some(actor) {
            continue;
        }
        let (Some(&velocity), Ok((_, tr))) = (s.people.get(&actor), people.roots.get(actor)) else {
            continue;
        };
        let motion = car.velocity_at(point.point);
        if motion.length() < rules.critical_speed {
            continue;
        }
        let speed = (motion - velocity).dot(normal).max(0.0);
        apply_vehicle_impact(
            &mut commands,
            &people,
            &rules,
            &driving,
            &mut prompt,
            actor,
            VehicleImpact {
                vehicle,
                speed,
                velocity: motion,
                at: *tr,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `TestImpactThresholds`.
    #[test]
    fn impact_thresholds() {
        let r = ImpactRules::default();
        for (speed, state) in [
            (0.0, LifeState::Healthy),
            (r.critical_speed - 0.001, LifeState::Healthy),
            (r.critical_speed, LifeState::Critical),
            (r.lethal_speed - 0.001, LifeState::Critical),
            (r.lethal_speed, LifeState::Dead),
            (20.0, LifeState::Dead),
        ] {
            assert_eq!(r.condition(speed), state, "speed {speed}");
        }
    }
}
