//! BSN composes entities; systems own simulation behavior.

use avian3d::prelude::*;
use bevy::prelude::*;

#[derive(Component, Default, Clone)]
pub struct Probe;

#[derive(Component, Default, Clone)]
pub struct Ground;

pub fn foundation_scene() -> impl SceneList {
    bsn_list! {
        Name::new("Ground")
        Ground
        Transform::from_xyz(0.0, -0.5, 0.0)
        RigidBody::Static
        Collider::cuboid(20.0, 1.0, 20.0)
        --
        Name::new("Physics probe")
        Probe
        Transform::from_xyz(0.0, 3.0, 0.0)
        RigidBody::Dynamic
        Collider::sphere(0.5)
        TransformInterpolation
    }
}
