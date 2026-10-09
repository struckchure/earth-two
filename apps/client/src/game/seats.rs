//! Vehicle offer/seat bridge: vehicle/drive.go and character/seat.go.
use crate::{
    character::{
        self, Anim, Body, CAPSULE_HEIGHT, CAPSULE_RADIUS, CharacterController, ControllerState,
        Intent, Player, State, Traversal,
    },
    presentation::{AnimationPlayer, MotionSamples, Roster, animate::play},
    vehicle::{
        self, Controls, Drivable, Driving, EnteredVehicle, LeftVehicle, Prompt, Seats,
        handling::SeatAnim,
    },
};
use avian3d::prelude::*;
use bevy::prelude::*;
#[derive(Component, Clone)]
pub struct Seated {
    pub vehicle: Entity,
    pub anim: Anim,
    pub hidden: bool,
    pub hands: Vec<Vec3>,
    pub feet: Vec<Vec3>,
}

pub fn driving_input(
    keys: Option<Res<ButtonInput<KeyCode>>>,
    controls: Res<character::Controls>,
    driving: Res<Driving>,
    mut cars: Query<&mut Controls>,
) {
    let Some(keys) = keys else { return };
    for mut input in &mut cars {
        *input = default();
    }
    if !controls.enabled {
        return;
    }
    let Some(e) = driving.vehicle else { return };
    let Ok(mut input) = cars.get_mut(e) else {
        return;
    };
    input.forward = f32::from(keys.any_pressed([KeyCode::KeyW, KeyCode::ArrowUp]))
        - f32::from(keys.any_pressed([KeyCode::KeyS, KeyCode::ArrowDown]));
    input.steer = f32::from(keys.any_pressed([KeyCode::KeyD, KeyCode::ArrowRight]))
        - f32::from(keys.any_pressed([KeyCode::KeyA, KeyCode::ArrowLeft]));
    input.hand_brake = keys.pressed(KeyCode::Space);
    input.right = keys.pressed(KeyCode::KeyR);
    input.exit = keys.just_pressed(KeyCode::KeyE);
    input.headlamps = keys.just_pressed(KeyCode::KeyH);
}
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn offer(
    mut commands: Commands,
    mut players: Query<
        (
            Entity,
            &mut Intent,
            &CharacterController,
            &Traversal,
            &Transform,
        ),
        With<Player>,
    >,
    mut cars: Query<(Entity, &mut Drivable, &Seats, &Transform), Without<Player>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    controls: Res<character::Controls>,
    mut prompt: ResMut<Prompt>,
    mut driving: ResMut<Driving>,
    mut entered: MessageWriter<EnteredVehicle>,
) {
    if !controls.enabled || driving.active() {
        return;
    }
    let Ok((player, mut intent, cc, traversal, tr)) = players.single_mut() else {
        return;
    };
    if !cc.grounded || traversal.active() {
        return;
    }
    let feet = tr.translation - Vec3::Y * cc.height * 0.5;
    let best = vehicle::nearest_seat(feet, cars.iter());
    let Some(best) = best else { return };
    let (e, mut d, seats, tr) = cars.get_mut(best).unwrap();
    prompt.key = "E".into();
    prompt.text = format!("Drive the {}", d.name);
    if keys.is_some_and(|k| k.just_pressed(KeyCode::KeyE)) {
        *intent = default();
        vehicle::enter(
            &mut commands,
            e,
            &mut d,
            seats,
            tr,
            player,
            &mut driving,
            &mut entered,
        );
    }
}
#[allow(clippy::type_complexity)]
pub fn receive(
    mut commands: Commands,
    mut entered: MessageReader<EnteredVehicle>,
    mut left: MessageReader<LeftVehicle>,
    roots: Query<&Children, With<Player>>,
    mut bodies: Query<(&mut Transform, &mut State, &mut AnimationPlayer), With<Body>>,
    roster: Res<Roster>,
) {
    for event in entered.read() {
        commands
            .entity(event.driver)
            .remove::<(
                CharacterController,
                ControllerState,
                Collider,
                RigidBody,
                Position,
                Rotation,
                LinearVelocity,
                AngularVelocity,
            )>()
            .insert((
                Seated {
                    vehicle: event.vehicle,
                    anim: if event.anim == SeatAnim::Ride {
                        Anim::Ride
                    } else {
                        Anim::Drive
                    },
                    hidden: event.hidden,
                    hands: event.hands.clone(),
                    feet: event.feet.clone(),
                },
                Intent::default(),
                Traversal::default(),
            ));
    }
    for event in left.read() {
        let centre = event.feet + Vec3::Y * CAPSULE_HEIGHT * 0.5;
        commands.entity(event.driver).remove::<Seated>().insert((
            CharacterController::standing(CAPSULE_RADIUS, CAPSULE_HEIGHT, 0.3),
            Transform::from_translation(centre),
            MotionSamples::at(centre, CAPSULE_HEIGHT),
            Intent::default(),
            Traversal::default(),
        ));
        if let Ok(children) = roots.get(event.driver) {
            for child in children {
                if let Ok((mut tr, mut st, mut animation)) = bodies.get_mut(*child) {
                    tr.translation = Vec3::NEG_Y * CAPSULE_HEIGHT * 0.5;
                    tr.rotation = Quat::from_rotation_y(event.yaw);
                    st.current = Anim::Idle;
                    st.hidden = false;
                    if let Some(skin) = roster.skins.get(st.skin) {
                        play(&mut animation, &skin.clip(Anim::Idle), false);
                        animation.fade_in(0.3);
                    }
                    #[cfg(feature = "viewer")]
                    commands.entity(*child).insert(Visibility::Inherited);
                }
            }
        }
    }
}
#[allow(clippy::type_complexity)]
pub fn sit(
    mut commands: Commands,
    mut roots: Query<
        (&Seated, &mut Transform, &mut Intent, &Children),
        (With<Player>, Without<Body>, Without<Drivable>),
    >,
    cars: Query<(&Transform, &Seats), (With<Drivable>, Without<Player>, Without<Body>)>,
    mut bodies: Query<
        (&mut Transform, &mut State, &mut AnimationPlayer),
        (With<Body>, Without<Player>, Without<Drivable>),
    >,
    roster: Res<Roster>,
    controls: Res<character::Controls>,
) {
    let _ = &mut commands;
    for (seat, mut root, mut intent, children) in &mut roots {
        let Ok((car, seats)) = cars.get(seat.vehicle) else {
            continue;
        };
        let Some(spec) = seats.0.first() else {
            continue;
        };
        let (position, rotation) = vehicle::spec::seat_pose(car.translation, car.rotation, spec);
        root.translation = position + rotation * Vec3::Y * CAPSULE_HEIGHT * 0.5;
        root.rotation = rotation;
        *intent = default();
        for child in children {
            if let Ok((mut tr, mut st, mut animation)) = bodies.get_mut(*child) {
                tr.translation = Vec3::NEG_Y * CAPSULE_HEIGHT * 0.5;
                tr.rotation = Quat::IDENTITY;
                st.turn = 0.;
                st.air = 0.;
                st.aloft = false;
                st.stairs = 0;
                st.stairs_left = 0.;
                animation.manual_time = false;
                animation.speed = 1.;
                if st.current != seat.anim {
                    st.current = seat.anim;
                    if let Some(skin) = roster.skins.get(st.skin) {
                        let anim = if seat.anim == Anim::Ride && !skin.has(Anim::Ride) {
                            Anim::Drive
                        } else {
                            seat.anim
                        };
                        play(&mut animation, &skin.clip(anim), false);
                        animation.fade_in(0.3);
                    }
                }
                animation.paused = !controls.enabled;
                st.hidden = seat.hidden;
                #[cfg(feature = "viewer")]
                commands.entity(*child).insert(if seat.hidden {
                    Visibility::Hidden
                } else {
                    Visibility::Inherited
                });
            }
        }
    }
}
