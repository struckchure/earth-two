use super::*;
use crate::{
    character::{Character, CharacterPlugin, Health, LifeState, knock_down},
    physics::EarthPhysicsPlugin,
};
use bevy::{asset::AssetPlugin, time::TimeUpdateStrategy};
use std::time::Duration;

fn skeleton() -> (Vec<BoneInfo>, Vec<BoneTransform>) {
    let names = [
        "upperarm_l",
        "lowerarm_l",
        "hand_l",
        "middle_03_l",
        "neck_01",
        "head",
    ];
    let positions = [
        Vec3::new(0., 1.4, 0.),
        Vec3::new(0.35, 1.35, 0.),
        Vec3::new(0.7, 1.3, 0.),
        Vec3::new(0.8, 1.3, 0.),
        Vec3::new(0., 1.5, 0.),
        Vec3::new(0., 1.68, 0.),
    ];
    (
        names
            .iter()
            .enumerate()
            .map(|(i, n)| BoneInfo {
                name: (*n).into(),
                parent: if i == 4 { -1 } else { i as i32 - 1 },
            })
            .collect(),
        positions
            .iter()
            .map(|p| BoneTransform {
                translation: *p,
                ..default()
            })
            .collect(),
    )
}
#[test]
fn gravity_folds_limbs_without_stretching_the_skeleton() {
    let (bones, pose) = skeleton();
    for yaw in [0., 0.7, 2.4] {
        let root = Transform::from_xyz(1., 2., 3.).with_rotation(Quat::from_rotation_y(yaw));
        let mut rig = RagdollRig::new(&bones, &pose, root.to_matrix(), root, Vec3::ZERO).unwrap();
        let before = rig.points[3].position.y;
        for _ in 0..120 {
            rig.step(root, Vec3::NEG_Y * 9.81, 1. / 60., &|_, _, _| None);
        }
        assert!(rig.points[3].position.y < before - 0.15);
        for link in &rig.links {
            let d = rig.points[link.a]
                .position
                .distance(rig.points[link.b].position);
            assert!(
                d >= link.min - 0.008 && d <= link.max + 0.008,
                "{d} {link:?}"
            );
        }
        let result = rig.pose(root.to_matrix(), 0.5);
        for link in rig.links.iter().filter(|l| l.bone) {
            let (a, b) = (rig.points[link.a].bone, rig.points[link.b].bone);
            assert!(
                (result[a].translation.distance(result[b].translation)
                    - pose[a].translation.distance(pose[b].translation))
                .abs()
                    < 0.0001
            );
        }
    }
}
#[test]
fn fast_limbs_sweep_ground_and_thin_walls() {
    let (bones, pose) = skeleton();
    let mut rig = RagdollRig::new(
        &bones,
        &pose,
        Mat4::IDENTITY,
        Transform::IDENTITY,
        Vec3::new(0., -15., 25.),
    )
    .unwrap();
    let sweep = |from: Vec3, delta: Vec3, radius: f32| {
        let (mut fraction, mut normal) = (2.0_f32, Vec3::ZERO);
        if delta.y < 0. && from.y + delta.y < radius {
            fraction = ((radius - from.y) / delta.y).max(0.);
            normal = Vec3::Y;
        }
        if delta.z > 0. && from.z + delta.z > 0.15 - radius {
            let f = ((0.15 - radius - from.z) / delta.z).max(0.);
            if f < fraction {
                fraction = f;
                normal = Vec3::NEG_Z;
            }
        }
        (fraction <= 1.).then_some(RayHit {
            entity: Entity::PLACEHOLDER,
            point: from + delta * fraction,
            distance: delta.length() * fraction,
            normal,
        })
    };
    for _ in 0..300 {
        rig.step(Transform::IDENTITY, Vec3::NEG_Y * 9.81, 1. / 60., &sweep);
        for p in rig.points.iter().filter(|p| p.weight > 0.) {
            assert!(
                p.position.is_finite()
                    && p.position.y >= p.radius - 0.001
                    && p.position.z <= 0.15 - p.radius + 0.001,
                "{p:?}"
            );
        }
    }
}
fn fallen(slope: f32, start: Vec3, velocity: Vec3) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        TransformPlugin,
        AssetPlugin::default(),
        EarthPhysicsPlugin,
        CharacterPlugin,
    ))
    .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_nanos(
        16_666_667,
    )));
    let rotation = Quat::from_rotation_z(slope.to_radians());
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(100., 1., 100.),
        Transform::from_translation(rotation * (Vec3::NEG_Y * 0.5)).with_rotation(rotation),
    ));
    let e = app.world_mut().spawn(Character::default()).id();
    knock_down(
        &mut app.world_mut().commands().entity(e),
        Transform::from_translation(start),
        Health {
            state: LifeState::Dead,
            ..default()
        },
        velocity,
    );
    app.world_mut().flush();
    app.finish();
    app.cleanup();
    app.update(); // Bevy initializes its clock before the first fixed tick.
    (app, e)
}
fn tick(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}
#[test]
fn fallen_bodies_settle_on_gentle_slopes_and_wake_on_a_new_impact() {
    for slope in [0., 5., 20.] {
        for speed in [0., 12.] {
            let (mut app, e) = fallen(slope, Vec3::Y * 0.9, Vec3::Z * speed);
            tick(&mut app, 360);
            let before = *app.world().get::<Transform>(e).unwrap();
            assert!(
                app.world().get::<Sleeping>(e).is_some(),
                "slope {slope} speed {speed}: {:?} {:?}",
                app.world().get::<LinearVelocity>(e),
                app.world().get::<AngularVelocity>(e)
            );
            tick(&mut app, 300);
            let after = *app.world().get::<Transform>(e).unwrap();
            assert!(
                before.translation.distance(after.translation) < 0.02
                    && (before.rotation * Vec3::Y).distance(after.rotation * Vec3::Y) < 0.02
            );
            app.world_mut()
                .entity_mut(e)
                .remove::<Sleeping>()
                .insert(LinearVelocity(Vec3::new(0., 2., 4.)));
            tick(&mut app, 30);
            assert!(
                app.world()
                    .get::<Transform>(e)
                    .unwrap()
                    .translation
                    .distance(after.translation)
                    > 0.2
            );
        }
    }
}
#[test]
fn airborne_and_steep_slope_motion_is_not_pinned() {
    let (mut app, e) = fallen(0., Vec3::Y * 20., Vec3::Z * 3.);
    tick(&mut app, 60);
    let v = app.world().get::<LinearVelocity>(e).unwrap().0;
    assert!(
        v.z > 1.8 && v.y < -8. && app.world().get::<AngularVelocity>(e).unwrap().0.length() > 0.3,
        "{v:?}"
    );
    let (mut app, e) = fallen(45., Vec3::Y * 0.9, Vec3::ZERO);
    tick(&mut app, 180);
    assert!(app.world().get::<Sleeping>(e).is_none());
    assert!(app.world().get::<Transform>(e).unwrap().translation.x < -1.);
}
#[test]
fn paused_and_settled_limbs_hold_then_wake_with_the_torso() {
    let (mut app, e) = fallen(5., Vec3::Y * 0.9, Vec3::Z * 4.);
    tick(&mut app, 1);
    let (bones, pose) = skeleton();
    let at = *app.world().get::<Transform>(e).unwrap();
    app.world_mut().get_mut::<Ragdoll>(e).unwrap().rig = RagdollRig::new(
        &bones,
        &pose,
        at.to_matrix() * Mat4::from_translation(Vec3::NEG_Y * 0.9),
        at,
        Vec3::Z * 4.,
    );
    tick(&mut app, 30);
    crate::physics::set_paused(&mut app.world_mut().resource_mut::<Time<Physics>>(), true);
    let before = app
        .world()
        .get::<Ragdoll>(e)
        .unwrap()
        .rig
        .as_ref()
        .unwrap()
        .points[1]
        .position;
    tick(&mut app, 20);
    assert_eq!(
        before,
        app.world()
            .get::<Ragdoll>(e)
            .unwrap()
            .rig
            .as_ref()
            .unwrap()
            .points[1]
            .position
    );
    crate::physics::set_paused(&mut app.world_mut().resource_mut::<Time<Physics>>(), false);
    tick(&mut app, 720);
    let ragdoll = app.world().get::<Ragdoll>(e).unwrap();
    assert!(
        app.world().get::<Sleeping>(e).is_some() && ragdoll.quiet >= 0.35,
        "quiet {}",
        ragdoll.quiet
    );
    let before: Vec<_> = ragdoll
        .rig
        .as_ref()
        .unwrap()
        .points
        .iter()
        .map(|p| p.position)
        .collect();
    tick(&mut app, 120);
    assert_eq!(
        before,
        app.world()
            .get::<Ragdoll>(e)
            .unwrap()
            .rig
            .as_ref()
            .unwrap()
            .points
            .iter()
            .map(|p| p.position)
            .collect::<Vec<_>>()
    );
    app.world_mut()
        .entity_mut(e)
        .remove::<Sleeping>()
        .insert(LinearVelocity(Vec3::new(0., 2., 4.)));
    tick(&mut app, 20);
    let ragdoll = app.world().get::<Ragdoll>(e).unwrap();
    assert!(
        ragdoll.quiet < 0.35
            && ragdoll.rig.as_ref().unwrap().points[0]
                .position
                .distance(before[0])
                > 0.1
    );
}
