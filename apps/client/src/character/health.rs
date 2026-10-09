//! Health and going down: a standing controller swapped for a finite-mass
//! torso, and back. Port of `character/health.go`'s physics half; the
//! ragdoll rig and its posing belong to the ragdoll module.

use avian3d::prelude::*;
use bevy::{ecs::system::EntityCommands, prelude::*};

use super::{
    CAPSULE_HEIGHT, CAPSULE_RADIUS, CharacterController, ControllerState, Intent, Traversal,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LifeState {
    #[default]
    Healthy,
    Critical,
    Dead,
}

/// Shared by players and NPCs. `impact_speed` is the closing speed at
/// impact, before the physics solver changes either body's velocity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Health {
    pub state: LifeState,
    pub impact_speed: f32,
    pub vehicle: Option<Entity>,
}

/// Marks a fallen, finite-mass body, without a standing controller.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct Downed;

/// Marks a character's rigid body (a downed torso), for impacts to tell
/// from props.
#[derive(Component, Clone, Copy, Debug, Default)]
pub struct CharacterBody;

pub const RAGDOLL_RADIUS: f32 = 0.20;
pub const RAGDOLL_HEIGHT: f32 = 0.65;
pub const RAGDOLL_OFFSET: f32 = 0.12;

/// The spin a strike gives a falling body: a strike low on the legs sweeps
/// them ahead of the upper body (the Hit On Legs reference), making the
/// torso fall back into the impact.
pub fn knock_down_spin(velocity: Vec3) -> Vec3 {
    let mut spin = Vec3::Y.cross(velocity) * -0.65;
    let speed = spin.length();
    if speed > 5.0 {
        spin *= 5.0 / speed;
    }
    if spin.length_squared() < 0.01 {
        spin = Vec3::new(0.8, 0.0, 0.3);
    }
    spin
}

/// Replaces the standing controller with a finite-mass torso. Keep the
/// current orientation and animation pose; gravity and the impact make the
/// body fall rather than snapping it sideways. The ragdoll module adds its
/// rig (with `velocity`) and removes the seat; this only does the physics.
pub fn knock_down(root: &mut EntityCommands, at: Transform, health: Health, velocity: Vec3) {
    let spin = knock_down_spin(velocity);
    root.remove::<(CharacterController, ControllerState)>()
        .insert((
            health,
            Downed,
            at,
            Intent::default(),
            Traversal::default(),
            RigidBody::Dynamic,
            Collider::compound(vec![(
                Vec3::new(0.0, RAGDOLL_OFFSET, 0.0),
                Quat::IDENTITY,
                Collider::capsule(RAGDOLL_RADIUS, RAGDOLL_HEIGHT - 2.0 * RAGDOLL_RADIUS),
            )]),
            CharacterBody,
            Mass(70.0),
            Friction::new(0.6),
            LinearDamping(0.4),
            AngularDamping(0.8),
            SweptCcd::default(),
            LinearVelocity(velocity),
            AngularVelocity(spin),
        ));
}

/// Prototype recovery: restore a healthy character at safe feet.
pub fn revive(commands: &mut Commands, entity: Entity, feet: Vec3) {
    let center = feet + Vec3::Y * (CAPSULE_HEIGHT / 2.0);
    commands
        .entity(entity)
        .remove::<(
            Downed,
            RigidBody,
            Collider,
            CharacterBody,
            Mass,
            Friction,
            LinearDamping,
            AngularDamping,
            SweptCcd,
            LinearVelocity,
            AngularVelocity,
        )>()
        .insert((
            Health::default(),
            Intent::default(),
            Traversal::default(),
            CharacterController::standing(CAPSULE_RADIUS, CAPSULE_HEIGHT, 0.3),
            Transform::from_translation(center),
        ));
}

/// A downed character wants nothing.
pub fn stop_downed(mut q: Query<&mut Intent, With<Downed>>) {
    for mut intent in &mut q {
        *intent = Intent::default();
    }
}
