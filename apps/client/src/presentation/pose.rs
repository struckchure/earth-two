//! Live Bevy joint adapter for character/ride.go and contacts.go. Read the
//! blended animation before transform propagation, then share the result with
//! body skinning, garments, outlines and desktop cloth.
use super::{
    Roster,
    distant::Distant,
    ride::RideRig,
    viewer::{MeshEntities, ModelScene},
};
use crate::{
    character::{
        Anim, Body, CharacterController, CharacterPhysics, Controls, State, Traversal,
        contacts::{self, BoneInfo, BoneTransform, ContactRig},
    },
    game::seats::Seated,
    vehicle::{Drivable, Seats},
    world::Ladder,
};
use avian3d::prelude::Physics;
use bevy::{
    app::AnimationSystems,
    mesh::skinning::SkinnedMesh,
    prelude::*,
    transform::{TransformSystems, helper::TransformHelper},
};

pub struct PosePlugin;
impl Plugin for PosePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, bind_rigs.after(super::PresentationSystems::Models))
            .add_systems(
                PostUpdate,
                restore
                    .before(AnimationSystems)
                    .before(super::PresentationSystems::Advance),
            )
            .add_systems(
                PostUpdate,
                fit.after(AnimationSystems)
                    .before(TransformSystems::Propagate),
            );
    }
}

#[derive(Component)]
struct LiveRig {
    scene: Entity,
    joints: Vec<Entity>,
    bones: Vec<BoneInfo>,
    contact: ContactRig,
    ride: RideRig,
    /// Restore authored local transforms before sampling the next animation.
    originals: Vec<Transform>,
    last_pose: Option<Vec<BoneTransform>>,
}

/// Runtime evidence for the graphical regression (also useful when inspecting ECS).
#[derive(Component, Default, Debug)]
pub struct PoseFit {
    pub riding: bool,
    pub downed: bool,
    pub contacts: bool,
    pub frames: u64,
}

fn bind_rigs(
    mut commands: Commands,
    bodies: Query<(Entity, &ModelScene, &MeshEntities, Option<&LiveRig>), With<Body>>,
    skins: Query<&SkinnedMesh>,
    names: Query<&Name>,
    parents: Query<&ChildOf>,
) {
    for (entity, scene, meshes, old) in &bodies {
        if old.is_some_and(|r| r.scene == scene.root) {
            continue;
        }
        let Some(skin) = meshes.0.iter().flatten().find_map(|e| skins.get(*e).ok()) else {
            continue;
        };
        let bones: Option<Vec<_>> = skin
            .joints
            .iter()
            .map(|e| {
                let parent = parents
                    .get(*e)
                    .ok()
                    .and_then(|p| skin.joints.iter().position(|j| *j == p.parent()));
                Some(BoneInfo {
                    name: names.get(*e).ok()?.as_str().into(),
                    parent: parent.map_or(-1, |i| i as i32),
                })
            })
            .collect();
        let Some(bones) = bones else { continue };
        let feet = ["foot_l", "foot_r"].map(|name| {
            bones
                .iter()
                .position(|b| b.name == name)
                .map(|i| skin.joints[i])
        });
        if let [Some(left), Some(right)] = feet {
            commands
                .entity(entity)
                .insert(crate::game::cues::AudibleFeet([left, right]));
        } else {
            commands
                .entity(entity)
                .remove::<crate::game::cues::AudibleFeet>();
        }
        commands.entity(entity).insert((
            LiveRig {
                scene: scene.root,
                joints: skin.joints.clone(),
                contact: contacts::make_contact_rig(&bones),
                ride: RideRig::new(&bones),
                bones,
                originals: vec![],
                last_pose: None,
            },
            PoseFit::default(),
        ));
    }
}

fn restore(mut rigs: Query<&mut LiveRig>, mut transforms: Query<&mut Transform>) {
    for mut rig in &mut rigs {
        for (joint, original) in rig.joints.iter().zip(&rig.originals) {
            if let Ok(mut tr) = transforms.get_mut(*joint) {
                *tr = *original;
            }
        }
        rig.originals.clear();
    }
}

#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn fit(
    mut bodies: Query<(&ChildOf, &State, &mut LiveRig, &mut PoseFit, Has<Distant>), With<Body>>,
    mut roots: Query<(
        Option<&CharacterController>,
        Option<&Traversal>,
        Option<&Seated>,
        Option<&mut crate::character::Ragdoll>,
    )>,
    cars: Query<&Seats, With<Drivable>>,
    ladders: Query<&Ladder>,
    parents: Query<&ChildOf>,
    mut transforms: ParamSet<(TransformHelper, Query<&mut Transform>)>,
    physics: CharacterPhysics,
    roster: Res<Roster>,
    controls: Res<Controls>,
    clock: Res<Time>,
    fixed: Res<Time<Fixed>>,
    physics_clock: Option<Res<Time<Physics>>>,
) {
    for (parent, state, mut rig, mut result, distant) in &mut bodies {
        let root = parent.parent();
        let Ok((controller, traversal, seated, ragdoll)) = roots.get_mut(root) else {
            continue;
        };
        result.riding = false;
        result.downed = false;
        result.contacts = false;
        let helper = transforms.p0();
        let Ok(model) = helper.compute_global_transform(rig.scene) else {
            continue;
        };
        let matrix = model.to_matrix();
        let inverse = matrix.inverse();
        let sampled: Option<Vec<BoneTransform>> = rig
            .joints
            .iter()
            .map(|e| {
                let tr = Transform::from_matrix(
                    inverse * helper.compute_global_transform(*e).ok()?.to_matrix(),
                );
                Some(BoneTransform {
                    translation: tr.translation,
                    rotation: tr.rotation,
                    scale: tr.scale,
                })
            })
            .collect();
        let Some(mut pose) = sampled else { continue };
        if let Some(mut fallen) = ragdoll {
            if fallen.rig.is_none() {
                let Ok(at) = helper.compute_global_transform(root) else {
                    continue;
                };
                fallen.rig = crate::character::ragdoll::RagdollRig::new(
                    &rig.bones,
                    rig.last_pose.as_deref().unwrap_or(&pose),
                    matrix,
                    at.compute_transform(),
                    fallen.velocity,
                );
            }
            if let Some(fallen) = &fallen.rig {
                pose = fallen.pose(
                    matrix,
                    if super::paused(physics_clock.as_deref()) {
                        1.0
                    } else {
                        fixed.overstep_fraction()
                    },
                );
                result.downed = true;
            }
        } else if let Some(seat) = seated {
            if seat.anim == Anim::Ride
                && state.current == Anim::Ride
                && roster
                    .skins
                    .get(state.skin)
                    .is_some_and(|s| !s.has(Anim::Ride))
            {
                let Ok(seats) = cars.get(seat.vehicle) else {
                    continue;
                };
                let Some(spec) = seats.0.first() else {
                    continue;
                };
                let Ok(car) = helper.compute_global_transform(seat.vehicle) else {
                    continue;
                };
                let car = car.compute_transform();
                let (position, rotation) =
                    crate::vehicle::spec::seat_pose(car.translation, car.rotation, spec);
                let seat_to_model = inverse * Mat4::from_rotation_translation(rotation, position);
                rig.ride.fit(
                    &mut pose,
                    &rig.bones,
                    seat_to_model,
                    &seat.hands,
                    &seat.feet,
                );
                result.riding = true;
            }
        } else if !distant
            && controller.is_some()
            && let Some(traversal) = traversal
        {
            let Ok(root_pose) = helper.compute_global_transform(root) else {
                continue;
            };
            let context = contacts::FitContext {
                climbing: matches!(traversal.mode, Anim::LadderEnter | Anim::LadderClimb),
                exiting: traversal.mode == Anim::LadderExit,
                ladder: traversal
                    .ladder
                    .and_then(|e| ladders.get(e).ok())
                    .map(|l| (l.bottom, l.facing)),
            };
            // dt=0 preserves existing clearance while paused; sampling the
            // same paused clip also prevents correction from accumulating.
            let dt = if !controls.enabled || super::paused(physics_clock.as_deref()) {
                0.0
            } else {
                clock.delta_secs()
            };
            let LiveRig { contact, bones, .. } = &mut *rig;
            result.contacts = contacts::fit_pose(
                contact,
                &mut pose,
                bones,
                matrix,
                root_pose.translation(),
                context,
                &|from, dir, length| physics.cast_ray_excluding(from, dir, length, root),
                &|hit| physics.static_surface(hit.entity) && hit.normal.y.abs() <= 0.2,
                dt,
            );
        }
        if !result.riding && !result.contacts && !result.downed {
            rig.last_pose = None;
            continue;
        }
        let worlds: Vec<Mat4> = pose
            .iter()
            .map(|p| {
                matrix * Mat4::from_scale_rotation_translation(p.scale, p.rotation, p.translation)
            })
            .collect();
        let locals: Option<Vec<Transform>> = rig
            .joints
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let parent = parents.get(*e).ok()?.parent();
                let parent_world = if rig.bones[i].parent >= 0 {
                    worlds[rig.bones[i].parent as usize]
                } else {
                    helper.compute_global_transform(parent).ok()?.to_matrix()
                };
                Some(Transform::from_matrix(parent_world.inverse() * worlds[i]))
            })
            .collect();
        let Some(locals) = locals else { continue };
        let mut write = transforms.p1();
        let originals: Option<Vec<_>> = rig
            .joints
            .iter()
            .map(|e| write.get(*e).ok().copied())
            .collect();
        let Some(originals) = originals else { continue };
        for (e, local) in rig.joints.iter().zip(locals) {
            *write.get_mut(*e).unwrap() = local;
        }
        rig.originals = originals;
        rig.last_pose = Some(pose);
        result.frames += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use avian3d::prelude::{Collider, RigidBody};
    use bevy::{asset::AssetPlugin, time::TimeUpdateStrategy};
    use std::time::Duration;

    #[test]
    fn live_wall_fit_runs_before_propagation_and_restores_after_leaving() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            TransformPlugin,
            AssetPlugin::default(),
            crate::physics::EarthPhysicsPlugin,
            PosePlugin,
        ))
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            1. / 60.,
        )))
        .insert_resource(Controls { enabled: true })
        .init_resource::<Roster>();
        let root = app
            .world_mut()
            .spawn((
                Transform::from_xyz(3., 0., 2.).with_rotation(Quat::from_rotation_y(0.5)),
                CharacterController::standing(0.3, 1.8, 0.3),
                Traversal::default(),
            ))
            .id();
        let body = app
            .world_mut()
            .spawn((
                Body,
                State::default(),
                Transform::from_scale(Vec3::splat(1.2)),
                ChildOf(root),
            ))
            .id();
        let scene = app
            .world_mut()
            .spawn((Transform::default(), ChildOf(body)))
            .id();
        let pelvis = app
            .world_mut()
            .spawn((
                Name::new("pelvis"),
                Transform::from_xyz(0., 0.8, 0.),
                ChildOf(scene),
            ))
            .id();
        let head = app
            .world_mut()
            .spawn((
                Name::new("head"),
                Transform::from_xyz(0.7, 0.8, 0.),
                ChildOf(pelvis),
            ))
            .id();
        let mesh = app
            .world_mut()
            .spawn(SkinnedMesh {
                inverse_bindposes: default(),
                joints: vec![pelvis, head],
            })
            .id();
        app.world_mut().entity_mut(body).insert((
            ModelScene {
                path: "test".into(),
                root: scene,
            },
            MeshEntities(vec![Some(mesh)]),
        ));
        let root_matrix = app.world().get::<Transform>(root).unwrap().to_matrix();
        let wall = app
            .world_mut()
            .spawn((
                RigidBody::Static,
                Collider::cuboid(0.2, 5., 4.),
                Transform::from_translation(root_matrix.transform_point3(Vec3::new(0.8, 1.5, 0.)))
                    .with_rotation(Quat::from_rotation_y(0.5)),
            ))
            .id();
        app.finish();
        app.cleanup();
        for _ in 0..5 {
            app.update();
        }
        assert!(
            app.world().get::<PoseFit>(body).unwrap().contacts,
            "wall did not reach the live pose adapter"
        );
        let sample = |app: &App| {
            root_matrix.inverse().transform_point3(
                app.world()
                    .get::<GlobalTransform>(head)
                    .unwrap()
                    .translation(),
            )
        };
        let fitted = sample(&app);
        assert!(
            fitted.x <= 0.7 - 0.18 * 1.2 + 0.001,
            "head still inside wall: {fitted:?}"
        );
        app.world_mut().resource_mut::<Controls>().enabled = false;
        for _ in 0..10 {
            app.update();
        }
        assert!(
            sample(&app).distance(fitted) < 1e-4,
            "paused pose accumulated corrections"
        );
        app.world_mut().despawn(wall);
        app.world_mut().resource_mut::<Controls>().enabled = true;
        for _ in 0..90 {
            app.update();
        }
        assert!(!app.world().get::<PoseFit>(body).unwrap().contacts);
        assert!(
            sample(&app).distance(Vec3::new(0.84, 1.92, 0.)) < 1e-4,
            "leaving wall kept a stale pose"
        );
        assert_eq!(
            app.world().get::<Transform>(head).unwrap().translation,
            Vec3::new(0.7, 0.8, 0.)
        );
    }
}
